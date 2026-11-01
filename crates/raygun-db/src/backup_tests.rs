use tempfile::TempDir;

use super::*;
use crate::titles::{self, TitleFields, UnitFields};

async fn open(temp: &TempDir, name: &str) -> Db {
    Db::open(&temp.path().join(name)).await.unwrap()
}

fn manga(remote_id: &str, title: &str) -> TitleFields {
    TitleFields {
        kind: "manga".into(),
        source_id: "dev.raygun.mangadex/mangadex".into(),
        remote_id: remote_id.into(),
        title: title.into(),
        cover: None,
        cover_headers: [("User-Agent".to_owned(), "Raygun".to_owned())].into(),
        authors: vec!["Yamada".into()],
        artists: vec![],
        description: None,
        genres: vec!["Fantasy".into()],
        status: "ongoing".into(),
        url: None,
        media_type: None,
        year: None,
    }
}

fn chapter(remote_id: &str, number: f64) -> UnitFields {
    UnitFields {
        remote_id: remote_id.into(),
        name: format!("Chapter {number}"),
        number: Some(number),
        volume: None,
        scanlator: None,
        date: None,
    }
}

/// A library title with two chapters, the first read and the second half read.
async fn reader_state(db: &Db) -> String {
    let id = titles::upsert_title(db, &manga("m1", "Frieren"), 1)
        .await
        .unwrap();
    titles::set_in_library(db, &id, Some(5)).await.unwrap();
    titles::set_reader_mode(db, &id, "pagedRtl").await.unwrap();
    titles::sync_units(
        db,
        &id,
        &[chapter("c2", 2.0), chapter("c1", 1.0)],
        "chapter",
        2,
    )
    .await
    .unwrap();
    let units = titles::units(db, &id).await.unwrap();
    titles::set_completed(db, &units[1].id, true, 6)
        .await
        .unwrap();
    titles::set_progress(db, &units[0].id, 4, Some(20), 7)
        .await
        .unwrap();
    id
}

#[tokio::test]
async fn a_backup_restores_into_an_empty_database() {
    let temp = TempDir::new().unwrap();
    let source = open(&temp, "a.db").await;
    let title_id = reader_state(&source).await;
    crate::settings::put(&source, "theme", "\"amoled\"")
        .await
        .unwrap();

    let backup = export(&source, 99).await.unwrap();
    assert_eq!((backup.format.as_str(), backup.version), (FORMAT, VERSION));
    assert_eq!(backup.live_rows("title"), 1);
    assert_eq!(backup.live_rows("unit"), 2);
    // Through text, as it travels in a file.
    let backup: BackupData =
        serde_json::from_str(&serde_json::to_string(&backup).unwrap()).unwrap();

    let target = open(&temp, "b.db").await;
    let restored = restore(&target, &backup).await.unwrap();
    assert!(restored.changed >= 5, "{restored:?}");

    let title = titles::title(&target, &title_id).await.unwrap().unwrap();
    assert_eq!(title.fields.title, "Frieren");
    assert_eq!(title.fields.cover_headers["User-Agent"], "Raygun");
    assert_eq!(
        (title.in_library_at, title.reader_mode.as_str()),
        (Some(5), "pagedRtl")
    );
    let units = titles::units(&target, &title_id).await.unwrap();
    assert_eq!(units[1].completed_at, Some(6));
    assert_eq!((units[0].position, units[0].total), (Some(4), Some(20)));
    assert_eq!(
        crate::settings::load(&target).await.unwrap(),
        [("theme".to_owned(), "\"amoled\"".to_owned())]
    );

    let again = restore(&target, &backup).await.unwrap();
    assert_eq!(again.changed, 0, "restoring twice changes nothing");
}

#[tokio::test]
async fn restore_keeps_newer_local_rows_and_adopts_local_ids() {
    let temp = TempDir::new().unwrap();
    let source = open(&temp, "a.db").await;
    let backup_title = reader_state(&source).await;
    crate::settings::put(&source, "theme", "\"light\"")
        .await
        .unwrap();
    let backup = export(&source, 99).await.unwrap();

    // The other device fetched the same title on its own (another id), read chapter 2 to the
    // end, and changed the theme after the backup was made.
    let target = open(&temp, "b.db").await;
    let local_title = titles::upsert_title(&target, &manga("m1", "Frieren"), 1)
        .await
        .unwrap();
    assert_ne!(local_title, backup_title);
    titles::sync_units(&target, &local_title, &[chapter("c2", 2.0)], "chapter", 3)
        .await
        .unwrap();
    let c2 = titles::units(&target, &local_title).await.unwrap()[0]
        .id
        .clone();
    target.observe(source.tick());
    titles::set_progress(&target, &c2, 19, Some(20), 8)
        .await
        .unwrap();
    crate::settings::put(&target, "theme", "\"dark\"")
        .await
        .unwrap();

    restore(&target, &backup).await.unwrap();

    assert!(
        titles::title(&target, &backup_title)
            .await
            .unwrap()
            .is_none(),
        "the backup's title took the local id"
    );
    let title = titles::title(&target, &local_title).await.unwrap().unwrap();
    assert_eq!(
        title.in_library_at,
        Some(5),
        "the local row is newer, but its library gap is filled"
    );
    let units = titles::units(&target, &local_title).await.unwrap();
    let names: Vec<_> = units
        .iter()
        .map(|unit| unit.fields.remote_id.as_str())
        .collect();
    assert_eq!(names, ["c2", "c1"], "chapters joined the local title");
    let c2 = units.iter().find(|unit| unit.id == c2).unwrap();
    assert_eq!(
        (c2.position, c2.completed_at),
        (Some(19), Some(8)),
        "newer local progress stays"
    );
    assert_eq!(
        crate::settings::load(&target).await.unwrap(),
        [("theme".to_owned(), "\"dark\"".to_owned())],
        "newer local setting stays"
    );
}

#[tokio::test]
async fn unknown_tables_are_ignored() {
    let temp = TempDir::new().unwrap();
    let db = open(&temp, "a.db").await;
    let mut backup = export(&db, 1).await.unwrap();
    backup.tables.insert(
        "from_the_future".into(),
        vec![serde_json::json!({"id": "x"})],
    );
    restore(&db, &backup).await.unwrap();
}
