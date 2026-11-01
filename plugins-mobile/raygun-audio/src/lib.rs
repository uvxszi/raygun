//! Native playback engine for Android (ARCHITECTURE §6.2, M06).
//!
//! The Kotlin side hosts a Media3 player in a media session service, which keeps playback alive in
//! the background and provides the notification and lock-screen controls. Rust sends it
//! [`EngineCommand`]s through [`NativeAudio::send`]; the player's reports come back as
//! `EngineReport` JSON through the handler installed with [`set_report_handler`], over JNI rather
//! than the webview. On other platforms the plugin does nothing and [`NativeAudio`] is not managed.

#[cfg(target_os = "android")]
mod android;

use std::sync::OnceLock;

use raygun_model::EngineCommand;
use tauri::Runtime;
use tauri::plugin::{Builder, TauriPlugin};

type ReportHandler = Box<dyn Fn(String) + Send + Sync>;

static REPORT_HANDLER: OnceLock<ReportHandler> = OnceLock::new();
static CONTROL_HANDLER: OnceLock<ReportHandler> = OnceLock::new();
static TTS_HANDLER: OnceLock<ReportHandler> = OnceLock::new();

/// Installs the receiver for text-to-speech progress (`{"state", "paragraph"}` JSON, M33).
/// Called on the Android main thread, like the report handler. Only the first call wins.
pub fn set_tts_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    let _ = TTS_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_tts(json: String) {
    if let Some(handler) = TTS_HANDLER.get() {
        handler(json);
    }
}

/// One piece of text to read aloud; `id` comes back in progress reports.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct SpeechItem {
    pub id: String,
    pub text: String,
}

/// Installs the receiver for engine reports (JSON in the `EngineReport` shape). It is called on
/// the Android main thread, so it must hand work off rather than block. Only the first call wins.
pub fn set_report_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    let _ = REPORT_HANDLER.set(Box::new(handler));
}

/// Installs the receiver for controls from the system UI or headsets ("next", "previous"). Called
/// on the Android main thread, like the report handler. Only the first call wins.
pub fn set_control_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    let _ = CONTROL_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_report(json: String) {
    if let Some(handler) = REPORT_HANDLER.get() {
        handler(json);
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_control(control: String) {
    if let Some(handler) = CONTROL_HANDLER.get() {
        handler(control);
    }
}

/// Handle to the Kotlin engine, managed as Tauri state on Android only.
pub struct NativeAudio<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

/// Result of asking Android for access to the device's music.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LibraryAccess {
    pub granted: bool,
    /// Public folders worth scanning (Music, Download).
    pub folders: Vec<String>,
}

impl<R: Runtime> NativeAudio<R> {
    /// Shows the system permission dialog if needed. Blocks until the user answers, so call it
    /// from a blocking-friendly thread.
    #[cfg(target_os = "android")]
    pub fn request_library_access(&self) -> Result<LibraryAccess, String> {
        self.handle
            .run_mobile_plugin("requestLibraryAccess", ())
            .map_err(|error| error.to_string())
    }

    /// Matches the system bars to Raygun's theme (`system`, `light`, `dark`, `amoled`).
    #[cfg(target_os = "android")]
    pub fn apply_theme(&self, theme: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "applyTheme",
                serde_json::json!({ "theme": theme }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "android"))]
    pub fn apply_theme(&self, _theme: &str) -> Result<(), String> {
        Ok(())
    }

    #[cfg(not(target_os = "android"))]
    pub fn request_library_access(&self) -> Result<LibraryAccess, String> {
        Err("library access requests are only needed on Android".into())
    }

    /// Whether the phone can read aloud. Blocks while the engine starts.
    #[cfg(target_os = "android")]
    pub fn tts_available(&self) -> bool {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("ttsAvailable", ())
            .ok()
            .and_then(|answer| answer.get("available").and_then(serde_json::Value::as_bool))
            .unwrap_or(false)
    }

    #[cfg(not(target_os = "android"))]
    pub fn tts_available(&self) -> bool {
        false
    }

    /// Replaces what is being read with `items`, at `rate` (1 = normal).
    #[cfg(target_os = "android")]
    pub fn tts_speak(&self, items: &[SpeechItem], rate: f64) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "ttsSpeak",
                serde_json::json!({ "items": items, "rate": rate }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "android"))]
    pub fn tts_speak(&self, _items: &[SpeechItem], _rate: f64) -> Result<(), String> {
        Err("text-to-speech is only available on Android".into())
    }

    #[cfg(target_os = "android")]
    pub fn tts_stop(&self) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("ttsStop", ())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "android"))]
    pub fn tts_stop(&self) -> Result<(), String> {
        Ok(())
    }

    /// Forwards one engine command to the native player.
    #[cfg(target_os = "android")]
    pub fn send(&self, command: &EngineCommand) -> Result<(), String> {
        android::send(&self.handle, command)
    }

    #[cfg(not(target_os = "android"))]
    pub fn send(&self, _command: &EngineCommand) -> Result<(), String> {
        Err("the native audio engine is only available on Android".into())
    }
}

pub fn init<R: Runtime>() -> TauriPlugin<R> {
    Builder::new("raygun-audio")
        .setup(|_app, _api| {
            #[cfg(target_os = "android")]
            {
                use tauri::Manager;
                let handle =
                    _api.register_android_plugin("app.raygun.audio", "RaygunAudioPlugin")?;
                _app.manage(NativeAudio { handle });
            }
            Ok(())
        })
        .build()
}
