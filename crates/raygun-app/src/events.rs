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

/// Progress reading a chapter aloud (M33).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct TtsEvent(pub raygun_model::TtsUpdate);

/// Extensions were (re)loaded; carries the new list.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct ExtensionsChangedEvent(pub Vec<raygun_model::ExtensionInfo>);

/// A model download's progress (M34).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct AiModelEvent(pub raygun_model::AiModelProgress);

/// Podcast data changed (a refresh found episodes, a download moved on); carries the podcast id,
/// or an empty string when several changed (M37).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct PodcastsChangedEvent(pub String);

/// Watching progress changed (the player closed or finished something); carries the content key
/// (M38).
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct WatchChangedEvent(pub String);
