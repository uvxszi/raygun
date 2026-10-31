use std::fs;
use std::path::Path;

use sqlx::Row;
use tempfile::TempDir;

use super::*;

async fn open_in(dir: &Path) -> Db {
    Db::open(&dir.join("raygun.db")).await.unwrap()
}

#[tokio::test]
async fn creates_the_database_and_applies_migrations() {
    let temp = TempDir::new().unwrap();

    let db = open_in(temp.path()).await;

    assert!(temp.path().join("raygun.db").is_file());
    assert!(settings::load(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn device_id_is_stable_across_reopen() {
    let temp = TempDir::new().unwrap();
    let first = open_in(temp.path()).await;
    let device_id = first.device_id();
    first.close().await;

    let second = open_in(temp.path()).await;

    assert_eq!(second.device_id(), device_id);
    assert_eq!(device_id.get_version_num(), 7);
}

#[tokio::test]
async fn settings_round_trip_with_sync_metadata() {
    let temp = TempDir::new().unwrap();
    let db = open_in(temp.path()).await;

    settings::put(&db, "theme", "\"dark\"").await.unwrap();
    settings::put(&db, "theme", "\"light\"").await.unwrap();

    assert_eq!(
        settings::load(&db).await.unwrap(),
        vec![("theme".to_owned(), "\"light\"".to_owned())]
    );
    let row = sqlx::query("SELECT updated_at_hlc, origin_device, deleted FROM setting")
        .fetch_one(db.pool())
        .await
        .unwrap();
    assert!(row.get::<i64, _>("updated_at_hlc") > 0);
    assert_eq!(
        row.get::<String, _>("origin_device"),
        db.device_id().to_string()
    );
    assert_eq!(row.get::<i64, _>("deleted"), 0);
}

#[tokio::test]
async fn clock_resumes_after_reopen() {
    let temp = TempDir::new().unwrap();
    let first = open_in(temp.path()).await;
    settings::put(&first, "theme", "\"dark\"").await.unwrap();
    let before = first.clock.lock().unwrap().last();
    first.close().await;

    let second = open_in(temp.path()).await;

    assert!(second.tick() > before);
}

#[tokio::test]
async fn data_survives_moving_the_data_folder() {
    let temp = TempDir::new().unwrap();
    let original = temp.path().join("usb-a/raygun-data");
    fs::create_dir_all(&original).unwrap();
    let db = open_in(&original).await;
    let device_id = db.device_id();
    settings::put(&db, "language", "\"ja\"").await.unwrap();
    db.close().await;

    let moved = temp.path().join("usb-b/raygun-data");
    fs::create_dir_all(&moved).unwrap();
    for entry in fs::read_dir(&original).unwrap() {
        let entry = entry.unwrap();
        fs::copy(entry.path(), moved.join(entry.file_name())).unwrap();
    }
    fs::remove_dir_all(temp.path().join("usb-a")).unwrap();
    let reopened = open_in(&moved).await;

    assert_eq!(reopened.device_id(), device_id);
    assert_eq!(
        settings::load(&reopened).await.unwrap(),
        vec![("language".to_owned(), "\"ja\"".to_owned())]
    );
}
