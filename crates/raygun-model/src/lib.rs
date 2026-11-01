//! Domain types shared between the Rust core and the TypeScript UI.
//!
//! Every type here derives `specta::Type`, so it is exported to
//! `packages/bindings` and stays identical on both sides of the boundary.

use serde::{Deserialize, Serialize};
use specta::Type;

/// Build and runtime facts about the running app.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppInfo {
    pub name: String,
    pub version: String,
    /// Operating system, as reported by `std::env::consts::OS` (e.g. `android`, `windows`).
    pub platform: String,
    pub arch: String,
    pub debug: bool,
}

/// How the data folder was chosen (ARCHITECTURE §3).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StorageMode {
    /// Next to the executable (desktop default).
    Portable,
    /// OS app-data folder, chosen because the app's own folder wasn't writable.
    Installed,
    /// App sandbox on Android/iOS.
    Mobile,
    /// Set explicitly through `RAYGUN_HOME`.
    Override,
}

/// Where Raygun keeps its data on this device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageInfo {
    pub mode: StorageMode,
    pub root: String,
    pub logs: String,
    pub webview: String,
}

/// Color theme. `System` follows the OS light/dark preference.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Theme {
    #[default]
    System,
    Light,
    Dark,
    /// Pure black background for OLED screens.
    Amoled,
}

/// User settings. Stored one row per field so devices can sync them field by field.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Settings {
    pub theme: Theme,
    /// UI language as a BCP 47 tag, e.g. `en` or `pt-BR`.
    pub language: String,
    /// Hours between automatic manga library updates; 0 turns them off (M28).
    pub manga_update_hours: u32,
    /// How novels are set (M31).
    pub novel_reader: NovelReaderPrefs,
    /// Shows NSFW (18+) extensions and content; off by default (M35).
    pub show_nsfw: bool,
}

/// The `language` setting that follows the device's language.
pub const SYSTEM_LANGUAGE: &str = "system";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NovelFont {
    Serif,
    Sans,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NovelWidth {
    Narrow,
    Wide,
}

/// The novel reader's typography and layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NovelReaderPrefs {
    /// CSS pixels, 14–30.
    pub font_size: u32,
    /// Line height in tenths, 12–22 (1.2–2.2).
    pub line_spacing: u32,
    pub font: NovelFont,
    pub width: NovelWidth,
    /// Pages turned sideways instead of one scrolling column.
    pub paged: bool,
    /// Reading-aloud speed in tenths, 5–20 (0.5×–2×).
    #[serde(default = "normal_rate")]
    pub tts_rate: u32,
}

fn normal_rate() -> u32 {
    10
}

impl Default for NovelReaderPrefs {
    fn default() -> Self {
        Self {
            font_size: 18,
            line_spacing: 16,
            font: NovelFont::Serif,
            width: NovelWidth::Narrow,
            paged: false,
            tts_rate: 10,
        }
    }
}

/// The update intervals offered, in hours (0 = off).
pub const MANGA_UPDATE_HOURS: [u32; 6] = [0, 6, 12, 24, 48, 168];

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: SYSTEM_LANGUAGE.into(),
            manga_update_hours: 12,
            novel_reader: NovelReaderPrefs::default(),
            show_nsfw: false,
        }
    }
}

impl Settings {
    pub fn apply(&mut self, change: SettingChange) {
        match change {
            SettingChange::Theme(theme) => self.theme = theme,
            SettingChange::Language(language) => self.language = language,
            SettingChange::MangaUpdateHours(hours) => self.manga_update_hours = hours,
            SettingChange::NovelReader(prefs) => self.novel_reader = prefs,
            SettingChange::ShowNsfw(show) => self.show_nsfw = show,
        }
    }
}

/// A change to a single setting. The tagged shape (`{ key, value }`) lets the UI send only valid
/// key/value pairs, and doubles as the format of a stored row.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "key", content = "value", rename_all = "camelCase")]
pub enum SettingChange {
    Theme(Theme),
    Language(String),
    MangaUpdateHours(u32),
    NovelReader(NovelReaderPrefs),
    ShowNsfw(bool),
}

impl SettingChange {
    /// Column key of the stored row.
    pub fn key(&self) -> &'static str {
        match self {
            Self::Theme(_) => "theme",
            Self::Language(_) => "language",
            Self::MangaUpdateHours(_) => "mangaUpdateHours",
            Self::NovelReader(_) => "novelReader",
            Self::ShowNsfw(_) => "showNsfw",
        }
    }

    /// Checks values the type system can't express.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Theme(_) | Self::ShowNsfw(_) => Ok(()),
            Self::NovelReader(prefs) => {
                if !(14..=30).contains(&prefs.font_size) {
                    Err(format!(
                        "{} px is outside the font sizes offered",
                        prefs.font_size
                    ))
                } else if !(12..=22).contains(&prefs.line_spacing) {
                    Err("That line spacing isn't offered".into())
                } else if !(5..=20).contains(&prefs.tts_rate) {
                    Err("That reading speed isn't offered".into())
                } else {
                    Ok(())
                }
            }
            Self::MangaUpdateHours(hours) => {
                if MANGA_UPDATE_HOURS.contains(hours) {
                    Ok(())
                } else {
                    Err(format!("{hours} hours is not an offered update interval"))
                }
            }
            Self::Language(tag) => {
                let well_formed = (2..=35).contains(&tag.len())
                    && tag.split('-').all(|part| {
                        !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric())
                    });
                if well_formed {
                    Ok(())
                } else {
                    Err(format!("\"{tag}\" is not a valid language tag"))
                }
            }
        }
    }
}

// ---- Playback (ARCHITECTURE §6) ----

/// Where the bytes of a playable item come from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum MediaSource {
    /// A remote stream. `headers` (Referer, User-Agent, Cookie, ...) are sent by the stream proxy,
    /// because the webview's media element can't set them itself.
    #[serde(rename_all = "camelCase")]
    Url {
        url: String,
        headers: std::collections::BTreeMap<String, String>,
    },
    /// A file on this device.
    #[serde(rename_all = "camelCase")]
    LocalFile { path: String },
    /// A YouTube video, played through YouTube's embedded player (M08).
    #[serde(rename_all = "camelCase")]
    YouTube { video_id: String },
    /// No stream yet: the resolver finds one from `music.stream` providers when it plays (M14).
    #[serde(rename_all = "camelCase")]
    Lookup {
        title: String,
        artist: Option<String>,
        album: Option<String>,
        duration_ms: Option<u32>,
    },
}

/// What the resolver searches for: a track's metadata.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackQuery {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
}

impl TrackQuery {
    /// The query for a queue item: its `Lookup` source, or its own metadata.
    pub fn for_item(item: &QueueItem) -> Self {
        match &item.source {
            MediaSource::Lookup {
                title,
                artist,
                album,
                duration_ms,
            } => Self {
                title: title.clone(),
                artist: artist.clone(),
                album: album.clone(),
                duration_ms: *duration_ms,
            },
            _ => Self {
                title: item.title.clone(),
                artist: item.artist.clone(),
                album: item.album.clone(),
                duration_ms: item.duration_ms,
            },
        }
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MediaKind {
    #[default]
    Audio,
    Video,
}

/// What a caller provides to add something to the queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueItemInput {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork_url: Option<String>,
    pub media_kind: MediaKind,
    pub source: MediaSource,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueItem {
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork_url: Option<String>,
    pub media_kind: MediaKind,
    pub source: MediaSource,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PlaybackStatus {
    Idle,
    Loading,
    Buffering,
    Playing,
    Paused,
    Ended,
    Error,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RepeatMode {
    #[default]
    Off,
    All,
    One,
}

/// Canonical player state, owned by Rust and mirrored by every client (UI, MCP, remote).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaybackState {
    pub status: PlaybackStatus,
    pub current_item_id: Option<String>,
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    /// 0.0 to 1.0.
    pub volume: f64,
    pub repeat: RepeatMode,
    pub shuffle: bool,
    pub error: Option<String>,
    /// Where the current item's stream came from when the resolver picked it (provider name).
    pub source_label: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct QueueSnapshot {
    pub items: Vec<QueueItem>,
    /// Indices into `items` in the order they will play (differs from `items` when shuffled).
    /// The current item is `PlaybackState::current_item_id`; the queue is only re-sent when its
    /// contents or order change, not on every track change.
    pub order: Vec<u32>,
}

/// A user or client request to the player.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum PlaybackControl {
    Play,
    Pause,
    Toggle,
    Next,
    Previous,
    #[serde(rename_all = "camelCase")]
    Seek {
        position_ms: f64,
    },
    SetVolume {
        volume: f64,
    },
    SetRepeat {
        mode: RepeatMode,
    },
    SetShuffle {
        enabled: bool,
    },
    #[serde(rename_all = "camelCase")]
    PlayItem {
        item_id: String,
    },
    #[serde(rename_all = "camelCase")]
    Remove {
        item_id: String,
    },
    Clear,
}

/// An instruction for the active playback engine.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EngineCommand {
    #[serde(rename_all = "camelCase")]
    Load {
        load_id: u32,
        url: String,
        /// For native engines' media session (notification, lock screen).
        title: String,
        artist: Option<String>,
        artwork_url: Option<String>,
        start_ms: f64,
        autoplay: bool,
        volume: f64,
    },
    /// Plays a YouTube video in the embedded player hosted at `embed_url` (webview only).
    #[serde(rename_all = "camelCase")]
    LoadYouTube {
        load_id: u32,
        embed_url: String,
        video_id: String,
        start_ms: f64,
        autoplay: bool,
        volume: f64,
    },
    Play,
    Pause,
    #[serde(rename_all = "camelCase")]
    Seek {
        position_ms: f64,
    },
    SetVolume {
        volume: f64,
    },
    Stop,
}

/// What the engine observed. `load_id` ties it to a specific `Load`, so reports about a track
/// that has already been replaced are ignored.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EngineReport {
    pub load_id: u32,
    pub event: EngineEvent,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum EngineEvent {
    #[serde(rename_all = "camelCase")]
    Progress {
        position_ms: f64,
        duration_ms: Option<f64>,
    },
    Playing,
    Paused,
    Buffering,
    Ended,
    Error {
        message: String,
    },
}

// ---- Local music library (M07) ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryStatus {
    /// Folders the library scans.
    pub roots: Vec<String>,
    pub track_count: u32,
    pub scanning: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ScanPhase {
    Discovering,
    Reading,
    Done,
    Failed,
}

/// Progress of a library scan, pushed as `LibraryScanEvent`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryScanProgress {
    pub phase: ScanPhase,
    /// Audio files found under the roots.
    pub found: u32,
    /// New or changed files to read, and how many are done.
    pub to_read: u32,
    pub read: u32,
    pub removed: u32,
    /// Files that couldn't be read.
    pub failed: u32,
    pub message: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAlbum {
    pub artist: String,
    pub title: String,
    pub year: Option<u32>,
    pub track_count: u32,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryArtist {
    pub name: String,
    pub album_count: u32,
    pub track_count: u32,
    pub artwork_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryTrack {
    pub id: String,
    pub title: String,
    /// The track's own artist (may differ from the album artist on compilations).
    pub artist: String,
    pub album_artist: String,
    pub album: String,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub duration_ms: Option<u32>,
    pub artwork_url: Option<String>,
}

/// Which part of the library to play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PlaySelection {
    Album { artist: String, album: String },
    Artist { artist: String },
    AllTracks,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayRequest {
    pub selection: PlaySelection,
    /// Track to start with; otherwise the first (or a random one when shuffling).
    pub start_track_id: Option<String>,
    pub shuffle: bool,
}

// ---- Extensions (ARCHITECTURE §7) ----

/// Where an extension was loaded from.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionOrigin {
    /// Compiled into the app.
    BuiltIn,
    /// Loaded from the `extensions/` folder; overrides a built-in extension with the same id.
    Installed,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExtensionStatus {
    Loaded,
    Failed,
}

/// A provider an extension registered, e.g. a `music.metadata` search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderInfo {
    /// Unique across extensions: `<extension id>/<provider id>`.
    pub id: String,
    pub extension_id: String,
    pub kind: String,
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionInfo {
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    /// `safe`, `suggestive` or `nsfw`; providers of `nsfw` ones are hidden while NSFW is off.
    pub content_rating: String,
    pub origin: ExtensionOrigin,
    pub status: ExtensionStatus,
    /// Why the extension failed to load, when it did.
    pub error: Option<String>,
    pub provides: Vec<String>,
    /// Host patterns the extension may reach (`archive.org`, `*.archive.org`).
    pub net: Vec<String>,
    pub providers: Vec<ProviderInfo>,
    pub settings: Vec<ExtensionSetting>,
    /// For a converted Nuclear plugin (M17): what works and what doesn't.
    pub compatibility: Option<CompatReport>,
}

/// How much of a Nuclear plugin runs in Raygun (M17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CompatReport {
    /// The original npm package name.
    pub package: String,
    /// Raygun provider kinds the plugin provides.
    pub supported: Vec<String>,
    /// Nuclear categories Raygun can't run (dashboard, playlists, ...).
    pub unsupported_categories: Vec<String>,
    /// Nuclear API domains the plugin calls that are stubs in Raygun.
    pub stubbed_apis: Vec<String>,
    /// UI modules it imports; they're inert in Raygun.
    pub ui_imports: Vec<String>,
}

/// A value of an extension setting: text and select choices are strings.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(untagged)]
pub enum SettingValue {
    Boolean(bool),
    Number(f64),
    Text(String),
}

impl SettingValue {
    pub fn from_json(value: &serde_json::Value) -> Option<Self> {
        match value {
            serde_json::Value::Bool(flag) => Some(Self::Boolean(*flag)),
            serde_json::Value::Number(number) => number.as_f64().map(Self::Number),
            serde_json::Value::String(text) => Some(Self::Text(text.clone())),
            _ => None,
        }
    }

    pub fn to_json(&self) -> serde_json::Value {
        match self {
            Self::Boolean(flag) => serde_json::Value::Bool(*flag),
            Self::Number(number) => serde_json::Number::from_f64(*number)
                .map_or(serde_json::Value::Null, serde_json::Value::Number),
            Self::Text(text) => serde_json::Value::String(text.clone()),
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SettingKind {
    Text,
    Number,
    Boolean,
    Select,
}

/// A setting an extension declares, with its current value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionSetting {
    pub key: String,
    pub label: String,
    pub kind: SettingKind,
    /// The choices of a `select`.
    pub options: Vec<String>,
    pub default: Option<SettingValue>,
    /// The user's value, or the default when they haven't set one.
    pub value: Option<SettingValue>,
}

/// A well-known repository the Repositories tab offers to add with one tap (M36).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SuggestedRepo {
    pub url: String,
    pub name: String,
    pub description: String,
    /// Already one of this device's repositories.
    pub added: bool,
}

/// A repository of installable extensions (an `index.json` URL).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExtensionRepo {
    pub id: String,
    pub url: String,
    /// The index's own name, once it has been fetched.
    pub name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CatalogState {
    Available,
    Installed,
    UpdateAvailable,
    /// This device has a newer version than the repository (a built-in or development copy).
    NewerInstalled,
}

/// An extension a repository offers.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CatalogEntry {
    pub repo_id: String,
    pub id: String,
    pub name: String,
    pub version: String,
    pub description: Option<String>,
    pub author: Option<String>,
    pub provides: Vec<String>,
    pub net: Vec<String>,
    pub content_rating: String,
    /// The content's language (`en`, `multi`), when the repository says.
    pub lang: Option<String>,
    /// Package size in bytes (packages are capped at 10 MiB).
    pub size: u32,
    pub state: CatalogState,
    /// A Nuclear plugin, converted when installed (M17).
    pub nuclear: bool,
    /// The version on this device (built-in or installed), if any.
    pub installed_version: Option<String>,
}

/// One repository's listing; a repository that couldn't be read reports why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RepoCatalog {
    pub repo: ExtensionRepo,
    pub entries: Vec<CatalogEntry>,
    pub error: Option<String>,
}

/// A track found by an online provider.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct OnlineTrack {
    /// The provider's own id for the track, when it has one.
    pub external_id: Option<String>,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork_url: Option<String>,
    /// Directly playable source, when the provider knows one (M14 resolves the others).
    pub source: Option<MediaSource>,
}

/// One timed line of synced lyrics.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricLine {
    pub time_ms: u32,
    pub text: String,
}

/// Lyrics for a track (M15): synced lines when a provider has them, else plain text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Lyrics {
    pub provider_name: String,
    /// Sorted by time; empty when only plain lyrics are known.
    pub synced: Vec<LyricLine>,
    pub plain: Option<String>,
}

/// One provider's answer to an online search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ProviderSearchResult {
    pub provider: ProviderInfo,
    pub tracks: Vec<OnlineTrack>,
    pub error: Option<String>,
}

/// Everything a search found (M18).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchResults {
    /// Local library tracks matching every word of the query.
    pub library: Vec<LibraryTrack>,
    /// Tracks merged across providers, best first; empty unless two or more providers found some.
    pub top: Vec<OnlineTrack>,
    pub providers: Vec<ProviderSearchResult>,
}

// ---- Favorites, playlists, history (M18) ----

/// A track to save, from the player, the library or a search result.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackInput {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork_url: Option<String>,
    /// Local files and lookups aren't stored; the track is found again by its key.
    pub source: Option<MediaSource>,
}

/// A track as saved in favorites, a playlist or history. Artwork comes from the local library when
/// the same song is there.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedTrack {
    /// The resolver's `artist|title` key: one favorite per key.
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork_url: Option<String>,
    /// A portable stream (http(s) or YouTube), when the track came with one.
    pub source: Option<MediaSource>,
    /// The same song is in this device's library.
    pub local: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub item_count: u32,
    /// Up to four distinct artworks of the first items, for the cover.
    pub artwork_urls: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    pub id: String,
    pub track: SavedTrack,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistDetail {
    pub playlist: PlaylistSummary,
    pub items: Vec<PlaylistItem>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HistoryEntry {
    pub track: SavedTrack,
    /// Unix ms.
    pub played_at: f64,
}

/// Which saved tracks to play.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Collection {
    Favorites,
    #[serde(rename_all = "camelCase")]
    Playlist {
        id: String,
    },
    History,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPlayRequest {
    pub collection: Collection,
    /// Track to start with; otherwise the first (or a random one when shuffling).
    pub start_index: Option<u32>,
    pub shuffle: bool,
}

// ---- MCP server (M19) ----

/// The MCP server's switch and how to reach it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct McpStatus {
    pub enabled: bool,
    /// `http://127.0.0.1:<port>/mcp` while it runs.
    pub url: Option<String>,
    /// The bearer token clients send. Device-local, never synced.
    pub token: String,
    /// Why it isn't running although enabled (e.g. no free port).
    pub error: Option<String>,
}

// ---- LAN remote (M20) ----

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteDevice {
    pub id: String,
    pub name: String,
    /// Unix ms.
    pub paired_at: f64,
}

/// The LAN remote's switch, address, open pairing code and paired devices.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RemoteStatus {
    pub enabled: bool,
    /// `http://<LAN IP>:<port>` while it runs.
    pub url: Option<String>,
    /// The address with the open pairing code, while a code is open.
    pub pair_url: Option<String>,
    /// `pair_url` as a QR code (SVG markup).
    pub pair_qr_svg: Option<String>,
    pub devices: Vec<RemoteDevice>,
    pub error: Option<String>,
}

// ---- Device sync (M21) ----

/// What a user can choose to sync between devices.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SyncKind {
    Settings,
    Library,
    History,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncPeerInfo {
    pub device_id: String,
    pub name: String,
    /// Groups of hex from the device's key, to compare by eye.
    pub fingerprint: String,
    pub address: Option<String>,
    /// Unix ms.
    pub paired_at: f64,
    pub last_sync_at: Option<f64>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncSessionInfo {
    pub id: u32,
    pub peer_name: String,
    /// Unix ms.
    pub started_at: f64,
    pub finished: bool,
    pub sent: u32,
    pub received: u32,
    /// Received rows that changed something on this device.
    pub changed: u32,
    pub error: Option<String>,
    pub undone: bool,
}

/// Device sync's switch, this device's identity, an open pairing, peers and the log.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SyncStatus {
    pub enabled: bool,
    pub device_name: String,
    pub fingerprint: String,
    /// The text another device pastes to pair, while a pairing code is open.
    pub pairing_text: Option<String>,
    /// `pairing_text` as a QR code (SVG markup).
    pub pairing_qr_svg: Option<String>,
    pub kinds: Vec<SyncKind>,
    pub peers: Vec<SyncPeerInfo>,
    pub log: Vec<SyncSessionInfo>,
    pub error: Option<String>,
}

// ---- Manga (M24) ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaSource {
    /// `<extension>/<provider>`.
    pub id: String,
    pub name: String,
    pub extension_id: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MangaBrowseMode {
    Popular,
    Latest,
    Search,
}

/// A title in a source's catalogue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaCard {
    pub source_id: String,
    /// The source's id for the title.
    pub id: String,
    pub title: String,
    /// Through the local image proxy.
    pub cover_url: Option<String>,
    pub in_library: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaListingPage {
    pub items: Vec<MangaCard>,
    pub has_next_page: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaChapterInfo {
    pub id: String,
    pub name: String,
    pub number: Option<f64>,
    pub volume: Option<String>,
    pub scanlator: Option<String>,
    /// Unix ms.
    pub date_upload: Option<f64>,
    pub read: bool,
    /// The page reached, while partly read.
    pub last_page: Option<u32>,
    pub total_pages: Option<u32>,
    /// Set once the chapter was queued for download (M28).
    pub download: Option<ChapterDownload>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DownloadState {
    Queued,
    Downloading,
    Done,
    Failed,
}

impl DownloadState {
    pub fn parse(text: &str) -> Self {
        match text {
            "downloading" => Self::Downloading,
            "done" => Self::Done,
            "failed" => Self::Failed,
            _ => Self::Queued,
        }
    }
}

/// Where a chapter's download stands.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ChapterDownload {
    pub state: DownloadState,
    pub pages_done: u32,
    pub pages_total: Option<u32>,
    pub error: Option<String>,
}

/// One row of the download queue.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaDownloadItem {
    pub chapter_id: String,
    pub title_id: String,
    pub title: String,
    pub chapter: String,
    pub download: ChapterDownload,
}

/// The download queue: chapters waiting, running or failed, plus how many are on the device.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaDownloads {
    pub paused: bool,
    pub items: Vec<MangaDownloadItem>,
    pub downloaded: u32,
}

/// Progress of a library update (M28).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaUpdateStatus {
    pub running: bool,
    pub checked: u32,
    pub total: u32,
    pub new_chapters: u32,
    pub failed: u32,
    /// Unix ms of the last finished run.
    pub last_run_at: Option<f64>,
}

/// A chapter a library update found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaUpdateItem {
    pub title_id: String,
    pub source_id: String,
    pub remote_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub chapter: MangaChapterInfo,
    /// Unix ms.
    pub found_at: f64,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaTitleView {
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub remote_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub authors: Vec<String>,
    pub artists: Vec<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub status: String,
    pub url: Option<String>,
    pub in_library: bool,
    /// An adult title while NSFW content is hidden: the UI shows a notice instead (M35).
    pub hidden_nsfw: bool,
    /// In the source's order (usually newest first).
    pub chapters: Vec<MangaChapterInfo>,
    /// Where "Continue" opens: the partly read chapter, else the first unread after the last read.
    pub continue_chapter_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaLibraryEntry {
    pub title_id: String,
    pub source_id: String,
    pub remote_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    pub chapters: u32,
    pub unread: u32,
    /// For filters and tags (M39).
    pub genres: Vec<String>,
    pub status: String,
    /// Unix ms the title joined the library.
    pub added_at: f64,
    /// Unix ms of the last reading progress.
    pub last_read_at: Option<f64>,
    /// Unix ms of the newest chapter.
    pub latest_chapter_at: Option<f64>,
    /// Some chapter has progress.
    pub started: bool,
    pub downloaded: u32,
    pub category_ids: Vec<String>,
}

/// A user-named shelf in the manga or novel library (M39).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCategory {
    pub id: String,
    pub name: String,
}

/// How a title is read (M27).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReaderMode {
    /// One continuous vertical strip.
    Webtoon,
    /// One page at a time, left to right.
    PagedLtr,
    /// One page at a time, right to left (most manga).
    PagedRtl,
}

impl ReaderMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Webtoon => "webtoon",
            Self::PagedLtr => "pagedLtr",
            Self::PagedRtl => "pagedRtl",
        }
    }

    pub fn parse(text: &str) -> Self {
        match text {
            "pagedLtr" => Self::PagedLtr,
            "pagedRtl" => Self::PagedRtl,
            _ => Self::Webtoon,
        }
    }
}

/// What the reader needs for one chapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaChapterPages {
    pub title_id: String,
    pub title: String,
    pub reader_mode: ReaderMode,
    pub chapter: MangaChapterInfo,
    /// Through the local image proxy, in reading order.
    pub pages: Vec<String>,
    /// The chapter before and after this one in reading order.
    pub previous_chapter_id: Option<String>,
    pub next_chapter_id: Option<String>,
}

// ---- Backups (M30) ----

/// A backup file kept on this device.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    pub name: String,
    pub size_bytes: f64,
    /// Unix ms.
    pub created_at: f64,
}

/// Titles of one Mihon source that Raygun has no source for.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SkippedSource {
    pub name: String,
    pub titles: u32,
}

/// What a Mihon backup import brought in.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MihonImport {
    pub titles: u32,
    pub library_titles: u32,
    pub chapters: u32,
    pub read_chapters: u32,
    pub skipped: Vec<SkippedSource>,
}

/// The outcome of restoring or importing a backup.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum BackupOutcome {
    /// A Raygun backup: rows read and rows that changed here.
    Raygun {
        received: u32,
        changed: u32,
    },
    Mihon {
        summary: MihonImport,
    },
}

// ---- Novels (M31) ----

/// One block of a novel chapter: text structure only, never source HTML.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum TextBlock {
    Paragraph {
        text: String,
    },
    Heading {
        text: String,
    },
    Separator,
    /// Through the local image proxy.
    Image {
        url: String,
    },
}

/// What the novel reader needs for one chapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NovelChapter {
    pub title_id: String,
    pub title: String,
    /// `lastPage` and `totalPages` hold the character offset and length while partly read.
    pub chapter: MangaChapterInfo,
    pub blocks: Vec<TextBlock>,
    pub previous_chapter_id: Option<String>,
    pub next_chapter_id: Option<String>,
}

/// Where an imported book can be opened (M33).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedBook {
    pub source_id: String,
    pub novel_id: String,
}

// ---- Reading aloud (M33) ----

/// One paragraph to read aloud; `id` is the block's index in the chapter.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct TtsParagraph {
    pub id: u32,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TtsState {
    /// Reading the paragraph.
    Speaking,
    /// Finished everything that was queued.
    Ended,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TtsUpdate {
    pub state: TtsState,
    pub paragraph: u32,
}

// ---- AI: translators and models (M34) ----

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AiProviderKind {
    Claude,
    OpenAiCompatible,
    Deepl,
}

/// A configured translator. Its key stays in the secrets store; only a hint is shown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiProvider {
    pub id: String,
    pub kind: AiProviderKind,
    pub name: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    pub has_key: bool,
    /// The key's last four characters.
    pub key_hint: Option<String>,
}

/// A provider to add (no `id`) or change.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiProviderInput {
    pub id: Option<String>,
    pub kind: AiProviderKind,
    pub name: String,
    pub base_url: Option<String>,
    pub model: Option<String>,
    /// `null` keeps the stored key; an empty string removes it.
    pub key: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiModel {
    pub id: String,
    pub name: String,
    pub kind: String,
    pub description: String,
    pub license: String,
    pub license_url: String,
    pub source_url: String,
    pub size_bytes: f64,
    pub installed: bool,
    /// Bytes received while a download is running.
    pub downloading: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AiModelState {
    Downloading,
    Installed,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AiModelProgress {
    pub id: String,
    pub state: AiModelState,
    pub received_bytes: f64,
    pub total_bytes: f64,
    pub error: Option<String>,
}

// ---- Podcasts (M37) ----

/// A podcast in lists: subscriptions and directory results opened in the app.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PodcastSummary {
    pub id: String,
    pub feed_url: String,
    pub title: String,
    pub author: Option<String>,
    pub artwork_url: Option<String>,
    pub episodes: u32,
    pub unplayed: u32,
    /// Unix ms of the newest episode.
    pub latest_published_at: Option<f64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum EpisodeDownloadState {
    Downloading,
    Done,
    Failed,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeDownload {
    pub state: EpisodeDownloadState,
    pub received_bytes: f64,
    pub total_bytes: Option<f64>,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PodcastEpisode {
    pub id: String,
    pub podcast_id: String,
    pub podcast_title: String,
    pub title: String,
    pub description: Option<String>,
    /// Unix ms.
    pub published_at: Option<f64>,
    pub duration_ms: Option<f64>,
    /// Where listening stopped (0 when played or not started).
    pub position_ms: f64,
    pub played: bool,
    pub artwork_url: Option<String>,
    pub video: bool,
    pub download: Option<EpisodeDownload>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PodcastView {
    pub id: String,
    pub feed_url: String,
    pub title: String,
    pub author: Option<String>,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
    pub link: Option<String>,
    pub subscribed: bool,
    /// Newest first.
    pub episodes: Vec<PodcastEpisode>,
}

/// A podcast a directory found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PodcastSearchResult {
    pub title: String,
    pub author: Option<String>,
    pub feed_url: String,
    pub artwork_url: Option<String>,
    pub description: Option<String>,
    pub subscribed: bool,
}

// ---- Watch: video playback (M38) ----

/// A subtitle file the player can load next to a video.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoSubtitle {
    pub url: String,
    pub lang: Option<String>,
    pub label: Option<String>,
    /// `srt`, `vtt`, `ass` or `ttml`; guessed from the URL when missing.
    pub format: Option<String>,
}

/// What to play in the video player.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoRequest {
    /// Stable id of the content, for resuming (`url:<address>` for links).
    pub key: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub url: String,
    /// Sent with every request (Referer, User-Agent...).
    pub headers: std::collections::BTreeMap<String, String>,
    /// `hls` or `dash` when the URL doesn't say.
    pub format: Option<String>,
    pub subtitles: Vec<VideoSubtitle>,
}

/// Something started and not finished.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ContinueWatching {
    pub key: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    /// Unix ms.
    pub updated_at: f64,
    pub watched: bool,
}

// ---- Search across sources and source filters (M40) ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FilterOption {
    pub value: String,
    pub label: String,
}

/// A filter a source offers for its search (Mihon's `FilterList`, simplified).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SourceFilter {
    /// Pick one option.
    Select {
        key: String,
        label: String,
        options: Vec<FilterOption>,
        default: Option<String>,
    },
    /// Tags to include or exclude (three-state chips).
    Tags {
        key: String,
        label: String,
        options: Vec<FilterOption>,
    },
    Toggle {
        key: String,
        label: String,
        default: bool,
    },
}

/// A filter's chosen value, keyed by the filter's `key`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(untagged)]
pub enum FilterValue {
    Toggle(bool),
    Select(String),
    Tags {
        include: Vec<String>,
        exclude: Vec<String>,
    },
}

/// One source's first page of results in a search across sources.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceSearchResult {
    pub source: MangaSource,
    pub items: Vec<MangaCard>,
    pub has_next_page: bool,
    pub error: Option<String>,
}

// ---- Stremio addons (M41) ----

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioAddon {
    pub id: String,
    pub url: String,
    pub name: String,
    pub description: Option<String>,
    pub logo: Option<String>,
    pub version: String,
    /// `catalog`, `meta`, `stream`, `subtitles`...
    pub resources: Vec<String>,
    pub types: Vec<String>,
    pub catalogs: u32,
    pub adult: bool,
    /// Why its manifest couldn't be read, when it couldn't.
    pub error: Option<String>,
}

/// A well-known addon offered with one tap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioSuggestion {
    pub url: String,
    pub name: String,
    pub description: String,
    pub added: bool,
}

/// A movie, series or channel in a catalog.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioItem {
    /// `movie`, `series`, `channel`, `tv`...
    pub kind: String,
    pub id: String,
    pub name: String,
    pub poster: Option<String>,
    /// `poster`, `landscape` or `square`.
    pub poster_shape: String,
    pub release_info: Option<String>,
    pub rating: Option<String>,
}

/// One catalog's first page on the Watch home, in search, or a catalog page.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioCatalogRow {
    pub addon_id: String,
    pub addon_name: String,
    pub kind: String,
    pub catalog_id: String,
    pub name: String,
    pub items: Vec<StremioItem>,
    /// Genre choices the catalog takes.
    pub genres: Vec<String>,
    pub has_more: bool,
    pub error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioVideo {
    /// The id streams are asked for (`tt0903747:1:2`).
    pub id: String,
    pub title: String,
    pub season: Option<u32>,
    pub episode: Option<u32>,
    pub released: Option<String>,
    pub thumbnail: Option<String>,
    pub overview: Option<String>,
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    pub watched: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioMeta {
    pub kind: String,
    pub id: String,
    pub name: String,
    pub poster: Option<String>,
    pub background: Option<String>,
    pub logo: Option<String>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub release_info: Option<String>,
    pub runtime: Option<String>,
    pub rating: Option<String>,
    pub cast: Vec<String>,
    pub director: Vec<String>,
    pub trailer_yt_ids: Vec<String>,
    /// Episodes, by season and number; empty for a movie.
    pub videos: Vec<StremioVideo>,
    /// For a movie: where watching stopped.
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    pub watched: bool,
    pub addon_name: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StremioStreamKind {
    /// Plays in Raygun's player.
    Direct,
    /// Plays in the embedded YouTube player.
    YouTube,
    /// Opens outside Raygun.
    External,
    /// Needs a torrent engine (not supported yet).
    Torrent,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioStream {
    pub kind: StremioStreamKind,
    pub name: Option<String>,
    pub description: Option<String>,
    /// URL, YouTube id, external URL or info hash, by `kind`.
    pub target: String,
    pub headers: std::collections::BTreeMap<String, String>,
    pub subtitles: Vec<VideoSubtitle>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioStreamGroup {
    pub addon_id: String,
    pub addon_name: String,
    pub streams: Vec<StremioStream>,
    pub error: Option<String>,
}

/// What to play: a stream of a movie or episode, with what the player shows.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioPlayRequest {
    pub kind: String,
    /// The movie's id, or the episode's (`tt0903747:1:2`).
    pub video_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub stream: StremioStream,
}
