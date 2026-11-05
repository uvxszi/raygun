//! Settings → Data's categories (M46.51): what each holds on this device, with sizes, and
//! clearing one. Rows in the database are counted and cleared by `raygun_db::data` (clears of
//! synced parts are tombstones that device sync carries); files and what services hold
//! (extensions, downloads, models, caches) go through their services.

use std::sync::Arc;

use raygun_db::backup::Part;
use raygun_model::{
    DataCategory, DataCount, DataItem, DataScope, DataUsage, DownloadsClear, ExtensionOrigin,
    LyricsClearScope, SettingChange, StorageArea, WebsiteDownloadState,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use super::format;
use crate::events::SyncChangedEvent;
use crate::startup::AppDb;

/// Whether clearing a category reaches linked devices: these are the parts device sync carries.
pub fn scope(category: DataCategory) -> DataScope {
    match category {
        DataCategory::Playlists
        | DataCategory::Pins
        | DataCategory::History
        | DataCategory::Settings => DataScope::Synced,
        _ => DataScope::Device,
    }
}

/// What every category holds here.
pub async fn usage(app: &AppHandle) -> Result<Vec<DataUsage>, String> {
    let db = app.state::<AppDb>().0.clone();
    let counted = raygun_db::data::counts(&db).await.map_err(to_message)?;
    let mut usage: Vec<DataUsage> = DataCategory::ALL
        .into_iter()
        .map(|category| DataUsage {
            category,
            counts: format::part_of(category)
                .and_then(|part| counted.get(&part))
                .into_iter()
                .flatten()
                .map(|(item, count)| DataCount {
                    item: format::item_of(*item),
                    count: *count,
                    bytes: None,
                })
                .collect(),
            bytes: 0.0,
            scope: scope(category),
        })
        .collect();

    let searches = crate::recent_searches::all(&db).await?.len() as u32;
    put(
        &mut usage,
        DataCategory::Searches,
        DataItem::Searches,
        searches,
        None,
    );
    let contexts = app
        .state::<crate::collections::CollectionsService>()
        .recent_contexts()
        .await
        .map_or(0, |contexts| contexts.len()) as u32;
    put(
        &mut usage,
        DataCategory::History,
        DataItem::Contexts,
        contexts,
        None,
    );

    let servers = app
        .state::<crate::servers::ServerService>()
        .configs()
        .await
        .unwrap_or_default()
        .len() as u32;
    let extensions = installed_extensions(app).await.len() as u32;
    put(
        &mut usage,
        DataCategory::Sources,
        DataItem::Extensions,
        extensions,
        None,
    );
    put(
        &mut usage,
        DataCategory::Sources,
        DataItem::Servers,
        servers,
        None,
    );

    let ai = app.state::<crate::ai::AiService>();
    let models: Vec<_> = ai
        .models()
        .into_iter()
        .filter(|model| model.installed)
        .collect();
    let model_bytes: f64 = models.iter().map(|model| model.size_bytes).sum();
    let translators = ai.translator_count().await as u32;
    put(
        &mut usage,
        DataCategory::AiModels,
        DataItem::Models,
        models.len() as u32,
        Some(model_bytes),
    );
    put(
        &mut usage,
        DataCategory::AiModels,
        DataItem::Translators,
        translators,
        None,
    );

    let prefs = crate::settings::load(&db).await.unwrap_or_default().cache;
    let caches = app.state::<crate::caches::Caches>().usage(&prefs).await;
    for found in &caches {
        match found.area {
            StorageArea::Covers => put(
                &mut usage,
                DataCategory::Caches,
                DataItem::Covers,
                found.files,
                Some(found.bytes),
            ),
            StorageArea::Pages => put(
                &mut usage,
                DataCategory::Caches,
                DataItem::Pages,
                found.files,
                Some(found.bytes),
            ),
            // Files made for sharing are a cache too: in the total.
            StorageArea::Shared => entry(&mut usage, DataCategory::Caches).bytes += found.bytes,
            StorageArea::Downloads => {
                entry(&mut usage, DataCategory::Downloads).bytes = found.bytes
            }
            StorageArea::MusicArt => {}
        }
    }
    let lyrics = app
        .state::<Arc<crate::lyrics_store::LyricsStore>>()
        .usage()
        .await
        .map_or(0, |kept| kept.songs);
    put(
        &mut usage,
        DataCategory::Caches,
        DataItem::Lyrics,
        lyrics,
        None,
    );
    let files = app
        .state::<Arc<crate::website_downloads::WebsiteDownloadService>>()
        .list()
        .into_iter()
        .filter(|download| download.state == WebsiteDownloadState::Done)
        .count() as u32;
    put(
        &mut usage,
        DataCategory::Downloads,
        DataItem::Files,
        files,
        None,
    );
    Ok(usage)
}

fn entry(usage: &mut [DataUsage], category: DataCategory) -> &mut DataUsage {
    usage
        .iter_mut()
        .find(|entry| entry.category == category)
        .expect("every category is listed")
}

/// Sets a count (replacing one the database gave), adding its bytes to the category's.
fn put(
    usage: &mut [DataUsage],
    category: DataCategory,
    item: DataItem,
    count: u32,
    bytes: Option<f64>,
) {
    let entry = entry(usage, category);
    entry.counts.retain(|known| known.item != item);
    if count > 0 || bytes.is_some_and(|bytes| bytes > 0.0) {
        entry.counts.push(DataCount { item, count, bytes });
    }
    entry.bytes += bytes.unwrap_or(0.0);
}

/// Extensions installed from repositories (connected servers aside).
async fn installed_extensions(app: &AppHandle) -> Vec<String> {
    app.state::<Arc<crate::extensions::ExtensionService>>()
        .list()
        .await
        .into_iter()
        .filter(|info| {
            info.origin == ExtensionOrigin::Installed
                && !info.id.starts_with(raygun_ext_host::servers::ID_PREFIX)
        })
        .map(|info| info.id)
        .collect()
}

/// Clears a category. `downloads` says what goes of the downloads (their files by default).
pub async fn clear(
    app: &AppHandle,
    category: DataCategory,
    downloads: Option<DownloadsClear>,
) -> Result<(), String> {
    let db = app.state::<AppDb>().0.clone();
    match category {
        DataCategory::Comics
        | DataCategory::Books
        | DataCategory::Videos
        | DataCategory::Podcasts
        | DataCategory::Audiobooks
        | DataCategory::Websites
        | DataCategory::Playlists
        | DataCategory::History
        | DataCategory::Settings => {
            let part = format::part_of(category).expect("a database part");
            let changed = raygun_db::data::clear(&db, part)
                .await
                .map_err(to_message)?;
            if part == Part::History {
                raygun_db::set_local_value(&db, crate::collections::RECENT_CONTEXTS, "[]")
                    .await
                    .map_err(to_message)?;
            }
            tracing::info!(?category, changed, "data cleared");
            // Cached lists in the UI are stale, as after a sync.
            let _ = SyncChangedEvent(changed.min(u64::from(u32::MAX)) as u32).emit(app);
            if part == Part::Settings {
                crate::settings::apply_stored(app);
            }
        }
        DataCategory::Pins => {
            let mut rail = crate::settings::load(&db).await.map_err(to_message)?.rail;
            rail.pins.clear();
            crate::settings::update(&db, SettingChange::Rail(rail))
                .await
                .map_err(to_message)?;
            let _ = SyncChangedEvent(1).emit(app);
        }
        DataCategory::Sources => clear_sources(app).await?,
        DataCategory::Downloads => {
            clear_downloads(app, downloads.unwrap_or(DownloadsClear::Files)).await?;
        }
        DataCategory::Searches => crate::recent_searches::clear(&db).await?,
        DataCategory::AiModels => {
            let ai = app.state::<crate::ai::AiService>();
            for model in ai.models().into_iter().filter(|model| model.installed) {
                ai.delete_model(&model.id).await?;
            }
        }
        DataCategory::Caches => {
            let caches = app.state::<crate::caches::Caches>();
            for area in [StorageArea::Covers, StorageArea::Pages, StorageArea::Shared] {
                caches.clear(area).await?;
            }
            app.state::<Arc<crate::lyrics_store::LyricsStore>>()
                .clear(LyricsClearScope::All, false)
                .await?;
        }
    }
    Ok(())
}

/// Connected servers, extensions from repositories, the repositories and the Stremio addons go.
/// Raygun's built-in sources stay: they're part of the app.
async fn clear_sources(app: &AppHandle) -> Result<(), String> {
    let servers = app.state::<crate::servers::ServerService>();
    for (id, _) in servers.configs().await? {
        servers.remove(&id).await?;
    }
    let extensions = app
        .state::<Arc<crate::extensions::ExtensionService>>()
        .inner()
        .clone();
    for id in installed_extensions(app).await {
        extensions.uninstall(&id).await?;
    }
    for repo in extensions.repos().await? {
        extensions.remove_repo(&repo.id).await?;
    }
    let stremio = app
        .state::<Arc<crate::stremio::StremioService>>()
        .inner()
        .clone();
    for addon in stremio.addons().await? {
        stremio.remove(&addon.id).await?;
    }
    Ok(())
}

/// `Files`: every download and its file (copies saved outside Raygun stay). `Records`: only the
/// entries waiting or failed, which have no file yet.
async fn clear_downloads(app: &AppHandle, what: DownloadsClear) -> Result<(), String> {
    let db = app.state::<AppDb>().0.clone();
    let kept = raygun_db::data::downloads(&db).await.map_err(to_message)?;
    let chosen = |list: &[(String, bool)]| -> Vec<String> {
        list.iter()
            .filter(|(_, done)| what == DownloadsClear::Files || !done)
            .map(|(id, _)| id.clone())
            .collect()
    };
    let songs = chosen(&kept.songs);
    if !songs.is_empty() {
        app.state::<Arc<crate::music_downloads::SongDownloadService>>()
            .remove_many(&songs)
            .await?;
    }
    let chapters = chosen(&kept.chapters);
    if !chapters.is_empty() {
        app.state::<Arc<crate::manga_downloads::MangaDownloadService>>()
            .remove(&chapters)
            .await?;
    }
    let podcasts = app
        .state::<Arc<crate::podcasts::PodcastService>>()
        .inner()
        .clone();
    for id in chosen(&kept.episodes) {
        podcasts.remove_download(&id).await?;
    }
    let videos = app
        .state::<Arc<crate::video_downloads::VideoDownloadService>>()
        .inner()
        .clone();
    for id in chosen(&kept.videos) {
        videos.remove(&id).await?;
    }
    let files = app
        .state::<Arc<crate::website_downloads::WebsiteDownloadService>>()
        .inner()
        .clone();
    for download in files.list() {
        let done = download.state == WebsiteDownloadState::Done;
        if what == DownloadsClear::Files || !done {
            files.remove(&download.id, done).await?;
        }
    }
    Ok(())
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
