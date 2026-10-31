use std::sync::Arc;

use raygun_model::{
    AppInfo, EngineReport, ExtensionInfo, ExtensionRepo, LibraryAlbum, LibraryArtist,
    LibraryStatus, LibraryTrack, Lyrics, PlayRequest, PlaybackControl, PlaybackState,
    ProviderSearchResult, QueueItemInput, QueueSnapshot, RepoCatalog, SettingChange, SettingValue,
    Settings, StorageInfo, StorageMode, TrackQuery,
};

use crate::extensions::ExtensionService;
use crate::library::LibraryService;
use crate::playback::PlaybackService;
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

/// Searches every online music provider at once; each answers with tracks or an error.
#[tauri::command]
#[specta::specta]
pub async fn online_search(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: String,
) -> Result<Vec<ProviderSearchResult>, String> {
    Ok(extensions.search(&query).await)
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
