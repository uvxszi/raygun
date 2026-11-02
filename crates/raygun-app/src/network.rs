//! What kind of connection this device is on (M46.18): the Audio quality setting for mobile data
//! applies while the phone streams over a cellular network. Desktop is never on mobile data.

#[cfg(target_os = "android")]
use std::sync::Mutex;
#[cfg(target_os = "android")]
use std::time::{Duration, Instant};

use tauri::AppHandle;

/// How long an answer from Android is reused: resolving a queue asks for every song.
#[cfg(target_os = "android")]
const REMEMBERED: Duration = Duration::from_secs(10);

#[cfg(target_os = "android")]
static LAST: Mutex<Option<(Instant, bool)>> = Mutex::new(None);

/// Whether the device's active network is a cellular one.
#[cfg(target_os = "android")]
pub fn on_mobile_data(app: &AppHandle) -> bool {
    use tauri::Manager;

    if let Some((asked, cellular)) = *LAST.lock().unwrap()
        && asked.elapsed() < REMEMBERED
    {
        return cellular;
    }
    let cellular = match app
        .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .network_kind()
    {
        Ok(kind) => kind == "cellular",
        Err(error) => {
            tracing::warn!(%error, "network kind unknown");
            false
        }
    };
    *LAST.lock().unwrap() = Some((Instant::now(), cellular));
    cellular
}

/// Whether the device's active network is a cellular one.
#[cfg(not(target_os = "android"))]
pub fn on_mobile_data(_app: &AppHandle) -> bool {
    false
}
