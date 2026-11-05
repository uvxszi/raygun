use sqlx::Row;
use tempfile::TempDir;

use super::*;
use crate::backup::tests::a_bit_of_everything;
use crate::backup::{PINS_KEY, export, restore};

async fn open(temp: &TempDir, name: &str) -> Db {
    Db::open(&temp.path().join(name)).await.unwrap()
}

async fn stamps(db: &Db, table: &str) -> Vec<(i64, i64, String)> {
    // The table name comes from the tests.
    sqlx::query(AssertSqlSafe(format!(
        "SELECT deleted, updated_at_hlc, origin_device FROM {table}"
    )))
    .fetch_all(db.pool())
    .await
    .unwrap()
    .iter()
    .map(|row| (row.get(0), row.get(1), row.get(2)))
    .collect()
}

#[tokio::test]
async fn clearing_a_synced_part_leaves_tombstones_that_travel() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a.db").await;
    a_bit_of_everything(&db).await;
    let before = crate::sync::latest_seq(&db).await.unwrap();
    let newest_before = stamps(&db, "music_playlist")
        .await
        .into_iter()
        .map(|(_, hlc, _)| hlc)
        .max()
        .unwrap();

    let changed = clear(&db, Part::Playlists).await.unwrap();
    assert_eq!(changed, 3, "a playlist, its song and a favorite");
    for table in ["music_playlist", "music_playlist_item", "music_favorite"] {
        for (deleted, hlc, device) in stamps(&db, table).await {
            assert_eq!(deleted, 1, "{table} keeps its rows as tombstones");
            assert!(hlc > newest_before, "stamped after the rows it removes");
            assert_eq!(device, db.device_id().to_string());
        }
    }
    // Each tombstone is a change device sync sends.
    let (rows, _) = crate::sync::changes_since(&db, before, &crate::sync::SyncKind::ALL, 100, None)
        .await
        .unwrap();
    assert_eq!(rows.len(), 3);
    assert!(!counts(&db).await.unwrap().contains_key(&Part::Playlists));
}

#[tokio::test]
async fn a_cleared_part_stays_cleared_when_an_older_backup_is_restored() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a.db").await;
    a_bit_of_everything(&db).await;
    let backup = export(&db, 9, &Part::ALL).await.unwrap();
    clear(&db, Part::Settings).await.unwrap();
    clear(&db, Part::History).await.unwrap();

    let settings = crate::settings::load(&db).await.unwrap();
    assert_eq!(settings.len(), 1, "the pins aren't settings here");
    assert_eq!(settings[0].0, PINS_KEY);

    // The clear is newer than every row the backup holds.
    restore(&db, &backup, &[Part::Settings]).await.unwrap();
    assert_eq!(crate::settings::load(&db).await.unwrap().len(), 1);
}

#[tokio::test]
async fn clearing_a_library_takes_its_titles_out_and_keeps_the_others() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a.db").await;
    a_bit_of_everything(&db).await;
    let category = crate::categories::create(&db, "manga", "Favorites")
        .await
        .unwrap();

    clear(&db, Part::Comics).await.unwrap();
    assert!(
        crate::titles::library(&db, "manga")
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(crate::titles::library(&db, "novel").await.unwrap().len(), 1);
    assert!(
        crate::categories::list(&db, "manga")
            .await
            .unwrap()
            .iter()
            .all(|kept| kept.id != category)
    );
    let counted = counts(&db).await.unwrap();
    assert!(!counted.contains_key(&Part::Comics));
    assert_eq!(counted[&Part::Books][&Item::Titles], 1);

    clear(&db, Part::Podcasts).await.unwrap();
    assert!(
        crate::podcasts::subscriptions(&db)
            .await
            .unwrap()
            .is_empty()
    );
}

#[tokio::test]
async fn parts_the_app_clears_change_nothing_here() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a.db").await;
    a_bit_of_everything(&db).await;
    for part in [Part::Pins, Part::Sources, Part::Downloads] {
        assert_eq!(clear(&db, part).await.unwrap(), 0);
    }
    assert_eq!(counts(&db).await.unwrap()[&Part::Pins][&Item::Pins], 2);
}
