use std::sync::Arc;

use raygun_model::{
    AiModel, AiProvider, AiProviderInput, AppInfo, BackupFile, BackupOutcome,
    CollectionPlayRequest, EngineReport, ExtensionInfo, ExtensionRepo, HistoryEntry, ImportedBook,
    LibraryAlbum, LibraryArtist, LibraryStatus, LibraryTrack, Lyrics, MangaBrowseMode,
    MangaChapterPages, MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView,
    MangaUpdateItem, MangaUpdateStatus, McpStatus, NovelChapter, PlayRequest, PlaybackControl,
    PlaybackState, PlaylistDetail, PlaylistSummary, QueueItemInput, QueueSnapshot, ReaderMode,
    RemoteStatus, RepoCatalog, SavedTrack, SearchResults, SettingChange, SettingValue, Settings,
    StorageInfo, StorageMode, SyncKind, SyncStatus, TrackInput, TrackQuery,
};

use crate::ai::AiService;
use crate::backup::BackupService;
use crate::collections::CollectionsService;
use crate::device_sync::SyncService;
use crate::discovery::DiscoveryService;
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

/// Changes one setting and returns the updated settings.
#[tauri::command]
#[specta::specta]
pub async fn settings_update(
    app: tauri::AppHandle,
    db: tauri::State<'_, AppDb>,
    change: SettingChange,
) -> Result<Settings, String> {
    let theme_changed = matches!(change, SettingChange::Theme(_));
    let nsfw_changed = matches!(change, SettingChange::ShowNsfw(_));
    let cache_changed = matches!(change, SettingChange::Cache(_));
    let sources_changed = matches!(change, SettingChange::DisabledSources(_));
    let discovery_changed = matches!(change, SettingChange::Discovery(_));
    let agent_changed = matches!(change, SettingChange::UserAgent(_));
    let sound_changed = matches!(change, SettingChange::Sound(_));
    let discord_changed = matches!(change, SettingChange::Discord(_));
    let settings = settings::update(&db.0, change)
        .await
        .map_err(|error| error.to_string())?;
    if discovery_changed {
        use tauri::Manager;
        app.state::<Arc<DiscoveryService>>().nudge();
    }
    if theme_changed {
        crate::system_ui::apply_theme(&app, settings.theme);
    }
    if agent_changed {
        raygun_net::set_user_agent(&settings.user_agent);
    }
    if discord_changed {
        use tauri::Manager;
        app.state::<Arc<crate::discord::DiscordPresence>>()
            .set_prefs(settings.discord.clone());
    }
    if sound_changed {
        use tauri::Manager;
        app.state::<crate::playback::PlaybackService>()
            .set_sound(settings.sound.clone());
    }
    if nsfw_changed {
        use tauri::Manager;
        app.state::<Arc<ExtensionService>>()
            .set_show_nsfw(settings.show_nsfw)
            .await;
    }
    if cache_changed {
        use tauri::Manager;
        app.state::<crate::caches::Caches>().apply(&settings.cache);
    }
    if sources_changed {
        use tauri::Manager;
        app.state::<Arc<ExtensionService>>()
            .set_disabled_sources(&settings.disabled_sources);
    }
    Ok(settings)
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
    Ok(())
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
    background: Option<String>,
) -> Result<(), String> {
    crate::miniplayer::open(&app, background.as_deref())
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

/// Lyrics for a track, or `null` when no provider has any. The song's own lyrics (M46.12) win, and
/// its lyrics offset comes along.
#[tauri::command]
#[specta::specta]
pub async fn lyrics_get(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    sound: tauri::State<'_, Arc<crate::sound::SoundService>>,
    query: TrackQuery,
) -> Result<Option<Lyrics>, String> {
    let fetched = if sound.has_own_lyrics(&query) {
        None
    } else {
        extensions.lyrics(&query).await?
    };
    Ok(sound.lyrics(&query, fetched))
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
/// with tracks or an error; songs several providers found are merged into `top`.
#[tauri::command]
#[specta::specta]
pub async fn online_search(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    library: tauri::State<'_, LibraryService>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
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
    // Lengths the sources sent are kept for lists that lack them (M46.26).
    for result in &providers {
        durations.learn_tracks(&result.tracks);
    }
    Ok(SearchResults {
        library,
        top: crate::search::merge_top(&providers),
        providers,
    })
}

/// Songs an artist page shows (M46.8).
const ARTIST_POPULAR_LIMIT: usize = 20;

/// Artists for a search, from every source that searches artists (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_search_artists(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: String,
) -> Result<Vec<raygun_model::MusicArtist>, String> {
    Ok(extensions.search_artists(&query).await)
}

/// Albums for a search, from every source that searches albums (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_search_albums(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    query: String,
) -> Result<Vec<raygun_model::MusicAlbum>, String> {
    Ok(extensions.search_albums(&query).await)
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
    let detail = extensions.music_album(&provider_id, &album_id).await?;
    durations.learn_tracks(&detail.tracks);
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

/// Removes a song download and its file (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn song_download_remove(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
) -> Result<(), String> {
    songs.remove(&id).await
}

/// Queues a failed song download again (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn song_download_retry(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
) -> Result<(), String> {
    songs.retry(&id).await
}

/// Plays a song only YouTube has and records it as it plays (M46.10).
#[tauri::command]
#[specta::specta]
pub async fn song_download_record(
    songs: tauri::State<'_, Arc<SongDownloadService>>,
    id: String,
) -> Result<(), String> {
    songs.record(&id).await
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

/// A page of books from the audiobook directories (M46.22).
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

/// Songs in uploaded screenshots of song lists (M46.21).
#[tauri::command]
#[specta::specta]
pub async fn import_read_screenshots(
    import: tauri::State<'_, crate::import::ImportService>,
    upload_ids: Vec<String>,
) -> Result<raygun_model::ImportedList, String> {
    import.read_screenshots(upload_ids).await
}

/// The folders in Local Files and what each holds (M46.27).
#[tauri::command]
#[specta::specta]
pub async fn local_folders(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
) -> Result<Vec<raygun_model::LocalFolder>, String> {
    local.folders().await
}

/// Adds a folder the user picks to Local Files and scans it; `None` when they cancelled.
#[tauri::command]
#[specta::specta]
pub async fn local_folder_add(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
) -> Result<Option<Vec<raygun_model::LocalFolder>>, String> {
    local.inner().add().await
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

/// Scans every Local Files folder again.
#[tauri::command]
#[specta::specta]
pub async fn local_folders_rescan(
    local: tauri::State<'_, Arc<crate::local_files::LocalFilesService>>,
) -> Result<(), String> {
    local.inner().rescan_all().await
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

/// Matches imported songs against the music sources (M46.21).
#[tauri::command]
#[specta::specta]
pub async fn import_match(
    import: tauri::State<'_, crate::import::ImportService>,
    songs: Vec<raygun_model::ImportedSong>,
) -> Result<Vec<raygun_model::MatchedSong>, String> {
    Ok(import.match_songs(songs).await)
}

/// Saves imported songs as a new playlist; returns its id (M46.21).
#[tauri::command]
#[specta::specta]
pub async fn import_save(
    import: tauri::State<'_, crate::import::ImportService>,
    name: String,
    songs: Vec<raygun_model::MatchedSong>,
) -> Result<String, String> {
    import.save(&name, songs).await
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
    id: String,
) -> Result<raygun_model::Mix, String> {
    service.mix(&id).await
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

/// Every chart source's songs for a genre or a mood (M46.19).
#[tauri::command]
#[specta::specta]
pub async fn music_tag_sources(
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    name: String,
    region: Option<String>,
) -> Result<raygun_model::MusicTagSources, String> {
    let sources = discovery.tag_sources(name, region).await;
    for row in &sources.rows {
        durations.learn_tracks(&row.tracks);
    }
    Ok(sources)
}

/// A chart's songs (M46.9).
#[tauri::command]
#[specta::specta]
pub async fn music_chart(
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    durations: tauri::State<'_, Arc<crate::durations::DurationService>>,
    provider_id: String,
    chart_id: String,
    region: Option<String>,
) -> Result<Vec<raygun_model::OnlineTrack>, String> {
    let tracks = discovery.chart(&provider_id, &chart_id, region).await?;
    durations.learn_tracks(&tracks);
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
    name: String,
) -> Result<raygun_model::ArtistOverview, String> {
    let name = name.trim().to_owned();
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
    Ok(raygun_model::ArtistOverview {
        popular: crate::search::artist_tracks(&providers, &name, ARTIST_POPULAR_LIMIT),
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
    name: String,
) -> Result<Option<raygun_model::ArtistInfo>, String> {
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

/// Raygun's test repository: extensions that fail on purpose (M46.17, `repo-test/`).
const TEST_REPO: &str = "https://raw.githubusercontent.com/uvxszi/raygun/main/repo-test/index.json";

/// Stremio addons for testing video (M46.17): Raygun's own static test addon (legal films with
/// dead links first, live news channels), then legal addons that exercise metadata, subtitles,
/// torrents (unsupported), outside links, live TV and radio.
const TEST_ADDONS: [&str; 8] = [
    "https://raw.githubusercontent.com/uvxszi/raygun/main/repo-test/stremio/manifest.json",
    "https://v3-cinemeta.strem.io/manifest.json",
    "https://opensubtitles-v3.strem.io/manifest.json",
    "https://caching.stremio.net/publicdomainmovies.now.sh/manifest.json",
    "https://watchhub.strem.io/manifest.json",
    "https://v3-channels.strem.io/manifest.json",
    "https://dev.nebulawp.org/stremio/pluto-tv-addon/manifest.json",
    "https://1fe84bc728af-stremio-radios.baby-beamup.club/manifest.json",
];

/// Adds the test sources (M46.17, Settings → Data → Developer): every suggested repository,
/// Raygun's test repository and the test addons. Never part of a fresh install.
#[tauri::command]
#[specta::specta]
pub async fn dev_add_test_sources(
    extensions: tauri::State<'_, Arc<ExtensionService>>,
    stremio: Stremio<'_>,
) -> Result<raygun_model::TestSourcesReport, String> {
    let mut report = raygun_model::TestSourcesReport {
        added: Vec::new(),
        already: Vec::new(),
        failed: Vec::new(),
    };
    let repos = extensions.repos().await?;
    let mut urls = crate::extensions::suggested_repo_urls();
    urls.push(TEST_REPO);
    for url in urls {
        if repos.iter().any(|repo| repo.url.eq_ignore_ascii_case(url)) {
            report.already.push(url.to_owned());
            continue;
        }
        match extensions.add_repo(url).await {
            Ok(_) => report.added.push(url.to_owned()),
            Err(error) => report.failed.push(raygun_model::TestSourceFailure {
                url: url.to_owned(),
                error,
            }),
        }
    }
    let addons = stremio.addons().await.unwrap_or_default();
    for url in TEST_ADDONS {
        let base = url.trim_end_matches("/manifest.json");
        if addons.iter().any(|addon| {
            addon
                .url
                .trim_end_matches("/manifest.json")
                .eq_ignore_ascii_case(base)
        }) {
            report.already.push(url.to_owned());
            continue;
        }
        match stremio.add(url).await {
            Ok(_) => report.added.push(url.to_owned()),
            Err(error) => report.failed.push(raygun_model::TestSourceFailure {
                url: url.to_owned(),
                error,
            }),
        }
    }
    Ok(report)
}

/// Where this device's player plays (M46.15).
#[tauri::command]
#[specta::specta]
pub fn connect_status(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
) -> raygun_model::ConnectStatus {
    connect.status()
}

/// Other Raygun devices to play on: heard on the network, added by address, or allowed before.
#[tauri::command]
#[specta::specta]
pub fn connect_devices(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
) -> Vec<raygun_model::ConnectDevice> {
    connect.inner().devices()
}

/// Adds a device by its address (`192.168.1.20:8820`).
#[tauri::command]
#[specta::specta]
pub async fn connect_add(
    connect: tauri::State<'_, Arc<crate::connect::ConnectService>>,
    address: String,
) -> Result<raygun_model::ConnectDevice, String> {
    connect.add(&address).await
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
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    track: TrackInput,
    favorite: bool,
) -> Result<(), String> {
    collections.set_favorite(track, favorite).await?;
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

/// Appends tracks to the end of a playlist.
#[tauri::command]
#[specta::specta]
pub async fn playlist_add(
    collections: tauri::State<'_, CollectionsService>,
    discovery: tauri::State<'_, Arc<DiscoveryService>>,
    id: String,
    tracks: Vec<TrackInput>,
) -> Result<(), String> {
    collections.add_to_playlist(&id, tracks).await?;
    discovery.nudge();
    Ok(())
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

/// Downloads an episode in the background; progress arrives as `PodcastsChangedEvent`s.
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
    if medium == "video" {
        return Err("Episodes are downloaded from their title page".into());
    }
    let catalog = catalog_for(&app, &medium)?;
    crate::library_actions::download_next(&app, catalog, &title_ids, amount).await
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

/// Updates one category's titles (manga and video libraries).
#[tauri::command]
#[specta::specta]
pub async fn library_update_category(
    app: tauri::AppHandle,
    manga: tauri::State<'_, MangaService>,
    medium: String,
    category_id: String,
) -> Result<raygun_model::MangaUpdateStatus, String> {
    if medium == "novel" {
        return Err("Novels aren't updated yet".into());
    }
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
) -> Result<Settings, String> {
    let current = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    let picked = pick_folder(&app).await?;
    let Some(uri) = picked.uri else {
        return Ok(current);
    };
    if let Some(old) = current.save.folder_uri.as_deref().filter(|old| *old != uri) {
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
) -> Result<Option<Settings>, String> {
    let current = settings::load(&db.0)
        .await
        .map_err(|error| error.to_string())?;
    let picked = pick_folder(&app).await?;
    let Some(uri) = picked.uri else {
        return Ok(None);
    };
    if let Some(old) = current.save.folder_uri.as_deref().filter(|old| *old != uri) {
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
    websites.add(name, &url).await
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
/// neither it lists the source's popular titles.
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
    catalog_for(&app, &medium)?
        .search(&source_id, &query, page, &filters)
        .await
}

/// The first page of every manga, novel or video source's results for `query` (their popular
/// titles when it's blank), or only `sources`' when any are given (Search's source chips).
#[tauri::command]
#[specta::specta]
pub async fn catalog_global_search(
    app: tauri::AppHandle,
    medium: String,
    query: String,
    sources: Vec<String>,
) -> Result<Vec<raygun_model::SourceSearchResult>, String> {
    catalog_for(&app, &medium)?
        .global_search(&query, &sources)
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

/// Adds an addon by its address (`.../manifest.json`, `stremio://...` or a bare host).
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

/// The Watch home: the first page of every browsable catalog.
#[tauri::command]
#[specta::specta]
pub async fn stremio_home(
    stremio: Stremio<'_>,
) -> Result<Vec<raygun_model::StremioCatalogRow>, String> {
    stremio.home().await
}

/// One catalog's page, optionally for a genre, `skip` items in.
#[tauri::command]
#[specta::specta]
pub async fn stremio_catalog(
    stremio: Stremio<'_>,
    addon_id: String,
    kind: String,
    catalog_id: String,
    genre: Option<String>,
    skip: u32,
) -> Result<raygun_model::StremioCatalogRow, String> {
    stremio
        .catalog(&addon_id, &kind, &catalog_id, genre.as_deref(), skip)
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

/// Streams for a movie or an episode, grouped by addon.
#[tauri::command]
#[specta::specta]
pub async fn stremio_streams(
    stremio: Stremio<'_>,
    kind: String,
    video_id: String,
) -> Result<Vec<raygun_model::StremioStreamGroup>, String> {
    stremio.streams(&kind, &video_id).await
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

/// A report from the webview's video player (desktop, M46.4), in the Android player's JSON.
#[tauri::command]
#[specta::specta]
pub fn video_player_report(reports: tauri::State<'_, crate::video::VideoReports>, report: String) {
    reports.send(report);
}

/// Plays a video in mpv or VLC (desktop, M46.4); returns the app's name.
#[tauri::command]
#[specta::specta]
pub fn video_player_external(play: raygun_model::ExternalPlay) -> Result<String, String> {
    #[cfg(not(target_os = "android"))]
    {
        crate::video_desktop::play_external(&play)
    }
    #[cfg(target_os = "android")]
    {
        let _ = play;
        Err("Android picks another app from the player".into())
    }
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

/// Downloads a link of a stored episode.
#[tauri::command]
#[specta::specta]
pub async fn video_source_download(
    sources: VideoSources<'_>,
    downloads: VideoDownloadsState<'_>,
    request: raygun_model::VideoSourcePlay,
) -> Result<(), String> {
    let title_id = request.title_id.clone();
    let video = sources.request(request).await?;
    downloads.enqueue(video, Some(title_id)).await
}

/// Downloads a direct stream of a movie or episode, with subtitles (`language` first).
#[tauri::command]
#[specta::specta]
pub async fn stremio_download(
    stremio: Stremio<'_>,
    downloads: VideoDownloadsState<'_>,
    request: raygun_model::StremioPlayRequest,
    language: String,
    title_id: Option<String>,
) -> Result<(), String> {
    let video = stremio.request(&request, &language).await?;
    downloads.enqueue(video, title_id).await
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
