//! Extensions: the built-in first-party extensions plus any installed under `extensions/` (M11),
//! installed from repositories with per-extension settings (M13).
//!
//! Debug builds also watch that folder and reload when it changes (M12), so `raygun-ext dev`
//! reaches the running app without a restart.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use raygun_db::extensions::{self as store, InstallRow};
use raygun_ext_host::nuclear;
use raygun_ext_host::package::{self, MAX_PACKAGE_BYTES};
use raygun_ext_host::repo::{self, PackageFormat, RepoEntry, RepoIndex};
use raygun_ext_host::shosetsu;
use raygun_ext_host::{
    ExtensionHost, ExtensionSource, HostConfig, ProviderCandidates, ResolvedStream, UserSettings,
    discover_installed,
};
use raygun_model::{
    CatalogEntry, CatalogState, ExtensionInfo, ExtensionOrigin, ExtensionRepo, ExtensionSetting,
    Lyrics, MusicChart, OnlineTrack, ProviderInfo, ProviderSearchResult, RepoCatalog, SettingKind,
    SettingValue, SuggestedRepo, TrackQuery,
};
use raygun_paths::Paths;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::RwLock;

use crate::events::ExtensionsChangedEvent;
use crate::startup::AppDb;

const SEARCH_LIMIT: u32 = 30;
/// Candidates asked of each `music.stream` provider per track.
const CANDIDATE_LIMIT: u32 = 10;
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
const INDEX_CACHE: Duration = Duration::from_secs(10 * 60);
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Extensions compiled into the app; an installed copy with the same id overrides one.
fn built_in() -> Vec<ExtensionSource> {
    vec![
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/podcast-directory/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/podcast-directory/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/internet-archive/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/internet-archive/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/musicbrainz/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/musicbrainz/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/deezer/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/deezer/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/apple-charts/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/apple-charts/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/listenbrainz/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/listenbrainz/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/librivox/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/librivox/dist/index.js"),
            ),
        ]),
        // A second audiobook source (M46.47).
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/loyalbooks/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/loyalbooks/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/audius/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/audius/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/lrclib/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/lrclib/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/mangadex/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/mangadex/dist/index.js"),
            ),
        ]),
        // Official free comics and openly licensed or public-domain ones.
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/webtoons/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/webtoons/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/tapas/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/tapas/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/mangaplus-creators/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/mangaplus-creators/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/naver-webtoon/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/naver-webtoon/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/archive-comics/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/archive-comics/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/peppercarrot/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/peppercarrot/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/xkcd/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/xkcd/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/gutenberg/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/gutenberg/dist/index.js"),
            ),
        ]),
    ]
}

type Fingerprint = Vec<(PathBuf, u64, Option<SystemTime>)>;

pub struct ExtensionService {
    app: AppHandle,
    host: ExtensionHost,
    http: reqwest::Client,
    extensions_dir: PathBuf,
    storage_dir: PathBuf,
    /// Artist facts and song credits already asked for (M46.14); `None` = nobody knew.
    info_cache: Mutex<InfoCache>,
    /// Held for writing while extensions (re)load, so calls wait instead of seeing a partial set.
    loading: Arc<RwLock<()>>,
    /// One install or uninstall at a time.
    changing: tokio::sync::Mutex<()>,
    /// The installed files as of the last load, so the dev watcher skips the app's own changes.
    loaded_files: Mutex<Fingerprint>,
    indexes: Mutex<HashMap<String, (Instant, RepoIndex)>>,
}

/// What `music.info` providers answered this run (M46.14).
#[derive(Default)]
struct InfoCache {
    artists: HashMap<String, Option<raygun_model::ArtistInfo>>,
    credits: HashMap<String, Option<raygun_model::SongCredits>>,
}

impl ExtensionService {
    /// Starts the host and loads extensions in the background.
    pub fn start(app: AppHandle, paths: &Paths) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let extensions_dir = paths.extensions();
        let storage_dir = extensions_dir.join(".data");
        let host = ExtensionHost::start(HostConfig {
            storage_dir: Some(storage_dir.clone()),
            ..HostConfig::default()
        })?;
        let service = Arc::new(Self {
            app,
            host,
            http: raygun_net::http_client()?,
            extensions_dir,
            storage_dir,
            loading: Arc::new(RwLock::new(())),
            changing: tokio::sync::Mutex::new(()),
            loaded_files: Mutex::new(Vec::new()),
            indexes: Mutex::new(HashMap::new()),
            info_cache: Mutex::new(InfoCache::default()),
        });
        // Before anything lists repositories: one quick query when there's nothing to move.
        tauri::async_runtime::block_on(service.replace_retired_repos());
        // Take the lock now, before any command can run, and load in the background.
        let guard = service
            .loading
            .clone()
            .try_write_owned()
            .expect("a new lock is free");
        let loader = service.clone();
        tauri::async_runtime::spawn(async move {
            match crate::settings::load(loader.db()).await {
                Ok(settings) => {
                    raygun_net::set_user_agent(&settings.user_agent);
                    loader.host.content().set_show_nsfw(settings.show_nsfw);
                    loader
                        .host
                        .content()
                        .set_show_explicit_music(settings.explicit_music);
                    loader
                        .host
                        .content()
                        .set_disabled(&settings.disabled_sources);
                }
                Err(error) => tracing::warn!(%error, "could not read the NSFW setting"),
            }
            loader.load_all().await;
            drop(guard);
            if cfg!(debug_assertions) {
                loader.watch().await;
            }
        });
        Ok(service)
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    pub async fn list(&self) -> Vec<ExtensionInfo> {
        let infos = {
            let _ready = self.loading.read().await;
            self.host.list().await
        };
        self.with_icons(infos).await
    }

    /// Adds the icons remembered at install, kept in the cover cache (M46.2). Reloads answer with
    /// them too, or the lists they replace in the UI lose every icon (M46.11).
    async fn with_icons(&self, infos: Vec<ExtensionInfo>) -> Vec<ExtensionInfo> {
        let images = self.app.state::<raygun_net::ImageLinks>();
        let mut with_icons = Vec::with_capacity(infos.len());
        for info in infos {
            let icon = raygun_db::local_value(self.db(), &icon_key(&info.id))
                .await
                .ok()
                .flatten()
                .filter(|url| images.is_remote(url))
                .map(|url| images.kept_cover(&url, &Default::default()));
            with_icons.push(ExtensionInfo { icon, ..info });
        }
        with_icons
    }

    /// Whether NSFW extensions and content are shown (M35).
    /// The sources and providers the user turned off (M46.8): searches and lookups skip them.
    pub fn set_disabled_sources(&self, ids: &[String]) {
        self.host.content().set_disabled(ids);
    }

    pub fn show_nsfw(&self) -> bool {
        self.host.content().show_nsfw()
    }

    /// Turns NSFW content and explicit music (M46.36) on or off and tells the UI, whose source
    /// lists depend on them.
    pub async fn set_content(&self, show_nsfw: bool, explicit_music: bool) {
        self.host.content().set_show_nsfw(show_nsfw);
        self.host.content().set_show_explicit_music(explicit_music);
        let infos = self.list().await;
        if let Err(error) = ExtensionsChangedEvent(infos).emit(&self.app) {
            tracing::warn!(%error, "could not announce extension changes");
        }
    }

    pub async fn search(&self, query: &str) -> Vec<ProviderSearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let _ready = self.loading.read().await;
        self.host.search_music(query, SEARCH_LIMIT).await
    }

    /// Stream candidates for a track from every `music.stream` provider (M14), aiming for
    /// `quality` (M46.18).
    pub async fn stream_candidates(
        &self,
        query: &TrackQuery,
        quality: raygun_resolver::QualityTarget,
    ) -> Vec<ProviderCandidates> {
        let _ready = self.loading.read().await;
        self.host
            .stream_candidates(query, CANDIDATE_LIMIT, quality)
            .await
    }

    /// Lyrics for a track from the `lyrics` providers (M15).
    /// The host once any (re)load finished; keep the guard while calling it.
    pub async fn ready(&self) -> (tokio::sync::RwLockReadGuard<'_, ()>, &ExtensionHost) {
        (self.loading.read().await, &self.host)
    }

    pub async fn lyrics(&self, query: &TrackQuery) -> Result<Option<Lyrics>, String> {
        let _ready = self.loading.read().await;
        self.host.lyrics(query).await
    }

    /// Every lyrics source's lyrics for a song (M46.36).
    pub async fn all_lyrics(&self, query: &TrackQuery) -> Result<Vec<Lyrics>, String> {
        let _ready = self.loading.read().await;
        self.host.all_lyrics(query).await
    }

    /// Artists for a search, from every source that searches artists (M46.9).
    pub async fn search_artists(&self, query: &str) -> Vec<raygun_model::MusicArtist> {
        let _ready = self.loading.read().await;
        self.host.search_artists(query, SEARCH_LIMIT).await
    }

    /// Albums for a search, from every source that searches albums (M46.9).
    pub async fn search_albums(&self, query: &str) -> Vec<raygun_model::MusicAlbum> {
        let _ready = self.loading.read().await;
        self.host.search_albums(query, SEARCH_LIMIT).await
    }

    /// An artist's discography (M46.36): every source's releases credited to them, one of each
    /// (the copy with artwork and a year wins), newest first.
    pub async fn artist_releases(&self, name: &str) -> Vec<raygun_model::MusicAlbum> {
        let found = {
            let _ready = self.loading.read().await;
            self.host.artist_releases(name, RELEASES_LIMIT).await
        };
        releases_of(found, name)
    }

    /// The album a song is on (M46.36: Go to album): the best of the sources' album hits named
    /// like `album`, by the song's artist when it has one.
    pub async fn find_album(
        &self,
        album: &str,
        artist: Option<&str>,
    ) -> Option<raygun_model::MusicAlbum> {
        let query = match artist {
            Some(artist) if !artist.trim().is_empty() => format!("{album} {artist}"),
            _ => album.to_owned(),
        };
        let found = {
            let _ready = self.loading.read().await;
            self.host.search_albums(&query, SEARCH_LIMIT).await
        };
        best_album(found, album, artist)
    }

    /// Covers the sources have for a song, an album or an artist (M46.36): the songs, albums and
    /// artists they find by that name, each picture once. A song also gets its album's covers.
    pub async fn cover_options(
        &self,
        target: &raygun_model::DetailTarget,
    ) -> Vec<raygun_model::CoverOption> {
        use raygun_model::DetailTarget;
        let _ready = self.loading.read().await;
        let mut options = Vec::new();
        match target {
            DetailTarget::Song { query } => {
                let artist = query.artist.as_deref().unwrap_or_default();
                let songs_query = format!("{artist} {}", query.title);
                let album_query = query
                    .album
                    .as_deref()
                    .map(|album| format!("{album} {artist}"));
                let (songs, albums) = futures_util::future::join(
                    self.host
                        .search_music(songs_query.trim(), COVER_SEARCH_LIMIT),
                    async {
                        match &album_query {
                            Some(text) => {
                                self.host
                                    .search_albums(text.trim(), COVER_SEARCH_LIMIT)
                                    .await
                            }
                            None => Vec::new(),
                        }
                    },
                )
                .await;
                let title = raygun_resolver::core_title(&query.title);
                for result in songs {
                    for track in result.tracks {
                        let same = raygun_resolver::core_title(&track.title) == title
                            && credited(track.artist.as_deref(), query.artist.as_deref());
                        if let (true, Some(url)) = (same, track.artwork_url) {
                            options.push(raygun_model::CoverOption {
                                url,
                                source: result.provider.name.clone(),
                                of: track.album.filter(|album| !album.is_empty()),
                            });
                        }
                    }
                }
                if let Some(album) = &query.album {
                    options.extend(album_covers(albums, album, query.artist.as_deref()));
                }
            }
            DetailTarget::Album { artist, album } => {
                let text = format!("{album} {}", artist.as_deref().unwrap_or_default());
                let albums = self
                    .host
                    .search_albums(text.trim(), COVER_SEARCH_LIMIT)
                    .await;
                options.extend(album_covers(albums, album, artist.as_deref()));
            }
            DetailTarget::Artist { name } => {
                let wanted = raygun_resolver::normalize(name);
                for artist in self.host.search_artists(name, COVER_SEARCH_LIMIT).await {
                    if raygun_resolver::normalize(&artist.name) == wanted
                        && let Some(url) = artist.image_url
                    {
                        options.push(raygun_model::CoverOption {
                            url,
                            source: artist.provider.name,
                            of: None,
                        });
                    }
                }
            }
            // Its songs' covers come from the playlist itself (`detail_cover_options`).
            DetailTarget::Playlist { .. } => {}
        }
        let mut seen = std::collections::HashSet::new();
        options.retain(|option| seen.insert(option.url.clone()));
        options.truncate(COVER_OPTIONS);
        options
    }

    /// An artist's coming events from every source that lists them (M46.42), soonest first.
    pub async fn artist_events(&self, name: &str) -> Vec<raygun_model::ArtistEvent> {
        let mut events = {
            let _ready = self.loading.read().await;
            self.host.artist_events(name, 25).await
        };
        events.sort_by(|a, b| a.date.cmp(&b.date));
        events.dedup_by(|a, b| a.date == b.date && a.name == b.name);
        events
    }

    /// Playlists for a search from every source that searches them (M46.39).
    pub async fn search_playlists(
        &self,
        query: &str,
        featured: bool,
    ) -> Vec<raygun_model::MusicPlaylist> {
        let _ready = self.loading.read().await;
        self.host
            .search_playlists(query, featured, SEARCH_LIMIT)
            .await
    }

    /// A playlist's songs (M46.39).
    pub async fn music_playlist(
        &self,
        provider_id: &str,
        playlist_id: &str,
    ) -> Result<raygun_model::MusicPlaylistDetail, String> {
        let _ready = self.loading.read().await;
        self.host
            .music_playlist(provider_id, playlist_id)
            .await
            .map_err(|error| error.to_string())
    }

    /// An album's songs (M46.9).
    pub async fn music_album(
        &self,
        provider_id: &str,
        album_id: &str,
    ) -> Result<raygun_model::MusicAlbumDetail, String> {
        let _ready = self.loading.read().await;
        self.host
            .music_album(provider_id, album_id)
            .await
            .map_err(|error| error.to_string())
    }

    /// Every chart source's charts (M46.9).
    /// What open databases say about an artist (M46.14), remembered for this run.
    pub async fn artist_info(&self, name: &str) -> Option<raygun_model::ArtistInfo> {
        let key = name.trim().to_lowercase();
        if let Some(known) = self.info_cache.lock().unwrap().artists.get(&key) {
            return known.clone();
        }
        let info = {
            let (_ready, host) = self.ready().await;
            host.artist_info(name.trim()).await
        };
        self.info_cache
            .lock()
            .unwrap()
            .artists
            .insert(key, info.clone());
        info
    }

    /// Who made a song (M46.14), remembered for this run.
    pub async fn song_credits(&self, query: &TrackQuery) -> Option<raygun_model::SongCredits> {
        let key = raygun_resolver::track_key(query);
        if let Some(known) = self.info_cache.lock().unwrap().credits.get(&key) {
            return known.clone();
        }
        let credits = {
            let (_ready, host) = self.ready().await;
            host.song_credits(query).await
        };
        self.info_cache
            .lock()
            .unwrap()
            .credits
            .insert(key, credits.clone());
        credits
    }

    pub async fn music_charts(
        &self,
        region: Option<&str>,
    ) -> Vec<(ProviderInfo, Result<Vec<MusicChart>, String>)> {
        let _ready = self.loading.read().await;
        self.host.music_charts(region).await
    }

    /// Artists like `artist` (M46.20).
    pub async fn similar_artists(&self, artist: &str, limit: u32) -> Vec<String> {
        let _ready = self.loading.read().await;
        self.host.similar_artists(artist, limit).await
    }

    /// Songs from a `music.recommend` call (M46.20).
    pub async fn recommended_tracks(
        &self,
        method: &str,
        args: serde_json::Value,
    ) -> Vec<OnlineTrack> {
        let _ready = self.loading.read().await;
        self.host.recommended_tracks(method, args).await
    }

    /// Every chart source's songs for a genre or a mood (M46.19).
    pub async fn music_tag_songs(
        &self,
        tag: &str,
        aliases: &[String],
        kind: &str,
        region: Option<&str>,
        limit: u32,
    ) -> Vec<(
        ProviderInfo,
        Result<(Option<String>, Vec<OnlineTrack>), String>,
    )> {
        let _ready = self.loading.read().await;
        self.host
            .music_tag_songs(tag, aliases, kind, region, limit)
            .await
    }

    /// A chart's songs (M46.9).
    pub async fn music_chart(
        &self,
        provider_id: &str,
        chart_id: &str,
        region: Option<&str>,
        limit: u32,
    ) -> Result<Vec<OnlineTrack>, String> {
        let _ready = self.loading.read().await;
        self.host
            .music_chart(provider_id, chart_id, region, limit)
            .await
            .map_err(|error| error.to_string())
    }

    /// Every tag source's genres for a song (M46.9).
    pub async fn music_tags(
        &self,
        query: &TrackQuery,
    ) -> Vec<(ProviderInfo, Result<Vec<(String, f64)>, String>)> {
        let _ready = self.loading.read().await;
        self.host.music_tags(query).await
    }

    /// The stream behind a candidate that came without one.
    pub async fn resolve_stream(
        &self,
        provider_id: &str,
        candidate_id: &str,
    ) -> Result<ResolvedStream, String> {
        let _ready = self.loading.read().await;
        self.host
            .resolve_stream(provider_id, candidate_id)
            .await
            .map_err(|error| error.to_string())
    }

    /// Reloads every extension from scratch (built-in and installed).
    pub async fn reload(&self) -> Vec<ExtensionInfo> {
        let _loading = self.loading.write().await;
        self.load_all().await
    }

    async fn load_all(&self) -> Vec<ExtensionInfo> {
        let dir = self.extensions_dir.clone();
        let files = tauri::async_runtime::spawn_blocking(move || fingerprint(&dir))
            .await
            .unwrap_or_default();
        *self.loaded_files.lock().unwrap() = files;

        let mut sources = built_in();
        sources.extend(discover_installed(&self.extensions_dir));
        let settings = self.user_settings().await;
        let infos = self.host.load_with_settings(sources, settings).await;
        let infos = self.with_icons(infos).await;
        tracing::info!(count = infos.len(), "extensions ready");
        if let Err(error) = ExtensionsChangedEvent(infos.clone()).emit(&self.app) {
            tracing::warn!(%error, "could not announce extension changes");
        }
        infos
    }

    async fn user_settings(&self) -> UserSettings {
        let rows = store::settings(self.db()).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "could not read extension settings");
            Vec::new()
        });
        let mut settings = UserSettings::new();
        for (extension_id, key, json) in rows {
            if let Ok(value) = serde_json::from_str(&json) {
                settings.entry(extension_id).or_default().insert(key, value);
            }
        }
        settings
    }

    /// Reloads whenever the installed files change. Polling (rather than OS file events) works the
    /// same on every platform, including files pushed into the Android sandbox over adb.
    async fn watch(&self) {
        loop {
            tokio::time::sleep(WATCH_INTERVAL).await;
            let dir = self.extensions_dir.clone();
            let current = tauri::async_runtime::spawn_blocking(move || fingerprint(&dir))
                .await
                .unwrap_or_default();
            if *self.loaded_files.lock().unwrap() != current {
                tracing::info!("installed extensions changed, reloading");
                self.reload().await;
            }
        }
    }

    // ---- Repositories ----

    pub async fn repos(&self) -> Result<Vec<ExtensionRepo>, String> {
        let rows = store::repos(self.db())
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows.into_iter().map(to_repo).collect())
    }

    /// Adds a repository after checking that its index can be read; an index Paraline replaced
    /// adds Paraline.
    pub async fn add_repo(&self, url: &str) -> Result<Vec<ExtensionRepo>, String> {
        let url = repo::check_index_url(url).map_err(|error| error.to_string())?;
        let mut url = current_repo_url(url);
        let index = match self.fetch_index(url.as_str(), true).await {
            Ok(index) => index,
            // Shosetsu repositories are shared by their folder or their GitHub/GitLab page.
            Err(error) => {
                let mut found = None;
                for candidate in repo::shosetsu_index_candidates(&url) {
                    if let Ok(index) = self.fetch_index(candidate.as_str(), true).await {
                        found = Some((candidate, index));
                        break;
                    }
                }
                let (inside, index) = found.ok_or(error)?;
                url = inside;
                index
            }
        };
        store::add_repo(self.db(), url.as_str(), &repo_name(&index, url.as_str()))
            .await
            .map_err(|error| error.to_string())?;
        self.repos().await
    }

    /// Folds the repositories Paraline replaced into it, keeping installed extensions matched
    /// for updates: the catalogue finds them by id in Paraline's index.
    async fn replace_retired_repos(&self) {
        let paraline = store::RepoRow {
            id: PARALINE_ID.to_owned(),
            url: PARALINE_URL.to_owned(),
            name: PARALINE_NAME.to_owned(),
        };
        match store::replace_repos(self.db(), &RETIRED_REPOS, &paraline).await {
            Ok(true) => tracing::info!("replaced repositories moved to Paraline"),
            Ok(false) => {}
            Err(error) => {
                tracing::warn!(%error, "could not move replaced repositories to Paraline")
            }
        }
    }

    pub async fn remove_repo(&self, id: &str) -> Result<Vec<ExtensionRepo>, String> {
        store::remove_repo(self.db(), id)
            .await
            .map_err(|error| error.to_string())?;
        self.repos().await
    }

    /// Every repository's extensions, marked against what this device has.
    /// Well-known repositories, marked when this device already has them (M36).
    pub async fn suggested_repos(&self) -> Result<Vec<SuggestedRepo>, String> {
        let repos = self.repos().await?;
        Ok(SUGGESTED_REPOS
            .iter()
            .map(|(url, name, description)| SuggestedRepo {
                added: repos.iter().any(|repo| same_url(&repo.url, url)),
                url: (*url).to_owned(),
                name: (*name).to_owned(),
                description: (*description).to_owned(),
            })
            .collect())
    }

    pub async fn catalog(&self, refresh: bool) -> Result<Vec<RepoCatalog>, String> {
        let repos = self.repos().await?;
        let local: HashMap<String, Installed> = self
            .list()
            .await
            .into_iter()
            .map(|info| {
                let installed = Installed {
                    stale_conversion: info.compatibility.as_ref().is_some_and(|report| {
                        report.converter < raygun_ext_host::nuclear::CONVERTER_VERSION
                    }),
                    version: info.version,
                };
                (info.id, installed)
            })
            .collect();
        let listings = repos.into_iter().map(|repo| {
            let local = &local;
            async move {
                match self.fetch_index(&repo.url, refresh).await {
                    Ok(index) => {
                        let name = repo_name(&index, &repo.url);
                        if name != repo.name {
                            let _ = store::rename_repo(self.db(), &repo.id, &name).await;
                        }
                        let repo = ExtensionRepo { name, ..repo };
                        let show_nsfw = self.host.content().show_nsfw();
                        let index_url: Option<reqwest::Url> = repo.url.parse().ok();
                        let images = self.app.state::<raygun_net::ImageLinks>();
                        let entries = index
                            .extensions
                            .iter()
                            .filter(|entry| show_nsfw || entry.content_rating != "nsfw")
                            .map(|entry| CatalogEntry {
                                icon: index_url
                                    .as_ref()
                                    .and_then(|base| repo::icon_url(base, entry))
                                    .map(|url| images.cover(url.as_str(), &Default::default())),
                                ..catalog_entry(&repo.id, entry, local.get(&entry.id))
                            })
                            .collect();
                        RepoCatalog {
                            repo,
                            entries,
                            error: None,
                        }
                    }
                    Err(error) => RepoCatalog {
                        repo,
                        entries: Vec::new(),
                        error: Some(error),
                    },
                }
            }
        });
        // A few repositories at a time (M46.7).
        Ok(crate::stremio::at_most(4, listings).await)
    }

    async fn fetch_index(&self, url: &str, refresh: bool) -> Result<RepoIndex, String> {
        if !refresh
            && let Some((fetched, index)) = self.indexes.lock().unwrap().get(url)
            && fetched.elapsed() < INDEX_CACHE
        {
            return Ok(index.clone());
        }
        let bytes = self.download(url, MAX_INDEX_BYTES).await?;
        let index = repo::parse_index(&String::from_utf8_lossy(&bytes))
            .map_err(|error| error.to_string())?;
        self.indexes
            .lock()
            .unwrap()
            .insert(url.to_owned(), (Instant::now(), index.clone()));
        Ok(index)
    }

    /// A repository file, asked for again after a pause when the host says too many requests
    /// (GitLab limits raw downloads, and a Shosetsu script comes with its libraries).
    async fn download_patiently(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut attempt = 0;
        loop {
            match self.download(url, limit).await {
                Err(error) if error.ends_with("answered HTTP 429") && attempt < 3 => {
                    attempt += 1;
                    tokio::time::sleep(Duration::from_secs(2 * attempt)).await;
                }
                other => return other,
            }
        }
    }

    async fn download(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        // A repository on this computer (M46.28: Paraline Community, desktop only).
        if let Ok(parsed) = reqwest::Url::parse(url)
            && parsed.scheme() == "file"
        {
            let path = parsed
                .to_file_path()
                .map_err(|()| format!("{url} isn't a usable path"))?;
            let bytes = tokio::fs::read(&path)
                .await
                .map_err(|error| format!("could not read {}: {error}", path.display()))?;
            if bytes.len() as u64 > limit {
                return Err(format!("{url} is larger than {limit} bytes"));
            }
            return Ok(bytes);
        }
        let mut response = self
            .http
            .get(url)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|error| format!("could not reach {url}: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "{url} answered HTTP {}",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > limit)
        {
            return Err(format!("{url} is larger than {limit} bytes"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("download from {url} failed: {error}"))?
        {
            if bytes.len() as u64 + chunk.len() as u64 > limit {
                return Err(format!("{url} is larger than {limit} bytes"));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    // ---- Install, update, uninstall ----

    /// Installs (or updates to) the version a repository currently lists.
    pub async fn install(
        &self,
        repo_id: &str,
        extension_id: &str,
    ) -> Result<Vec<ExtensionInfo>, String> {
        let _changing = self.changing.lock().await;
        let repo = self
            .repos()
            .await?
            .into_iter()
            .find(|repo| repo.id == repo_id)
            .ok_or("that repository was removed")?;
        // Always the current index: a cached one could name a package that has since changed.
        let index = self.fetch_index(&repo.url, true).await?;
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == extension_id)
            .ok_or_else(|| format!("{} no longer lists {extension_id}", repo.name))?;
        let index_url: reqwest::Url = repo.url.parse().map_err(|_| "invalid repository URL")?;
        let package_url =
            repo::package_url(&index_url, entry).map_err(|error| error.to_string())?;
        let limit = if entry.size == 0 {
            MAX_PACKAGE_BYTES
        } else {
            entry.size.min(MAX_PACKAGE_BYTES)
        };
        let bytes = self.download_patiently(package_url.as_str(), limit).await?;

        let verified = match entry.format {
            PackageFormat::Raygun => {
                package::verify(&bytes, entry).map_err(|error| error.to_string())?
            }
            PackageFormat::Nuclear => {
                package::check_nuclear_download(&bytes, entry)
                    .map_err(|error| error.to_string())?;
                let sources = nuclear::unzip_sources(&bytes).map_err(|error| error.to_string())?;
                let mut converted =
                    tauri::async_runtime::spawn_blocking(move || nuclear::convert(&sources))
                        .await
                        .map_err(|error| error.to_string())??;
                // The repository's version, not the package's own: plugins forget to bump theirs,
                // and the catalogue then offered the same update forever (M46.8).
                converted.manifest = with_version(&converted.manifest, &entry.version)?;
                let verified = package::from_converted(&converted.manifest, &converted.bundle)
                    .map_err(|error| error.to_string())?;
                if verified.manifest.id != entry.id {
                    return Err(format!(
                        "the plugin's package name gives {} rather than {}",
                        verified.manifest.id, entry.id
                    ));
                }
                tracing::info!(extension = %entry.id, hosts = ?converted.report.hosts, "Nuclear plugin converted");
                verified
            }
            PackageFormat::Shosetsu => {
                // The libraries it needs, and theirs: from its own repository, else (as Shosetsu
                // shares every repository's libraries) from the other Shosetsu ones added, which
                // are only asked when its own lacks one.
                let needs_others =
                    shosetsu::package::dependencies(&String::from_utf8_lossy(&bytes))
                        .iter()
                        .any(|name| !index.libraries.iter().any(|library| library.name == *name));
                let mut others: Vec<(reqwest::Url, RepoIndex)> = Vec::new();
                for other in self.repos().await? {
                    if !needs_others || other.id == repo.id {
                        continue;
                    }
                    if let (Ok(url), Ok(index)) = (
                        other.url.parse::<reqwest::Url>(),
                        self.fetch_index(&other.url, false).await,
                    ) && !index.libraries.is_empty()
                    {
                        others.push((url, index));
                    }
                }
                let mut sources = vec![shosetsu::package::LibrarySource {
                    index_url: &index_url,
                    index: &index,
                }];
                sources.extend(others.iter().map(|(index_url, index)| {
                    shosetsu::package::LibrarySource { index_url, index }
                }));
                shosetsu::package::assemble(entry, &bytes, &sources, |url| async move {
                    self.download_patiently(url.as_str(), MAX_PACKAGE_BYTES)
                        .await
                })
                .await?
            }
        };
        let extensions_dir = self.extensions_dir.clone();
        tauri::async_runtime::spawn_blocking(move || package::install(&extensions_dir, &verified))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        store::record_install(
            self.db(),
            &InstallRow {
                extension_id: entry.id.clone(),
                repo_url: repo.url.clone(),
                version: entry.version.clone(),
                sha256: entry.sha256.to_ascii_lowercase(),
                installed_at: unix_ms(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        // Its icon, for the installed list (M46.2).
        let icon = repo::icon_url(&index_url, entry).map_or_else(String::new, String::from);
        if let Err(error) = raygun_db::set_local_value(self.db(), &icon_key(&entry.id), &icon).await
        {
            tracing::debug!(%error, "could not remember an extension's icon");
        }
        tracing::info!(extension = %entry.id, version = %entry.version, repo = %repo.url, "extension installed");
        Ok(self.reload().await)
    }

    /// Forgets what an extension stored and its cookies (M46.11), then reloads every extension
    /// so it starts afresh.
    pub async fn clear_data(&self, extension_id: &str) -> Result<Vec<ExtensionInfo>, String> {
        let _changing = self.changing.lock().await;
        let (storage_dir, id) = (self.storage_dir.clone(), extension_id.to_owned());
        tauri::async_runtime::spawn_blocking(move || package::clear_data(&storage_dir, &id))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| format!("could not clear its data: {error}"))?;
        tracing::info!(extension = %extension_id, "extension data cleared");
        Ok(self.reload().await)
    }

    /// The site an extension's sources read, when it names one (M46.11).
    pub async fn website(&self, extension_id: &str) -> Option<String> {
        let _ready = self.loading.read().await;
        self.host
            .list()
            .await
            .into_iter()
            .find(|info| info.id == extension_id)?
            .website
    }

    /// Removes an installed extension and its storage. A built-in one it overrode comes back.
    pub async fn uninstall(&self, extension_id: &str) -> Result<Vec<ExtensionInfo>, String> {
        let _changing = self.changing.lock().await;
        let installed = self
            .list()
            .await
            .into_iter()
            .any(|info| info.id == extension_id && info.origin == ExtensionOrigin::Installed);
        if !installed {
            return Err(format!("{extension_id} is not an installed extension"));
        }
        let (extensions_dir, storage_dir, id) = (
            self.extensions_dir.clone(),
            self.storage_dir.clone(),
            extension_id.to_owned(),
        );
        tauri::async_runtime::spawn_blocking(move || {
            package::uninstall(&extensions_dir, &storage_dir, &id)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("could not uninstall: {error}"))?;
        store::remove_install(self.db(), extension_id)
            .await
            .map_err(|error| error.to_string())?;
        tracing::info!(extension = %extension_id, "extension uninstalled");
        Ok(self.reload().await)
    }

    // ---- Settings ----

    pub async fn set_setting(
        &self,
        extension_id: &str,
        key: &str,
        value: SettingValue,
    ) -> Result<Vec<ExtensionInfo>, String> {
        let infos = self.list().await;
        let setting = infos
            .iter()
            .find(|info| info.id == extension_id)
            .and_then(|info| info.settings.iter().find(|setting| setting.key == key))
            .ok_or_else(|| format!("{extension_id} has no setting {key:?}"))?;
        if !accepts(setting, &value) {
            return Err(format!("not a valid value for {}", setting.label));
        }
        store::put_setting(self.db(), extension_id, key, &value.to_json().to_string())
            .await
            .map_err(|error| error.to_string())?;
        Ok(self.reload().await)
    }
}

fn to_repo(row: store::RepoRow) -> ExtensionRepo {
    ExtensionRepo {
        id: row.id,
        url: row.url,
        name: row.name,
    }
}

/// The suggested repositories' index URLs (M46.17: the test sources add them all).
pub fn suggested_repo_urls() -> Vec<&'static str> {
    SUGGESTED_REPOS.iter().map(|(url, _, _)| *url).collect()
}

/// Raygun's repository, Paraline: Raygun's extensions, community sources and fixed copies of
/// novel sources, in one index. Every new install starts with it.
const PARALINE_URL: &str =
    "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/index.json";
const PARALINE_NAME: &str = "Paraline";
/// Paraline's row, under a fixed id like the other seeds, so every device's copy is one row.
const PARALINE_ID: &str = "0192a000-0000-7000-8000-000000000002";

/// Repositories offered with one tap: Raygun's own and the ecosystems Raygun can run.
const SUGGESTED_REPOS: [(&str, &str, &str); 3] = [
    (
        PARALINE_URL,
        PARALINE_NAME,
        "Raygun's extensions, community sources and fixed copies of novel sources.",
    ),
    (
        "https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json",
        raygun_ext_host::repo::SHOSETSU_REPO_NAME,
        "Novel sources made for the Shosetsu app.",
    ),
    (
        "https://cdn.jsdelivr.net/gh/NuclearPlayer/plugin-registry@master/plugins.json",
        "Nuclear plugins",
        "Music plugins made for the Nuclear player, converted when installed.",
    ),
];

/// The indexes Paraline replaced, still published for Raygun 1.0.0: Raygun's own
/// (`repo/`, whose packages Paraline lists) and Paraline Raygun's novel and community ones. A
/// device that lists one gets Paraline instead, on start and when one is added again.
const RETIRED_REPOS: [&str; 3] = [
    "https://raw.githubusercontent.com/uvxszi/raygun/main/repo/index.json",
    "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/shosetsu/index.json",
    "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/raygun/index.json",
];

/// The repository to add for an address: Paraline for one it replaced, else the address itself.
fn current_repo_url(url: reqwest::Url) -> reqwest::Url {
    if RETIRED_REPOS
        .iter()
        .any(|retired| same_url(url.as_str(), retired))
    {
        PARALINE_URL.parse().expect("Paraline's address is valid")
    } else {
        url
    }
}

/// What to call a repository: its index's own name; for an index without one (Shosetsu's), the
/// suggested name of its address, else a name from the address (`repo::default_name`).
fn repo_name(index: &RepoIndex, url: &str) -> String {
    if !index.name.is_empty() {
        return index.name.clone();
    }
    SUGGESTED_REPOS
        .iter()
        .find(|(suggested, _, _)| same_url(url, suggested))
        .map(|(_, name, _)| (*name).to_owned())
        .or_else(|| url.parse().ok().map(|url| repo::default_name(&url)))
        .unwrap_or_else(|| url.to_owned())
}

/// Whether two repository URLs point at the same index (case and a trailing slash aside).
fn same_url(a: &str, b: &str) -> bool {
    a.trim()
        .trim_end_matches('/')
        .eq_ignore_ascii_case(b.trim().trim_end_matches('/'))
}

/// Where an installed extension's icon address is kept.
fn icon_key(extension_id: &str) -> String {
    format!("extension.icon.{extension_id}")
}

/// What the catalogue needs to know about an installed extension.
#[derive(Debug, Clone, Default)]
struct Installed {
    version: String,
    /// A Nuclear plugin converted by an older converter: installing it again brings the fixes
    /// in the shim (M46.9.1).
    stale_conversion: bool,
}

fn catalog_entry(repo_id: &str, entry: &RepoEntry, local: Option<&Installed>) -> CatalogEntry {
    let state = match local {
        None => CatalogState::Available,
        Some(installed) => match repo::compare_versions(&entry.version, &installed.version) {
            std::cmp::Ordering::Greater => CatalogState::UpdateAvailable,
            std::cmp::Ordering::Equal
                if installed.stale_conversion && entry.format == PackageFormat::Nuclear =>
            {
                CatalogState::UpdateAvailable
            }
            std::cmp::Ordering::Equal => CatalogState::Installed,
            std::cmp::Ordering::Less => CatalogState::NewerInstalled,
        },
    };
    CatalogEntry {
        repo_id: repo_id.to_owned(),
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        description: entry.description.clone(),
        author: entry.author.clone(),
        provides: entry.provides.clone(),
        net: entry.permissions.net.clone(),
        content_rating: entry.content_rating.clone(),
        lang: entry.lang.clone(),
        size: u32::try_from(entry.size).unwrap_or(u32::MAX),
        nuclear: entry.format == PackageFormat::Nuclear,
        state,
        installed_version: local.map(|installed| installed.version.clone()),
        icon: None,
    }
}

/// Whether `value` fits a setting: the right type, and one of the options for a select.
fn accepts(setting: &ExtensionSetting, value: &SettingValue) -> bool {
    match (setting.kind, value) {
        (SettingKind::Text, SettingValue::Text(_))
        | (SettingKind::Boolean, SettingValue::Boolean(_)) => true,
        (SettingKind::Number, SettingValue::Number(number)) => number.is_finite(),
        (SettingKind::Select, SettingValue::Text(choice)) => setting.options.contains(choice),
        _ => false,
    }
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// Path, size and modification time of every file under `dir`, skipping dot-folders (storage and
/// staging copies) and `node_modules`.
pub fn fingerprint(dir: &Path) -> Fingerprint {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                files.push((entry.path(), metadata.len(), metadata.modified().ok()));
            }
        }
    }
    files.sort();
    files
}

/// A converted manifest (JSON) with `version` set.
fn with_version(manifest: &str, version: &str) -> Result<String, String> {
    let mut value: serde_json::Value =
        serde_json::from_str(manifest).map_err(|error| error.to_string())?;
    value["version"] = serde_json::Value::String(version.to_owned());
    Ok(value.to_string())
}

/// Releases asked of each source for an artist's discography (M46.36).
const RELEASES_LIMIT: u32 = 100;

/// An artist's releases from every source (M46.36): only those credited to `name`, one per title
/// and kind, editions ("(Deluxe Edition)") folded into the plain title. The copy kept is the plain
/// one, then one with artwork, a year, a kind, the most songs; its year is the earliest. Newest
/// first.
fn releases_of(found: Vec<raygun_model::MusicAlbum>, name: &str) -> Vec<raygun_model::MusicAlbum> {
    let mut kept: Vec<raygun_model::MusicAlbum> = Vec::new();
    for album in found {
        if let Some(artist) = &album.artist
            && !crate::search::credits_include(artist, name)
        {
            continue;
        }
        let key = raygun_resolver::core_title(&album.title);
        let fuller = |a: &raygun_model::MusicAlbum| {
            (
                raygun_resolver::normalize(&a.title) == raygun_resolver::core_title(&a.title),
                a.artwork_url.is_some(),
                a.year.is_some(),
                a.kind.is_some(),
                a.track_count.unwrap_or(0),
            )
        };
        match kept.iter_mut().find(|other| {
            raygun_resolver::core_title(&other.title) == key
                && (other.kind == album.kind || other.kind.is_none() || album.kind.is_none())
        }) {
            Some(other) => {
                let year = match (other.year, album.year) {
                    (Some(one), Some(two)) => Some(one.min(two)),
                    (one, two) => one.or(two),
                };
                if fuller(&album) > fuller(other) {
                    let kind = album.kind.or(other.kind);
                    *other = raygun_model::MusicAlbum {
                        year,
                        kind,
                        ..album
                    };
                } else {
                    other.year = year;
                    other.kind = other.kind.or(album.kind);
                }
            }
            None => kept.push(album),
        }
    }
    // Newest first; undated last, in the sources' order.
    kept.sort_by_key(|album| std::cmp::Reverse(album.year.unwrap_or(0)));
    kept
}

/// The album hit that is `album` (M46.36): the same core title, by the artist when one is named;
/// among several, the one with artwork.
fn best_album(
    found: Vec<raygun_model::MusicAlbum>,
    album: &str,
    artist: Option<&str>,
) -> Option<raygun_model::MusicAlbum> {
    let wanted = raygun_resolver::core_title(album);
    if wanted.is_empty() {
        return None;
    }
    let by_artist = |candidate: &raygun_model::MusicAlbum| match (artist, &candidate.artist) {
        (Some(artist), Some(credited)) if !artist.trim().is_empty() => {
            crate::search::credits_include(credited, artist)
                || crate::search::credits_include(artist, credited)
        }
        _ => true,
    };
    let same_title: Vec<_> = found
        .into_iter()
        .filter(|candidate| raygun_resolver::core_title(&candidate.title) == wanted)
        .collect();
    // A song on a compilation is credited to its artist, the album to "Various Artists": then the
    // album's exact title will do.
    let compilation = |candidate: &raygun_model::MusicAlbum| {
        candidate.artist.as_deref().is_some_and(|credit| {
            matches!(
                raygun_resolver::normalize(credit).as_str(),
                "various artists" | "various" | "va"
            )
        }) && raygun_resolver::normalize(&candidate.title) == raygun_resolver::normalize(album)
    };
    let mut matches: Vec<_> = same_title
        .iter()
        .filter(|c| by_artist(c))
        .cloned()
        .collect();
    if matches.is_empty() {
        matches = same_title.into_iter().filter(compilation).collect();
    }
    matches.sort_by_key(|candidate| candidate.artwork_url.is_none());
    matches.into_iter().next()
}

/// Results asked of each source when looking for covers (M46.36), and covers offered at most.
const COVER_SEARCH_LIMIT: u32 = 10;
const COVER_OPTIONS: usize = 24;

/// Whether a source's credit is the wanted artist's (either way round); no wanted artist agrees.
fn credited(credit: Option<&str>, wanted: Option<&str>) -> bool {
    match (credit, wanted) {
        (Some(credit), Some(wanted)) if !wanted.trim().is_empty() => {
            crate::search::credits_include(credit, wanted)
                || crate::search::credits_include(wanted, credit)
        }
        _ => true,
    }
}

/// The covers of the album hits that are `album` by `artist` (M46.36).
fn album_covers(
    found: Vec<raygun_model::MusicAlbum>,
    album: &str,
    artist: Option<&str>,
) -> Vec<raygun_model::CoverOption> {
    let wanted = raygun_resolver::core_title(album);
    found
        .into_iter()
        .filter(|candidate| raygun_resolver::core_title(&candidate.title) == wanted)
        .filter(|candidate| credited(candidate.artist.as_deref(), artist))
        .filter_map(|candidate| {
            Some(raygun_model::CoverOption {
                url: candidate.artwork_url?,
                source: candidate.provider.name,
                of: Some(candidate.title),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    #[test]
    fn album_covers_need_the_title_and_artist() {
        let mut right = album("deezer", "Discovery", "Daft Punk", None);
        right.artwork_url = Some("https://example.com/a.jpg".into());
        let mut wrong = album("deezer", "Discovery", "Someone Else", None);
        wrong.artwork_url = Some("https://example.com/b.jpg".into());
        let covers = album_covers(vec![right, wrong], "Discovery", Some("Daft Punk"));
        assert_eq!(covers.len(), 1);
        assert_eq!(covers[0].url, "https://example.com/a.jpg");
    }

    fn album(
        provider: &str,
        title: &str,
        artist: &str,
        year: Option<u32>,
    ) -> raygun_model::MusicAlbum {
        raygun_model::MusicAlbum {
            provider: raygun_model::ProviderInfo {
                id: provider.into(),
                extension_id: provider.into(),
                name: provider.into(),
                kind: "music.metadata".into(),
                search_pill: None,
            },
            id: title.into(),
            title: title.into(),
            artist: Some(artist.into()),
            year,
            artwork_url: None,
            track_count: None,
            kind: None,
        }
    }

    #[test]
    fn releases_keep_the_artist_once_each_newest_first() {
        let mut deezer = album("deezer", "Discovery", "Daft Punk", None);
        deezer.artwork_url = Some("https://example.com/d.jpg".into());
        deezer.kind = Some(raygun_model::ReleaseKind::Album);
        let mut deluxe = album(
            "deezer",
            "Homework (25th Anniversary Edition)",
            "Daft Punk",
            Some(2022),
        );
        deluxe.artwork_url = Some("https://example.com/h.jpg".into());
        let found = vec![
            deluxe,
            album("mb", "Homework", "Daft Punk", Some(1997)),
            album("mb", "Discovery", "Daft Punk", Some(2001)),
            deezer,
            album("mb", "Something Else", "Another Band", Some(2020)),
        ];
        let releases = releases_of(found, "Daft Punk");
        let titles: Vec<_> = releases.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(titles, ["Discovery", "Homework"]);
        // The copy with artwork wins and keeps the other's year.
        assert_eq!(releases[0].provider.id, "deezer");
        assert_eq!(releases[0].year, Some(2001));
        assert_eq!(releases[0].kind, Some(raygun_model::ReleaseKind::Album));
        // The plain title stands for its editions, with the first year.
        assert_eq!(releases[1].title, "Homework");
        assert_eq!(releases[1].year, Some(1997));
    }

    #[test]
    fn the_album_of_a_song_needs_its_title_and_artist() {
        let found = vec![
            album("mb", "Discovery (Remastered)", "Someone Else", None),
            album("mb", "Discovery", "Daft Punk", Some(2001)),
        ];
        let best = best_album(found.clone(), "Discovery", Some("Daft Punk")).unwrap();
        assert_eq!(best.artist.as_deref(), Some("Daft Punk"));
        assert!(best_album(found, "Homework", Some("Daft Punk")).is_none());
        // A compilation credits "Various Artists".
        let various = vec![album(
            "dz",
            "Alternative Times, Volume 111",
            "Various Artists",
            None,
        )];
        assert!(
            best_album(
                various,
                "Alternative Times, Volume 111",
                Some("Julian Casablancas")
            )
            .is_some()
        );
    }

    use super::*;

    #[test]
    fn fingerprint_tracks_extension_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let empty = fingerprint(root);
        assert!(empty.is_empty());

        std::fs::create_dir_all(root.join("dev.example.a/dist")).unwrap();
        std::fs::write(root.join("dev.example.a/dist/index.js"), "1").unwrap();
        let one = fingerprint(root);
        assert_eq!(one.len(), 1);

        // Storage writes and staging copies don't count as changes.
        std::fs::create_dir_all(root.join(".data/dev.example.a")).unwrap();
        std::fs::write(root.join(".data/dev.example.a/storage.json"), "{}").unwrap();
        std::fs::create_dir_all(root.join(".staging-dev.example.a")).unwrap();
        std::fs::write(root.join(".staging-dev.example.a/x.js"), "x").unwrap();
        assert_eq!(fingerprint(root), one);

        std::fs::write(root.join("dev.example.a/dist/index.js"), "12").unwrap();
        assert_ne!(fingerprint(root), one);
    }

    #[test]
    fn built_in_extensions_parse() {
        for source in built_in() {
            let raygun_ext_host::ExtensionFiles::Memory(files) = &source.files else {
                panic!("built-in extensions live in memory");
            };
            let manifest =
                raygun_ext_host::Manifest::parse(&files["raygun-extension.json"]).unwrap();
            assert!(
                files.contains_key(&manifest.entry),
                "{} has no entry",
                manifest.id
            );
        }
    }

    fn entry(version: &str) -> RepoEntry {
        RepoEntry {
            id: "dev.example.a".into(),
            name: "A".into(),
            version: version.into(),
            api_version: 1,
            description: None,
            author: None,
            provides: vec![],
            permissions: Default::default(),
            content_rating: "safe".into(),
            lang: None,
            package: "a.zip".into(),
            icon: None,
            sha256: "0".repeat(64),
            size: 1,
            format: Default::default(),
        }
    }

    #[test]
    fn suggested_repos_match_added_urls() {
        assert!(same_url(
            "https://Example.com/index.json/",
            "https://example.com/index.json"
        ));
        assert!(!same_url(
            "https://a.example/index.json",
            "https://b.example/index.json"
        ));
        assert!(
            SUGGESTED_REPOS
                .iter()
                .all(|(url, _, _)| url.starts_with("https://"))
        );
    }

    #[test]
    fn paraline_replaces_the_indexes_it_took_over() {
        let names: Vec<_> = SUGGESTED_REPOS.iter().map(|(_, name, _)| *name).collect();
        assert_eq!(
            names,
            ["Paraline", "Shosetsu extensions", "Nuclear plugins"]
        );
        for (url, _, _) in SUGGESTED_REPOS {
            assert!(!RETIRED_REPOS.iter().any(|retired| same_url(url, retired)));
        }
        for retired in RETIRED_REPOS {
            for spelling in [retired.to_owned(), format!("{}/", retired.to_uppercase())] {
                let url = repo::check_index_url(&spelling).unwrap();
                assert_eq!(current_repo_url(url).as_str(), PARALINE_URL, "{spelling}");
            }
        }
        let other = repo::check_index_url("https://example.org/index.json").unwrap();
        assert_eq!(current_repo_url(other.clone()), other);
    }

    #[test]
    fn repositories_without_a_name_are_named_by_their_address() {
        let index = |name: &str| RepoIndex {
            format_version: 1,
            name: name.to_owned(),
            extensions: Vec::new(),
            libraries: Vec::new(),
        };
        let shosetsu = "https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json";
        assert_eq!(repo_name(&index(""), shosetsu), "Shosetsu extensions");
        // Another Shosetsu repository isn't presented as Shosetsu's.
        assert_eq!(
            repo_name(
                &index(""),
                "https://raw.githubusercontent.com/someone/novel-sources/main/index.json"
            ),
            "novel-sources"
        );
        assert_eq!(repo_name(&index("Paraline"), PARALINE_URL), "Paraline");
    }

    #[test]
    fn converted_plugins_take_the_repository_version() {
        let manifest = r#"{"id":"nuclear.x","version":"1.0.0","provides":["music.stream"]}"#;
        let updated: serde_json::Value =
            serde_json::from_str(&super::with_version(manifest, "1.0.1").unwrap()).unwrap();
        assert_eq!(updated["version"], "1.0.1");
        assert_eq!(updated["provides"][0], "music.stream");
    }

    #[test]
    fn catalog_state_compares_versions() {
        let state = |repo: &str, local: Option<&str>| {
            let installed = local.map(|version| Installed {
                version: version.to_owned(),
                stale_conversion: false,
            });
            catalog_entry("r", &entry(repo), installed.as_ref()).state
        };
        assert_eq!(state("1.0.0", None), CatalogState::Available);
        assert_eq!(state("1.0.0", Some("1.0.0")), CatalogState::Installed);
        assert_eq!(
            state("1.10.0", Some("1.9.0")),
            CatalogState::UpdateAvailable
        );
        assert_eq!(
            state("1.0.0", Some("1.0.1-dev")),
            CatalogState::NewerInstalled
        );

        // A Nuclear plugin converted by an older converter is offered again; other kinds aren't.
        let stale = Installed {
            version: "1.0.0".into(),
            stale_conversion: true,
        };
        let nuclear = RepoEntry {
            format: PackageFormat::Nuclear,
            ..entry("1.0.0")
        };
        assert_eq!(
            catalog_entry("r", &nuclear, Some(&stale)).state,
            CatalogState::UpdateAvailable
        );
        assert_eq!(
            catalog_entry("r", &entry("1.0.0"), Some(&stale)).state,
            CatalogState::Installed
        );
    }

    #[test]
    fn setting_values_must_fit_their_kind() {
        let setting = |kind, options: &[&str]| ExtensionSetting {
            key: "k".into(),
            label: "K".into(),
            kind,
            options: options.iter().map(|option| (*option).to_owned()).collect(),
            default: None,
            value: None,
        };
        let text = |value: &str| SettingValue::Text(value.into());
        assert!(accepts(&setting(SettingKind::Text, &[]), &text("x")));
        assert!(!accepts(
            &setting(SettingKind::Text, &[]),
            &SettingValue::Number(1.0)
        ));
        assert!(accepts(
            &setting(SettingKind::Number, &[]),
            &SettingValue::Number(2.5)
        ));
        assert!(!accepts(
            &setting(SettingKind::Number, &[]),
            &SettingValue::Number(f64::NAN)
        ));
        assert!(accepts(
            &setting(SettingKind::Boolean, &[]),
            &SettingValue::Boolean(true)
        ));
        assert!(accepts(
            &setting(SettingKind::Select, &["eu", "us"]),
            &text("us")
        ));
        assert!(!accepts(
            &setting(SettingKind::Select, &["eu", "us"]),
            &text("asia")
        ));
    }
}
