//! The app's own values in a backup (M46.51), beside the database's rows: recent searches, what
//! music was played from, connected servers, AI models and translators, files saved from websites,
//! and (only when chosen) passwords, sign-ins and keys. Restoring them goes through their
//! services, which also start installs and downloads again (a backup keeps what was downloaded,
//! never the files) and add back Local Files' folders this device still has.

use std::collections::{BTreeMap, HashSet};
use std::sync::Arc;

use raygun_db::backup::{BackupData, Restored};
use raygun_ext_host::servers::{self as host_servers, ServerConfig};
use raygun_model::{
    DataCategory, DataItem, ExtensionInstallRequest, ExtensionOrigin, MediaSource, RecentSearch,
    TrackInput,
};
use serde_json::Value;
use tauri::{AppHandle, Manager};

use crate::startup::AppDb;

const SEARCHES: &str = "searches";
const CONTEXTS: &str = "recentContexts";
const SERVERS: &str = "servers";
const SERVER_SECRETS: &str = "serverSecrets";
const MODELS: &str = "aiModels";
const TRANSLATORS: &str = "translators";
const TRANSLATOR_KEYS: &str = "translatorKeys";
const INSTALLS: &str = "local_extension_install";
const FOLDERS: &str = "local_folder";
const WEBSITE_FILES: &str = "websiteFiles";

/// What collecting added: counts for the manifest, and whether a secret went in.
#[derive(Default)]
pub struct Collected {
    pub counts: Vec<(DataCategory, DataItem, u32)>,
    pub secrets: bool,
}

/// Adds the app's values of `categories` to a backup.
pub async fn collect(
    app: &AppHandle,
    data: &mut BackupData,
    categories: &[DataCategory],
    secrets: bool,
) -> Result<Collected, String> {
    let db = app.state::<AppDb>().0.clone();
    let mut collected = Collected::default();
    if categories.contains(&DataCategory::Searches) {
        let searches = crate::recent_searches::all(&db).await?;
        collected.counts.push((
            DataCategory::Searches,
            DataItem::Searches,
            searches.len() as u32,
        ));
        data.values.insert(
            SEARCHES.into(),
            serde_json::to_value(searches).map_err(to_message)?,
        );
    }
    if categories.contains(&DataCategory::History) {
        let contexts = stored_contexts(&db).await;
        collected.counts.push((
            DataCategory::History,
            DataItem::Contexts,
            contexts.len() as u32,
        ));
        data.values.insert(CONTEXTS.into(), Value::Array(contexts));
    }
    if categories.contains(&DataCategory::Sources) {
        let servers = app
            .state::<crate::servers::ServerService>()
            .configs()
            .await?;
        // Connections come back as servers, never through a repository.
        if let Some(rows) = data.tables.get_mut(INSTALLS) {
            rows.retain(|row| !is_server(row));
        }
        let extensions = data.tables.get(INSTALLS).map_or(0, Vec::len) as u32;
        collected
            .counts
            .push((DataCategory::Sources, DataItem::Extensions, extensions));
        collected.counts.push((
            DataCategory::Sources,
            DataItem::Servers,
            servers.len() as u32,
        ));
        if secrets {
            let ai = app.state::<crate::ai::AiService>();
            let kept: BTreeMap<String, String> = servers
                .iter()
                .filter_map(|(id, _)| Some((id.clone(), ai.server_secret(id)?)))
                .collect();
            collected
                .counts
                .push((DataCategory::Sources, DataItem::Secrets, kept.len() as u32));
            collected.secrets |= !kept.is_empty();
            data.values.insert(
                SERVER_SECRETS.into(),
                serde_json::to_value(kept).map_err(to_message)?,
            );
        }
        let configs: Vec<ServerConfig> = servers.into_iter().map(|(_, config)| config).collect();
        data.values.insert(
            SERVERS.into(),
            serde_json::to_value(configs).map_err(to_message)?,
        );
    }
    if categories.contains(&DataCategory::AiModels) {
        let ai = app.state::<crate::ai::AiService>();
        let models: Vec<String> = ai
            .models()
            .into_iter()
            .filter(|model| model.installed)
            .map(|model| model.id)
            .collect();
        let (translators, keys) = ai.backup_translators(secrets).await?;
        let translator_count = translators.as_array().map_or(0, Vec::len) as u32;
        collected.counts.push((
            DataCategory::AiModels,
            DataItem::Models,
            models.len() as u32,
        ));
        collected.counts.push((
            DataCategory::AiModels,
            DataItem::Translators,
            translator_count,
        ));
        if secrets {
            collected
                .counts
                .push((DataCategory::AiModels, DataItem::Secrets, keys.len() as u32));
            collected.secrets |= !keys.is_empty();
            data.values.insert(
                TRANSLATOR_KEYS.into(),
                serde_json::to_value(keys).map_err(to_message)?,
            );
        }
        data.values.insert(
            MODELS.into(),
            serde_json::to_value(models).map_err(to_message)?,
        );
        data.values.insert(TRANSLATORS.into(), translators);
    }
    if categories.contains(&DataCategory::Downloads) {
        // Files saved from websites: what was saved and where, as for the other downloads.
        let files = app
            .state::<Arc<crate::website_downloads::WebsiteDownloadService>>()
            .backup();
        collected
            .counts
            .push((DataCategory::Downloads, DataItem::Files, files.len() as u32));
        data.values
            .insert(WEBSITE_FILES.into(), Value::Array(files));
    }
    Ok(collected)
}

fn is_server(row: &Value) -> bool {
    row.get("extension_id")
        .and_then(Value::as_str)
        .is_some_and(|id| id.starts_with(host_servers::ID_PREFIX))
}

async fn stored_contexts(db: &raygun_db::Db) -> Vec<Value> {
    raygun_db::local_value(db, crate::collections::RECENT_CONTEXTS)
        .await
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

/// What a restore started and what it couldn't bring back.
#[derive(Debug, Default)]
pub struct Brought {
    pub queued: u32,
    pub problems: Vec<String>,
}

/// Brings back the app's values of `categories`, after the database's rows are in.
pub async fn restore(
    app: &AppHandle,
    data: &BackupData,
    categories: &[DataCategory],
    restored: &Restored,
) -> Brought {
    let mut brought = Brought::default();
    let db = app.state::<AppDb>().0.clone();
    let value = |name: &str| data.values.get(name).cloned();
    if categories.contains(&DataCategory::Searches)
        && let Some(searches) =
            value(SEARCHES).and_then(|list| serde_json::from_value::<Vec<RecentSearch>>(list).ok())
        && let Err(error) = crate::recent_searches::merge(&db, searches).await
    {
        brought.problems.push(format!("Recent searches: {error}"));
    }
    if categories.contains(&DataCategory::History)
        && let Some(Value::Array(theirs)) = value(CONTEXTS)
    {
        let mut contexts = stored_contexts(&db).await;
        for context in theirs {
            if !contexts.contains(&context) {
                contexts.push(context);
            }
        }
        contexts.truncate(crate::collections::RECENT_CONTEXTS_KEPT);
        let json = Value::Array(contexts).to_string();
        if let Err(error) =
            raygun_db::set_local_value(&db, crate::collections::RECENT_CONTEXTS, &json).await
        {
            brought.problems.push(format!("Played from: {error}"));
        }
    }
    if categories.contains(&DataCategory::Sources) {
        restore_sources(app, data, &mut brought).await;
    }
    if categories.contains(&DataCategory::AiModels) {
        restore_ai(app, data, &mut brought).await;
    }
    if categories.contains(&DataCategory::Downloads) {
        restore_downloads(app, data, restored, &mut brought).await;
    }
    if categories.contains(&DataCategory::LocalFiles) {
        restore_local_folders(app, data, &mut brought).await;
    }
    brought
}

/// Local Files' folders come back where this device still has them, each scanned again.
async fn restore_local_folders(app: &AppHandle, data: &BackupData, brought: &mut Brought) {
    use raygun_db::local_folders::{FolderAccess, FolderRow};
    let text = |row: &Value, field: &str| row.get(field).and_then(Value::as_str).map(str::to_owned);
    let folders: Vec<FolderRow> = data
        .rows(FOLDERS)
        .filter_map(|row| {
            let location = text(row, "location")?;
            Some(FolderRow {
                id: text(row, "id").unwrap_or_default(),
                access: FolderAccess::parse(&text(row, "access").unwrap_or_default(), &location),
                name: text(row, "name").unwrap_or_else(|| location.clone()),
                path: text(row, "path"),
                added_at: row
                    .get("added_at")
                    .and_then(Value::as_i64)
                    .unwrap_or_default(),
                scanned_at: None,
                counts: String::new(),
                location,
            })
        })
        .collect();
    if folders.is_empty() {
        return;
    }
    match app
        .state::<Arc<crate::local_files::LocalFilesService>>()
        .inner()
        .restore(folders)
        .await
    {
        Ok((added, missing)) => {
            brought.queued += added;
            if missing > 0 {
                brought.problems.push(format!(
                    "{missing} Local Files {} aren't on this device; add {} again in Local Files",
                    if missing == 1 { "folder" } else { "folders" },
                    if missing == 1 { "it" } else { "them" },
                ));
            }
        }
        Err(error) => brought.problems.push(format!("Local Files: {error}")),
    }
}

fn secrets_of(data: &BackupData, name: &str) -> BTreeMap<String, String> {
    data.values
        .get(name)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default()
}

async fn restore_sources(app: &AppHandle, data: &BackupData, brought: &mut Brought) {
    let configs: Vec<ServerConfig> = data
        .values
        .get(SERVERS)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    let secrets = secrets_of(data, SERVER_SECRETS);
    let servers = app.state::<crate::servers::ServerService>();
    let mut unsigned = 0;
    for config in &configs {
        let secret = secrets.get(&config.extension_id()).map(String::as_str);
        match servers.restore(config, secret).await {
            Ok(true) => {
                brought.queued += 1;
                if secret.is_none() && config.secret != host_servers::SecretKind::None {
                    unsigned += 1;
                }
            }
            Ok(false) => {}
            Err(error) => brought.problems.push(format!("{}: {error}", config.name)),
        }
    }
    if unsigned > 0 {
        brought.problems.push(format!(
            "Sign in again to {unsigned} {}: the backup has no passwords",
            if unsigned == 1 { "server" } else { "servers" }
        ));
    }

    let extensions = app
        .state::<Arc<crate::extensions::ExtensionService>>()
        .inner()
        .clone();
    let installed: HashSet<String> = extensions
        .list()
        .await
        .into_iter()
        .filter(|info| info.origin == ExtensionOrigin::Installed)
        .map(|info| info.id)
        .collect();
    let repos = extensions.repos().await.unwrap_or_default();
    let mut requests = Vec::new();
    let mut without_repo = 0;
    for row in data.rows(INSTALLS).filter(|row| !is_server(row)) {
        let (Some(id), Some(url)) = (
            row.get("extension_id").and_then(Value::as_str),
            row.get("repo_url").and_then(Value::as_str),
        ) else {
            continue;
        };
        if installed.contains(id) {
            continue;
        }
        match repos.iter().find(|repo| repo.url == url) {
            Some(repo) => requests.push(ExtensionInstallRequest {
                repo_id: repo.id.clone(),
                extension_id: id.to_owned(),
                name: id.to_owned(),
            }),
            None => without_repo += 1,
        }
    }
    if !requests.is_empty() {
        app.state::<Arc<crate::extension_installs::ExtensionInstalls>>()
            .enqueue(&requests);
        brought.queued += requests.len() as u32;
    }
    if without_repo > 0 {
        brought.problems.push(format!(
            "{without_repo} {} came from repositories that aren't here",
            if without_repo == 1 {
                "extension"
            } else {
                "extensions"
            }
        ));
    }
}

async fn restore_ai(app: &AppHandle, data: &BackupData, brought: &mut Brought) {
    let ai = app.state::<crate::ai::AiService>();
    if let Some(list) = data.values.get(TRANSLATORS)
        && let Err(error) = ai
            .restore_translators(list, &secrets_of(data, TRANSLATOR_KEYS))
            .await
    {
        brought.problems.push(format!("Translators: {error}"));
    }
    let wanted: Vec<String> = data
        .values
        .get(MODELS)
        .and_then(|value| serde_json::from_value(value.clone()).ok())
        .unwrap_or_default();
    let installed: HashSet<String> = ai
        .models()
        .into_iter()
        .filter(|model| model.installed)
        .map(|model| model.id)
        .collect();
    for id in wanted {
        if installed.contains(&id) || ai.is_downloading(&id) {
            continue;
        }
        brought.queued += 1;
        let app = app.clone();
        // Models are large: they download in the background, reporting as `AiModelEvent`s.
        tauri::async_runtime::spawn(async move {
            if let Err(error) = app
                .state::<crate::ai::AiService>()
                .download_model(&id)
                .await
            {
                tracing::warn!(model = %id, %error, "a restored model didn't download");
            }
        });
    }
}

async fn restore_downloads(
    app: &AppHandle,
    data: &BackupData,
    restored: &Restored,
    brought: &mut Brought,
) {
    let db = app.state::<AppDb>().0.clone();
    let text = |row: &Value, field: &str| row.get(field).and_then(Value::as_str).map(str::to_owned);
    let tracks: Vec<TrackInput> = data
        .rows("local_music_download")
        .filter_map(|row| {
            Some(TrackInput {
                title: text(row, "title")?,
                artist: text(row, "artist"),
                album: text(row, "album"),
                duration_ms: row
                    .get("duration_ms")
                    .and_then(Value::as_u64)
                    .and_then(|ms| u32::try_from(ms).ok()),
                artwork_url: text(row, "artwork_url"),
                source: text(row, "source")
                    .and_then(|json| serde_json::from_str::<MediaSource>(&json).ok()),
                explicit: false,
            })
        })
        .collect();
    if !tracks.is_empty() {
        let count = tracks.len() as u32;
        match app
            .state::<Arc<crate::music_downloads::SongDownloadService>>()
            .enqueue(tracks)
            .await
        {
            Ok(()) => brought.queued += count,
            Err(error) => brought.problems.push(format!("Songs: {error}")),
        }
    }

    let mut chapters = Vec::new();
    let mut missing = 0;
    for row in data.rows("local_manga_download") {
        let Some(id) = text(row, "unit_id").map(|id| restored.ours(&id)) else {
            continue;
        };
        match raygun_db::titles::unit(&db, &id).await {
            Ok(Some(_)) => chapters.push(id),
            _ => missing += 1,
        }
    }
    if !chapters.is_empty() {
        let count = chapters.len() as u32;
        match app
            .state::<Arc<crate::manga_downloads::MangaDownloadService>>()
            .enqueue(&chapters)
            .await
        {
            Ok(()) => brought.queued += count,
            Err(error) => brought.problems.push(format!("Chapters: {error}")),
        }
    }
    if missing > 0 {
        brought.problems.push(format!(
            "{missing} downloaded {} weren't queued again: their titles aren't in the library here",
            if missing == 1 { "chapter" } else { "chapters" }
        ));
    }

    let podcasts = app
        .state::<Arc<crate::podcasts::PodcastService>>()
        .inner()
        .clone();
    for row in data.rows("local_podcast_download") {
        if let Some(id) = text(row, "episode_id").map(|id| restored.ours(&id))
            && podcasts.download(&id).await.is_ok()
        {
            brought.queued += 1;
        }
    }

    if let Some(Value::Array(files)) = data.values.get(WEBSITE_FILES) {
        // Listed again where they were saved: the files stay on their device.
        app.state::<Arc<crate::website_downloads::WebsiteDownloadService>>()
            .restore(files);
    }

    let videos = data.rows("local_video_download").count();
    if videos > 0 {
        brought.problems.push(format!(
            "{videos} {} weren't downloaded again; download them from their pages",
            if videos == 1 { "video" } else { "videos" }
        ));
    }
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
