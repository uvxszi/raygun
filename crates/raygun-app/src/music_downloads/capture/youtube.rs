//! Songs only YouTube's player plays (M46.52): no source gives a file of them, and YouTube's embed
//! plays them in the webview, where neither Raygun nor its capture player can read the stream.
//! Each capture plays the video in a hidden YouTube player of its own in Raygun's window, and
//! Raygun's tap script (`youtube_tap.js`, an initialization script of every frame there) reads its
//! sound inside the player's frame and posts it to Raygun's page, which reports it as any webview
//! capture does ([`super::sound::report`]). Nothing is heard, no permission is asked, and whatever
//! the person listens to plays on; the screen is never recorded.
//!
//! On desktop the player's sound goes through Web Audio at 2× (its pitch left free and read back
//! at its own pace, as the desktop capture player's is). On Android the player is muted, so it
//! doesn't take Android's audio focus from Raygun's own player, and its sound is read from its
//! capture, in real time: faster, the top of its sound would be lost there (words for
//! transcription go at 2×, which keeps what speech needs). A webview whose muted capture gives only
//! zeros is remembered for the run; its captures then tap the sound itself, which would take the
//! audio focus, so they start only while Raygun's player is quiet and stop when it plays
//! ([`super::CaptureError::Interrupted`]).

use std::sync::atomic::{AtomicBool, Ordering};

use raygun_model::PlaybackStatus;
use tauri::{AppHandle, Manager};

use crate::playback::PlaybackService;

/// The tap, run in every frame of Raygun's main window.
pub const TAP_SCRIPT: &str = include_str!("youtube_tap.js");

/// What a muted tap reports when the player's capture gives only zeros.
pub(crate) const SILENT: &str = "silent";

/// A muted capture gave only zeros in this webview (this run of Raygun).
static MUTED_SILENT: AtomicBool = AtomicBool::new(false);

/// One YouTube capture at a time: each is a hidden YouTube player in Raygun's window.
pub(crate) static TURN: tokio::sync::Mutex<()> = tokio::sync::Mutex::const_new(());

/// What a capture is for: a song's file wants all of its sound; its words, what speech needs.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Purpose {
    Song,
    Words,
}

/// How the tap hears YouTube's player.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TapMode {
    /// Its sound through Web Audio, at `speed`, the player unmuted but unheard.
    Graph,
    /// The player muted and its sound read from its capture.
    Muted,
}

#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Tap {
    pub mode: TapMode,
    pub speed: f64,
}

impl Tap {
    /// How to tap on this platform now, for `purpose`.
    pub fn for_purpose(purpose: Purpose) -> Self {
        Self::choose(
            cfg!(target_os = "android"),
            MUTED_SILENT.load(Ordering::Relaxed),
            purpose,
        )
    }

    fn choose(android: bool, muted_silent: bool, purpose: Purpose) -> Self {
        if android && !muted_silent {
            let speed = match purpose {
                Purpose::Song => 1.0,
                Purpose::Words => 2.0,
            };
            return Self {
                mode: TapMode::Muted,
                speed,
            };
        }
        Self {
            mode: TapMode::Graph,
            speed: 2.0,
        }
    }

    /// It takes Android's audio focus (an unmuted player): it waits while Raygun's player plays.
    pub fn needs_quiet(self) -> bool {
        cfg!(target_os = "android") && self.mode == TapMode::Graph
    }

    pub fn mode_name(self) -> &'static str {
        match self.mode {
            TapMode::Graph => "graph",
            TapMode::Muted => "muted",
        }
    }
}

/// A muted tap heard only zeros: captures tap the sound itself from now on.
pub(crate) fn note_silent() {
    if !MUTED_SILENT.swap(true, Ordering::Relaxed) {
        tracing::info!(
            "the webview gives no sound from a muted player; YouTube captures wait for Raygun's player to be quiet"
        );
    }
}

/// Whether Raygun's window is open, its page listening: the hidden player plays in it (Android:
/// the window goes with Raygun's activity, which can close while downloads carry on).
pub(crate) fn window_open(app: &AppHandle) -> bool {
    app.get_webview_window("main").is_some()
        && app
            .try_state::<PlaybackService>()
            .is_none_or(|playback| playback.webview_present())
}

/// Whether Raygun's own player plays (or is about to).
pub(crate) fn player_busy(app: &AppHandle) -> bool {
    app.try_state::<PlaybackService>().is_some_and(|playback| {
        matches!(
            playback.state().status,
            PlaybackStatus::Playing | PlaybackStatus::Loading | PlaybackStatus::Buffering
        )
    })
}

/// Whether a capture with `tap` can start now: Raygun's window is there, and, when the tap takes
/// Android's audio focus, Raygun's player is quiet.
pub(crate) fn can_start(app: &AppHandle, tap: Tap) -> bool {
    window_open(app) && !(tap.needs_quiet() && player_busy(app))
}

/// The YouTube embed host page the hidden player loads in.
pub(crate) fn embed_url(app: &AppHandle) -> Option<String> {
    app.try_state::<PlaybackService>()
        .map(|playback| playback.youtube_embed_url())
}

/// Android: keeps the webview running while Raygun is off screen, for the hidden player (`on`),
/// or lets it go. Nothing to do elsewhere.
pub(crate) async fn hold_webview(app: &AppHandle, on: bool) {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        let held = tokio::task::spawn_blocking(move || {
            app.try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .map(|native| native.capture_hold_webview(on))
        })
        .await;
        if let Ok(Some(Err(error))) = held {
            tracing::info!(%error, "couldn't keep the webview running for a capture");
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, on);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn android_taps_a_muted_player_unless_its_capture_is_silent() {
        let song = Tap::choose(true, false, Purpose::Song);
        assert_eq!(song.mode, TapMode::Muted);
        assert_eq!(song.speed, 1.0, "a song's sound whole: in real time");
        assert_eq!(Tap::choose(true, false, Purpose::Words).speed, 2.0);
        // This webview gave zeros: the sound itself, faster, while Raygun's player is quiet.
        let fallback = Tap::choose(true, true, Purpose::Song);
        assert_eq!(fallback.mode, TapMode::Graph);
        assert_eq!(fallback.speed, 2.0);
        // Desktop always taps the sound itself at 2×: nothing there takes the audio focus.
        let desktop = Tap::choose(false, false, Purpose::Song);
        assert_eq!(desktop.mode, TapMode::Graph);
        assert_eq!(desktop.mode_name(), "graph");
    }

    #[test]
    fn the_tap_stays_idle_outside_raygun_s_own_players() {
        // It only answers YouTube's frames inside Raygun's embed host inside Raygun's page.
        assert!(TAP_SCRIPT.contains("https://www.youtube.com"));
        assert!(TAP_SCRIPT.contains("ancestorOrigins"));
        assert!(TAP_SCRIPT.contains("event.source !== window.parent"));
        assert!(TAP_SCRIPT.contains(&format!("'{SILENT}'")));
    }
}
