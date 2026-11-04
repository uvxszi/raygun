//! What the device lets Raygun do right now: whether it's on screen, battery saver, and the
//! system's animation settings.
//!
//! On Android the `raygun-audio` plugin reports all three (`DeviceState.kt`: Raygun's activities
//! starting and stopping, `ACTION_POWER_SAVE_MODE_CHANGED`, the animator, transition and window
//! scales). The webview adds its own visibility (`device_set_visible`), which is all desktop has: a
//! minimized or hidden window. The UI follows `DeviceStateEvent` (animations, polling); background
//! loops here call [`rest`] instead of sleeping, which stretches their waits while Raygun is in the
//! background with nothing playing, or in battery saver. Downloads wait in battery saver unless
//! `Settings::downloads_in_battery_saver` allows them ([`wait_for_downloads`]).

use std::sync::Mutex;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::watch;

/// How much longer background loops wait while Raygun is idle in the background or saving power.
const RELAXED_FACTOR: u32 = 6;

/// How much the system lets things move.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SystemMotion {
    /// The system's animations run at their usual speed (or faster).
    #[default]
    Full,
    /// Animations are slowed down or cut short by the system (a scale below 1, a transition scale
    /// of 0): only small, short changes.
    Reduced,
    /// Remove animations, or an animator scale of 0: nothing moves.
    Off,
}

/// The device's state as the UI sees it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct DeviceState {
    /// Raygun is on screen (an activity started on Android; the webview visible).
    pub foreground: bool,
    /// Battery saver is on.
    pub power_save: bool,
    /// The system's animation settings.
    pub motion: SystemMotion,
}

impl Default for DeviceState {
    fn default() -> Self {
        Self {
            foreground: true,
            power_save: false,
            motion: SystemMotion::Full,
        }
    }
}

/// The device's state changed.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct DeviceStateEvent(pub DeviceState);

/// What the plugin reports (`DeviceState.kt`).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct NativeReport {
    #[serde(default = "yes")]
    foreground: bool,
    #[serde(default)]
    power_save: bool,
    #[serde(default = "one")]
    animator_scale: f64,
    #[serde(default = "one")]
    transition_scale: f64,
    #[serde(default = "one")]
    window_scale: f64,
    #[serde(default = "yes")]
    animators_enabled: bool,
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn yes() -> bool {
    true
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn one() -> f64 {
    1.0
}

/// The system's animation settings as one level: a scale of 0 for animators (or animators
/// disabled) stops everything; any scale between 0 and 1, or transitions and windows set to 0
/// alone, asks for less.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn motion_of(report: &NativeReport) -> SystemMotion {
    if !report.animators_enabled || report.animator_scale <= 0.0 {
        return SystemMotion::Off;
    }
    let scales = [
        report.animator_scale,
        report.transition_scale,
        report.window_scale,
    ];
    if scales.iter().any(|scale| *scale < 1.0) {
        SystemMotion::Reduced
    } else {
        SystemMotion::Full
    }
}

/// Everything known, kept apart so each source changes only its own part.
#[derive(Debug, Clone, Copy, PartialEq)]
struct Known {
    native_foreground: bool,
    webview_visible: bool,
    power_save: bool,
    motion: SystemMotion,
}

impl Known {
    const START: Self = Self {
        native_foreground: true,
        webview_visible: true,
        power_save: false,
        motion: SystemMotion::Full,
    };

    fn state(&self) -> DeviceState {
        DeviceState {
            foreground: self.native_foreground && self.webview_visible,
            power_save: self.power_save,
            motion: self.motion,
        }
    }
}

static KNOWN: Mutex<Known> = Mutex::new(Known::START);
/// Something plays or loads (the player's status), set by `PlaybackService`.
static BUSY: AtomicBool = AtomicBool::new(false);
/// Downloads may run in battery saver (`Settings::downloads_in_battery_saver`).
static DOWNLOADS_ALLOWED: AtomicBool = AtomicBool::new(false);

/// Bumped on every change, so waits can end early.
fn changes() -> &'static watch::Sender<u64> {
    static CHANGES: std::sync::OnceLock<watch::Sender<u64>> = std::sync::OnceLock::new();
    CHANGES.get_or_init(|| watch::channel(0).0)
}

fn bump() {
    changes().send_modify(|count| *count = count.wrapping_add(1));
}

/// The state now.
pub fn current() -> DeviceState {
    KNOWN.lock().unwrap_or_else(|p| p.into_inner()).state()
}

/// Whether background loops should take it slow: battery saver, or Raygun in the background
/// with nothing playing.
pub fn relaxed() -> bool {
    let state = current();
    state.power_save || (!state.foreground && !BUSY.load(Ordering::Relaxed))
}

/// `base`, stretched while [`relaxed`].
pub fn pace(base: Duration) -> Duration {
    if relaxed() {
        base * RELAXED_FACTOR
    } else {
        base
    }
}

/// Waits `pace(base)`, ending early when the state changes (Raygun back on screen, battery saver
/// off): a loop then looks again at once, at its usual pace.
pub async fn rest(base: Duration) {
    let mut changed = changes().subscribe();
    tokio::select! {
        () = tokio::time::sleep(pace(base)) => {}
        _ = changed.changed() => {}
    }
}

/// Waits until something plays or loads. A loop that only matters then (a watchdog) costs
/// nothing in between.
pub async fn wait_until_busy() {
    let mut changed = changes().subscribe();
    while !BUSY.load(Ordering::Relaxed) {
        if changed.changed().await.is_err() {
            return;
        }
    }
}

/// Whether downloads should wait now (battery saver, unless allowed).
pub fn downloads_held() -> bool {
    current().power_save && !DOWNLOADS_ALLOWED.load(Ordering::Relaxed)
}

/// Waits while [`downloads_held`]; returns at once otherwise.
pub async fn wait_for_downloads() {
    let mut changed = changes().subscribe();
    while downloads_held() {
        if changed.changed().await.is_err() {
            return;
        }
    }
}

/// Follows `Settings::downloads_in_battery_saver`.
pub fn set_downloads_allowed(allowed: bool) {
    if DOWNLOADS_ALLOWED.swap(allowed, Ordering::Relaxed) != allowed {
        bump();
    }
}

/// Called with every playback state: loading, buffering or playing counts as busy.
pub fn set_busy(busy: bool) {
    if BUSY.swap(busy, Ordering::Relaxed) != busy {
        bump();
    }
}

/// Applies `change` and tells the UI when what it sees changed.
fn update(app: Option<&AppHandle>, change: impl FnOnce(&mut Known)) {
    let (before, after) = {
        let mut known = KNOWN.lock().unwrap_or_else(|p| p.into_inner());
        let before = *known;
        change(&mut known);
        (before, *known)
    };
    if before == after {
        return;
    }
    bump();
    if before.state() != after.state()
        && let Some(app) = app
        && let Err(error) = DeviceStateEvent(after.state()).emit(app)
    {
        tracing::debug!(%error, "device state not sent");
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn apply_native(app: Option<&AppHandle>, json: &str) {
    let Ok(report) = serde_json::from_str::<NativeReport>(json) else {
        tracing::debug!(json, "device state not understood");
        return;
    };
    let motion = motion_of(&report);
    update(app, |known| {
        known.native_foreground = report.foreground;
        known.power_save = report.power_save;
        known.motion = motion;
    });
}

/// Starts following the plugin's reports (Android) and reads the state once. The setting for
/// downloads comes through `settings::apply_live`.
pub fn install(#[cfg_attr(not(target_os = "android"), allow(unused_variables))] app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        let handle = app.clone();
        tauri_plugin_raygun_audio::set_device_state_handler(move |json| {
            apply_native(Some(&handle), &json);
        });
        let handle = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            match handle
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .device_state()
            {
                Ok(value) => apply_native(Some(&handle), &value.to_string()),
                Err(error) => tracing::warn!(%error, "device state unknown"),
            }
        });
    }
}

/// The device's state now.
#[tauri::command]
#[specta::specta]
pub fn device_state() -> DeviceState {
    current()
}

/// The webview's visibility (`document.visibilityState`): a hidden page counts as background.
#[tauri::command]
#[specta::specta]
pub fn device_set_visible(app: AppHandle, visible: bool) {
    update(Some(&app), |known| known.webview_visible = visible);
}

#[cfg(test)]
mod tests {
    use super::*;

    fn report(json: &str) -> NativeReport {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn animation_settings_become_one_level() {
        assert_eq!(motion_of(&report("{}")), SystemMotion::Full);
        assert_eq!(
            motion_of(&report(r#"{"animatorScale": 1.5}"#)),
            SystemMotion::Full
        );
        // Remove animations sets every scale to 0; animators report themselves disabled.
        assert_eq!(
            motion_of(&report(
                r#"{"animatorScale": 0, "transitionScale": 0, "windowScale": 0}"#
            )),
            SystemMotion::Off
        );
        assert_eq!(
            motion_of(&report(r#"{"animatorsEnabled": false}"#)),
            SystemMotion::Off
        );
        assert_eq!(
            motion_of(&report(r#"{"animatorScale": 0.5}"#)),
            SystemMotion::Reduced
        );
        assert_eq!(
            motion_of(&report(r#"{"transitionScale": 0}"#)),
            SystemMotion::Reduced
        );
    }

    /// One test drives the shared state, so parallel tests don't see each other's changes.
    #[tokio::test]
    async fn loops_slow_down_in_the_background_and_in_battery_saver() {
        let base = Duration::from_secs(8);
        set_busy(false);
        apply_native(None, r#"{"foreground": true}"#);
        assert_eq!(pace(base), base);

        // In the background with nothing playing: slower.
        apply_native(None, r#"{"foreground": false}"#);
        assert!(!current().foreground);
        assert_eq!(pace(base), base * RELAXED_FACTOR);
        // Playing in the background keeps the usual pace.
        set_busy(true);
        assert_eq!(pace(base), base);
        set_busy(false);

        // A hidden webview counts as background even when the activity is started.
        apply_native(None, r#"{"foreground": true}"#);
        update(None, |known| known.webview_visible = false);
        assert!(!current().foreground);
        update(None, |known| known.webview_visible = true);
        assert_eq!(pace(base), base);

        // Battery saver: slower even on screen, and downloads wait unless allowed.
        apply_native(None, r#"{"foreground": true, "powerSave": true}"#);
        assert_eq!(pace(base), base * RELAXED_FACTOR);
        assert!(downloads_held());
        set_downloads_allowed(true);
        assert!(!downloads_held());
        set_downloads_allowed(false);

        // A rest ends as soon as the state changes.
        let waiting = tokio::spawn(rest(Duration::from_secs(3600)));
        tokio::task::yield_now().await;
        apply_native(None, r#"{"foreground": true}"#);
        tokio::time::timeout(Duration::from_secs(5), waiting)
            .await
            .expect("rest ended on the change")
            .unwrap();
        // So does waiting for downloads.
        assert!(!downloads_held());
        tokio::time::timeout(Duration::from_secs(1), wait_for_downloads())
            .await
            .unwrap();
    }
}
