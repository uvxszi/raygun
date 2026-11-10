use std::sync::Arc;

use raygun_model::{
    AiModel, AiProvider, AiProviderInput, AppInfo, BackupDestinations, BackupEntry, BackupOutcome,
    BackupRequest, CollectionPlayRequest, DataCategory, DataUsage, DownloadsClear, EngineReport,
    ExtensionInfo, ExtensionRepo, HistoryEntry, ImportedBook, LibraryAlbum, LibraryArtist,
    LibraryStatus, LibraryTrack, Lyrics, MangaBrowseMode, MangaChapterInfo, MangaChapterPages,
    MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView, MangaUpdateItem,
    MangaUpdateStatus, McpStatus, NovelChapter, PlayRequest, PlaybackControl, PlaybackState,
    PlaylistDetail, PlaylistSummary, QueueItemInput, QueueSnapshot, ReaderMode, RemoteStatus,
    RepoCatalog, SavedTrack, SearchResults, SettingChange, SettingValue, Settings, StorageInfo,
    StorageMode, SyncKind, SyncStatus, TrackInput, TrackQuery,
};

use crate::ai::AiService;
use crate::backup::BackupService;
use crate::collections::CollectionsService;
use crate::device_sync::SyncService;
use crate::discovery::DiscoveryService;
use crate::extension_installs::ExtensionInstalls;
use crate::extensions::ExtensionService;
use crate::library::LibraryService;
use crate::local_network::LocalNetwork;
use crate::manga::MangaService;
use crate::manga_downloads::MangaDownloadService;
use crate::mcp::McpService;
use crate::music_downloads::SongDownloadService;
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

/// What was played from lately: albums, playlists, mixes, artists, podcasts, books (M46.37).
/// Hidden artists' pages and tiles marked Not interested are left out.
#[tauri::command]
#[specta::specta]
pub async fn recent_contexts(
    db: tauri::State<'_, AppDb>,
    collections: tauri::State<'_, CollectionsService>,
) -> Result<Vec<raygun_model::PlayContext>, String> {
    let mut contexts = collections.recent_contexts().await?;
    let hidden = crate::hidden::Hidden::load(&db.0).await;
    contexts.retain(|context| !hidden.hides_context(context));
    Ok(contexts)
}

/// Remembers where a play started (the page the user played from), for Home's Your music.
#[tauri::command]
#[specta::specta]
pub async fn recent_contexts_add(
    collections: tauri::State<'_, CollectionsService>,
    context: raygun_model::PlayContext,
) -> Result<(), String> {
    collections.add_recent_context(context).await
}

/// Changes one setting and returns the updated settings.
#[tauri::command]
#[specta::specta]
pub async fn settings_update(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    change: SettingChange,
) -> Result<Settings, String> {
    settings_update_many(app, db, vec![change]).await
}

/// Changes several settings together (a screen's Reset to defaults): none is written when one
/// is refused.
#[tauri::command]
#[specta::specta]
pub async fn settings_update_many(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    changes: Vec<SettingChange>,
) -> Result<Settings, String> {
    let mut kept = Vec::with_capacity(changes.len());
    for change in changes {
        // When quick access's pages were played from is Rust's to keep (and never while
        // incognito, M47): the UI's copy stays out.
        kept.push(crate::quick_access::keep_stamps(&db.0, change).await);
    }
    let keys: Vec<&'static str> = kept.iter().map(SettingChange::key).collect();
    let settings = settings::update_many(&db.0, kept)
        .await
        .map_err(|error| error.to_string())?;
    settings::apply_live(&app, &settings, Some(&keys)).await;
    if keys.contains(&"rail") {
        crate::quick_access::changed(&app, "");
    }
    Ok(settings)
}

/// Every setting as a fresh install has it, for resetting a settings screen.
#[tauri::command]
#[specta::specta]
pub fn settings_defaults() -> Settings {
    Settings::default()
}

/// The font families installed on this device, for the font pickers: read once and kept, again
/// with `refresh`.
#[tauri::command]
#[specta::specta]
pub async fn fonts_list(
    fonts: tauri::State<'_, crate::fonts::FontService>,
    refresh: bool,
) -> Result<Vec<raygun_model::SystemFont>, String> {
    Ok(fonts.list(refresh).await)
}

/// Current player state. Afterwards, changes arrive as `PlaybackStateEvent`.
#[tauri::command]
#[specta::specta]
pub fn playback_get_state(playback: tauri::State<'_, PlaybackService>) -> PlaybackState {
    playback.state()
}

/// Puts back what played when Raygun closed, paused where it was (M46.16); the UI asks once its
/// engines listen. Only the first call of a run does anything.
#[tauri::command]
#[specta::specta]
pub async fn playback_restore(
    app: tauri::AppHandle,
    playback: tauri::State<'_, PlaybackService>,
) -> Result<(), String> {
    playback.restore_last(&app).await;
    // The UI is up: what another app opened with Raygun before it was goes ahead.
    crate::open_with::ready(&app);
    Ok(())
}

/// Current queue. Afterwards, changes arrive as `QueueEvent`.
#[tauri::command]
#[specta::specta]
pub fn playback_get_queue(playback: tauri::State<'_, PlaybackService>) -> QueueSnapshot {
    playback.queue()
}

/// Says where the next play comes from (a page's context, or `None` for nowhere in particular):
/// the next queue it replaces shows it as "Playing from". Sent just before the play command.
#[tauri::command]
#[specta::specta]
pub fn playback_arm_context(
    app: tauri::AppHandle,
    playback: tauri::State<'_, PlaybackService>,
    context: Option<raygun_model::PlayContext>,
) {
    // Quick access's Recently played: the page played from goes first.
    if let Some(context) = &context {
        let (kind, to, search) = (
            context.kind.clone(),
            context.to.clone(),
            context.search.clone(),
        );
        tauri::async_runtime::spawn(async move {
            crate::quick_access::played(&app, &kind, &to, &search).await;
        });
    }
    playback.arm_context(context);
}

/// Quick access: the audio library (playlists, albums, artists, shows, books and the fixed
/// blocks), the most recently played from first, with the pins; `names` are the fixed blocks'
/// names in the UI's language, for sorting by name.
#[tauri::command]
#[specta::specta]
pub async fn quick_access_list(
    app: tauri::AppHandle,
    names: raygun_model::QuickAccessNames,
) -> Result<raygun_model::QuickAccessList, String> {
    crate::quick_access::list(&app, names).await
}

/// Shows or hides YouTube songs' videos (M46.51): they play as their audio file, and in YouTube's
/// embed while the video shows (the song playing now moves there).
#[tauri::command]
#[specta::specta]
pub fn playback_set_video(playback: tauri::State<'_, PlaybackService>, shown: bool) {
    playback.set_video(shown);
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

/// Queues songs from anywhere (M46.8): right after the current one with `next` (Play next), else
/// at the end. Each plays from the library's file when it has the song.
#[tauri::command]
#[specta::specta]
pub async fn playback_queue_tracks(
    collections: tauri::State<'_, CollectionsService>,
    playback: tauri::State<'_, PlaybackService>,
    tracks: Vec<TrackInput>,
    next: bool,
) -> Result<(), String> {
    let mut items = Vec::with_capacity(tracks.len());
    for track in tracks {
        items.push(collections.queue_item_for(track).await?);
    }
    if next {
        playback.enqueue_next(items);
    } else {
        playback.enqueue(items, false);
    }
    Ok(())
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

/// Opens the miniplayer (M46.13): a small window on top with the song and its controls. Async:
/// on Windows a window made in a synchronous command never loads its page (it stays blank).
#[tauri::command]
#[specta::specta]
pub async fn miniplayer_open(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    background: Option<String>,
) -> Result<(), String> {
    let zoom = settings::load(&db.0)
        .await
        .map(|settings| settings.zoom)
        .unwrap_or(100);
    crate::miniplayer::open(&app, background.as_deref(), zoom)
}

/// Closes the miniplayer; with `show_main`, Raygun's main window comes forward.
#[tauri::command]
#[specta::specta]
pub async fn miniplayer_close(app: tauri::AppHandle, show_main: bool) -> Result<(), String> {
    crate::miniplayer::close(&app, show_main)
}

/// Forgets an extension's cookies and stored data (M46.11), then reloads it.
#[tauri::command]
#[specta::specta]
pub async fn extension_clear_data(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    id: String,
) -> Result<Vec<ExtensionInfo>, String> {
    extensions.clear_data(&id).await
}

/// Downloads a Mihon extension's APK and checks it against its store, while
/// `Settings::run_source_code` is on (`ExtensionService::download_mihon_apk`).
#[tauri::command]
#[specta::specta]
pub async fn extension_mihon_apk(
    db: tauri::State<'_, AppDb>,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    repo_id: String,
    id: String,
) -> Result<raygun_model::MihonApk, String> {
    let settings = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    if !settings.run_source_code {
        return Err("Turn on Allow running source code first (Settings → Browse)".into());
    }
    extensions.download_mihon_apk(&repo_id, &id).await
}

/// Whether this device can run a Mihon extension's own code, and whether its tools are in place
/// (`MihonContainer`). Shown in a source's details and Settings.
#[tauri::command]
#[specta::specta]
pub async fn mihon_runtime_state(
    container: tauri::State<'_, std::sync::Arc<crate::mihon_container::MihonContainer>>,
) -> Result<raygun_model::MihonRuntimeState, String> {
    Ok(container.runtime_state())
}

/// Fetches the Mihon runtime's tools (desktop: a JRE and dex2jar) while
/// `Settings::run_source_code` is on; progress comes as `MihonRuntimeEvent`.
#[tauri::command]
#[specta::specta]
pub async fn mihon_runtime_setup(
    db: tauri::State<'_, AppDb>,
    container: tauri::State<'_, std::sync::Arc<crate::mihon_container::MihonContainer>>,
) -> Result<raygun_model::MihonRuntimeState, String> {
    let settings = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    if !settings.run_source_code {
        return Err("Turn on Allow running source code first (Settings → Browse)".into());
    }
    #[cfg(not(target_os = "android"))]
    container.setup().await?;
    Ok(container.runtime_state())
}

/// Stops a Mihon runtime setup that's fetching tools.
#[tauri::command]
#[specta::specta]
pub async fn mihon_runtime_cancel(
    container: tauri::State<'_, std::sync::Arc<crate::mihon_container::MihonContainer>>,
) -> Result<(), String> {
    #[cfg(not(target_os = "android"))]
    container.cancel_setup();
    let _ = container;
    Ok(())
}

/// Runs one `manga.source` call (`sources`, `popular`, `latest`, `search`, `details`, `chapters`,
/// `pages`) on an installed, checked Mihon APK through the container, while
/// `Settings::run_source_code` is on. A source's details page drives this to browse it; the
/// result is the host's JSON for that call.
#[tauri::command]
#[specta::specta]
pub async fn mihon_run(
    db: tauri::State<'_, AppDb>,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    container: tauri::State<'_, std::sync::Arc<crate::mihon_container::MihonContainer>>,
    pkg: String,
    method: String,
    params: String,
) -> Result<String, String> {
    let settings = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    if !settings.run_source_code {
        return Err("Turn on Allow running source code first (Settings → Browse)".into());
    }
    let params: serde_json::Value = serde_json::from_str(&params)
        .map_err(|error| format!("the call's params aren't JSON: {error}"))?;
    let _ = &extensions;
    #[cfg(not(target_os = "android"))]
    {
        let apk = extensions.mihon_apk_path(&pkg)?;
        let facts = raygun_ext_host::mihon::apk::read(
            &std::fs::read(&apk).map_err(|error| error.to_string())?,
        )?;
        let class = facts.source_class.ok_or("the APK names no source class")?;
        let extension = crate::mihon_container::ApkExtension {
            package: pkg.clone(),
            apk,
            class,
            cookies: Vec::new(),
        };
        let result = if method == "sources" {
            serde_json::Value::Array(container.sources(&extension).await?)
        } else {
            container.call(&extension, &method, params).await?
        };
        Ok(result.to_string())
    }
    #[cfg(target_os = "android")]
    {
        let _ = (pkg, method, params, container);
        Err("Running a source's own code on Android isn't available in this build yet".into())
    }
}

/// The source behind a failure's text whose browser check can be passed by hand, while
/// `Settings::manual_browser_checks` is on (`browser_check.rs`).
#[tauri::command]
#[specta::specta]
pub async fn browser_check_target(
    db: tauri::State<'_, AppDb>,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    checks: tauri::State<'_, crate::browser_check::BrowserChecks>,
    error: String,
    source: Option<String>,
) -> Result<Option<raygun_model::BrowserCheckTarget>, String> {
    let settings = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    Ok(checks
        .target(&settings, &extensions, &error, source.as_deref())
        .await)
}

/// Opens the request `extension_id`'s source met a browser check on, for the person to pass the
/// check, and waits until the browser closes. A passed check gives the extension the browser's
/// cookies and User-Agent, and the failed call is asked again; the result says how it went (a
/// page that loads without a check says so).
#[tauri::command]
#[specta::specta]
pub async fn browser_check_open(
    db: tauri::State<'_, AppDb>,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    checks: tauri::State<'_, crate::browser_check::BrowserChecks>,
    extension_id: String,
) -> Result<raygun_model::BrowserCheckResult, String> {
    let settings = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    checks.open(&settings, &extensions, &extension_id).await
}

/// Opens the site an extension's sources read in Raygun's browser (M46.11).
#[tauri::command]
#[specta::specta]
pub async fn extension_open_website(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    websites: Websites<'_>,
    id: String,
) -> Result<(), String> {
    let url = extensions
        .website(&id)
        .await
        .ok_or("This source doesn't name its website")?;
    websites
        .inner()
        .clone()
        .open_page(&format!("{}{id}", crate::websites::PAGE_PREFIX), &url)
        .await
}

/// Opens a title's page at its source in Raygun's browser (M46.38: the website button on comic,
/// book and video pages): the source's address for it, joined to the source's website when it's
/// only a path; without one, the website itself.
#[tauri::command]
#[specta::specta]
pub async fn catalog_open_page(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    websites: Websites<'_>,
    source_id: String,
    url: Option<String>,
) -> Result<(), String> {
    let extension_id = source_id.split('/').next().unwrap_or(&source_id).to_owned();
    let website = extensions.website(&extension_id).await;
    let address = page_address(url.as_deref(), website.as_deref())
        .ok_or("This source doesn't name its website")?;
    websites
        .inner()
        .clone()
        .open_page(
            &format!("{}{extension_id}", crate::websites::PAGE_PREFIX),
            &address,
        )
        .await
}

/// A title's page: `url` when it's a web address, else joined to `website`; else the website.
fn page_address(url: Option<&str>, website: Option<&str>) -> Option<String> {
    let url = url.map(str::trim).filter(|url| !url.is_empty());
    match url {
        Some(url) => match reqwest::Url::parse(url) {
            Ok(parsed) if matches!(parsed.scheme(), "http" | "https") => Some(parsed.to_string()),
            _ => reqwest::Url::parse(website?)
                .ok()?
                .join(url)
                .ok()
                .filter(|joined| matches!(joined.scheme(), "http" | "https"))
                .map(|joined| joined.to_string()),
        },
        None => website.map(str::to_owned),
    }
}

/// What an address typed in Sources → Add is (M46.52): a Stremio addon, a repository (Raygun's,
/// Shosetsu's, Nuclear's, Miru's, LNReader's, Kodi's, Mihon's), one extension, or a server; a
/// GitHub project's page stands for what it publishes, a folder for the index in it.
#[tauri::command]
#[specta::specta]
pub async fn sources_detect(
    detector: tauri::State<'_, crate::source_detect::SourceDetector>,
    address: String,
) -> Result<raygun_model::DetectedSource, String> {
    detector.detect(&address).await
}

/// Adds what an address is: the addon, the repository, or the extension installed. A server is
/// connected with its sign-in (`servers_add`).
#[tauri::command]
#[specta::specta]
pub async fn sources_add(
    detector: tauri::State<'_, crate::source_detect::SourceDetector>,
    address: String,
) -> Result<raygun_model::DetectedSource, String> {
    detector.add(&address).await
}

/// Shows the system's file or folder picker for a field that takes a link or a path; `null`
/// when nothing was picked. On Android the item is copied into Raygun's data folder.
#[tauri::command]
#[specta::specta]
pub async fn files_pick(
    app: tauri::AppHandle,
    paths: tauri::State<'_, AppPaths>,
    target: raygun_model::PickTarget,
) -> Result<Option<raygun_model::PickedPath>, String> {
    crate::picker::pick(&app, &paths.0, target).await
}

/// Repositories extensions can be installed from.
#[tauri::command]
#[specta::specta]
pub async fn extension_repos(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
) -> Result<Vec<ExtensionRepo>, String> {
    extensions.repos().await
}

/// Well-known repositories to add with one tap, marked when already added.
#[tauri::command]
#[specta::specta]
pub async fn extension_suggested_repos(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
) -> Result<Vec<raygun_model::SuggestedRepo>, String> {
    extensions.suggested_repos().await
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

/// What a repository holds, read from its index without adding it (the repository's `id` is
/// empty).
#[tauri::command]
#[specta::specta]
pub async fn extension_repo_preview(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    url: String,
) -> Result<RepoCatalog, String> {
    extensions.preview_repo(&url).await
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

/// Adds installs to the queue (one, a selection, every update); they run in the background.
#[tauri::command]
#[specta::specta]
pub fn extension_installs_enqueue(
    installs: tauri::State<'_, Arc<ExtensionInstalls>>,
    entries: Vec<raygun_model::ExtensionInstallRequest>,
) -> raygun_model::ExtensionInstallStatus {
    installs.enqueue(&entries)
}

#[tauri::command]
#[specta::specta]
pub fn extension_installs_status(
    installs: tauri::State<'_, Arc<ExtensionInstalls>>,
) -> raygun_model::ExtensionInstallStatus {
    installs.status()
}

/// Takes one waiting install out of the queue (a running one finishes).
#[tauri::command]
#[specta::specta]
pub fn extension_installs_cancel(
    installs: tauri::State<'_, Arc<ExtensionInstalls>>,
    repo_id: String,
    extension_id: String,
) -> raygun_model::ExtensionInstallStatus {
    installs.cancel(&repo_id, &extension_id)
}

/// Stops the installs still waiting; the running one finishes.
#[tauri::command]
#[specta::specta]
pub fn extension_installs_cancel_all(
    installs: tauri::State<'_, Arc<ExtensionInstalls>>,
) -> raygun_model::ExtensionInstallStatus {
    installs.cancel_all()
}

/// Queues every failed install again.
#[tauri::command]
#[specta::specta]
pub fn extension_installs_retry(
    installs: tauri::State<'_, Arc<ExtensionInstalls>>,
) -> raygun_model::ExtensionInstallStatus {
    installs.retry_failed()
}

/// Forgets a failed install, or every failure when no extension is named.
#[tauri::command]
#[specta::specta]
pub fn extension_installs_dismiss(
    installs: tauri::State<'_, Arc<ExtensionInstalls>>,
    repo_id: Option<String>,
    extension_id: Option<String>,
) -> raygun_model::ExtensionInstallStatus {
    installs.dismiss(repo_id.as_deref().zip(extension_id.as_deref()))
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

/// The servers connected (Jellyfin, Emby, Subsonic, Audiobookshelf, OPDS catalogues), each
/// with why it isn't working when it isn't.
#[tauri::command]
#[specta::specta]
pub async fn servers_list(
    servers: tauri::State<'_, crate::servers::ServerService>,
) -> Result<Vec<raygun_model::ServerInfo>, String> {
    servers.list().await
}

/// Signs in to a server and connects it; the same server and user again signs in anew.
#[tauri::command]
#[specta::specta]
pub async fn servers_add(
    servers: tauri::State<'_, crate::servers::ServerService>,
    input: raygun_model::ServerInput,
) -> Result<Vec<raygun_model::ServerInfo>, String> {
    servers.add(input).await
}

/// Disconnects a server and forgets its sign-in.
#[tauri::command]
#[specta::specta]
pub async fn servers_remove(
    servers: tauri::State<'_, crate::servers::ServerService>,
    id: String,
) -> Result<Vec<raygun_model::ServerInfo>, String> {
    servers.remove(&id).await
}

/// Lyrics for a track, or `null` when no provider has any. The song's own lyrics (M46.12) win, and
/// its lyrics offset comes along, with every translation kept for the song lined up under the
/// lines (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_get(
    lyrics: tauri::State<'_, Arc<crate::lyrics_store::LyricsStore>>,
    sound: tauri::State<'_, Arc<crate::sound::SoundService>>,
    translations: tauri::State<'_, Arc<crate::lyrics_translation::LyricsTranslations>>,
    query: TrackQuery,
) -> Result<Option<Lyrics>, String> {
    // Kept on the device, and asked again with simpler words when none come (M46.46).
    let fetched = if sound.has_own_lyrics(&query) {
        lyrics.touch(&query).await;
        None
    } else {
        lyrics.lyrics(&query).await?
    };
    let Some(shown) = sound.lyrics_untrimmed(&query, fetched) else {
        return Ok(None);
    };
    let shown = translations.attach(&query, shown).await;
    Ok(Some(sound.trim(&query, shown)))
}

/// Translates a song's lyrics into `language` (a BCP 47 tag) with `translator`: `device` (a model
/// from the AI catalogue, downloaded first) or `ai:<provider id>`. The translation is kept and
/// comes with the song's lyrics from then on (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_translate(
    translations: tauri::State<'_, Arc<crate::lyrics_translation::LyricsTranslations>>,
    query: TrackQuery,
    language: String,
    translator: String,
) -> Result<raygun_model::LyricsTranslation, String> {
    translations.translate(&query, &language, &translator).await
}

/// Who can translate a song's lyrics into `language`: this device, with what it would download,
/// then the translators from Settings → AI (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_translate_options(
    translations: tauri::State<'_, Arc<crate::lyrics_translation::LyricsTranslations>>,
    query: TrackQuery,
    language: String,
) -> Result<Vec<raygun_model::LyricsTranslatorOption>, String> {
    translations.options(&query, &language).await
}

/// Makes one of a song's kept translations the one that shows first (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_translation_pick(
    translations: tauri::State<'_, Arc<crate::lyrics_translation::LyricsTranslations>>,
    query: TrackQuery,
    language: String,
    translator: String,
) -> Result<(), String> {
    translations.pick(&query, &language, &translator).await
}

/// Forgets a song's kept lyrics, transcription and own lyrics (M46.46: Delete saved lyrics).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_forget(
    lyrics: tauri::State<'_, Arc<crate::lyrics_store::LyricsStore>>,
    query: TrackQuery,
) -> Result<(), String> {
    lyrics.forget(&query).await
}

/// How many songs have lyrics and transcriptions kept (M46.46).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_cache_usage(
    lyrics: tauri::State<'_, Arc<crate::lyrics_store::LyricsStore>>,
) -> Result<raygun_model::LyricsCacheUsage, String> {
    lyrics.usage().await
}

/// Clears kept lyrics (and transcriptions when asked) of the songs `scope` names; returns how
/// many songs lost something (M46.46).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_cache_clear(
    lyrics: tauri::State<'_, Arc<crate::lyrics_store::LyricsStore>>,
    scope: raygun_model::LyricsClearScope,
    transcriptions: bool,
) -> Result<u32, String> {
    lyrics.clear(scope, transcriptions).await
}

/// Hears several songs' words one after another, in order (M46.46).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_transcribe_many(
    captions: tauri::State<'_, Arc<crate::captions::CaptionService>>,
    queries: Vec<TrackQuery>,
) -> Result<(), String> {
    captions.transcribe_songs(queries).await
}

/// Every lyrics source's lyrics for a song (M46.36): the user picks which to keep.
#[tauri::command]
#[specta::specta]
pub async fn lyrics_options(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    captions: tauri::State<'_, Arc<crate::captions::CaptionService>>,
    lyrics: tauri::State<'_, Arc<crate::lyrics_store::LyricsStore>>,
    query: TrackQuery,
) -> Result<Vec<Lyrics>, String> {
    let mut options = extensions.all_lyrics(&query).await?;
    // Their translations follow whichever version is picked (M46.48).
    lyrics.keep_sources(&query, &options).await;
    // What a transcription heard before stays one of the choices (M46.45).
    if let Some(heard) = captions.song_transcript(&query).await {
        options.push(heard);
    }
    Ok(options)
}

/// Hears a song's words with the transcription set up in Settings → AI, into its own synced
/// lyrics (M46.36). Follow it with `captions` on the key it returns (`song:<track key>`).
#[tauri::command]
#[specta::specta]
pub async fn lyrics_transcribe(
    captions: tauri::State<'_, Arc<crate::captions::CaptionService>>,
    query: TrackQuery,
) -> Result<String, String> {
    captions.transcribe_song(query.clone()).await?;
    Ok(crate::captions::song_key(&query))
}

/// A song's own trims, volume and lyrics edits (M46.12); defaults when it has none.
#[tauri::command]
#[specta::specta]
pub fn song_edit_get(
    sound: tauri::State<'_, Arc<crate::sound::SoundService>>,
    query: TrackQuery,
) -> raygun_model::SongEdit {
    sound.edit(&crate::sound::key(&query))
}

/// Saves a song's edit; one with nothing changed forgets it. The playing song changes at once.
#[tauri::command]
#[specta::specta]
pub async fn song_edit_set(
    sound: tauri::State<'_, Arc<crate::sound::SoundService>>,
    query: TrackQuery,
    edit: raygun_model::SongEdit,
) -> Result<raygun_model::SongEdit, String> {
    sound.set(&query, edit).await
}

/// Local library tracks shown above online results.
const SEARCH_LIBRARY_LIMIT: u32 = 8;

/// Searches the local library and every online music provider at once. Each provider answers
/// with tracks or an error; songs several providers found are merged into `top`. Each answer is
/// also sent as a [`crate::events::SearchProgressEvent`] as it comes, in the order they came.
#[tauri::command]
#[specta::specta]
pub async fn online_search(
    app: tauri::AppHandle,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    library: tauri::State<'_, LibraryService>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    query: String,
) -> Result<SearchResults, String> {
    let trimmed = query.trim();
    let details = tauri::Manager::state::<Arc<crate::details::DetailService>>(&app);
    // Search's "No 18+" and "No explicit": 18+ music sources aren't asked, explicit songs drop.
    let filter = crate::search_filter::current(&app).await;
    let listings = crate::search_filter::listings(&app).await;
    let songs = extensions.search_as_answered(trimmed, listings, |result, total| {
        // Lengths the sources sent are kept for lists that lack them (M46.26).
        durations.learn_tracks(&result.tracks);
        let mut result = result.clone();
        crate::search_filter::keep_songs(filter, &mut result);
        details.apply_online(&mut result.tracks);
        report_progress(
            &app,
            trimmed,
            total,
            raygun_model::SearchPart::Songs { result },
        );
    });
    let songs = raygun_ext_host::without_adult_sources(
        crate::search_filter::skips_adult_music(filter),
        songs,
    );
    let (local, providers) =
        futures_util::future::join(library.search(&query, SEARCH_LIBRARY_LIMIT), songs).await;
    let library = local.unwrap_or_else(|message| {
        tracing::warn!(%message, "library search failed");
        Vec::new()
    });
    let mut providers = providers;
    for result in &mut providers {
        crate::search_filter::keep_songs(filter, result);
        details.apply_online(&mut result.tracks);
    }
    Ok(SearchResults {
        library,
        top: crate::search::merge_top(&providers),
        providers,
    })
}

/// Tells Search what one source answered while the others are still asked.
fn report_progress(
    app: &tauri::AppHandle,
    query: &str,
    total: usize,
    part: raygun_model::SearchPart,
) {
    use tauri_specta::Event;
    let progress = raygun_model::SearchProgress {
        query: query.to_owned(),
        total: u32::try_from(total).unwrap_or(u32::MAX),
        part,
    };
    if let Err(error) = crate::events::SearchProgressEvent(progress).emit(app) {
        tracing::warn!(%error, "could not send a search's progress");
    }
}

/// Searches kept on this device, newest first.
#[tauri::command]
#[specta::specta]
pub async fn search_recent(
    db: tauri::State<'_, AppDb>,
) -> Result<Vec<raygun_model::RecentSearch>, String> {
    crate::recent_searches::list(&db.0).await
}

/// Remembers a search (`kind` is Search's kind); answers with the searches kept. Nothing is
/// remembered while incognito is on (M47).
#[tauri::command]
#[specta::specta]
pub async fn search_recent_add(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    query: String,
    kind: String,
) -> Result<Vec<raygun_model::RecentSearch>, String> {
    if crate::privacy::incognito(&app) {
        return crate::recent_searches::list(&db.0).await;
    }
    let now = crate::catalog::now_ms() as f64;
    crate::recent_searches::add(&db.0, &query, &kind, now).await
}

/// Forgets one kept search; answers with the searches left.
#[tauri::command]
#[specta::specta]
pub async fn search_recent_remove(
    db: tauri::State<'_, AppDb>,
    query: String,
    kind: String,
) -> Result<Vec<raygun_model::RecentSearch>, String> {
    crate::recent_searches::remove(&db.0, &query, &kind).await
}

/// Forgets every kept search.
#[tauri::command]
#[specta::specta]
pub async fn search_recent_clear(db: tauri::State<'_, AppDb>) -> Result<(), String> {
    crate::recent_searches::clear(&db.0).await
}

/// Songs an artist page shows (M46.8).
const ARTIST_POPULAR_LIMIT: usize = 20;

/// Artists for a search, from every source that searches artists (M46.9); each source's are also
/// sent as a [`crate::events::SearchProgressEvent`] as they come.
#[tauri::command]
#[specta::specta]
pub async fn music_search_artists(
    app: tauri::AppHandle,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: String,
) -> Result<Vec<raygun_model::MusicArtist>, String> {
    let query = query.trim();
    let filter = crate::search_filter::current(&app).await;
    let found = extensions.search_artists(query, |provider, artists, total| {
        let part = raygun_model::SearchPart::Artists {
            provider: provider.clone(),
            artists: artists.to_vec(),
        };
        report_progress(&app, query, total, part);
    });
    Ok(raygun_ext_host::without_adult_sources(
        crate::search_filter::skips_adult_music(filter),
        found,
    )
    .await)
}

/// Albums for a search, from every source that searches albums (M46.9); each source's are also
/// sent as a [`crate::events::SearchProgressEvent`] as they come.
#[tauri::command]
#[specta::specta]
pub async fn music_search_albums(
    app: tauri::AppHandle,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: String,
) -> Result<Vec<raygun_model::MusicAlbum>, String> {
    let query = query.trim();
    let filter = crate::search_filter::current(&app).await;
    let found = extensions.search_albums(query, |provider, albums, total| {
        let part = raygun_model::SearchPart::Albums {
            provider: provider.clone(),
            albums: albums.to_vec(),
        };
        report_progress(&app, query, total, part);
    });
    Ok(raygun_ext_host::without_adult_sources(
        crate::search_filter::skips_adult_music(filter),
        found,
    )
    .await)
}

/// Where this device's sound can come out (M46.43). Android lists its outputs; desktop's are the
/// webview's own, so this is empty there.
#[tauri::command]
#[specta::specta]
pub fn audio_outputs(app: tauri::AppHandle) -> raygun_model::AudioOutputs {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let answer = app
            .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .audio_outputs();
        match answer.map(serde_json::from_value::<raygun_model::AudioOutputs>) {
            Ok(Ok(outputs)) => return outputs,
            Ok(Err(error)) => tracing::warn!(%error, "unreadable audio outputs"),
            Err(error) => tracing::warn!(%error, "no audio outputs"),
        }
    }
    let _ = app;
    raygun_model::AudioOutputs::default()
}

/// Plays through one of this device's outputs, or the system's choice for `null` (M46.43).
#[tauri::command]
#[specta::specta]
pub fn audio_output_set(app: tauri::AppHandle, id: Option<String>) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .set_audio_output(id)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, id);
        Ok(())
    }
}

/// Hides Android's status and navigation bars while a reader is open and its own bars are put
/// away, or shows them again. Desktop has F11 instead, so this does nothing there.
#[tauri::command]
#[specta::specta]
pub fn reader_system_bars(app: tauri::AppHandle, hidden: bool) -> Result<(), String> {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .system_bars(hidden)
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = (app, hidden);
        Ok(())
    }
}

/// Followed artists, the latest followed first (M46.42).
#[tauri::command]
#[specta::specta]
pub async fn artists_followed(
    follows: tauri::State<'_, Arc<crate::follows::ArtistFollows>>,
) -> Result<Vec<raygun_model::FollowedArtist>, String> {
    Ok(follows.list().await)
}

/// Follows an artist: the library and quick access (M46.42).
#[tauri::command]
#[specta::specta]
pub async fn artist_follow(
    follows: tauri::State<'_, Arc<crate::follows::ArtistFollows>>,
    name: String,
    image_url: Option<String>,
) -> Result<(), String> {
    follows.follow(&name, image_url).await
}

#[tauri::command]
#[specta::specta]
pub async fn artist_unfollow(
    follows: tauri::State<'_, Arc<crate::follows::ArtistFollows>>,
    name: String,
) -> Result<(), String> {
    follows.unfollow(&name).await
}

/// Turns alerts for an artist's new releases on or off (M46.42).
#[tauri::command]
#[specta::specta]
pub async fn artist_set_alerts(
    follows: tauri::State<'_, Arc<crate::follows::ArtistFollows>>,
    name: String,
    on: bool,
) -> Result<(), String> {
    follows.set_alerts(&name, on).await
}

/// New releases found for followed artists, newest first (M46.42).
#[tauri::command]
#[specta::specta]
pub async fn artist_release_news(
    follows: tauri::State<'_, Arc<crate::follows::ArtistFollows>>,
) -> Result<Vec<raygun_model::ArtistReleaseNews>, String> {
    Ok(follows.news().await)
}

/// Checks followed artists with alerts for new releases now; returns the news afterwards.
#[tauri::command]
#[specta::specta]
pub async fn artist_releases_check(
    follows: tauri::State<'_, Arc<crate::follows::ArtistFollows>>,
) -> Result<Vec<raygun_model::ArtistReleaseNews>, String> {
    follows.check_now().await;
    Ok(follows.news().await)
}

/// An artist's coming concerts and other events (M46.42).
#[tauri::command]
#[specta::specta]
pub async fn music_artist_events(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    name: String,
) -> Result<Vec<raygun_model::ArtistEvent>, String> {
    let name = details.original_artist(name.trim());
    Ok(extensions.artist_events(&name).await)
}

/// Playlists for a search from every source that searches them; `featured` keeps the services'
/// own (M46.39). Each source's are also sent as a [`crate::events::SearchProgressEvent`] as they
/// come.
#[tauri::command]
#[specta::specta]
pub async fn music_search_playlists(
    app: tauri::AppHandle,
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: String,
    featured: bool,
) -> Result<Vec<raygun_model::MusicPlaylist>, String> {
    let query = query.trim();
    let filter = crate::search_filter::current(&app).await;
    let found = extensions.search_playlists(query, featured, |provider, playlists, total| {
        let part = raygun_model::SearchPart::Playlists {
            provider: provider.clone(),
            featured,
            playlists: playlists.to_vec(),
        };
        report_progress(&app, query, total, part);
    });
    Ok(raygun_ext_host::without_adult_sources(
        crate::search_filter::skips_adult_music(filter),
        found,
    )
    .await)
}

/// A source's playlist with its songs (M46.39).
#[tauri::command]
#[specta::specta]
pub async fn music_playlist(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    provider_id: String,
    playlist_id: String,
) -> Result<raygun_model::MusicPlaylistDetail, String> {
    let mut detail = extensions
        .music_playlist(&provider_id, &playlist_id)
        .await?;
    durations.learn_tracks(&detail.tracks);
    details.apply_online(&mut detail.tracks);
    Ok(detail)
}

/// An artist's albums, singles and more from every source that lists them (M46.36).
#[tauri::command]
#[specta::specta]
pub async fn music_artist_releases(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    name: String,
) -> Result<Vec<raygun_model::MusicAlbum>, String> {
    let name = details.original_artist(name.trim());
    if name.is_empty() {
        return Ok(Vec::new());
    }
    Ok(extensions.artist_releases(&name).await)
}

/// The album a song is on, from the sources' album search (M46.36: Go to album).
#[tauri::command]
#[specta::specta]
pub async fn music_find_album(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    album: String,
    artist: Option<String>,
) -> Result<Option<raygun_model::MusicAlbum>, String> {
    if album.trim().is_empty() {
        return Ok(None);
    }
    Ok(extensions
        .find_album(album.trim(), artist.as_deref().map(str::trim))
        .await)
}

/// An album's songs from the source that knows it (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_album(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    provider_id: String,
    album_id: String,
) -> Result<raygun_model::MusicAlbumDetail, String> {
    let mut detail = extensions.music_album(&provider_id, &album_id).await?;
    durations.learn_tracks(&detail.tracks);
    // The user's own details (M46.30).
    let details = tauri::Manager::state::<Arc<crate::details::DetailService>>(durations.app());
    details.apply_online(&mut detail.tracks);
    let artist = detail.album.artist.clone();
    details.apply_album(
        artist.as_deref(),
        &mut detail.album.title,
        &mut detail.album.artwork_url,
    );
    Ok(detail)
}

/// Queues songs to save into the app (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn songs_download(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    tracks: Vec<TrackInput>,
) -> Result<(), String> {
    songs.enqueue(tracks).await
}

/// The download of one song, if it has one (M46.18: the download button beside +).
#[tauri::command]
#[specta::specta]
pub async fn song_download_for(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    track: TrackInput,
) -> Result<Option<raygun_model::SongDownload>, String> {
    songs.find(&track).await
}

/// Songs downloaded into the app, as a collection (M46.18: Library → Audio → Downloaded).
#[tauri::command]
#[specta::specta]
pub async fn downloaded_songs(
    collections: tauri::State<'_, CollectionsService>,
) -> Result<Vec<SavedTrack>, String> {
    collections.downloaded().await
}

/// Every song download, newest first (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn song_downloads(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
) -> Result<Vec<raygun_model::SongDownload>, String> {
    songs.list().await
}

/// One song download as it is now; `None` once it was removed.
#[tauri::command]
#[specta::specta]
pub async fn song_download(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
) -> Result<Option<raygun_model::SongDownload>, String> {
    songs.get(&id).await
}

/// Removes a song download and its file (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn song_download_remove(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
) -> Result<(), String> {
    songs.remove(&id).await
}

/// Removes several song downloads and their files at once (cancels waiting ones).
#[tauri::command]
#[specta::specta]
pub async fn song_downloads_remove(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    ids: Vec<String>,
) -> Result<(), String> {
    songs.remove_many(&ids).await
}

/// Queues a failed song download again, first in the queue (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn song_download_retry(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
) -> Result<(), String> {
    songs.retry(&id).await
}

/// Queues every failed song download again; returns how many.
#[tauri::command]
#[specta::specta]
pub async fn song_downloads_retry_failed(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
) -> Result<u32, String> {
    songs.retry_failed().await
}

/// Puts the queued song downloads in this order, first to download first.
#[tauri::command]
#[specta::specta]
pub async fn song_downloads_reorder(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    ids: Vec<String>,
) -> Result<(), String> {
    songs.reorder(&ids).await
}

/// Whether song downloads are paused.
#[tauri::command]
#[specta::specta]
pub fn song_downloads_paused(songs: tauri::State<'_, Arc<SongDownloadService>>) -> bool {
    songs.paused()
}

/// Pauses song downloads (the running one waits, keeping what it saved) or resumes them.
#[tauri::command]
#[specta::specta]
pub fn song_downloads_set_paused(songs: tauri::State<'_, Arc<SongDownloadService>>, paused: bool) {
    songs.set_paused(paused);
}

/// Checks downloaded songs (`ids`, else all, M46.49): files gone or damaged, songs given another
/// source or other names since. Each download says what's wrong in its `problem`.
#[tauri::command]
#[specta::specta]
pub async fn song_downloads_check(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    ids: Option<Vec<String>>,
) -> Result<raygun_model::SongDownloadsChecked, String> {
    songs.check(ids).await
}

/// Downloads songs again, from the source each has now (M46.49): the copy that plays meanwhile is
/// kept until the new one is done. Returns how many were queued.
#[tauri::command]
#[specta::specta]
pub async fn song_downloads_redownload(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    ids: Vec<String>,
) -> Result<u32, String> {
    songs.redownload(ids).await
}

/// A piece of a background capture from the desktop webview (M46.49). An error means the capture
/// was stopped.
#[tauri::command]
#[specta::specta]
pub fn song_capture_report(report: raygun_model::SongCaptureReport) -> Result<(), String> {
    crate::music_downloads::capture::sound::report(report)
}

/// What the webview's recording player did (Android). An error means the recording was stopped.
#[tauri::command]
#[specta::specta]
pub fn song_recording_report(report: raygun_model::SongRecordingReport) -> Result<(), String> {
    crate::music_downloads::capture::record::report(report)
}

/// Asks to record songs as they play (Android: RECORD_AUDIO, then Android's capture question,
/// after the shell explained it). Allowed, the songs waiting to be recorded are, one at a time.
/// Returns whether it was allowed.
#[tauri::command]
#[specta::specta]
pub async fn song_recording_consent(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
) -> Result<bool, String> {
    songs.inner().clone().allow_recording().await
}

/// "Not now" to recording songs as they play: they wait without asking again until retried.
#[tauri::command]
#[specta::specta]
pub fn song_recording_decline(songs: tauri::State<'_, Arc<SongDownloadService>>) {
    songs.decline_recording();
}

/// Stops the song being recorded: it waits to be recorded again, and recording is given back.
#[tauri::command]
#[specta::specta]
pub async fn song_download_cancel_recording(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
) -> Result<(), String> {
    songs.cancel_recording(&id).await
}

/// Writes saved songs out as files (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn songs_save(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    ids: Vec<String>,
    format: raygun_model::AudioFileFormat,
    destination: raygun_model::SongDestination,
) -> Result<raygun_model::SongsSaved, String> {
    songs.save(ids, format, destination).await
}

/// Keys of every song in Favorites or a playlist, to mark them in lists (M46.45).
#[tauri::command]
#[specta::specta]
pub async fn collections_saved_keys(
    collections: tauri::State<'_, CollectionsService>,
) -> Result<Vec<String>, String> {
    collections.saved_keys().await
}

/// Downloads a song from a picked source (M46.47: Choose a source on a failed download), first
/// in the queue; the others are tried after it if it fails. `source` is the source's name, shown
/// while it downloads and in what went wrong.
#[tauri::command]
#[specta::specta]
pub async fn song_download_use_source(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
    track: raygun_model::OnlineTrack,
    source: Option<String>,
) -> Result<(), String> {
    songs.use_source(&id, track, source).await
}

/// A YouTube address typed in Search (M46.47): what it points at, with the video's title and
/// channel when YouTube says; `None` for anything else.
#[tauri::command]
#[specta::specta]
pub async fn youtube_link(text: String) -> Result<Option<raygun_model::YouTubeLink>, String> {
    let Some(link) = crate::ytdlp::parse_link(&text) else {
        return Ok(None);
    };
    let client = raygun_net::http_client().map_err(|error| error.to_string())?;
    Ok(Some(crate::ytdlp::describe(&client, link).await))
}

/// Plays a YouTube video from an address typed in Search, in YouTube's embed (M46.47).
#[tauri::command]
#[specta::specta]
pub async fn youtube_play(
    app: tauri::AppHandle,
    video_id: String,
    title: String,
    channel: Option<String>,
) -> Result<(), String> {
    crate::video::play_youtube(&app, &video_id, &title, channel).await
}

/// Raygun's newest release next to this copy (M46.47); `refresh` asks GitHub now.
#[tauri::command]
#[specta::specta]
pub async fn app_update(
    updates: tauri::State<'_, Arc<crate::updates::UpdateService>>,
    refresh: bool,
) -> Result<raygun_model::AppUpdate, String> {
    if refresh {
        updates.check().await
    } else {
        Ok(updates.current())
    }
}

/// Downloads the newest release and starts its installer, or opens its page (M46.47).
#[tauri::command]
#[specta::specta]
pub async fn app_update_install(
    updates: tauri::State<'_, Arc<crate::updates::UpdateService>>,
) -> Result<raygun_model::AppUpdate, String> {
    updates.install().await
}

/// Opens Raygun's release page or repository in the system's browser (M46.47).
#[tauri::command]
#[specta::specta]
pub fn app_open_page(
    updates: tauri::State<'_, Arc<crate::updates::UpdateService>>,
    url: String,
) -> Result<(), String> {
    updates.open_page(&url)
}

/// Opens an https link from release notes in the system's browser.
#[tauri::command]
#[specta::specta]
pub fn app_open_link(
    updates: tauri::State<'_, Arc<crate::updates::UpdateService>>,
    url: String,
) -> Result<(), String> {
    updates.open_link(&url)
}

/// This version's notes on its first start after an update, until `app_whats_new_seen`.
#[tauri::command]
#[specta::specta]
pub async fn app_whats_new(
    updates: tauri::State<'_, Arc<crate::updates::UpdateService>>,
) -> Result<Option<raygun_model::ReleaseNotes>, String> {
    Ok(updates.whats_new().await)
}

/// The "What's new" sheet was shown for this version.
#[tauri::command]
#[specta::specta]
pub async fn app_whats_new_seen(
    updates: tauri::State<'_, Arc<crate::updates::UpdateService>>,
) -> Result<(), String> {
    updates.whats_new_seen().await;
    Ok(())
}

/// The notes bundled with this version and the ones before it, newest first.
#[tauri::command]
#[specta::specta]
pub fn app_release_notes() -> Vec<raygun_model::ReleaseNotes> {
    crate::updates::bundled_notes()
}

/// yt-dlp and FFmpeg on this device (M46.47).
#[tauri::command]
#[specta::specta]
pub async fn ytdlp_status(
    ytdlp: tauri::State<'_, Arc<crate::ytdlp::YtDlpService>>,
) -> Result<raygun_model::YtDlpStatus, String> {
    Ok(ytdlp.status().await)
}

/// Fetches yt-dlp's latest release (checked against its checksums) (M46.47).
#[tauri::command]
#[specta::specta]
pub async fn ytdlp_install(
    ytdlp: tauri::State<'_, Arc<crate::ytdlp::YtDlpService>>,
) -> Result<raygun_model::YtDlpStatus, String> {
    ytdlp.install().await
}

/// Fetches FFmpeg for YouTube videos above 360p (Windows) (M46.47).
#[tauri::command]
#[specta::specta]
pub async fn ytdlp_install_ffmpeg(
    ytdlp: tauri::State<'_, Arc<crate::ytdlp::YtDlpService>>,
) -> Result<raygun_model::YtDlpStatus, String> {
    ytdlp.install_ffmpeg().await
}

/// Saves a YouTube video or its sound to Downloads/Raygun/YouTube (M46.47); the job's id.
#[tauri::command]
#[specta::specta]
pub fn ytdlp_download(
    ytdlp: tauri::State<'_, Arc<crate::ytdlp::YtDlpService>>,
    url: String,
    title: String,
    kind: raygun_model::YtDlpKind,
) -> Result<String, String> {
    ytdlp.download(&url, &title, kind)
}

/// yt-dlp's downloads, newest first (M46.47).
#[tauri::command]
#[specta::specta]
pub fn ytdlp_jobs(
    ytdlp: tauri::State<'_, Arc<crate::ytdlp::YtDlpService>>,
) -> Vec<raygun_model::YtDlpJob> {
    ytdlp.jobs()
}

/// Stops a yt-dlp download, or takes a finished one off the list (M46.47).
#[tauri::command]
#[specta::specta]
pub fn ytdlp_cancel(ytdlp: tauri::State<'_, Arc<crate::ytdlp::YtDlpService>>, id: String) {
    ytdlp.cancel(&id);
}

/// Shows a finished yt-dlp download in the file manager (M46.47).
#[tauri::command]
#[specta::specta]
pub fn ytdlp_reveal(
    ytdlp: tauri::State<'_, Arc<crate::ytdlp::YtDlpService>>,
    id: String,
) -> Result<(), String> {
    ytdlp.reveal(&id)
}

/// Keys of every downloaded song, to mark them in lists (M46.10).
#[tauri::command]
#[specta::specta]
pub fn song_downloaded_keys(songs: tauri::State<'_, Arc<SongDownloadService>>) -> Vec<String> {
    songs.downloaded_keys()
}

/// Every source's charts for a country (M46.9), cached for an hour.
#[tauri::command]
#[specta::specta]
pub async fn music_charts(
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    region: Option<String>,
) -> Result<raygun_model::MusicCharts, String> {
    Ok(discovery.charts(region).await)
}

/// The audiobooks in the library (M46.22).
#[tauri::command]
#[specta::specta]
pub async fn audiobooks(
    podcasts: Podcasts<'_>,
) -> Result<Vec<raygun_model::AudiobookSummary>, String> {
    podcasts.audiobooks().await
}

/// A book with its chapters in order (M46.22).
#[tauri::command]
#[specta::specta]
pub async fn audiobook(
    podcasts: Podcasts<'_>,
    id: String,
) -> Result<raygun_model::AudiobookView, String> {
    podcasts.audiobook(&id).await
}

/// Opens a book by its chapter feed; returns its id (M46.22).
#[tauri::command]
#[specta::specta]
pub async fn audiobook_open(podcasts: Podcasts<'_>, feed_url: String) -> Result<String, String> {
    podcasts.open_audiobook(&feed_url).await
}

/// Plays a book from a chapter, or where it was left, with the rest after it (M46.22).
#[tauri::command]
#[specta::specta]
pub async fn audiobook_play(
    podcasts: Podcasts<'_>,
    id: String,
    chapter_id: Option<String>,
) -> Result<(), String> {
    podcasts.play_audiobook(&id, chapter_id.as_deref()).await
}

/// A page of books from every audiobook source at once (M46.22): `popular`, `latest`, `search`
/// or `subject`; searches rank by title and author and a book two sources share shows once
/// (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn audiobook_directory(
    podcasts: Podcasts<'_>,
    list: String,
    query: String,
    page: u32,
) -> Result<raygun_model::AudiobookPage, String> {
    podcasts.audiobook_directory(&list, &query, page).await
}

/// Downloads every chapter of a book not downloaded yet, two at a time; returns how many were
/// queued (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn audiobook_download(podcasts: Podcasts<'_>, id: String) -> Result<u32, String> {
    podcasts.download_all(&id).await
}

/// Episodes and books started and not finished, the last listened first (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_continue_listening(
    podcasts: Podcasts<'_>,
    limit: u32,
) -> Result<Vec<raygun_model::ListeningItem>, String> {
    podcasts.continue_listening(limit).await
}

/// An episode's or chapter's captions, with where transcribing stands (M46.32).
#[tauri::command]
#[specta::specta]
pub async fn captions(
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    episode_id: String,
) -> Result<raygun_model::Captions, String> {
    captions.inner().captions(&episode_id).await
}

/// Transcribes an episode from `from_ms` with the engine set up in Settings → AI (M46.32).
#[tauri::command]
#[specta::specta]
pub async fn captions_transcribe(
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    episode_id: String,
    from_ms: u32,
) -> Result<(), String> {
    captions.inner().transcribe(&episode_id, from_ms).await
}

/// Stops transcribing an episode; what was heard stays (M46.32).
#[tauri::command]
#[specta::specta]
pub fn captions_stop(
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    episode_id: String,
) {
    captions.stop(&episode_id);
}

/// Goes back to an episode's own transcript from what AI heard (M46.36).
#[tauri::command]
#[specta::specta]
pub async fn captions_use_feed(
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    episode_id: String,
) -> Result<(), String> {
    captions.use_feed(&episode_id).await
}

/// Every Stremio addon's ratings for a title, with their sites (M46.45).
#[tauri::command]
#[specta::specta]
pub async fn stremio_ratings(
    stremio: tauri::State<'_, Arc<crate::stremio::StremioService>>,
    kind: String,
    id: String,
) -> Result<Vec<raygun_model::TitleRating>, String> {
    stremio.ratings(&kind, &id).await
}

/// Uses AI captions instead of the feed's transcript (M46.45): what AI heard before comes
/// back, else transcribing starts at `from_ms`; the feed's is kept to switch back to.
#[tauri::command]
#[specta::specta]
pub async fn captions_use_ai(
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    episode_id: String,
    from_ms: u32,
) -> Result<(), String> {
    captions.inner().use_ai(&episode_id, from_ms).await
}

/// Forgets an episode's captions, to transcribe it again (M46.32).
#[tauri::command]
#[specta::specta]
pub async fn captions_clear(
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    episode_id: String,
) -> Result<(), String> {
    captions.clear(&episode_id).await
}

/// Makes AI subtitles for the video playing at `url` (the player's address), staying ahead of
/// `from_ms`; called again as the viewer moves on (M46.33).
#[tauri::command]
#[specta::specta]
pub async fn captions_video_transcribe(
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    key: String,
    url: String,
    format: Option<String>,
    from_ms: u32,
) -> Result<(), String> {
    captions
        .inner()
        .transcribe_video(&key, &url, format, from_ms, false)
        .await
}

/// Makes AI subtitles for a downloaded video, the whole of it, in the background (M46.33).
#[tauri::command]
#[specta::specta]
pub async fn captions_video_make(
    app: tauri::AppHandle,
    captions: tauri::State<'_, std::sync::Arc<crate::captions::CaptionService>>,
    key: String,
) -> Result<(), String> {
    let offline = crate::video_downloads::offline(&app, &key)
        .await
        .ok_or_else(|| "This video isn't downloaded".to_owned())?;
    captions
        .inner()
        .transcribe_video(&key, &offline.url, offline.format, 0, true)
        .await
}

type VideoSubtitlesState<'a> = tauri::State<'a, Arc<crate::video_subtitles::VideoSubtitles>>;

/// Translates the subtitles a video shows (a track's lines, or its AI subtitles) into
/// `request.language` from where the viewer is, keeping ahead of them in batches; what was
/// translated before for the same lines carries on. With this device's translator, models it
/// lacks are downloaded only once `request.download` is set: until then `downloadBytes` says
/// how much and nothing starts.
#[tauri::command]
#[specta::specta]
pub async fn video_subtitles_translate(
    subtitles: VideoSubtitlesState<'_>,
    request: raygun_model::SubtitleTranslateRequest,
) -> Result<raygun_model::SubtitleTranslation, String> {
    subtitles.inner().translate(request).await
}

/// A video's translated subtitles: those translating now and those kept from before, with
/// their lines so far.
#[tauri::command]
#[specta::specta]
pub async fn video_subtitles_translations(
    subtitles: VideoSubtitlesState<'_>,
    key: String,
) -> Result<Vec<raygun_model::SubtitleTranslation>, String> {
    subtitles.translations(&key).await
}

/// Stops translating a video's subtitles track `id`; what's translated stays.
#[tauri::command]
#[specta::specta]
pub fn video_subtitles_stop(subtitles: VideoSubtitlesState<'_>, key: String, id: String) {
    subtitles.stop(&key, &id);
}

/// Who can translate a video's subtitles into `language`: this device, then the translators
/// from Settings → AI.
#[tauri::command]
#[specta::specta]
pub async fn video_subtitles_translators(
    subtitles: VideoSubtitlesState<'_>,
    language: String,
) -> Result<Vec<raygun_model::LyricsTranslatorOption>, String> {
    subtitles.translators(&language).await
}

/// What the readers' AI layers can do on this device.
#[tauri::command]
#[specta::specta]
pub fn reader_ai_support() -> raygun_model::ReaderAiSupport {
    crate::reader_ai::ReaderAi::support()
}

/// The words on a comic page (one of the reader's page links), in blocks, for a translation
/// layer.
#[tauri::command]
#[specta::specta]
pub async fn reader_ai_page_text(
    reader_ai: tauri::State<'_, Arc<crate::reader_ai::ReaderAi>>,
    url: String,
) -> Result<Vec<raygun_model::PageTextBlock>, String> {
    reader_ai.page_text(&url).await
}

/// Texts translated for a reader's translation layer, in order.
#[tauri::command]
#[specta::specta]
pub async fn reader_ai_translate(
    reader_ai: tauri::State<'_, Arc<crate::reader_ai::ReaderAi>>,
    request: raygun_model::ReaderAiTranslateRequest,
) -> Result<Vec<String>, String> {
    reader_ai.translate(request).await
}

/// A file of an installed model the readers run themselves (Anime4K's shaders).
#[tauri::command]
#[specta::specta]
pub fn ai_model_text(app: tauri::AppHandle, id: String) -> Result<String, String> {
    crate::reader_ai::model_text(&app, &id)
}

/// Whether a queue item is a podcast episode or a book's chapter, and whose (M46.32).
#[tauri::command]
#[specta::specta]
pub async fn spoken_item(
    podcasts: Podcasts<'_>,
    source: raygun_model::MediaSource,
) -> Result<Option<raygun_model::SpokenItem>, String> {
    podcasts.spoken_item(&source).await
}

/// The songs a Spotify playlist, album or song link points at (M46.21).
#[tauri::command]
#[specta::specta]
pub async fn import_read_link(
    import: tauri::State<'_, crate::import::ImportService>,
    link: String,
) -> Result<raygun_model::ImportedList, String> {
    import.read_link(&link).await
}

/// Songs in a CSV export or pasted lines (M46.21).
#[tauri::command]
#[specta::specta]
pub fn import_read_text(
    import: tauri::State<'_, crate::import::ImportService>,
    text: String,
    name: Option<String>,
) -> Result<raygun_model::ImportedList, String> {
    import.read_text(&text, name)
}

/// Whether screenshots can be read now (the text reader is built in or downloaded) (M46.21).
#[tauri::command]
#[specta::specta]
pub fn import_reader_ready(import: tauri::State<'_, crate::import::ImportService>) -> bool {
    import.reader_ready()
}

/// Downloads the text reader for screenshots where it isn't built in (M46.21).
#[tauri::command]
#[specta::specta]
pub async fn import_prepare_reader(
    import: tauri::State<'_, crate::import::ImportService>,
) -> Result<(), String> {
    import.prepare_reader().await
}

/// Songs in uploaded screenshots of song lists (M46.21); `join_split` joins artist lines read as
/// rows of their own back to their songs.
#[tauri::command]
#[specta::specta]
pub async fn import_read_screenshots(
    import: tauri::State<'_, crate::import::ImportService>,
    upload_ids: Vec<String>,
    join_split: bool,
) -> Result<raygun_model::ImportedList, String> {
    import.read_screenshots(upload_ids, join_split).await
}

/// The folders in Local Files and what each holds (M46.27).
#[tauri::command]
#[specta::specta]
pub async fn local_folders(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
) -> Result<Vec<raygun_model::LocalFolder>, String> {
    local.folders().await
}

/// Adds a folder the user picks in the system's picker to Local Files and scans it; `None` when
/// they cancelled.
#[tauri::command]
#[specta::specta]
pub async fn local_folder_add(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
) -> Result<Option<Vec<raygun_model::LocalFolder>>, String> {
    local.inner().add().await
}

/// Adds a folder chosen in Local Files' own folder browser, by its path, and scans it (M46.51).
#[tauri::command]
#[specta::specta]
pub async fn local_folder_add_path(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
    path: String,
) -> Result<Vec<raygun_model::LocalFolder>, String> {
    local.inner().add_path(&path).await
}

/// A folder of the device's storage for Local Files' folder browser; its top for `None` (M46.51).
#[tauri::command]
#[specta::specta]
pub async fn local_files_browse(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
    path: Option<String>,
) -> Result<raygun_model::LocalBrowse, String> {
    local.browse(path.as_deref()).await
}

/// Add files (M46.51): files picked in the system's file picker, imported or copied in; `None`
/// when the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn local_files_add_files(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
) -> Result<Option<raygun_model::LocalFilesAdded>, String> {
    local.inner().add_files().await
}

/// Takes a folder out of Local Files.
#[tauri::command]
#[specta::specta]
pub async fn local_folder_remove(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
    id: String,
) -> Result<Vec<raygun_model::LocalFolder>, String> {
    local.remove(&id).await
}

/// Scans a Local Files folder again, or every one; files that couldn't be added are tried again.
#[tauri::command]
#[specta::specta]
pub async fn local_folders_rescan(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
    id: Option<String>,
) -> Result<(), String> {
    local.inner().rescan(id.as_deref(), true).await
}

/// Asks for the media access Local Files' songs and videos need (Android); opens Raygun's page
/// in the system settings when the system won't ask. Whether it's allowed now.
#[tauri::command]
#[specta::specta]
pub async fn local_files_allow_access(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
) -> Result<bool, String> {
    local.inner().allow_access().await
}

/// The user's own details for a song, an album or an artist (M46.30).
#[tauri::command]
#[specta::specta]
pub async fn detail_edit(
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    target: raygun_model::DetailTarget,
) -> Result<raygun_model::DetailEdit, String> {
    Ok(details.get(&target))
}

/// Saves the user's details for a song, an album or an artist; an empty edit resets them.
#[tauri::command]
#[specta::specta]
pub async fn detail_edit_set(
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    target: raygun_model::DetailTarget,
    edit: raygun_model::DetailEdit,
) -> Result<raygun_model::DetailEdit, String> {
    details.set(&target, edit).await
}

/// Covers the music sources have for a song, an album or an artist (M46.36: Edit details).
#[tauri::command]
#[specta::specta]
pub async fn detail_cover_options(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    collections: tauri::State<'_, CollectionsService>,
    target: raygun_model::DetailTarget,
) -> Result<Vec<raygun_model::CoverOption>, String> {
    use raygun_model::DetailTarget;
    // A playlist's: the covers of its songs (M46.45).
    if let DetailTarget::Playlist { id } = &target {
        let detail = collections.playlist(id).await?;
        let mut options: Vec<raygun_model::CoverOption> = Vec::new();
        for item in detail.items {
            if let Some(url) = item.track.artwork_url
                && !options.iter().any(|option| option.url == url)
            {
                options.push(raygun_model::CoverOption {
                    url,
                    source: item.track.title,
                    of: item.track.artist,
                });
            }
        }
        return Ok(options);
    }
    // The names sources know, not the user's renames (M46.30).
    let target = match target {
        DetailTarget::Artist { name } => DetailTarget::Artist {
            name: details.original_artist(&name),
        },
        other => other,
    };
    Ok(extensions.cover_options(&target).await)
}

/// Another source's cover for a song, an album or an artist whose own is missing or failed to
/// load (M46.40), remembered on this device; `null` when no source has one.
#[tauri::command]
#[specta::specta]
pub async fn cover_fallback(
    fallbacks: tauri::State<'_, Arc<crate::cover_fallback::CoverFallbacks>>,
    collections: tauri::State<'_, CollectionsService>,
    images: tauri::State<'_, raygun_net::ImageLinks>,
    target: raygun_model::DetailTarget,
) -> Result<Option<String>, String> {
    let url = fallbacks.cover(&target).await;
    // Through the cover cache, like every other cover (M46.2); a saved song's stays there
    // whatever the size limit (M46.46).
    let saved = match &target {
        raygun_model::DetailTarget::Song { query } => collections
            .saved_keys()
            .await
            .unwrap_or_default()
            .contains(&raygun_resolver::track_key(query)),
        _ => false,
    };
    Ok(url.map(|url| {
        if saved {
            images.kept_cover(&url, &Default::default())
        } else {
            images.cover(&url, &Default::default())
        }
    }))
}

/// Keeps a picked image (an upload) as a cover; returns its address for an edit.
#[tauri::command]
#[specta::specta]
pub async fn detail_cover_import(
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    uploads: tauri::State<'_, crate::uploads::Uploads>,
    upload_id: String,
) -> Result<String, String> {
    let path = uploads.take(&upload_id)?;
    details.import_cover(path).await
}

/// Saves a cover image (a song's, an album's, an artist's picture) at the source's largest size, as
/// PNG, JPEG or WebP; returns where it went (M46.26).
#[tauri::command]
#[specta::specta]
pub async fn cover_save(
    app: tauri::AppHandle,
    url: String,
    title: String,
    format: raygun_model::ImageFileFormat,
    destination: raygun_model::ImageDestination,
) -> Result<String, String> {
    crate::export::covers::save(&app, &url, &title, format, destination).await
}

/// Links to the larger versions of a cover, poster or backdrop that its host serves, largest
/// first, the cover itself last: the cover viewer shows the first that loads.
#[tauri::command]
#[specta::specta]
pub fn cover_versions(app: tauri::AppHandle, url: String) -> Vec<String> {
    crate::export::covers::shown_versions(&app, &url)
}

/// Opens the share sheet for a cover at its largest size (desktop: saves it to Pictures and shows
/// it in its folder).
#[tauri::command]
#[specta::specta]
pub async fn cover_share(app: tauri::AppHandle, url: String, title: String) -> Result<(), String> {
    crate::export::covers::share(&app, &url, &title, false).await
}

/// Puts a cover at its largest size on the clipboard.
#[tauri::command]
#[specta::specta]
pub async fn cover_copy(app: tauri::AppHandle, url: String, title: String) -> Result<(), String> {
    crate::export::covers::share(&app, &url, &title, true).await
}

/// Song lengths Raygun knows for `tracks`, in their order; the rest are looked up in the
/// background and arrive as `TrackDurationsEvent`s (M46.26).
#[tauri::command]
#[specta::specta]
pub async fn track_durations(
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    tracks: Vec<TrackQuery>,
) -> Result<Vec<raygun_model::TrackDuration>, String> {
    durations.lookup(tracks).await
}

/// Gives every saved copy of a song the source the user picked (M46.31).
#[tauri::command]
#[specta::specta]
pub async fn track_assign_source(
    import: tauri::State<'_, crate::import::ImportService>,
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    song: TrackInput,
    chosen: raygun_model::OnlineTrack,
) -> Result<(), String> {
    import.assign_source(song.clone(), chosen).await?;
    check_download_later(songs.inner().clone(), song);
    Ok(())
}

/// A source picked for a song (M46.48): with `choice.assign`, its stream for every saved copy,
/// the queue and the resolver's remembered pick; the details `choice` ticks become the song's
/// own (Edit details reverts them).
#[tauri::command]
#[specta::specta]
pub async fn track_use_source(
    collections: tauri::State<'_, CollectionsService>,
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    song: TrackInput,
    chosen: raygun_model::OnlineTrack,
    choice: raygun_model::SourceChoice,
) -> Result<(), String> {
    collections
        .assign_source(song.clone(), chosen, choice)
        .await?;
    // A downloaded song now has another source than its copy (M46.49).
    check_download_later(songs.inner().clone(), song);
    Ok(())
}

/// The source Raygun found for a song by itself and plays it from, with what that source has for
/// it (names, album, cover), for "Use details from…"; `null` until one played it.
#[tauri::command]
#[specta::specta]
pub async fn track_found_source(
    collections: tauri::State<'_, CollectionsService>,
    song: TrackInput,
) -> Result<Option<raygun_model::SongCandidate>, String> {
    collections.found_source(&song).await
}

/// Checks a song's download in the background once its source changed (M46.49).
fn check_download_later(songs: Arc<SongDownloadService>, song: TrackInput) {
    tauri::async_runtime::spawn(async move { songs.check_song(&song).await });
}

/// Whether a saved song came in through Import songs, and whether no source had it then
/// (M46.48: picking a source offers its details by default for those).
#[tauri::command]
#[specta::specta]
pub async fn track_import_status(
    collections: tauri::State<'_, CollectionsService>,
    song: TrackInput,
) -> Result<raygun_model::TrackImportStatus, String> {
    collections.import_status(&song).await
}

/// Which saved songs no source had when imported (M46.31).
#[tauri::command]
#[specta::specta]
pub async fn tracks_unavailable(
    import: tauri::State<'_, crate::import::ImportService>,
    keys: Vec<String>,
) -> Result<Vec<String>, String> {
    import.unavailable(keys).await
}

/// Matches imported songs against the music sources (M46.21).
#[tauri::command]
#[specta::specta]
pub async fn import_match(
    import: tauri::State<'_, crate::import::ImportService>,
    songs: Vec<raygun_model::ImportedSong>,
) -> Result<Vec<raygun_model::MatchedSong>, String> {
    Ok(import.match_songs(songs).await)
}

/// Saves imported songs as a new playlist, into one, or into Favorites; returns the playlist's
/// id or `favorites` (M46.21, M46.45). Songs already there stay out unless `add_existing`.
#[tauri::command]
#[specta::specta]
pub async fn import_save(
    import: tauri::State<'_, crate::import::ImportService>,
    target: raygun_model::ImportTarget,
    songs: Vec<raygun_model::MatchedSong>,
    add_existing: bool,
) -> Result<String, String> {
    import.save(target, songs, add_existing).await
}

/// Made for You (M46.20): today's mixes (made in the background when they're from another day,
/// or when `refresh`) and the listening numbers. `utc_offset_minutes` is the user's clock.
#[tauri::command]
#[specta::specta]
pub async fn made_for_you(
    service: tauri::State<'_, Arc<crate::made_for_you::MadeForYouService>>,
    refresh: bool,
    utc_offset_minutes: i32,
) -> Result<raygun_model::MadeForYou, String> {
    Ok(service.overview(refresh, utc_offset_minutes).await)
}

/// One Made for You mix with its songs (M46.20).
#[tauri::command]
#[specta::specta]
pub async fn made_for_you_mix(
    service: tauri::State<'_, Arc<crate::made_for_you::MadeForYouService>>,
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    id: String,
) -> Result<raygun_model::Mix, String> {
    let mut mix = service.mix(&id).await?;
    durations.learn_tracks(&mix.tracks);
    details.apply_online(&mut mix.tracks);
    Ok(mix)
}

/// Saves a mix as a playlist named `name`; returns the playlist's id (M46.20).
#[tauri::command]
#[specta::specta]
pub async fn made_for_you_save(
    service: tauri::State<'_, Arc<crate::made_for_you::MadeForYouService>>,
    id: String,
    name: String,
) -> Result<String, String> {
    service.save(&id, &name).await
}

/// Hides an artist from Home, Made for You, charts, genre rows and recommendations, or shows
/// them again; returns the settings as saved.
#[tauri::command]
#[specta::specta]
pub async fn hidden_artist_set(
    app: tauri::AppHandle,
    name: String,
    hidden: bool,
) -> Result<Settings, String> {
    crate::hidden::set_artist(&app, &name, hidden).await
}

/// Marks a song, album, podcast or other Home item Not interested, or takes the mark away.
#[tauri::command]
#[specta::specta]
pub async fn hidden_item_set(
    app: tauri::AppHandle,
    item: raygun_model::HiddenItem,
    hidden: bool,
) -> Result<Settings, String> {
    crate::hidden::set_item(&app, item, hidden).await
}

/// Shows every hidden artist, every hidden item, or both, again.
#[tauri::command]
#[specta::specta]
pub async fn hidden_clear(
    app: tauri::AppHandle,
    artists: bool,
    items: bool,
) -> Result<Settings, String> {
    crate::hidden::clear(&app, artists, items).await
}

/// Every chart source's songs for a genre or a mood (M46.19), without hidden artists' songs.
#[tauri::command]
#[specta::specta]
pub async fn music_tag_sources(
    db: tauri::State<'_, AppDb>,
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    name: String,
    region: Option<String>,
) -> Result<raygun_model::MusicTagSources, String> {
    let mut sources = discovery.tag_sources(name, region).await;
    let details = tauri::Manager::state::<Arc<crate::details::DetailService>>(durations.app());
    let hidden = crate::hidden::Hidden::load(&db.0).await;
    for row in &mut sources.rows {
        durations.learn_tracks(&row.tracks);
        details.apply_online(&mut row.tracks);
        hidden.retain_tracks(&mut row.tracks);
    }
    sources.rows.retain(|row| !row.tracks.is_empty());
    Ok(sources)
}

/// A chart's songs (M46.9), without hidden artists' songs.
#[tauri::command]
#[specta::specta]
pub async fn music_chart(
    db: tauri::State<'_, AppDb>,
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    provider_id: String,
    chart_id: String,
    region: Option<String>,
) -> Result<Vec<raygun_model::OnlineTrack>, String> {
    let mut tracks = discovery.chart(&provider_id, &chart_id, region).await?;
    durations.learn_tracks(&tracks);
    tauri::Manager::state::<Arc<crate::details::DetailService>>(durations.app())
        .apply_online(&mut tracks);
    crate::hidden::Hidden::load(&db.0)
        .await
        .retain_tracks(&mut tracks);
    Ok(tracks)
}

/// A song's genres and moods, found now if they aren't stored yet (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_track_tags(
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    query: TrackQuery,
) -> Result<Vec<raygun_model::MusicTag>, String> {
    discovery.track_tags(query).await
}

/// Stored tags for songs by track key; songs without any are left out (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_tags_for(
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    keys: Vec<String>,
) -> Result<Vec<raygun_model::TrackTags>, String> {
    discovery.tags_for(keys).await
}

/// The tags across the user's songs, the most common first (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_tag_summary(
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
) -> Result<Vec<raygun_model::TagCount>, String> {
    discovery.summary().await
}

/// Favorites and playlist songs with a tag (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_tag_songs(
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    name: String,
) -> Result<Vec<SavedTrack>, String> {
    discovery.songs_with_tag(&name).await
}

/// Every genre and mood Raygun knows, moods first (M46.9).
#[tauri::command]
#[specta::specta]
pub fn music_tag_vocabulary() -> Vec<raygun_model::MusicTag> {
    crate::genres::vocabulary()
}

/// An artist's page: popular songs from the sources, and the library's albums and songs by them.
#[tauri::command]
#[specta::specta]
pub async fn music_artist(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    library: tauri::State<'_, LibraryService>,
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    name: String,
) -> Result<raygun_model::ArtistOverview, String> {
    // A renamed artist is found by the name sources know (M46.30).
    let name = details.original_artist(name.trim());
    if name.is_empty() {
        return Err("No artist named".into());
    }
    let (providers, albums, local) = futures_util::future::join3(
        extensions.search(&name),
        library.albums(Some(&name)),
        library.search(&name, 500),
    )
    .await;
    let tracks = local
        .unwrap_or_default()
        .into_iter()
        .filter(|track| {
            crate::search::credits_include(&track.artist, &name)
                || crate::search::credits_include(&track.album_artist, &name)
        })
        .collect();
    let mut popular = crate::search::artist_tracks(&providers, &name, ARTIST_POPULAR_LIMIT);
    // Lengths the sources sent are kept for lists that lack them (M46.26).
    durations.learn_tracks(&popular);
    details.apply_online(&mut popular);
    Ok(raygun_model::ArtistOverview {
        popular,
        albums: albums.unwrap_or_default(),
        tracks,
        name,
    })
}

/// What open databases say about an artist (M46.14), or `null`.
#[tauri::command]
#[specta::specta]
pub async fn music_artist_info(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    images: tauri::State<'_, raygun_net::ImageLinks>,
    details: tauri::State<'_, Arc<crate::details::DetailService>>,
    name: String,
) -> Result<Option<raygun_model::ArtistInfo>, String> {
    let name = details.original_artist(&name);
    Ok(extensions
        .artist_info(&name)
        .await
        .map(|info| raygun_model::ArtistInfo {
            image_url: info
                .image_url
                .as_deref()
                .map(|url| images.cover(url, &Default::default())),
            ..info
        }))
}

/// The test repository and Stremio addon older builds could add from Settings → Data →
/// Developer (M46.17). The test pack is gone; installs that added it can still take it off.
const TEST_REPO: &str = "https://raw.githubusercontent.com/uvxszi/raygun/main/repo-test/index.json";
const TEST_ADDON: &str = "https://raw.githubusercontent.com/uvxszi/raygun/main/repo-test/stremio";

/// Takes the old test sources off (M46.47): the test repository, its extensions and its Stremio
/// addon.
#[tauri::command]
#[specta::specta]
pub async fn dev_remove_test_sources(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    stremio: Stremio<'_>,
) -> Result<u32, String> {
    let mut removed = 0;
    for info in extensions.list().await {
        if info.id.starts_with("dev.raygun.test.") {
            extensions.uninstall(&info.id).await?;
            removed += 1;
        }
    }
    for repo in extensions.repos().await? {
        if repo.url.eq_ignore_ascii_case(TEST_REPO) {
            extensions.remove_repo(&repo.id).await?;
            removed += 1;
        }
    }
    for addon in stremio.addons().await.unwrap_or_default() {
        if addon
            .url
            .trim_end_matches("/manifest.json")
            .eq_ignore_ascii_case(TEST_ADDON)
        {
            stremio.remove(&addon.id).await?;
            removed += 1;
        }
    }
    Ok(removed)
}

/// Where this device's player plays (M46.15).
#[tauri::command]
#[specta::specta]
pub fn connect_status(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
) -> raygun_model::ConnectStatus {
    connect.status()
}

/// Other Raygun devices to play on: heard on the network, added by address, or allowed before;
/// linked ones say when they last synced (M46.47).
#[tauri::command]
#[specta::specta]
pub async fn connect_devices(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    sync: tauri::State<'_, crate::device_sync::SyncService>,
) -> Result<Vec<raygun_model::ConnectDevice>, String> {
    let synced = sync.last_synced().await;
    Ok(connect
        .inner()
        .devices()
        .into_iter()
        .map(|mut device| {
            device.last_sync_at = device
                .sync_id
                .as_ref()
                .and_then(|id| synced.get(id))
                .map(|at| *at as f64);
            device
        })
        .collect())
}

/// Links with a device (M46.47): it's asked once; then the two sync and either plays on the other.
#[tauri::command]
#[specta::specta]
pub async fn connect_link(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    id: String,
) -> Result<Vec<raygun_model::ConnectDevice>, String> {
    connect.inner().link(&id).await
}

/// Lets other devices find, link with and play on this one, and sync with it (M46.47): the
/// remote server and the sync server together.
#[tauri::command]
#[specta::specta]
pub async fn devices_set_sharing(app: tauri::AppHandle, on: bool) -> Result<(), String> {
    crate::device_link::set_sharing(&app, on).await
}

/// Links with the device whose link QR code was scanned (or its text pasted): the usual
/// question is asked there, with the code's one-time code.
#[tauri::command]
#[specta::specta]
pub async fn connect_link_code(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    text: String,
) -> Result<Vec<raygun_model::ConnectDevice>, String> {
    connect.inner().link_code(&text).await
}

/// Plays on another device: asks it the first time, then moves the queue there.
#[tauri::command]
#[specta::specta]
pub async fn connect_to(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    id: String,
) -> Result<raygun_model::ConnectStatus, String> {
    connect.inner().connect(&id).await
}

/// Stops playing on another device; with `play_here` its queue comes back to this one.
#[tauri::command]
#[specta::specta]
pub async fn connect_disconnect(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    play_here: bool,
) -> Result<raygun_model::ConnectStatus, String> {
    Ok(connect.disconnect(play_here).await)
}

/// Forgets a device this one was allowed on or added.
#[tauri::command]
#[specta::specta]
pub async fn connect_forget(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    id: String,
) -> Result<Vec<raygun_model::ConnectDevice>, String> {
    Ok(connect.inner().forget(&id).await)
}

/// This device's user answers another device that asked to play here.
#[tauri::command]
#[specta::specta]
pub fn connect_answer(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    id: String,
    allow: bool,
) {
    connect.answer(&id, allow);
}

/// Opens where an artist's or a song's facts came from (M46.14) in Raygun's browser.
#[tauri::command]
#[specta::specta]
pub async fn music_open_info_page(websites: Websites<'_>, url: String) -> Result<(), String> {
    websites
        .inner()
        .clone()
        .open_page(&format!("{}music-info", crate::websites::PAGE_PREFIX), &url)
        .await
}

/// Who made a song (M46.14), or `null`.
#[tauri::command]
#[specta::specta]
pub async fn music_song_credits(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: TrackQuery,
) -> Result<Option<raygun_model::SongCredits>, String> {
    Ok(extensions.song_credits(&query).await)
}

/// Where a song is saved: Favorites and the playlists holding it (M46.8).
#[tauri::command]
#[specta::specta]
pub async fn collections_saves(
    collections: tauri::State<'_, CollectionsService>,
    track: TrackInput,
) -> Result<raygun_model::TrackSaves, String> {
    collections.saves(track).await
}

/// Every saved song with the keys it answers to and where it's saved (M46.48): what lists'
/// saved marks and Save buttons read, matching a shown song with `sameSong`.
#[tauri::command]
#[specta::specta]
pub async fn collections_saved_index(
    collections: tauri::State<'_, CollectionsService>,
) -> Result<Vec<raygun_model::SavedSong>, String> {
    collections.saved_index().await
}

/// Saves a song exactly where `saves` says (the Add to playlist popover's Done, M46.8).
#[tauri::command]
#[specta::specta]
pub async fn collections_set_saves(
    collections: tauri::State<'_, CollectionsService>,
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    track: TrackInput,
    saves: raygun_model::TrackSaves,
) -> Result<(), String> {
    collections.set_saves(track, saves).await?;
    discovery.nudge();
    Ok(())
}

/// Favorite tracks in the user's order (newest first until some are moved, M46.44).
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
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    track: TrackInput,
    favorite: bool,
) -> Result<(), String> {
    collections.set_favorite(track, favorite).await?;
    discovery.nudge();
    Ok(())
}

/// Favorites several songs, or takes them all out of Favorites, in one call (M46.49).
#[tauri::command]
#[specta::specta]
pub async fn favorites_set_many(
    collections: tauri::State<'_, CollectionsService>,
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    tracks: Vec<TrackInput>,
    favorite: bool,
) -> Result<(), String> {
    collections.set_favorites(tracks, favorite).await?;
    discovery.nudge();
    Ok(())
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

/// Appends tracks to the end of a playlist; songs it already holds stay out unless `duplicates`
/// (Add anyway, M46.48).
#[tauri::command]
#[specta::specta]
pub async fn playlist_add(
    collections: tauri::State<'_, CollectionsService>,
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    id: String,
    tracks: Vec<TrackInput>,
    duplicates: bool,
) -> Result<raygun_model::PlaylistAdded, String> {
    let added = collections.add_to_playlist(&id, tracks, duplicates).await?;
    if added.added > 0 {
        discovery.nudge();
    }
    Ok(added)
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

/// Merges playlists into a new one, in the order given (M46.45).
#[tauri::command]
#[specta::specta]
pub async fn playlists_merge(
    collections: tauri::State<'_, CollectionsService>,
    ids: Vec<String>,
    name: String,
    skip_duplicates: bool,
    remove_merged: bool,
) -> Result<PlaylistSummary, String> {
    collections
        .merge_playlists(&ids, &name, skip_duplicates, remove_merged)
        .await
}

/// Saves a playlist in the order given (M46.44: a sorted table dragged into shape).
#[tauri::command]
#[specta::specta]
pub async fn playlist_set_order(
    collections: tauri::State<'_, CollectionsService>,
    playlist_id: String,
    item_ids: Vec<String>,
) -> Result<(), String> {
    collections
        .set_playlist_order(&playlist_id, &item_ids)
        .await
}

/// Moves a favorite (by track key) before another, or last when `before_key` is null (M46.44).
#[tauri::command]
#[specta::specta]
pub async fn favorite_move(
    collections: tauri::State<'_, CollectionsService>,
    track_key: String,
    before_key: Option<String>,
) -> Result<(), String> {
    collections
        .move_favorite(&track_key, before_key.as_deref())
        .await
}

/// Saves Favorites in the order given (M46.44).
#[tauri::command]
#[specta::specta]
pub async fn favorites_set_order(
    collections: tauri::State<'_, CollectionsService>,
    track_keys: Vec<String>,
) -> Result<(), String> {
    collections.set_favorites_order(&track_keys).await
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

/// Whether incognito is on (M47). Afterwards, changes arrive as `IncognitoEvent`.
#[tauri::command]
#[specta::specta]
pub fn incognito_status(
    privacy: tauri::State<'_, crate::privacy::Privacy>,
) -> raygun_model::IncognitoStatus {
    privacy.status()
}

/// Turns incognito on or off (M47): while it's on, nothing records what's played, watched, read
/// or searched.
#[tauri::command]
#[specta::specta]
pub async fn incognito_set_enabled(
    app: tauri::AppHandle,
    enabled: bool,
) -> Result<raygun_model::IncognitoStatus, String> {
    crate::privacy::set_incognito(&app, enabled).await
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

/// The linking server's switch, address and open link code (M46.47).
#[tauri::command]
#[specta::specta]
pub async fn remote_status(
    remote: tauri::State<'_, RemoteService>,
) -> Result<RemoteStatus, String> {
    remote.status().await
}

/// Turns the linking server on or off (`devices_set_sharing` does it with sync).
#[tauri::command]
#[specta::specta]
pub async fn remote_set_enabled(
    remote: tauri::State<'_, RemoteService>,
    enabled: bool,
) -> Result<RemoteStatus, String> {
    remote.set_enabled(enabled).await
}

/// Opens a new one-time code for this device's link QR code, replacing the previous one.
#[tauri::command]
#[specta::specta]
pub async fn remote_new_code(
    remote: tauri::State<'_, RemoteService>,
) -> Result<RemoteStatus, String> {
    remote.new_code().await
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
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    remote: tauri::State<'_, RemoteService>,
    name: String,
) -> Result<SyncStatus, String> {
    let status = sync.set_device_name(&name).await?;
    // One name everywhere: Connect, the network and linked devices see it at once.
    connect.rename(&status.device_name);
    remote.readvertise();
    Ok(status)
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

/// A stored title's chapters (read and download states, no fetching), for the reader's list.
#[tauri::command]
#[specta::specta]
pub async fn manga_chapters(
    manga: tauri::State<'_, MangaService>,
    title_id: String,
) -> Result<Vec<MangaChapterInfo>, String> {
    manga.chapters(&title_id).await
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

/// Remembers how a title is read (webtoon strip or paged, left or right to left); `null` follows
/// the reader's default mode again.
#[tauri::command]
#[specta::specta]
pub async fn manga_set_reader_mode(
    manga: tauri::State<'_, MangaService>,
    title_id: String,
    mode: Option<ReaderMode>,
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
    app: tauri::AppHandle,
    collections: tauri::State<'_, CollectionsService>,
    request: CollectionPlayRequest,
) -> Result<(), String> {
    // Played from quick access's menus too, wherever the page on screen is.
    let page: Option<(&'static str, &'static str, &'static str, String)> = match &request.collection
    {
        raygun_model::Collection::Favorites => {
            Some(("favorites", "/library", "view", "favorites".to_owned()))
        }
        raygun_model::Collection::Downloaded => {
            Some(("downloaded", "/library", "view", "downloaded".to_owned()))
        }
        raygun_model::Collection::Playlist { id } => {
            Some(("playlist", "/library/playlist", "id", id.clone()))
        }
        raygun_model::Collection::History => None,
    };
    if let Some((kind, to, key, value)) = page {
        let search = [(key.to_owned(), value)].into_iter().collect();
        tauri::async_runtime::spawn(async move {
            crate::quick_access::played(&app, kind, to, &search).await;
        });
    }
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
    #[test]
    fn title_pages_resolve_against_the_website() {
        let site = Some("https://example.com/novels/");
        assert_eq!(
            page_address(Some("/book/12"), site).as_deref(),
            Some("https://example.com/book/12")
        );
        assert_eq!(
            page_address(Some("https://other.org/x"), site).as_deref(),
            Some("https://other.org/x")
        );
        assert_eq!(page_address(None, site).as_deref(), site);
        assert_eq!(page_address(Some("/book/12"), None), None);
        assert_eq!(page_address(Some("javascript:alert(1)"), site), None);
    }

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

/// Where backups can be saved: the usual place, the folders picked before, the one used last.
#[tauri::command]
#[specta::specta]
pub async fn backup_destinations(
    backups: tauri::State<'_, BackupService>,
) -> Result<BackupDestinations, String> {
    Ok(backups.destinations().await)
}

/// Shows the system's folder picker; the folder chosen is remembered for backups and used next.
/// `None` when the picker was cancelled.
#[tauri::command]
#[specta::specta]
pub async fn backup_pick_folder(
    backups: tauri::State<'_, BackupService>,
) -> Result<Option<BackupDestinations>, String> {
    backups.pick_folder().await
}

/// Stops saving and looking for backups in a folder; its files stay.
#[tauri::command]
#[specta::specta]
pub async fn backup_forget_folder(
    backups: tauri::State<'_, BackupService>,
    uri: String,
) -> Result<BackupDestinations, String> {
    backups.forget_folder(&uri).await
}

/// Writes a backup of the categories asked for, where asked (M46.51).
#[tauri::command]
#[specta::specta]
pub async fn backup_create(
    backups: tauri::State<'_, BackupService>,
    request: BackupRequest,
) -> Result<BackupEntry, String> {
    backups.create(request).await
}

/// Every backup Raygun finds (its data folder, Download/Raygun, the folders picked), newest
/// first, each with what it holds.
#[tauri::command]
#[specta::specta]
pub async fn backup_scan(
    backups: tauri::State<'_, BackupService>,
) -> Result<Vec<BackupEntry>, String> {
    backups.scan().await
}

/// Merges the categories chosen from a backup the last scan found (newer rows win).
#[tauri::command]
#[specta::specta]
pub async fn backup_restore(
    backups: tauri::State<'_, BackupService>,
    id: String,
    categories: Vec<DataCategory>,
) -> Result<BackupOutcome, String> {
    backups.restore(&id, categories).await
}

#[tauri::command]
#[specta::specta]
pub async fn backup_delete(
    backups: tauri::State<'_, BackupService>,
    id: String,
) -> Result<(), String> {
    backups.delete(&id).await
}

/// What each kind of data holds on this device, with sizes (Settings → Data).
#[tauri::command]
#[specta::specta]
pub async fn data_usage(app: tauri::AppHandle) -> Result<Vec<DataUsage>, String> {
    crate::backup::categories::usage(&app).await
}

/// Clears one kind of data (`downloads` says what goes of the downloads); returns what's left.
#[tauri::command]
#[specta::specta]
pub async fn data_clear(
    app: tauri::AppHandle,
    category: DataCategory,
    downloads: Option<DownloadsClear>,
) -> Result<Vec<DataUsage>, String> {
    crate::backup::categories::clear(&app, category, downloads).await?;
    crate::backup::categories::usage(&app).await
}

/// Erases everything but the backups and starts Raygun again as after a new install.
/// `confirm` must be `ERASE`.
#[tauri::command]
#[specta::specta]
pub async fn data_erase_everything(
    app: tauri::AppHandle,
    paths: tauri::State<'_, crate::startup::AppPaths>,
    confirm: String,
) -> Result<(), String> {
    if confirm.trim() != "ERASE" {
        return Err("Type ERASE to erase everything".into());
    }
    let root = paths.0.root().to_owned();
    tauri::async_runtime::spawn_blocking(move || crate::backup::erase::request(&root))
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("Raygun couldn't prepare the erase: {error}"))?;
    tracing::warn!("everything is erased at the next start");
    restart_erased(&app)
}

#[cfg(target_os = "android")]
fn restart_erased(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .restart_app()
}

/// The webview's storage goes now (its profile folder goes at the next start too), then Raygun
/// starts again.
#[cfg(not(target_os = "android"))]
fn restart_erased(app: &tauri::AppHandle) -> Result<(), String> {
    use tauri::Manager;
    for window in app.webview_windows().values() {
        if let Err(error) = window.clear_all_browsing_data() {
            tracing::warn!(%error, "couldn't clear the webview's storage");
        }
    }
    app.request_restart();
    Ok(())
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

/// Imports an uploaded EPUB (sent with `upload_*` in slices) into the novel library; returns its
/// source and id.
#[tauri::command]
#[specta::specta]
pub async fn novel_import_epub_upload(
    novels: tauri::State<'_, NovelService>,
    uploads: tauri::State<'_, crate::uploads::Uploads>,
    upload_id: String,
) -> Result<ImportedBook, String> {
    let file = uploads.take(&upload_id)?;
    let (source_id, novel_id) = novels.import_epub_file(file).await?;
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

/// A report from the webview's speech engine (desktop, M46.6), in the Android engine's JSON.
#[tauri::command]
#[specta::specta]
pub fn tts_report(app: tauri::AppHandle, report: String) {
    crate::tts::report(&app, &report);
}

/// Whether the webview can speak (desktop, M46.6); its speech engine says so when it starts.
#[tauri::command]
#[specta::specta]
pub fn tts_set_available(app: tauri::AppHandle, available: bool) {
    crate::tts::set_available(&app, available);
}

/// Stops reading aloud.
#[tauri::command]
#[specta::specta]
pub async fn tts_stop(app: tauri::AppHandle) -> Result<(), String> {
    crate::tts::stop(&app).await
}

// ---- AI (M34) ----

/// Configured translators (keys are shown only as a hint).
#[tauri::command]
#[specta::specta]
pub async fn ai_providers(ai: tauri::State<'_, AiService>) -> Result<Vec<AiProvider>, String> {
    ai.providers().await
}

/// Adds (no id) or changes a translator. A `null` key keeps the stored key; `""` removes it.
#[tauri::command]
#[specta::specta]
pub async fn ai_provider_save(
    ai: tauri::State<'_, AiService>,
    input: AiProviderInput,
) -> Result<AiProvider, String> {
    ai.save_provider(input).await
}

/// Removes a translator and its key.
#[tauri::command]
#[specta::specta]
pub async fn ai_provider_delete(ai: tauri::State<'_, AiService>, id: String) -> Result<(), String> {
    ai.delete_provider(&id).await
}

/// Translates a sample sentence with the translator and returns the translation.
#[tauri::command]
#[specta::specta]
pub async fn ai_provider_test(
    ai: tauri::State<'_, AiService>,
    id: String,
) -> Result<String, String> {
    ai.test_provider(&id).await
}

/// Models the model manager offers, and which are installed.
#[tauri::command]
#[specta::specta]
pub fn ai_models(ai: tauri::State<'_, AiService>) -> Vec<AiModel> {
    ai.models()
}

/// Downloads and installs a model; progress arrives as `AiModelEvent`s.
#[tauri::command]
#[specta::specta]
pub async fn ai_model_download(ai: tauri::State<'_, AiService>, id: String) -> Result<(), String> {
    ai.download_model(&id).await
}

#[tauri::command]
#[specta::specta]
pub async fn ai_model_delete(ai: tauri::State<'_, AiService>, id: String) -> Result<(), String> {
    ai.delete_model(&id).await
}

// ---- Podcasts (M37) ----

type Podcasts<'a> = tauri::State<'a, Arc<crate::podcasts::PodcastService>>;

/// Subscribed podcasts, most recently updated first.
#[tauri::command]
#[specta::specta]
pub async fn podcast_subscriptions(
    podcasts: Podcasts<'_>,
) -> Result<Vec<raygun_model::PodcastSummary>, String> {
    podcasts.subscriptions().await
}

/// The newest episodes across subscriptions.
#[tauri::command]
#[specta::specta]
pub async fn podcast_latest(
    podcasts: Podcasts<'_>,
    limit: u32,
) -> Result<Vec<raygun_model::PodcastEpisode>, String> {
    podcasts.latest(limit).await
}

/// Opens a podcast by its feed address (from the directory or typed), without subscribing.
#[tauri::command]
#[specta::specta]
pub async fn podcast_open_feed(
    podcasts: Podcasts<'_>,
    feed_url: String,
) -> Result<raygun_model::PodcastView, String> {
    podcasts.open_feed(&feed_url).await
}

/// A stored podcast and its episodes; `refresh` reads the feed again first.
#[tauri::command]
#[specta::specta]
pub async fn podcast_get(
    podcasts: Podcasts<'_>,
    id: String,
    refresh: bool,
) -> Result<raygun_model::PodcastView, String> {
    podcasts.podcast(&id, refresh).await
}

#[tauri::command]
#[specta::specta]
pub async fn podcast_set_subscribed(
    podcasts: Podcasts<'_>,
    id: String,
    subscribed: bool,
) -> Result<(), String> {
    podcasts.set_subscribed(&id, subscribed).await
}

/// Reads every subscription's feed again; returns how many episodes were new.
#[tauri::command]
#[specta::specta]
pub async fn podcast_refresh_all(podcasts: Podcasts<'_>) -> Result<u32, String> {
    podcasts.refresh_all().await
}

/// Searches the podcast directories.
#[tauri::command]
#[specta::specta]
pub async fn podcast_search(
    podcasts: Podcasts<'_>,
    query: String,
) -> Result<Vec<raygun_model::PodcastSearchResult>, String> {
    podcasts.search(&query).await
}

/// Popular podcasts from the directories.
#[tauri::command]
#[specta::specta]
pub async fn podcast_top(
    podcasts: Podcasts<'_>,
) -> Result<Vec<raygun_model::PodcastSearchResult>, String> {
    podcasts.top().await
}

/// Plays an episode, resuming where listening stopped.
#[tauri::command]
#[specta::specta]
pub async fn podcast_play(podcasts: Podcasts<'_>, episode_id: String) -> Result<(), String> {
    podcasts.play(&episode_id).await
}

/// Queues an episode after what plays (`next`) or at the end (M46.16).
#[tauri::command]
#[specta::specta]
pub async fn podcast_queue(
    podcasts: Podcasts<'_>,
    episode_id: String,
    next: bool,
) -> Result<(), String> {
    podcasts.queue(&episode_id, next).await
}

#[tauri::command]
#[specta::specta]
pub async fn podcast_set_played(
    podcasts: Podcasts<'_>,
    episode_id: String,
    played: bool,
) -> Result<(), String> {
    podcasts.set_played(&episode_id, played).await
}

/// Marks every episode of a show (or chapter of a book) played or unplayed (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_set_all_played(
    podcasts: Podcasts<'_>,
    id: String,
    played: bool,
) -> Result<(), String> {
    podcasts.set_all_played(&id, played).await
}

/// Marks the episodes before this one played: older ones, or a book's earlier chapters (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_set_earlier_played(
    podcasts: Podcasts<'_>,
    episode_id: String,
) -> Result<u32, String> {
    podcasts.set_earlier_played(&episode_id).await
}

/// Forgets where listening stopped, without marking it played (M46.48: Continue listening).
#[tauri::command]
#[specta::specta]
pub async fn podcast_forget_position(
    podcasts: Podcasts<'_>,
    episode_id: String,
) -> Result<(), String> {
    podcasts.forget_position(&episode_id).await
}

/// How a show's or book's episodes list, kept for it; `None` goes back to the kind's own
/// (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_set_episode_order(
    podcasts: Podcasts<'_>,
    id: String,
    order: Option<raygun_model::EpisodeOrder>,
) -> Result<(), String> {
    podcasts.set_episode_order(&id, order).await
}

/// The speed a show's or book's episodes play at; `None` leaves the player's (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_set_speed(
    podcasts: Podcasts<'_>,
    id: String,
    speed: Option<f64>,
) -> Result<(), String> {
    podcasts.set_speed(&id, speed).await
}

/// Downloads an episode in the background, two at a time; progress arrives as
/// `PodcastsChangedEvent`s.
#[tauri::command]
#[specta::specta]
pub async fn podcast_download(podcasts: Podcasts<'_>, episode_id: String) -> Result<(), String> {
    podcasts.download(&episode_id).await
}

#[tauri::command]
#[specta::specta]
pub async fn podcast_remove_download(
    podcasts: Podcasts<'_>,
    episode_id: String,
) -> Result<(), String> {
    podcasts.remove_download(&episode_id).await
}

/// Deletes every downloaded episode of a show or chapter of a book (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_remove_downloads(podcasts: Podcasts<'_>, id: String) -> Result<(), String> {
    podcasts.remove_downloads(&id).await
}

/// Stops a download that's waiting or under way (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_cancel_download(
    podcasts: Podcasts<'_>,
    episode_id: String,
) -> Result<(), String> {
    podcasts.cancel_download(&episode_id).await
}

/// Opens a show's or book's website, or a link from its notes, in Raygun's browser (M46.48).
#[tauri::command]
#[specta::specta]
pub async fn podcast_open_page(websites: Websites<'_>, url: String) -> Result<(), String> {
    let url = url.trim();
    if !(url.starts_with("https://") || url.starts_with("http://")) {
        return Err("That isn't a web address".into());
    }
    websites
        .inner()
        .clone()
        .open_page(&format!("{}podcasts", crate::websites::PAGE_PREFIX), url)
        .await
}

// ---- Watch (M38) ----

/// Plays a video in the player, resuming where it was left.
#[tauri::command]
#[specta::specta]
pub async fn video_play(
    app: tauri::AppHandle,
    request: raygun_model::VideoRequest,
) -> Result<(), String> {
    crate::video::play(&app, request).await
}

/// Plays a video link (with an optional subtitle file).
#[tauri::command]
#[specta::specta]
pub async fn video_play_link(
    app: tauri::AppHandle,
    url: String,
    title: Option<String>,
    subtitle_url: Option<String>,
) -> Result<(), String> {
    let request = crate::video::link_request(&url, title.as_deref(), subtitle_url.as_deref());
    crate::video::play(&app, request).await
}

/// Videos started and not finished, most recent first.
#[tauri::command]
#[specta::specta]
pub async fn video_continue_watching(
    app: tauri::AppHandle,
) -> Result<Vec<raygun_model::ContinueWatching>, String> {
    crate::video::continue_watching(&app).await
}

/// Plays a "continue watching" entry again.
#[tauri::command]
#[specta::specta]
pub async fn video_resume(app: tauri::AppHandle, key: String) -> Result<(), String> {
    crate::video::resume(&app, &key).await
}

#[tauri::command]
#[specta::specta]
pub async fn video_remove(app: tauri::AppHandle, key: String) -> Result<(), String> {
    crate::video::remove(&app, &key).await
}

// ---- Library categories (M39) ----

/// The catalog for `medium` (`manga`, `novel` or `video`).
fn catalog_for<'a>(
    app: &'a tauri::AppHandle,
    medium: &str,
) -> Result<&'a crate::catalog::Catalog, String> {
    use tauri::Manager;
    match medium {
        "manga" => Ok(app.state::<MangaService>().inner().catalog()),
        "novel" => Ok(app.state::<NovelService>().inner().catalog()),
        "video" => Ok(app
            .state::<crate::video_sources::VideoSourceService>()
            .inner()
            .catalog()),
        _ => Err(format!("unknown library {medium:?}")),
    }
}

#[tauri::command]
#[specta::specta]
pub async fn library_categories(
    app: tauri::AppHandle,
    medium: String,
) -> Result<Vec<raygun_model::LibraryCategory>, String> {
    catalog_for(&app, &medium)?.categories().await
}

#[tauri::command]
#[specta::specta]
pub async fn library_category_create(
    app: tauri::AppHandle,
    medium: String,
    name: String,
) -> Result<(), String> {
    catalog_for(&app, &medium)?.create_category(&name).await
}

#[tauri::command]
#[specta::specta]
pub async fn library_category_rename(
    app: tauri::AppHandle,
    medium: String,
    id: String,
    name: String,
) -> Result<(), String> {
    catalog_for(&app, &medium)?
        .rename_category(&id, &name)
        .await
}

/// Removes a category; its titles stay in the library.
#[tauri::command]
#[specta::specta]
pub async fn library_category_delete(
    app: tauri::AppHandle,
    medium: String,
    id: String,
) -> Result<(), String> {
    catalog_for(&app, &medium)?.delete_category(&id).await
}

/// Puts the categories in the order of `ids`.
#[tauri::command]
#[specta::specta]
pub async fn library_category_reorder(
    app: tauri::AppHandle,
    medium: String,
    ids: Vec<String>,
) -> Result<(), String> {
    catalog_for(&app, &medium)?.reorder_categories(&ids).await
}

/// Makes `category_ids` exactly the title's categories.
#[tauri::command]
#[specta::specta]
pub async fn library_set_title_categories(
    app: tauri::AppHandle,
    medium: String,
    title_id: String,
    category_ids: Vec<String>,
) -> Result<(), String> {
    catalog_for(&app, &medium)?
        .set_title_categories(&title_id, &category_ids)
        .await
}

// ---- Library depth (M46) ----

/// A title's list, favorite and pin, for its page.
#[tauri::command]
#[specta::specta]
pub async fn library_title_marks(
    app: tauri::AppHandle,
    medium: String,
    title_id: String,
) -> Result<raygun_model::TitleMarks, String> {
    crate::library_actions::marks(catalog_for(&app, &medium)?, &title_id).await
}

/// Puts titles on a list (adding them to the library) or takes them off theirs.
#[tauri::command]
#[specta::specta]
pub async fn library_set_list(
    app: tauri::AppHandle,
    medium: String,
    title_ids: Vec<String>,
    list: Option<raygun_model::LibraryList>,
) -> Result<(), String> {
    use crate::library_actions::{Mark, set_mark};
    set_mark(catalog_for(&app, &medium)?, &title_ids, Mark::List(list)).await
}

/// Makes titles favorites (adding them to the library) or not.
#[tauri::command]
#[specta::specta]
pub async fn library_set_favorite(
    app: tauri::AppHandle,
    medium: String,
    title_ids: Vec<String>,
    favorite: bool,
) -> Result<(), String> {
    use crate::library_actions::{Mark, set_mark};
    set_mark(
        catalog_for(&app, &medium)?,
        &title_ids,
        Mark::Favorite(favorite),
    )
    .await
}

/// Pins titles to the top of the library, or unpins them.
#[tauri::command]
#[specta::specta]
pub async fn library_set_pinned(
    app: tauri::AppHandle,
    medium: String,
    title_ids: Vec<String>,
    pinned: bool,
) -> Result<(), String> {
    use crate::library_actions::{Mark, set_mark};
    set_mark(
        catalog_for(&app, &medium)?,
        &title_ids,
        Mark::Pinned(pinned),
    )
    .await
}

/// Marks every released chapter or episode of titles read (or all unread).
#[tauri::command]
#[specta::specta]
pub async fn library_mark_read(
    app: tauri::AppHandle,
    medium: String,
    title_ids: Vec<String>,
    read: bool,
) -> Result<(), String> {
    crate::library_actions::set_read(catalog_for(&app, &medium)?, &title_ids, read).await
}

/// Marks chapters of a title read or unread together (a selection in its chapter list).
#[tauri::command]
#[specta::specta]
pub async fn library_set_chapters_read(
    app: tauri::AppHandle,
    medium: String,
    chapter_ids: Vec<String>,
    read: bool,
) -> Result<(), String> {
    catalog_for(&app, &medium)?
        .set_units_read(&chapter_ids, read)
        .await
}

/// Marks every chapter before one read (in reading order); returns how many were marked.
#[tauri::command]
#[specta::specta]
pub async fn library_mark_previous_read(
    app: tauri::AppHandle,
    medium: String,
    title_id: String,
    chapter_id: String,
) -> Result<u32, String> {
    catalog_for(&app, &medium)?
        .mark_previous_read(&title_id, &chapter_id)
        .await
}

/// Takes titles out of the library and/or deletes their downloads.
#[tauri::command]
#[specta::specta]
pub async fn library_remove(
    app: tauri::AppHandle,
    medium: String,
    title_ids: Vec<String>,
    from_library: bool,
    delete_downloads: bool,
) -> Result<(), String> {
    let catalog = catalog_for(&app, &medium)?;
    crate::library_actions::remove(&app, catalog, &title_ids, from_library, delete_downloads).await
}

/// Adds titles to some categories and takes them out of others.
#[tauri::command]
#[specta::specta]
pub async fn library_change_categories(
    app: tauri::AppHandle,
    medium: String,
    title_ids: Vec<String>,
    add: Vec<String>,
    remove: Vec<String>,
) -> Result<(), String> {
    crate::library_actions::change_categories(
        catalog_for(&app, &medium)?,
        &title_ids,
        &add,
        &remove,
    )
    .await
}

/// Queues the next `amount` unread chapters of titles (all unread ones without an amount);
/// returns how many were queued.
#[tauri::command]
#[specta::specta]
pub async fn library_download_next(
    app: tauri::AppHandle,
    medium: String,
    title_ids: Vec<String>,
    amount: Option<u32>,
) -> Result<u32, String> {
    let catalog = catalog_for(&app, &medium)?;
    if medium == "video" {
        let (queued, failed) =
            crate::library_actions::download_episodes(&app, catalog, &title_ids, amount).await;
        return match failed.into_iter().next() {
            Some(failure) if queued == 0 => Err(failure.message),
            _ => Ok(queued),
        };
    }
    crate::library_actions::download_next(&app, catalog, &title_ids, amount).await
}

/// Adds titles picked from search or a source's page to the library (fetching the ones never
/// opened) and puts them into `category_ids` (groups for comics and books, folders for videos).
#[tauri::command]
#[specta::specta]
pub async fn library_add_titles(
    app: tauri::AppHandle,
    medium: String,
    titles: Vec<raygun_model::RemoteTitleRef>,
    category_ids: Vec<String>,
) -> Result<raygun_model::BulkResult, String> {
    let catalog = catalog_for(&app, &medium)?;
    Ok(crate::library_actions::add_titles(&app, catalog, &titles, &category_ids).await)
}

/// Downloads the next `amount` chapters or episodes (all unread ones without an amount) of
/// titles picked from search or a source's page, through the normal download queues.
#[tauri::command]
#[specta::specta]
pub async fn library_download_titles(
    app: tauri::AppHandle,
    medium: String,
    titles: Vec<raygun_model::RemoteTitleRef>,
    amount: Option<u32>,
) -> Result<raygun_model::BulkResult, String> {
    let catalog = catalog_for(&app, &medium)?;
    Ok(crate::library_actions::download_titles(&app, catalog, &titles, amount).await)
}

/// The chapter or episode a title continues at.
#[tauri::command]
#[specta::specta]
pub async fn library_continue(
    app: tauri::AppHandle,
    medium: String,
    title_id: String,
) -> Result<Option<String>, String> {
    crate::library_actions::continue_unit(catalog_for(&app, &medium)?, &title_id).await
}

/// Titles of `medium` with downloaded chapters or episodes, in the library or not.
#[tauri::command]
#[specta::specta]
pub async fn library_downloaded(
    app: tauri::AppHandle,
    medium: String,
) -> Result<Vec<MangaLibraryEntry>, String> {
    catalog_for(&app, &medium)?.downloaded().await
}

/// Library titles with the same name as `title` from other sources.
#[tauri::command]
#[specta::specta]
pub async fn library_duplicates(
    app: tauri::AppHandle,
    medium: String,
    title_id: String,
    title: String,
) -> Result<Vec<raygun_model::LibraryDuplicate>, String> {
    crate::library_actions::duplicates(catalog_for(&app, &medium)?, &title_id, &title).await
}

/// "Use this source instead": moves a library title onto the same title in another source,
/// keeping progress by chapter or episode number. Returns the new title's id.
#[tauri::command]
#[specta::specta]
pub async fn library_migrate(
    app: tauri::AppHandle,
    medium: String,
    title_id: String,
    source_id: String,
    remote_id: String,
) -> Result<String, String> {
    let catalog = catalog_for(&app, &medium)?;
    crate::library_actions::migrate(&app, catalog, &title_id, &source_id, &remote_id).await
}

/// Gives a category its own display and sort, or makes it follow the library's again.
#[tauri::command]
#[specta::specta]
pub async fn library_set_category_display(
    app: tauri::AppHandle,
    medium: String,
    category_id: String,
    display: Option<raygun_model::CategoryDisplay>,
) -> Result<(), String> {
    crate::library_actions::set_category_display(catalog_for(&app, &medium)?, &category_id, display)
        .await
}

/// Updates one category's titles (comics, books and videos).
#[tauri::command]
#[specta::specta]
pub async fn library_update_category(
    app: tauri::AppHandle,
    manga: tauri::State<'_, MangaService>,
    medium: String,
    category_id: String,
) -> Result<raygun_model::MangaUpdateStatus, String> {
    let only =
        crate::library_actions::category_titles(catalog_for(&app, &medium)?, &category_id).await?;
    Ok(manga.start_update_of(true, Some(only)).await)
}

/// Starts sending a file picked in the app; returns the upload's id.
#[tauri::command]
#[specta::specta]
pub fn upload_begin(uploads: tauri::State<'_, crate::uploads::Uploads>) -> Result<String, String> {
    uploads.begin()
}

/// Adds a base64 slice to an upload; returns its size so far.
#[tauri::command]
#[specta::specta]
pub fn upload_append(
    uploads: tauri::State<'_, crate::uploads::Uploads>,
    id: String,
    chunk: String,
) -> Result<f64, String> {
    uploads.append(&id, &chunk).map(|size| size as f64)
}

/// Drops an upload that won't be finished.
#[tauri::command]
#[specta::specta]
pub fn upload_cancel(uploads: tauri::State<'_, crate::uploads::Uploads>, id: String) {
    uploads.cancel(&id);
}

/// Imports an uploaded CBZ or ZIP as a chapter of a local comic; returns the title's source and id.
#[tauri::command]
#[specta::specta]
pub async fn manga_import_comic(
    manga: tauri::State<'_, MangaService>,
    uploads: tauri::State<'_, crate::uploads::Uploads>,
    upload_id: String,
    file_name: String,
) -> Result<raygun_model::ImportedTitle, String> {
    let file = uploads.take(&upload_id)?;
    let (source_id, remote_id) = manga.import_comic(file, &file_name).await?;
    Ok(raygun_model::ImportedTitle {
        source_id,
        remote_id,
    })
}

// ---- Saving outside the app and exports (M46.1) ----

/// Shows the system folder picker; the chosen folder becomes where downloads are saved. Returns
/// the settings, unchanged when the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn save_pick_folder(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    exports: tauri::State<'_, Arc<crate::export::ExportService>>,
) -> Result<Settings, String> {
    let current = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    let picked = pick_folder(&app).await?;
    let Some(uri) = picked.uri else {
        return Ok(current);
    };
    let export_folder = exports.last_folder().await.map(|folder| folder.uri);
    if let Some(old) = current
        .save
        .folder_uri
        .as_deref()
        .filter(|old| *old != uri && export_folder.as_deref() != Some(*old))
    {
        release_folder(&app, old);
    }
    let prefs = raygun_model::SavePrefs {
        location: raygun_model::SaveLocation::Folder,
        folder_name: picked.name.or_else(|| Some("Folder".into())),
        folder_uri: Some(uri),
        ..current.save
    };
    settings::update(&db.0, SettingChange::Save(prefs))
        .await
        .map_err(|error| error.to_string())
}

/// The folder picker for saving songs as files (M46.18): the chosen folder is remembered as the
/// save folder, but where finished downloads go stays as it was. `None` when the user cancelled.
#[tauri::command]
#[specta::specta]
pub async fn songs_pick_folder(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    exports: tauri::State<'_, Arc<crate::export::ExportService>>,
) -> Result<Option<Settings>, String> {
    let current = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    let picked = pick_folder(&app).await?;
    let Some(uri) = picked.uri else {
        return Ok(None);
    };
    let export_folder = exports.last_folder().await.map(|folder| folder.uri);
    if let Some(old) = current
        .save
        .folder_uri
        .as_deref()
        .filter(|old| *old != uri && export_folder.as_deref() != Some(*old))
    {
        release_folder(&app, old);
    }
    let prefs = raygun_model::SavePrefs {
        folder_name: picked.name.or_else(|| Some("Folder".into())),
        folder_uri: Some(uri),
        ..current.save
    };
    settings::update(&db.0, SettingChange::Save(prefs))
        .await
        .map(Some)
        .map_err(|error| error.to_string())
}

#[cfg(target_os = "android")]
async fn pick_folder(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .pick_folder()
    })
    .await
    .map_err(|error| error.to_string())?
}

/// A folder for backups (M46.51): the system picker on Android, a folder dialog on desktop.
#[cfg(target_os = "android")]
pub(crate) async fn pick_backup_folder(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    pick_folder(app).await
}

#[cfg(not(target_os = "android"))]
pub(crate) async fn pick_backup_folder(
    _app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Where Raygun saves backups")
        .pick_folder()
        .await;
    Ok(match picked {
        Some(folder) => {
            let path = folder.path().to_path_buf();
            tauri_plugin_raygun_audio::PickedFolder {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .or_else(|| Some(path.to_string_lossy().into_owned())),
                uri: Some(path.to_string_lossy().into_owned()),
            }
        }
        None => tauri_plugin_raygun_audio::PickedFolder {
            uri: None,
            name: None,
        },
    })
}

/// A folder for Local Files (M46.27): the system picker on Android, a folder dialog on desktop.
#[cfg(target_os = "android")]
pub(crate) async fn pick_local_folder(
    app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    pick_folder(app).await
}

#[cfg(not(target_os = "android"))]
pub(crate) async fn pick_local_folder(
    _app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Add a folder to Local Files")
        .pick_folder()
        .await;
    Ok(match picked {
        Some(folder) => {
            let path = folder.path().to_path_buf();
            tauri_plugin_raygun_audio::PickedFolder {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .or_else(|| Some(path.to_string_lossy().into_owned())),
                uri: Some(path.to_string_lossy().into_owned()),
            }
        }
        None => tauri_plugin_raygun_audio::PickedFolder {
            uri: None,
            name: None,
        },
    })
}

/// Asks to read the device's songs and videos (Android); desktop needs nothing.
#[cfg(target_os = "android")]
pub(crate) async fn request_media_access(app: &tauri::AppHandle) -> bool {
    let app = app.clone();
    tauri::async_runtime::spawn_blocking(move || {
        use tauri::Manager;
        app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .request_media_access()
            .unwrap_or(false)
    })
    .await
    .unwrap_or(false)
}

#[cfg(not(target_os = "android"))]
pub(crate) async fn request_media_access(_app: &tauri::AppHandle) -> bool {
    true
}

/// The system's folder dialog on desktop (M46.6); a picked folder is a plain path there.
#[cfg(not(target_os = "android"))]
async fn pick_folder(
    _app: &tauri::AppHandle,
) -> Result<tauri_plugin_raygun_audio::PickedFolder, String> {
    let picked = rfd::AsyncFileDialog::new()
        .set_title("Where Raygun saves downloads")
        .pick_folder()
        .await;
    Ok(match picked {
        Some(folder) => {
            let path = folder.path().to_path_buf();
            tauri_plugin_raygun_audio::PickedFolder {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .or_else(|| Some(path.to_string_lossy().into_owned())),
                uri: Some(path.to_string_lossy().into_owned()),
            }
        }
        None => tauri_plugin_raygun_audio::PickedFolder {
            uri: None,
            name: None,
        },
    })
}

/// Gives up the grant of a folder no longer used.
fn release_folder(_app: &tauri::AppHandle, _uri: &str) {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let _ = _app
            .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .release_folder(_uri);
    }
}

/// Whether Raygun may still write to the picked folder.
#[tauri::command]
#[specta::specta]
pub async fn save_folder_state(
    app: tauri::AppHandle,
    uri: String,
) -> Result<raygun_model::SaveFolderState, String> {
    #[cfg(target_os = "android")]
    {
        let state = tauri::async_runtime::spawn_blocking(move || {
            use tauri::Manager;
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .folder_state(&uri)
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(raygun_model::SaveFolderState {
            granted: state.granted,
            name: state.name,
        })
    }
    // A plain folder: writable while it's there.
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        let path = std::path::PathBuf::from(&uri);
        Ok(raygun_model::SaveFolderState {
            granted: path.is_dir(),
            name: path
                .file_name()
                .map(|name| name.to_string_lossy().into_owned()),
        })
    }
}

/// What an export would do: chapters, those to download first, files, or why it can't run.
#[tauri::command]
#[specta::specta]
pub async fn export_plan(
    exports: tauri::State<'_, Arc<crate::export::ExportService>>,
    request: raygun_model::ExportRequest,
) -> Result<raygun_model::ExportPlan, String> {
    exports.plan(&request).await
}

/// Starts an export in the background; progress comes as `ExportEvent`s.
#[tauri::command]
#[specta::specta]
pub async fn export_start(
    exports: tauri::State<'_, Arc<crate::export::ExportService>>,
    request: raygun_model::ExportRequest,
) -> Result<raygun_model::ExportJob, String> {
    exports.inner().clone().start(request).await
}

/// Shows the system folder picker for one export: the folder chosen is remembered for the export
/// sheet, and Settings → Downloads stays as it was. `None` when the picker was cancelled.
#[tauri::command]
#[specta::specta]
pub async fn export_pick_folder(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    exports: tauri::State<'_, Arc<crate::export::ExportService>>,
) -> Result<Option<raygun_model::ExportFolder>, String> {
    let picked = pick_folder(&app).await?;
    let Some(uri) = picked.uri else {
        return Ok(None);
    };
    let folder = raygun_model::ExportFolder {
        uri,
        name: picked.name.unwrap_or_else(|| "Folder".into()),
    };
    if let Some(old) = exports.remember_folder(&folder).await? {
        let settings_folder = settings::load(&db.0)
            .await
            .ok()
            .and_then(|settings| settings.save.folder_uri);
        if settings_folder.as_deref() != Some(old.as_str()) {
            release_folder(&app, &old);
        }
    }
    Ok(Some(folder))
}

/// The folder last chosen in the export sheet, if any.
#[tauri::command]
#[specta::specta]
pub async fn export_last_folder(
    exports: tauri::State<'_, Arc<crate::export::ExportService>>,
) -> Result<Option<raygun_model::ExportFolder>, String> {
    Ok(exports.last_folder().await)
}

#[tauri::command]
#[specta::specta]
pub fn export_cancel(exports: tauri::State<'_, Arc<crate::export::ExportService>>, id: String) {
    exports.cancel(&id);
}

/// Takes a finished export off the list.
#[tauri::command]
#[specta::specta]
pub fn export_dismiss(exports: tauri::State<'_, Arc<crate::export::ExportService>>, id: String) {
    exports.dismiss(&id);
}

/// Exports of this session, newest first.
#[tauri::command]
#[specta::specta]
pub fn export_jobs(
    exports: tauri::State<'_, Arc<crate::export::ExportService>>,
) -> Vec<raygun_model::ExportJob> {
    exports.jobs()
}

/// What Raygun stores: each cache with its size and limit, music artwork and downloads (M46.2).
#[tauri::command]
#[specta::specta]
pub async fn storage_usage(
    caches: tauri::State<'_, crate::caches::Caches>,
    db: tauri::State<'_, AppDb>,
) -> Result<Vec<raygun_model::StorageUsage>, String> {
    let prefs = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?
        .cache;
    Ok(caches.usage(&prefs).await)
}

/// Empties one cache; returns the new sizes.
#[tauri::command]
#[specta::specta]
pub async fn storage_clear(
    caches: tauri::State<'_, crate::caches::Caches>,
    db: tauri::State<'_, AppDb>,
    area: raygun_model::StorageArea,
) -> Result<Vec<raygun_model::StorageUsage>, String> {
    caches.clear(area).await?;
    let prefs = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?
        .cache;
    Ok(caches.usage(&prefs).await)
}

// ---- Websites and the built-in browser (M46.3) ----

type Websites<'a> = tauri::State<'a, Arc<crate::websites::WebsiteService>>;

#[tauri::command]
#[specta::specta]
pub async fn website_list(websites: Websites<'_>) -> Result<Vec<raygun_model::Website>, String> {
    websites.list().await
}

/// Adds a site by its address (`example.com` is enough); the name defaults to its host.
#[tauri::command]
#[specta::specta]
pub async fn website_add(
    websites: Websites<'_>,
    name: Option<String>,
    url: String,
) -> Result<raygun_model::Website, String> {
    websites.inner().add(name, &url).await
}

#[tauri::command]
#[specta::specta]
pub async fn website_edit(
    websites: Websites<'_>,
    id: String,
    name: String,
    url: String,
) -> Result<(), String> {
    websites.edit(&id, &name, &url).await
}

#[tauri::command]
#[specta::specta]
pub async fn website_remove(websites: Websites<'_>, id: String) -> Result<(), String> {
    websites.remove(&id).await
}

/// Forgets where a site was left.
#[tauri::command]
#[specta::specta]
pub async fn website_reset(websites: Websites<'_>, id: String) -> Result<(), String> {
    websites.reset(&id).await
}

/// Turns a site's own ad blocker on or off.
#[tauri::command]
#[specta::specta]
pub async fn website_set_block_ads(
    websites: Websites<'_>,
    id: String,
    on: bool,
) -> Result<(), String> {
    websites.set_block_ads(&id, on).await
}

/// Opens a site in the built-in browser.
#[tauri::command]
#[specta::specta]
pub async fn website_open(websites: Websites<'_>, id: String) -> Result<(), String> {
    websites.inner().clone().open(&id).await
}

#[tauri::command]
#[specta::specta]
pub async fn website_blocklist_status(
    websites: Websites<'_>,
) -> Result<raygun_model::BlocklistStatus, String> {
    Ok(websites.status().await)
}

/// Downloads the chosen blocklist now.
#[tauri::command]
#[specta::specta]
pub async fn website_blocklist_update(
    websites: Websites<'_>,
) -> Result<raygun_model::BlocklistStatus, String> {
    websites.update_blocklist().await
}

/// Clears the built-in browser's cookies, storage and cache.
#[tauri::command]
#[specta::specta]
pub fn website_clear_data(websites: Websites<'_>) -> Result<(), String> {
    websites.clear_data()
}

/// Saves a reader page to Pictures/Raygun; returns where it went.
#[tauri::command]
#[specta::specta]
pub async fn page_save(
    app: tauri::AppHandle,
    page: raygun_model::PageRef,
) -> Result<String, String> {
    crate::export::pages::act(&app, &page, crate::export::pages::PageAction::Save).await
}

/// Opens the share sheet for a reader page.
#[tauri::command]
#[specta::specta]
pub async fn page_share(app: tauri::AppHandle, page: raygun_model::PageRef) -> Result<(), String> {
    crate::export::pages::act(&app, &page, crate::export::pages::PageAction::Share)
        .await
        .map(|_| ())
}

/// Puts a reader page on the clipboard.
#[tauri::command]
#[specta::specta]
pub async fn page_copy(app: tauri::AppHandle, page: raygun_model::PageRef) -> Result<(), String> {
    crate::export::pages::act(&app, &page, crate::export::pages::PageAction::Copy)
        .await
        .map(|_| ())
}

/// Everything watched or started, most recent first (the Videos library).
#[tauri::command]
#[specta::specta]
pub async fn video_history(
    app: tauri::AppHandle,
) -> Result<Vec<raygun_model::ContinueWatching>, String> {
    crate::video::history(&app).await
}

// ---- Search with filters and across sources (M40) ----

/// The filters a manga or novel source offers for its search.
#[tauri::command]
#[specta::specta]
pub async fn catalog_filters(
    app: tauri::AppHandle,
    medium: String,
    source_id: String,
) -> Result<Vec<raygun_model::SourceFilter>, String> {
    catalog_for(&app, &medium)?.filters(&source_id).await
}

/// Searches one source with its filters; the query may be empty when filters are set, and with
/// neither it lists the source's popular titles. A first page its search fails at or finds
/// nothing for comes from its listings (`Settings::search_listings`).
#[tauri::command]
#[specta::specta]
pub async fn catalog_search(
    app: tauri::AppHandle,
    medium: String,
    source_id: String,
    query: String,
    page: u32,
    filters: std::collections::BTreeMap<String, raygun_model::FilterValue>,
) -> Result<MangaListingPage, String> {
    let listings = crate::search_filter::listings(&app).await;
    catalog_for(&app, &medium)?
        .search(&source_id, &query, page, &filters, listings)
        .await
}

/// The first page of every manga, novel or video source's results for `query` (their popular
/// titles when it's blank), or only `sources`' when any are given (Search's source chips). A
/// source whose search fails or finds nothing answers from its listings
/// (`Settings::search_listings`).
#[tauri::command]
#[specta::specta]
pub async fn catalog_global_search(
    app: tauri::AppHandle,
    medium: String,
    query: String,
    sources: Vec<String>,
) -> Result<Vec<raygun_model::SourceSearchResult>, String> {
    let filter = crate::search_filter::current(&app).await;
    let listings = crate::search_filter::listings(&app).await;
    catalog_for(&app, &medium)?
        .global_search(&query, &sources, filter, listings)
        .await
}

/// Tracks in the device's library matching `query` (title, artist or album).
#[tauri::command]
#[specta::specta]
pub async fn library_search(
    library: tauri::State<'_, LibraryService>,
    query: String,
) -> Result<Vec<LibraryTrack>, String> {
    library.search(&query, 50).await
}

// ---- Stremio addons (M41) ----

type Stremio<'a> = tauri::State<'a, Arc<crate::stremio::StremioService>>;

#[tauri::command]
#[specta::specta]
pub async fn stremio_addons(
    stremio: Stremio<'_>,
) -> Result<Vec<raygun_model::StremioAddon>, String> {
    stremio.addons().await
}

/// Well-known addons to add with one tap, marked when already added.
#[tauri::command]
#[specta::specta]
pub async fn stremio_suggestions(
    stremio: Stremio<'_>,
) -> Result<Vec<raygun_model::StremioSuggestion>, String> {
    stremio.suggestions().await
}

/// What the addon at an address is, before adding it: its name, what it provides, whether it
/// must be set up first (`id` is empty unless it's added).
#[tauri::command]
#[specta::specta]
pub async fn stremio_preview(
    stremio: Stremio<'_>,
    url: String,
) -> Result<raygun_model::StremioAddon, String> {
    stremio.preview(&url).await
}

/// Adds an addon by its address (`.../manifest.json`, `stremio://...` or a bare host). The same
/// addon set up again under a new address takes the old one's place.
#[tauri::command]
#[specta::specta]
pub async fn stremio_add(
    stremio: Stremio<'_>,
    url: String,
) -> Result<raygun_model::StremioAddon, String> {
    stremio.add(&url).await
}

#[tauri::command]
#[specta::specta]
pub async fn stremio_remove(stremio: Stremio<'_>, id: String) -> Result<(), String> {
    stremio.remove(&id).await
}

/// Turns an addon on or off; one that's off is asked for nothing.
#[tauri::command]
#[specta::specta]
pub async fn stremio_set_enabled(
    stremio: Stremio<'_>,
    id: String,
    enabled: bool,
) -> Result<(), String> {
    stremio.set_enabled(&id, enabled).await
}

/// Puts the addons in this order: details come from the first that has them.
#[tauri::command]
#[specta::specta]
pub async fn stremio_set_order(stremio: Stremio<'_>, ids: Vec<String>) -> Result<(), String> {
    stremio.set_order(&ids).await
}

/// Opens the setup page of the addon at `url` in the browser.
#[tauri::command]
#[specta::specta]
pub fn stremio_configure(stremio: Stremio<'_>, url: String) -> Result<(), String> {
    stremio.configure(&url)
}

/// Opens a stream's web page in the browser, or (on desktop) a torrent's magnet link in the
/// torrent app.
#[tauri::command]
#[specta::specta]
pub fn stremio_open_link(stremio: Stremio<'_>, url: String) -> Result<(), String> {
    stremio.open_link(&url)
}

/// Every catalog of the addons that answer: board rows, searches and what Discover browses.
#[tauri::command]
#[specta::specta]
pub async fn stremio_catalogs(
    stremio: Stremio<'_>,
) -> Result<Vec<raygun_model::StremioCatalogInfo>, String> {
    stremio.catalogs().await
}

/// The Watch home: the first page of every browsable catalog.
#[tauri::command]
#[specta::specta]
pub async fn stremio_home(
    stremio: Stremio<'_>,
) -> Result<Vec<raygun_model::StremioCatalogRow>, String> {
    stremio.home().await
}

/// One catalog's page with the extras chosen (a genre, a year...), `skip` items in.
#[tauri::command]
#[specta::specta]
pub async fn stremio_catalog(
    stremio: Stremio<'_>,
    addon_id: String,
    kind: String,
    catalog_id: String,
    extra: Vec<raygun_model::StremioExtraValue>,
    skip: u32,
) -> Result<raygun_model::StremioCatalogRow, String> {
    stremio
        .catalog(&addon_id, &kind, &catalog_id, &extra, skip)
        .await
}

/// Every searchable catalog's results for `query`.
#[tauri::command]
#[specta::specta]
pub async fn stremio_search(
    stremio: Stremio<'_>,
    query: String,
) -> Result<Vec<raygun_model::StremioCatalogRow>, String> {
    stremio.search(&query).await
}

/// One searchable catalog's results for `query`; a search for another query stops it.
#[tauri::command]
#[specta::specta]
pub async fn stremio_search_catalog(
    stremio: Stremio<'_>,
    addon_id: String,
    kind: String,
    catalog_id: String,
    query: String,
) -> Result<raygun_model::StremioCatalogRow, String> {
    stremio
        .search_catalog(&addon_id, &kind, &catalog_id, &query)
        .await
}

/// A title's details, seasons and episodes, with watching progress.
#[tauri::command]
#[specta::specta]
pub async fn stremio_meta(
    stremio: Stremio<'_>,
    kind: String,
    id: String,
) -> Result<raygun_model::StremioMeta, String> {
    stremio.meta(&kind, &id).await
}

/// Streams for a movie or an episode of the title `meta_id`, grouped by addon.
#[tauri::command]
#[specta::specta]
pub async fn stremio_streams(
    stremio: Stremio<'_>,
    kind: String,
    meta_id: Option<String>,
    video_id: String,
) -> Result<Vec<raygun_model::StremioStreamGroup>, String> {
    stremio.streams(&kind, meta_id.as_deref(), &video_id).await
}

/// Plays a stream; `language` puts its subtitles first.
#[tauri::command]
#[specta::specta]
pub async fn stremio_play(
    stremio: Stremio<'_>,
    request: raygun_model::StremioPlayRequest,
    language: String,
) -> Result<(), String> {
    stremio.play(request, &language).await
}

/// Marks a movie or episode watched (or not).
#[tauri::command]
#[specta::specta]
pub async fn stremio_set_watched(
    stremio: Stremio<'_>,
    kind: String,
    video_id: String,
    title: String,
    subtitle: Option<String>,
    poster: Option<String>,
    watched: bool,
) -> Result<(), String> {
    stremio
        .set_watched(&kind, &video_id, &title, subtitle, poster, watched)
        .await
}

// ---- Local network access (Android 17) ----

/// Android 17's local network permission for reaching `address` (an addon's URL), or for devices
/// on the network reaching the LAN remote when there is none.
#[tauri::command]
#[specta::specta]
pub async fn local_network_access(
    local_network: tauri::State<'_, LocalNetwork>,
    address: Option<String>,
) -> Result<raygun_model::LocalNetworkAccess, String> {
    local_network.access(address.as_deref()).await
}

/// Shows Android's dialog for local network access, unless it's granted or not needed.
#[tauri::command]
#[specta::specta]
pub async fn local_network_request(
    local_network: tauri::State<'_, LocalNetwork>,
) -> Result<raygun_model::LocalNetworkAccess, String> {
    local_network.request().await
}

/// Opens Raygun's page in Android's settings, where a permission it stopped asking for can be
/// allowed.
#[tauri::command]
#[specta::specta]
pub async fn local_network_open_settings(
    local_network: tauri::State<'_, LocalNetwork>,
) -> Result<(), String> {
    local_network.open_settings().await
}

// ---- Video sources (M42) ----

type VideoSources<'a> = tauri::State<'a, crate::video_sources::VideoSourceService>;

/// Installed video sources (`video.source` providers).
#[tauri::command]
#[specta::specta]
pub async fn video_sources(sources: VideoSources<'_>) -> Result<Vec<MangaSource>, String> {
    Ok(sources.catalog().sources().await)
}

/// A page of a video source's catalogue: popular, latest, or search results.
#[tauri::command]
#[specta::specta]
pub async fn video_browse(
    sources: VideoSources<'_>,
    source_id: String,
    mode: MangaBrowseMode,
    query: Option<String>,
    page: u32,
) -> Result<MangaListingPage, String> {
    sources
        .catalog()
        .browse(&source_id, mode, query.as_deref(), page)
        .await
}

/// A video title with its episodes and watching progress; `refresh` asks the source again.
#[tauri::command]
#[specta::specta]
pub async fn video_title(
    sources: VideoSources<'_>,
    source_id: String,
    remote_id: String,
    refresh: bool,
) -> Result<raygun_model::VideoTitleView, String> {
    sources.title(&source_id, &remote_id, refresh).await
}

/// A title's comments from its source, a page at a time (M46.45: YouTube's).
#[tauri::command]
#[specta::specta]
pub async fn video_comments(
    sources: VideoSources<'_>,
    source_id: String,
    remote_id: String,
    page: Option<String>,
) -> Result<raygun_model::VideoComments, String> {
    sources
        .comments(&source_id, &remote_id, page.as_deref())
        .await
}

/// An episode's servers (`episode_id` is the source's id); resolved ones come with their links.
#[tauri::command]
#[specta::specta]
pub async fn video_servers(
    sources: VideoSources<'_>,
    source_id: String,
    episode_id: String,
) -> Result<Vec<raygun_model::VideoServerView>, String> {
    sources.servers(&source_id, &episode_id).await
}

/// One server's links, the preferred quality first.
#[tauri::command]
#[specta::specta]
pub async fn video_links(
    sources: VideoSources<'_>,
    source_id: String,
    server_id: String,
) -> Result<Vec<raygun_model::VideoLinkView>, String> {
    sources.links(&source_id, &server_id).await
}

/// Plays a link of a stored episode in the video player.
#[tauri::command]
#[specta::specta]
pub async fn video_source_play(
    sources: VideoSources<'_>,
    request: raygun_model::VideoSourcePlay,
) -> Result<(), String> {
    sources.play(request).await
}

/// Marks a stored episode watched (or not).
#[tauri::command]
#[specta::specta]
pub async fn video_set_episode_watched(
    sources: VideoSources<'_>,
    title_id: String,
    episode_id: String,
    watched: bool,
) -> Result<(), String> {
    sources.set_watched(&title_id, &episode_id, watched).await
}

// ---- Playback sessions (M44) ----

type Sessions<'a> = tauri::State<'a, crate::video_session::VideoSessions>;

/// Watches a video source's stored title: the player opens at once on `episode_id` (the source's
/// id; the first unwatched when none) and plays the best link as it resolves.
#[tauri::command]
#[specta::specta]
pub async fn video_source_watch(
    sessions: Sessions<'_>,
    title_id: String,
    episode_id: Option<String>,
) -> Result<(), String> {
    let origin = crate::video_session::Origin::Source { title_id };
    sessions.start(origin, episode_id).await
}

/// Watches a Stremio movie or series from `video_id` (the first unwatched episode when none).
#[tauri::command]
#[specta::specta]
pub async fn stremio_watch(
    sessions: Sessions<'_>,
    kind: String,
    id: String,
    video_id: Option<String>,
) -> Result<(), String> {
    let origin = crate::video_session::Origin::Stremio { kind, id };
    sessions.start(origin, video_id).await
}

/// A source picker's servers or addons in the order a session tries them for a title (`group`:
/// `title:<id>` or `stremio:<kind>:<id>`; `title` the show's name), with what played last time
/// marked (M46.49).
#[tauri::command]
#[specta::specta]
pub async fn video_source_order(
    sessions: Sessions<'_>,
    group: String,
    title: Option<String>,
    sources: Vec<crate::video_session::SourceCandidate>,
) -> Result<Vec<crate::video_session::SourceRank>, String> {
    Ok(sessions.source_order(&group, title, sources).await)
}

/// A report from the webview's video player (desktop, M46.4), in the Android player's JSON.
#[tauri::command]
#[specta::specta]
pub fn video_player_report(reports: tauri::State<'_, crate::video::VideoReports>, report: String) {
    reports.send(report);
}

/// Plays a video in another app (desktop, M46.4): `player` is one of `video_external_players`,
/// else the first installed (M46.33). Returns the app's name.
#[tauri::command]
#[specta::specta]
pub fn video_player_external(
    play: raygun_model::ExternalPlay,
    player: Option<String>,
) -> Result<String, String> {
    #[cfg(not(target_os = "android"))]
    {
        crate::video_desktop::play_external(&play, player.as_deref())
    }
    #[cfg(target_os = "android")]
    {
        let _ = (play, player);
        Err("Android picks another app from the player".into())
    }
}

/// Looks for subtitles for what plays under `key` (`stremio:<type>:<id>`) in every added addon
/// (M46.33); offers OpenSubtitles when no addon serves subtitles.
#[tauri::command]
#[specta::specta]
pub async fn video_find_subtitles(
    app: tauri::AppHandle,
    stremio: tauri::State<'_, Arc<crate::stremio::StremioService>>,
    key: String,
) -> Result<raygun_model::FoundSubtitles, String> {
    let Some((kind, video_id)) = key
        .strip_prefix("stremio:")
        .and_then(|rest| rest.split_once(':'))
    else {
        return Ok(raygun_model::FoundSubtitles {
            tracks: Vec::new(),
            suggestion: None,
            note: Some("Subtitles can be looked for in addons for movies and series".into()),
        });
    };
    let db = tauri::Manager::state::<crate::startup::AppDb>(&app)
        .0
        .clone();
    let language = crate::video::subtitle_language(&db).await;
    let (tracks, serves) = stremio.find_subtitles(kind, video_id, &language).await;
    let suggestion = if serves {
        None
    } else {
        stremio.subtitles_suggestion().await
    };
    Ok(raygun_model::FoundSubtitles {
        tracks: crate::video::proxied_subtitles(&app, &tracks),
        suggestion,
        note: None,
    })
}

/// The YouTube embed host page (M08), with YouTube's controls or without (M46.34: trailers on
/// title pages play in a small window of Raygun's own).
#[tauri::command]
#[specta::specta]
pub fn youtube_embed_url(
    media: tauri::State<'_, raygun_net::MediaLinks>,
    controls: bool,
) -> String {
    let url = media.youtube_embed_url();
    if controls {
        url
    } else {
        url.replace("?controls=1", "")
    }
}

/// The video players installed on this computer, best first (desktop, M46.33).
#[tauri::command]
#[specta::specta]
pub fn video_external_players() -> Vec<raygun_model::ExternalPlayer> {
    #[cfg(not(target_os = "android"))]
    {
        crate::video_desktop::external_players()
    }
    #[cfg(target_os = "android")]
    {
        Vec::new()
    }
}

// ---- Play on TV ----

type Casting<'a> = tauri::State<'a, Arc<crate::cast::CastService>>;

/// TVs and other devices the player's video can play on. Asking searches the network, again
/// every few seconds while asked.
#[tauri::command]
#[specta::specta]
pub fn cast_devices(cast: Casting<'_>) -> raygun_model::CastDeviceList {
    let devices = cast.inner().devices();
    raygun_model::CastDeviceList {
        devices,
        searching: cast.searching(),
    }
}

/// Adds a TV by its address (`192.168.1.20`, with a port, or its description's address).
#[tauri::command]
#[specta::specta]
pub async fn cast_add_device(
    cast: Casting<'_>,
    address: String,
) -> Result<raygun_model::CastDevice, String> {
    cast.inner().add(&address).await
}

/// Plays the player's video on a device from `position_ms`, with the subtitle file at
/// `subtitle` (an index into the video's subtitles) on.
#[tauri::command]
#[specta::specta]
pub async fn cast_start(
    cast: Casting<'_>,
    device_id: String,
    position_ms: f64,
    subtitle: Option<u32>,
) -> Result<(), String> {
    cast.inner()
        .start(
            &device_id,
            position_ms,
            subtitle.map(|index| index as usize),
        )
        .await
}

/// A button of the device's remote control.
#[tauri::command]
#[specta::specta]
pub async fn cast_control(
    cast: Casting<'_>,
    control: raygun_model::CastControl,
) -> Result<(), String> {
    cast.inner().control(control).await
}

/// Stops playing on the device; with `resume`, the player goes on from where it was.
#[tauri::command]
#[specta::specta]
pub async fn cast_disconnect(cast: Casting<'_>, resume: bool) -> Result<(), String> {
    cast.inner().disconnect(resume).await
}

/// What plays on a device, if anything.
#[tauri::command]
#[specta::specta]
pub fn cast_status(cast: Casting<'_>) -> Option<raygun_model::CastStatus> {
    cast.inner().status()
}

/// The video player's words in the app's language (episodes, sources, next episode...).
#[tauri::command]
#[specta::specta]
pub fn video_set_player_labels(
    sessions: Sessions<'_>,
    labels: std::collections::HashMap<String, String>,
) {
    sessions.set_labels(
        labels
            .into_iter()
            .map(|(key, text)| (key, serde_json::Value::String(text)))
            .collect(),
    );
}

// ---- Video library, updates and downloads (M43) ----

type VideoDownloadsState<'a> = tauri::State<'a, Arc<crate::video_downloads::VideoDownloadService>>;

/// Saves a video source's title to the library (or takes it out).
#[tauri::command]
#[specta::specta]
pub async fn video_set_in_library(
    sources: VideoSources<'_>,
    title_id: String,
    in_library: bool,
) -> Result<(), String> {
    sources.set_in_library(&title_id, in_library).await
}

/// Saved movies and series, from video sources and Stremio addons, with watched counts.
#[tauri::command]
#[specta::specta]
pub async fn video_library(sources: VideoSources<'_>) -> Result<Vec<MangaLibraryEntry>, String> {
    sources.catalog().library().await
}

/// Episodes library updates found, newest first.
#[tauri::command]
#[specta::specta]
pub async fn video_updates(
    db: tauri::State<'_, AppDb>,
    images: tauri::State<'_, raygun_net::ImageLinks>,
) -> Result<Vec<raygun_model::VideoUpdateItem>, String> {
    crate::video_library::updates(&db.0, &images).await
}

/// Episodes of library series that aren't out yet, soonest first.
#[tauri::command]
#[specta::specta]
pub async fn video_upcoming(
    db: tauri::State<'_, AppDb>,
    images: tauri::State<'_, raygun_net::ImageLinks>,
) -> Result<Vec<raygun_model::VideoUpdateItem>, String> {
    crate::video_library::upcoming(&db.0, &images, crate::catalog::now_ms()).await
}

/// Saves a movie or series from Stremio addons to the library, with its episodes (or takes it
/// out).
#[tauri::command]
#[specta::specta]
pub async fn stremio_set_in_library(
    stremio: Stremio<'_>,
    kind: String,
    id: String,
    in_library: bool,
) -> Result<(), String> {
    stremio.set_in_library(&kind, &id, in_library).await
}

/// Every video download, newest first; changes arrive as `VideoDownloadsEvent`s.
#[tauri::command]
#[specta::specta]
pub async fn video_downloads(
    downloads: VideoDownloadsState<'_>,
) -> Result<raygun_model::VideoDownloads, String> {
    downloads.list().await
}

/// Downloads a link of a stored episode, with the subtitle tracks ticked (`subtitles`, their
/// addresses; `None` keeps the source's remembered choice).
#[tauri::command]
#[specta::specta]
pub async fn video_source_download(
    sources: VideoSources<'_>,
    downloads: VideoDownloadsState<'_>,
    db: tauri::State<'_, AppDb>,
    request: raygun_model::VideoSourcePlay,
    subtitles: Option<Vec<String>>,
) -> Result<(), String> {
    let title_id = request.title_id.clone();
    let source = raygun_db::titles::title(&db.0, &title_id)
        .await
        .map_err(crate::catalog::to_message)?
        .map(|row| row.fields.source_id)
        .unwrap_or_default();
    let shown = request.link.subtitles.clone();
    let video = sources.request(request).await?;
    let pick = crate::video_downloads::SubtitlePick {
        source,
        shown,
        chosen: subtitles,
    };
    downloads.enqueue(video, Some(title_id), pick).await
}

/// Downloads a direct stream of a movie or episode, with the subtitle tracks ticked among the
/// stream's (`subtitles`), and the subtitle addons' in the languages picked before.
#[tauri::command]
#[specta::specta]
pub async fn stremio_download(
    stremio: Stremio<'_>,
    downloads: VideoDownloadsState<'_>,
    request: raygun_model::StremioPlayRequest,
    language: String,
    title_id: Option<String>,
    subtitles: Option<Vec<String>>,
) -> Result<(), String> {
    let shown = request.stream.subtitles.clone();
    let video = stremio.request(&request, &language).await?;
    let pick = crate::video_downloads::SubtitlePick {
        source: crate::video_downloads::auto::STREMIO_SUBTITLES.into(),
        shown,
        chosen: subtitles,
    };
    downloads.enqueue(video, title_id, pick).await
}

/// Downloads episodes of a title without picking links: `group` is the title's player group
/// (`title:<id>` or `stremio:<type>:<id>`), `episode_ids` the episodes' ids at the source. Each
/// finds its best link when its turn comes. Returns how many were queued.
#[tauri::command]
#[specta::specta]
pub async fn video_download_episodes(
    downloads: VideoDownloadsState<'_>,
    group: String,
    episode_ids: Vec<String>,
) -> Result<u32, String> {
    downloads.enqueue_episodes(&group, &episode_ids).await
}

/// The subtitle tracks ticked at first when downloading from `source` (a video source's id, or
/// `stremio`): the languages picked there before, and the device's unless it was left out.
#[tauri::command]
#[specta::specta]
pub async fn video_download_subtitle_defaults(
    downloads: VideoDownloadsState<'_>,
    source: String,
    tracks: Vec<raygun_model::VideoSubtitle>,
) -> Result<Vec<String>, String> {
    Ok(downloads.subtitle_defaults(&source, &tracks).await)
}

/// The device's language, for the subtitles downloads keep.
#[tauri::command]
#[specta::specta]
pub async fn video_downloads_set_device_language(
    downloads: VideoDownloadsState<'_>,
    language: String,
) -> Result<(), String> {
    downloads.set_device_language(&language).await;
    Ok(())
}

/// Tries a failed download again, keeping what it saved.
#[tauri::command]
#[specta::specta]
pub async fn video_download_retry(
    downloads: VideoDownloadsState<'_>,
    id: String,
) -> Result<(), String> {
    downloads.retry(&id).await
}

/// Cancels a download or deletes a downloaded video.
#[tauri::command]
#[specta::specta]
pub async fn video_download_remove(
    downloads: VideoDownloadsState<'_>,
    id: String,
) -> Result<(), String> {
    downloads.remove(&id).await
}

/// Plays a download (from the device once it's finished).
#[tauri::command]
#[specta::specta]
pub async fn video_download_play(
    downloads: VideoDownloadsState<'_>,
    id: String,
) -> Result<(), String> {
    downloads.play(&id).await
}

#[tauri::command]
#[specta::specta]
pub async fn video_downloads_set_paused(
    downloads: VideoDownloadsState<'_>,
    paused: bool,
) -> Result<(), String> {
    downloads.set_paused(paused);
    Ok(())
}
