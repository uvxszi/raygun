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
static VIDEO_HANDLER: OnceLock<ReportHandler> = OnceLock::new();
static WEBSITE_HANDLER: OnceLock<ReportHandler> = OnceLock::new();

/// Installs the receiver for the built-in browser's reports (`{"id", "url", "blockAds"}` JSON,
/// M46.3). Called on the Android main thread. Only the first call wins.
pub fn set_website_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    let _ = WEBSITE_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_website(json: String) {
    if let Some(handler) = WEBSITE_HANDLER.get() {
        handler(json);
    }
}

/// Installs the receiver for video player reports (`{"id", "state", "positionMs", "durationMs"}`
/// JSON, M38). Called on the Android main thread. Only the first call wins.
pub fn set_video_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    let _ = VIDEO_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_video(json: String) {
    if let Some(handler) = VIDEO_HANDLER.get() {
        handler(json);
    }
}

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

/// Android 17's local network permission, as the system reports it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Serialize, serde::Deserialize)]
pub struct LocalNetworkState {
    /// The system restricts the local network for the app (Android 17+, target SDK 37+).
    pub enforced: bool,
    /// Granted, or not needed.
    pub granted: bool,
    /// Denied for good: only the app's settings can allow it now.
    pub blocked: bool,
}

/// A folder chosen in the system picker (M46.1); both are missing when the user cancelled.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PickedFolder {
    pub uri: Option<String>,
    pub name: Option<String>,
}

/// Whether Raygun may still write to a picked folder.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct FolderState {
    pub granted: bool,
    pub name: Option<String>,
}

/// Where a file made outside the app goes.
#[derive(Debug, Clone, serde::Serialize)]
pub struct OutputRequest<'a> {
    /// `downloads`, `pictures` or `tree`.
    pub place: &'a str,
    /// The picked folder, for `tree`.
    pub tree: Option<&'a str>,
    /// Sub-folders, made when missing.
    pub path: &'a [String],
    pub name: &'a str,
    pub mime: &'a str,
}

/// A file made outside the app: its URI, and a descriptor the caller now owns and must close
/// before [`NativeAudio::finish_output`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputFile {
    pub uri: String,
    pub fd: i32,
}

impl<R: Runtime> NativeAudio<R> {
    /// Opens a website in the built-in browser (M46.3).
    #[cfg(target_os = "android")]
    pub fn website_open(&self, request: &serde_json::Value) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("websiteOpen", request.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Lets the app hear multicast (mDNS) while held: Connect listens for devices (M46.15).
    #[cfg(target_os = "android")]
    pub fn multicast(&self, hold: bool) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "setMulticast",
                serde_json::json!({ "hold": hold }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Clears the built-in browser's cookies, storage and cache.
    #[cfg(target_os = "android")]
    pub fn website_clear(&self) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("websiteClear", ())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Shows the system folder picker and keeps the grant. Blocks until the user answers.
    #[cfg(target_os = "android")]
    pub fn pick_folder(&self) -> Result<PickedFolder, String> {
        self.handle
            .run_mobile_plugin("pickFolder", ())
            .map_err(|error| error.to_string())
    }

    #[cfg(target_os = "android")]
    pub fn folder_state(&self, uri: &str) -> Result<FolderState, String> {
        self.handle
            .run_mobile_plugin("folderState", serde_json::json!({ "uri": uri }))
            .map_err(|error| error.to_string())
    }

    /// Gives up the grant of a folder no longer used.
    #[cfg(target_os = "android")]
    pub fn release_folder(&self, uri: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "releaseFolder",
                serde_json::json!({ "uri": uri }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Makes (or replaces) a file outside the app and opens it for writing. Blocking.
    #[cfg(target_os = "android")]
    pub fn create_output(&self, request: &OutputRequest<'_>) -> Result<OutputFile, String> {
        self.handle
            .run_mobile_plugin("createOutput", request)
            .map_err(|error| error.to_string())
    }

    /// Publishes a written file (`ok`), or deletes it. Blocking.
    #[cfg(target_os = "android")]
    pub fn finish_output(&self, uri: &str, ok: bool) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "finishOutput",
                serde_json::json!({ "uri": uri, "ok": ok }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens the share sheet for a file in the app's cache folder.
    #[cfg(target_os = "android")]
    pub fn share_file(&self, path: &str, mime: &str, title: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "shareFile",
                serde_json::json!({ "path": path, "mime": mime, "title": title }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Puts an image from the app's cache folder on the clipboard.
    #[cfg(target_os = "android")]
    pub fn copy_image(&self, path: &str, label: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "copyImage",
                serde_json::json!({ "path": path, "title": label }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Whether the app may reach the local network. Blocks briefly on the main thread.
    #[cfg(target_os = "android")]
    pub fn local_network_state(&self) -> Result<LocalNetworkState, String> {
        self.handle
            .run_mobile_plugin("localNetworkState", ())
            .map_err(|error| error.to_string())
    }

    /// Shows the system dialog for local network access if needed. Blocks until the user answers,
    /// so call it from a blocking-friendly thread.
    #[cfg(target_os = "android")]
    pub fn request_local_network(&self) -> Result<LocalNetworkState, String> {
        self.handle
            .run_mobile_plugin("requestLocalNetwork", ())
            .map_err(|error| error.to_string())
    }

    /// Opens the app's page in the system settings.
    #[cfg(target_os = "android")]
    pub fn open_app_settings(&self) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("openAppSettings", ())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

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

    /// Opens the native video player with a request (url, title, headers, subtitles, startMs, id).
    #[cfg(target_os = "android")]
    pub fn video_open(&self, request: &serde_json::Value) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("videoOpen", request.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "android"))]
    pub fn video_open(&self, _request: &serde_json::Value) -> Result<(), String> {
        Err("the video player is only available on Android for now".into())
    }

    /// Sends an update to the open video player (M44): `{"type": "load" | "episodes" | "sources" |
    /// "loading" | "error", ...}`. Ignored when no player is open.
    #[cfg(target_os = "android")]
    pub fn video_update(&self, update: &serde_json::Value) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("videoUpdate", update.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "android"))]
    pub fn video_update(&self, _update: &serde_json::Value) -> Result<(), String> {
        Err("the video player is only available on Android for now".into())
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
