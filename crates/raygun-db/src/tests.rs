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

#[tokio::test]
async fn extension_repos_settings_and_installs_round_trip() {
    let temp = TempDir::new().unwrap();
    let db = open_in(temp.path()).await;

    // The official repository is seeded.
    let seeded = extensions::repos(&db).await.unwrap();
    assert_eq!(seeded.len(), 1);
    assert!(seeded[0].url.ends_with("/repo/index.json"));

    let added = extensions::add_repo(&db, "https://example.org/index.json", "Example")
        .await
        .unwrap();
    assert_eq!(added.id.len(), 36);
    extensions::rename_repo(&db, &added.id, "Renamed")
        .await
        .unwrap();
    let repos = extensions::repos(&db).await.unwrap();
    assert_eq!(repos.len(), 2);
    assert_eq!(
        repos[0].id, seeded[0].id,
        "the official repository stays first"
    );
    assert_eq!(repos[1].name, "Renamed");

    extensions::remove_repo(&db, &added.id).await.unwrap();
    assert_eq!(extensions::repos(&db).await.unwrap().len(), 1);
    // Adding the same URL again revives the row instead of duplicating it.
    let revived = extensions::add_repo(&db, "https://example.org/index.json", "Example")
        .await
        .unwrap();
    assert_eq!(revived.id, added.id);

    extensions::put_setting(&db, "dev.example.a", "region", "\"eu\"")
        .await
        .unwrap();
    extensions::put_setting(&db, "dev.example.a", "region", "\"us\"")
        .await
        .unwrap();
    assert_eq!(
        extensions::settings(&db).await.unwrap(),
        vec![("dev.example.a".into(), "region".into(), "\"us\"".into())]
    );

    let install = extensions::InstallRow {
        extension_id: "dev.example.a".into(),
        repo_url: "https://example.org/index.json".into(),
        version: "1.0.0".into(),
        sha256: "ab".repeat(32),
        installed_at: 1,
    };
    extensions::record_install(&db, &install).await.unwrap();
    extensions::record_install(
        &db,
        &extensions::InstallRow {
            version: "1.1.0".into(),
            ..install.clone()
        },
    )
    .await
    .unwrap();
    let installs = extensions::installs(&db).await.unwrap();
    assert_eq!(installs.len(), 1);
    assert_eq!(installs[0].version, "1.1.0");
    extensions::remove_install(&db, "dev.example.a")
        .await
        .unwrap();
    assert!(extensions::installs(&db).await.unwrap().is_empty());
}

#[tokio::test]
async fn stream_candidates_and_provider_stats_round_trip() {
    let temp = TempDir::new().unwrap();
    let db = open_in(temp.path()).await;
    let key = "louis armstrong|what a wonderful world";

    streams::mark_failed(&db, key, "dev.a/stream", "1", "Live", "HTTP 404", 10)
        .await
        .unwrap();
    streams::mark_verified(
        &db,
        key,
        "dev.b/stream",
        "2",
        "Studio",
        r#"{"kind":"url"}"#,
        Some(99),
        20,
    )
    .await
    .unwrap();
    // Verifying a candidate that failed before clears the failure.
    streams::mark_verified(
        &db,
        key,
        "dev.a/stream",
        "1",
        "Live",
        r#"{"kind":"url"}"#,
        None,
        30,
    )
    .await
    .unwrap();

    let rows = streams::candidates(&db, key).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].candidate_id, "1", "most recently verified first");
    assert_eq!(rows[0].failed_at, None);
    assert_eq!(rows[1].expires_at, Some(99));
    assert!(
        streams::candidates(&db, "other|key")
            .await
            .unwrap()
            .is_empty()
    );

    let stats = streams::provider_stats(&db).await.unwrap();
    assert_eq!(
        stats,
        vec![
            streams::ProviderStats {
                provider_id: "dev.a/stream".into(),
                successes: 1,
                failures: 1
            },
            streams::ProviderStats {
                provider_id: "dev.b/stream".into(),
                successes: 1,
                failures: 0
            },
        ]
    );
}

#[tokio::test]
async fn local_values_round_trip_and_never_touch_synced_settings() {
    let dir = TempDir::new().unwrap();
    let db = open_in(dir.path()).await;
    assert_eq!(local_value(&db, "mcp_token").await.unwrap(), None);
    set_local_value(&db, "mcp_token", "abc").await.unwrap();
    set_local_value(&db, "mcp_token", "def").await.unwrap();
    assert_eq!(
        local_value(&db, "mcp_token").await.unwrap().as_deref(),
        Some("def")
    );
    assert!(settings::load(&db).await.unwrap().is_empty());
}
