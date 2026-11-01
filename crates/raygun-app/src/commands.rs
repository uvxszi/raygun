use std::sync::Arc;

use raygun_model::{
    AppInfo, BackupFile, BackupOutcome, CollectionPlayRequest, EngineReport, ExtensionInfo,
    ExtensionRepo, HistoryEntry, ImportedBook, LibraryAlbum, LibraryArtist, LibraryStatus,
    LibraryTrack, Lyrics, MangaBrowseMode, MangaChapterPages, MangaLibraryEntry, MangaListingPage,
    MangaSource, MangaTitleView, MangaUpdateItem, MangaUpdateStatus, McpStatus, NovelChapter,
    PlayRequest, PlaybackControl, PlaybackState, PlaylistDetail, PlaylistSummary, QueueItemInput,
    QueueSnapshot, ReaderMode, RemoteStatus, RepoCatalog, SavedTrack, SearchResults, SettingChange,
    SettingValue, Settings, StorageInfo, StorageMode, SyncKind, SyncStatus, TrackInput, TrackQuery,
};

use crate::backup::BackupService;
use crate::collections::CollectionsService;
use crate::device_sync::SyncService;
use crate::extensions::ExtensionService;
use crate::library::LibraryService;
use crate::manga::MangaService;
use crate::manga_downloads::MangaDownloadService;
use crate::mcp::McpService;
use crate::novel::NovelService;
use crate::playback::PlaybackService;
use crate::remote::RemoteService;
use crate::settings;
use crate::startup::{AppDb, AppPaths};

/// Reports build and runtime facts about the running app.
#[tauri::command]
#[specta::specta]
pub fn app_info() -> AppInfo {
    AppInfo {
        name: "Raygun".into(),
        version: env!("CARGO_PKG_VERSION").into(),
        platform: std::env::consts::OS.into(),
        arch: std::env::consts::ARCH.into(),
        debug: cfg!(debug_assertions),
    }
}

/// Reports where Raygun keeps its data on this device.
#[tauri::command]
#[specta::specta]
pub fn storage_info(paths: tauri::State<'_, AppPaths>) -> StorageInfo {
    to_storage_info(&paths.0)
}

/// Current user settings, with defaults for anything never set.
#[tauri::command]
#[specta::specta]
pub async fn settings_get(db: tauri::State<'_, AppDb>) -> Result<Settings, String> {
    settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())
}

/// Changes one setting and returns the updated settings.
#[tauri::command]
#[specta::specta]
pub async fn settings_update(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    change: SettingChange,
) -> Result<Settings, String> {
    let theme_changed = matches!(change, SettingChange::Theme(_));
    let settings = settings::update(&db.0, change)
        .await
        .map_err(|error| error.to_string())?;
    if theme_changed {
        crate::system_ui::apply_theme(&app, settings.theme);
    }
    Ok(settings)
}

/// Current player state. Afterwards, changes arrive as `PlaybackStateEvent`.
#[tauri::command]
#[specta::specta]
pub fn playback_get_state(playback: tauri::State<'_, PlaybackService>) -> PlaybackState {
    playback.state()
}

/// Current queue. Afterwards, changes arrive as `QueueEvent`.
#[tauri::command]
#[specta::specta]
pub fn playback_get_queue(playback: tauri::State<'_, PlaybackService>) -> QueueSnapshot {
    playback.queue()
}

/// Adds items to the queue; with `play_now` the first one starts immediately.
#[tauri::command]
#[specta::specta]
pub fn playback_enqueue(
    playback: tauri::State<'_, PlaybackService>,
    items: Vec<QueueItemInput>,
    play_now: bool,
) {
    playback.enqueue(items, play_now);
}

/// Replaces the queue with `items` and starts playing at `start_index` (default: the first).
#[tauri::command]
#[specta::specta]
pub fn playback_replace(
    playback: tauri::State<'_, PlaybackService>,
    items: Vec<QueueItemInput>,
    start_index: Option<u32>,
) {
    playback.replace(items, start_index.map(|index| index as usize), false);
}

/// Play, pause, skip, seek, volume, repeat, shuffle and queue edits.
#[tauri::command]
#[specta::specta]
pub fn playback_control(playback: tauri::State<'_, PlaybackService>, control: PlaybackControl) {
    playback.control(control);
}

/// Called by the playback engine in the webview to report what the media element did.
#[tauri::command]
#[specta::specta]
pub fn playback_engine_report(playback: tauri::State<'_, PlaybackService>, report: EngineReport) {
    playback.report(report);
}

/// Library folders, track count and whether a scan is running.
#[tauri::command]
#[specta::specta]
pub async fn library_status(
    library: tauri::State<'_, LibraryService>,
) -> Result<LibraryStatus, String> {
    library.status().await
}

/// Asks for access to the device's music (Android) and scans its music folders.
#[tauri::command]
#[specta::specta]
pub async fn library_connect_device_music(
    library: tauri::State<'_, LibraryService>,
) -> Result<LibraryStatus, String> {
    library.connect_device_music().await
}

/// Adds a folder to the library and scans it.
#[tauri::command]
#[specta::specta]
pub async fn library_add_root(
    library: tauri::State<'_, LibraryService>,
    path: String,
) -> Result<LibraryStatus, String> {
    library.add_root(&path).await
}

/// Removes a folder from the library; its tracks disappear with the next scan (started now).
#[tauri::command]
#[specta::specta]
pub async fn library_remove_root(
    library: tauri::State<'_, LibraryService>,
    path: String,
) -> Result<LibraryStatus, String> {
    library.remove_root(&path).await
}

/// Starts an incremental rescan; progress arrives as `LibraryScanEvent`.
#[tauri::command]
#[specta::specta]
pub fn library_rescan(library: tauri::State<'_, LibraryService>) {
    library.start_scan();
}

/// Albums, optionally only one artist's.
#[tauri::command]
#[specta::specta]
pub async fn library_albums(
    library: tauri::State<'_, LibraryService>,
    artist: Option<String>,
) -> Result<Vec<LibraryAlbum>, String> {
    library.albums(artist.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn library_artists(
    library: tauri::State<'_, LibraryService>,
) -> Result<Vec<LibraryArtist>, String> {
    library.artists().await
}

#[tauri::command]
#[specta::specta]
pub async fn library_album_tracks(
    library: tauri::State<'_, LibraryService>,
    artist: String,
    album: String,
) -> Result<Vec<LibraryTrack>, String> {
    library.album_tracks(&artist, &album).await
}

/// All tracks in library order, a page at a time.
#[tauri::command]
#[specta::specta]
pub async fn library_tracks(
    library: tauri::State<'_, LibraryService>,
    offset: u32,
    limit: u32,
) -> Result<Vec<LibraryTrack>, String> {
    library.tracks(offset, limit).await
}

/// Replaces the play queue with part of the library and starts playing.
#[tauri::command]
#[specta::specta]
pub async fn library_play(
    library: tauri::State<'_, LibraryService>,
    request: PlayRequest,
) -> Result<(), String> {
    library.play(request).await
}

/// Installed and built-in extensions, with their status and providers.
#[tauri::command]
#[specta::specta]
pub async fn extensions_list(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
) -> Result<Vec<ExtensionInfo>, String> {
    Ok(extensions.list().await)
}

/// Reloads all extensions, picking up any copied into the `extensions/` folder.
#[tauri::command]
#[specta::specta]
pub async fn extensions_reload(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
) -> Result<Vec<ExtensionInfo>, String> {
    Ok(extensions.reload().await)
}

/// Repositories extensions can be installed from.
#[tauri::command]
#[specta::specta]
pub async fn extension_repos(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
) -> Result<Vec<ExtensionRepo>, String> {
    extensions.repos().await
}

/// Adds a repository by its `index.json` URL, after checking the index can be read.
#[tauri::command]
#[specta::specta]
pub async fn extension_repo_add(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    url: String,
) -> Result<Vec<ExtensionRepo>, String> {
    extensions.add_repo(&url).await
}

#[tauri::command]
#[specta::specta]
pub async fn extension_repo_remove(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    id: String,
) -> Result<Vec<ExtensionRepo>, String> {
    extensions.remove_repo(&id).await
}

/// Every repository's extensions, marked against what this device has. Indexes are cached for a
/// few minutes unless `refresh` is set.
#[tauri::command]
#[specta::specta]
pub async fn extension_catalog(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    refresh: bool,
) -> Result<Vec<RepoCatalog>, String> {
    extensions.catalog(refresh).await
}

/// Installs, or updates to, the version a repository lists.
#[tauri::command]
#[specta::specta]
pub async fn extension_install(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    repo_id: String,
    extension_id: String,
) -> Result<Vec<ExtensionInfo>, String> {
    extensions.install(&repo_id, &extension_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn extension_uninstall(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    extension_id: String,
) -> Result<Vec<ExtensionInfo>, String> {
    extensions.uninstall(&extension_id).await
}

/// Saves one of an extension's settings and reloads it.
#[tauri::command]
#[specta::specta]
pub async fn extension_setting_set(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    extension_id: String,
    key: String,
    value: SettingValue,
) -> Result<Vec<ExtensionInfo>, String> {
    extensions.set_setting(&extension_id, &key, value).await
}

/// Lyrics for a track, or `null` when no provider has any.
#[tauri::command]
#[specta::specta]
pub async fn lyrics_get(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: TrackQuery,
) -> Result<Option<Lyrics>, String> {
    extensions.lyrics(&query).await
}

/// Local library tracks shown above online results.
const SEARCH_LIBRARY_LIMIT: u32 = 8;

/// Searches the local library and every online music provider at once. Each provider answers
/// with tracks or an error; songs several providers found are merged into `top`.
#[tauri::command]
#[specta::specta]
pub async fn online_search(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    library: tauri::State<'_, LibraryService>,
    query: String,
) -> Result<SearchResults, String> {
    let (local, providers) = futures_util::future::join(
        library.search(&query, SEARCH_LIBRARY_LIMIT),
        extensions.search(&query),
    )
    .await;
    let library = local.unwrap_or_else(|message| {
        tracing::warn!(%message, "library search failed");
        Vec::new()
    });
    Ok(SearchResults {
        library,
        top: crate::search::merge_top(&providers),
        providers,
    })
}

/// Favorite tracks, most recently added first.
#[tauri::command]
#[specta::specta]
pub async fn favorites_list(
    collections: tauri::State<'_, CollectionsService>,
) -> Result<Vec<SavedTrack>, String> {
    collections.favorites().await
}

/// Whether a track (by its artist and title) is a favorite.
#[tauri::command]
#[specta::specta]
pub async fn favorite_get(
    collections: tauri::State<'_, CollectionsService>,
    track: TrackInput,
) -> Result<bool, String> {
    collections.is_favorite(track).await
}

#[tauri::command]
#[specta::specta]
pub async fn favorite_set(
    collections: tauri::State<'_, CollectionsService>,
    track: TrackInput,
    favorite: bool,
) -> Result<(), String> {
    collections.set_favorite(track, favorite).await
}

#[tauri::command]
#[specta::specta]
pub async fn playlists_list(
    collections: tauri::State<'_, CollectionsService>,
) -> Result<Vec<PlaylistSummary>, String> {
    collections.playlists().await
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_get(
    collections: tauri::State<'_, CollectionsService>,
    id: String,
) -> Result<PlaylistDetail, String> {
    collections.playlist(&id).await
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_create(
    collections: tauri::State<'_, CollectionsService>,
    name: String,
) -> Result<PlaylistSummary, String> {
    collections.create_playlist(&name).await
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_rename(
    collections: tauri::State<'_, CollectionsService>,
    id: String,
    name: String,
) -> Result<(), String> {
    collections.rename_playlist(&id, &name).await
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_delete(
    collections: tauri::State<'_, CollectionsService>,
    id: String,
) -> Result<(), String> {
    collections.delete_playlist(&id).await
}

/// Appends tracks to the end of a playlist.
#[tauri::command]
#[specta::specta]
pub async fn playlist_add(
    collections: tauri::State<'_, CollectionsService>,
    id: String,
    tracks: Vec<TrackInput>,
) -> Result<(), String> {
    collections.add_to_playlist(&id, tracks).await
}

#[tauri::command]
#[specta::specta]
pub async fn playlist_remove_item(
    collections: tauri::State<'_, CollectionsService>,
    item_id: String,
) -> Result<(), String> {
    collections.remove_playlist_item(&item_id).await
}

/// Moves an item before another one, or to the end when `before_item_id` is null.
#[tauri::command]
#[specta::specta]
pub async fn playlist_move_item(
    collections: tauri::State<'_, CollectionsService>,
    item_id: String,
    before_item_id: Option<String>,
) -> Result<(), String> {
    collections
        .move_playlist_item(&item_id, before_item_id.as_deref())
        .await
}

/// The latest play of each recently played track, newest first.
#[tauri::command]
#[specta::specta]
pub async fn history_recent(
    collections: tauri::State<'_, CollectionsService>,
    limit: u32,
) -> Result<Vec<HistoryEntry>, String> {
    collections.history(limit).await
}

#[tauri::command]
#[specta::specta]
pub async fn history_clear(
    collections: tauri::State<'_, CollectionsService>,
) -> Result<(), String> {
    collections.clear_history().await
}

/// The MCP server's switch, address and token.
#[tauri::command]
#[specta::specta]
pub async fn mcp_status(mcp: tauri::State<'_, McpService>) -> Result<McpStatus, String> {
    mcp.status().await
}

/// Turns the MCP server on or off; the choice is kept for the next start.
#[tauri::command]
#[specta::specta]
pub async fn mcp_set_enabled(
    mcp: tauri::State<'_, McpService>,
    enabled: bool,
) -> Result<McpStatus, String> {
    mcp.set_enabled(enabled).await
}

/// Issues a new MCP token; clients with the old one must be given the new one.
#[tauri::command]
#[specta::specta]
pub async fn mcp_new_token(mcp: tauri::State<'_, McpService>) -> Result<McpStatus, String> {
    mcp.new_token().await
}

/// The LAN remote's switch, address, pairing code and paired devices.
#[tauri::command]
#[specta::specta]
pub async fn remote_status(
    remote: tauri::State<'_, RemoteService>,
) -> Result<RemoteStatus, String> {
    remote.status().await
}

/// Turns the LAN remote on (opening a pairing code) or off.
#[tauri::command]
#[specta::specta]
pub async fn remote_set_enabled(
    remote: tauri::State<'_, RemoteService>,
    enabled: bool,
) -> Result<RemoteStatus, String> {
    remote.set_enabled(enabled).await
}

/// Opens a new pairing code, replacing the previous one.
#[tauri::command]
#[specta::specta]
pub async fn remote_new_code(
    remote: tauri::State<'_, RemoteService>,
) -> Result<RemoteStatus, String> {
    remote.new_code().await
}

/// Removes a paired device; it loses access at once.
#[tauri::command]
#[specta::specta]
pub async fn remote_forget_device(
    remote: tauri::State<'_, RemoteService>,
    id: String,
) -> Result<RemoteStatus, String> {
    remote.forget(&id).await
}

/// Device sync's switch, identity, open pairing, peers and log.
#[tauri::command]
#[specta::specta]
pub async fn sync_status(sync: tauri::State<'_, SyncService>) -> Result<SyncStatus, String> {
    sync.status().await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_set_enabled(
    sync: tauri::State<'_, SyncService>,
    enabled: bool,
) -> Result<SyncStatus, String> {
    sync.set_enabled(enabled).await
}

/// Opens a pairing code; the status carries the text and QR code another device needs.
#[tauri::command]
#[specta::specta]
pub async fn sync_open_pairing(sync: tauri::State<'_, SyncService>) -> Result<SyncStatus, String> {
    sync.open_pairing().await
}

/// Pairs with the device whose pairing text this is, then syncs with it.
#[tauri::command]
#[specta::specta]
pub async fn sync_join(
    sync: tauri::State<'_, SyncService>,
    text: String,
) -> Result<SyncStatus, String> {
    sync.join(&text).await
}

/// Syncs with one paired device, or all of them when `device_id` is null.
#[tauri::command]
#[specta::specta]
pub async fn sync_now(
    sync: tauri::State<'_, SyncService>,
    device_id: Option<String>,
) -> Result<SyncStatus, String> {
    sync.sync_now(device_id.as_deref()).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_remove_peer(
    sync: tauri::State<'_, SyncService>,
    device_id: String,
) -> Result<SyncStatus, String> {
    sync.remove_peer(&device_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_set_kinds(
    sync: tauri::State<'_, SyncService>,
    kinds: Vec<SyncKind>,
) -> Result<SyncStatus, String> {
    sync.set_kinds(kinds).await
}

#[tauri::command]
#[specta::specta]
pub async fn sync_set_device_name(
    sync: tauri::State<'_, SyncService>,
    name: String,
) -> Result<SyncStatus, String> {
    sync.set_device_name(&name).await
}

/// Undoes the newest sync that changed something on this device.
#[tauri::command]
#[specta::specta]
pub async fn sync_undo_last(sync: tauri::State<'_, SyncService>) -> Result<SyncStatus, String> {
    sync.undo_last().await
}

/// Installed manga sources (`manga.source` providers).
#[tauri::command]
#[specta::specta]
pub async fn manga_sources(
    manga: tauri::State<'_, MangaService>,
) -> Result<Vec<MangaSource>, String> {
    Ok(manga.sources().await)
}

/// A page of a source's popular, latest or search results.
#[tauri::command]
#[specta::specta]
pub async fn manga_browse(
    manga: tauri::State<'_, MangaService>,
    source_id: String,
    mode: MangaBrowseMode,
    query: Option<String>,
    page: u32,
) -> Result<MangaListingPage, String> {
    manga.browse(&source_id, mode, query.as_deref(), page).await
}

/// A title with its chapters; `refresh` asks the source again instead of using stored data.
#[tauri::command]
#[specta::specta]
pub async fn manga_title(
    manga: tauri::State<'_, MangaService>,
    source_id: String,
    manga_id: String,
    refresh: bool,
) -> Result<MangaTitleView, String> {
    manga.title(&source_id, &manga_id, refresh).await
}

#[tauri::command]
#[specta::specta]
pub async fn manga_set_in_library(
    manga: tauri::State<'_, MangaService>,
    title_id: String,
    in_library: bool,
) -> Result<(), String> {
    manga.set_in_library(&title_id, in_library).await
}

#[tauri::command]
#[specta::specta]
pub async fn manga_library(
    manga: tauri::State<'_, MangaService>,
) -> Result<Vec<MangaLibraryEntry>, String> {
    manga.library().await
}

/// A chapter's pages and its neighbours, for the reader.
#[tauri::command]
#[specta::specta]
pub async fn manga_chapter(
    manga: tauri::State<'_, MangaService>,
    title_id: String,
    chapter_id: String,
) -> Result<MangaChapterPages, String> {
    manga.chapter(&title_id, &chapter_id).await
}

/// The page reached in a chapter; reaching the last page marks it read.
#[tauri::command]
#[specta::specta]
pub async fn manga_set_progress(
    manga: tauri::State<'_, MangaService>,
    chapter_id: String,
    page: u32,
    total: u32,
) -> Result<(), String> {
    manga.set_progress(&chapter_id, page, total).await
}

/// Remembers how a title is read (webtoon strip or paged, left or right to left).
#[tauri::command]
#[specta::specta]
pub async fn manga_set_reader_mode(
    manga: tauri::State<'_, MangaService>,
    title_id: String,
    mode: ReaderMode,
) -> Result<(), String> {
    manga.set_reader_mode(&title_id, mode).await
}

/// Chapters library updates found, newest first.
#[tauri::command]
#[specta::specta]
pub async fn manga_updates(
    manga: tauri::State<'_, MangaService>,
) -> Result<Vec<MangaUpdateItem>, String> {
    manga.updates().await
}

/// Starts a library update (also checking finished series); progress arrives as
/// `MangaUpdateEvent`s.
#[tauri::command]
#[specta::specta]
pub async fn manga_update_library(
    manga: tauri::State<'_, MangaService>,
) -> Result<MangaUpdateStatus, String> {
    Ok(manga.start_update(true).await)
}

#[tauri::command]
#[specta::specta]
pub async fn manga_update_status(
    manga: tauri::State<'_, MangaService>,
) -> Result<MangaUpdateStatus, String> {
    Ok(manga.update_status().await)
}

/// The download queue; changes arrive as `MangaDownloadsEvent`s.
#[tauri::command]
#[specta::specta]
pub async fn manga_downloads(
    downloads: tauri::State<'_, Arc<MangaDownloadService>>,
) -> Result<raygun_model::MangaDownloads, String> {
    downloads.queue().await
}

/// Queues chapters for download, in the given order; failed ones are retried.
#[tauri::command]
#[specta::specta]
pub async fn manga_download(
    downloads: tauri::State<'_, Arc<MangaDownloadService>>,
    chapter_ids: Vec<String>,
) -> Result<(), String> {
    downloads.enqueue(&chapter_ids).await
}

/// Cancels queued chapters and deletes downloaded ones.
#[tauri::command]
#[specta::specta]
pub async fn manga_remove_downloads(
    downloads: tauri::State<'_, Arc<MangaDownloadService>>,
    chapter_ids: Vec<String>,
) -> Result<(), String> {
    downloads.remove(&chapter_ids).await
}

#[tauri::command]
#[specta::specta]
pub async fn manga_set_downloads_paused(
    downloads: tauri::State<'_, Arc<MangaDownloadService>>,
    paused: bool,
) -> Result<(), String> {
    downloads.set_paused(paused);
    Ok(())
}

#[tauri::command]
#[specta::specta]
pub async fn manga_set_read(
    manga: tauri::State<'_, MangaService>,
    chapter_id: String,
    read: bool,
) -> Result<(), String> {
    manga.set_read(&chapter_id, read).await
}

/// Replaces the queue with favorites, a playlist or history and starts playing.
#[tauri::command]
#[specta::specta]
pub async fn collection_play(
    collections: tauri::State<'_, CollectionsService>,
    request: CollectionPlayRequest,
) -> Result<(), String> {
    collections.play(request).await
}

fn to_storage_info(paths: &raygun_paths::Paths) -> StorageInfo {
    StorageInfo {
        mode: match paths.mode() {
            raygun_paths::StorageMode::Portable => StorageMode::Portable,
            raygun_paths::StorageMode::Installed => StorageMode::Installed,
            raygun_paths::StorageMode::Mobile => StorageMode::Mobile,
            raygun_paths::StorageMode::Override => StorageMode::Override,
        },
        root: paths.root().display().to_string(),
        logs: paths.logs().display().to_string(),
        webview: paths.webview().display().to_string(),
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use super::*;

    #[test]
    fn app_info_reports_host_platform() {
        let info = app_info();
        assert_eq!(info.name, "Raygun");
        assert_eq!(info.version, env!("CARGO_PKG_VERSION"));
        assert_eq!(info.platform, std::env::consts::OS);
        assert_eq!(info.arch, std::env::consts::ARCH);
    }

    #[test]
    fn storage_info_mirrors_resolved_paths() {
        let root = PathBuf::from("portable-root");
        let paths = raygun_paths::Paths::new(root.clone(), raygun_paths::StorageMode::Portable);

        let info = to_storage_info(&paths);

        assert_eq!(info.mode, StorageMode::Portable);
        assert_eq!(info.root, root.display().to_string());
        assert_eq!(info.logs, root.join("logs").display().to_string());
        assert_eq!(info.webview, root.join("webview").display().to_string());
    }
}

/// Writes a backup of settings, collections, history, the manga library and progress.
#[tauri::command]
#[specta::specta]
pub async fn backup_create(backups: tauri::State<'_, BackupService>) -> Result<BackupFile, String> {
    backups.create().await
}

/// Backups kept on this device, newest first.
#[tauri::command]
#[specta::specta]
pub async fn backup_list(
    backups: tauri::State<'_, BackupService>,
) -> Result<Vec<BackupFile>, String> {
    backups.list().await
}

/// Merges a kept backup into the current data (newer rows win).
#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    backups: tauri::State<'_, BackupService>,
    name: String,
) -> Result<BackupOutcome, String> {
    backups.restore(&name).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_delete(
    backups: tauri::State<'_, BackupService>,
    name: String,
) -> Result<(), String> {
    backups.delete(&name).await
}

/// Restores a Raygun backup or imports a Mihon backup from a picked file (base64 contents).
#[tauri::command]
#[specta::specta]
pub async fn backup_import(
    backups: tauri::State<'_, BackupService>,
    contents: String,
) -> Result<BackupOutcome, String> {
    backups.import(&contents).await
}

/// Installed novel sources (`novel.source` providers).
#[tauri::command]
#[specta::specta]
pub async fn novel_sources(
    novels: tauri::State<'_, NovelService>,
) -> Result<Vec<MangaSource>, String> {
    Ok(novels.sources().await)
}

/// A page of a novel source's catalogue: popular, latest, or search results.
#[tauri::command]
#[specta::specta]
pub async fn novel_browse(
    novels: tauri::State<'_, NovelService>,
    source_id: String,
    mode: MangaBrowseMode,
    query: Option<String>,
    page: u32,
) -> Result<MangaListingPage, String> {
    novels
        .browse(&source_id, mode, query.as_deref(), page)
        .await
}

/// A novel with its chapters; `refresh` fetches it from the source again.
#[tauri::command]
#[specta::specta]
pub async fn novel_title(
    novels: tauri::State<'_, NovelService>,
    source_id: String,
    novel_id: String,
    refresh: bool,
) -> Result<MangaTitleView, String> {
    novels.title(&source_id, &novel_id, refresh).await
}

#[tauri::command]
#[specta::specta]
pub async fn novel_set_in_library(
    novels: tauri::State<'_, NovelService>,
    title_id: String,
    in_library: bool,
) -> Result<(), String> {
    novels.set_in_library(&title_id, in_library).await
}

#[tauri::command]
#[specta::specta]
pub async fn novel_library(
    novels: tauri::State<'_, NovelService>,
) -> Result<Vec<MangaLibraryEntry>, String> {
    novels.library().await
}

/// One chapter's text as blocks, with its neighbours.
#[tauri::command]
#[specta::specta]
pub async fn novel_chapter(
    novels: tauri::State<'_, NovelService>,
    title_id: String,
    chapter_id: String,
) -> Result<NovelChapter, String> {
    novels.chapter(&title_id, &chapter_id).await
}

/// Saves the character offset reached in a chapter of `total` characters.
#[tauri::command]
#[specta::specta]
pub async fn novel_set_progress(
    novels: tauri::State<'_, NovelService>,
    chapter_id: String,
    offset: u32,
    total: u32,
) -> Result<(), String> {
    novels.set_progress(&chapter_id, offset, total).await
}

#[tauri::command]
#[specta::specta]
pub async fn novel_set_read(
    novels: tauri::State<'_, NovelService>,
    chapter_id: String,
    read: bool,
) -> Result<(), String> {
    novels.set_read(&chapter_id, read).await
}

/// Imports an EPUB (its contents as base64) into the novel library; returns its source and id.
#[tauri::command]
#[specta::specta]
pub async fn novel_import_epub(
    novels: tauri::State<'_, NovelService>,
    contents: String,
) -> Result<ImportedBook, String> {
    let (source_id, novel_id) = novels.import_epub(&contents).await?;
    Ok(ImportedBook {
        source_id,
        novel_id,
    })
}

/// Whether the device can read text aloud.
#[tauri::command]
#[specta::specta]
pub async fn tts_available(app: tauri::AppHandle) -> Result<bool, String> {
    Ok(crate::tts::available(&app).await)
}

/// Reads the paragraphs aloud in order, replacing anything being read; `rate` 1 is normal.
/// Progress arrives as `TtsEvent`s.
#[tauri::command]
#[specta::specta]
pub async fn tts_speak(
    app: tauri::AppHandle,
    paragraphs: Vec<raygun_model::TtsParagraph>,
    rate: f64,
) -> Result<(), String> {
    crate::tts::speak(&app, paragraphs, rate).await
}

#[tauri::command]
#[specta::specta]
pub async fn tts_stop(app: tauri::AppHandle) -> Result<(), String> {
    crate::tts::stop(&app).await
}
