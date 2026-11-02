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
    /// The video height links are picked by (M42); `None` picks the highest.
    pub preferred_video_quality: Option<u32>,
    /// The colour of buttons, tabs and highlights (M42.1).
    pub accent: Accent,
    /// The screen the app opens on.
    pub start_screen: StartScreen,
    /// Navigation bar labels: on every tab, or on the selected one only.
    pub nav_labels: NavLabels,
    /// Covers per row in library and browse grids; 0 fits them to the screen.
    pub grid_columns: u32,
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
    /// Subtitles turn on by themselves when a video has them in the subtitle language.
    pub auto_subtitles: bool,
    /// Subtitles' language as a BCP 47 tag; empty follows the app's language.
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
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum StartScreen {
    #[default]
    Home,
    Library,
    Browse,
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
    pub subtitle_style: SubtitleStyle,
    /// Empty plays the video's own default track.
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
            volume_boost: 30,
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
            subtitle_style: SubtitleStyle::default(),
            audio_language: String::new(),
            audio_fallback: String::new(),
            pip_on_leave: true,
            pip_on_back: false,
            decoder: DecoderMode::Auto,
            external_player: false,
            external_package: String::new(),
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
            preferred_video_quality: None,
            accent: Accent::Red,
            start_screen: StartScreen::Home,
            nav_labels: NavLabels::Always,
            grid_columns: 0,
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
            SettingChange::PreferredVideoQuality(height) => self.preferred_video_quality = height,
            SettingChange::Accent(accent) => self.accent = accent,
            SettingChange::StartScreen(screen) => self.start_screen = screen,
            SettingChange::NavLabels(labels) => self.nav_labels = labels,
            SettingChange::GridColumns(columns) => self.grid_columns = columns,
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
    PreferredVideoQuality(Option<u32>),
    Accent(Accent),
    StartScreen(StartScreen),
    NavLabels(NavLabels),
    GridColumns(u32),
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
            Self::PreferredVideoQuality(_) => "preferredVideoQuality",
            Self::Accent(_) => "accent",
            Self::StartScreen(_) => "startScreen",
            Self::NavLabels(_) => "navLabels",
            Self::GridColumns(_) => "gridColumns",
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
        }
    }

    /// Checks values the type system can't express.
    pub fn validate(&self) -> Result<(), String> {
        match self {
            Self::Theme(_)
            | Self::ShowNsfw(_)
            | Self::PreferredVideoQuality(None)
            | Self::Accent(_)
            | Self::StartScreen(_)
            | Self::NavLabels(_)
            | Self::CoverStyle(_)
            | Self::UnreadBadges(_)
            | Self::DownloadBadges(_)
            | Self::ResumeVideos(_)
            | Self::AutoSubtitles(_)
            | Self::AutoplayNext(_) => Ok(()),
            Self::NextPromptSeconds(seconds) => {
                if NEXT_PROMPT_SECONDS.contains(seconds) {
                    Ok(())
                } else {
                    Err(format!("{seconds} s isn't an offered prompt time"))
                }
            }
            Self::GridColumns(columns) => {
                if GRID_COLUMNS.contains(columns) {
                    Ok(())
                } else {
                    Err(format!("{columns} covers per row isn't offered"))
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
            Self::Save(prefs) => {
                if prefs.location == SaveLocation::Folder && prefs.folder_uri.is_none() {
                    Err("Pick a folder first".into())
                } else {
                    Ok(())
                }
            }
            Self::LibraryDisplay(prefs) => {
                if GRID_COLUMNS.contains(&prefs.columns_landscape) {
                    Ok(())
                } else {
                    Err(format!(
                        "{} covers per row isn't offered",
                        prefs.columns_landscape
                    ))
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
    /// The icon its repository lists, through the cover cache (M46.2).
    pub icon: Option<String>,
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
}

impl Default for SavePrefs {
    fn default() -> Self {
        Self {
            location: SaveLocation::App,
            folder_uri: None,
            folder_name: None,
            chapter_format: ChapterFormat::Cbz,
            pages_per_title: true,
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
    /// Covers per row in landscape; 0 fits them to the screen. (`Settings::grid_columns` is the
    /// portrait value.)
    pub columns_landscape: u32,
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
            columns_landscape: 0,
            tabs: LibraryTabs::Categories,
            show_counts: true,
            per_category: false,
            continue_button: true,
            local_badge: true,
            pins_on_top: true,
        }
    }
}

/// Covers per row offered (0 fits them to the screen).
pub const GRID_COLUMNS: [u32; 8] = [0, 2, 3, 4, 5, 6, 7, 8];

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
    /// Saved on the device (M43).
    pub downloaded: bool,
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
    /// For a movie: saved on the device (M43).
    pub downloaded: bool,
    pub addon_name: String,
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
    /// Streams of one release across episodes (Stremio's `bingeGroup`), for binge watching.
    pub binge_group: Option<String>,
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
