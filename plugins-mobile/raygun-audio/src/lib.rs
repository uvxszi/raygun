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

/// Installs the receiver for engine reports (JSON in the `EngineReport` shape). It is called on
/// the Android main thread, so it must hand work off rather than block. Only the first call wins.
pub fn set_report_handler(handler: impl Fn(String) + Send + Sync + 'static) {
    let _ = REPORT_HANDLER.set(Box::new(handler));
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn deliver_report(json: String) {
    if let Some(handler) = REPORT_HANDLER.get() {
        handler(json);
    }
}

/// Handle to the Kotlin engine, managed as Tauri state on Android only.
pub struct NativeAudio<R: Runtime> {
    #[cfg(target_os = "android")]
    handle: tauri::plugin::PluginHandle<R>,
    #[cfg(not(target_os = "android"))]
    _runtime: std::marker::PhantomData<fn() -> R>,
}

impl<R: Runtime> NativeAudio<R> {
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
