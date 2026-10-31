//! Integration with the phone's own UI (M09): media controls from the notification, lock screen
//! and headsets, and system bar colours that follow Raygun's theme.

use raygun_model::{PlaybackControl, Theme};
use tauri::AppHandle;

/// Maps a control sent by the native media session to a player request (used on Android only).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn control_from_system(control: &str) -> Option<PlaybackControl> {
    match control {
        "next" => Some(PlaybackControl::Next),
        "previous" => Some(PlaybackControl::Previous),
        "play" => Some(PlaybackControl::Play),
        "pause" => Some(PlaybackControl::Pause),
        _ => None,
    }
}

fn theme_name(theme: Theme) -> &'static str {
    match theme {
        Theme::System => "system",
        Theme::Light => "light",
        Theme::Dark => "dark",
        Theme::Amoled => "amoled",
    }
}

/// Matches the status and navigation bars to `theme`. A no-op off Android.
pub fn apply_theme(app: &AppHandle, theme: Theme) {
    #[cfg(target_os = "android")]
    {
        use tauri::Manager;
        use tauri_plugin_raygun_audio::NativeAudio;

        if let Err(error) = app
            .state::<NativeAudio<tauri::Wry>>()
            .apply_theme(theme_name(theme))
        {
            tracing::warn!(%error, "could not style the system bars");
        }
    }
    #[cfg(not(target_os = "android"))]
    let _ = (app, theme_name(theme));
}

/// Applies the stored theme once settings can be read. Runs off the main thread, because the native
/// call waits for the Android main thread.
pub fn apply_saved_theme(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        use tauri::Manager;

        let db = &app.state::<crate::startup::AppDb>().inner().0;
        match crate::settings::load(db).await {
            Ok(settings) => apply_theme(&app, settings.theme),
            Err(error) => tracing::warn!(%error, "could not read the theme for the system bars"),
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_system_media_controls() {
        assert_eq!(control_from_system("next"), Some(PlaybackControl::Next));
        assert_eq!(
            control_from_system("previous"),
            Some(PlaybackControl::Previous)
        );
        assert_eq!(control_from_system("play"), Some(PlaybackControl::Play));
        assert_eq!(control_from_system("pause"), Some(PlaybackControl::Pause));
        assert_eq!(control_from_system("rewind"), None);
    }

    #[test]
    fn theme_names_match_the_native_side() {
        assert_eq!(theme_name(Theme::System), "system");
        assert_eq!(theme_name(Theme::Light), "light");
        assert_eq!(theme_name(Theme::Dark), "dark");
        assert_eq!(theme_name(Theme::Amoled), "amoled");
    }
}
