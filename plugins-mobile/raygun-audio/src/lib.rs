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
static NOTIFICATION_ACTION_HANDLER: Later = Later::new();
static OPEN_HANDLER: Later = Later::new();
static DEVICE_STATE_HANDLER: Later = Later::new();

/// A receiver for messages that can come before it's installed (a notification tapped to start
/// the app): those wait for it.
struct Later {
    handler: OnceLock<ReportHandler>,
    waiting: std::sync::Mutex<Vec<String>>,
}

/// Messages kept for a receiver not installed yet.
const MAX_WAITING: usize = 16;

impl Later {
    const fn new() -> Self {
        Self {
            handler: OnceLock::new(),
            waiting: std::sync::Mutex::new(Vec::new()),
        }
    }

    fn set(&self, handler: ReportHandler) {
        if self.handler.set(handler).is_err() {
            return;
        }
        let waiting = std::mem::take(&mut *self.waiting.lock().unwrap_or_else(|p| p.into_inner()));
        if let Some(handler) = self.handler.get() {
            for message in waiting {
                handler(message);
            }
        }
    }

    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn deliver(&self, message: String) {
        {
            let mut waiting = self.waiting.lock().unwrap_or_else(|p| p.into_inner());
            if self.handler.get().is_none() {
                if waiting.len() < MAX_WAITING {
                    waiting.push(message);
                }
                return;
            }
        }
        if let Some(handler) = self.handler.get() {
            handler(message);
        }
    }
}

/// Installs the receiver for download notifications' buttons (`{"action", "keys"}` JSON).
/// Called on the Android main thread. Only the first call wins.
pub fn set_notification_action_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    NOTIFICATION_ACTION_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_notification_action(json: String) {
    NOTIFICATION_ACTION_HANDLER.deliver(json);
}

/// Installs the receiver for tapped notifications: the screen to open (`/downloads?tab=music`,
/// `player`). Called on the Android main thread. Only the first call wins.
pub fn set_open_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    OPEN_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_open(route: String) {
    OPEN_HANDLER.deliver(route);
}

/// Installs the receiver for the device's state (`{"foreground", "powerSave", "animatorScale",
/// "transitionScale", "windowScale", "animatorsEnabled"}` JSON): sent when Raygun comes and goes
/// from the screen, battery saver changes, or the system's animation settings change. Called on
/// the Android main thread. Only the first call wins; reports from before it wait for it.
pub fn set_device_state_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    DEVICE_STATE_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_device_state(json: String) {
    DEVICE_STATE_HANDLER.deliver(json);
}

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
    /// Keeps a file already there: a taken name becomes `name (1)`… instead of replacing it.
    pub unique: bool,
}

/// A file made outside the app: its URI, and a descriptor the caller now owns and must close
/// before [`NativeAudio::finish_output`].
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
pub struct OutputFile {
    pub uri: String,
    pub fd: i32,
    /// The name it was made under.
    #[serde(default)]
    pub name: Option<String>,
}

/// A file under a picked folder (M46.27).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct TreeFile {
    /// Its document address, to open it.
    pub uri: String,
    /// Its path inside the folder, with `/`.
    pub path: String,
    pub size: i64,
    /// Unix ms.
    pub modified: i64,
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

    /// Opens a site for the person to pass its browser check by hand (`BrowserCheckActivity`;
    /// request `{url, userAgent}`). Blocks until the browser closes; answers the site's cookies
    /// as a `Cookie` header's pairs.
    #[cfg(target_os = "android")]
    pub fn browser_check(&self, request: &serde_json::Value) -> Result<String, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("browserCheck", request.clone())
            .map(|answer| answer["cookies"].as_str().unwrap_or_default().to_owned())
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

    /// Keeps the app running in the foreground while a TV plays from it (Play on TV), with a quiet
    /// notification saying so; `on: false` stops it.
    #[cfg(target_os = "android")]
    pub fn cast_keep_alive(&self, on: bool, title: &str, text: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "setCastKeepAlive",
                serde_json::json!({ "on": on, "title": title, "text": text }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Hides the status and navigation bars (a swipe from an edge shows them for a moment), or
    /// shows them again: the readers' full screen.
    #[cfg(target_os = "android")]
    pub fn system_bars(&self, hidden: bool) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "setSystemBars",
                serde_json::json!({ "hidden": hidden }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The outputs sound can go to (M46.43) and the one picked, if any: `{outputs, chosen}`.
    #[cfg(target_os = "android")]
    pub fn audio_outputs(&self) -> Result<serde_json::Value, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("audioOutputs", ())
            .map_err(|error| error.to_string())
    }

    /// Plays through one output, or Android's choice for `None` (M46.43).
    #[cfg(target_os = "android")]
    pub fn set_audio_output(&self, id: Option<String>) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "setAudioOutput",
                serde_json::json!({ "id": id }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The device's state as `set_device_state_handler` reports it, asked once at start.
    #[cfg(target_os = "android")]
    pub fn device_state(&self) -> Result<serde_json::Value, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("deviceState", ())
            .map_err(|error| error.to_string())
    }

    /// The active network's kind: `cellular`, `wifi`, `ethernet`, `other` or `none` (M46.18:
    /// the Audio quality for mobile data).
    #[cfg(target_os = "android")]
    pub fn network_kind(&self) -> Result<String, String> {
        #[derive(serde::Deserialize)]
        struct Kind {
            kind: String,
        }
        self.handle
            .run_mobile_plugin::<Kind>("networkKind", ())
            .map(|answer| answer.kind)
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

    /// Asks to read the device's songs and videos (M46.27). Blocks until the user answers.
    #[cfg(target_os = "android")]
    pub fn request_media_access(&self) -> Result<bool, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("requestMediaAccess", ())
            .map(|value| value["granted"].as_bool().unwrap_or(false))
            .map_err(|error| error.to_string())
    }

    /// Every file under a picked folder (M46.27). Blocking.
    #[cfg(target_os = "android")]
    pub fn list_tree(&self, uri: &str) -> Result<Vec<TreeFile>, String> {
        #[derive(serde::Deserialize)]
        struct Listing {
            files: Vec<TreeFile>,
        }
        self.handle
            .run_mobile_plugin::<Listing>("listTree", serde_json::json!({ "uri": uri }))
            .map(|listing| listing.files)
            .map_err(|error| error.to_string())
    }

    /// Opens a picked file for reading; the caller owns (and closes) the descriptor. Blocking.
    #[cfg(target_os = "android")]
    pub fn open_input(&self, uri: &str) -> Result<i32, String> {
        #[derive(serde::Deserialize)]
        struct Opened {
            fd: i32,
        }
        self.handle
            .run_mobile_plugin::<Opened>("openInput", serde_json::json!({ "uri": uri }))
            .map(|opened| opened.fd)
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

    /// Opens Android's installer for a downloaded update (M46.47); `false` when Raygun first
    /// needs "Install unknown apps" (its settings page was opened instead).
    #[cfg(target_os = "android")]
    pub fn install_apk(&self, path: &str) -> Result<bool, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "installApk",
                serde_json::json!({ "path": path }),
            )
            .map(|value| value["started"].as_bool().unwrap_or(false))
            .map_err(|error| error.to_string())
    }

    /// Whether Raygun may install its own updates (M46.47).
    #[cfg(target_os = "android")]
    pub fn can_install_packages(&self) -> Result<bool, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("canInstallPackages", ())
            .map(|value| value["allowed"].as_bool().unwrap_or(false))
            .map_err(|error| error.to_string())
    }

    /// Starts capturing a stream's sound in the background (M46.49): a muted player of its own
    /// writes the decoded PCM into pieces in `dir`. The request: `{session, url, mime, fromMs,
    /// speed, dir}`. One session at a time; a new one stops the last.
    #[cfg(target_os = "android")]
    pub fn capture_start(&self, request: &serde_json::Value) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("captureStart", request.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// A capture's state: `{state, positionMs, durationMs, error, pieces: [{file, startMs, rate,
    /// channels, frames}]}`; `state` is `loading`, `playing`, `buffering`, `ended` or `failed`.
    #[cfg(target_os = "android")]
    pub fn capture_status(&self, session: &str) -> Result<serde_json::Value, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "captureStatus",
                serde_json::json!({ "session": session }),
            )
            .map_err(|error| error.to_string())
    }

    /// Stops a capture and releases its player.
    #[cfg(target_os = "android")]
    pub fn capture_stop(&self, session: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "captureStop",
                serde_json::json!({ "session": session }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens a saved file (a MediaStore, document or `file:` URI) in the app Android picks.
    #[cfg(target_os = "android")]
    pub fn open_file(&self, uri: &str, mime: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "openFile",
                serde_json::json!({ "uri": uri, "mime": mime }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Shows the folder of a saved file: the system's downloads, or the picked folder.
    #[cfg(target_os = "android")]
    pub fn show_file_folder(&self, uri: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "showFileFolder",
                serde_json::json!({ "uri": uri }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Deletes a saved file. Blocking.
    #[cfg(target_os = "android")]
    pub fn delete_file(&self, uri: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("deleteFile", serde_json::json!({ "uri": uri }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens an https address in the phone's browser (M46.47).
    #[cfg(target_os = "android")]
    pub fn open_url(&self, url: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("openUrl", serde_json::json!({ "url": url }))
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

    /// What Rust's player is doing (`{"state": {...} | null}`), for the media session while the
    /// native engine isn't the one playing: a song in the webview, or the next one being found.
    #[cfg(target_os = "android")]
    pub fn media_state(&self, state: &serde_json::Value) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("mediaState", state.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The download notifications to show now (`{"plan", "finished"}`).
    #[cfg(target_os = "android")]
    pub fn downloads_notify(&self, notices: &serde_json::Value) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("downloadsNotify", notices.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Every permission Raygun uses and its state: `{"permissions": [{"kind", "state", "count"}]}`.
    #[cfg(target_os = "android")]
    pub fn permission_states(&self) -> Result<serde_json::Value, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>("permissionStates", ())
            .map_err(|error| error.to_string())
    }

    /// Asks for a permission (Android's question, or its settings page) and answers with every
    /// state afterwards. Blocks until the user is done, so call it from a blocking-friendly thread.
    #[cfg(target_os = "android")]
    pub fn request_permission(&self, kind: &str) -> Result<serde_json::Value, String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "requestPermission",
                serde_json::json!({ "kind": kind }),
            )
            .map_err(|error| error.to_string())
    }

    /// Opens a permission's page in the system settings.
    #[cfg(target_os = "android")]
    pub fn open_permission_settings(&self, kind: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "openPermissionSettings",
                serde_json::json!({ "kind": kind }),
            )
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// "Not now" to Raygun's own question: Android's isn't shown right after either.
    #[cfg(target_os = "android")]
    pub fn decline_permission(&self, kind: &str) -> Result<(), String> {
        self.handle
            .run_mobile_plugin::<serde_json::Value>(
                "declinePermission",
                serde_json::json!({ "kind": kind }),
            )
            .map(|_| ())
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

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use super::{Later, MAX_WAITING};

    #[test]
    fn messages_before_the_receiver_wait_for_it() {
        let later = Later::new();
        later.deliver("/downloads".into());
        let got = Arc::new(Mutex::new(Vec::new()));
        let sink = got.clone();
        later.set(Box::new(move |message| sink.lock().unwrap().push(message)));
        later.deliver("player".into());
        assert_eq!(*got.lock().unwrap(), ["/downloads", "player"]);
    }

    #[test]
    fn only_a_few_messages_wait() {
        let later = Later::new();
        for index in 0..40 {
            later.deliver(index.to_string());
        }
        let got = Arc::new(Mutex::new(0));
        let sink = got.clone();
        later.set(Box::new(move |_| *sink.lock().unwrap() += 1));
        assert_eq!(*got.lock().unwrap(), MAX_WAITING);
    }
}
