//! The miniplayer (M46.13): Spotify's small always-on-top window with the artwork, the song and,
//! on hover, the controls. It is another window on the same page, told apart by its label (an
//! app URL is a path only: a `?window=` query never reached the page). It only draws the
//! miniplayer: playback stays in the main window's engines, and the miniplayer follows it
//! through the same events. Desktop only.

use std::path::PathBuf;

use tauri::AppHandle;

/// The miniplayer window's label (its capability lists it).
#[cfg_attr(any(target_os = "android", target_os = "ios"), allow(dead_code))]
pub const LABEL: &str = "miniplayer";

/// Where the webviews keep their data: the miniplayer shares the main window's profile.
#[cfg_attr(any(target_os = "android", target_os = "ios"), allow(dead_code))]
pub struct MiniplayerDir(pub PathBuf);

/// `#rrggbb` as a window colour.
#[cfg_attr(any(target_os = "android", target_os = "ios"), allow(dead_code))]
fn parse_color(text: &str) -> Option<tauri::window::Color> {
    let hex = text.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let value = u32::from_str_radix(hex, 16).ok()?;
    Some(tauri::window::Color(
        (value >> 16) as u8,
        (value >> 8) as u8,
        value as u8,
        255,
    ))
}

/// Opens the miniplayer, or brings it forward. `background` (`#rrggbb`, the app's own) shows
/// while its page loads instead of white.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn open(app: &AppHandle, background: Option<&str>) -> Result<(), String> {
    use tauri::{Manager, WebviewUrl, WebviewWindowBuilder};

    if let Some(window) = app.get_webview_window(LABEL) {
        let _ = window.unminimize();
        let _ = window.show();
        return window.set_focus().map_err(|error| error.to_string());
    }
    let mut builder = WebviewWindowBuilder::new(app, LABEL, WebviewUrl::App("index.html".into()))
        .title("Raygun")
        .inner_size(320.0, 384.0)
        .min_inner_size(200.0, 240.0)
        .decorations(false)
        .always_on_top(true)
        .resizable(true);
    if let Some(dir) = app.try_state::<MiniplayerDir>() {
        builder = builder.data_directory(dir.0.clone());
    }
    if let Some(color) = background.and_then(parse_color) {
        builder = builder.background_color(color);
    }
    builder
        .build()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn open(_app: &AppHandle, _background: Option<&str>) -> Result<(), String> {
    Err("The miniplayer is a desktop window".into())
}

/// Closes the miniplayer and, with `show_main`, brings Raygun's main window forward.
#[cfg(not(any(target_os = "android", target_os = "ios")))]
pub fn close(app: &AppHandle, show_main: bool) -> Result<(), String> {
    use tauri::Manager;

    if show_main && let Some(main) = app.get_webview_window("main") {
        let _ = main.unminimize();
        let _ = main.show();
        let _ = main.set_focus();
    }
    if let Some(window) = app.get_webview_window(LABEL) {
        window.close().map_err(|error| error.to_string())?;
    }
    Ok(())
}

#[cfg(any(target_os = "android", target_os = "ios"))]
pub fn close(_app: &AppHandle, _show_main: bool) -> Result<(), String> {
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_are_read_from_hex() {
        assert_eq!(
            parse_color("#0a141e"),
            Some(tauri::window::Color(10, 20, 30, 255))
        );
        assert_eq!(parse_color("0a141e"), None);
        assert_eq!(parse_color("#fff"), None);
    }
}
