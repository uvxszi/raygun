//! Typed events pushed from Rust to the UI (exported to `packages/bindings` by tauri-specta).

use raygun_model::{EngineCommand, PlaybackState, QueueSnapshot};
use serde::{Deserialize, Serialize};
use specta::Type;

/// An instruction for the playback engine running in the webview.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct EngineCommandEvent(pub EngineCommand);

/// The canonical player state changed.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct PlaybackStateEvent(pub PlaybackState);

/// The queue changed.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct QueueEvent(pub QueueSnapshot);

/// Where this device's player plays changed, or the device it plays on reported (M46.15).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ConnectEvent(pub raygun_model::ConnectStatus);

/// Another device asks to play on this one, or the question closed (M46.15).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ConnectRequestEvent(pub raygun_model::ConnectRequest);

/// A feature needs a permission for the first time: the UI asks, with the reason.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct PermissionPromptEvent(pub raygun_model::PermissionPrompt);

/// A screen to open: a tapped notification's (`/downloads?tab=music`, or `player`), or the one
/// for what another app handed over (`player`, a search, `import?link=` for a Spotify link).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct OpenRouteEvent(pub String);

/// Progress of a local library scan.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct LibraryScanEvent(pub raygun_model::LibraryScanProgress);

/// A sync with another device finished and changed rows here; carries how many. Cached
/// settings, favorites, playlists and history are stale.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SyncChangedEvent(pub u32);

/// A manga library update made progress or finished (M28).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MangaUpdateEvent(pub raygun_model::MangaUpdateStatus);

/// The manga download queue changed; carries the chapter id when one chapter did (else empty).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MangaDownloadsEvent(pub String);

/// The video downloads changed; carries the download's id when one did (else empty) (M43).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct VideoDownloadsEvent(pub String);

/// Raygun's own update changed: checked, downloading, installing (M46.47).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct AppUpdateEvent(pub raygun_model::AppUpdate);

/// yt-dlp's downloads or its install changed; carries the job's id when one did (M46.47).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct YtDlpEvent(pub String);

/// Files saved from websites changed; carries the download's id (empty when the list did).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct WebsiteDownloadsEvent(pub String);

/// Setting up the Mihon run-source-code tools: the current runtime state (its `settingUp` says
/// whether a fetch is still going).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MihonRuntimeEvent(pub raygun_model::MihonRuntimeState);

/// The song downloads changed; carries the download's id when one did (else empty) (M46.10).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SongDownloadsEvent(pub String);

/// One song download as it is now: sent when its state or source changes, and while it downloads
/// or is captured, at most four times a second and only when what it shows moved on (its percent,
/// or 100 KB without a known size). Not sent for removals (`SongDownloadsEvent` is).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SongDownloadProgressEvent(pub raygun_model::SongDownload);

/// A stream for the desktop webview to capture in the background (M46.49), or a YouTube video for
/// either platform's webview to capture from a hidden player of its own (`video_id`, M46.52), or
/// (neither) to stop capturing; it reports with `song_capture_report`.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SongCaptureEvent(pub raygun_model::SongCaptureRequest);

/// An import matched another song in the background; carries the playlist's id (M46.31).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ImportEvent(pub String);

/// Playback found no source for a song, or found one for it again: its unavailable mark and
/// saved copies changed; carries the song's title.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct UnavailableEvent(pub String);

/// An episode's captions changed: a feed transcript arrived, a window was transcribed, or
/// transcribing started or stopped; carries the episode's id (M46.32).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct CaptionsEvent(pub String);

/// A song's transcription finished and its words are now the song's lyrics; carries its
/// captions key (`song:<track key>`).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct LyricsTranscribedEvent(pub String);

/// How far a song's lyrics translation on this device is (M46.48), a few times a second.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct LyricsTranslationEvent(pub raygun_model::LyricsTranslationProgress);

/// A video's translated subtitles changed: lines were translated, or translating started,
/// stopped or failed; carries the video's progress key.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct VideoSubtitlesEvent(pub String);

/// Followed artists or their release news changed (M46.42).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ArtistReleasesEvent;

/// A Local Files folder was added, scanned or removed; carries its id (M46.27).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct LocalFilesEvent(pub String);

/// Song lengths Raygun just learned (M46.26).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct TrackDurationsEvent(pub Vec<raygun_model::TrackDuration>);

/// New Made for You mixes are ready (M46.20).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct MadeForYouEvent;

/// Progress reading a chapter aloud (M33).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct TtsEvent(pub raygun_model::TtsUpdate);

/// Extensions were (re)loaded; carries the new list.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ExtensionsChangedEvent(pub Vec<raygun_model::ExtensionInfo>);

/// The extension install queue changed: an install started, moved on a phase or ended.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ExtensionInstallsEvent(pub raygun_model::ExtensionInstallStatus);

/// One source answered a music search that's still waiting for others; one event per source.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SearchProgressEvent(pub raygun_model::SearchProgress);

/// A model download's progress (M34).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct AiModelEvent(pub raygun_model::AiModelProgress);

/// Podcast data changed (a refresh found episodes, a download moved on); carries the podcast id,
/// or an empty string when several changed (M37).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct PodcastsChangedEvent(pub String);

/// An audiobook source answered after its list was shown (M46.48): `[list, query]`, so the list
/// asks again and shows the late books (kept ready by then).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct AudiobooksFoundEvent(pub String, pub String);

/// An export job changed (M46.1); carries the job, or nothing when one was removed from the list.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ExportEvent(pub Option<raygun_model::ExportJob>);

/// A setting changed outside the Settings screens (in the video player, M45); carries its key.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SettingsChangedEvent(pub String);

/// What quick access lists changed (`quick_access`); carries the page that changed (its route),
/// or an empty string.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct QuickAccessChangedEvent(pub String);

/// Incognito was turned on or off (M47).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct IncognitoEvent(pub raygun_model::IncognitoStatus);

/// Watching progress changed (the player closed or finished something); carries the content key
/// (M38).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct WatchChangedEvent(pub String);

/// A message for the video player in the webview (desktop, M46.4).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct VideoPlayerEvent(pub raygun_model::VideoPlayerMessage);

/// What plays on a TV changed (Play on TV); `None` once nothing does.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct CastStatusEvent(pub Option<raygun_model::CastStatus>);

/// What the webview's speech engine should do (read-aloud on desktop, M46.6).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct SpeechCommandEvent(pub raygun_model::SpeechCommand);

/// A YouTube video to open in its own full-window player (trailers, M46.4).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct YouTubeVideoEvent(pub raygun_model::YouTubeVideo);

/// Something another app opened with Raygun or shared to it didn't play (`open_with`).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct OpenWithEvent(pub raygun_model::OpenWithProblem);
