//! The video player on desktop (M46.4). The webview plays video there
//! (`apps/raygun/src/video/`), so opening the player and its updates are `VideoPlayerEvent`s with
//! the same JSON the Android player gets, and its reports come back through
//! `video_player_report` into the same queue as Android's.
//!
//! Streams are linked through the proxy's `/media/` route, which sends their headers (the webview
//! can't) and keeps HLS and DASH playlists inside it. What the webview can't decode plays in mpv
//! or VLC when one is installed.

use std::collections::BTreeMap;
use std::path::PathBuf;

use raygun_model::{ExternalPlay, VideoPlayerMessage, VideoPlayerMessageKind};
use raygun_net::MediaLinks;
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::VideoPlayerEvent;

/// Opens the webview's player with `payload` (a request, or a session's shell).
pub fn open(app: &AppHandle, payload: &Value) -> Result<(), String> {
    let payload = match app.try_state::<MediaLinks>() {
        Some(media) => for_webview(&media, payload),
        None => payload.clone(),
    };
    emit(app, VideoPlayerMessageKind::Open, &payload)
}

/// Sends an update to the webview's player; a `load` gets its stream linked like an opening.
pub fn update(app: &AppHandle, update: &Value) -> Result<(), String> {
    let mut update = update.clone();
    if update.get("type").and_then(Value::as_str) == Some("load")
        && let Some(request) = update.get("request")
        && let Some(media) = app.try_state::<MediaLinks>()
    {
        update["request"] = for_webview(&media, request);
    }
    emit(app, VideoPlayerMessageKind::Update, &update)
}

fn emit(app: &AppHandle, kind: VideoPlayerMessageKind, json: &Value) -> Result<(), String> {
    VideoPlayerEvent(VideoPlayerMessage {
        kind,
        json: json.to_string(),
    })
    .emit(app)
    .map_err(|error| error.to_string())
}

/// A request as the webview plays it: a remote stream through `/media/` with its headers left
/// there. The proxy's own links (downloads, subtitles) stay as they are.
fn for_webview(media: &MediaLinks, payload: &Value) -> Value {
    let mut payload = payload.clone();
    let Some(url) = payload
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty() && !media.is_local(url))
    else {
        return payload;
    };
    let headers: BTreeMap<String, String> = payload
        .get("headers")
        .cloned()
        .and_then(|headers| serde_json::from_value(headers).ok())
        .unwrap_or_default();
    if let Some(link) = media.link(url, &headers) {
        payload["url"] = json!(link);
        payload["headers"] = json!({});
    }
    payload
}

/// The apps that can take a video over, best first: mpv plays nearly everything and takes
/// subtitle addresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalApp {
    Mpv,
    Vlc,
}

impl ExternalApp {
    fn name(self) -> &'static str {
        match self {
            Self::Mpv => "mpv",
            Self::Vlc => "VLC",
        }
    }
}

/// Plays `play` in mpv or VLC; returns the app's name. Nothing is passed through a shell.
pub fn play_external(play: &ExternalPlay) -> Result<String, String> {
    let web = |address: &str| address.starts_with("http://") || address.starts_with("https://");
    if !web(&play.url) {
        return Err("Only web addresses can be played".into());
    }
    let (app, path) =
        find_external().ok_or("Install mpv or VLC to play videos this player can't".to_owned())?;
    std::process::Command::new(&path)
        .args(arguments(app, play))
        .stdin(std::process::Stdio::null())
        .stdout(std::process::Stdio::null())
        .stderr(std::process::Stdio::null())
        .spawn()
        .map_err(|error| format!("{} didn't start: {error}", app.name()))?;
    Ok(app.name().to_owned())
}

fn arguments(app: ExternalApp, play: &ExternalPlay) -> Vec<String> {
    let start = (play.start_ms.max(0.0) / 1000.0).floor() as u64;
    let subtitle = play
        .subtitles
        .iter()
        .find(|address| address.starts_with("http://") || address.starts_with("https://"));
    let mut arguments = Vec::new();
    match app {
        ExternalApp::Mpv => {
            arguments.push(format!("--force-media-title={}", play.title));
            if start > 0 {
                arguments.push(format!("--start={start}"));
            }
            if let Some(subtitle) = subtitle {
                arguments.push(format!("--sub-file={subtitle}"));
            }
            arguments.push("--".into());
        }
        ExternalApp::Vlc => {
            arguments.push(format!("--meta-title={}", play.title));
            if start > 0 {
                arguments.push(format!("--start-time={start}"));
            }
            if let Some(subtitle) = subtitle {
                arguments.push(format!("--sub-file={subtitle}"));
            }
        }
    }
    arguments.push(play.url.clone());
    arguments
}

/// The first installed of mpv and VLC: on `PATH`, then where their installers put them.
fn find_external() -> Option<(ExternalApp, PathBuf)> {
    candidates().into_iter().find(|(_, path)| path.is_file())
}

fn candidates() -> Vec<(ExternalApp, PathBuf)> {
    let executable = |name: &str| {
        if cfg!(windows) {
            format!("{name}.exe")
        } else {
            name.to_owned()
        }
    };
    let on_path: Vec<PathBuf> = std::env::var_os("PATH")
        .map(|path| std::env::split_paths(&path).collect())
        .unwrap_or_default();
    let mut found = Vec::new();
    for (app, name) in [(ExternalApp::Mpv, "mpv"), (ExternalApp::Vlc, "vlc")] {
        for folder in &on_path {
            found.push((app, folder.join(executable(name))));
        }
    }
    let env = |name: &str| std::env::var_os(name).map(PathBuf::from);
    if cfg!(windows) {
        for root in [env("ProgramFiles"), env("ProgramFiles(x86)")]
            .into_iter()
            .flatten()
        {
            found.push((ExternalApp::Mpv, root.join("mpv").join("mpv.exe")));
            found.push((
                ExternalApp::Vlc,
                root.join("VideoLAN").join("VLC").join("vlc.exe"),
            ));
        }
        if let Some(local) = env("LOCALAPPDATA") {
            found.push((
                ExternalApp::Mpv,
                local.join("Programs").join("mpv").join("mpv.exe"),
            ));
        }
        if let Some(home) = env("USERPROFILE") {
            let scoop = home.join("scoop");
            found.push((ExternalApp::Mpv, scoop.join("apps/mpv/current/mpv.exe")));
            found.push((ExternalApp::Vlc, scoop.join("apps/vlc/current/vlc.exe")));
        }
    } else if cfg!(target_os = "macos") {
        found.push((
            ExternalApp::Mpv,
            PathBuf::from("/Applications/mpv.app/Contents/MacOS/mpv"),
        ));
        found.push((ExternalApp::Mpv, PathBuf::from("/opt/homebrew/bin/mpv")));
        found.push((
            ExternalApp::Vlc,
            PathBuf::from("/Applications/VLC.app/Contents/MacOS/VLC"),
        ));
    }
    // mpv anywhere before VLC anywhere.
    found.sort_by_key(|(app, _)| *app == ExternalApp::Vlc);
    found
}

#[cfg(test)]
mod tests {
    use super::*;

    fn play() -> ExternalPlay {
        ExternalPlay {
            url: "http://127.0.0.1:1/media/t/https/cdn.example/a.mkv".into(),
            title: "Show — S1E2".into(),
            subtitles: vec![
                "file:///nope.srt".into(),
                "http://127.0.0.1:1/subtitle/s".into(),
            ],
            start_ms: 61_900.0,
        }
    }

    #[test]
    fn mpv_and_vlc_get_the_title_start_and_subtitles() {
        assert_eq!(
            arguments(ExternalApp::Mpv, &play()),
            [
                "--force-media-title=Show — S1E2",
                "--start=61",
                "--sub-file=http://127.0.0.1:1/subtitle/s",
                "--",
                "http://127.0.0.1:1/media/t/https/cdn.example/a.mkv",
            ]
        );
        let from_start = ExternalPlay {
            start_ms: 0.0,
            subtitles: Vec::new(),
            ..play()
        };
        assert_eq!(
            arguments(ExternalApp::Vlc, &from_start),
            [
                "--meta-title=Show — S1E2",
                "http://127.0.0.1:1/media/t/https/cdn.example/a.mkv",
            ]
        );
    }

    #[test]
    fn only_web_addresses_are_handed_over() {
        let local = ExternalPlay {
            url: "C:\\Windows\\system32\\calc.exe".into(),
            ..play()
        };
        assert!(play_external(&local).is_err());
    }

    #[test]
    fn mpv_is_looked_for_before_vlc() {
        let found = candidates();
        let first_vlc = found.iter().position(|(app, _)| *app == ExternalApp::Vlc);
        let last_mpv = found.iter().rposition(|(app, _)| *app == ExternalApp::Mpv);
        if let (Some(vlc), Some(mpv)) = (first_vlc, last_mpv) {
            assert!(mpv < vlc);
        }
    }

    #[tokio::test]
    async fn streams_go_through_the_media_route_and_downloads_stay() {
        let proxy = raygun_net::StreamProxy::start(raygun_net::http_client().unwrap(), None)
            .await
            .unwrap();
        let media = proxy.media();
        let request = json!({
            "id": "stremio:movie:tt1",
            "url": "https://cdn.example/v/master.m3u8?sig=1",
            "headers": { "Referer": "https://site.example/" },
            "format": "hls",
            "subtitles": [],
        });
        let linked = for_webview(&media, &request);
        let url = linked["url"].as_str().unwrap();
        assert!(media.is_local(url));
        assert!(url.ends_with("/https/cdn.example/v/master.m3u8?sig=1"));
        assert_eq!(linked["headers"], json!({}));
        assert_eq!(linked["format"], "hls");

        let download =
            json!({ "url": format!("http://{}/local/video/1/index.m3u8", proxy.address()) });
        assert_eq!(for_webview(&media, &download), download);
        let shell = json!({ "session": true, "id": "", "title": "Show" });
        assert_eq!(for_webview(&media, &shell), shell);
    }
}
