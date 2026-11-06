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
static OPEN_WITH_HANDLER: Later = Later::new();
static DEVICE_STATE_HANDLER: Later = Later::new();
static WINDOW_HANDLER: Later = Later::new();
static WINDOW_CLOSING_HANDLER: OnceLock<Box<dyn Fn() + Send + Sync>> = OnceLock::new();

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

/// Installs the receiver for what other apps open with Raygun or share to it (`{"items": [{"uri",
/// "mime", "name", "size"}], "text", "subject"}` JSON, see `OpenWith.kt`). Called on a thread of
/// the plugin's own. Only the first call wins; what came before it (the intent that started the
/// app) waits for it.
pub fn set_open_with_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    OPEN_WITH_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_open_with(json: String) {
    OPEN_WITH_HANDLER.deliver(json);
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

/// Installs the receiver for new activities that need Raygun's main window: Android made one in a
/// process that carried on after the last one went (music played with Raygun swiped away). Called
/// on the Android main thread. Only the first call wins; one that came before it waits for it.
pub fn set_window_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    WINDOW_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_window_wanted() {
    WINDOW_HANDLER.deliver(String::new());
}

/// Installs what runs as Raygun's activity is destroyed for good, before wry lets go of it: Tauri
/// must stop sending its webview scripts first (one arriving after would crash Raygun as it
/// carries on). Called on the Android main thread, which waits for it to return. Only the first
/// call wins.
pub fn set_window_closing_handler(handler: impl Fn() + Send + Sync + 'static) {
    let _ = WINDOW_CLOSING_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_window_closing() {
    if let Some(handler) = WINDOW_CLOSING_HANDLER.get() {
        handler();
    }
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

/// The person's answer to recording what Raygun plays.
#[derive(Debug, Clone, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RecordingConsent {
    pub granted: bool,
    /// Why not: `unsupported` (before Android 10), `microphone` (RECORD_AUDIO refused),
    /// `declined` (Android's capture question refused) or `failed`.
    #[serde(default)]
    pub reason: Option<String>,
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

/// A file found where Raygun saves (M46.51): `Download/Raygun/` or a picked folder.
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct SavedFile {
    /// Its address, to open or delete it.
    pub uri: String,
    pub name: String,
    pub size: i64,
    /// Unix ms.
    pub modified: i64,
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

/// Commands that need Raygun's window (Android's pickers and questions, other apps' screens, the
/// window itself): they go through Tauri, which reaches the plugin through the activity. Every
/// other command goes on the plugin's own line ([`android::call`]), which works without one: while
/// a song plays or downloads run with Raygun swiped away, the process kept by a foreground
/// service, Tauri's calls fail.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
const WINDOW_COMMANDS: &[&str] = &[
    "applyTheme",
    "browserCheck",
    "copyImage",
    "installApk",
    "openAppSettings",
    "openFile",
    "openPermissionSettings",
    "openUrl",
    "pickDocuments",
    "pickFile",
    "pickFolder",
    "recordingConsent",
    "requestLibraryAccess",
    "requestLocalNetwork",
    "requestMediaAccess",
    "requestPermission",
    "restartApp",
    "setSystemBars",
    "shareFile",
    "showFileFolder",
    "videoOpen",
    "websiteClear",
    "websiteOpen",
];

/// Whether `command` needs Raygun's window.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn needs_window(command: &str) -> bool {
    WINDOW_COMMANDS.contains(&command)
}

/// An engine command as the Kotlin plugin takes it: the command named after its `type` tag, the
/// rest of the object as its arguments.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn engine_call(command: &EngineCommand) -> Result<(String, serde_json::Value), String> {
    let mut payload = serde_json::to_value(command).map_err(|error| error.to_string())?;
    match payload
        .as_object_mut()
        .and_then(|fields| fields.remove("type"))
    {
        Some(serde_json::Value::String(name)) => Ok((name, payload)),
        _ => Err("engine command has no type".into()),
    }
}

/// A command's answer on the plugin's own line: `ok:<JSON>`, or `error:<message>`.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn parse_reply(reply: &str) -> Result<serde_json::Value, String> {
    match reply.strip_prefix("ok:") {
        Some(json) => serde_json::from_str(json)
            .map_err(|error| format!("The player's answer was unreadable: {error}")),
        None => Err(reply.strip_prefix("error:").unwrap_or(reply).to_owned()),
    }
}

/// A file chosen in the system's file picker (M46.51: Local Files' Add files).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct PickedDocument {
    /// Its document address, to open it with [`NativeAudio::open_input`].
    pub uri: String,
    pub name: String,
    #[serde(default)]
    pub size: i64,
    #[serde(default)]
    pub mime: Option<String>,
}

/// What Raygun may read of shared storage (M46.51).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StorageAccess {
    /// Shared storage's top folder (`/storage/emulated/0`).
    pub root: String,
    /// Songs, videos or pictures: any of the media permissions.
    pub media: bool,
    /// Every file: access to all files (Android 11+), or the storage permission before it.
    pub all_files: bool,
    /// Android has a page for access to all files (11 and later).
    pub can_ask_all_files: bool,
}

/// A folder of shared storage as MediaStore knows it: its folders that hold files of the kinds
/// asked for, with how many, and how many are right in it (M46.51).
#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MediaListing {
    pub folders: Vec<MediaFolder>,
    pub items: u32,
}

#[derive(Debug, Clone, Default, serde::Serialize, serde::Deserialize)]
pub struct MediaFolder {
    pub name: String,
    pub items: u32,
}

impl<R: Runtime> NativeAudio<R> {
    /// Runs a command of the Kotlin plugin: on its own line unless it needs the window (see
    /// [`WINDOW_COMMANDS`]). Blocks until it answers, so never call it on Android's main thread.
    #[cfg(target_os = "android")]
    fn run<T: serde::de::DeserializeOwned>(
        &self,
        command: &str,
        payload: impl serde::Serialize,
    ) -> Result<T, String> {
        let payload = serde_json::to_value(payload).map_err(|error| error.to_string())?;
        let answer = if !needs_window(command) && android::attached() {
            android::call(command, &payload)?
        } else {
            // Through the activity. Tauri panics when there is none (Raygun swiped away): that's
            // an error here, never a crash.
            let handle = &self.handle;
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
                handle.run_mobile_plugin::<serde_json::Value>(command, payload)
            }))
            .map_err(|_| "Raygun's window is closed".to_owned())?
            .map_err(|error| error.to_string())?
        };
        serde_json::from_value(answer).map_err(|error| error.to_string())
    }

    /// Elsewhere there's no native plugin (and [`NativeAudio`] isn't managed).
    #[cfg(not(target_os = "android"))]
    fn run<T: serde::de::DeserializeOwned>(
        &self,
        _command: &str,
        _payload: impl serde::Serialize,
    ) -> Result<T, String> {
        Err("Raygun's native player is only on Android".into())
    }

    /// Opens a website in the built-in browser (M46.3).
    #[cfg(target_os = "android")]
    pub fn website_open(&self, request: &serde_json::Value) -> Result<(), String> {
        self.run::<serde_json::Value>("websiteOpen", request.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Tells the built-in browser how the downloads it started stand (`{"downloads": [...]}`),
    /// in a package-only broadcast that its process hears while it's open.
    #[cfg(target_os = "android")]
    pub fn website_downloads(&self, update: &serde_json::Value) -> Result<(), String> {
        self.run::<serde_json::Value>("websiteDownloads", update.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens the request that met a browser check for the person to pass it by hand
    /// (`BrowserCheckActivity`; request `{url, method, headers, body, userAgent}`). Blocks until
    /// the browser closes; answers `{cookies, userAgent, seen}`: the address's cookies as a
    /// `Cookie` header's pairs, the browser's User-Agent, and what it saw of its pages.
    #[cfg(target_os = "android")]
    pub fn browser_check(&self, request: &serde_json::Value) -> Result<serde_json::Value, String> {
        self.run::<serde_json::Value>("browserCheck", request.clone())
            .map_err(|error| error.to_string())
    }

    /// Lets the app hear multicast (mDNS) while held: Connect listens for devices (M46.15).
    #[cfg(target_os = "android")]
    pub fn multicast(&self, hold: bool) -> Result<(), String> {
        self.run::<serde_json::Value>("setMulticast", serde_json::json!({ "hold": hold }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Keeps the app running in the foreground while a TV plays from it (Play on TV), with a quiet
    /// notification saying so; `on: false` stops it.
    #[cfg(target_os = "android")]
    pub fn cast_keep_alive(&self, on: bool, title: &str, text: &str) -> Result<(), String> {
        self.run::<serde_json::Value>(
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
        self.run::<serde_json::Value>("setSystemBars", serde_json::json!({ "hidden": hidden }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The outputs sound can go to (M46.43) and the one picked, if any: `{outputs, chosen}`.
    #[cfg(target_os = "android")]
    pub fn audio_outputs(&self) -> Result<serde_json::Value, String> {
        self.run::<serde_json::Value>("audioOutputs", ())
            .map_err(|error| error.to_string())
    }

    /// Plays through one output, or Android's choice for `None` (M46.43).
    #[cfg(target_os = "android")]
    pub fn set_audio_output(&self, id: Option<String>) -> Result<(), String> {
        self.run::<serde_json::Value>("setAudioOutput", serde_json::json!({ "id": id }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The device's state as `set_device_state_handler` reports it, asked once at start.
    #[cfg(target_os = "android")]
    pub fn device_state(&self) -> Result<serde_json::Value, String> {
        self.run::<serde_json::Value>("deviceState", ())
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
        self.run::<Kind>("networkKind", ())
            .map(|answer| answer.kind)
            .map_err(|error| error.to_string())
    }

    /// Clears the built-in browser's cookies, storage and cache.
    #[cfg(target_os = "android")]
    pub fn website_clear(&self) -> Result<(), String> {
        self.run::<serde_json::Value>("websiteClear", ())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Shows the system folder picker and keeps the grant. Blocks until the user answers.
    #[cfg(target_os = "android")]
    pub fn pick_folder(&self) -> Result<PickedFolder, String> {
        self.run("pickFolder", ())
            .map_err(|error| error.to_string())
    }

    /// Shows the system file picker (M46.52); `uri` and `name` are missing when the user
    /// cancelled. The file is read with [`Self::open_input`]. Blocks until the user answers.
    #[cfg(target_os = "android")]
    pub fn pick_file(&self) -> Result<PickedFolder, String> {
        self.run("pickFile", ()).map_err(|error| error.to_string())
    }

    #[cfg(target_os = "android")]
    pub fn folder_state(&self, uri: &str) -> Result<FolderState, String> {
        self.run("folderState", serde_json::json!({ "uri": uri }))
            .map_err(|error| error.to_string())
    }

    /// Asks to read the device's songs and videos (M46.27). Blocks until the user answers.
    #[cfg(target_os = "android")]
    pub fn request_media_access(&self) -> Result<bool, String> {
        self.run::<serde_json::Value>("requestMediaAccess", ())
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
        self.run::<Listing>("listTree", serde_json::json!({ "uri": uri }))
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
        self.run::<Opened>("openInput", serde_json::json!({ "uri": uri }))
            .map(|opened| opened.fd)
            .map_err(|error| error.to_string())
    }

    /// What Raygun may read of shared storage (M46.51).
    #[cfg(target_os = "android")]
    pub fn storage_access(&self) -> Result<StorageAccess, String> {
        self.run("storageAccess", ())
    }

    /// A folder of shared storage (an absolute path) as MediaStore lists it: the folders in it
    /// holding files with these extensions, and how many (M46.51). Blocking.
    #[cfg(target_os = "android")]
    pub fn browse_media(&self, path: &str, extensions: &[&str]) -> Result<MediaListing, String> {
        self.run(
            "browseMedia",
            serde_json::json!({ "path": path, "extensions": extensions }),
        )
    }

    /// Shows the system's file picker for several files of these types (M46.51); none when the
    /// user cancelled. Blocks until the user answers.
    #[cfg(target_os = "android")]
    pub fn pick_documents(&self, mimes: &[&str]) -> Result<Vec<PickedDocument>, String> {
        #[derive(serde::Deserialize)]
        struct Picked {
            #[serde(default)]
            files: Vec<PickedDocument>,
        }
        self.run::<Picked>("pickDocuments", serde_json::json!({ "mimes": mimes }))
            .map(|picked| picked.files)
    }

    /// Gives up the grant of a folder no longer used.
    #[cfg(target_os = "android")]
    pub fn release_folder(&self, uri: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("releaseFolder", serde_json::json!({ "uri": uri }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Makes (or replaces) a file outside the app and opens it for writing. Blocking.
    #[cfg(target_os = "android")]
    pub fn create_output(&self, request: &OutputRequest<'_>) -> Result<OutputFile, String> {
        self.run("createOutput", request)
            .map_err(|error| error.to_string())
    }

    /// Publishes a written file (`ok`), or deletes it. Blocking.
    #[cfg(target_os = "android")]
    pub fn finish_output(&self, uri: &str, ok: bool) -> Result<(), String> {
        self.run::<serde_json::Value>("finishOutput", serde_json::json!({ "uri": uri, "ok": ok }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens Android's installer for a downloaded update (M46.47); `false` when Raygun first
    /// needs "Install unknown apps" (its settings page was opened instead).
    #[cfg(target_os = "android")]
    pub fn install_apk(&self, path: &str) -> Result<bool, String> {
        self.run::<serde_json::Value>("installApk", serde_json::json!({ "path": path }))
            .map(|value| value["started"].as_bool().unwrap_or(false))
            .map_err(|error| error.to_string())
    }

    /// Whether Raygun may install its own updates (M46.47).
    #[cfg(target_os = "android")]
    pub fn can_install_packages(&self) -> Result<bool, String> {
        self.run::<serde_json::Value>("canInstallPackages", ())
            .map(|value| value["allowed"].as_bool().unwrap_or(false))
            .map_err(|error| error.to_string())
    }

    /// Starts capturing a stream's sound in the background (M46.49): a muted player of its own
    /// writes the decoded PCM into pieces in `dir`. The request: `{session, url, mime, fromMs,
    /// speed, dir}`. One session at a time; a new one stops the last.
    pub fn capture_start(&self, request: &serde_json::Value) -> Result<(), String> {
        self.run::<serde_json::Value>("captureStart", request.clone())
            .map(|_| ())
    }

    /// A capture's state: `{state, positionMs, durationMs, error, pieces: [{file, startMs, rate,
    /// channels, frames}]}`; `state` is `loading`, `playing`, `buffering`, `ended` or `failed`.
    pub fn capture_status(&self, session: &str) -> Result<serde_json::Value, String> {
        self.run("captureStatus", serde_json::json!({ "session": session }))
    }

    /// Stops a capture and releases its player.
    pub fn capture_stop(&self, session: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("captureStop", serde_json::json!({ "session": session }))
            .map(|_| ())
    }

    /// Asks the person to let Raygun record what it plays (RECORD_AUDIO, then Android's capture
    /// question); the consent is held until [`Self::recording_end`]. Needs Raygun's window, and
    /// blocks until they answer.
    pub fn recording_consent(&self) -> Result<RecordingConsent, String> {
        self.run("recordingConsent", ())
    }

    /// Whether a consent is held: songs can be recorded now.
    pub fn recording_ready(&self) -> Result<bool, String> {
        #[derive(serde::Deserialize)]
        struct Ready {
            ready: bool,
        }
        self.run::<Ready>("recordingReady", ())
            .map(|answer| answer.ready)
    }

    /// Starts recording what Raygun plays into pieces in `dir` (`{session, dir}`); nothing is
    /// kept until [`Self::recording_mark`] says where the song plays from.
    pub fn recording_start(&self, session: &str, dir: &str) -> Result<(), String> {
        self.run::<serde_json::Value>(
            "recordingStart",
            serde_json::json!({ "session": session, "dir": dir }),
        )
        .map(|_| ())
    }

    /// The song plays from `position_ms` now (a run starts there), or doesn't (`None`: held).
    pub fn recording_mark(&self, session: &str, position_ms: Option<f64>) -> Result<(), String> {
        self.run::<serde_json::Value>(
            "recordingMark",
            serde_json::json!({ "session": session, "positionMs": position_ms }),
        )
        .map(|_| ())
    }

    /// A recording's state, as [`Self::capture_status`] gives a capture's: `{state, error,
    /// pieces}`; `state` is `playing` or `failed`.
    pub fn recording_status(&self, session: &str) -> Result<serde_json::Value, String> {
        self.run("recordingStatus", serde_json::json!({ "session": session }))
    }

    /// Stops a recording.
    pub fn recording_stop(&self, session: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("recordingStop", serde_json::json!({ "session": session }))
            .map(|_| ())
    }

    /// No song waits to be recorded: the consent is given back (and Android's notice of it).
    pub fn recording_end(&self) -> Result<(), String> {
        self.run::<serde_json::Value>("recordingEnd", ())
            .map(|_| ())
    }

    /// Opens a saved file (a MediaStore, document or `file:` URI) in the app Android picks.
    #[cfg(target_os = "android")]
    pub fn open_file(&self, uri: &str, mime: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("openFile", serde_json::json!({ "uri": uri, "mime": mime }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Shows the folder of a saved file: the system's downloads, or the picked folder.
    #[cfg(target_os = "android")]
    pub fn show_file_folder(&self, uri: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("showFileFolder", serde_json::json!({ "uri": uri }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The files in `Download/Raygun/` (`place` `downloads`) or a picked folder (`tree`) whose
    /// names contain `contains`, sub-folders left out (M46.51). Blocking.
    #[cfg(target_os = "android")]
    pub fn list_saved(
        &self,
        place: &str,
        tree: Option<&str>,
        contains: &str,
    ) -> Result<Vec<SavedFile>, String> {
        #[derive(serde::Deserialize)]
        struct Listing {
            files: Vec<SavedFile>,
        }
        self.run::<Listing>(
            "listSaved",
            serde_json::json!({ "place": place, "tree": tree, "contains": contains }),
        )
        .map(|listing| listing.files)
    }

    /// Starts Raygun again from its first screen, ending its other processes (M46.51). The
    /// WebView's storage and cookies are cleared first.
    #[cfg(target_os = "android")]
    pub fn restart_app(&self) -> Result<(), String> {
        self.run::<serde_json::Value>("restartApp", ()).map(|_| ())
    }

    /// Deletes a saved file. Blocking.
    #[cfg(target_os = "android")]
    pub fn delete_file(&self, uri: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("deleteFile", serde_json::json!({ "uri": uri }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens an https address in the phone's browser (M46.47).
    #[cfg(target_os = "android")]
    pub fn open_url(&self, url: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("openUrl", serde_json::json!({ "url": url }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens the share sheet for a file in the app's cache folder.
    #[cfg(target_os = "android")]
    pub fn share_file(&self, path: &str, mime: &str, title: &str) -> Result<(), String> {
        self.run::<serde_json::Value>(
            "shareFile",
            serde_json::json!({ "path": path, "mime": mime, "title": title }),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    /// Puts an image from the app's cache folder on the clipboard.
    #[cfg(target_os = "android")]
    pub fn copy_image(&self, path: &str, label: &str) -> Result<(), String> {
        self.run::<serde_json::Value>(
            "copyImage",
            serde_json::json!({ "path": path, "title": label }),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    /// Whether the app may reach the local network. Blocks briefly on the main thread.
    #[cfg(target_os = "android")]
    pub fn local_network_state(&self) -> Result<LocalNetworkState, String> {
        self.run("localNetworkState", ())
            .map_err(|error| error.to_string())
    }

    /// Shows the system dialog for local network access if needed. Blocks until the user answers,
    /// so call it from a blocking-friendly thread.
    #[cfg(target_os = "android")]
    pub fn request_local_network(&self) -> Result<LocalNetworkState, String> {
        self.run("requestLocalNetwork", ())
            .map_err(|error| error.to_string())
    }

    /// What Rust's player is doing (`{"state": {...} | null}`), for the media session while the
    /// native engine isn't the one playing: a song in the webview, or the next one being found.
    #[cfg(target_os = "android")]
    pub fn media_state(&self, state: &serde_json::Value) -> Result<(), String> {
        self.run::<serde_json::Value>("mediaState", state.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// The download notifications to show now (`{"plan", "finished"}`).
    #[cfg(target_os = "android")]
    pub fn downloads_notify(&self, notices: &serde_json::Value) -> Result<(), String> {
        self.run::<serde_json::Value>("downloadsNotify", notices.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// A notification that opens a screen when tapped (`{"id", "channel", "channelName",
    /// "channelDescription", "title", "text", "route"}`): the tap comes back through the handler
    /// installed with [`set_open_handler`]. The same `id` in a channel replaces the one shown.
    #[cfg(target_os = "android")]
    pub fn notice(&self, notice: &serde_json::Value) -> Result<(), String> {
        self.run::<serde_json::Value>("notice", notice.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Every permission Raygun uses and its state: `{"permissions": [{"kind", "state", "count"}]}`.
    #[cfg(target_os = "android")]
    pub fn permission_states(&self) -> Result<serde_json::Value, String> {
        self.run::<serde_json::Value>("permissionStates", ())
            .map_err(|error| error.to_string())
    }

    /// Asks for a permission (Android's question, or its settings page) and answers with every
    /// state afterwards. Blocks until the user is done, so call it from a blocking-friendly thread.
    #[cfg(target_os = "android")]
    pub fn request_permission(&self, kind: &str) -> Result<serde_json::Value, String> {
        self.run::<serde_json::Value>("requestPermission", serde_json::json!({ "kind": kind }))
            .map_err(|error| error.to_string())
    }

    /// Opens a permission's page in the system settings.
    #[cfg(target_os = "android")]
    pub fn open_permission_settings(&self, kind: &str) -> Result<(), String> {
        self.run::<serde_json::Value>(
            "openPermissionSettings",
            serde_json::json!({ "kind": kind }),
        )
        .map(|_| ())
        .map_err(|error| error.to_string())
    }

    /// "Not now" to Raygun's own question: Android's isn't shown right after either.
    #[cfg(target_os = "android")]
    pub fn decline_permission(&self, kind: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("declinePermission", serde_json::json!({ "kind": kind }))
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Opens the app's page in the system settings.
    #[cfg(target_os = "android")]
    pub fn open_app_settings(&self) -> Result<(), String> {
        self.run::<serde_json::Value>("openAppSettings", ())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    /// Shows the system permission dialog if needed. Blocks until the user answers, so call it
    /// from a blocking-friendly thread.
    #[cfg(target_os = "android")]
    pub fn request_library_access(&self) -> Result<LibraryAccess, String> {
        self.run("requestLibraryAccess", ())
            .map_err(|error| error.to_string())
    }

    /// Matches the system bars to Raygun's theme (`system`, `light`, `dark`, `amoled`).
    #[cfg(target_os = "android")]
    pub fn apply_theme(&self, theme: &str) -> Result<(), String> {
        self.run::<serde_json::Value>("applyTheme", serde_json::json!({ "theme": theme }))
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
        self.run::<serde_json::Value>("ttsAvailable", ())
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
        self.run::<serde_json::Value>(
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
        self.run::<serde_json::Value>("ttsStop", ())
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
        self.run::<serde_json::Value>("videoOpen", request.clone())
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
        self.run::<serde_json::Value>("videoUpdate", update.clone())
            .map(|_| ())
            .map_err(|error| error.to_string())
    }

    #[cfg(not(target_os = "android"))]
    pub fn video_update(&self, _update: &serde_json::Value) -> Result<(), String> {
        Err("the video player is only available on Android for now".into())
    }

    /// Forwards one engine command to the native player. The Kotlin commands are named after the
    /// `type` tag; the rest of the object is the payload.
    #[cfg(target_os = "android")]
    pub fn send(&self, command: &EngineCommand) -> Result<(), String> {
        let (name, payload) = engine_call(command)?;
        self.run::<serde_json::Value>(&name, payload).map(|_| ())
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

    use raygun_model::EngineCommand;

    use super::{Later, MAX_WAITING, engine_call, needs_window, parse_reply};

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

    /// What runs while Raygun's window is gone (the app swiped away, the process kept by the
    /// playback or download service) goes on the plugin's own line: Tauri's calls go through the
    /// activity and failed then, so the next song never loaded and captures never started.
    #[test]
    fn background_work_never_needs_the_window() {
        let background = [
            // The player and the media session.
            "load",
            "play",
            "pause",
            "seek",
            "stop",
            "fadeOut",
            "setEffects",
            "setTrackGain",
            "setEnd",
            "setVolume",
            "setRate",
            "mediaState",
            // Downloads, their files and notifications.
            "downloadsNotify",
            "notice",
            "websiteDownloads",
            "captureStart",
            "captureStatus",
            "captureStop",
            "recordingStart",
            "recordingMark",
            "recordingStatus",
            "recordingStop",
            "recordingEnd",
            "recordingReady",
            "createOutput",
            "finishOutput",
            "deleteFile",
            "listTree",
            "listSaved",
            "openInput",
            "storageAccess",
            "browseMedia",
            // What downloads and playback ask about.
            "networkKind",
            "localNetworkState",
            "deviceState",
            "ttsSpeak",
            "ttsStop",
        ];
        for command in background {
            assert!(
                !needs_window(command),
                "{command} must work without a window"
            );
        }
        for command in [
            "pickFolder",
            "pickFile",
            "pickDocuments",
            "requestPermission",
            "recordingConsent",
            "restartApp",
            "shareFile",
        ] {
            assert!(needs_window(command), "{command} asks the person");
        }
        // Every engine command the native player gets is one of them.
        let (name, payload) = engine_call(&EngineCommand::Pause).unwrap();
        assert_eq!((name.as_str(), payload), ("pause", serde_json::json!({})));
        let (name, payload) = engine_call(&EngineCommand::Seek {
            position_ms: 1_500.0,
        })
        .unwrap();
        assert!(!needs_window(&name));
        assert_eq!(payload["positionMs"], 1_500.0);
    }

    #[test]
    fn answers_on_the_plugins_line_say_ok_or_why_not() {
        assert_eq!(parse_reply("ok:null").unwrap(), serde_json::Value::Null);
        assert_eq!(
            parse_reply(r#"ok:{"kind":"wifi"}"#).unwrap()["kind"],
            "wifi"
        );
        assert_eq!(
            parse_reply("error:Raygun's player isn't ready").unwrap_err(),
            "Raygun's player isn't ready"
        );
        assert!(parse_reply("ok:{").unwrap_err().contains("unreadable"));
    }

    /// The media session's stand-in for Rust's player (`MirrorPlayer.kt`) leaves Android's audio
    /// focus to the webview, which takes it itself once a YouTube song is audible and pauses its
    /// media when it loses it. A request from the stand-in lost the focus to the webview's as the
    /// song started, which reached Rust as a pause, and took it back when Play was pressed, which
    /// paused the webview's song.
    #[test]
    fn the_session_mirror_never_asks_for_audio_focus() {
        let source = include_str!("../android/src/main/java/app/raygun/audio/MirrorPlayer.kt");
        for focus in [
            "requestAudioFocus",
            "AudioFocusRequest",
            "OnAudioFocusChangeListener",
            "AUDIOFOCUS_",
        ] {
            assert!(!source.contains(focus), "MirrorPlayer.kt uses {focus}");
        }
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
