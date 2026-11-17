//! The video player as Play on TV reaches it: raygun-app sets a [`Player`] state at start, since
//! the player lives above this crate.

use std::sync::Arc;

use serde_json::Value;
use tauri::{AppHandle, Manager};

/// What Play on TV asks of the video player.
pub trait PlayerLink: Send + Sync {
    /// Sends the player an update (as `video::update_player`).
    fn update(&self, update: &Value) -> Result<(), String>;
    /// Hands a report to the player's report queue, as if the player sent it.
    fn report(&self, report: String);
    /// The player's words in the app's language.
    fn labels(&self) -> Value;
}

/// The video player, as Tauri state.
pub type Player = Arc<dyn PlayerLink>;

pub(crate) fn update(app: &AppHandle, update: &Value) -> Result<(), String> {
    app.state::<Player>().update(update)
}

pub(crate) fn report(app: &AppHandle, report: String) {
    app.state::<Player>().report(report);
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub(crate) fn labels(app: &AppHandle) -> Value {
    app.state::<Player>().labels()
}
