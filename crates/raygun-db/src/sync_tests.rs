use tempfile::TempDir;

use super::*;
use crate::collections::{self, TrackFields};

async fn open(temp: &TempDir, name: &str) -> Db {
    Db::open(&temp.path().join(format!("{name}.db")))
        .await
        .unwrap()
}

fn song(key: &str) -> TrackFields {
    TrackFields {
        track_key: key.into(),
        title: key.rsplit('|').next().unwrap().into(),
        artist: Some(key.split('|').next().unwrap().into()),
        album: None,
        duration_ms: Some(1000),
        artwork_url: None,
        source: None,
    }
}

/// Everything `from` has, merged into `into`, in a new session. Returns what was applied.
async fn push(from: &Db, into: &Db) -> (i64, Applied) {
    let (rows, _) = changes_since(from, 0, &SyncKind::ALL, 10_000, None)
        .await
        .unwrap();
    let session = start_session(into, "peer", "Peer", 0).await.unwrap();
    let applied = apply(into, session, &rows, &SyncKind::ALL, "peer")
        .await
        .unwrap();
    (session, applied)
}

fn favorite_titles(rows: &[collections::FavoriteRow]) -> Vec<&str> {
    rows.iter().map(|row| row.track.title.as_str()).collect()
}

#[tokio::test]
async fn the_change_log_keeps_the_latest_change_per_row() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a").await;
    let before = latest_seq(&db).await.unwrap();
    collections::set_favorite(&db, &song("a|one"), true, 1)
        .await
        .unwrap();
    collections::set_favorite(&db, &song("a|one"), false, 2)
        .await
        .unwrap();
    let id = collections::create_playlist(&db, "Mix", 3).await.unwrap();

    let (rows, last) = changes_since(&db, before, &SyncKind::ALL, 100, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 2, "one entry per row, not per write");
    assert_eq!(rows[0].table, "music_favorite");
    assert_eq!(rows[0].row["deleted"], 1, "the latest state travels");
    assert_eq!(rows[1].row["id"], id.as_str());
    assert_eq!(last, latest_seq(&db).await.unwrap());

    let (library_only, last_filtered) = changes_since(&db, before, &[SyncKind::History], 100, None)
        .await
        .unwrap();
    assert!(library_only.is_empty());
    assert_eq!(last_filtered, last, "skipped kinds still move the seq on");
}

#[tokio::test]
async fn merging_keeps_the_newest_write_of_each_row() {
    let temp = TempDir::new().unwrap();
    let (a, b) = (open(&temp, "a").await, open(&temp, "b").await);
    let id = collections::create_playlist(&a, "Road trip", 1)
        .await
        .unwrap();
    collections::add_to_playlist(&a, &id, &[song("x|one")], 2)
        .await
        .unwrap();
    let (_, applied) = push(&a, &b).await;
    assert_eq!(
        applied,
        Applied {
            received: 2,
            changed: 2
        }
    );
    assert_eq!(collections::playlist_items(&b, &id).await.unwrap().len(), 1);

    // b renames after seeing a's write: b's stamp is newer, so a takes it and b ignores a's old row.
    assert!(
        collections::rename_playlist(&b, &id, "Roadtrip!")
            .await
            .unwrap()
    );
    let (_, again) = push(&a, &b).await;
    assert_eq!(again.changed, 0, "older rows change nothing");
    push(&b, &a).await;
    assert_eq!(
        collections::playlist(&a, &id).await.unwrap().unwrap().name,
        "Roadtrip!"
    );

    // Deleting on a reaches b as a tombstone, items included.
    collections::delete_playlist(&a, &id).await.unwrap();
    push(&a, &b).await;
    assert!(collections::playlist(&b, &id).await.unwrap().is_none());
    assert!(
        collections::playlist_items(&b, &id)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn equal_stamps_are_broken_by_device_id() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a").await;
    let spec = table("setting").unwrap();
    let row = |origin: &str, value: &str| SyncRow {
        table: spec.name.into(),
        row: serde_json::json!({
            "key": "theme", "value": format!("\"{value}\""), "updated_at_hlc": 500,
            "origin_device": origin, "deleted": 0,
        }),
    };
    let session = start_session(&db, "p", "P", 0).await.unwrap();
    apply(&db, session, &[row("bbbb", "dark")], &SyncKind::ALL, "peer")
        .await
        .unwrap();
    let lower = apply(
        &db,
        session,
        &[row("aaaa", "light")],
        &SyncKind::ALL,
        "peer",
    )
    .await
    .unwrap();
    assert_eq!(lower.changed, 0);
    let higher = apply(
        &db,
        session,
        &[row("cccc", "amoled")],
        &SyncKind::ALL,
        "peer",
    )
    .await
    .unwrap();
    assert_eq!(higher.changed, 1);
    let settings = crate::settings::load(&db).await.unwrap();
    assert_eq!(
        settings,
        vec![("theme".to_owned(), "\"amoled\"".to_owned())]
    );

    // The clock moved past the remote stamp, so the next local write wins.
    crate::settings::put(&db, "theme", "\"system\"")
        .await
        .unwrap();
    let (rows, _) = changes_since(&db, 0, &SyncKind::ALL, 10, None)
        .await
        .unwrap();
    assert!(rows[0].row["updated_at_hlc"].as_i64().unwrap() > 500);
}

#[tokio::test]
async fn two_devices_favoriting_one_song_settle_on_one_row() {
    let temp = TempDir::new().unwrap();
    let (a, b) = (open(&temp, "a").await, open(&temp, "b").await);
    collections::set_favorite(&a, &song("nina simone|sinnerman"), true, 1)
        .await
        .unwrap();
    collections::set_favorite(&b, &song("nina simone|sinnerman"), true, 2)
        .await
        .unwrap();

    push(&a, &b).await;
    push(&b, &a).await;
    push(&a, &b).await;

    let (on_a, on_b) = (
        collections::favorites(&a).await.unwrap(),
        collections::favorites(&b).await.unwrap(),
    );
    assert_eq!(favorite_titles(&on_a), ["sinnerman"]);
    assert_eq!(on_a.len(), 1);
    assert_eq!(on_b.len(), 1);
    assert_eq!(on_a[0].id, on_b[0].id, "both keep the same row");
}

#[tokio::test]
async fn undo_restores_what_a_session_changed() {
    let temp = TempDir::new().unwrap();
    let (a, b) = (open(&temp, "a").await, open(&temp, "b").await);
    let id = collections::create_playlist(&b, "Mine", 1).await.unwrap();
    push(&b, &a).await;
    assert!(
        collections::rename_playlist(&a, &id, "Theirs")
            .await
            .unwrap()
    );
    collections::set_favorite(&a, &song("x|new"), true, 2)
        .await
        .unwrap();

    let (session, applied) = push(&a, &b).await;
    assert_eq!(applied.changed, 2);
    assert_eq!(
        collections::playlist(&b, &id).await.unwrap().unwrap().name,
        "Theirs"
    );

    let seq_before = latest_seq(&b).await.unwrap();
    assert_eq!(undo(&b, session).await.unwrap(), 2);
    assert_eq!(
        collections::playlist(&b, &id).await.unwrap().unwrap().name,
        "Mine"
    );
    assert!(
        collections::favorites(&b).await.unwrap().is_empty(),
        "the new row is tombstoned"
    );
    let (restored, _) = changes_since(&b, seq_before, &SyncKind::ALL, 10, None)
        .await
        .unwrap();
    assert_eq!(
        restored.len(),
        2,
        "the undo travels to other devices as new writes"
    );
    assert!(sessions(&b, 1).await.unwrap()[0].undone);

    // Undoing wins over the rows it replaced when they come back.
    push(&b, &a).await;
    assert_eq!(
        collections::playlist(&a, &id).await.unwrap().unwrap().name,
        "Mine"
    );
}

#[tokio::test]
async fn fonts_stay_on_the_device_that_has_them() {
    let temp = TempDir::new().unwrap();
    let (a, b) = (open(&temp, "a").await, open(&temp, "b").await);
    crate::settings::put(&a, "fonts", r#"{"interface":"Inter","reading":""}"#)
        .await
        .unwrap();
    crate::settings::put(&a, "textSize", "20").await.unwrap();
    let (rows, _) = changes_since(&a, 0, &SyncKind::ALL, 10, None)
        .await
        .unwrap();
    let keys: Vec<_> = rows.iter().map(|row| row.row["key"].clone()).collect();
    assert_eq!(keys, ["textSize"], "the text size travels, the fonts don't");

    // Nor are they taken from a device that sends them anyway.
    let fonts = SyncRow {
        table: "setting".into(),
        row: serde_json::json!({
            "key": "fonts", "value": r#"{"interface":"Inter","reading":""}"#,
            "updated_at_hlc": 500, "origin_device": "aaaa", "deleted": 0,
        }),
    };
    let session = start_session(&b, "a", "A", 0).await.unwrap();
    let applied = apply(&b, session, &[fonts], &SyncKind::ALL, "peer")
        .await
        .unwrap();
    assert_eq!(applied.received, 0);
    assert!(crate::settings::load(&b).await.unwrap().is_empty());
}

#[tokio::test]
async fn disabled_kinds_are_neither_sent_nor_taken() {
    let temp = TempDir::new().unwrap();
    let (a, b) = (open(&temp, "a").await, open(&temp, "b").await);
    collections::record_play(&a, &song("x|heard"), 5)
        .await
        .unwrap();
    collections::set_favorite(&a, &song("x|loved"), true, 5)
        .await
        .unwrap();
    let (rows, _) = changes_since(&a, 0, &SyncKind::ALL, 10, None)
        .await
        .unwrap();
    let session = start_session(&b, "a", "A", 0).await.unwrap();
    let applied = apply(&b, session, &rows, &[SyncKind::Library], "peer")
        .await
        .unwrap();
    assert_eq!(applied.received, 1);
    assert!(
        collections::recent_history(&b, 10)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(collections::favorites(&b).await.unwrap().len(), 1);
}

#[tokio::test]
async fn peers_and_sessions() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a").await;
    let mut tablet = Peer {
        device_id: "dev-b".into(),
        name: "Tablet".into(),
        public_key: "ab".repeat(32),
        address: Some("10.0.2.2:8830".into()),
        paired_at: 1,
        last_sync_at: None,
        sent_seq: 0,
        kinds: vec![],
    };
    save_peer(&db, &tablet).await.unwrap();
    set_sent_seq(&db, "dev-b", 7, &[]).await.unwrap();
    set_sent_seq(&db, "dev-b", 3, &[]).await.unwrap();
    assert_eq!(
        peers(&db).await.unwrap()[0].sent_seq,
        7,
        "watermarks only move forward"
    );
    peer_seen(&db, "dev-b", Some("192.168.1.9:8831"), Some(99))
        .await
        .unwrap();
    let seen = peer(&db, "dev-b").await.unwrap().unwrap();
    assert_eq!(seen.address.as_deref(), Some("192.168.1.9:8831"));
    assert_eq!(seen.last_sync_at, Some(99));

    tablet.name = "Tablet 2".into();
    save_peer(&db, &tablet).await.unwrap();
    assert_eq!(
        peers(&db).await.unwrap()[0].sent_seq,
        0,
        "re-pairing sends everything again"
    );
    set_sent_seq(&db, "dev-b", 5, &SyncKind::ALL).await.unwrap();
    assert_eq!(peers(&db).await.unwrap()[0].kinds, SyncKind::ALL);

    let session = start_session(&db, "dev-b", "Tablet", 10).await.unwrap();
    finish_session(&db, session, 3, 4, 2, Some("lost connection"), 20)
        .await
        .unwrap();
    let logged = sessions(&db, 5).await.unwrap();
    assert_eq!(logged[0].received, 4);
    assert_eq!(logged[0].error.as_deref(), Some("lost connection"));

    remove_peer(&db, "dev-b").await.unwrap();
    assert!(peers(&db).await.unwrap().is_empty());
}
