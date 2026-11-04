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

    // Raygun's first repository is seeded (the app moves it to Paraline: `replace_repos`).
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
async fn replaced_repositories_fold_into_their_replacement() {
    const OFFICIAL: &str = "https://raw.githubusercontent.com/uvxszi/raygun/main/repo/index.json";
    const NOVELS: &str = "https://example.org/old/shosetsu/index.json";
    const SOURCES: &str = "https://example.org/old/raygun/index.json";
    let replacement = extensions::RepoRow {
        id: "0192a000-0000-7000-8000-000000000002".into(),
        url: "https://example.org/index.json".into(),
        name: "Paraline".into(),
    };
    let retired = [OFFICIAL, NOVELS, SOURCES];
    let install = |extension_id: &str, repo_url: &str| extensions::InstallRow {
        extension_id: extension_id.into(),
        repo_url: repo_url.into(),
        version: "1.0.0".into(),
        sha256: "ab".repeat(32),
        installed_at: 1,
    };

    // A new install: the seeded official repository becomes the replacement, which leads.
    let temp = TempDir::new().unwrap();
    let db = open_in(temp.path()).await;
    let other = extensions::add_repo(&db, "https://example.org/other/index.json", "Other")
        .await
        .unwrap();
    // Another spelling of a retired address counts too.
    extensions::add_repo(&db, &format!("{}/", NOVELS.to_uppercase()), "Novels")
        .await
        .unwrap();
    extensions::record_install(&db, &install("dev.raygun.deezer", OFFICIAL))
        .await
        .unwrap();
    extensions::record_install(&db, &install("dev.example.x", &other.url))
        .await
        .unwrap();
    assert!(
        extensions::replace_repos(&db, &retired, &replacement)
            .await
            .unwrap()
    );
    let repos = extensions::repos(&db).await.unwrap();
    assert_eq!(repos, vec![replacement.clone(), other.clone()]);
    let installs = extensions::installs(&db).await.unwrap();
    let from = |id: &str| {
        installs
            .iter()
            .find(|install| install.extension_id == id)
            .map(|install| install.repo_url.clone())
    };
    assert_eq!(
        from("dev.raygun.deezer").as_deref(),
        Some(replacement.url.as_str())
    );
    assert_eq!(from("dev.example.x"), Some(other.url.clone()));
    // Nothing more to do on the next start.
    assert!(
        !extensions::replace_repos(&db, &retired, &replacement)
            .await
            .unwrap()
    );
    // Removing the replacement stays removed.
    extensions::remove_repo(&db, &replacement.id).await.unwrap();
    assert!(
        !extensions::replace_repos(&db, &retired, &replacement)
            .await
            .unwrap()
    );
    assert_eq!(extensions::repos(&db).await.unwrap(), vec![other]);

    // A retired repository back from a backup revives the replacement, under its row.
    let revived = extensions::add_repo(&db, SOURCES, "Sources").await.unwrap();
    assert!(
        extensions::replace_repos(&db, &retired, &replacement)
            .await
            .unwrap()
    );
    let repos = extensions::repos(&db).await.unwrap();
    assert!(repos.iter().all(|repo| repo.id != revived.id));
    assert!(repos.contains(&replacement));

    // A device that had removed every retired repository doesn't get the replacement.
    let temp = TempDir::new().unwrap();
    let db = open_in(temp.path()).await;
    let seeded = extensions::repos(&db).await.unwrap();
    extensions::remove_repo(&db, &seeded[0].id).await.unwrap();
    assert!(
        !extensions::replace_repos(&db, &retired, &replacement)
            .await
            .unwrap()
    );
    assert!(extensions::repos(&db).await.unwrap().is_empty());
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
        streams::Played {
            track_key: key,
            provider_id: "dev.b/stream",
            candidate_id: "2",
            title: "Studio",
            source_json: r#"{"kind":"url"}"#,
            expires_at: Some(99),
            bitrate_kbps: Some(128),
        },
        20,
    )
    .await
    .unwrap();
    // Verifying a candidate that failed before clears the failure.
    streams::mark_verified(
        &db,
        streams::Played {
            track_key: key,
            provider_id: "dev.a/stream",
            candidate_id: "1",
            title: "Live",
            source_json: r#"{"kind":"url"}"#,
            expires_at: None,
            bitrate_kbps: None,
        },
        30,
    )
    .await
    .unwrap();

    let rows = streams::candidates(&db, key).await.unwrap();
    assert_eq!(rows.len(), 2);
    assert_eq!(rows[0].candidate_id, "1", "most recently verified first");
    assert_eq!(rows[0].failed_at, None);
    assert_eq!(rows[1].expires_at, Some(99));
    assert_eq!(rows[1].bitrate_kbps, Some(128));
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
