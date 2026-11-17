//! What crates below raygun-app ask of it by name, set at start (raygun-app's `startup`): a route
//! opened in the UI, an address handed to Open with, a video sent to a linked browser.

use std::path::PathBuf;
use std::sync::OnceLock;

use raygun_model::VideoPlayerMessageKind;
use serde_json::Value;
use tauri::AppHandle;

/// raygun-app's side of the hooks.
pub struct Hooks {
    /// Opens a route in the UI (a notification's tap).
    pub open_route: fn(&AppHandle, String),
    /// Hands an address over as Open with does.
    pub hand_over: fn(&AppHandle, &str, Option<&str>),
    /// Sends a video player message to a linked browser; whether one took it.
    pub to_viewer: fn(&AppHandle, VideoPlayerMessageKind, &Value) -> bool,
    /// The path of a `file:` address.
    pub file_path: fn(&str) -> Option<PathBuf>,
}

static HOOKS: OnceLock<Hooks> = OnceLock::new();

/// Sets the hooks, once at start.
pub fn set(hooks: Hooks) {
    let _ = HOOKS.set(hooks);
}

pub fn open_route(app: &AppHandle, route: String) {
    if let Some(hooks) = HOOKS.get() {
        (hooks.open_route)(app, route);
    }
}

pub fn hand_over(app: &AppHandle, url: &str, title: Option<&str>) {
    if let Some(hooks) = HOOKS.get() {
        (hooks.hand_over)(app, url, title);
    }
}

pub fn to_viewer(app: &AppHandle, kind: VideoPlayerMessageKind, payload: &Value) -> bool {
    HOOKS
        .get()
        .is_some_and(|hooks| (hooks.to_viewer)(app, kind, payload))
}

pub fn file_path(address: &str) -> Option<PathBuf> {
    HOOKS.get().and_then(|hooks| (hooks.file_path)(address))
}
