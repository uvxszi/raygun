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
