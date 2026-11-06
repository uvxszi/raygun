//! Raygun's window on Android, which can go while Raygun carries on.
//!
//! The window is Raygun's activity. Android destroys it when Raygun is swiped away from the recent
//! apps or left with Back from its first screen, and Tauri ends the app with its last window. While
//! something is queued to play or downloads run, the playback or download service keeps the process
//! in the foreground, and Rust must keep going with it: the next song is found and loaded, the
//! lock screen's controls work, downloads go on. So the app doesn't end then. The song playing in
//! YouTube's embedded player can't go on without the webview: the player moves it to a stream the
//! native player plays, or pauses it with a notification
//! ([`crate::playback::PlaybackService::webview_gone`]).
//!
//! When Raygun is opened again in that process, Android makes a new activity, and `MainActivity`
//! says so through the `raygun-audio` plugin: the main window is made again for it, and its page
//! starts as on a cold start, finding the player as it is.

use tauri::{AppHandle, Manager, RunEvent};

/// Tauri's run events as the app takes them (the shell's `run` callback). Only Android's window
/// goes while Raygun carries on; a desktop window closing ends Raygun as Tauri does.
pub fn on_event(app: &AppHandle, event: RunEvent) {
    if !cfg!(target_os = "android") {
        return;
    }
    match event {
        RunEvent::WindowEvent {
            label,
            event: tauri::WindowEvent::Destroyed,
            ..
        } if label == crate::startup::MAIN_WINDOW => {
            if let Some(playback) = app.try_state::<crate::playback::PlaybackService>() {
                playback.webview_gone();
            }
        }
        // The last window went (a programmatic exit carries a code, and isn't held back).
        RunEvent::ExitRequested {
            code: None, api, ..
        } => {
            if carries_on(app) {
                tracing::info!("Raygun's window went; the music or downloads go on without it");
                api.prevent_exit();
            } else {
                tracing::info!("Raygun's window went with nothing playing or downloading");
            }
        }
        _ => {}
    }
}

/// Whether Raygun goes on without its window: something is queued to play, or downloads run or
/// wait (their service keeps the process).
fn carries_on(app: &AppHandle) -> bool {
    let queued = app
        .try_state::<crate::playback::PlaybackService>()
        .is_some_and(|playback| playback.holds_queue());
    let downloading = app
        .try_state::<std::sync::Arc<crate::notifications::DownloadNotices>>()
        .is_some_and(|notices| notices.running());
    queued || downloading
}

/// Listens for Raygun's activity going and new ones that need the main window again (Android).
pub fn start(app: &AppHandle) {
    if cfg!(target_os = "android") {
        let closing = app.clone();
        tauri_plugin_raygun_audio::set_window_closing_handler(move || let_go(&closing));
        let app = app.clone();
        tauri_plugin_raygun_audio::set_window_handler(move |_| reopen(&app));
    }
}

/// How long the activity's last moments wait for Tauri to let go of its window.
const LET_GO_WAIT: std::time::Duration = std::time::Duration::from_millis(500);

/// Raygun's activity is about to be destroyed for good, on Android's main thread, which waits:
/// Tauri stops sending the window's webview scripts before wry lets go of the activity. wry
/// panics on a script for an activity it no longer knows, which would end the process Raygun
/// carries on in, and Tauri sends the scripts of anything emitted until its event loop takes the
/// window's end in. A destroyed window's scripts go nowhere, so what's emitted meanwhile is
/// dropped; the next window's page reads everything afresh.
fn let_go(app: &AppHandle) {
    let Some(window) = app.get_webview_window(crate::startup::MAIN_WINDOW) else {
        return;
    };
    if let Err(error) = window.destroy() {
        tracing::warn!(%error, "couldn't let go of Raygun's window");
        return;
    }
    // The event loop takes tasks in order: one queued after the destroy runs once it's done.
    let (done, taken) = std::sync::mpsc::channel();
    if app
        .run_on_main_thread(move || {
            let _ = done.send(());
        })
        .is_ok()
        && taken.recv_timeout(LET_GO_WAIT).is_err()
    {
        tracing::warn!("Raygun's window wasn't let go of in time");
    }
}

/// A new activity came for Raygun after its last one went: it gets the main window. Made on a
/// thread of its own, since Tauri waits for its event loop to make it, and the activity's own
/// thread (where this is called) must go on.
fn reopen(app: &AppHandle) {
    if app
        .get_webview_window(crate::startup::MAIN_WINDOW)
        .is_some()
    {
        return;
    }
    let app = app.clone();
    let spawned = std::thread::Builder::new()
        .name("raygun-window".into())
        .spawn(move || match crate::startup::reopen_main_window(&app) {
            Ok(()) => tracing::info!("Raygun's window is back"),
            Err(error) => tracing::error!(%error, "couldn't make Raygun's window again"),
        });
    if let Err(error) = spawned {
        tracing::error!(%error, "couldn't start making Raygun's window again");
    }
}
