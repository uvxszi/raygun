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

/// How this copy of Raygun was installed (M46.47), which decides how it updates.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum InstallKind {
    /// The portable exe: updates are a download from the release page.
    Portable,
    /// Installed with the setup exe (NSIS): the new setup runs over it.
    Setup,
    /// Installed with the MSI: the new MSI runs over it.
    Msi,
    /// Android: the new APK, through Android's installer.
    Android,
    /// Anything else (Linux, macOS, a development build): the release page.
    Other,
}

/// Where an update stands (M46.47).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum UpdateState {
    Idle,
    Checking,
    Downloading,
    /// The installer is open (or about to take over).
    Installing,
    /// Android needs "Install unknown apps" for Raygun first; its settings page was opened.
    NeedsPermission,
    Failed,
}

/// Raygun's newest release next to this copy's version (M46.47), from GitHub.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppUpdate {
    pub current: String,
    /// The newest release's version, once checked.
    pub latest: Option<String>,
    /// The newest release is newer than this copy.
    pub available: bool,
    /// The release's notes (Markdown).
    pub notes: Option<String>,
    /// The release's page.
    pub page_url: String,
    pub repo_url: String,
    pub install_kind: InstallKind,
    /// The release file this copy updates itself from; `None` means the page is the way.
    pub asset_name: Option<String>,
    pub asset_size: Option<f64>,
    /// Unix ms of the last check.
    pub checked_at: Option<f64>,
    pub state: UpdateState,
    /// 0 to 1 while downloading.
    pub progress: Option<f64>,
    pub message: Option<String>,
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
    /// Shows NSFW (18+) extensions and content; off by default (M35). Music isn't covered since
    /// M46.36: that's `explicit_music`.
    pub show_nsfw: bool,
    /// Shows songs marked explicit and music sources rated 18+ (M46.36); on by default.
    pub explicit_music: bool,
    /// The video height links are picked by (M42); `None` picks the highest.
    pub preferred_video_quality: Option<u32>,
    /// The colour of buttons, tabs and highlights (M42.1).
    pub accent: Accent,
    /// The screen the app opens on.
    pub start_screen: StartScreen,
    /// Navigation bar labels: on every tab, or on the selected one only.
    pub nav_labels: NavLabels,
    /// Titles over covers (compact, as in Mihon) or under them (comfortable).
    pub cover_style: CoverStyle,
    /// The unread count on library covers.
    pub unread_badges: bool,
    /// The downloaded count on library covers.
    pub download_badges: bool,
    /// Videos pick up where they were left.
    pub resume_videos: bool,
    /// Share of a video, in percent, after which it counts as watched.
    pub watched_percent: u32,
    /// Seconds a double tap or the skip buttons jump in the video player.
    pub seek_seconds: u32,
    /// Subtitles turn on by themselves when a video has them in the subtitle language. Read only
    /// while `VideoPlayerPrefs::subtitle_mode` is unset (on: always, off: for other languages).
    pub auto_subtitles: bool,
    /// Subtitles' language as a BCP 47 tag; empty follows the device's languages, then the
    /// app's (the player knows the device's).
    pub subtitle_language: String,
    /// The next episode plays by itself at the end of one (M44).
    pub autoplay_next: bool,
    /// Seconds before the end the next-episode prompt shows; 0 shows it at the end only (M44).
    pub next_prompt_seconds: u32,
    /// The video player's controls, gestures, tracks and tools (M45).
    pub video_player: VideoPlayerPrefs,
    /// How the manga, novel and video libraries look (M46).
    pub library_display: LibraryDisplayPrefs,
    /// Where downloads and pages are saved outside the app (M46.1).
    pub save: SavePrefs,
    /// Image cache limits and refreshing (M46.2).
    pub cache: CachePrefs,
    /// The built-in browser and its blocker (M46.3).
    pub websites: WebsitePrefs,
    /// How tabs, library tabs and the Home, Browse and More screens are arranged (M46.5).
    pub layout: LayoutPrefs,
    /// The accent when `accent` is `Custom`: `#rrggbb` (M46.7).
    pub accent_color: String,
    /// Covers in library and browse grids, in percent more (or less) than the screen's own size
    /// (M46.48): as many as fit across at that size.
    pub cover_size: i32,
    /// How manga pages fit the reader (M46.7).
    pub manga_reader: MangaReaderPrefs,
    /// How the full music player is laid out (M46.7).
    pub player_view: PlayerViewPrefs,
    /// How scrollbars look everywhere (M46.7).
    pub scrollbars: ScrollbarStyle,
    /// How much buttons, panels and sheets move (M46.7).
    pub motion: MotionLevel,
    /// Each kind of song table's sort, view and columns (M46.8).
    pub track_tables: TrackTablePrefs,
    /// Sizes of the panels the user resizes (M46.8).
    pub panels: PanelPrefs,
    /// The user's own colours over the theme's (M46.8).
    pub colors: CustomColors,
    /// Sources and providers (`<extension>/<provider>`) the user turned off (M46.8).
    pub disabled_sources: Vec<String>,
    /// Loading a source's next page while scrolling (M46.8).
    pub auto_load: AutoLoadPrefs,
    /// Who draws the desktop window's top edge (M46.9).
    pub window_frame: WindowFrame,
    /// Interface size in percent of the size the screen gives (M46.48): 50 to 200. The desktop
    /// app zooms its window; phones scale their text and spacing.
    pub zoom: u32,
    /// Charts and song tags (M46.9).
    pub discovery: DiscoveryPrefs,
    /// Captions for podcasts and audiobooks heard by AI (M46.32).
    pub transcription: TranscriptionPrefs,
    /// What's remembered of listening (M46.41).
    pub privacy: PrivacyPrefs,
    /// Artists and things not to show on Home and in recommendations.
    pub hidden: HiddenPrefs,
    /// How long lyrics stay kept (M46.46).
    pub lyrics_cache: LyricsCachePrefs,
    /// Translations under lyrics (M46.48).
    pub lyrics_translation: LyricsTranslationPrefs,
    /// Sources that didn't answer lately go to the end of Browse and Search (M46.9).
    pub failing_sources_last: bool,
    /// The User-Agent sources send when they don't name one (M46.11); empty for Raygun's default.
    pub user_agent: String,
    /// The equalizer, effects, normalization and crossfade for every song (M46.12).
    pub sound: SoundPrefs,
    /// Music keeps playing when a video, a trailer or read-aloud starts (M46.16); off pauses it.
    pub keep_music_playing: bool,
    /// Opening Raygun shows what played last, paused where it was (M46.16).
    pub resume_on_start: bool,
    /// How good songs sound against the data they use, streaming and downloading.
    pub audio_quality: AudioQualityPrefs,
    /// What the desktop sidebar shows and in what order (M46.29).
    pub rail: RailPrefs,
}

/// The desktop sidebar's pins (M46.29): anything with a page, in the user's order or by when it
/// was last played from or opened. Quick access (M46.41): what's added to the library is pinned
/// too, up to `limit` automatic pins.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RailPrefs {
    pub pins: Vec<RailPin>,
    pub order: RailOrder,
    /// The first run's pins were added (Favorites, Downloaded, Local files and the playlists).
    pub seeded: bool,
    /// Pin what's added to the library (M46.41).
    #[serde(default = "pin_added")]
    pub auto_add: bool,
    /// Automatic pins kept at most, 1–1000; the oldest go first. Pins made by hand stay.
    #[serde(default = "auto_pin_limit")]
    pub limit: u32,
}

fn pin_added() -> bool {
    true
}

fn auto_pin_limit() -> u32 {
    20
}

/// Automatic quick access pins allowed at most (M46.41).
pub const RAIL_AUTO_LIMIT_MAX: u32 = 1000;

impl Default for RailPrefs {
    fn default() -> Self {
        Self {
            pins: Vec::new(),
            order: RailOrder::default(),
            seeded: false,
            auto_add: true,
            limit: auto_pin_limit(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RailPin {
    /// `favorites`, `downloaded`, `localFiles`, `playlist`, `album`, `artist`, `podcast`,
    /// `audiobook`, `manga`, `novel`, `video`, `mix`.
    pub kind: String,
    /// The page it opens: a route and its search parameters.
    pub to: String,
    pub search: std::collections::BTreeMap<String, String>,
    /// Its name and cover as last seen.
    pub name: String,
    pub artwork_url: Option<String>,
    /// Unix seconds; 0 when never.
    pub pinned_at: u32,
    pub played_at: u32,
    pub opened_at: u32,
    /// Pinned because it was added to the library (M46.41), not by hand: it goes when it leaves
    /// the library, or when newer automatic pins pass the limit.
    #[serde(default)]
    pub auto: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum RailOrder {
    #[default]
    Custom,
    /// The last time something was played from it.
    Played,
    /// The last time its page was opened.
    Opened,
}

/// Pins the sidebar keeps, at most.
pub const RAIL_MAX_PINS: usize = 1100;

impl RailPrefs {
    pub fn validate(&self) -> Result<(), String> {
        if self.pins.len() > RAIL_MAX_PINS {
            return Err(format!("the sidebar holds at most {RAIL_MAX_PINS} pins"));
        }
        if let Some(pin) = self.pins.iter().find(|pin| !pin.to.starts_with('/')) {
            return Err(format!("{:?} isn't a page of Raygun", pin.to));
        }
        if !(1..=RAIL_AUTO_LIMIT_MAX).contains(&self.limit) {
            return Err(format!(
                "quick access keeps 1 to {RAIL_AUTO_LIMIT_MAX} automatic pins"
            ));
        }
        Ok(())
    }
}

/// How long lyrics stay kept on the device (M46.46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsCachePrefs {
    /// Lyrics of songs not played for this many days are forgotten; 0 keeps them.
    pub forget_after_days: u32,
    /// Favorites' lyrics stay whatever the days.
    pub keep_favorites: bool,
    /// Downloaded songs' lyrics stay.
    pub keep_downloaded: bool,
    /// Local files' lyrics stay.
    pub keep_local: bool,
}

impl Default for LyricsCachePrefs {
    fn default() -> Self {
        Self {
            forget_after_days: 30,
            keep_favorites: true,
            keep_downloaded: true,
            keep_local: true,
        }
    }
}

/// How many songs have lyrics kept, and transcriptions (M46.46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricsCacheUsage {
    pub songs: u32,
    pub transcriptions: u32,
    /// Songs with translations made on this device or by AI (M46.48).
    #[serde(default)]
    pub translations: u32,
}

/// Translations under lyrics (M46.48).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct LyricsTranslationPrefs {
    /// The language translations show in, a BCP 47 tag; empty follows the app's language.
    pub language: String,
    /// Translations in that language show under the lines by themselves: a lyrics source's, or
    /// one made before. The lyrics view's Translate menu switches it too.
    pub automatic: bool,
    /// Romanized lines show when a source has them.
    pub romanized: bool,
    /// The AI catalogue model "On this device" translates with; empty picks one by language.
    pub local_model: String,
}

impl Default for LyricsTranslationPrefs {
    fn default() -> Self {
        Self {
            language: String::new(),
            automatic: true,
            romanized: false,
            local_model: String::new(),
        }
    }
}

/// Whose kept lyrics a clear takes (M46.46).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LyricsClearScope {
    All,
    /// Songs not in Favorites, a playlist, downloads or local files.
    NotSaved,
    Favorites,
    /// Favorites and playlists.
    Saved,
    Downloaded,
}

/// What's remembered of listening (M46.41); both on by default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase", default)]
pub struct PrivacyPrefs {
    /// Plays go into the listening history (play counts, Made for You's taste).
    pub history: bool,
    /// Where plays started from is kept for Home (Recently played, Your music).
    pub recently_played: bool,
    /// Searches are kept for Search's recent searches.
    pub search_history: bool,
    /// Incognito stays on when Raygun starts again; otherwise it ends with the app (M47).
    pub keep_incognito: bool,
}

impl Default for PrivacyPrefs {
    fn default() -> Self {
        Self {
            history: true,
            recently_played: true,
            search_history: true,
            keep_incognito: false,
        }
    }
}

/// Incognito (M47): while it's on, nothing records what's played, watched, read or searched.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct IncognitoStatus {
    pub on: bool,
    /// When it was turned on (Unix ms), or when Raygun started with it kept on.
    pub since_ms: Option<f64>,
}

/// What the listener asked not to be shown, as Spotify's "Don't play this artist" and "Not
/// interested": artists left out of Home, Made for You, charts, genre rows and recommendations,
/// and things Home doesn't show again. Searches and an artist's own page still find them.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HiddenPrefs {
    pub artists: Vec<HiddenArtist>,
    pub items: Vec<HiddenItem>,
}

/// An artist not shown (Don't show this artist).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HiddenArtist {
    /// As it was shown; compared without case.
    pub name: String,
    /// Unix seconds.
    pub hidden_at: u32,
}

/// Something Home doesn't show again (Not interested).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct HiddenItem {
    /// One of [`HIDDEN_KINDS`].
    pub kind: String,
    /// What it's known by: a song's track key, the address of its page
    /// (`/music/album?id=…&provider=…`), or its feed (`feed:<url>`).
    pub key: String,
    /// Its name, artist or show and cover as last seen, for the list in Settings.
    pub name: String,
    pub subtitle: Option<String>,
    pub artwork_url: Option<String>,
    /// Unix seconds.
    pub hidden_at: u32,
}

/// What can be hidden from Home.
pub const HIDDEN_KINDS: [&str; 14] = [
    "song",
    "album",
    "playlist",
    "mix",
    "artist",
    "podcast",
    "episode",
    "audiobook",
    "chart",
    "tag",
    "manga",
    "novel",
    "video",
    "page",
];
/// Hidden artists and items kept at most; past them the oldest go first.
pub const HIDDEN_ARTISTS_MAX: usize = 1_000;
pub const HIDDEN_ITEMS_MAX: usize = 2_000;

impl HiddenPrefs {
    pub fn validate(&self) -> Result<(), String> {
        if self.artists.len() > HIDDEN_ARTISTS_MAX || self.items.len() > HIDDEN_ITEMS_MAX {
            return Err(format!(
                "at most {HIDDEN_ARTISTS_MAX} artists and {HIDDEN_ITEMS_MAX} other things can be hidden"
            ));
        }
        if self
            .artists
            .iter()
            .any(|artist| artist.name.trim().is_empty() || artist.name.len() > 300)
        {
            return Err("a hidden artist needs a name of up to 300 characters".to_owned());
        }
        if let Some(item) = self.items.iter().find(|item| {
            !HIDDEN_KINDS.contains(&item.kind.as_str())
                || item.key.trim().is_empty()
                || item.key.len() > 2_000
                || item.name.len() > 500
        }) {
            return Err(format!("{:?} can't be hidden", item.name));
        }
        Ok(())
    }
}

/// How good a song should sound against how much data it takes, as Spotify offers it. Sources
/// that give several streams of a song (bitrates, formats) pick the one nearest to it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AudioQuality {
    /// About 64 kbps.
    Low,
    /// About 128 kbps.
    Normal,
    /// About 192 kbps.
    High,
    /// About 320 kbps.
    VeryHigh,
    /// The best a source has, lossless included.
    Best,
}

impl AudioQuality {
    /// The bitrate aimed for, kbps; `None` takes the best there is.
    pub fn target_kbps(self) -> Option<u32> {
        match self {
            Self::Low => Some(64),
            Self::Normal => Some(128),
            Self::High => Some(192),
            Self::VeryHigh => Some(320),
            Self::Best => None,
        }
    }
}

/// The quality for streaming, for streaming on mobile data, and for downloads.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioQualityPrefs {
    pub streaming: AudioQuality,
    /// Used instead of `streaming` while the phone is on mobile data.
    pub cellular: AudioQuality,
    pub download: AudioQuality,
}

impl Default for AudioQualityPrefs {
    fn default() -> Self {
        Self {
            streaming: AudioQuality::High,
            cellular: AudioQuality::Normal,
            download: AudioQuality::VeryHigh,
        }
    }
}

/// Centre frequencies of the equalizer's bands in Hz (M46.12): ten octaves, as most players have.
pub const EQ_FREQUENCIES: [u32; 10] = [31, 62, 125, 250, 500, 1_000, 2_000, 4_000, 8_000, 16_000];
/// The equalizer's and the preamp's range, in tenths of a dB (±12 dB).
pub const EQ_LIMIT: i32 = 120;
/// The longest crossfade, ms.
pub const CROSSFADE_MAX_MS: u32 = 12_000;

/// A reverb, as Android's `PresetReverb` names them (M46.12).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Reverb {
    #[default]
    Off,
    SmallRoom,
    MediumRoom,
    LargeRoom,
    MediumHall,
    LargeHall,
    Plate,
}

/// Sound for every song (M46.12): what the engines apply, plus normalization and crossfade. One
/// stored row; missing fields keep their defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct SoundPrefs {
    pub eq_enabled: bool,
    /// The preset the bands came from ("flat", "rock", ...), or "custom".
    pub eq_preset: String,
    /// Gains for [`EQ_FREQUENCIES`], in tenths of a dB (−120 to 120).
    pub eq_bands: Vec<i32>,
    /// Gain before the equalizer, in tenths of a dB (−120 to 120).
    pub preamp: i32,
    /// A low shelf below 100 Hz, 0 to 100 (100 = +12 dB).
    pub bass_boost: u32,
    pub reverb: Reverb,
    /// −100 (left only) to 100 (right only).
    pub balance: i32,
    /// Both channels mixed into one.
    pub mono: bool,
    /// Songs are brought to `normalize_target` by their measured loudness.
    pub normalize: bool,
    /// Loudness songs are brought to, LUFS (−23 to −8).
    pub normalize_target: i32,
    /// How long one song fades into the next, ms (0 = off, up to [`CROSSFADE_MAX_MS`]).
    pub crossfade_ms: u32,
}

impl Default for SoundPrefs {
    fn default() -> Self {
        Self {
            eq_enabled: false,
            eq_preset: "flat".to_owned(),
            eq_bands: vec![0; EQ_FREQUENCIES.len()],
            preamp: 0,
            bass_boost: 0,
            reverb: Reverb::Off,
            balance: 0,
            mono: false,
            normalize: false,
            normalize_target: -14,
            crossfade_ms: 0,
        }
    }
}

impl SoundPrefs {
    pub fn validate(&self) -> Result<(), String> {
        if self.eq_bands.len() != EQ_FREQUENCIES.len() {
            return Err(format!(
                "the equalizer has {} bands, not {}",
                EQ_FREQUENCIES.len(),
                self.eq_bands.len()
            ));
        }
        if let Some(gain) = self
            .eq_bands
            .iter()
            .chain([&self.preamp])
            .find(|gain| gain.abs() > EQ_LIMIT)
        {
            return Err(format!("{} dB is outside ±12 dB", f64::from(*gain) / 10.0));
        }
        if self.eq_preset.is_empty() || self.eq_preset.len() > 40 {
            return Err("a preset needs a short name".to_owned());
        }
        if self.bass_boost > 100 {
            return Err("bass boost goes up to 100".to_owned());
        }
        if self.balance.abs() > 100 {
            return Err("balance goes from −100 to 100".to_owned());
        }
        if !(-23..=-8).contains(&self.normalize_target) {
            return Err(format!(
                "{} LUFS isn't an offered loudness",
                self.normalize_target
            ));
        }
        if self.crossfade_ms > CROSSFADE_MAX_MS {
            return Err("a crossfade is at most 12 seconds".to_owned());
        }
        Ok(())
    }
}

/// Lyrics a person wrote or corrected for a song (M46.12), replacing what sources have.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct EditedLyrics {
    /// Sorted by time; empty when the lyrics are plain.
    pub synced: Vec<LyricLine>,
    pub plain: Option<String>,
}

/// A song's own sound and lyrics (M46.12), kept by its track key.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct SongEdit {
    /// Where the song starts playing, ms (0 = its start).
    pub start_ms: u32,
    /// Where it stops, ms; `None` = its end.
    pub end_ms: Option<u32>,
    /// Its volume, tenths of a dB (−120 to 120).
    pub gain: i32,
    /// Shifts synced lyrics, ms: positive shows each line later.
    pub lyrics_offset_ms: i32,
    /// Lyrics that replace the sources'.
    pub lyrics: Option<EditedLyrics>,
}

impl SongEdit {
    /// Nothing changed from how the song plays by default.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }

    pub fn validate(&self) -> Result<(), String> {
        if let Some(end) = self.end_ms
            && end <= self.start_ms
        {
            return Err("the song must end after it starts".to_owned());
        }
        if self.gain.abs() > EQ_LIMIT {
            return Err("a song's volume goes from −12 to 12 dB".to_owned());
        }
        if self.lyrics_offset_ms.abs() > 60_000 {
            return Err("lyrics move by at most a minute".to_owned());
        }
        if let Some(lyrics) = &self.lyrics {
            if lyrics
                .synced
                .windows(2)
                .any(|pair| pair[1].time_ms < pair[0].time_ms)
            {
                return Err("lyrics lines must be in time order".to_owned());
            }
            if lyrics.synced.len() > 2_000
                || lyrics
                    .plain
                    .as_ref()
                    .is_some_and(|plain| plain.len() > 100_000)
            {
                return Err("those lyrics are too long".to_owned());
            }
        }
        Ok(())
    }
}

/// Charts and song tags (M46.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct DiscoveryPrefs {
    /// The charts' country, an ISO 3166 code in lowercase ("gb"); empty follows the app language.
    pub region: String,
    /// Find genres and moods for saved songs in the background.
    pub tag_songs: bool,
}

impl Default for DiscoveryPrefs {
    fn default() -> Self {
        Self {
            region: String::new(),
            tag_songs: true,
        }
    }
}

/// Who hears podcasts and audiobooks for captions (M46.32).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TranscriptionEngine {
    Off,
    /// Whisper on this device, from the AI models.
    Local,
    /// An OpenAI-compatible translator's `audio/transcriptions`.
    Online,
}

/// Captions heard by AI (M46.32).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct TranscriptionPrefs {
    pub engine: TranscriptionEngine,
    /// The AI model used on this device (`whisper-tiny`).
    pub local_model: String,
    /// The OpenAI-compatible provider (from Settings → AI) used online.
    pub provider_id: Option<String>,
    /// The online model (`whisper-1`, `whisper-large-v3`…).
    pub online_model: String,
    /// Online: confirmed once that batches cost money.
    pub online_confirmed: bool,
}

impl Default for TranscriptionPrefs {
    fn default() -> Self {
        Self {
            engine: TranscriptionEngine::Off,
            local_model: "whisper-tiny".into(),
            provider_id: None,
            online_model: "whisper-1".into(),
            online_confirmed: false,
        }
    }
}

/// One caption line, timed in milliseconds.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Caption {
    pub start_ms: u32,
    pub end_ms: u32,
    pub text: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CaptionSource {
    /// The feed's own transcript.
    Feed,
    Local,
    Online,
}

/// An episode's captions and where transcribing stands (M46.32).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Captions {
    pub episode_id: String,
    pub cues: Vec<Caption>,
    pub source: Option<CaptionSource>,
    /// Stretches with captions, `[start, end]` in ms.
    pub covered: Vec<(u32, u32)>,
    pub complete: bool,
    /// Transcribing now.
    pub running: bool,
    /// Getting the audio before the first batch.
    pub preparing: bool,
    pub error: Option<String>,
    /// Transcription is set up (a model downloaded, or a provider picked).
    pub can_transcribe: bool,
    pub engine: TranscriptionEngine,
    /// The episode's feed has a transcript of its own (M46.36: the listener can go back to it).
    #[serde(default)]
    pub has_feed: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SpokenKind {
    Podcast,
    Audiobook,
}

/// What a queue item is when it's spoken word: a podcast episode or a book's chapter (M46.32).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SpokenItem {
    pub kind: SpokenKind,
    pub podcast_id: String,
    pub episode_id: String,
    /// The show's or the book's title.
    pub show: String,
    pub author: Option<String>,
}

/// Interface sizes offered, in percent of the screen's (M46.9, M46.48).
pub const ZOOM_LEVELS: std::ops::RangeInclusive<u32> = 50..=200;

/// Next pages load as a list's end comes near (M46.8), at most one per `cooldown_ms`, so sources
/// don't rate-limit the user; off, a Load more button stays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct AutoLoadPrefs {
    pub enabled: bool,
    /// 0 to 10 000 ms.
    pub cooldown_ms: u32,
}

impl Default for AutoLoadPrefs {
    fn default() -> Self {
        Self {
            enabled: true,
            cooldown_ms: 1_500,
        }
    }
}

/// Colours the user picked instead of the theme's (M46.8): each `#rrggbb`, or empty for the
/// theme's own. The accent has its own setting.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct CustomColors {
    /// The page behind everything.
    pub background: String,
    /// Cards, bars, panels and sheets.
    pub surface: String,
    /// Text and icons.
    pub foreground: String,
    /// Secondary text and icons.
    pub muted: String,
}

impl CustomColors {
    pub fn validate(&self) -> Result<(), String> {
        for color in [
            &self.background,
            &self.surface,
            &self.foreground,
            &self.muted,
        ] {
            if !color.is_empty() && !is_hex_color(color) {
                return Err(format!("{color} isn't a #rrggbb colour"));
            }
        }
        Ok(())
    }
}

/// What a song table is sorted by (M46.8). `Custom` is the collection's own order.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TrackSortKey {
    #[default]
    Custom,
    Title,
    Artist,
    Album,
    Added,
    Duration,
    Plays,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TrackListView {
    /// Artwork and two lines.
    #[default]
    List,
    /// One line each, no artwork.
    Compact,
}

/// A song table's optional columns; the title (with artists) always shows.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TrackColumnId {
    Album,
    Added,
    Plays,
    Duration,
    /// The song's genres and moods (M46.9).
    Tags,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackColumn {
    pub id: TrackColumnId,
    pub visible: bool,
    /// In px; 0 lets the table choose.
    pub width: u32,
}

/// One kind of song table's look (M46.8).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackTableView {
    pub sort: TrackSortKey,
    pub descending: bool,
    pub view: TrackListView,
    /// In display order.
    pub columns: Vec<TrackColumn>,
}

impl Default for TrackTableView {
    fn default() -> Self {
        let column = |id, visible| TrackColumn {
            id,
            visible,
            width: 0,
        };
        Self {
            sort: TrackSortKey::Custom,
            descending: false,
            view: TrackListView::List,
            columns: vec![
                column(TrackColumnId::Album, true),
                column(TrackColumnId::Added, true),
                column(TrackColumnId::Plays, false),
                column(TrackColumnId::Tags, false),
                column(TrackColumnId::Duration, true),
            ],
        }
    }
}

/// The song tables' looks by kind: `favorites`, `playlist`, `library`, `album`, `artist`,
/// `history`, `search` (M46.8), `downloaded` (M46.18). Kinds not set use
/// `TrackTableView::default()`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct TrackTablePrefs {
    pub tables: std::collections::BTreeMap<String, TrackTableView>,
}

/// The kinds of song table.
pub const TRACK_TABLE_KINDS: [&str; 8] = [
    "favorites",
    "downloaded",
    "playlist",
    "library",
    "album",
    "artist",
    "history",
    "search",
];

impl TrackTablePrefs {
    pub fn validate(&self) -> Result<(), String> {
        for (kind, view) in &self.tables {
            if !TRACK_TABLE_KINDS.contains(&kind.as_str()) {
                return Err(format!("{kind} isn't a song table"));
            }
            let mut seen = Vec::new();
            for column in &view.columns {
                if seen.contains(&column.id) {
                    return Err("A column shows up twice".into());
                }
                seen.push(column.id);
                if column.width != 0 && !(40..=800).contains(&column.width) {
                    return Err(format!("{} px isn't a column width", column.width));
                }
            }
        }
        Ok(())
    }
}

/// Panel sizes the user dragged (M46.8); 0 is the default for the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct PanelPrefs {
    /// The side rail, 72 (covers only) to 360 px.
    pub rail_width: u32,
    /// The Now playing panel, 280 to 560 px.
    pub side_panel_width: u32,
    /// The full player's left column, 30 to 70 % (side by side).
    pub player_split: u32,
    /// Songs the side panel's "Next in queue" card shows, 1 to 10 (M46.24).
    pub next_up_rows: u32,
}

impl PanelPrefs {
    pub fn validate(&self) -> Result<(), String> {
        let within = |value: u32, low: u32, high: u32| value == 0 || (low..=high).contains(&value);
        if !within(self.rail_width, 72, 360) {
            return Err(format!("{} px isn't a rail width", self.rail_width));
        }
        if !within(self.side_panel_width, 280, 560) {
            return Err(format!("{} px isn't a panel width", self.side_panel_width));
        }
        if !within(self.player_split, 30, 70) {
            return Err(format!("{}% isn't a player split", self.player_split));
        }
        if !within(self.next_up_rows, 1, 10) {
            return Err(format!(
                "{} isn't a number of next-up rows",
                self.next_up_rows
            ));
        }
        Ok(())
    }
}

/// Animations (M46.7): buttons that answer hover and press, panels and popovers that slide and
/// fade in. The system's reduced-motion setting still turns the movement off.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MotionLevel {
    Off,
    Calm,
    #[default]
    Lively,
}

/// The desktop window's top edge (M46.9): Raygun's own bar (menu, back and forward, home, search
/// and the window's buttons, as Spotify's), or the system's title bar with Raygun's bar under it.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum WindowFrame {
    #[default]
    App,
    System,
}

/// Scrollbars (M46.7): thin over a clear track by default, the system's width, or none at all
/// (scrolling still works).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ScrollbarStyle {
    #[default]
    Thin,
    Wide,
    Hidden,
}

/// How the full player arranges itself (M46.7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PlayerLayout {
    /// Side by side on wide windows, one above the other on phones.
    #[default]
    Auto,
    /// The artwork, lyrics or queue above the controls.
    Stacked,
    /// The artwork and controls beside the lyrics or queue.
    SideBySide,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TextAlign {
    #[default]
    Left,
    Center,
}

/// The full music player's layout (M46.7). One stored row; missing fields keep their defaults.
///
/// Sizes start from the screen: the cover is as large as fits beside the controls, the column as
/// wide as the window suits, lyrics as large as their column. Each offset moves away from that
/// size, so the player stays right on any window, zoom or orientation.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct PlayerViewPrefs {
    pub layout: PlayerLayout,
    /// The cover, in percent larger or smaller than what fits ([`PLAYER_PERCENT_OFFSETS`]).
    pub cover_offset: i32,
    /// The player's width, in percent wider or narrower than the screen's ([`PLAYER_PERCENT_OFFSETS`]).
    pub width_offset: i32,
    /// Lyrics text, in px (at 100 % zoom) larger or smaller than by screen ([`LYRICS_OFFSETS`]).
    pub lyrics_offset: i32,
    pub lyrics_align: TextAlign,
    /// The artwork, blurred, behind the player.
    pub background: bool,
}

/// How far the cover and the player's width move from their size by screen, in percent.
pub const PLAYER_PERCENT_OFFSETS: std::ops::RangeInclusive<i32> = -40..=40;
/// How far lyrics text moves from its size by screen, in px.
pub const LYRICS_OFFSETS: std::ops::RangeInclusive<i32> = -5..=10;

impl Default for PlayerViewPrefs {
    fn default() -> Self {
        Self {
            layout: PlayerLayout::Auto,
            cover_offset: 0,
            width_offset: 0,
            lyrics_offset: 0,
            lyrics_align: TextAlign::Left,
            background: true,
        }
    }
}

impl PlayerViewPrefs {
    pub fn validate(&self) -> Result<(), String> {
        for (name, offset) in [("cover", self.cover_offset), ("width", self.width_offset)] {
            if !PLAYER_PERCENT_OFFSETS.contains(&offset) {
                return Err(format!("{offset}% is out of range for the player's {name}"));
            }
        }
        if !LYRICS_OFFSETS.contains(&self.lyrics_offset) {
            return Err(format!(
                "{} px is out of range for lyrics",
                self.lyrics_offset
            ));
        }
        Ok(())
    }
}

/// Reads a stored row, also one from before the offsets: a width in percent of the window and
/// a lyrics size in px (0 meant "by screen") become the nearest offsets.
impl<'de> Deserialize<'de> for PlayerViewPrefs {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(default, rename_all = "camelCase")]
        struct Stored {
            layout: PlayerLayout,
            cover_offset: Option<i32>,
            width_offset: Option<i32>,
            lyrics_offset: Option<i32>,
            lyrics_align: TextAlign,
            background: Option<bool>,
            /// Before the offsets: 40–100 % of the window.
            width: Option<u32>,
            /// Before the offsets: 14–48 px.
            lyrics_size: Option<u32>,
        }
        impl Default for Stored {
            fn default() -> Self {
                Self {
                    layout: PlayerLayout::Auto,
                    cover_offset: None,
                    width_offset: None,
                    lyrics_offset: None,
                    lyrics_align: TextAlign::Left,
                    background: None,
                    width: None,
                    lyrics_size: None,
                }
            }
        }
        let stored = Stored::deserialize(deserializer)?;
        // The by-screen column was about three quarters of a desktop window, and lyrics about
        // 20 px on a phone: an old choice keeps its direction and roughly its size.
        let width_offset = stored.width_offset.unwrap_or_else(|| match stored.width {
            Some(width) if width > 0 => (((i64::from(width) - 75) * 4) / 3) as i32,
            _ => 0,
        });
        let lyrics_offset = stored
            .lyrics_offset
            .unwrap_or_else(|| match stored.lyrics_size {
                Some(size) if size > 0 => (i64::from(size) - 20) as i32,
                _ => 0,
            });
        let clamp = |offset: i32, range: &std::ops::RangeInclusive<i32>| {
            offset.clamp(*range.start(), *range.end())
        };
        Ok(Self {
            layout: stored.layout,
            cover_offset: clamp(stored.cover_offset.unwrap_or(0), &PLAYER_PERCENT_OFFSETS),
            width_offset: clamp(width_offset, &PLAYER_PERCENT_OFFSETS),
            lyrics_offset: clamp(lyrics_offset, &LYRICS_OFFSETS),
            lyrics_align: stored.lyrics_align,
            background: stored.background.unwrap_or(true),
        })
    }
}

/// Cover sizes offered, in percent more or less than the screen's own (M46.48).
pub const COVER_SIZES: std::ops::RangeInclusive<i32> = -50..=100;

/// How a page fits the screen in the paged reader modes (M46.7).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PagedScale {
    /// All of the page on screen.
    #[default]
    FitScreen,
    /// The screen's width; tall pages scroll.
    FitWidth,
    /// The screen's height; wide pages scroll.
    FitHeight,
    /// The image's own size.
    Original,
}

/// Where taps turn pages in the manga reader (Mihon's navigation layouts).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TapZones {
    /// The sides in the paged modes, the L shape in the vertical strip.
    #[default]
    Auto,
    /// The top and the left of the middle go back; the right of the middle and the bottom on.
    LShaped,
    /// The top shows the controls; the left third goes back, the rest on.
    Kindle,
    /// Both sides go on; the bottom of the middle goes back.
    Edge,
    /// The left third and the right third, as the reading direction has them.
    Sides,
    /// Taps only show and hide the controls.
    Off,
}

/// How the tap zones are mirrored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TapInvert {
    #[default]
    None,
    Horizontal,
    Vertical,
    Both,
}

/// The manga reader's display (M46.7). One stored row; missing fields keep their defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct MangaReaderPrefs {
    pub paged_scale: PagedScale,
    /// The webtoon column, in percent more or less than the screen's own (M46.48): the screen's
    /// width on phones, at most 768 px on wide windows; never wider than the window.
    pub webtoon_size: i32,
    /// The page bar, in percent more or less than the screen's own: the full width on phones,
    /// 70 % on wide windows; never wider than the window.
    pub bar_size: i32,
    /// The page bar's buttons beside the page slider.
    pub bar_chapters: bool,
    pub bar_zoom: bool,
    pub bar_mode: bool,
    pub bar_fullscreen: bool,
    /// Pages loaded ahead of the one being read, 1–10.
    pub preload_pages: u32,
    /// How titles without a reading mode of their own are read.
    pub default_mode: ReaderMode,
    pub tap_zones: TapZones,
    pub invert_taps: TapInvert,
    /// A double tap zooms; single taps then wait a moment to tell the two apart.
    pub double_tap_zoom: bool,
    /// The page number stays on screen while the controls are hidden.
    pub page_number: bool,
    /// The screen doesn't turn off while a chapter is open.
    pub keep_screen_on: bool,
    /// Next and previous chapter skip other uploads of the same chapter number.
    pub skip_duplicates: bool,
}

impl Default for MangaReaderPrefs {
    fn default() -> Self {
        Self {
            paged_scale: PagedScale::FitScreen,
            webtoon_size: 0,
            bar_size: 0,
            bar_chapters: true,
            bar_zoom: true,
            bar_mode: true,
            bar_fullscreen: true,
            preload_pages: 3,
            default_mode: ReaderMode::Webtoon,
            tap_zones: TapZones::Auto,
            invert_taps: TapInvert::None,
            double_tap_zoom: true,
            page_number: true,
            keep_screen_on: true,
            skip_duplicates: true,
        }
    }
}

impl MangaReaderPrefs {
    pub fn validate(&self) -> Result<(), String> {
        if !READER_SIZES.contains(&self.webtoon_size) {
            return Err(format!(
                "{}% isn't an offered webtoon size",
                self.webtoon_size
            ));
        }
        if !READER_SIZES.contains(&self.bar_size) {
            return Err(format!("{}% isn't an offered page bar size", self.bar_size));
        }
        if !(1..=10).contains(&self.preload_pages) {
            return Err("Load between 1 and 10 pages ahead".into());
        }
        Ok(())
    }
}

/// The manga reader's column and page bar sizes, in percent more or less than the screen's.
pub const READER_SIZES: std::ops::RangeInclusive<i32> = -70..=100;

/// Whether `color` is a `#rrggbb` colour.
fn is_hex_color(color: &str) -> bool {
    color.len() == 7
        && color.starts_with('#')
        && color[1..].chars().all(|char| char.is_ascii_hexdigit())
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Accent {
    #[default]
    Red,
    Orange,
    Green,
    Teal,
    Blue,
    Purple,
    Pink,
    /// `Settings::accent_color` (M46.7).
    Custom,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StartScreen {
    #[default]
    Home,
    Library,
    Browse,
    /// Tabs that can be added (M46.5).
    Watch,
    Music,
    Comics,
    Books,
    Podcasts,
    Search,
    Updates,
    Downloads,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NavLabels {
    #[default]
    Always,
    Selected,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum CoverStyle {
    #[default]
    Compact,
    Comfortable,
}

/// The double-tap seek steps offered, in seconds.
pub const SEEK_SECONDS: [u32; 5] = [5, 10, 15, 30, 60];

/// When the next-episode prompt can show, in seconds before the end (0: at the end).
pub const NEXT_PROMPT_SECONDS: [u32; 5] = [0, 5, 10, 15, 30];

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

/// The novel reader's page colours: the app's, or a page of its own.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum NovelTheme {
    #[default]
    App,
    Light,
    Sepia,
    Dark,
    Black,
}

/// The novel reader's typography and layout.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct NovelReaderPrefs {
    /// Text size in CSS pixels more or less than the screen's own (M46.48), −6 to 12.
    #[serde(default)]
    pub text_size: i32,
    /// Line height in tenths, 12–22 (1.2–2.2).
    pub line_spacing: u32,
    pub font: NovelFont,
    pub width: NovelWidth,
    /// Pages turned sideways instead of one scrolling column.
    pub paged: bool,
    /// Reading-aloud speed in tenths, 5–20 (0.5×–2×).
    #[serde(default = "normal_rate")]
    pub tts_rate: u32,
    /// The first line of each paragraph starts further in, as in printed books (M46.10).
    #[serde(default)]
    pub indent: bool,
    #[serde(default)]
    pub theme: NovelTheme,
    /// Lines run to both edges, words hyphenated.
    #[serde(default)]
    pub justify: bool,
    /// The screen doesn't turn off while a chapter is open.
    #[serde(default = "screen_kept_on")]
    pub keep_screen_on: bool,
}

/// Novel text sizes offered, in CSS pixels more or less than the screen's own (M46.48).
pub const NOVEL_TEXT_SIZES: std::ops::RangeInclusive<i32> = -6..=12;

fn normal_rate() -> u32 {
    10
}

fn screen_kept_on() -> bool {
    true
}

impl Default for NovelReaderPrefs {
    fn default() -> Self {
        Self {
            text_size: 0,
            line_spacing: 16,
            font: NovelFont::Serif,
            width: NovelWidth::Narrow,
            paged: false,
            tts_rate: 10,
            indent: false,
            theme: NovelTheme::App,
            justify: false,
            keep_screen_on: true,
        }
    }
}

/// How the video player turns (M45, Aniyomi's choices).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PlayerOrientation {
    /// Follows the device.
    Free,
    /// Landscape for wide videos, portrait for tall ones.
    Video,
    Portrait,
    ReversePortrait,
    SensorPortrait,
    Landscape,
    ReverseLandscape,
    #[default]
    SensorLandscape,
}

/// How the picture fills the screen.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum VideoAspect {
    /// All of the picture, with bars.
    #[default]
    Fit,
    /// The screen filled, the edges cut.
    Crop,
    /// The screen filled, the picture stretched.
    Stretch,
}

/// What a double tap on one part of the player does.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum TapAction {
    Seek,
    PlayPause,
    /// The previous episode on the left, the next on the right.
    SwitchEpisode,
    Off,
}

/// What the media keys' previous and next do (headsets, keyboards, remotes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MediaKeyAction {
    #[default]
    SwitchEpisode,
    Seek,
}

/// Which decoders play video.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum DecoderMode {
    /// Hardware, falling back to software when it fails.
    #[default]
    Auto,
    /// Software first: slower, but plays what the hardware can't.
    Software,
}

/// The edge drawn around subtitle text.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SubtitleEdge {
    None,
    #[default]
    Outline,
    Shadow,
    Raised,
    Depressed,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SubtitleFont {
    #[default]
    Default,
    Sans,
    Serif,
    Mono,
}

/// When the video player turns subtitles on by itself, in the first subtitle language a video
/// has (mpv's `subs-with-matching-audio`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SubtitleMode {
    /// When the audio isn't in one of the subtitle languages; forced subtitles otherwise.
    #[default]
    Foreign,
    /// Whenever the video has subtitles in one of them.
    Always,
    /// Only when turned on in the player.
    Never,
}

/// `VideoPlayerPrefs::audio_language` for the video's own track (the one marked default).
pub const ORIGINAL_AUDIO: &str = "original";

/// How subtitles look in the video player (M45).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct SubtitleStyle {
    /// Percent of the normal size, 50–200.
    pub size: u32,
    pub font: SubtitleFont,
    pub bold: bool,
    pub italic: bool,
    /// Colours are 0xAARRGGBB.
    pub text_color: u32,
    pub edge: SubtitleEdge,
    pub edge_color: u32,
    /// Behind each line; fully transparent by default.
    pub background_color: u32,
    /// Distance from the bottom, in percent of the picture's height, 0–40.
    pub position: u32,
    /// Ignores the colours, fonts and positions a subtitle file sets itself.
    pub ignore_embedded: bool,
}

impl Default for SubtitleStyle {
    fn default() -> Self {
        Self {
            size: 100,
            font: SubtitleFont::Default,
            bold: false,
            italic: false,
            text_color: 0xFFFF_FFFF,
            edge: SubtitleEdge::Outline,
            edge_color: 0xFF00_0000,
            background_color: 0,
            position: 8,
            ignore_embedded: false,
        }
    }
}

/// The video player's controls, gestures, tracks and tools (M45). One stored row; fields missing
/// from it keep their defaults, so later fields need no migration.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct VideoPlayerPrefs {
    pub orientation: PlayerOrientation,
    /// For titles with no aspect of their own yet.
    pub aspect: VideoAspect,
    /// Seconds the controls stay up.
    pub controls_seconds: u32,
    /// The time left instead of the length.
    pub show_remaining: bool,
    /// Speeds are in hundredths (100 = 1×), 25–400.
    pub default_speed: u32,
    /// The speeds the speed chip goes through.
    pub speed_presets: Vec<u32>,
    /// Faster speech keeps its pitch.
    pub pitch_correction: bool,
    /// The speed while the picture is held down; 0 turns that off.
    pub hold_speed: u32,
    pub remember_brightness: bool,

    pub brightness_gesture: bool,
    pub volume_gesture: bool,
    /// Volume on the left, brightness on the right.
    pub swap_sides: bool,
    /// A sideways swipe seeks.
    pub seek_gesture: bool,
    pub double_tap_left: TapAction,
    pub double_tap_center: TapAction,
    pub double_tap_right: TapAction,
    pub media_keys: MediaKeyAction,
    /// Percent the volume may go past 100 %, 0–100.
    pub volume_boost: u32,

    /// The skip button: "Skip opening" in a chapter, else the intro length.
    pub skip_button: bool,
    /// The skip button's jump outside chapters, for titles without their own.
    pub intro_seconds: u32,
    /// Openings, endings and recaps are skipped at once.
    pub auto_skip: bool,
    /// A countdown on the skip button skips unless cancelled (Netflix style).
    pub skip_countdown: bool,
    pub skip_wait_seconds: u32,
    /// Openings and endings of anime from AniSkip.
    pub aniskip: bool,
    /// No AniSkip lookup when the source gives chapters.
    pub aniskip_unless_chapters: bool,

    /// Subtitles' second language (the first is `Settings::subtitle_language`).
    pub subtitle_fallback: String,
    /// Comma-separated words that make a subtitle track preferred ("Full, Dialogue").
    pub subtitle_prefer_words: String,
    /// Comma-separated words that keep a subtitle track from being picked ("Signs, Songs").
    pub subtitle_skip_words: String,
    /// When subtitles turn on by themselves. Unset follows `Settings::auto_subtitles`, the
    /// older switch: on is `Always`, off is `Foreign`.
    pub subtitle_mode: Option<SubtitleMode>,
    pub subtitle_style: SubtitleStyle,
    /// The audio track's language: empty follows the device's languages, then the app's;
    /// `ORIGINAL_AUDIO` plays the video's own default track; else a BCP 47 tag. Without a
    /// match, the track marked default (or original) plays, else the first.
    pub audio_language: String,
    pub audio_fallback: String,

    /// Leaving the app while a video plays keeps it in a small window.
    pub pip_on_leave: bool,
    /// Back shrinks the player into a small window instead of closing it.
    pub pip_on_back: bool,

    pub decoder: DecoderMode,
    /// Videos open in another app instead.
    pub external_player: bool,
    /// That app's package; empty asks each time.
    pub external_package: String,
    /// Movie and series pages play their trailer in a small window (M46.34).
    pub title_trailers: bool,
    /// The player's Sources list shows servers that failed or had nothing.
    pub show_failed_sources: bool,
    pub show_empty_sources: bool,
}

impl Default for VideoPlayerPrefs {
    fn default() -> Self {
        Self {
            orientation: PlayerOrientation::SensorLandscape,
            aspect: VideoAspect::Fit,
            controls_seconds: 4,
            show_remaining: false,
            default_speed: 100,
            speed_presets: SPEED_PRESETS.to_vec(),
            pitch_correction: true,
            hold_speed: 200,
            remember_brightness: false,
            brightness_gesture: true,
            volume_gesture: true,
            swap_sides: false,
            seek_gesture: true,
            double_tap_left: TapAction::Seek,
            double_tap_center: TapAction::PlayPause,
            double_tap_right: TapAction::Seek,
            media_keys: MediaKeyAction::SwitchEpisode,
            volume_boost: 100,
            skip_button: true,
            intro_seconds: 85,
            auto_skip: false,
            skip_countdown: false,
            skip_wait_seconds: 5,
            aniskip: false,
            aniskip_unless_chapters: true,
            subtitle_fallback: String::new(),
            subtitle_prefer_words: String::new(),
            subtitle_skip_words: String::new(),
            subtitle_mode: None,
            subtitle_style: SubtitleStyle::default(),
            audio_language: String::new(),
            audio_fallback: String::new(),
            pip_on_leave: true,
            pip_on_back: false,
            decoder: DecoderMode::Auto,
            external_player: false,
            external_package: String::new(),
            title_trailers: true,
            show_failed_sources: true,
            show_empty_sources: true,
        }
    }
}

/// The speed chip's speeds unless changed, in hundredths.
pub const SPEED_PRESETS: [u32; 12] = [25, 50, 75, 100, 125, 150, 175, 200, 250, 300, 350, 400];

/// How long the player's controls can stay up, in seconds.
pub const CONTROLS_SECONDS: [u32; 5] = [2, 3, 4, 5, 8];

/// The speeds holding the picture can play at (0: off), in hundredths.
pub const HOLD_SPEEDS: [u32; 4] = [0, 150, 200, 300];

/// How long the skip button counts down, in seconds.
pub const SKIP_WAIT_SECONDS: [u32; 4] = [3, 5, 7, 10];

impl VideoPlayerPrefs {
    /// Checks values the type system can't express.
    pub fn validate(&self) -> Result<(), String> {
        let speed = |value: u32| (25..=400).contains(&value);
        if !CONTROLS_SECONDS.contains(&self.controls_seconds) {
            return Err(format!(
                "{} s isn't an offered controls timeout",
                self.controls_seconds
            ));
        }
        if !speed(self.default_speed) {
            return Err("Speeds go from 0.25× to 4×".into());
        }
        if self.speed_presets.is_empty()
            || self.speed_presets.len() > 12
            || !self.speed_presets.iter().copied().all(speed)
        {
            return Err("Speed presets are 1 to 12 speeds from 0.25× to 4×".into());
        }
        if !HOLD_SPEEDS.contains(&self.hold_speed) {
            return Err("That hold speed isn't offered".into());
        }
        if self.volume_boost > 100 || !self.volume_boost.is_multiple_of(10) {
            return Err(format!(
                "{} % isn't an offered volume boost",
                self.volume_boost
            ));
        }
        if !(5..=600).contains(&self.intro_seconds) {
            return Err("Intros are 5 seconds to 10 minutes long".into());
        }
        if !SKIP_WAIT_SECONDS.contains(&self.skip_wait_seconds) {
            return Err("That countdown isn't offered".into());
        }
        let style = &self.subtitle_style;
        if !(50..=200).contains(&style.size) || style.position > 40 {
            return Err("That subtitle size or position isn't offered".into());
        }
        for tag in [
            &self.subtitle_fallback,
            &self.audio_language,
            &self.audio_fallback,
        ] {
            if !tag.is_empty() {
                language_tag(tag)?;
            }
        }
        let words = [&self.subtitle_prefer_words, &self.subtitle_skip_words];
        if words.iter().any(|list| list.len() > 200) || self.external_package.len() > 200 {
            return Err("That text is too long".into());
        }
        Ok(())
    }
}

/// The update intervals offered, in hours (0 = off).
pub const MANGA_UPDATE_HOURS: [u32; 6] = [0, 6, 12, 24, 48, 168];

/// The preferred video qualities offered, as heights in pixels (M42).
pub const VIDEO_QUALITIES: [u32; 4] = [1080, 720, 480, 360];

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: Theme::System,
            language: SYSTEM_LANGUAGE.into(),
            manga_update_hours: 12,
            novel_reader: NovelReaderPrefs::default(),
            show_nsfw: false,
            explicit_music: true,
            preferred_video_quality: None,
            accent: Accent::Red,
            start_screen: StartScreen::Home,
            nav_labels: NavLabels::Always,
            cover_style: CoverStyle::Compact,
            unread_badges: true,
            download_badges: true,
            resume_videos: true,
            watched_percent: 90,
            seek_seconds: 10,
            auto_subtitles: false,
            subtitle_language: String::new(),
            autoplay_next: true,
            next_prompt_seconds: 10,
            video_player: VideoPlayerPrefs::default(),
            library_display: LibraryDisplayPrefs::default(),
            save: SavePrefs::default(),
            cache: CachePrefs::default(),
            websites: WebsitePrefs::default(),
            layout: LayoutPrefs::default(),
            accent_color: "#e5484d".into(),
            cover_size: 0,
            manga_reader: MangaReaderPrefs::default(),
            player_view: PlayerViewPrefs::default(),
            scrollbars: ScrollbarStyle::Thin,
            motion: MotionLevel::Lively,
            track_tables: TrackTablePrefs::default(),
            panels: PanelPrefs::default(),
            colors: CustomColors::default(),
            disabled_sources: Vec::new(),
            auto_load: AutoLoadPrefs::default(),
            window_frame: WindowFrame::App,
            zoom: 100,
            discovery: DiscoveryPrefs::default(),
            transcription: TranscriptionPrefs::default(),
            privacy: PrivacyPrefs::default(),
            hidden: HiddenPrefs::default(),
            lyrics_cache: LyricsCachePrefs::default(),
            lyrics_translation: LyricsTranslationPrefs::default(),
            failing_sources_last: true,
            user_agent: String::new(),
            sound: SoundPrefs::default(),
            keep_music_playing: false,
            resume_on_start: true,
            audio_quality: AudioQualityPrefs::default(),
            rail: RailPrefs::default(),
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
            SettingChange::ExplicitMusic(show) => self.explicit_music = show,
            SettingChange::PreferredVideoQuality(height) => self.preferred_video_quality = height,
            SettingChange::Accent(accent) => self.accent = accent,
            SettingChange::StartScreen(screen) => self.start_screen = screen,
            SettingChange::NavLabels(labels) => self.nav_labels = labels,
            SettingChange::CoverStyle(style) => self.cover_style = style,
            SettingChange::UnreadBadges(show) => self.unread_badges = show,
            SettingChange::DownloadBadges(show) => self.download_badges = show,
            SettingChange::ResumeVideos(resume) => self.resume_videos = resume,
            SettingChange::WatchedPercent(percent) => self.watched_percent = percent,
            SettingChange::SeekSeconds(seconds) => self.seek_seconds = seconds,
            SettingChange::AutoSubtitles(on) => self.auto_subtitles = on,
            SettingChange::SubtitleLanguage(tag) => self.subtitle_language = tag,
            SettingChange::AutoplayNext(on) => self.autoplay_next = on,
            SettingChange::NextPromptSeconds(seconds) => self.next_prompt_seconds = seconds,
            SettingChange::VideoPlayer(prefs) => self.video_player = *prefs,
            SettingChange::LibraryDisplay(prefs) => self.library_display = prefs,
            SettingChange::Save(prefs) => self.save = prefs,
            SettingChange::Cache(prefs) => self.cache = prefs,
            SettingChange::Websites(prefs) => self.websites = prefs,
            SettingChange::Layout(prefs) => self.layout = prefs.normalized(),
            SettingChange::AccentColor(color) => self.accent_color = color.to_ascii_lowercase(),
            SettingChange::CoverSize(percent) => self.cover_size = percent,
            SettingChange::MangaReader(prefs) => self.manga_reader = prefs,
            SettingChange::PlayerView(prefs) => self.player_view = prefs,
            SettingChange::Scrollbars(style) => self.scrollbars = style,
            SettingChange::Motion(level) => self.motion = level,
            SettingChange::TrackTables(prefs) => self.track_tables = prefs,
            SettingChange::Panels(prefs) => self.panels = prefs,
            SettingChange::DisabledSources(ids) => self.disabled_sources = ids,
            SettingChange::AutoLoad(prefs) => self.auto_load = prefs,
            SettingChange::WindowFrame(frame) => self.window_frame = frame,
            SettingChange::Zoom(percent) => self.zoom = percent,
            SettingChange::FailingSourcesLast(on) => self.failing_sources_last = on,
            SettingChange::UserAgent(agent) => self.user_agent = agent.trim().to_owned(),
            SettingChange::Sound(prefs) => self.sound = prefs,
            SettingChange::KeepMusicPlaying(on) => self.keep_music_playing = on,
            SettingChange::ResumeOnStart(on) => self.resume_on_start = on,
            SettingChange::AudioQuality(prefs) => self.audio_quality = prefs,
            SettingChange::Rail(prefs) => self.rail = prefs,
            SettingChange::Privacy(prefs) => self.privacy = prefs,
            SettingChange::Hidden(prefs) => self.hidden = prefs,
            SettingChange::LyricsCache(prefs) => self.lyrics_cache = prefs,
            SettingChange::LyricsTranslation(prefs) => {
                self.lyrics_translation = LyricsTranslationPrefs {
                    language: prefs.language.trim().to_owned(),
                    ..prefs
                }
            }
            SettingChange::Transcription(prefs) => {
                self.transcription = TranscriptionPrefs {
                    online_model: prefs.online_model.trim().to_owned(),
                    ..prefs
                }
            }
            SettingChange::Discovery(prefs) => {
                self.discovery = DiscoveryPrefs {
                    region: prefs.region.to_ascii_lowercase(),
                    ..prefs
                }
            }
            SettingChange::Colors(colors) => {
                self.colors = CustomColors {
                    background: colors.background.to_ascii_lowercase(),
                    surface: colors.surface.to_ascii_lowercase(),
                    foreground: colors.foreground.to_ascii_lowercase(),
                    muted: colors.muted.to_ascii_lowercase(),
                }
            }
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
    ExplicitMusic(bool),
    PreferredVideoQuality(Option<u32>),
    Accent(Accent),
    StartScreen(StartScreen),
    NavLabels(NavLabels),
    CoverStyle(CoverStyle),
    UnreadBadges(bool),
    DownloadBadges(bool),
    ResumeVideos(bool),
    WatchedPercent(u32),
    SeekSeconds(u32),
    AutoSubtitles(bool),
    SubtitleLanguage(String),
    AutoplayNext(bool),
    NextPromptSeconds(u32),
    /// Boxed: much larger than the other settings.
    VideoPlayer(Box<VideoPlayerPrefs>),
    LibraryDisplay(LibraryDisplayPrefs),
    Save(SavePrefs),
    Cache(CachePrefs),
    Websites(WebsitePrefs),
    Layout(LayoutPrefs),
    AccentColor(String),
    CoverSize(i32),
    MangaReader(MangaReaderPrefs),
    PlayerView(PlayerViewPrefs),
    Scrollbars(ScrollbarStyle),
    Motion(MotionLevel),
    TrackTables(TrackTablePrefs),
    Panels(PanelPrefs),
    Colors(CustomColors),
    DisabledSources(Vec<String>),
    AutoLoad(AutoLoadPrefs),
    WindowFrame(WindowFrame),
    Zoom(u32),
    Discovery(DiscoveryPrefs),
    FailingSourcesLast(bool),
    UserAgent(String),
    Sound(SoundPrefs),
    KeepMusicPlaying(bool),
    ResumeOnStart(bool),
    AudioQuality(AudioQualityPrefs),
    Rail(RailPrefs),
    Transcription(TranscriptionPrefs),
    Privacy(PrivacyPrefs),
    Hidden(HiddenPrefs),
    LyricsCache(LyricsCachePrefs),
    LyricsTranslation(LyricsTranslationPrefs),
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
            Self::ExplicitMusic(_) => "explicitMusic",
            Self::PreferredVideoQuality(_) => "preferredVideoQuality",
            Self::Accent(_) => "accent",
            Self::StartScreen(_) => "startScreen",
            Self::NavLabels(_) => "navLabels",
            Self::CoverStyle(_) => "coverStyle",
            Self::UnreadBadges(_) => "unreadBadges",
            Self::DownloadBadges(_) => "downloadBadges",
            Self::ResumeVideos(_) => "resumeVideos",
            Self::WatchedPercent(_) => "watchedPercent",
            Self::SeekSeconds(_) => "seekSeconds",
            Self::AutoSubtitles(_) => "autoSubtitles",
            Self::SubtitleLanguage(_) => "subtitleLanguage",
            Self::AutoplayNext(_) => "autoplayNext",
            Self::NextPromptSeconds(_) => "nextPromptSeconds",
            Self::VideoPlayer(_) => "videoPlayer",
            Self::LibraryDisplay(_) => "libraryDisplay",
            Self::Save(_) => "save",
            Self::Cache(_) => "cache",
            Self::Websites(_) => "websites",
            Self::Layout(_) => "layout",
            Self::AccentColor(_) => "accentColor",
            Self::CoverSize(_) => "coverSize",
            Self::MangaReader(_) => "mangaReader",
            Self::PlayerView(_) => "playerView",
            Self::Scrollbars(_) => "scrollbars",
            Self::Motion(_) => "motion",
            Self::TrackTables(_) => "trackTables",
            Self::Panels(_) => "panels",
            Self::Colors(_) => "colors",
            Self::DisabledSources(_) => "disabledSources",
            Self::AutoLoad(_) => "autoLoad",
            Self::WindowFrame(_) => "windowFrame",
            Self::Zoom(_) => "zoom",
            Self::Discovery(_) => "discovery",
            Self::FailingSourcesLast(_) => "failingSourcesLast",
            Self::UserAgent(_) => "userAgent",
            Self::Sound(_) => "sound",
            Self::KeepMusicPlaying(_) => "keepMusicPlaying",
            Self::ResumeOnStart(_) => "resumeOnStart",
            Self::AudioQuality(_) => "audioQuality",
            Self::Rail(_) => "rail",
            Self::Transcription(_) => "transcription",
            Self::Privacy(_) => "privacy",
            Self::Hidden(_) => "hidden",
            Self::LyricsCache(_) => "lyricsCache",
            Self::LyricsTranslation(_) => "lyricsTranslation",
        }
    }

    /// Checks values the type system can't express.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Theme(_)
            | Self::ShowNsfw(_)
            | Self::ExplicitMusic(_)
            | Self::PreferredVideoQuality(None)
            | Self::Accent(_)
            | Self::StartScreen(_)
            | Self::NavLabels(_)
            | Self::CoverStyle(_)
            | Self::UnreadBadges(_)
            | Self::DownloadBadges(_)
            | Self::ResumeVideos(_)
            | Self::AutoSubtitles(_)
            | Self::AutoplayNext(_)
            | Self::Scrollbars(_)
            | Self::Motion(_)
            | Self::WindowFrame(_)
            | Self::FailingSourcesLast(_)
            | Self::KeepMusicPlaying(_)
            | Self::ResumeOnStart(_)
            | Self::AudioQuality(_)
            | Self::LibraryDisplay(_)
            | Self::Privacy(_) => Ok(()),
            Self::LyricsCache(prefs) if prefs.forget_after_days > 3650 => {
                Err("Lyrics can be kept for at most ten years".into())
            }
            Self::LyricsCache(_) => Ok(()),
            Self::LyricsTranslation(prefs) => {
                let language = prefs.language.trim();
                if !language.is_empty() {
                    language_tag(language)?;
                }
                if prefs.local_model.len() > 100 {
                    return Err("a model id is at most 100 characters".to_owned());
                }
                Ok(())
            }
            Self::Hidden(prefs) => prefs.validate(),
            Self::Rail(prefs) => prefs.validate(),
            Self::Transcription(prefs) => {
                if prefs.online_model.trim().len() > 200 {
                    Err("a model name is at most 200 characters".to_owned())
                } else {
                    Ok(())
                }
            }
            Self::Sound(prefs) => prefs.validate(),
            // What an HTTP header can carry: printable ASCII.
            Self::UserAgent(agent) => {
                if agent.len() <= 512 && agent.bytes().all(|byte| (32..127).contains(&byte)) {
                    Ok(())
                } else {
                    Err("a user agent is printable ASCII, up to 512 characters".to_owned())
                }
            }
            Self::Discovery(prefs) => {
                if prefs.region.is_empty()
                    || (prefs.region.len() == 2
                        && prefs.region.bytes().all(|byte| byte.is_ascii_alphabetic()))
                {
                    Ok(())
                } else {
                    Err(format!("{} isn't a country code", prefs.region))
                }
            }
            Self::Zoom(percent) => {
                if ZOOM_LEVELS.contains(percent) {
                    Ok(())
                } else {
                    Err(format!("{percent}% isn't an offered zoom"))
                }
            }
            Self::NextPromptSeconds(seconds) => {
                if NEXT_PROMPT_SECONDS.contains(seconds) {
                    Ok(())
                } else {
                    Err(format!("{seconds} s isn't an offered prompt time"))
                }
            }
            Self::WatchedPercent(percent) => {
                if (50..=100).contains(percent) && percent % 5 == 0 {
                    Ok(())
                } else {
                    Err(format!("{percent}% isn't an offered watched point"))
                }
            }
            Self::SeekSeconds(seconds) => {
                if SEEK_SECONDS.contains(seconds) {
                    Ok(())
                } else {
                    Err(format!("{seconds} s isn't an offered seek step"))
                }
            }
            Self::VideoPlayer(prefs) => prefs.validate(),
            Self::Cache(prefs) => prefs.validate(),
            Self::Websites(prefs) => prefs.validate(),
            Self::Layout(prefs) => prefs.validate(),
            Self::MangaReader(prefs) => prefs.validate(),
            Self::PlayerView(prefs) => prefs.validate(),
            Self::TrackTables(prefs) => prefs.validate(),
            Self::Panels(prefs) => prefs.validate(),
            Self::Colors(colors) => colors.validate(),
            Self::AutoLoad(prefs) => {
                if prefs.cooldown_ms <= 10_000 {
                    Ok(())
                } else {
                    Err("Wait at most 10 seconds between pages".into())
                }
            }
            Self::DisabledSources(ids) => {
                if ids.len() > 2_000 || ids.iter().any(|id| id.is_empty() || id.len() > 300) {
                    Err("That isn't a list of sources".into())
                } else {
                    Ok(())
                }
            }
            Self::AccentColor(color) => {
                if is_hex_color(color) {
                    Ok(())
                } else {
                    Err(format!("{color} isn't a colour like #e5484d"))
                }
            }
            Self::CoverSize(percent) => {
                if COVER_SIZES.contains(percent) {
                    Ok(())
                } else {
                    Err(format!("{percent}% isn't an offered cover size"))
                }
            }
            Self::Save(prefs) => {
                if prefs.location == SaveLocation::Folder && prefs.folder_uri.is_none() {
                    Err("Pick a folder first".into())
                } else {
                    Ok(())
                }
            }
            Self::SubtitleLanguage(tag) if tag.is_empty() => Ok(()),
            Self::SubtitleLanguage(tag) => language_tag(tag),
            Self::PreferredVideoQuality(Some(height)) => {
                if VIDEO_QUALITIES.contains(height) {
                    Ok(())
                } else {
                    Err(format!("{height}p is not an offered video quality"))
                }
            }
            Self::NovelReader(prefs) => {
                if !NOVEL_TEXT_SIZES.contains(&prefs.text_size) {
                    Err(format!(
                        "{} px is outside the text sizes offered",
                        prefs.text_size
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
            Self::Language(tag) => language_tag(tag),
        }
    }
}

/// Checks a BCP 47-shaped language tag (`en`, `pt-BR`, `system`).
fn language_tag(tag: &str) -> Result<(), String> {
    let well_formed = (2..=35).contains(&tag.len())
        && tag
            .split('-')
            .all(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_alphanumeric()));
    if well_formed {
        Ok(())
    } else {
        Err(format!("\"{tag}\" is not a valid language tag"))
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

/// A song's length as Raygun learned it (M46.26), by the resolver's track key; `None` while
/// unknown.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackDuration {
    pub track_key: String,
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
    /// Playback speed, 0.5 to 2 (M46.8).
    pub rate: f64,
    /// When the sleep timer pauses playback (unix ms), if it's set to a time (M46.8).
    pub sleep_at: Option<f64>,
    /// The sleep timer pauses when the current song ends (M46.8).
    pub stop_after_current: bool,
}

/// The sleep timer (M46.8).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SleepTimer {
    Off,
    Minutes { minutes: u32 },
    EndOfTrack,
}

/// Speeds offered for music (M46.8).
pub const PLAYBACK_RATES: [f64; 9] = [0.5, 0.75, 0.9, 1.0, 1.1, 1.25, 1.5, 1.75, 2.0];

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
    /// Removes everything after the current item in play order, keeping it playing.
    ClearUpcoming,
    /// Moves a queued item before another in the play order, or to the end (M46.8).
    #[serde(rename_all = "camelCase")]
    Move {
        item_id: String,
        before_item_id: Option<String>,
    },
    /// Playback speed, one of `PLAYBACK_RATES` (M46.8).
    SetRate {
        rate: f64,
    },
    SetSleepTimer {
        timer: SleepTimer,
    },
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
        /// This song's gain in dB: its own volume plus normalization (M46.12).
        gain_db: f64,
        /// Where the song ends (its end trim), ms; the engine reports `Ended` there.
        end_ms: Option<f64>,
        /// Fade from what plays now into this song over this long (a crossfade), ms; 0 cuts.
        fade_ms: f64,
        /// Measure this song's loudness and report it (it isn't known yet).
        measure: bool,
    },
    /// Plays a YouTube video in the embedded player hosted at `embed_url` (webview only). Only its
    /// volume can change: `gain_db` scales it down, never up.
    #[serde(rename_all = "camelCase")]
    LoadYouTube {
        load_id: u32,
        embed_url: String,
        video_id: String,
        start_ms: f64,
        autoplay: bool,
        volume: f64,
        gain_db: f64,
        end_ms: Option<f64>,
        fade_ms: f64,
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
    /// Playback speed (M46.8); engines keep it across loads.
    SetRate {
        rate: f64,
    },
    /// The equalizer and effects for every song (M46.12); engines keep them across loads.
    SetEffects {
        sound: SoundPrefs,
    },
    /// The playing song's gain changed (its volume was edited), dB.
    #[serde(rename_all = "camelCase")]
    SetTrackGain {
        gain_db: f64,
    },
    /// The playing song's end trim changed, ms.
    #[serde(rename_all = "camelCase")]
    SetEnd {
        end_ms: Option<f64>,
    },
    /// Fades the playing song out over `fade_ms`, then stops (another engine takes over).
    #[serde(rename_all = "camelCase")]
    FadeOut {
        fade_ms: f64,
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
    /// The song's integrated loudness, measured while it played (M46.12).
    Loudness {
        lufs: f64,
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
    /// A video source's own Search pill (M46.35: "YouTube", "PeerTube").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub search_pill: Option<String>,
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
    /// The icon its repository lists, through the cover cache (M46.2).
    pub icon: Option<String>,
    /// The site its sources read (the manifest's `website` or a Shosetsu script's `baseURL`),
    /// opened in Raygun's browser from a source's menu (M46.11).
    pub website: Option<String>,
}

/// How much of a Nuclear plugin runs in Raygun (M17).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CompatReport {
    /// The original npm package name.
    pub package: String,
    /// The version of the converter that made it (1 when it wasn't recorded).
    pub converter: u32,
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
    /// The icon the repository lists, through the cover cache (M46.2).
    pub icon: Option<String>,
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
    /// The source marks it explicit (M46.36); hidden while Explicit music is off.
    #[serde(default)]
    pub explicit: bool,
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
    /// The song's lyrics offset (M46.12): show each line this much later, ms.
    #[serde(default)]
    pub offset_ms: i32,
    /// These are the person's own lyrics, not a source's (M46.12).
    #[serde(default)]
    pub edited: bool,
    /// The lines in other languages or romanized (M46.48), each aligned to these lines.
    #[serde(default)]
    pub translations: Vec<LyricsTranslation>,
}

impl Lyrics {
    /// The lines translations line up with: the synced lines' words, else the plain text's lines.
    pub fn line_texts(&self) -> Vec<&str> {
        if self.synced.is_empty() {
            self.plain
                .as_deref()
                .map(|plain| plain.lines().collect())
                .unwrap_or_default()
        } else {
            self.synced.iter().map(|line| line.text.as_str()).collect()
        }
    }
}

/// What lines under the lyrics are (M46.48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LyricsTranslationKind {
    /// The words in another language.
    Translation,
    /// The original's sounds in Latin letters (romaji, pinyin, romanized Korean).
    Romanization,
}

/// Who made a translation of lyrics (M46.48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LyricsTranslationOrigin {
    /// The lyrics' own source.
    Source,
    /// Another lyrics source's version of the song, matched to these lines.
    OtherSource,
    /// A translation model on this device.
    Device,
    /// A translator set up in Settings → AI.
    Ai,
}

/// Lyrics in another language, or romanized, line for line (M46.48).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricsTranslation {
    /// BCP 47 tag of what it's written in: a translation's language (`en`, `zh-Hans`); a
    /// romanization's is the original's with `-Latn` (`ja-Latn`), or `und-Latn`.
    pub language: String,
    pub kind: LyricsTranslationKind,
    pub origin: LyricsTranslationOrigin,
    /// Who made it: the source's or the translator's name; empty for this device.
    pub by: String,
    /// What made it: `device`, `ai:<provider id>`, or `source:<name>` for a lyrics source.
    #[serde(default)]
    pub translator: String,
    /// One per line of the lyrics (`Lyrics::line_texts`); empty where a line has none.
    pub lines: Vec<String>,
}

/// A translator that can translate a song's lyrics (M46.48), as the Translate menu lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricsTranslatorOption {
    /// `device`, or `ai:<provider id>`.
    pub id: String,
    /// The translator's kind; `None` for this device.
    pub kind: Option<AiProviderKind>,
    /// The provider's name; empty for this device.
    pub name: String,
    /// The models this device uses for it, from the AI catalogue.
    pub models: Vec<String>,
    /// What's still to download first, in bytes; 0 when it's ready.
    pub download_bytes: f64,
    /// It can translate into the language: this device has a model for it, a translator its key.
    pub supported: bool,
}

/// A lyrics translation being made (M46.48): lines done out of all of them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LyricsTranslationProgress {
    /// The song's track key.
    pub key: String,
    pub language: String,
    pub translator: String,
    pub done: u32,
    pub total: u32,
}

/// A chart a `music.charts` provider keeps (M46.9): its most played, trending, or a genre's top.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicChart {
    pub provider: ProviderInfo,
    /// The provider's own id, passed back for the songs.
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
    /// The genre the chart belongs to, in the app's spelling ("Hip-Hop").
    pub tag: Option<String>,
    /// What kind of chart it is (M46.19).
    pub group: ChartGroup,
}

/// Kinds of chart (M46.19), in the order Browse → Music lists them.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type,
)]
#[serde(rename_all = "camelCase")]
pub enum ChartGroup {
    /// The most played overall.
    Top,
    /// Rising now.
    Trending,
    /// New releases.
    New,
    /// Spreading fast.
    Viral,
    Genre,
    Mood,
    Decade,
    /// Rising artists with smaller followings.
    Underground,
}

impl ChartGroup {
    /// A provider's name for the group; unknown ones are `None`.
    pub fn parse(text: &str) -> Option<Self> {
        Some(match text.trim().to_ascii_lowercase().as_str() {
            "top" => Self::Top,
            "trending" => Self::Trending,
            "new" => Self::New,
            "viral" => Self::Viral,
            "genre" => Self::Genre,
            "mood" => Self::Mood,
            "decade" => Self::Decade,
            "underground" => Self::Underground,
            _ => return None,
        })
    }
}

// ---- Importing songs (M46.21) ----

/// A song read from a link, a file, pasted text or a screenshot.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedSong {
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    pub artwork_url: Option<String>,
}

/// Songs read for importing, and what to call them.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedList {
    pub name: String,
    pub artwork_url: Option<String>,
    pub songs: Vec<ImportedSong>,
    /// The source shared only part of the list (Spotify's embed stops at 100 songs).
    pub capped: bool,
}

/// A song after matching it against the music sources: what the best match added to it, which
/// source that was, and its stream when the source plays it directly.
/// Where imported songs go (M46.45): a new playlist, one the user has, or Favorites.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ImportTarget {
    New { name: String },
    Playlist { id: String },
    Favorites,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MatchedSong {
    pub song: ImportedSong,
    pub matched_by: Option<String>,
    pub source: Option<MediaSource>,
    /// Every source that has the song, best first (M46.31: the user can pick another).
    pub candidates: Vec<SongCandidate>,
    /// Not matched yet: importing keeps matching it in the background (M46.31).
    pub pending: bool,
}

/// One source's answer for an imported song (M46.31).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SongCandidate {
    pub provider_id: String,
    pub provider_name: String,
    pub track: OnlineTrack,
    /// The resolver's score; higher is a closer match.
    pub points: i32,
}

// ---- Made for You (M46.20) ----

/// Kinds of mix made for the user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum MixKind {
    /// The user's songs of a genre (or artist) with similar artists' songs.
    Daily,
    /// Songs the user hasn't heard, by artists like theirs; new each week.
    DiscoverWeekly,
    /// New releases by the user's artists.
    ReleaseRadar,
    /// The most played of the last 30 days.
    OnRepeat,
    /// Songs played a lot before but not lately.
    RepeatRewind,
    /// What the user plays at this time of day.
    TimeOfDay,
    /// Songs like an artist the user listens to (`seed`).
    BecauseYouListened,
    /// A source's radio for an artist (`seed`).
    ArtistRadio,
    /// The user's songs of a genre (`seed`) with the genre sources' songs.
    Genre,
    /// The same for a mood.
    Mood,
}

/// A mix with its songs. Its name is made by the UI from the kind, `seed` and `number`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Mix {
    /// Steady across days ("daily-1", "discover-weekly", "because-nina simone").
    pub id: String,
    pub kind: MixKind,
    /// The artist, genre, mood or time of day it's about.
    pub seed: Option<String>,
    /// A Daily Mix's number.
    pub number: Option<u32>,
    /// The first three main artists, for the description.
    pub artists: Vec<String>,
    /// Up to four covers, for the mosaic.
    pub artwork_urls: Vec<String>,
    pub tracks: Vec<OnlineTrack>,
}

/// A mix without its songs, for the shelves.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MixSummary {
    pub id: String,
    pub kind: MixKind,
    pub seed: Option<String>,
    pub number: Option<u32>,
    pub artists: Vec<String>,
    pub artwork_urls: Vec<String>,
    pub track_count: u32,
}

/// The "Your listening" numbers (QuietPlay's stat cards).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListeningStats {
    pub plays_this_week: u32,
    pub minutes_this_week: u32,
    pub top_artist: Option<String>,
    pub top_song: Option<OnlineTrack>,
    /// The hour (0-23, the user's clock) they play most at.
    pub peak_hour: Option<u8>,
    /// Songs the user has played or saved.
    pub songs_known: u32,
    /// The artists played most now, the most first (Home's Your top artists), without hidden
    /// ones.
    #[serde(default)]
    pub top_artists: Vec<String>,
}

/// Made for You: today's mixes, and whether new ones are being made.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MadeForYou {
    pub mixes: Vec<MixSummary>,
    pub stats: ListeningStats,
    /// When the mixes were made (unix ms).
    pub made_at: Option<f64>,
    pub making: bool,
}

/// One source's songs for a genre or a mood (M46.19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagSongsRow {
    pub provider: ProviderInfo,
    /// What the source calls them ("Breakcore on ListenBrainz").
    pub name: String,
    pub tracks: Vec<OnlineTrack>,
}

/// A source that didn't answer, and why.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceFailure {
    pub source: String,
    pub error: String,
}

/// Every source's songs for a genre or a mood (M46.19).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicTagSources {
    pub rows: Vec<TagSongsRow>,
    pub failures: Vec<SourceFailure>,
}

/// What open databases say about an artist (M46.14): the player's "About the artist" card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistInfo {
    pub name: String,
    /// "Person", "Group", "Orchestra"...
    pub kind: Option<String>,
    /// A few sentences, plain text.
    pub bio: Option<String>,
    pub image_url: Option<String>,
    /// Where to read more (Wikipedia).
    pub url: Option<String>,
    /// Where they're from.
    pub origin: Option<String>,
    /// "1975–", "1962–1970".
    pub active: Option<String>,
    /// A group's members.
    pub members: Vec<String>,
    pub provider_name: String,
}

/// People with one role on a song (M46.14): "Written by", "Produced by", "Guitar"...
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CreditGroup {
    pub role: String,
    pub names: Vec<String>,
}

/// Who made a song (M46.14): the player's Credits card.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SongCredits {
    pub credits: Vec<CreditGroup>,
    /// The song's page at the source.
    pub url: Option<String>,
    pub provider_name: String,
}

/// An artist a music source knows (M46.9): Search → Music → Artists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicArtist {
    pub provider: ProviderInfo,
    pub id: String,
    pub name: String,
    pub image_url: Option<String>,
}

/// An album a music source knows (M46.9): Search → Music → Albums.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicAlbum {
    pub provider: ProviderInfo,
    pub id: String,
    pub title: String,
    pub artist: Option<String>,
    pub year: Option<u32>,
    pub artwork_url: Option<String>,
    pub track_count: Option<u32>,
    /// What kind of release it is, when the source says (M46.36: an artist's discography).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub kind: Option<ReleaseKind>,
}

/// A release's kind (M46.36), as MusicBrainz and Deezer name them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ReleaseKind {
    Album,
    Single,
    Ep,
    Compilation,
    Live,
    Other,
}

impl ReleaseKind {
    /// Reads a source's word for it leniently; unknown words are `Other`.
    pub fn parse(word: &str) -> Self {
        match word.trim().to_ascii_lowercase().as_str() {
            "album" => Self::Album,
            "single" => Self::Single,
            "ep" => Self::Ep,
            "compilation" | "compile" => Self::Compilation,
            "live" => Self::Live,
            _ => Self::Other,
        }
    }
}

/// A place sound can come out of on this device (M46.43): the speaker, wired, Bluetooth or USB
/// headphones, HDMI. Desktop lists its own in the webview.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutput {
    pub id: String,
    pub name: String,
    /// `speaker`, `wired`, `bluetooth`, `usb` or `hdmi`.
    pub kind: String,
}

/// This device's outputs and the one picked (`None`: the system's choice).
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudioOutputs {
    pub outputs: Vec<AudioOutput>,
    pub chosen: Option<String>,
}

/// An artist the user follows (M46.42): in the library and quick access, with alerts for new
/// releases when `alerts` is on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FollowedArtist {
    pub name: String,
    pub image_url: Option<String>,
    /// Unix ms.
    pub followed_at: f64,
    pub alerts: bool,
}

/// A release found for a followed artist since the last check (M46.42).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistReleaseNews {
    pub artist: String,
    pub album: MusicAlbum,
    /// Unix ms.
    pub found_at: f64,
}

/// A concert or other event of an artist (M46.42), from a source's open data (MusicBrainz).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistEvent {
    pub name: String,
    /// `YYYY-MM-DD` (or less, as the source knows it).
    pub date: String,
    /// The venue and its city, when known.
    pub place: Option<String>,
    /// The event's page at the source.
    pub url: Option<String>,
    pub source: String,
}

/// A playlist a music source has (M46.39): Search → Music → Playlists.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylist {
    pub provider: ProviderInfo,
    pub id: String,
    pub title: String,
    /// Who made it (a user, or the service for its own).
    pub author: Option<String>,
    pub artwork_url: Option<String>,
    pub track_count: Option<u32>,
    /// One of the service's own (editorial) playlists.
    pub featured: bool,
}

/// A playlist with its songs, in order (M46.39).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicPlaylistDetail {
    pub playlist: MusicPlaylist,
    pub tracks: Vec<OnlineTrack>,
}

/// An album with its songs, in order (M46.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicAlbumDetail {
    pub album: MusicAlbum,
    pub tracks: Vec<OnlineTrack>,
}

/// Every source's charts (M46.9), and the sources that failed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicCharts {
    pub charts: Vec<MusicChart>,
    /// `<provider name>: <error>` for each source that didn't answer.
    pub errors: Vec<String>,
}

/// What a song's tag says (M46.9): a genre a source gave, or a mood derived from genres.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize, Type,
)]
#[serde(rename_all = "camelCase")]
pub enum TagKind {
    Genre,
    Mood,
}

/// One tag on a song (M46.9).
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MusicTag {
    pub name: String,
    pub kind: TagKind,
}

/// A song's stored tags (M46.9), by the resolver's track key.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackTags {
    pub track_key: String,
    pub tags: Vec<MusicTag>,
}

/// A tag across the user's saved songs, with how many have it (M46.9).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TagCount {
    pub name: String,
    pub kind: TagKind,
    pub count: u32,
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

/// One source's answer while a search still waits for the others, so Search shows the first
/// answers without waiting for the slowest source.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SearchProgress {
    /// What was searched for, trimmed as the search command trims it.
    pub query: String,
    /// How many sources the search asks.
    pub total: u32,
    pub part: SearchPart,
}

/// What one source answered, by kind of search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum SearchPart {
    Songs {
        result: ProviderSearchResult,
    },
    Artists {
        provider: ProviderInfo,
        artists: Vec<MusicArtist>,
    },
    Albums {
        provider: ProviderInfo,
        albums: Vec<MusicAlbum>,
    },
    Playlists {
        provider: ProviderInfo,
        featured: bool,
        playlists: Vec<MusicPlaylist>,
    },
}

/// A search someone made, kept on this device for Search's recent searches.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct RecentSearch {
    pub query: String,
    /// What it looked through: Search's kind (`all`, `music`, `video`, `manga`, `novel`,
    /// `library`).
    pub kind: String,
    /// Unix ms.
    pub searched_at: f64,
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

/// Where something was played from (M46.37: Home's Your music shows the latest): an album, a
/// playlist, a mix, an artist, a chart, a podcast or a book, with the page that opens it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlayContext {
    /// `album`, `playlist`, `mix`, `artist`, `chart`, `tag`, `podcast` or `audiobook`.
    pub kind: String,
    pub name: String,
    pub subtitle: Option<String>,
    pub artwork_url: Option<String>,
    /// The page's path (`/music/album`) and its search parameters.
    pub to: String,
    pub search: std::collections::BTreeMap<String, String>,
    /// Unix ms; set when it's remembered.
    #[serde(default)]
    pub played_at: f64,
}

/// A track as saved in favorites, a playlist or history. Artwork comes from the local library when
/// the same song is there.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
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
    /// When it was saved here (unix ms): a favorite's or a playlist item's date (M46.8).
    pub added_at: Option<f64>,
    /// Times it was played, from the history (M46.8).
    pub play_count: u32,
}

/// Where a song is saved (M46.8): Favorites and the playlists that hold it. The Add to playlist
/// popover reads it and sends back what should be true.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackSaves {
    pub favorite: bool,
    pub playlist_ids: Vec<String>,
}

/// A saved song as lists mark it (M46.48): the key it's stored under, every key its copies answer
/// to (their names', and the names they show with the user's details), and where it's saved.
/// `music/trackKey.ts` finds a shown song among them with `sameSong`.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SavedSong {
    pub track_key: String,
    pub song_keys: Vec<String>,
    pub favorite: bool,
    pub playlist_ids: Vec<String>,
}

/// What picking a source for a song does (M46.48): with `assign`, every saved copy, the queue
/// and the resolver's remembered pick take its stream (the download mode only downloads from
/// it); the details ticked become the song's own (Edit details reverts them). `source_name` is
/// the source's name, which the player shows the stream came from.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceChoice {
    pub assign: bool,
    pub source_name: Option<String>,
    pub title: bool,
    pub artist: bool,
    pub album: bool,
    pub cover: bool,
}

/// Where a saved song came from (M46.48): Import songs, and whether no source had it then.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TrackImportStatus {
    pub imported: bool,
    pub unavailable: bool,
}

/// What adding songs to a playlist did (M46.48): songs it already held stay out unless they're
/// added anyway.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistAdded {
    pub added: u32,
    pub already: u32,
}

/// An artist's page (M46.8): popular songs from the sources, and what the library has by them.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ArtistOverview {
    pub name: String,
    /// Songs the sources found for the name whose artists include it, without repeats.
    pub popular: Vec<OnlineTrack>,
    pub albums: Vec<LibraryAlbum>,
    pub tracks: Vec<LibraryTrack>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistSummary {
    pub id: String,
    pub name: String,
    pub item_count: u32,
    /// Up to four distinct artworks of the first items, for the cover.
    pub artwork_urls: Vec<String>,
    /// The cover the user picked (M46.45), shown instead of the artworks.
    pub cover_url: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PlaylistItem {
    pub id: String,
    pub track: SavedTrack,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
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
    /// Songs downloaded into the app (M46.18).
    Downloaded,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CollectionPlayRequest {
    pub collection: Collection,
    /// Track to start with; otherwise the first (or a random one when shuffling). With `order`, an
    /// index into that order.
    pub start_index: Option<u32>,
    pub shuffle: bool,
    /// The tracks' keys in the order a sorted or filtered table shows them (M46.8); the
    /// collection's own order when missing. Keys not in the collection are skipped.
    #[serde(default)]
    pub order: Option<Vec<String>>,
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

// ---- Test sources (M46.17) ----

/// What adding the test sources did: what was added and what couldn't be.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestSourcesReport {
    pub added: Vec<String>,
    pub already: Vec<String>,
    pub failed: Vec<TestSourceFailure>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TestSourceFailure {
    pub url: String,
    pub error: String,
}

// ---- Connect (M46.15) ----

/// What one device sends another it plays on (M46.15): the same changes as its own player's.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum ConnectCommand {
    /// Replaces the queue and starts `start_index` at `start_ms`; paused unless `play`.
    #[serde(rename_all = "camelCase")]
    Replace {
        items: Vec<QueueItemInput>,
        start_index: Option<u32>,
        shuffle: bool,
        start_ms: f64,
        play: bool,
    },
    #[serde(rename_all = "camelCase")]
    Enqueue {
        items: Vec<QueueItemInput>,
        play_now: bool,
    },
    #[serde(rename_all = "camelCase")]
    EnqueueNext { items: Vec<QueueItemInput> },
    #[serde(rename_all = "camelCase")]
    Control { control: PlaybackControl },
}

/// Another Raygun device this one can play on (M46.15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectDevice {
    pub id: String,
    pub name: String,
    /// `host:port`.
    pub address: String,
    /// `phone` or `computer`.
    pub kind: String,
    /// It allowed this device before, so playing there asks nothing.
    pub paired: bool,
    /// Heard on the network, or reached at its address, this run.
    pub found: bool,
    /// Linked (M46.47): its sync id; the two keep each other in sync.
    pub sync_id: Option<String>,
    /// When the two last synced (Unix ms).
    pub last_sync_at: Option<f64>,
}

/// Playing on another device (M46.15): which one, and its player as it last reported.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectSession {
    pub device_id: String,
    pub device_name: String,
    /// `phone` or `computer`.
    pub device_kind: String,
    pub state: Option<PlaybackState>,
    pub queue: Option<QueueSnapshot>,
}

/// Where this device's player plays (M46.15).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectStatus {
    /// This device's name, as others see it.
    pub this_device: String,
    /// Set while playing on another device.
    pub session: Option<ConnectSession>,
    /// The device being asked, until it answers.
    pub asking: Option<String>,
    /// What went wrong last: a refusal, a device that went away.
    pub error: Option<String>,
}

/// Another device asks to play on this one (M46.15); `open` is false once it's answered or gone.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ConnectRequest {
    pub id: String,
    pub name: String,
    pub open: bool,
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
    /// False when the user disabled it: it stays listed, but searches skip it (M46.8).
    pub enabled: bool,
    /// Its own Search pill, when the source declares one (M46.35).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    #[specta(optional)]
    pub search_pill: Option<String>,
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

/// Where a song saved into the app stands (M46.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SongDownloadState {
    Queued,
    Downloading,
    /// Playing and being recorded (only YouTube had it).
    Recording,
    Done,
    Failed,
    /// Only YouTube has the song: it can be recorded as it plays.
    NeedsRecording,
}

impl SongDownloadState {
    pub fn parse(text: &str) -> Self {
        match text {
            "downloading" => Self::Downloading,
            "recording" => Self::Recording,
            "done" => Self::Done,
            "failed" => Self::Failed,
            "needsRecording" => Self::NeedsRecording,
            _ => Self::Queued,
        }
    }
}

/// How a song download is made: a source's file (yt-dlp's copy of a YouTube video included), or a
/// recording of the song as it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SongDownloadKind {
    File,
    Recording,
}

impl SongDownloadKind {
    pub fn parse(text: &str) -> Option<Self> {
        match text {
            "file" => Some(Self::File),
            "recording" => Some(Self::Recording),
            _ => None,
        }
    }
}

/// A song saved (or being saved) into the app to play offline (M46.10). It is also the payload of
/// `SongDownloadProgressEvent`, sent on every change of state or source and while it downloads.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SongDownload {
    pub id: String,
    pub track_key: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub duration_ms: Option<u32>,
    /// A cover link through the image cache.
    pub artwork_url: Option<String>,
    pub state: SongDownloadState,
    /// How it's made: `file` while downloading, `recording` while recorded, and how it was made
    /// once done.
    pub kind: Option<SongDownloadKind>,
    /// Where the audio came from once done; while downloading, the source being tried now; while
    /// queued, the source picked for it, if any.
    pub provider: Option<String>,
    /// The saved file's extension ("mp3", "flac"...), once done.
    pub format: Option<String>,
    /// While downloading, what came so far; once done, the file's size.
    pub done_bytes: f64,
    pub total_bytes: Option<f64>,
    /// How far it is, 0–1, when known: the file's bytes, or how much of a recording has played.
    pub fraction: Option<f64>,
    /// How fast it downloads now, while it does.
    pub bytes_per_second: Option<f64>,
    /// Time left in ms, when known: from the speed and the size, or the song left to record.
    pub eta_ms: Option<f64>,
    /// What went wrong, in a line for the listener.
    pub error: Option<String>,
    /// Each source's own error behind `error` ("More info").
    pub error_detail: Option<String>,
    /// This device can record what plays (Windows for now).
    pub can_record: bool,
    /// Its place in the queue (unix ms): queued songs download earliest first.
    pub queued_at: f64,
    /// When it finished, or stopped with an error (unix ms).
    pub finished_at: Option<f64>,
}

/// yt-dlp on this device (M46.47): YouTube downloads on desktop, and songs only YouTube has.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct YtDlpStatus {
    /// Desktop only: phones have no yt-dlp.
    pub supported: bool,
    /// Its version, when found (Raygun's `tools` folder or the PATH).
    pub version: Option<String>,
    /// FFmpeg found: videos above 360p need it to join picture and sound.
    pub ffmpeg: bool,
    /// yt-dlp or FFmpeg is being fetched now.
    pub installing: bool,
    /// Why the last fetch failed.
    pub message: Option<String>,
}

/// What a YouTube address points at (M46.47).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum YouTubeLinkKind {
    Video,
    Playlist,
    Channel,
}

/// A YouTube address typed or pasted in Search (M46.47).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeLink {
    pub kind: YouTubeLinkKind,
    /// The video's, playlist's or channel's id.
    pub id: String,
    /// The address, tidied (`https://www.youtube.com/watch?v=<id>`).
    pub url: String,
    pub title: Option<String>,
    pub author: Option<String>,
    pub thumbnail_url: Option<String>,
}

/// What a yt-dlp download keeps (M46.47).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum YtDlpKind {
    Audio,
    Video,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum YtDlpState {
    Queued,
    Downloading,
    Done,
    Failed,
    Cancelled,
}

/// A YouTube video or its sound being saved with yt-dlp (M46.47), as Downloads lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct YtDlpJob {
    pub id: String,
    pub url: String,
    pub title: String,
    pub kind: YtDlpKind,
    pub state: YtDlpState,
    /// 0 to 1, once yt-dlp knows the size.
    pub progress: Option<f64>,
    /// Where the file went, as the user sees it.
    pub file: Option<String>,
    pub message: Option<String>,
}

/// What a saved song is written out as (M46.10).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AudioFileFormat {
    /// The bytes as downloaded (or the recording's FLAC).
    #[default]
    Original,
    Wav,
    Flac,
    /// Encoded with LAME at the Downloads audio quality (M46.18); an MP3 download is copied.
    Mp3,
}

/// A cover a source has for a song, an album or an artist (M46.36), offered in Edit details.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CoverOption {
    /// The image's address at the source (stored as is when picked).
    pub url: String,
    /// The source's name.
    pub source: String,
    /// What it's the cover of, when that isn't the thing edited (a song's album).
    pub of: Option<String>,
}

/// What the user edits the details of (M46.30): a song, an album or an artist; a playlist's
/// cover (M46.45).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum DetailTarget {
    Song {
        query: TrackQuery,
    },
    Album {
        artist: Option<String>,
        album: String,
    },
    Artist {
        name: String,
    },
    /// Only its cover is the user's (its name is renamed with the playlist).
    Playlist {
        id: String,
    },
}

/// The user's own details (M46.30); `None` keeps what the source said. For an album `title` is
/// its title, for an artist their name; `artist` and `album` are songs' only.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DetailEdit {
    pub title: Option<String>,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork_url: Option<String>,
}

/// A folder in Local Files (M46.27) and what its last scan found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalFolder {
    pub id: String,
    pub name: String,
    /// Its path (or, without one, the address the picker gave).
    pub location: String,
    pub counts: LocalCounts,
    /// Unix ms.
    pub scanned_at: Option<f64>,
    pub scanning: bool,
    /// How far a running scan got with the comics and books it adds; `None` while it lists.
    pub progress: Option<LocalScanProgress>,
    /// Its songs and videos can't be read until media access is allowed (Android).
    pub needs_access: bool,
    /// The folder couldn't be opened at the last scan (moved, unplugged, or its access lost);
    /// what it added stays until it's back.
    pub missing: bool,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalCounts {
    pub songs: u32,
    pub comics: u32,
    pub books: u32,
    pub videos: u32,
    /// Comics and books that couldn't be read or added.
    pub failed: u32,
    /// Songs and videos that can't play from where the folder is (a cloud or Downloads folder
    /// the picker gave without a path).
    pub unplayable: u32,
}

/// A Local Files scan adding comics and books: how many it has done of how many.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalScanProgress {
    pub done: u32,
    pub total: u32,
}

/// The file type a saved cover image gets (M46.26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ImageFileFormat {
    Png,
    Jpeg,
    Webp,
    /// JPEG XL, lossless (M46.38).
    Jxl,
}

/// Where a saved cover image goes (M46.26).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ImageDestination {
    /// `Pictures/Raygun/`.
    Pictures,
    /// The folder picked for saving (Settings → Downloads).
    Folder,
}

/// Where saved songs are written (M46.10).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SongDestination {
    /// `Music/Raygun/<artist>/`.
    Music,
    /// The folder picked in Settings → Downloads.
    Folder,
}

/// What saving songs as files did (M46.10).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SongsSaved {
    /// Where each file went, as the user sees it.
    pub saved: Vec<String>,
    /// `<title>: <why>` for each song that couldn't be written.
    pub failed: Vec<String>,
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
    /// The titles this session's last update couldn't check, and why.
    #[serde(default)]
    pub failures: Vec<UpdateFailure>,
}

/// A library title an update couldn't check.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct UpdateFailure {
    /// `manga`, `novel` or `video`: which page opens it.
    pub medium: String,
    pub source_id: String,
    pub remote_id: String,
    pub title: String,
    pub source_name: String,
    /// The source's error, for "More info".
    pub message: String,
}

/// A chapter a library update found.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaUpdateItem {
    /// `manga` or `novel`: which reader opens it.
    pub medium: String,
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
    /// The user's list (M46).
    pub list: Option<LibraryList>,
    pub favorite: bool,
    /// Kept at the top of the library (M46).
    pub pinned: bool,
    pub authors: Vec<String>,
    pub artists: Vec<String>,
    /// The source's name, for search (`source:`) and the list display.
    pub source_name: String,
    /// Unix ms the title was last checked for new chapters.
    pub refreshed_at: Option<f64>,
    /// Unix ms the newest chapter was found (Mihon's "chapter fetch date").
    pub latest_fetched_at: Option<f64>,
    /// Imported from the device (EPUB, comics).
    pub local: bool,
}

/// The user's lists for titles (M46, CloudStream's watch status). Each title is in at most one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LibraryList {
    Reading,
    Planned,
    Completed,
    OnHold,
    Dropped,
}

impl LibraryList {
    pub fn key(self) -> &'static str {
        match self {
            Self::Reading => "reading",
            Self::Planned => "planned",
            Self::Completed => "completed",
            Self::OnHold => "onHold",
            Self::Dropped => "dropped",
        }
    }

    pub fn from_key(key: &str) -> Option<Self> {
        Some(match key {
            "reading" => Self::Reading,
            "planned" => Self::Planned,
            "completed" => Self::Completed,
            "onHold" => Self::OnHold,
            "dropped" => Self::Dropped,
            _ => return None,
        })
    }
}

/// A title's list, favorite and pin, for its page (M46).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleMarks {
    pub in_library: bool,
    pub list: Option<LibraryList>,
    pub favorite: bool,
    pub pinned: bool,
}

/// A library title another one may duplicate (same name, another source; M46).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryDuplicate {
    pub title_id: String,
    pub source_id: String,
    pub remote_id: String,
    pub title: String,
    pub source_name: String,
    pub cover_url: Option<String>,
}

/// A user-named shelf in the manga or novel library (M39).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LibraryCategory {
    pub id: String,
    pub name: String,
    /// Its own display and sort, when the library keeps them per category (M46).
    pub display: Option<CategoryDisplay>,
}

/// A category's own display and sort (M46, Mihon's per-category settings).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CategoryDisplay {
    pub mode: LibraryDisplayMode,
    /// One of the library's sorts, as the UI names them (`title`, `lastRead`...).
    pub sort: String,
    pub descending: bool,
}

/// How library titles are laid out (M46, Mihon's display modes).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LibraryDisplayMode {
    /// The title over the cover's bottom edge.
    #[default]
    CompactGrid,
    /// The title under the cover.
    ComfortableGrid,
    CoverOnlyGrid,
    List,
}

/// What the library's tabs group by (M46).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum LibraryTabs {
    /// The user's categories (Mihon).
    #[default]
    Categories,
    /// Favorites and the lists (CloudStream).
    Lists,
    None,
}

// ---- Websites and the built-in browser (M46.3) ----

/// The list of ad and tracker domains the built-in browser blocks.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum Blocklist {
    /// HaGeZi's Multi NORMAL: ads, trackers, malware, few false positives.
    #[default]
    HageziMulti,
    /// HaGeZi's Light: the smallest.
    HageziLight,
    /// HaGeZi's PRO: the strictest.
    HageziPro,
    /// StevenBlack's unified hosts.
    StevenBlack,
    /// An address the user gave (domains, hosts or `||domain^` rules).
    Custom,
}

impl Blocklist {
    /// Where the list is downloaded from.
    pub fn url(self) -> Option<&'static str> {
        match self {
            Self::HageziMulti => Some(
                "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/wildcard/multi-onlydomains.txt",
            ),
            Self::HageziLight => Some(
                "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/wildcard/light-onlydomains.txt",
            ),
            Self::HageziPro => Some(
                "https://raw.githubusercontent.com/hagezi/dns-blocklists/main/wildcard/pro-onlydomains.txt",
            ),
            Self::StevenBlack => {
                Some("https://raw.githubusercontent.com/StevenBlack/hosts/master/hosts")
            }
            Self::Custom => None,
        }
    }
}

/// One entry of a list the user arranges (M46.5): a tab, a section, a chip, a menu entry.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LayoutItem {
    pub id: String,
    pub visible: bool,
}

/// How the app's screens are arranged (M46.5): each list in the user's order, each entry shown
/// or hidden. One stored row; [`LayoutPrefs::normalized`] drops entries Raygun no longer has and
/// puts new ones at their default place, so an update never loses the user's order.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct LayoutPrefs {
    /// The navigation tabs; hidden ones are listed in More.
    pub tabs: Vec<LayoutItem>,
    /// Library's media tabs (Audio, Comics, Books, Videos); the first shown is the default.
    pub library_media: Vec<LayoutItem>,
    /// Library → Audio's chips; the first shown is the default.
    pub audio_views: Vec<LayoutItem>,
    /// Home's sections.
    pub home_sections: Vec<LayoutItem>,
    /// Browse's tabs (Search, Sources, Extensions).
    pub browse_tabs: Vec<LayoutItem>,
    /// Browse → Sources' sections.
    pub browse_sections: Vec<LayoutItem>,
    /// More's entries.
    pub more_entries: Vec<LayoutItem>,
}

/// Every tab there is, in the default order: the four shown, then the ones that can be added.
pub const LAYOUT_TABS: [(&str, bool); 13] = [
    ("home", true),
    ("library", true),
    ("browse", true),
    ("more", true),
    ("watch", false),
    ("music", false),
    ("comics", false),
    ("books", false),
    ("podcasts", false),
    ("search", false),
    ("updates", false),
    ("downloads", false),
    ("settings", false),
];
pub const LAYOUT_LIBRARY_MEDIA: [(&str, bool); 5] = [
    // Every kind at once (M46.37), first so the Library opens on it.
    ("all", true),
    ("music", true),
    ("manga", true),
    ("novel", true),
    ("video", true),
];
pub const LAYOUT_AUDIO_VIEWS: [(&str, bool); 9] = [
    ("all", true),
    ("podcasts", true),
    ("audiobooks", true),
    ("playlists", true),
    ("favorites", true),
    ("downloaded", true),
    ("albums", true),
    ("artists", true),
    ("tracks", true),
];
pub const LAYOUT_HOME_SECTIONS: [(&str, bool); 23] = [
    // "Get started" (M46.37): shown while nothing is under way; closing it hides it.
    ("getStarted", true),
    ("continueWatching", true),
    ("continueReading", true),
    ("continueListening", true),
    ("madeForYou", true),
    ("yourMusic", true),
    // The artists played most now, from Made for You's numbers.
    ("topArtists", true),
    ("recentlyPlayed", true),
    // New episodes of followed shows.
    ("newEpisodes", true),
    ("popularMusic", true),
    ("trendingMusic", true),
    // More rows from Browse all's kinds (M46.47).
    ("newReleases", true),
    // Releases found for followed artists (M46.42).
    ("followedReleases", true),
    ("popularVideos", true),
    ("popularComics", true),
    ("popularBooks", true),
    ("popularAudiobooks", true),
    ("popularPodcasts", true),
    ("charts", true),
    ("viralMusic", true),
    ("genres", true),
    ("decades", true),
    ("undergroundMusic", true),
];
pub const LAYOUT_BROWSE_TABS: [(&str, bool); 4] = [
    ("search", true),
    ("discover", true),
    ("sources", true),
    ("extensions", true),
];
pub const LAYOUT_BROWSE_SECTIONS: [(&str, bool); 4] = [
    ("video", true),
    ("manga", true),
    ("novel", true),
    ("websites", true),
];
pub const LAYOUT_MORE_ENTRIES: [(&str, bool); 9] = [
    ("updates", true),
    ("downloads", true),
    ("categories", true),
    ("addons", true),
    ("repos", true),
    ("ai", true),
    ("devices", true),
    ("settings", true),
    ("about", true),
];

/// `saved` in its order with only known ids (once each), then every default it lacks, placed
/// after the entry that comes before it by default.
fn arrange(saved: &[LayoutItem], defaults: &[(&str, bool)]) -> Vec<LayoutItem> {
    let mut items: Vec<LayoutItem> = Vec::with_capacity(defaults.len());
    for item in saved {
        let known = defaults.iter().any(|(id, _)| *id == item.id);
        if known && !items.iter().any(|other| other.id == item.id) {
            items.push(item.clone());
        }
    }
    for (index, (id, visible)) in defaults.iter().enumerate() {
        if items.iter().any(|item| item.id == *id) {
            continue;
        }
        let at = defaults[..index]
            .iter()
            .rev()
            .find_map(|(before, _)| items.iter().position(|item| item.id == *before))
            .map_or(0, |position| position + 1);
        items.insert(
            at,
            LayoutItem {
                id: (*id).to_owned(),
                visible: *visible,
            },
        );
    }
    items
}

fn shows(items: &[LayoutItem], id: &str) -> bool {
    items.iter().any(|item| item.id == id && item.visible)
}

impl LayoutPrefs {
    /// Every list with Raygun's current entries, in the user's order where they set one.
    pub fn normalized(&self) -> Self {
        Self {
            tabs: arrange(&self.tabs, &LAYOUT_TABS),
            library_media: arrange(&self.library_media, &LAYOUT_LIBRARY_MEDIA),
            audio_views: arrange(&self.audio_views, &LAYOUT_AUDIO_VIEWS),
            home_sections: arrange(&self.home_sections, &LAYOUT_HOME_SECTIONS),
            browse_tabs: arrange(&self.browse_tabs, &LAYOUT_BROWSE_TABS),
            browse_sections: arrange(&self.browse_sections, &LAYOUT_BROWSE_SECTIONS),
            more_entries: arrange(&self.more_entries, &LAYOUT_MORE_ENTRIES),
        }
    }

    /// More and Settings can't be hidden (they lead everywhere else), Browse keeps Search, and
    /// every list that picks a default keeps something to show.
    pub fn validate(&self) -> Result<(), String> {
        let layout = self.normalized();
        if !shows(&layout.tabs, "more") {
            return Err("More can't be hidden: it leads to everything else".into());
        }
        if layout.tabs.iter().filter(|item| item.visible).count() < 2 {
            return Err("Keep at least one tab besides More".into());
        }
        if !shows(&layout.more_entries, "settings") {
            return Err("Settings can't be hidden from More".into());
        }
        if !shows(&layout.browse_tabs, "search") {
            return Err("Browse's Search can't be hidden".into());
        }
        if !layout.library_media.iter().any(|item| item.visible) {
            return Err("Keep at least one library tab".into());
        }
        if !layout.audio_views.iter().any(|item| item.visible) {
            return Err("Keep at least one Audio view".into());
        }
        Ok(())
    }
}

impl Default for LayoutPrefs {
    fn default() -> Self {
        Self {
            tabs: Vec::new(),
            library_media: Vec::new(),
            audio_views: Vec::new(),
            home_sections: Vec::new(),
            browse_tabs: Vec::new(),
            browse_sections: Vec::new(),
            more_entries: Vec::new(),
        }
        .normalized()
    }
}

/// The built-in browser and its blocker. One stored row; missing fields keep their defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct WebsitePrefs {
    /// Listed ad and tracker domains are blocked.
    pub block_ads: bool,
    /// Popups and redirects to other sites no tap asked for are blocked.
    pub block_popups: bool,
    pub blocklist: Blocklist,
    /// For [`Blocklist::Custom`].
    pub custom_list_url: Option<String>,
    /// Sites open in desktop mode.
    pub desktop: bool,
}

impl Default for WebsitePrefs {
    fn default() -> Self {
        Self {
            block_ads: true,
            block_popups: true,
            blocklist: Blocklist::HageziMulti,
            custom_list_url: None,
            desktop: false,
        }
    }
}

impl WebsitePrefs {
    pub fn validate(&self) -> Result<(), String> {
        if self.blocklist == Blocklist::Custom {
            let url = self.custom_list_url.as_deref().unwrap_or_default();
            if !url.starts_with("https://") && !url.starts_with("http://") {
                return Err("Enter the list's web address".into());
            }
        }
        Ok(())
    }

    /// Where the chosen list is downloaded from.
    pub fn list_url(&self) -> Option<String> {
        match self.blocklist {
            Blocklist::Custom => self.custom_list_url.clone(),
            list => list.url().map(str::to_owned),
        }
    }
}

/// A website the user added.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct Website {
    pub id: String,
    pub name: String,
    pub url: String,
    pub host: String,
    /// Its icon through the cover cache.
    pub icon: Option<String>,
    /// Where it was left, when it opens there rather than at `url`.
    pub last_url: Option<String>,
    pub block_ads: bool,
}

/// The downloaded blocklist.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BlocklistStatus {
    /// Domains listed; 0 before the first download (a short built-in list is used then).
    pub domains: u32,
    /// Unix ms.
    pub updated_at: Option<f64>,
    pub source: Option<String>,
    pub error: Option<String>,
}

// ---- Image caches (M46.2) ----

/// The cache sizes offered, in megabytes.
pub const CACHE_SIZES_MB: [u32; 6] = [128, 256, 512, 1024, 2048, 4096];
/// How old a cached cover may get before it's checked again, in days; 0 never checks.
pub const COVER_REFRESH_DAYS: [u32; 5] = [0, 1, 3, 7, 30];

/// Image cache limits and refreshing. One stored row; missing fields keep their defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct CachePrefs {
    /// Covers, posters, thumbnails and icons. Library covers count but are never removed.
    pub cover_cache_mb: u32,
    /// Reader pages.
    pub page_cache_mb: u32,
    pub cover_refresh_days: u32,
    /// The page cache is emptied each time the app starts (Mihon's option).
    pub clear_pages_on_start: bool,
}

impl Default for CachePrefs {
    fn default() -> Self {
        Self {
            cover_cache_mb: 512,
            page_cache_mb: 512,
            cover_refresh_days: 7,
            clear_pages_on_start: false,
        }
    }
}

impl CachePrefs {
    pub fn validate(&self) -> Result<(), String> {
        for size in [self.cover_cache_mb, self.page_cache_mb] {
            if !CACHE_SIZES_MB.contains(&size) {
                return Err(format!("{size} MB isn't an offered cache size"));
            }
        }
        if !COVER_REFRESH_DAYS.contains(&self.cover_refresh_days) {
            return Err(format!(
                "{} days isn't an offered refresh age",
                self.cover_refresh_days
            ));
        }
        Ok(())
    }
}

/// A part of what Raygun stores, as Settings → Data → Storage lists it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StorageArea {
    /// Covers, posters, thumbnails and icons (a cache).
    Covers,
    /// Reader pages (a cache).
    Pages,
    /// Files made for sharing (a cache).
    Shared,
    /// Artwork read from the music library's files; it comes back only with a rescan.
    MusicArt,
    /// Downloaded chapters, books, comics and videos: not a cache.
    Downloads,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StorageUsage {
    pub area: StorageArea,
    pub bytes: f64,
    pub files: u32,
    /// The size limit, for caches that have one.
    pub limit_bytes: Option<f64>,
    /// Whether Clear is offered.
    pub clearable: bool,
}

// ---- Saving outside the app and exports (M46.1) ----

/// Where finished downloads are also saved. The app always keeps its own copy.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SaveLocation {
    /// Only in the app.
    #[default]
    App,
    /// `Download/Raygun/` on the phone.
    Downloads,
    /// A folder the user picked.
    Folder,
}

/// How a manga chapter is written out.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChapterFormat {
    #[default]
    Cbz,
    /// A folder of page images.
    Images,
}

/// Saving downloads and pages outside the app. One stored row; missing fields keep defaults.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct SavePrefs {
    pub location: SaveLocation,
    /// The picked folder (a Storage Access Framework tree URI) and its name.
    pub folder_uri: Option<String>,
    pub folder_name: Option<String>,
    pub chapter_format: ChapterFormat,
    /// Saved pages go to `Pictures/Raygun/<Title>/` rather than `Pictures/Raygun/`.
    pub pages_per_title: bool,
    /// How finished song downloads are also written out, when they are (M46.10).
    pub song_format: AudioFileFormat,
}

impl Default for SavePrefs {
    fn default() -> Self {
        Self {
            location: SaveLocation::App,
            folder_uri: None,
            folder_name: None,
            chapter_format: ChapterFormat::Cbz,
            pages_per_title: true,
            song_format: AudioFileFormat::Original,
        }
    }
}

/// The picked folder as Android sees it now.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SaveFolderState {
    /// Raygun may still write there.
    pub granted: bool,
    pub name: Option<String>,
}

/// What an export writes.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExportFormat {
    /// Manga: one CBZ (with ComicInfo.xml) per chapter.
    Cbz,
    /// Manga: one PDF for everything chosen, a bookmark per chapter.
    Pdf,
    /// Manga: one PDF per chapter.
    PdfPerChapter,
    /// Manga: a folder of page images per chapter.
    Images,
    /// Novels: one EPUB book.
    Epub,
    /// Novels: one text file.
    Text,
    /// Videos: each downloaded episode as one video file.
    Video,
}

/// Which chapters or episodes of each title an export takes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum ExportScope {
    All,
    /// Only those already downloaded: nothing is fetched.
    Downloaded,
    Unread,
    /// From one chapter to another, inclusive, in reading order (one title).
    Range {
        from: String,
        to: String,
    },
    /// Exactly these.
    Chosen {
        ids: Vec<String>,
    },
}

/// Where an export's files go.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExportDestination {
    /// `Download/Raygun/`.
    Downloads,
    /// The folder picked in Settings → Downloads.
    Folder,
    /// The share sheet (one file).
    Share,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    pub medium: String,
    pub title_ids: Vec<String>,
    pub scope: ExportScope,
    pub format: ExportFormat,
    pub destination: ExportDestination,
}

/// What an export would do, shown before it starts.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportPlan {
    pub chapters: u32,
    /// Chapters that must download first.
    pub missing: u32,
    /// Files the export will write.
    pub files: u32,
    /// Why it can't run as asked, if so (e.g. a video stream with separate audio).
    pub problem: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ExportState {
    /// Waiting for chapters to download.
    Downloading,
    Writing,
    Done,
    Failed,
    Cancelled,
}

/// One export job, as Downloads → Exports lists it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExportJob {
    pub id: String,
    /// The title, or "3 titles".
    pub label: String,
    pub medium: String,
    pub format: ExportFormat,
    pub destination: ExportDestination,
    pub state: ExportState,
    /// Chapters downloaded so far / needed, while downloading.
    pub downloaded: u32,
    pub to_download: u32,
    /// Chapters written so far / to write.
    pub written: u32,
    pub to_write: u32,
    /// Names of the files written.
    pub files: Vec<String>,
    /// Chapters left out (their download failed).
    pub failed_chapters: u32,
    /// Pages left out of a PDF (formats it can't hold).
    pub skipped_pages: u32,
    pub message: Option<String>,
    pub started_at: f64,
}

/// A page of the manga reader, by its proxy URL.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PageRef {
    pub title_id: String,
    pub chapter_id: String,
    /// 0-based.
    pub index: u32,
    pub url: String,
}

/// The manga, novel and video libraries' display (M46). One stored row; fields missing from it
/// keep their defaults.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(default, rename_all = "camelCase")]
pub struct LibraryDisplayPrefs {
    pub mode: LibraryDisplayMode,
    pub tabs: LibraryTabs,
    /// The number of titles on each tab.
    pub show_counts: bool,
    /// Each category keeps its own display and sort.
    pub per_category: bool,
    /// A play button on covers that continues reading or watching.
    pub continue_button: bool,
    /// A badge on titles imported from the device.
    pub local_badge: bool,
    /// Pinned titles come first whatever the sort.
    pub pins_on_top: bool,
}

impl Default for LibraryDisplayPrefs {
    fn default() -> Self {
        Self {
            mode: LibraryDisplayMode::CompactGrid,
            tabs: LibraryTabs::Categories,
            show_counts: true,
            per_category: false,
            continue_button: true,
            local_badge: true,
            pins_on_top: true,
        }
    }
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
        Self::chosen(text).unwrap_or(Self::Webtoon)
    }

    /// A title's own mode as stored; `None` (an empty value) follows the reader's default.
    pub fn chosen(text: &str) -> Option<Self> {
        match text {
            "webtoon" => Some(Self::Webtoon),
            "pagedLtr" => Some(Self::PagedLtr),
            "pagedRtl" => Some(Self::PagedRtl),
            _ => None,
        }
    }
}

/// What the reader needs for one chapter.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct MangaChapterPages {
    pub title_id: String,
    pub title: String,
    /// How the chapter is read: the title's own mode, else the reader's default.
    pub reader_mode: ReaderMode,
    /// The title's own mode; `None` follows the default.
    pub own_reader_mode: Option<ReaderMode>,
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

/// A title imported from the device (comics, M46): its source and the source's id for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ImportedTitle {
    pub source_id: String,
    pub remote_id: String,
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

/// One piece for a speech engine: `id` is the paragraph's, with `:end` on the very last piece.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
pub struct SpeechLine {
    pub id: String,
    pub text: String,
}

/// What the webview's speech engine does on desktop (M46.6).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(tag = "type", rename_all = "camelCase")]
pub enum SpeechCommand {
    /// Speaks `items` in order at `rate` (1 is normal), after stopping anything playing.
    Speak {
        items: Vec<SpeechLine>,
        rate: f64,
    },
    Stop,
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
    /// Waiting for another download to finish (M46.48: two at a time).
    Queued,
    Downloading,
    Done,
    Failed,
}

/// What a show's or book's episodes are sorted by (M46.48).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum EpisodeSort {
    /// Release date.
    Date,
    /// Episode number, or a book's chapter order.
    Number,
    Title,
    Duration,
    /// Started ones, then the rest not played, then the played.
    Unplayed,
}

/// Which episodes show (M46.48); several at once must all hold.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum EpisodeFilter {
    Unplayed,
    Started,
    Downloaded,
}

/// How a show's or book's episodes list, kept per show (M46.48).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct EpisodeOrder {
    pub sort: EpisodeSort,
    /// Newest, highest, longest or last first.
    pub descending: bool,
    pub filters: Vec<EpisodeFilter>,
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
    /// The episode's number (`itunes:episode`), or a chapter's place in its book (M46.48).
    pub number: Option<u32>,
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
    /// As `order` lists them (M46.48; newest first unless changed).
    pub episodes: Vec<PodcastEpisode>,
    pub order: EpisodeOrder,
    /// `order` isn't the kind's own (M46.48).
    pub custom_order: bool,
    /// Every episode, shown or filtered out.
    pub episode_count: u32,
    /// The speed its episodes play at, when it has its own (M46.48).
    pub speed: Option<f64>,
}

// ---- Audiobooks (M46.22) ----

/// A book in the library, with how far it was listened.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookSummary {
    pub id: String,
    pub title: String,
    pub author: Option<String>,
    /// Which recording, when a source has several ("Version 2"), M46.48.
    pub edition: Option<String>,
    pub artwork_url: Option<String>,
    pub chapters: u32,
    /// Chapters listened to the end.
    pub finished: u32,
    /// Its length and how much of it was listened, when the feed gives lengths (M46.48).
    pub total_ms: f64,
    pub listened_ms: f64,
}

/// Where a book goes on from (M46.48): the chapter being listened, or the next one.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct BookResume {
    pub chapter_id: String,
    /// Its place in the book, from 1.
    pub number: u32,
    pub title: String,
    pub position_ms: f64,
}

/// A book with its chapters.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookView {
    pub id: String,
    pub feed_url: String,
    pub title: String,
    pub author: Option<String>,
    /// Which recording, when a source has several ("Version 2"), M46.48.
    pub edition: Option<String>,
    pub description: Option<String>,
    pub artwork_url: Option<String>,
    pub link: Option<String>,
    pub in_library: bool,
    /// As `order` lists them (M46.48; in chapter order unless changed).
    pub chapters: Vec<PodcastEpisode>,
    pub order: EpisodeOrder,
    pub custom_order: bool,
    /// Every chapter, shown or filtered out, how many were finished, and how many are downloaded
    /// or on their way.
    pub chapter_count: u32,
    pub finished: u32,
    pub downloaded: u32,
    pub downloading: u32,
    pub total_ms: f64,
    pub listened_ms: f64,
    /// `None` once every chapter is finished (or there are none).
    pub resume: Option<BookResume>,
    pub speed: Option<f64>,
}

/// An episode or a book to go on with, for Home's Continue listening (M46.48).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ListeningItem {
    pub kind: SpokenKind,
    pub podcast_id: String,
    pub episode_id: String,
    /// The episode's or the chapter's title.
    pub title: String,
    /// The show's or the book's.
    pub show: String,
    pub author: Option<String>,
    pub artwork_url: Option<String>,
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    /// A book's: the chapter's place and how many there are.
    pub chapter: Option<u32>,
    pub chapters: Option<u32>,
    /// Unix ms.
    pub listened_at: Option<f64>,
}

/// A book an audiobook directory found.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookResult {
    pub title: String,
    pub author: Option<String>,
    /// Which recording, when a source has several ("Version 2"), M46.48.
    pub edition: Option<String>,
    pub feed_url: String,
    pub artwork_url: Option<String>,
    pub runtime: Option<String>,
    pub language: Option<String>,
    pub description: Option<String>,
    pub source: String,
}

/// A page of books from the directories.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AudiobookPage {
    pub books: Vec<AudiobookResult>,
    pub has_more: bool,
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

/// What a chapter of a video is, for the skip button.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum ChapterKind {
    Opening,
    Ending,
    Recap,
    #[default]
    Other,
}

/// A chapter of a video (M45): from its source, or from AniSkip.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoChapter {
    /// Milliseconds from the start.
    pub start_ms: u32,
    /// Where it ends; the next chapter's start when missing.
    pub end_ms: Option<u32>,
    pub name: Option<String>,
    pub kind: ChapterKind,
}

/// An anime episode, for looking up its openings and endings on AniSkip (M45).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AnimeEpisode {
    /// The series' name, searched on AniList when there is no MyAnimeList id.
    pub title: String,
    pub episode: u32,
    pub mal_id: Option<u32>,
    /// The entry's Kitsu or AniList id, whose MyAnimeList id is looked up before searching by
    /// name (each season has its own).
    #[serde(default)]
    pub kitsu_id: Option<u32>,
    #[serde(default)]
    pub anilist_id: Option<u32>,
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
    /// The title it belongs to, whose remembered speed, aspect and tracks it uses (M45):
    /// `title:<id>` or `stremio:<type>:<id>`. None: the video's own key.
    #[serde(default)]
    pub group: Option<String>,
    #[serde(default)]
    pub chapters: Vec<VideoChapter>,
    #[serde(default)]
    pub anime: Option<AnimeEpisode>,
}

/// Whether a message for the webview's video player (M46.4) opens it or changes what it plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum VideoPlayerMessageKind {
    Open,
    Update,
}

/// A message for the video player in the webview (desktop, M46.4). `json` is exactly what the
/// Android player receives: a request when opening, `{ "type": "load" | "loading" | … }` updates.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoPlayerMessage {
    pub kind: VideoPlayerMessageKind,
    pub json: String,
}

/// A YouTube video to watch in its own player (trailers, YouTube streams; M46.4).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct YouTubeVideo {
    pub video_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// The embed host page, with YouTube's own controls.
    pub embed_url: String,
}

/// What the desktop player hands to mpv or VLC (M46.4).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalPlay {
    /// The player's address for the stream (through the proxy).
    /// (M46.33: which app is the command's `player` argument.)
    pub url: String,
    pub title: String,
    /// Subtitle files' addresses; the first is passed on.
    pub subtitles: Vec<String>,
    pub start_ms: f64,
}

/// Subtitles found for a video in the player (M46.33).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct FoundSubtitles {
    pub tracks: Vec<VideoSubtitle>,
    /// The suggested subtitles addon to add, when no added addon serves subtitles.
    pub suggestion: Option<StremioSuggestion>,
    /// Why nothing could be looked for (a video from a source that names no film or series).
    pub note: Option<String>,
}

/// A video player installed on this computer (M46.33).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct ExternalPlayer {
    /// `mpv`, `vlc`, `mpc-hc`, `mpc-be` or `potplayer`.
    pub id: String,
    pub name: String,
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
    /// Off: listed, asked for nothing.
    pub enabled: bool,
    /// What it offers, as the Addons page lists it.
    pub provides: Vec<StremioProvides>,
    /// Its streams are torrents (`behaviorHints.p2p`).
    pub p2p: bool,
    /// Its setup page, when it can be set up.
    pub configure_url: Option<String>,
    /// It must be set up on its website before it's added.
    pub needs_setup: bool,
    /// It didn't answer lately: failed since it last worked, within three days.
    pub failing: bool,
}

/// One thing an addon offers: catalogs of a type (with how many), details of a type, streams,
/// subtitles.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioProvides {
    /// `catalog`, `meta`, `stream` or `subtitles`.
    pub resource: String,
    /// The type it's for (`movie`, `series`...), for catalogs and details.
    pub kind: Option<String>,
    pub count: u32,
}

/// A catalog an addon offers, before anything is asked of it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioCatalogInfo {
    pub addon_id: String,
    pub addon_name: String,
    pub kind: String,
    pub catalog_id: String,
    pub name: String,
    /// What it can be narrowed by (genres, years...), with the choices.
    pub extras: Vec<StremioExtra>,
    /// A row on the board: it lists without being asked anything.
    pub board: bool,
    pub searchable: bool,
    /// Discover can browse it (what it requires has choices).
    pub discoverable: bool,
    /// It pages (`skip`).
    pub pages: bool,
    /// Its addon didn't answer lately.
    pub failing: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioExtra {
    /// `genre`, or another name an addon gives.
    pub name: String,
    /// The catalog lists nothing without one (its first choice then).
    pub required: bool,
    pub options: Vec<String>,
}

/// A choice for a catalog's extra.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioExtraValue {
    pub name: String,
    pub value: String,
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
    /// Saved on the device (M43).
    pub downloaded: bool,
}

/// A rating and who gave it (M46.45): IMDb's, or another site an addon links.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct TitleRating {
    /// "IMDb", or the site's name as the addon gives it.
    pub source: String,
    /// As given: "7.8", "85%", "8/10".
    pub value: String,
    /// The title's page on that site.
    pub url: Option<String>,
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
    /// Ratings with their sources (M46.45), from the addon whose details these are.
    pub ratings: Vec<TitleRating>,
    pub cast: Vec<String>,
    pub director: Vec<String>,
    pub trailer_yt_ids: Vec<String>,
    /// Episodes, by season and number; empty for a movie.
    pub videos: Vec<StremioVideo>,
    /// The episode Continue plays: the one watched last, or the next one that is out once that was
    /// finished; the first unwatched one (not an extra) before any. None for a movie.
    pub continue_video_id: Option<String>,
    /// For a movie: where watching stopped.
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    pub watched: bool,
    /// For a movie: saved on the device (M43).
    pub downloaded: bool,
    pub addon_name: String,
    /// The addon whose details these are (empty when shown as stored).
    pub addon_id: String,
    /// What the title itself plays by (a movie, a channel): the details' default video, else
    /// its id.
    pub video_id: String,
    /// A live channel.
    pub live: bool,
    /// Saved in the video library (M43).
    pub in_library: bool,
    /// The stored title, once it was saved.
    pub title_id: Option<String>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StremioStreamKind {
    /// Plays in Raygun's player.
    Direct,
    /// Plays in the embedded YouTube player.
    YouTube,
    /// Opens outside Raygun (a web page).
    External,
    /// A torrent: a torrent app plays it; `target` is its magnet link.
    Torrent,
    /// Opens another title's page; `target` is `<type>/<id>`.
    Title,
    /// Archives, Usenet and other streams only Stremio's streaming server opens.
    Server,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioStream {
    pub kind: StremioStreamKind,
    pub name: Option<String>,
    pub description: Option<String>,
    /// URL, YouTube id, external URL or info hash, by `kind`.
    pub target: String,
    pub headers: std::collections::BTreeMap<String, String>,
    pub subtitles: Vec<VideoSubtitle>,
    /// Streams of one release across episodes (Stremio's `bingeGroup`), for binge watching.
    pub binge_group: Option<String>,
    /// The file's name, size (bytes) and OpenSubtitles hash, which subtitle addons match by.
    #[serde(default)]
    pub filename: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    #[serde(default)]
    pub video_hash: Option<String>,
    /// A browser can't play it as it is (`behaviorHints.notWebReady`).
    #[serde(default)]
    pub not_web_ready: bool,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioStreamGroup {
    pub addon_id: String,
    pub addon_name: String,
    pub streams: Vec<StremioStream>,
    pub error: Option<String>,
    /// The addon didn't answer lately.
    pub failing: bool,
}

/// What to play: a stream of a movie or episode, with what the player shows.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct StremioPlayRequest {
    pub kind: String,
    /// The movie's id, or the episode's (`tt0903747:1:2`).
    pub video_id: String,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub stream: StremioStream,
    /// The movie's or series' id, whose remembered player settings apply (M45).
    #[serde(default)]
    pub meta_id: Option<String>,
    /// Opens it in another app instead of Raygun's player (M45).
    #[serde(default)]
    pub external: bool,
}

// ---- Local network access (Android 17) ----

/// Android 17's local network permission, for one connection: reaching an addon on the user's
/// network, or (with no address) devices on the network reaching the LAN remote.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct LocalNetworkAccess {
    /// The system restricts the local network for Raygun and the connection goes there.
    pub needed: bool,
    pub granted: bool,
    /// Denied for good when Android was last asked: it won't ask again, so only Raygun's app
    /// settings can allow it. Settings can change that since, so this is a hint until the next ask.
    pub blocked: bool,
}

// ---- Permissions (Settings → Permissions) ----

/// A permission Raygun uses on this device.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PermissionKind {
    /// Download progress and finished downloads (playback controls show without it).
    Notifications,
    /// Running in the background without Android's battery limits.
    Battery,
    /// Reading the phone's songs and videos (Local Files, the device's music).
    Media,
    /// Folders picked in the system's folder picker.
    Folders,
    /// Android 17's local network access.
    LocalNetwork,
    /// Installing Raygun's own updates.
    InstallApps,
}

/// Where a permission stands.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum AccessState {
    /// Allowed (battery: unrestricted).
    Granted,
    /// Not allowed; asking shows the system's question.
    Denied,
    /// Not allowed, and only the system settings can change it now.
    Blocked,
    /// Battery: Android's default limits apply (asking shows its question).
    Optimized,
    /// Battery: restricted in the background, so playback and downloads stop when Raygun leaves
    /// the screen; only the system settings can lift it.
    Restricted,
}

/// One row of Settings → Permissions.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct AppPermission {
    pub kind: PermissionKind,
    pub state: AccessState,
    /// Folders: how many picked folders Raygun may still read.
    pub count: Option<u32>,
}

/// What a permission is asked for when a feature first needs it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum PermissionReason {
    /// The first playback: Raygun is restricted, so music would stop in the background.
    Playback,
    /// The first download: progress shows in notifications.
    Download,
    /// Adding a folder to Local Files: its songs and videos are read by path.
    Folders,
}

/// A question to show once: the permission and why it's needed now.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct PermissionPrompt {
    pub kind: PermissionKind,
    pub reason: PermissionReason,
}

// ---- Video sources (M42) ----

/// A video source's title page.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoTitleView {
    /// The stored title's id.
    pub id: String,
    pub source_id: String,
    pub source_name: String,
    pub remote_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    /// `movie`, `series`, `anime` or `other`.
    pub media_type: Option<String>,
    pub year: Option<i32>,
    pub description: Option<String>,
    pub genres: Vec<String>,
    pub cast: Vec<String>,
    pub directors: Vec<String>,
    pub status: String,
    /// The title's page on the source's website.
    pub url: Option<String>,
    pub in_library: bool,
    /// Adult content while NSFW is off: the page shows a notice instead.
    pub hidden_nsfw: bool,
    /// In the source's order; a movie has one.
    pub episodes: Vec<VideoEpisodeView>,
    /// The episode Continue plays (its stored id): the one watched last, or the next one that is
    /// out once that was finished; the first unwatched one before any.
    pub continue_episode_id: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoEpisodeView {
    /// The stored episode's id.
    pub id: String,
    /// The source's id for it, passed to `video_servers`.
    pub remote_id: String,
    pub name: String,
    pub number: Option<f64>,
    pub season: Option<f64>,
    /// Unix ms.
    pub date: Option<f64>,
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    pub watched: bool,
    /// Saved on the device (M43).
    pub downloaded: bool,
}

/// One comment on a video (M46.45), as the source shows it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoComment {
    pub author: String,
    #[serde(default)]
    pub avatar_url: Option<String>,
    pub text: String,
    /// "220K".
    #[serde(default)]
    pub likes: Option<String>,
    /// "6 years ago".
    #[serde(default)]
    pub published: Option<String>,
    #[serde(default)]
    pub replies: Option<u32>,
}

/// A page of a video's comments; `next` asks for the following one.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoComments {
    pub comments: Vec<VideoComment>,
    #[serde(default)]
    pub next: Option<String>,
}

/// A server (hoster) for an episode; `links` when the source resolved it already.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoServerView {
    pub id: String,
    pub name: String,
    pub links: Option<Vec<VideoLinkView>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoLinkView {
    pub url: String,
    pub label: Option<String>,
    /// Height in pixels.
    pub quality: Option<u32>,
    pub headers: std::collections::BTreeMap<String, String>,
    /// `hls` or `dash`; none for a plain video file.
    pub format: Option<String>,
    pub subtitles: Vec<VideoSubtitle>,
    /// The link the preferred quality picks (the first of its server).
    pub preferred: bool,
    /// Openings, endings and other chapters the source marks (M45).
    #[serde(default)]
    pub chapters: Vec<VideoChapter>,
}

/// Plays a link of a stored episode.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoSourcePlay {
    pub title_id: String,
    pub episode_id: String,
    pub link: VideoLinkView,
    /// Opens it in another app instead of Raygun's player (M45).
    #[serde(default)]
    pub external: bool,
}

// ---- Video library, updates and downloads (M43) ----

/// An episode of a library title: new from a library update, or announced for later.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoUpdateItem {
    pub title_id: String,
    /// A video source, or `stremio` (then `remote_id` is `<type>:<id>`).
    pub source_id: String,
    pub remote_id: String,
    pub title: String,
    pub cover_url: Option<String>,
    /// The stored episode's id.
    pub episode_id: String,
    /// The source's id for it (Stremio: the video id).
    pub episode_remote_id: String,
    pub name: String,
    pub season: Option<f64>,
    pub number: Option<f64>,
    /// Release date, Unix ms.
    pub date: Option<f64>,
    /// When a library update found it, Unix ms.
    pub found_at: f64,
    pub watched: bool,
    pub downloaded: bool,
}

/// One video download.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoDownloadItem {
    pub id: String,
    /// The progress key it plays under.
    pub key: String,
    pub title_id: Option<String>,
    pub title: String,
    pub subtitle: Option<String>,
    pub poster: Option<String>,
    pub state: DownloadState,
    pub done_bytes: f64,
    pub total_bytes: Option<f64>,
    /// HLS segments.
    pub done_parts: u32,
    pub total_parts: Option<u32>,
    pub error: Option<String>,
}

/// The video downloads, newest first.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct VideoDownloads {
    pub paused: bool,
    pub items: Vec<VideoDownloadItem>,
}
