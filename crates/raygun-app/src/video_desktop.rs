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

/// Sends an update to the webview's player; a `load` gets its stream linked like an opening, and
/// the streams offered for their sound (`audioSources`) each get theirs.
pub fn update(app: &AppHandle, update: &Value) -> Result<(), String> {
    let update = match app.try_state::<MediaLinks>() {
        Some(media) => linked_update(&media, update),
        None => update.clone(),
    };
    emit(app, VideoPlayerMessageKind::Update, &update)
}

/// An update with the streams it carries linked through `/media/`.
fn linked_update(media: &MediaLinks, update: &Value) -> Value {
    let mut update = update.clone();
    match update.get("type").and_then(Value::as_str) {
        Some("load") => {
            if let Some(request) = update.get("request") {
                update["request"] = for_webview(media, request);
            }
        }
        Some("audioSources") => {
            if let Some(items) = update.get_mut("items").and_then(Value::as_array_mut) {
                for item in items.iter_mut() {
                    *item = for_webview(media, item);
                }
            }
        }
        _ => {}
    }
    update
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
/// subtitle addresses (M46.33: the user picks one from those installed).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum ExternalApp {
    Mpv,
    Vlc,
    MpcHc,
    MpcBe,
    PotPlayer,
}

impl ExternalApp {
    const ALL: [Self; 5] = [
        Self::Mpv,
        Self::Vlc,
        Self::MpcHc,
        Self::MpcBe,
        Self::PotPlayer,
    ];

    fn id(self) -> &'static str {
        match self {
            Self::Mpv => "mpv",
            Self::Vlc => "vlc",
            Self::MpcHc => "mpc-hc",
            Self::MpcBe => "mpc-be",
            Self::PotPlayer => "potplayer",
        }
    }

    fn name(self) -> &'static str {
        match self {
            Self::Mpv => "mpv",
            Self::Vlc => "VLC",
            Self::MpcHc => "MPC-HC",
            Self::MpcBe => "MPC-BE",
            Self::PotPlayer => "PotPlayer",
        }
    }
}

/// The players installed here, best first, each once.
pub fn external_players() -> Vec<raygun_model::ExternalPlayer> {
    let mut seen = Vec::new();
    for (app, path) in candidates() {
        if !seen.contains(&app) && path.is_file() {
            seen.push(app);
        }
    }
    seen.into_iter()
        .map(|app| raygun_model::ExternalPlayer {
            id: app.id().to_owned(),
            name: app.name().to_owned(),
        })
        .collect()
}

/// Plays `play` in the chosen player (`player`, else the first installed); returns its name.
/// Nothing is passed through a shell.
pub fn play_external(play: &ExternalPlay, player: Option<&str>) -> Result<String, String> {
    let web = |address: &str| address.starts_with("http://") || address.starts_with("https://");
    if !web(&play.url) {
        return Err("Only web addresses can be played".into());
    }
    let installed = candidates()
        .into_iter()
        .filter(|(_, path)| path.is_file())
        .collect::<Vec<_>>();
    let chosen = player
        .and_then(|wanted| installed.iter().find(|(app, _)| app.id() == wanted))
        .or_else(|| installed.first())
        .cloned();
    let (app, path) = chosen.ok_or_else(|| {
        "Install mpv, VLC, MPC-HC or PotPlayer to play videos this player can't".to_owned()
    })?;
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
    let start_ms = play.start_ms.max(0.0) as u64;
    let start = start_ms / 1000;
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
            arguments.push(play.url.clone());
        }
        ExternalApp::Vlc => {
            arguments.push(format!("--meta-title={}", play.title));
            if start > 0 {
                arguments.push(format!("--start-time={start}"));
            }
            if let Some(subtitle) = subtitle {
                arguments.push(format!("--sub-file={subtitle}"));
            }
            arguments.push(play.url.clone());
        }
        // MPC-HC and MPC-BE: the address first, then switches; `/start` is in milliseconds.
        ExternalApp::MpcHc | ExternalApp::MpcBe => {
            arguments.push(play.url.clone());
            arguments.push("/play".into());
            if start > 0 {
                arguments.push("/start".into());
                arguments.push((start * 1000).to_string());
            }
            if let Some(subtitle) = subtitle {
                arguments.push("/sub".into());
                arguments.push(subtitle.clone());
            }
        }
        ExternalApp::PotPlayer => {
            arguments.push(play.url.clone());
            if start > 0 {
                arguments.push(format!(
                    "/seek={:02}:{:02}:{:02}",
                    start / 3600,
                    start / 60 % 60,
                    start % 60
                ));
            }
            if let Some(subtitle) = subtitle {
                arguments.push(format!("/sub={subtitle}"));
            }
            arguments.push(format!("/title={}", play.title));
        }
    }
    arguments
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
    for (app, name) in [
        (ExternalApp::Mpv, "mpv"),
        (ExternalApp::Vlc, "vlc"),
        (ExternalApp::MpcHc, "mpc-hc64"),
        (ExternalApp::MpcBe, "mpc-be64"),
    ] {
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
            found.push((ExternalApp::MpcHc, root.join("MPC-HC").join("mpc-hc64.exe")));
            found.push((ExternalApp::MpcHc, root.join("MPC-HC").join("mpc-hc.exe")));
            found.push((
                ExternalApp::MpcHc,
                root.join("K-Lite Codec Pack")
                    .join("MPC-HC64")
                    .join("mpc-hc64.exe"),
            ));
            found.push((
                ExternalApp::MpcBe,
                root.join("MPC-BE x64").join("mpc-be64.exe"),
            ));
            found.push((ExternalApp::MpcBe, root.join("MPC-BE").join("mpc-be.exe")));
            found.push((
                ExternalApp::PotPlayer,
                root.join("DAUM")
                    .join("PotPlayer")
                    .join("PotPlayerMini64.exe"),
            ));
            found.push((
                ExternalApp::PotPlayer,
                root.join("DAUM")
                    .join("PotPlayer")
                    .join("PotPlayerMini.exe"),
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
    // Best app first, wherever it is.
    found.sort_by_key(|(app, _)| ExternalApp::ALL.iter().position(|known| known == app));
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
    fn mpc_and_potplayer_get_their_own_switches() {
        assert_eq!(
            arguments(ExternalApp::MpcHc, &play()),
            [
                "http://127.0.0.1:1/media/t/https/cdn.example/a.mkv",
                "/play",
                "/start",
                "61000",
                "/sub",
                "http://127.0.0.1:1/subtitle/s",
            ]
        );
        assert_eq!(
            arguments(ExternalApp::PotPlayer, &play())[1..3],
            ["/seek=00:01:01", "/sub=http://127.0.0.1:1/subtitle/s"]
        );
    }

    #[test]
    fn only_web_addresses_are_handed_over() {
        let local = ExternalPlay {
            url: "C:\\Windows\\system32\\calc.exe".into(),
            ..play()
        };
        assert!(play_external(&local, None).is_err());
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

    #[tokio::test]
    async fn streams_offered_for_their_sound_are_linked_too() {
        let proxy = raygun_net::StreamProxy::start(raygun_net::http_client().unwrap(), None)
            .await
            .unwrap();
        let media = proxy.media();
        let update = json!({
            "type": "audioSources",
            "id": "stremio:movie:tt1",
            "items": [{
                "id": "1:0",
                "url": "https://cdn.example/ita.mkv",
                "headers": { "Referer": "https://site.example/" },
                "format": null,
            }],
        });
        let linked = linked_update(&media, &update);
        let item = &linked["items"][0];
        assert!(media.is_local(item["url"].as_str().unwrap()));
        assert_eq!(item["headers"], json!({}));
        assert_eq!(item["id"], "1:0");
        // Other updates pass as they are.
        let sources = json!({ "type": "sources", "groups": [] });
        assert_eq!(linked_update(&media, &sources), sources);
    }
}
