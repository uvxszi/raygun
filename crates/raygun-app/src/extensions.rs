//! Extensions: the ones installed under `extensions/` (M11), from repositories with per-extension
//! settings (M13). Raygun ships none (v1.3.0); Paraline is offered as a suggestion.
//!
//! Debug builds also watch that folder and reload when it changes (M12), so `raygun-ext dev`
//! reaches the running app without a restart.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

use raygun_db::extensions::{self as store, InstallRow};
use raygun_ext_host::mihon;
use raygun_ext_host::nuclear;
use raygun_ext_host::package::{self, MAX_PACKAGE_BYTES};

/// The largest Mihon APK Raygun downloads (Keiyoushi's are mostly under 1 MB).
const MAX_APK_BYTES: u64 = 40 * 1024 * 1024;
use raygun_ext_host::repo::{self, PackageFormat, RepoEntry, RepoIndex};
use raygun_ext_host::shosetsu;
use raygun_ext_host::{
    ExtensionHost, HostConfig, ProviderCandidates, ResolvedStream, UserSettings, discover_installed,
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

mod catalog;

const SEARCH_LIMIT: u32 = 30;
/// Candidates asked of each `music.stream` provider per track.
const CANDIDATE_LIMIT: u32 = 10;
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
const INDEX_CACHE: Duration = Duration::from_secs(10 * 60);
/// An index read this recently counts as current for installing (a queue installs many
/// extensions from one repository, and a large index takes seconds to read and parse).
const FRESH_INDEX: Duration = Duration::from_secs(30);
/// How long a built listing is kept while nothing it depends on changes (`catalog::Listings`).
const LISTING_CACHE: Duration = Duration::from_secs(2 * 60);
/// Where installed extensions' icon addresses are kept (`local_value`), one key each.
const ICON_PREFIX: &str = "extension.icon.";
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);
/// Where the reasons Mihon extensions couldn't be converted are kept (`local_value`).
const MIHON_VERDICTS: &str = "extensions.mihonVerdicts";

/// Why a Mihon extension couldn't be converted, for the version and converter that tried.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct MihonVerdict {
    version: String,
    converter: u32,
    reason: String,
}

impl MihonVerdict {
    /// What the catalogue says of an entry this verdict is about, while it still holds: the same
    /// version, tried by this converter.
    fn unsupported(&self, entry: &RepoEntry) -> Option<String> {
        (self.version == entry.version && self.converter == mihon::CONVERTER_VERSION)
            .then(|| format!("Raygun can't convert it: {}", self.reason))
    }
}

/// Raygun's first-party extensions, read from their committed bundles: the tests check they load.
#[cfg(test)]
fn first_party() -> Vec<raygun_ext_host::ExtensionSource> {
    use raygun_ext_host::ExtensionSource;
    vec![
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
        // Public-domain and openly licensed recordings and videos.
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/wikimedia-commons/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/wikimedia-commons/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/nasa/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/nasa/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/blender-movies/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/blender-movies/dist/index.js"),
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
        // More public-domain books, in edited editions and proofread transcriptions.
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/standard-ebooks/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/standard-ebooks/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/wikisource/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/wikisource/dist/index.js"),
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
    /// Song credits already asked for (M46.14); `None` = nobody knew.
    info_cache: Mutex<InfoCache>,
    /// Held for writing while extensions (re)load, so calls wait instead of seeing a partial set.
    loading: Arc<RwLock<()>>,
    /// One install or uninstall at a time.
    changing: tokio::sync::Mutex<()>,
    /// The installed files as of the last load, so the dev watcher skips the app's own changes.
    loaded_files: Mutex<Fingerprint>,
    indexes: Mutex<catalog::Indexes>,
    /// Each repository's catalogue listing as last built (`catalog`).
    listings: Mutex<catalog::Listings>,
    /// The converter each installed Mihon extension was made by, by id: its version and the
    /// converter, read from its manifest once per load.
    mihon_converters: Mutex<HashMap<String, (String, Option<u64>)>>,
    /// Counts changes to the 18+ switch and marks, which listings depend on.
    content_changes: AtomicU64,
    /// Mihon stores read with their index, by the repository's address: where the extensions
    /// came from and what the store says of each (its sources, where its code is).
    mihon_stores: Mutex<HashMap<String, (reqwest::Url, Arc<mihon::Store>)>>,
}

/// What `music.info` providers answered this run about songs (M46.14).
#[derive(Default)]
struct InfoCache {
    credits: HashMap<String, Option<raygun_model::SongCredits>>,
}

impl ExtensionService {
    /// Starts the host and loads extensions in the background.
    pub fn start(app: AppHandle, paths: &Paths) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let extensions_dir = paths.extensions();
        let storage_dir = extensions_dir.join(".data");
        // Android reads repositories from the copies of picked folders (M46.52).
        repo::allow_local_files_in(crate::picker::picked_dir(paths));
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
            indexes: Mutex::new(catalog::Indexes::default()),
            listings: Mutex::new(catalog::Listings::default()),
            mihon_converters: Mutex::new(HashMap::new()),
            content_changes: AtomicU64::new(0),
            mihon_stores: Mutex::new(HashMap::new()),
            info_cache: Mutex::new(InfoCache::default()),
        });
        // Pages whose pictures their source draws (converted Mihon sources) are fetched
        // through it.
        let pictures = Arc::downgrade(&service);
        raygun_net::images::set_source_images(Arc::new(move |provider, url, headers| {
            let pictures = pictures.clone();
            Box::pin(async move {
                let Some(service) = pictures.upgrade() else {
                    return Err(raygun_net::images::ImageError::Network(
                        "extensions have stopped".into(),
                    ));
                };
                let (_ready, host) = service.ready().await;
                host.manga_image(&provider, &url, &headers)
                    .await
                    .map_err(raygun_net::images::ImageError::Network)
            })
        }));
        // Which sources need a key or sign-in: told to the UI after every load and whenever a
        // source says its key was refused.
        let announcer = Arc::downgrade(&service);
        let mut setup_changes = service.host.setup_changes();
        tauri::async_runtime::spawn(async move {
            while setup_changes.changed().await.is_ok() {
                let Some(service) = announcer.upgrade() else {
                    break;
                };
                service.announce_setup();
            }
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
                    loader
                        .host
                        .content()
                        .set_overrides(settings.content_overrides);
                }
                Err(error) => tracing::warn!(%error, "could not read the NSFW setting"),
            }
            loader.load_all().await;
            drop(guard);
            loader.move_former_built_ins().await;
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
    /// Each also names the repository it was installed from.
    async fn with_icons(&self, infos: Vec<ExtensionInfo>) -> Vec<ExtensionInfo> {
        let images = self.app.state::<raygun_net::ImageLinks>();
        let installs = store::installs(self.db()).await.unwrap_or_default();
        let repos = self.repos().await.unwrap_or_default();
        // Every icon in one query, and every repository's name at once: hundreds of extensions
        // took a query and a search through the installs each.
        let icons = raygun_db::local_values_with_prefix(self.db(), ICON_PREFIX)
            .await
            .unwrap_or_default();
        let repositories = repository_names(&installs, &repos);
        infos
            .into_iter()
            .map(|info| {
                let icon = icons
                    .get(&icon_key(&info.id))
                    .filter(|url| images.is_remote(url))
                    .map(|url| images.kept_cover(url, &Default::default()));
                let repository = repositories.get(&info.id).cloned();
                ExtensionInfo {
                    icon,
                    repository,
                    ..info
                }
            })
            .collect()
    }

    /// The sources that need a key or sign-in before they're asked anything (turned off and
    /// hidden 18+ ones left out), once extensions have loaded.
    pub async fn sources_needing_setup(&self) -> Vec<raygun_model::SourceSetup> {
        let _ready = self.loading.read().await;
        self.host.sources_needing_setup()
    }

    /// Tells the UI which sources need a key or sign-in now.
    fn announce_setup(&self) {
        let waiting = self.host.sources_needing_setup();
        if let Err(error) = crate::events::SourcesSetupEvent(waiting).emit(&self.app) {
            tracing::warn!(%error, "could not announce sources that need setup");
        }
    }

    /// Whether NSFW extensions and content are shown (M35).
    /// The sources and providers the user turned off (M46.8): searches and lookups skip them.
    pub fn set_disabled_sources(&self, ids: &[String]) {
        self.host.content().set_disabled(ids);
        self.announce_setup();
    }

    pub fn show_nsfw(&self) -> bool {
        self.host.content().show_nsfw()
    }

    /// Turns NSFW content and explicit music (M46.36) on or off and tells the UI, whose source
    /// lists depend on them.
    pub async fn set_content(&self, show_nsfw: bool, explicit_music: bool) {
        self.host.content().set_show_nsfw(show_nsfw);
        self.host.content().set_show_explicit_music(explicit_music);
        self.content_changes.fetch_add(1, Ordering::Relaxed);
        let infos = self.list().await;
        if let Err(error) = ExtensionsChangedEvent(infos).emit(&self.app) {
            tracing::warn!(%error, "could not announce extension changes");
        }
        self.announce_setup();
    }

    /// Puts the user's 18+ marks into effect at once and tells the UI, whose source lists and
    /// 18+ tags depend on them.
    pub async fn set_content_overrides(&self, overrides: raygun_model::ContentOverrides) {
        self.host.content().set_overrides(overrides);
        self.content_changes.fetch_add(1, Ordering::Relaxed);
        let infos = self.list().await;
        if let Err(error) = ExtensionsChangedEvent(infos).emit(&self.app) {
            tracing::warn!(%error, "could not announce extension changes");
        }
        self.announce_setup();
    }

    /// Whether a Stremio addon (by its stored id) that says `own` counts as 18+, the user's
    /// mark deciding over it.
    pub fn addon_adult(&self, addon_id: &str, own: bool) -> bool {
        self.host.content().addon_adult(addon_id, own)
    }

    /// Tells the content filter which repository each installed extension came from, so a
    /// repository's mark reaches them.
    async fn learn_extension_repositories(&self) {
        let (installs, repos) = match (store::installs(self.db()).await, self.repos().await) {
            (Ok(installs), Ok(repos)) => (installs, repos),
            (Err(error), _) => {
                tracing::warn!(%error, "could not read where extensions came from");
                return;
            }
            (_, Err(error)) => {
                tracing::warn!(%error, "could not read where extensions came from");
                return;
            }
        };
        self.host
            .content()
            .set_extension_repositories(extension_repositories(&installs, &repos));
    }

    pub async fn search(&self, query: &str) -> Vec<ProviderSearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let _ready = self.loading.read().await;
        self.host.search_music(query, SEARCH_LIMIT).await
    }

    /// [`Self::search`] in one extension's catalogue only (the source a song streamed from).
    pub async fn search_of(&self, extension_id: &str, query: &str) -> Vec<ProviderSearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let _ready = self.loading.read().await;
        self.host
            .search_music_of(extension_id, query, SEARCH_LIMIT)
            .await
    }

    /// [`Self::search`] for Search itself: each source's answer goes to `report` as it comes,
    /// with how many sources were asked; the answers come back in that order. A source whose
    /// search fails or finds nothing answers from its charts, as `listings` allows.
    pub async fn search_as_answered(
        &self,
        query: &str,
        listings: raygun_model::SearchListingsPrefs,
        report: impl Fn(&ProviderSearchResult, usize),
    ) -> Vec<ProviderSearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let _ready = self.loading.read().await;
        self.host
            .search_music_as_answered(query, SEARCH_LIMIT, listings, report)
            .await
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

    /// Artists for a search, from every source that searches artists (M46.9); each source's go
    /// to `report` as they come, with how many sources were asked.
    pub async fn search_artists(
        &self,
        query: &str,
        report: impl Fn(&ProviderInfo, &[raygun_model::MusicArtist], usize),
    ) -> Vec<raygun_model::MusicArtist> {
        let _ready = self.loading.read().await;
        self.host
            .search_artists_as_answered(query, SEARCH_LIMIT, report)
            .await
    }

    /// Albums for a search, from every source that searches albums (M46.9); each source's go to
    /// `report` as they come, with how many sources were asked.
    pub async fn search_albums(
        &self,
        query: &str,
        report: impl Fn(&ProviderInfo, &[raygun_model::MusicAlbum], usize),
    ) -> Vec<raygun_model::MusicAlbum> {
        let _ready = self.loading.read().await;
        self.host
            .search_albums_as_answered(query, SEARCH_LIMIT, report)
            .await
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
        self.cover_options_answered(target).await.0
    }

    /// [`Self::cover_options`], and whether a source answered: for a song, a search that failed
    /// at every source (offline, say) found nothing without saying there is nothing.
    pub async fn cover_options_answered(
        &self,
        target: &raygun_model::DetailTarget,
    ) -> (Vec<raygun_model::CoverOption>, bool) {
        use raygun_model::DetailTarget;
        let _ready = self.loading.read().await;
        let mut options = Vec::new();
        let mut answered = true;
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
                answered = songs.iter().any(|result| result.error.is_none()) || !albums.is_empty();
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
        (options, answered)
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

    /// Playlists for a search from every source that searches them (M46.39); each source's go
    /// to `report` as they come, with how many sources were asked.
    pub async fn search_playlists(
        &self,
        query: &str,
        featured: bool,
        report: impl Fn(&ProviderInfo, &[raygun_model::MusicPlaylist], usize),
    ) -> Vec<raygun_model::MusicPlaylist> {
        let _ready = self.loading.read().await;
        self.host
            .search_playlists_as_answered(query, featured, SEARCH_LIMIT, report)
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
    /// What open databases say about an artist (M46.14), asked now: the artist's page keeps it
    /// (`artist_profiles`).
    pub async fn artist_info(&self, name: &str) -> Option<raygun_model::ArtistInfo> {
        let (_ready, host) = self.ready().await;
        host.artist_info(name.trim()).await
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

    /// The stream behind a candidate that came without one, for `using` (M46.51).
    pub async fn resolve_stream(
        &self,
        provider_id: &str,
        candidate_id: &str,
        using: raygun_ext_host::StreamUse,
    ) -> Result<ResolvedStream, String> {
        self.resolve_stream_call(provider_id, candidate_id, using)
            .await
            .map_err(|error| error.to_string())
    }

    /// [`Self::resolve_stream`] with the call's own error, which says what kind of failure it was
    /// (a source busy now, or a stream that's gone). Answers and the site's own trouble count in
    /// the sources' health, as fan-outs' calls do; one song a source doesn't have doesn't.
    pub async fn resolve_stream_call(
        &self,
        provider_id: &str,
        candidate_id: &str,
        using: raygun_ext_host::StreamUse,
    ) -> Result<ResolvedStream, raygun_ext_host::CallError> {
        let _ready = self.loading.read().await;
        let started = std::time::Instant::now();
        let answer = self
            .host
            .resolve_stream(provider_id, candidate_id, using)
            .await;
        let counts = match &answer {
            Ok(_) => true,
            Err(error) => error
                .kind()
                .is_some_and(raygun_net::IssueKind::is_transient),
        };
        if counts {
            self.host
                .health()
                .record(provider_id, answer.as_ref().map(|_| ()), started.elapsed());
        }
        answer
    }

    /// A YouTube video's own audio file from Raygun's YouTube module, asked through an installed
    /// YouTube source (`extension_id`): its files play to their end (downloads).
    pub async fn youtube_audio(
        &self,
        extension_id: &str,
        video: &str,
        using: raygun_ext_host::StreamUse,
        limit: std::time::Duration,
    ) -> Result<ResolvedStream, raygun_ext_host::CallError> {
        let _ready = self.loading.read().await;
        self.host
            .youtube_audio(extension_id, video, using, limit)
            .await
    }

    /// The stream sources searches use now: not turned off, nor left out as 18+.
    pub async fn stream_provider_ids(&self) -> std::collections::HashSet<String> {
        let _ready = self.loading.read().await;
        self.host
            .providers(raygun_ext_host::MUSIC_STREAM)
            .await
            .into_iter()
            .map(|provider| provider.id)
            .collect()
    }

    /// Whether a source sits out after failing again and again lately: it isn't asked meanwhile.
    pub fn resting(&self, provider_id: &str) -> bool {
        self.host.health().resting(provider_id).is_some()
    }

    /// Whether a provider's source needs a key or sign-in before it's asked anything: it isn't
    /// asked, and that says nothing about the song or the pick.
    pub fn needs_setup(&self, provider_id: &str) -> bool {
        self.host.provider_needs_setup(provider_id)
    }

    /// Whether any source that finds streams waits for its key or sign-in: until it's set up,
    /// it isn't known that no source has a song.
    pub async fn stream_sources_need_setup(&self) -> bool {
        let _ready = self.loading.read().await;
        self.host.sources_needing_setup().iter().any(|source| {
            source
                .kinds
                .iter()
                .any(|kind| kind == raygun_ext_host::MUSIC_STREAM)
        })
    }

    /// [`Self::stream_candidates`] asking each provider for up to `limit` candidates.
    pub async fn stream_candidates_up_to(
        &self,
        query: &TrackQuery,
        limit: u32,
        quality: raygun_resolver::QualityTarget,
    ) -> Vec<ProviderCandidates> {
        let _ready = self.loading.read().await;
        self.host.stream_candidates(query, limit, quality).await
    }

    /// On a device that ran a version carrying sources inside the app (migration 0041 marks it),
    /// installs those sources from Paraline, adding Paraline when it's missing, so nothing the
    /// user had stops working. Tried again on the next start until every one is in.
    async fn move_former_built_ins(&self) {
        let pending = raygun_db::local_value(self.db(), FORMER_BUILT_INS_KEY)
            .await
            .ok()
            .flatten()
            .is_some_and(|value| value == "pending");
        if !pending {
            return;
        }
        let repo_id = match self.paraline_id().await {
            Ok(id) => id,
            Err(error) => {
                tracing::warn!(%error, "could not add Paraline for the former built-in sources");
                return;
            }
        };
        let installed: Vec<String> = store::installs(self.db())
            .await
            .unwrap_or_default()
            .into_iter()
            .map(|install| install.extension_id)
            .collect();
        let mut missing = 0;
        for id in FORMER_BUILT_INS {
            if installed.iter().any(|done| done == id) {
                continue;
            }
            if let Err(error) = self.install_package(&repo_id, id, &|_| {}).await {
                tracing::warn!(extension = id, %error, "could not install a former built-in source");
                missing += 1;
            }
        }
        if missing == 0
            && let Err(error) =
                raygun_db::set_local_value(self.db(), FORMER_BUILT_INS_KEY, "done").await
        {
            tracing::warn!(%error, "could not note the former built-in sources as moved");
        }
        self.reload().await;
    }

    /// Paraline's repository id, adding Paraline when this device doesn't list it.
    async fn paraline_id(&self) -> Result<String, String> {
        let find = |repos: &[ExtensionRepo]| {
            repos
                .iter()
                .find(|repo| same_url(&repo.url, PARALINE_URL))
                .map(|repo| repo.id.clone())
        };
        if let Some(id) = find(&self.repos().await?) {
            return Ok(id);
        }
        let repos = self.add_repo(PARALINE_URL).await?;
        find(&repos).ok_or_else(|| "Paraline wasn't added".to_owned())
    }

    /// Reloads every installed extension from scratch.
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
        // Installs may have written new manifests, even at the same version.
        self.mihon_converters.lock().unwrap().clear();

        self.learn_extension_repositories().await;
        let sources = discover_installed(&self.extensions_dir);
        let settings = self.user_settings().await;
        // Connected servers' sign-ins, from the secrets store (`servers`).
        let (app, dir) = (self.app.clone(), self.extensions_dir.clone());
        let secrets =
            tauri::async_runtime::spawn_blocking(move || crate::servers::secrets(&app, &dir))
                .await
                .unwrap_or_default();
        let infos = self.host.load_with(sources, settings, secrets).await;
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
        let url = current_repo_url(url);
        let (url, index) = match self.index_at(url.clone(), true).await {
            Ok(found) => found,
            // Shosetsu repositories are shared by their folder or their GitHub/GitLab page.
            Err(error) => {
                let mut found = None;
                for candidate in repo::shosetsu_index_candidates(&url) {
                    if let Ok(index) = self.fetch_index(candidate.as_str(), true).await {
                        found = Some((candidate, index));
                        break;
                    }
                }
                found.ok_or(error)?
            }
        };
        let repos = self.repos().await?;
        // A project's page read at `HEAD` is the branch an added address may name: no second copy.
        if repos.iter().any(|repo| same_repo(&repo.url, url.as_str())) {
            return Ok(repos);
        }
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
        self.learn_extension_repositories().await;
        self.repos().await
    }

    /// Every repository's extensions, marked against what this device has.
    /// Well-known repositories, marked when this device already has them (M36).
    pub async fn suggested_repos(&self) -> Result<Vec<SuggestedRepo>, String> {
        let repos = self.repos().await?;
        Ok(SUGGESTED_REPOS
            .iter()
            .map(|(url, name, description)| SuggestedRepo {
                added: repos.iter().any(|repo| same_repo(&repo.url, url)),
                url: (*url).to_owned(),
                name: (*name).to_owned(),
                description: (*description).to_owned(),
            })
            .collect())
    }

    /// What a repository holds, read from its index without adding it, so the extensions it
    /// brings can be shown before it's added.
    pub async fn preview_repo(&self, url: &str) -> Result<RepoCatalog, String> {
        let url = repo::check_index_url(url).map_err(|error| error.to_string())?;
        let (url, index) = self.index_at(url, false).await?;
        Ok(self.listing_of(&url, &index).await)
    }

    /// The listing of an index read elsewhere (Sources' look at an address, M46.52), as a
    /// repository not added yet (its `id` is empty).
    pub async fn listing_of(&self, url: &reqwest::Url, index: &RepoIndex) -> RepoCatalog {
        let local = self.installed_versions().await;
        let verdicts = self.mihon_verdicts().await;
        let repo = ExtensionRepo {
            id: String::new(),
            url: url.to_string(),
            name: repo_name(index, url.as_str()),
        };
        let addons = self.added_addons().await;
        let entries = self.entries_of(&repo, index, &local, &verdicts, &addons);
        RepoCatalog {
            repo,
            entries,
            error: None,
        }
    }

    /// Whether the repository at `url` (or the one an index Paraline replaced stands for) is
    /// added.
    pub async fn has_repo(&self, url: &str) -> bool {
        let wanted = repo::check_index_url(url)
            .map(current_repo_url)
            .map_or_else(|_| url.to_owned(), |url| url.to_string());
        self.repos()
            .await
            .unwrap_or_default()
            .iter()
            .any(|repo| same_repo(&repo.url, &wanted))
    }

    /// How the catalogue shows an extension found on its own, marked against this device's.
    pub async fn entry_view(&self, entry: &RepoEntry) -> CatalogEntry {
        let local = self.installed_versions().await;
        catalog_entry("", entry, local.get(&entry.id))
    }

    /// Installs one extension found on its own (Sources → Add, M46.52) from the file read when it
    /// was found: a Raygun package, or a Miru or Shosetsu script, whose libraries come from the
    /// Shosetsu repositories added. It's recorded as installed from its own address, so no
    /// repository offers it updates. Reloads the extensions.
    pub async fn install_file(
        &self,
        url: &reqwest::Url,
        entry: &RepoEntry,
        bytes: &[u8],
    ) -> Result<Vec<ExtensionInfo>, String> {
        {
            let _changing = self.changing.lock().await;
            let verified = match entry.format {
                PackageFormat::Raygun => {
                    package::verify(bytes, entry).map_err(|error| error.to_string())?
                }
                PackageFormat::Miru => {
                    let (script, header) =
                        raygun_ext_host::miru::package::verify_script(bytes, entry)
                            .map_err(|error| error.to_string())?;
                    raygun_ext_host::miru::package::package(entry, script, &header)
                        .map_err(|error| error.to_string())?
                }
                PackageFormat::Shosetsu => {
                    let mut libraries: Vec<(reqwest::Url, Arc<RepoIndex>)> = Vec::new();
                    for repo in self.repos().await? {
                        if let (Ok(index_url), Ok(index)) = (
                            repo.url.parse::<reqwest::Url>(),
                            self.fetch_index(&repo.url, false).await,
                        ) && !index.libraries.is_empty()
                        {
                            libraries.push((index_url, index));
                        }
                    }
                    let sources: Vec<shosetsu::package::LibrarySource<'_>> = libraries
                        .iter()
                        .map(|(index_url, index)| shosetsu::package::LibrarySource {
                            index_url,
                            index,
                        })
                        .collect();
                    shosetsu::package::assemble(entry, bytes, &sources, |url| async move {
                        self.download_patiently(url.as_str(), MAX_PACKAGE_BYTES)
                            .await
                    })
                    .await?
                }
                other => {
                    return Err(format!(
                        "{other:?} extensions are installed from their repository"
                    ));
                }
            };
            let extensions_dir = self.extensions_dir.clone();
            tauri::async_runtime::spawn_blocking(move || {
                package::install(&extensions_dir, &verified)
            })
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
            store::record_install(
                self.db(),
                &InstallRow {
                    extension_id: entry.id.clone(),
                    repo_url: url.to_string(),
                    version: entry.version.clone(),
                    sha256: entry.sha256.to_ascii_lowercase(),
                    installed_at: unix_ms(),
                },
            )
            .await
            .map_err(|error| error.to_string())?;
            tracing::info!(extension = %entry.id, version = %entry.version, from = %url, "extension installed from its file");
        }
        Ok(self.reload().await)
    }

    /// The extensions on this device by id, for marking a repository's entries. Their icons and
    /// repositories (`list`) aren't needed here.
    async fn installed_versions(&self) -> HashMap<String, Installed> {
        let infos = {
            let _ready = self.loading.read().await;
            self.host.list().await
        };
        let mut installed = HashMap::new();
        for info in infos {
            // A converted Mihon extension says which converter made it in its manifest.
            let stale_mihon = info.id.starts_with("mihon.")
                && self
                    .mihon_converter(&info.id, &info.version)
                    .await
                    .is_some_and(|converter| converter < u64::from(mihon::CONVERTER_VERSION));
            let entry = Installed {
                stale_conversion: stale_mihon
                    || info.compatibility.as_ref().is_some_and(|report| {
                        report.converter < raygun_ext_host::nuclear::CONVERTER_VERSION
                    }),
                version: info.version,
            };
            installed.insert(info.id, entry);
        }
        installed
    }

    /// The addresses of the Stremio addons added, as repositories list them.
    async fn added_addons(&self) -> HashSet<String> {
        match self
            .app
            .state::<Arc<crate::stremio::StremioService>>()
            .addons()
            .await
        {
            Ok(addons) => addons.into_iter().map(|addon| addon.url).collect(),
            Err(_) => HashSet::new(),
        }
    }

    /// The address a repository lists the Stremio addon `id` at, as added addons keep it.
    async fn listed_addon_url(&self, id: &str) -> Option<String> {
        for repo in self.repos().await.ok()? {
            let Ok(index) = self.fetch_index(&repo.url, false).await else {
                continue;
            };
            let Some(entry) = index
                .extensions
                .iter()
                .find(|entry| entry.id == id && entry.format == PackageFormat::Stremio)
            else {
                continue;
            };
            let base: reqwest::Url = repo.url.parse().ok()?;
            let url = repo::package_url(&base, entry).ok()?;
            return crate::stremio::protocol::normalize_addon_url(url.as_str()).ok();
        }
        None
    }

    /// The converter an installed Mihon extension (at `version`) was made by, read from its
    /// manifest once per load: hundreds installed from a store were read on every look at the
    /// catalogue.
    async fn mihon_converter(&self, id: &str, version: &str) -> Option<u64> {
        if let Some((read, converter)) = self.mihon_converters.lock().unwrap().get(id)
            && read == version
        {
            return *converter;
        }
        let path = self
            .extensions_dir
            .join(id)
            .join(raygun_ext_host::MANIFEST_FILE);
        let converter = async {
            let text = tokio::fs::read_to_string(path).await.ok()?;
            let manifest: serde_json::Value = serde_json::from_str(&text).ok()?;
            manifest.get("mihon")?.get("converter")?.as_u64()
        }
        .await;
        self.mihon_converters
            .lock()
            .unwrap()
            .insert(id.to_owned(), (version.to_owned(), converter));
        converter
    }

    /// A repository index's entries as the catalog lists them: 18+ ones (rated `nsfw`, or marked
    /// so on the entry or the repository) only while NSFW content is shown, icons through the
    /// cover cache.
    fn entries_of(
        &self,
        repo: &ExtensionRepo,
        index: &RepoIndex,
        local: &HashMap<String, Installed>,
        verdicts: &HashMap<String, MihonVerdict>,
        addons: &HashSet<String>,
    ) -> Vec<CatalogEntry> {
        let content = self.host.content();
        let show_nsfw = content.show_nsfw();
        let index_url: Option<reqwest::Url> = repo.url.parse().ok();
        let images = self.app.state::<raygun_net::ImageLinks>();
        index
            .extensions
            .iter()
            .filter_map(|entry| {
                let adult = content.listed_adult(&entry.id, &repo.id, &entry.content_rating);
                (show_nsfw || !adult).then_some((entry, adult))
            })
            .map(|(entry, adult)| {
                // A Stremio addon is installed when an addon at its address is added.
                let added = (entry.format == PackageFormat::Stremio)
                    .then(|| {
                        let url = repo::package_url(index_url.as_ref()?, entry).ok()?;
                        let url =
                            crate::stremio::protocol::normalize_addon_url(url.as_str()).ok()?;
                        addons.contains(&url).then(|| Installed {
                            version: entry.version.clone(),
                            stale_conversion: false,
                        })
                    })
                    .flatten();
                let base = catalog_entry(
                    &repo.id,
                    entry,
                    added.as_ref().or_else(|| local.get(&entry.id)),
                );
                CatalogEntry {
                    icon: index_url
                        .as_ref()
                        .and_then(|base| repo::icon_url(base, entry))
                        .map(|url| images.cover(url.as_str(), &Default::default())),
                    adult,
                    // A Mihon extension tried before and refused, until its next version.
                    unsupported: base.unsupported.clone().or_else(|| {
                        (entry.format == PackageFormat::Mihon)
                            .then(|| verdicts.get(&entry.id)?.unsupported(entry))
                            .flatten()
                    }),
                    ..base
                }
            })
            .collect()
    }

    /// Every repository's listing. A listing is built again only when its index was read again
    /// or something it shows changed on this device (`catalog::Listings`): a store of thousands
    /// of entries took long to build on each look.
    pub async fn catalog(&self, refresh: bool) -> Result<Vec<RepoCatalog>, String> {
        let repos = self.repos().await?;
        let local = self.installed_versions().await;
        let verdicts = self.mihon_verdicts().await;
        let addons = self.added_addons().await;
        let inputs = self.listing_inputs(&local, &verdicts, &addons);
        self.listings.lock().unwrap().keep_only(&repos);
        let listings = repos.into_iter().map(|repo| {
            let (local, verdicts, addons) = (&local, &verdicts, &addons);
            async move {
                match self.fetch_index(&repo.url, refresh).await {
                    Ok(index) => {
                        let name = repo_name(&index, &repo.url);
                        if name != repo.name {
                            let _ = store::rename_repo(self.db(), &repo.id, &name).await;
                        }
                        let repo = ExtensionRepo { name, ..repo };
                        let kept =
                            self.listings
                                .lock()
                                .unwrap()
                                .get(&repo, &index, inputs, LISTING_CACHE);
                        let entries = match kept {
                            Some(entries) => entries,
                            None => {
                                let entries =
                                    self.entries_of(&repo, &index, local, verdicts, addons);
                                self.listings.lock().unwrap().put(
                                    &repo,
                                    index.clone(),
                                    inputs,
                                    entries.clone(),
                                );
                                entries
                            }
                        };
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

    /// What every listing depends on besides its index, as one fingerprint: the extensions on
    /// this device, the conversions tried, the addons added, the 18+ switch and marks, and
    /// whether Mihon extensions' own code can run here.
    fn listing_inputs(
        &self,
        local: &HashMap<String, Installed>,
        verdicts: &HashMap<String, MihonVerdict>,
        addons: &HashSet<String>,
    ) -> catalog::Inputs {
        let local = catalog::sorted(
            local
                .iter()
                .map(|(id, installed)| (id, &installed.version, installed.stale_conversion)),
        );
        let verdicts = catalog::sorted(
            verdicts
                .iter()
                .map(|(id, verdict)| (id, &verdict.version, verdict.converter, &verdict.reason)),
        );
        catalog::Inputs::of(&(
            local,
            verdicts,
            catalog::sorted(addons),
            self.host.content().show_nsfw(),
            self.content_changes.load(Ordering::Relaxed),
            mihon::runtime_present(),
        ))
    }

    /// The index at `url`, or for a folder's address (as Miru takes repositories) the
    /// `index.json` in it; with the address it was read from.
    async fn index_at(
        &self,
        url: reqwest::Url,
        refresh: bool,
    ) -> Result<(reqwest::Url, Arc<RepoIndex>), String> {
        match self.fetch_index(url.as_str(), refresh).await {
            Ok(index) => Ok((url, index)),
            Err(error) => {
                // A Kodi repository shared as its own add-on (its addon.xml or zip) names its
                // index.
                let path = url.path().to_ascii_lowercase();
                // A large Kodi index (Kodi's own is 12 MB) is read gzipped, as Kodi reads it.
                if path.ends_with("/addons.xml")
                    && let Ok(packed) = reqwest::Url::parse(&format!("{url}.gz"))
                    && let Ok(index) = self.fetch_index(packed.as_str(), refresh).await
                {
                    return Ok((packed, index));
                }
                let addon = path.ends_with(".xml") || path.ends_with(".zip");
                if addon
                    && let Ok(bytes) = self.download(url.as_str(), MAX_PACKAGE_BYTES).await
                    && let Some(named) = raygun_ext_host::kodi::index_address(&bytes, &url)
                {
                    let index = self.fetch_index(named.as_str(), refresh).await?;
                    return Ok((named, index));
                }
                let beside = repo::index_beside(&url).ok_or_else(|| error.clone())?;
                let index = self
                    .fetch_index(beside.as_str(), refresh)
                    .await
                    .map_err(|_| error)?;
                Ok((beside, index))
            }
        }
    }

    /// The index at `url`: the one read within [`INDEX_CACHE`] unless `refresh`, shared rather
    /// than copied (a store's index holds thousands of entries).
    async fn fetch_index(&self, url: &str, refresh: bool) -> Result<Arc<RepoIndex>, String> {
        if !refresh && let Some(index) = self.indexes.lock().unwrap().get(url, INDEX_CACHE) {
            return Ok(index);
        }
        let bytes = self.download(url, MAX_INDEX_BYTES).await?;
        // A Mihon store (`index.min.json`, `repo.json`, `index.pb`), followed to its extensions.
        if mihon::detect(&bytes) {
            let address: reqwest::Url = url.parse().map_err(|_| "invalid repository URL")?;
            let (from, store) = mihon::load(&address, bytes, |next: reqwest::Url| async move {
                self.download(next.as_str(), MAX_INDEX_BYTES).await
            })
            .await?;
            let index = mihon::to_repo_index(&store)?;
            self.mihon_stores
                .lock()
                .unwrap()
                .insert(url.to_owned(), (from, Arc::new(store)));
            return Ok(self.indexes.lock().unwrap().put(url, index));
        }
        // Kodi's indexes come gzipped as often as not (`addons.xml.gz`). Parsing a large index
        // takes a while: off the async workers.
        let index = tauri::async_runtime::spawn_blocking(move || {
            let text = raygun_ext_host::kodi::unpack(&bytes)?;
            repo::parse_index(&text).map_err(|error| error.to_string())
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(self.indexes.lock().unwrap().put(url, index))
    }

    /// The index at `url` as it is now, for installing from it: one read moments ago counts
    /// ([`FRESH_INDEX`]), and says so.
    async fn current_index(&self, url: &str) -> Result<(Arc<RepoIndex>, bool), String> {
        if let Some(index) = self.indexes.lock().unwrap().get(url, FRESH_INDEX) {
            return Ok((index, true));
        }
        Ok((self.fetch_index(url, true).await?, false))
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

    /// Installs (or updates to) the version a repository currently lists, then loads it.
    pub async fn install(
        &self,
        repo_id: &str,
        extension_id: &str,
    ) -> Result<Vec<ExtensionInfo>, String> {
        self.install_package(repo_id, extension_id, &|_| {}).await?;
        Ok(self.reload().await)
    }

    /// Installs the version a repository currently lists without loading it (the install queue
    /// loads once for a whole batch); `phase` says how far it got.
    pub async fn install_package(
        &self,
        repo_id: &str,
        extension_id: &str,
        phase: &(dyn Fn(raygun_model::ExtensionInstallPhase) + Send + Sync),
    ) -> Result<(), String> {
        use raygun_model::ExtensionInstallPhase as Phase;
        let _changing = self.changing.lock().await;
        phase(Phase::Downloading);
        let repo = self
            .repos()
            .await?
            .into_iter()
            .find(|repo| repo.id == repo_id)
            .ok_or("that repository was removed")?;
        // The current index, not the catalogue's (a package may have changed since); the
        // installs of a queue share one read moments ago (`FRESH_INDEX`).
        let (index, reused) = self.current_index(&repo.url).await?;
        match self.install_from(&repo, &index, extension_id, phase).await {
            // The repository may have changed the entry in the meantime: read it again, and
            // install what it lists now if it did.
            Err(error) if reused => {
                let current = self.fetch_index(&repo.url, true).await?;
                let listed = |index: &RepoIndex| {
                    index
                        .extensions
                        .iter()
                        .find(|entry| entry.id == extension_id)
                        .cloned()
                };
                if listed(&current) == listed(&index) {
                    return Err(error);
                }
                self.install_from(&repo, &current, extension_id, phase)
                    .await
            }
            done => done,
        }
    }

    /// Installs `extension_id` as `index`, the repository's current index, lists it.
    async fn install_from(
        &self,
        repo: &ExtensionRepo,
        index: &RepoIndex,
        extension_id: &str,
        phase: &(dyn Fn(raygun_model::ExtensionInstallPhase) + Send + Sync),
    ) -> Result<(), String> {
        use raygun_model::ExtensionInstallPhase as Phase;
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == extension_id)
            .ok_or_else(|| format!("{} no longer lists {extension_id}", repo.name))?;
        if entry.format == PackageFormat::Kodi {
            return Err(raygun_ext_host::kodi::UNSUPPORTED.to_owned());
        }
        // A Stremio addon a repository lists: installing it adds the addon by its address.
        if entry.format == PackageFormat::Stremio {
            let index_url: reqwest::Url = repo.url.parse().map_err(|_| "invalid repository URL")?;
            let manifest =
                repo::package_url(&index_url, entry).map_err(|error| error.to_string())?;
            phase(Phase::Installing);
            self.app
                .state::<Arc<crate::stremio::StremioService>>()
                .add(manifest.as_str())
                .await?;
            tracing::info!(addon = %entry.id, url = %manifest, "Stremio addon added from a repository");
            return Ok(());
        }
        // Mihon's extensions are converted from their source code; their APKs aren't run.
        if entry.format == PackageFormat::Mihon {
            let verified = self.convert_mihon(repo, entry, phase).await?;
            return self.finish_install(repo, entry, verified, phase).await;
        }
        let index_url: reqwest::Url = repo.url.parse().map_err(|_| "invalid repository URL")?;
        let package_url =
            repo::package_url(&index_url, entry).map_err(|error| error.to_string())?;
        let limit = if entry.size == 0 {
            MAX_PACKAGE_BYTES
        } else {
            entry.size.min(MAX_PACKAGE_BYTES)
        };
        let bytes = self.download_patiently(package_url.as_str(), limit).await?;

        phase(Phase::Converting);
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
                let mut others: Vec<(reqwest::Url, Arc<RepoIndex>)> = Vec::new();
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
                    index,
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
            PackageFormat::Miru => {
                let (script, header) = raygun_ext_host::miru::package::verify_script(&bytes, entry)
                    .map_err(|error| error.to_string())?;
                raygun_ext_host::miru::package::package(entry, script, &header)
                    .map_err(|error| error.to_string())?
            }
            PackageFormat::LnReader => {
                let script = raygun_ext_host::lnreader::package::verify_script(&bytes, entry)
                    .map_err(|error| error.to_string())?;
                raygun_ext_host::lnreader::package::package(entry, script)
                    .map_err(|error| error.to_string())?
            }
            PackageFormat::Kodi => return Err(raygun_ext_host::kodi::UNSUPPORTED.to_owned()),
            PackageFormat::Mihon => unreachable!("converted before downloading"),
            PackageFormat::Stremio => unreachable!("added before downloading"),
        };
        self.finish_install(repo, entry, verified, phase).await
    }

    /// The kept, checked APK for a Mihon package (`.apk/<package>.apk`), when it's there.
    pub fn mihon_apk_path(&self, package: &str) -> Result<std::path::PathBuf, String> {
        if package.contains(['/', '\\']) || package.contains("..") {
            return Err("that isn't a package name".into());
        }
        let path = self
            .extensions_dir
            .join(".apk")
            .join(format!("{package}.apk"));
        if path.is_file() {
            Ok(path)
        } else {
            Err("Download the APK first".into())
        }
    }

    /// Downloads a Mihon extension's APK from its store and checks it against the store's entry
    /// (package, version code, signing key) before keeping it in the extensions folder
    /// (`.apk/<package>.apk`). Running it needs a container for its own code, which this device
    /// doesn't have yet: the answer says so.
    pub async fn download_mihon_apk(
        &self,
        repo_id: &str,
        entry_id: &str,
    ) -> Result<raygun_model::MihonApk, String> {
        let repo = self
            .repos()
            .await?
            .into_iter()
            .find(|repo| repo.id == repo_id)
            .ok_or("The repository is no longer added")?;
        let (store_url, store) = self
            .mihon_stores
            .lock()
            .unwrap()
            .get(&repo.url)
            .cloned()
            .ok_or("Open the store's list first, so Raygun reads it")?;
        let extension = mihon::index::find(&store, entry_id)
            .cloned()
            .ok_or_else(|| format!("{} no longer lists {entry_id}", repo.name))?;
        let url = store_url
            .join(&extension.apk)
            .map_err(|error| format!("the store's APK address is odd: {error}"))?;
        let bytes = self.download_patiently(url.as_str(), MAX_APK_BYTES).await?;
        let facts = mihon::apk::read(&bytes)?;
        mihon::apk::check(&facts, &extension, store.signing_key.as_deref())
            .map_err(|reason| format!("Raygun didn't keep the APK: {reason}"))?;
        let size = u32::try_from(bytes.len()).unwrap_or(u32::MAX);
        let dir = self.extensions_dir.join(".apk");
        let path = dir.join(format!("{}.apk", extension.pkg));
        tokio::task::spawn_blocking(move || -> std::io::Result<()> {
            std::fs::create_dir_all(&dir)?;
            std::fs::write(&path, &bytes)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("Raygun couldn't save the APK: {error}"))?;
        Ok(raygun_model::MihonApk {
            package: facts.package,
            version_code: u32::try_from(facts.version_code).unwrap_or(u32::MAX),
            signer: facts.signers.first().cloned(),
            size,
            runnable: false,
            note: mihon::CONTAINER_UNAVAILABLE.to_owned(),
        })
    }

    /// Converts a Mihon extension from its source code (`raygun_ext_host::mihon`): reads its code
    /// from the store's repository, converts it, and loads the result once before it's installed.
    /// An extension that can't be converted is remembered with the reason, which the catalogue
    /// then shows.
    async fn convert_mihon(
        &self,
        repo: &ExtensionRepo,
        entry: &RepoEntry,
        phase: &(dyn Fn(raygun_model::ExtensionInstallPhase) + Send + Sync),
    ) -> Result<package::VerifiedPackage, String> {
        use raygun_model::ExtensionInstallPhase as Phase;
        let (store_url, store) = self
            .mihon_stores
            .lock()
            .unwrap()
            .get(&repo.url)
            .cloned()
            .ok_or("the store wasn't read")?;
        let extension = mihon::index::find(&store, &entry.id)
            .cloned()
            .ok_or_else(|| format!("{} no longer lists {}", repo.name, entry.id))?;
        let folder = mihon::locate(&extension, &store_url)
            .ok_or("Raygun can't tell where this extension's source code is")?;
        let mut sources =
            mihon::source::fetch(&folder, &extension, |url: reqwest::Url| async move {
                self.download_patiently(url.as_str(), MAX_PACKAGE_BYTES)
                    .await
            })
            .await?;
        phase(Phase::Converting);
        let mut result = mihon::convert(&extension, &sources);
        // A theme Raygun has no port of, or whose port refused it: converted with the theme's code.
        if result.is_err()
            && let Some(theme) = mihon::convert::theme_to_read(&sources)
        {
            let read = mihon::source::fetch_theme(
                &folder,
                &theme,
                &mut sources,
                |url: reqwest::Url| async move {
                    self.download_patiently(url.as_str(), MAX_PACKAGE_BYTES)
                        .await
                },
            )
            .await;
            match read {
                Ok(()) => result = mihon::convert(&extension, &sources),
                Err(error) => tracing::info!(%error, theme, "couldn't read the theme's code"),
            }
        }
        // Keiyoushi's libraries it (or its theme) builds on: their code converts with its own.
        for _ in 0..3 {
            let wanted = mihon::convert::libraries_to_read(&sources);
            if result.is_ok() || wanted.is_empty() {
                break;
            }
            for library in wanted {
                let read = mihon::source::fetch_library(
                    &folder,
                    &library,
                    &mut sources,
                    |url: reqwest::Url| async move {
                        self.download_patiently(url.as_str(), MAX_PACKAGE_BYTES)
                            .await
                    },
                )
                .await;
                if let Err(error) = read {
                    tracing::info!(%error, library, "couldn't read the library's code");
                }
            }
            result = mihon::convert(&extension, &sources);
            // A theme whose port refused it may still convert with its own code.
            if result.is_err()
                && let Some(theme) = mihon::convert::theme_to_read(&sources)
                && mihon::source::fetch_theme(
                    &folder,
                    &theme,
                    &mut sources,
                    |url: reqwest::Url| async move {
                        self.download_patiently(url.as_str(), MAX_PACKAGE_BYTES)
                            .await
                    },
                )
                .await
                .is_ok()
            {
                result = mihon::convert(&extension, &sources);
            }
        }
        let converted = match result {
            Ok(converted) => converted,
            Err(reason) => {
                self.remember_mihon_verdict(&extension, Some(&reason)).await;
                return Err(format!("{} can't be converted: {reason}", entry.name));
            }
        };
        let verified = mihon::package(&extension, &converted, &store.name, Some(&folder.web_url()))
            .map_err(|error| error.to_string())?;
        if let Err(reason) = mihon::convert::validate(&verified, extension.sources.len()).await {
            self.remember_mihon_verdict(&extension, Some(&reason)).await;
            return Err(format!("{}'s conversion didn't run: {reason}", entry.name));
        }
        self.remember_mihon_verdict(&extension, None).await;
        tracing::info!(extension = %entry.id, theme = ?converted.theme, "Mihon extension converted");
        Ok(verified)
    }

    /// Why each Mihon extension tried couldn't be converted (`None`: it was), for its version and
    /// the converter that tried, kept on this device.
    async fn mihon_verdicts(&self) -> HashMap<String, MihonVerdict> {
        raygun_db::local_value(self.db(), MIHON_VERDICTS)
            .await
            .ok()
            .flatten()
            .and_then(|text| serde_json::from_str(&text).ok())
            .unwrap_or_default()
    }

    async fn remember_mihon_verdict(
        &self,
        extension: &mihon::StoreExtension,
        reason: Option<&str>,
    ) {
        let Some(id) = mihon::index::extension_id(&extension.pkg) else {
            return;
        };
        let mut verdicts = self.mihon_verdicts().await;
        match reason {
            Some(reason) => {
                verdicts.insert(
                    id,
                    MihonVerdict {
                        version: mihon::index::semver_version(extension),
                        converter: mihon::CONVERTER_VERSION,
                        reason: reason.to_owned(),
                    },
                );
            }
            None => {
                verdicts.remove(&id);
            }
        }
        let text = serde_json::to_string(&verdicts).unwrap_or_default();
        if let Err(error) = raygun_db::set_local_value(self.db(), MIHON_VERDICTS, &text).await {
            tracing::debug!(%error, "could not remember a conversion's result");
        }
    }

    /// Writes a checked package into `extensions/`, records where it came from and keeps its
    /// icon.
    async fn finish_install(
        &self,
        repo: &ExtensionRepo,
        entry: &RepoEntry,
        verified: package::VerifiedPackage,
        phase: &(dyn Fn(raygun_model::ExtensionInstallPhase) + Send + Sync),
    ) -> Result<(), String> {
        use raygun_model::ExtensionInstallPhase as Phase;
        let index_url: reqwest::Url = repo.url.parse().map_err(|_| "invalid repository URL")?;
        phase(Phase::Installing);
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
        Ok(())
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

    /// Where installed extensions live (connected servers read their folders there).
    pub fn extensions_dir(&self) -> &Path {
        &self.extensions_dir
    }

    /// Installs a package built on this device (a connected server's folder), then reloads.
    pub async fn install_verified(
        &self,
        verified: package::VerifiedPackage,
    ) -> Result<Vec<ExtensionInfo>, String> {
        {
            let _changing = self.changing.lock().await;
            let extensions_dir = self.extensions_dir.clone();
            tauri::async_runtime::spawn_blocking(move || {
                package::install(&extensions_dir, &verified)
            })
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        }
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

    /// Removes an installed extension and its storage.
    pub async fn uninstall(&self, extension_id: &str) -> Result<Vec<ExtensionInfo>, String> {
        let _changing = self.changing.lock().await;
        let installed = self
            .list()
            .await
            .into_iter()
            .any(|info| info.id == extension_id && info.origin == ExtensionOrigin::Installed);
        if !installed {
            // A repository's Stremio addon: removing it removes the addon.
            if let Some(url) = self.listed_addon_url(extension_id).await {
                let stremio = self.app.state::<Arc<crate::stremio::StremioService>>();
                if let Some(addon) = stremio
                    .addons()
                    .await?
                    .into_iter()
                    .find(|addon| addon.url == url)
                {
                    stremio.remove(&addon.id).await?;
                    return Ok(self.list().await);
                }
            }
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
        // A connected server's sign-in goes with it (`servers`).
        if extension_id.starts_with(raygun_ext_host::servers::ID_PREFIX)
            && let Some(ai) = self.app.try_state::<crate::ai::AiService>()
            && let Err(error) = ai.set_server_secret(extension_id, None)
        {
            tracing::warn!(%error, "could not forget a server's sign-in");
        }
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

/// The sources earlier versions carried inside the app; Paraline lists every one.
const FORMER_BUILT_INS: [&str; 13] = [
    "dev.raygun.internet-archive",
    "dev.raygun.wikimedia-commons",
    "dev.raygun.nasa",
    "dev.raygun.blender-movies",
    "dev.raygun.musicbrainz",
    "dev.raygun.librivox",
    "dev.raygun.loyalbooks",
    "dev.raygun.archive-comics",
    "dev.raygun.peppercarrot",
    "dev.raygun.xkcd",
    "dev.raygun.gutenberg",
    "dev.raygun.standard-ebooks",
    "dev.raygun.wikisource",
];
/// `local_meta`: `pending` until the former built-in sources are installed (migration 0041).
const FORMER_BUILT_INS_KEY: &str = "extensions.formerBuiltIns";

/// Raygun's repository, Paraline: public-domain and openly licensed sources in one index. New
/// installs start empty and offer it as a suggestion.
const PARALINE_URL: &str =
    "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/index.json";
const PARALINE_NAME: &str = "Paraline";
/// Paraline's row, under a fixed id like the other seeds, so every device's copy is one row.
const PARALINE_ID: &str = "0192a000-0000-7000-8000-000000000002";

/// Repositories offered with one tap: Raygun's own only. Other repositories (Shosetsu's, Nuclear's
/// registry, Miru's and anyone's) are added by their address, and ones already added stay.
const SUGGESTED_REPOS: [(&str, &str, &str); 1] = [(
    PARALINE_URL,
    PARALINE_NAME,
    "Public-domain and openly licensed books, audiobooks, films, comics and music.",
)];

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
/// Installed extensions' repositories by id, matched by the address each was installed from.
fn extension_repositories(
    installs: &[InstallRow],
    repos: &[ExtensionRepo],
) -> HashMap<String, String> {
    let by_address = by_address(repos);
    installs
        .iter()
        .filter_map(|install| {
            let repo = by_address.get(&url_key(&install.repo_url))?;
            Some((install.extension_id.clone(), repo.id.clone()))
        })
        .collect()
}

/// Repositories by their address as [`same_url`] compares them, the first of any two alike.
fn by_address(repos: &[ExtensionRepo]) -> HashMap<String, &ExtensionRepo> {
    let mut found = HashMap::with_capacity(repos.len());
    for repo in repos {
        found.entry(url_key(&repo.url)).or_insert(repo);
    }
    found
}

/// The name of the repository each installed extension came from, by its id: matched by the
/// address it was installed from; Paraline for one installed from an index Paraline replaced.
fn repository_names(installs: &[InstallRow], repos: &[ExtensionRepo]) -> HashMap<String, String> {
    let by_address = by_address(repos);
    let mut names = HashMap::with_capacity(installs.len());
    for install in installs {
        let name = by_address
            .get(&url_key(&install.repo_url))
            .map(|repo| repo.name.clone())
            .or_else(|| {
                RETIRED_REPOS
                    .iter()
                    .any(|retired| same_url(&install.repo_url, retired))
                    .then(|| PARALINE_NAME.to_owned())
            });
        if let Some(name) = name {
            names.entry(install.extension_id.clone()).or_insert(name);
        }
    }
    names
}

/// [`repository_names`] for one extension.
#[cfg(test)]
fn repository_name(
    extension_id: &str,
    installs: &[InstallRow],
    repos: &[ExtensionRepo],
) -> Option<String> {
    let install = installs
        .iter()
        .find(|install| install.extension_id == extension_id)?;
    repository_names(std::slice::from_ref(install), repos).remove(extension_id)
}

fn same_url(a: &str, b: &str) -> bool {
    a.trim()
        .trim_end_matches('/')
        .eq_ignore_ascii_case(b.trim().trim_end_matches('/'))
}

/// An address as [`same_url`] compares it: trimmed, without trailing slashes, in lower case.
fn url_key(url: &str) -> String {
    url.trim().trim_end_matches('/').to_ascii_lowercase()
}

/// A GitHub file's address as owner, project, branch and path within the project.
fn github_file(url: &str) -> Option<(&str, &str, &str, &str)> {
    let rest = url
        .trim()
        .strip_prefix("https://raw.githubusercontent.com/")?
        .trim_end_matches('/');
    let mut parts = rest.splitn(4, '/');
    Some((parts.next()?, parts.next()?, parts.next()?, parts.next()?))
}

/// Whether two addresses are one repository's index: the same address, or the same file of a
/// GitHub project where one names the default branch as `HEAD` (as a project's page is read).
fn same_repo(a: &str, b: &str) -> bool {
    if same_url(a, b) {
        return true;
    }
    match (github_file(a), github_file(b)) {
        (Some((owner, project, branch, path)), Some((owner_b, project_b, branch_b, path_b))) => {
            owner.eq_ignore_ascii_case(owner_b)
                && project.eq_ignore_ascii_case(project_b)
                && path == path_b
                && (branch == branch_b || branch == "HEAD" || branch_b == "HEAD")
        }
        _ => false,
    }
}

/// Where an installed extension's icon address is kept.
fn icon_key(extension_id: &str) -> String {
    format!("extension.icon.{extension_id}")
}

/// What the catalogue needs to know about an installed extension.
#[derive(Debug, Clone, Default)]
struct Installed {
    version: String,
    /// A Nuclear plugin or a Mihon extension converted by an older converter: installing it
    /// again brings the newer one's fixes (M46.9.1).
    stale_conversion: bool,
}

fn catalog_entry(repo_id: &str, entry: &RepoEntry, local: Option<&Installed>) -> CatalogEntry {
    let state = match local {
        None => CatalogState::Available,
        Some(installed) => match repo::compare_versions(&entry.version, &installed.version) {
            std::cmp::Ordering::Greater => CatalogState::UpdateAvailable,
            // Converted by an older converter (a Nuclear plugin, a Mihon extension): installing
            // it again brings what the newer one does.
            std::cmp::Ordering::Equal
                if installed.stale_conversion
                    && matches!(entry.format, PackageFormat::Nuclear | PackageFormat::Mihon) =>
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
        adult: entry.content_rating == "nsfw",
        lang: entry.lang.clone(),
        size: u32::try_from(entry.size).unwrap_or(u32::MAX),
        nuclear: entry.format == PackageFormat::Nuclear,
        state,
        installed_version: local.map(|installed| installed.version.clone()),
        icon: None,
        // A Mihon extension says why it can't be converted once it was tried (`entries_of`).
        unsupported: match entry.format {
            PackageFormat::Kodi => Some(raygun_ext_host::kodi::UNSUPPORTED.to_owned()),
            PackageFormat::Mihon if !raygun_ext_host::mihon::runtime_present() => {
                Some(raygun_ext_host::mihon::UNSUPPORTED.to_owned())
            }
            _ => None,
        },
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
    fn installed_extensions_belong_to_the_repository_they_came_from() {
        let install = |id: &str, url: &str| super::InstallRow {
            extension_id: id.into(),
            repo_url: url.into(),
            version: "1.0.0".into(),
            sha256: String::new(),
            installed_at: 0,
        };
        let repos = [raygun_model::ExtensionRepo {
            id: "r1".into(),
            url: "https://example.com/index.json".into(),
            name: "Example".into(),
        }];
        let found = super::extension_repositories(
            &[
                install("a", "https://EXAMPLE.com/index.json/"),
                install("b", "https://gone.example/index.json"),
            ],
            &repos,
        );
        assert_eq!(found.get("a").map(String::as_str), Some("r1"));
        assert!(!found.contains_key("b"), "its repository was removed");
    }

    #[test]
    fn album_covers_need_the_title_and_artist() {
        let mut right = album("tunes", "Discovery", "Daft Punk", None);
        right.artwork_url = Some("https://example.com/a.jpg".into());
        let mut wrong = album("tunes", "Discovery", "Someone Else", None);
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
                adult: false,
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
        let mut tunes = album("tunes", "Discovery", "Daft Punk", None);
        tunes.artwork_url = Some("https://example.com/d.jpg".into());
        tunes.kind = Some(raygun_model::ReleaseKind::Album);
        let mut deluxe = album(
            "tunes",
            "Homework (25th Anniversary Edition)",
            "Daft Punk",
            Some(2022),
        );
        deluxe.artwork_url = Some("https://example.com/h.jpg".into());
        let found = vec![
            deluxe,
            album("mb", "Homework", "Daft Punk", Some(1997)),
            album("mb", "Discovery", "Daft Punk", Some(2001)),
            tunes,
            album("mb", "Something Else", "Another Band", Some(2020)),
        ];
        let releases = releases_of(found, "Daft Punk");
        let titles: Vec<_> = releases.iter().map(|a| a.title.as_str()).collect();
        assert_eq!(titles, ["Discovery", "Homework"]);
        // The copy with artwork wins and keeps the other's year.
        assert_eq!(releases[0].provider.id, "tunes");
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
    fn first_party_extensions_parse() {
        for source in first_party() {
            let raygun_ext_host::ExtensionFiles::Memory(files) = &source.files else {
                panic!("first-party bundles are read from memory");
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

    /// First-party sources added since the built-ins moved out load too.
    #[test]
    fn newer_first_party_extensions_parse() {
        let manifest = raygun_ext_host::Manifest::parse(include_str!(
            "../../../extensions/listenbrainz/raygun-extension.json"
        ))
        .unwrap();
        assert_eq!(manifest.provides, ["music.charts", "music.recommend"]);
        assert!(include_str!("../../../extensions/listenbrainz/dist/index.js").contains("charts"));
        for (manifest, bundle, kinds) in [
            (
                include_str!("../../../extensions/openstax/raygun-extension.json"),
                include_str!("../../../extensions/openstax/dist/index.js"),
                &["novel.source"][..],
            ),
            (
                include_str!("../../../extensions/wikibooks/raygun-extension.json"),
                include_str!("../../../extensions/wikibooks/dist/index.js"),
                &["novel.source"][..],
            ),
            (
                include_str!("../../../extensions/bccampus/raygun-extension.json"),
                include_str!("../../../extensions/bccampus/dist/index.js"),
                &["novel.source"][..],
            ),
            (
                include_str!("../../../extensions/gnu-manuals/raygun-extension.json"),
                include_str!("../../../extensions/gnu-manuals/dist/index.js"),
                &["novel.source"][..],
            ),
            (
                include_str!("../../../extensions/green-tea-press/raygun-extension.json"),
                include_str!("../../../extensions/green-tea-press/dist/index.js"),
                &["novel.source"][..],
            ),
            (
                include_str!("../../../extensions/saylor/raygun-extension.json"),
                include_str!("../../../extensions/saylor/dist/index.js"),
                &["novel.source"][..],
            ),
            (
                include_str!("../../../extensions/lastfm/raygun-extension.json"),
                include_str!("../../../extensions/lastfm/dist/index.js"),
                &[
                    "music.charts",
                    "music.tags",
                    "music.recommend",
                    "music.info",
                ][..],
            ),
            (
                include_str!("../../../extensions/europeana/raygun-extension.json"),
                include_str!("../../../extensions/europeana/dist/index.js"),
                &["video.source", "music.metadata"][..],
            ),
            (
                include_str!("../../../extensions/loc/raygun-extension.json"),
                include_str!("../../../extensions/loc/dist/index.js"),
                &["video.source", "music.metadata"][..],
            ),
            (
                include_str!("../../../extensions/nara/raygun-extension.json"),
                include_str!("../../../extensions/nara/dist/index.js"),
                &["video.source"][..],
            ),
            (
                include_str!("../../../extensions/nps/raygun-extension.json"),
                include_str!("../../../extensions/nps/dist/index.js"),
                &["video.source"][..],
            ),
            (
                include_str!("../../../extensions/esa-hubble/raygun-extension.json"),
                include_str!("../../../extensions/esa-hubble/dist/index.js"),
                &["video.source"][..],
            ),
            (
                include_str!("../../../extensions/fyyd/raygun-extension.json"),
                include_str!("../../../extensions/fyyd/dist/index.js"),
                &["podcast.directory"][..],
            ),
            (
                include_str!("../../../extensions/podcast-index/raygun-extension.json"),
                include_str!("../../../extensions/podcast-index/dist/index.js"),
                &["podcast.directory"][..],
            ),
            (
                include_str!("../../../extensions/incompetech/raygun-extension.json"),
                include_str!("../../../extensions/incompetech/dist/index.js"),
                &["music.metadata", "music.charts", "music.stream"][..],
            ),
            (
                include_str!("../../../extensions/live-music-archive/raygun-extension.json"),
                include_str!("../../../extensions/live-music-archive/dist/index.js"),
                &["music.metadata"][..],
            ),
            (
                include_str!("../../../extensions/gutenberg-audiobooks/raygun-extension.json"),
                include_str!("../../../extensions/gutenberg-audiobooks/dist/index.js"),
                &["audiobook.directory"][..],
            ),
        ] {
            let manifest = raygun_ext_host::Manifest::parse(manifest).unwrap();
            assert_eq!(manifest.provides, kinds, "{}", manifest.id);
            assert!(bundle.contains("registerProvider") || bundle.contains("providers.register"));
        }
    }

    /// Every source earlier versions carried is moved to an install, and nothing else is.
    #[test]
    fn former_built_ins_are_the_first_party_extensions() {
        let mut bundled: Vec<String> = first_party()
            .iter()
            .map(|source| {
                let raygun_ext_host::ExtensionFiles::Memory(files) = &source.files else {
                    panic!("first-party bundles are read from memory");
                };
                raygun_ext_host::Manifest::parse(&files["raygun-extension.json"])
                    .unwrap()
                    .id
            })
            .collect();
        let mut former: Vec<String> = FORMER_BUILT_INS.iter().map(|id| (*id).to_owned()).collect();
        bundled.sort();
        former.sort();
        assert_eq!(former, bundled);
    }

    #[test]
    fn extensions_name_the_repository_they_came_from() {
        let install = |id: &str, url: &str| InstallRow {
            extension_id: id.into(),
            repo_url: url.into(),
            version: "1.0.0".into(),
            sha256: String::new(),
            installed_at: 0,
        };
        let installs = [
            install("a", &format!("{PARALINE_URL}/")),
            install("b", RETIRED_REPOS[0]),
            install("c", "https://example.org/own.zip"),
        ];
        let repos = [ExtensionRepo {
            id: "p".into(),
            url: PARALINE_URL.into(),
            name: "Paraline".into(),
        }];
        assert_eq!(
            repository_name("a", &installs, &repos).as_deref(),
            Some("Paraline")
        );
        // Installed from an index Paraline replaced, before it was folded in.
        assert_eq!(
            repository_name("b", &installs, &[]).as_deref(),
            Some("Paraline")
        );
        assert_eq!(repository_name("c", &installs, &repos), None);
        assert_eq!(repository_name("missing", &installs, &repos), None);
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
        // A project's page is read at HEAD: the branch an added address names.
        let added = "https://raw.githubusercontent.com/uvxszi/paraline-raygun/main/index.json";
        assert!(same_repo(
            added,
            "https://raw.githubusercontent.com/uvxszi/Paraline-Raygun/HEAD/index.json"
        ));
        assert!(!same_repo(
            added,
            "https://raw.githubusercontent.com/uvxszi/paraline-raygun/dev/index.json"
        ));
        assert!(!same_repo(
            added,
            "https://raw.githubusercontent.com/uvxszi/paraline-raygun/HEAD/novels/index.json"
        ));
        assert!(!same_repo(
            added,
            "https://raw.githubusercontent.com/someone/paraline-raygun/HEAD/index.json"
        ));
        assert!(
            SUGGESTED_REPOS
                .iter()
                .all(|(url, _, _)| url.starts_with("https://"))
        );
    }

    #[test]
    fn paraline_replaces_the_indexes_it_took_over() {
        // Paraline is the only suggestion; Shosetsu's and Nuclear's are added by address.
        let names: Vec<_> = SUGGESTED_REPOS.iter().map(|(_, name, _)| *name).collect();
        assert_eq!(names, ["Paraline"]);
        assert!(
            SUGGESTED_REPOS
                .iter()
                .all(|(url, _, _)| !url.contains("shosetsuorg") && !url.contains("NuclearPlayer"))
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
        assert_eq!(repo_name(&index(""), shosetsu), "shosetsuorg");
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
            required: false,
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
