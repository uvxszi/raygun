//! Raygun on the web using this device as its back end (docs/WEB.md, "The app as a back end").
//!
//! A linked browser opens a back-end channel beside sync (`raygun_sync::remote`, authenticated by
//! the Noise key it linked with). Over it the page asks for commands by their Rust names, which run
//! through Tauri's own invoke path on the main window's webview, so every command answers exactly
//! as it does for this device's own interface; events the page's screens follow come back as they
//! happen; and the proxy's pictures and media (`/cover/`, `/media/`, ...) come in pieces, fetched
//! from the proxy on loopback. Commands about this device itself (its settings, player, windows,
//! files outside Raygun's data, backups, permissions) are refused: the page keeps its own.
//!
//! Messages (JSON, `t` names the kind):
//! - page → app: `call {id, cmd, args}`, `fetch {id, path, range?}`, `cancel {id}`, `ping`
//! - app → page: `welcome {deviceId, name, proxy}`, `result {id, ok | err | denied}`,
//!   `event {event, payload}`, `head {id, status, headers}`, body pieces, `end {id, error?}`, `pong`

use std::collections::HashMap;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};

use raygun_sync::remote::{BODY_PART, Outgoing, RemoteHandler, RemoteSession};
use serde_json::{Value, json};
use tauri::ipc::{CallbackFn, InvokeBody, InvokeError, InvokeResponse, InvokeResponseBody};
use tauri::{AppHandle, Listener, Manager};
use tokio::sync::{mpsc, oneshot};

use crate::events;

/// Where the stream proxy listens (startup), so pictures and media can be fetched for a page.
pub struct ProxyOrigin(pub SocketAddr);

/// The proxy's routes a page may fetch through the channel.
const ROUTES: &[&str] = &[
    "stream", "image", "cover", "subtitle", "art", "local", "font", "pdf", "torrent", "media",
];

/// A body without a range is sent whole up to this size.
const MAX_WHOLE: usize = 64 * 1024 * 1024;

/// Commands a page may not run here, by prefix: this device's own settings, player, windows,
/// pairing, backups and data, permissions, files outside Raygun's data, and what each page keeps
/// itself (library, history, quick access).
const DENIED_PREFIXES: &[&str] = &[
    "settings_",
    "sync_",
    "connect_",
    "devices_",
    "remote_",
    "mcp_",
    "backup_",
    "data_",
    "crash_log",
    "logs_",
    "permission",
    "local_network_",
    "local_folder",
    "local_files_",
    "miniplayer_",
    "playback_",
    "tts_",
    "cast_",
    "video_player_",
    "browser_check_",
    "app_update",
    "app_restart",
    "app_open_",
    "app_whats_new",
    "ai_provider",
    "ai_model",
    "audio_output",
    "website",
    "dev_",
    "incognito_",
    "device_",
    "network_set",
    "safe_start",
    "quick_access_",
    "recent_contexts",
    "history_",
    "favorite",
    "playlist",
    "collection",
    "hidden_",
    "notification_",
];

/// Commands a page may not run here, by name.
const DENIED: &[&str] = &[
    "app_info",
    "storage_info",
    "fonts_list",
    "files_pick",
    "library_connect_device_music",
    "library_add_root",
    "library_remove_root",
    "library_rescan",
    "library_play",
    "extension_open_website",
    "extension_hold_back",
    "catalog_open_page",
    "music_open_info_page",
    "podcast_open_page",
    "dictionary_open_page",
    "answers_open_page",
    "stremio_open_link",
    "sources_queue_read_file",
    "sources_queue_local_address",
    "ytdlp_reveal",
    "export_pick_folder",
    "save_pick_folder",
    "songs_pick_folder",
    "cover_share",
    "cover_copy",
    "page_share",
    "page_copy",
    "covers_webview_jxl",
    "reader_system_bars",
    "song_capture_report",
    "youtube_play",
    "youtube_embed_url",
    "podcast_play",
    "podcast_queue",
    "podcast_queue_picks",
    "audiobook_play",
    "video_play",
    "video_play_link",
    "video_resume",
    "video_play_from_start",
    "video_play_next",
    "video_link_play",
    "video_source_play",
    "video_download_play",
    "stremio_play",
    "video_external_players",
    "storage_clear",
];

/// Why a page may not run `command` here, or None when it may.
pub(crate) fn refusal(command: &str) -> Option<String> {
    let denied = DENIED.contains(&command)
        || DENIED_PREFIXES
            .iter()
            .any(|prefix| command.starts_with(prefix))
        || command.starts_with("plugin:")
        || command.is_empty();
    denied.then(|| format!("{command} runs only on the device itself"))
}

/// The proxy path a page asked for, when it's one of [`ROUTES`] and stays inside it.
pub(crate) fn proxy_path(path: &str) -> Option<&str> {
    let path = path.trim_start_matches('/');
    let (route, rest) = path.split_once('/')?;
    let target = rest.split(['?', '#']).next().unwrap_or_default();
    let clean = !target.is_empty()
        && !target.contains('\\')
        && target.split('/').all(|part| {
            part != ".." && part != "." && !part.contains("%2e%2e") && !part.contains("%2E%2E")
        });
    (ROUTES.contains(&route) && clean).then_some(path)
}

/// Event names the page's screens follow. This device's player, windows, prompts and settings
/// stay its own.
fn forwarded_events() -> Vec<&'static str> {
    use tauri_specta::Event;
    vec![
        events::ExtensionsChangedEvent::NAME,
        events::SourcesSetupEvent::NAME,
        events::ExtensionInstallsEvent::NAME,
        events::SourceQueueEvent::NAME,
        events::SearchProgressEvent::NAME,
        events::MangaUpdateEvent::NAME,
        events::MangaDownloadsEvent::NAME,
        events::PodcastsChangedEvent::NAME,
        events::AudiobooksFoundEvent::NAME,
        events::WatchChangedEvent::NAME,
        events::VideoDownloadsEvent::NAME,
        events::SongDownloadsEvent::NAME,
        events::SongDownloadProgressEvent::NAME,
        events::YtDlpEvent::NAME,
        events::MihonRuntimeEvent::NAME,
        events::MadeForYouEvent::NAME,
        events::TrackDurationsEvent::NAME,
        events::ImportEvent::NAME,
        events::CaptionsEvent::NAME,
        events::LyricsTranscribedEvent::NAME,
        events::LyricsTranslationEvent::NAME,
        events::ArtistReleasesEvent::NAME,
        events::ArtistProfileEvent::NAME,
        events::ExportEvent::NAME,
    ]
}

/// Takes each linked browser's channel (`raygun_sync::Engine::on_remote`).
pub fn handler(app: AppHandle) -> RemoteHandler {
    Arc::new(move |session| {
        tauri::async_runtime::spawn(serve(app.clone(), session));
    })
}

async fn serve(app: AppHandle, session: RemoteSession) {
    let RemoteSession {
        peer,
        mut incoming,
        outgoing,
    } = session;
    tracing::info!(browser = %peer.name, "a browser uses this device as its back end");
    let proxy = app.try_state::<ProxyOrigin>().map(|origin| origin.0);
    let (device_id, name) = app
        .try_state::<crate::device_sync::SyncService>()
        .map(|sync| sync.this_device())
        .unwrap_or_default();
    let _ = outgoing.send(Outgoing::Json(json!({
        "t": "welcome",
        "deviceId": device_id,
        "name": name,
        "proxy": proxy.map(|address| format!("http://{address}")),
    })));
    let listeners: Vec<_> = forwarded_events()
        .into_iter()
        .map(|event| {
            let outgoing = outgoing.clone();
            app.listen_any(event, move |emitted| {
                let payload =
                    serde_json::from_str::<Value>(emitted.payload()).unwrap_or(Value::Null);
                let _ = outgoing.send(Outgoing::Json(
                    json!({ "t": "event", "event": event, "payload": payload }),
                ));
            })
        })
        .collect();
    let fetches: Arc<Mutex<HashMap<u64, tauri::async_runtime::JoinHandle<()>>>> = Arc::default();
    let client = reqwest::Client::builder().no_proxy().build().ok();
    while let Some(message) = incoming.recv().await {
        let id = message["id"].as_u64().unwrap_or_default();
        match message["t"].as_str().unwrap_or_default() {
            "call" => {
                let app = app.clone();
                let outgoing = outgoing.clone();
                let command = message["cmd"].as_str().unwrap_or_default().to_owned();
                let args = match &message["args"] {
                    Value::Null => json!({}),
                    args => args.clone(),
                };
                tauri::async_runtime::spawn(async move {
                    let answer = match refusal(&command) {
                        Some(reason) => json!({ "t": "result", "id": id, "denied": reason }),
                        None => match call(&app, &command, args).await {
                            Ok(value) => json!({ "t": "result", "id": id, "ok": value }),
                            Err(error) => json!({ "t": "result", "id": id, "err": error }),
                        },
                    };
                    let _ = outgoing.send(Outgoing::Json(answer));
                });
            }
            "fetch" => {
                let outgoing = outgoing.clone();
                let path = message["path"].as_str().unwrap_or_default().to_owned();
                let range = message["range"].as_str().map(str::to_owned);
                let (Some(proxy), Some(client)) = (proxy, client.clone()) else {
                    let _ = outgoing.send(Outgoing::Json(
                        json!({ "t": "end", "id": id, "error": "nothing to fetch from" }),
                    ));
                    continue;
                };
                let done = fetches.clone();
                let task = tauri::async_runtime::spawn(async move {
                    let result = fetch(&client, proxy, id, &path, range, &outgoing).await;
                    let end = match result {
                        Ok(()) => json!({ "t": "end", "id": id }),
                        Err(error) => json!({ "t": "end", "id": id, "error": error }),
                    };
                    let _ = outgoing.send(Outgoing::Json(end));
                    done.lock().expect("fetches lock poisoned").remove(&id);
                });
                fetches
                    .lock()
                    .expect("fetches lock poisoned")
                    .insert(id, task);
            }
            "cancel" => {
                if let Some(task) = fetches.lock().expect("fetches lock poisoned").remove(&id) {
                    task.abort();
                }
            }
            "ping" => {
                let _ = outgoing.send(Outgoing::Json(json!({ "t": "pong" })));
            }
            _ => {}
        }
    }
    for listener in listeners {
        app.unlisten(listener);
    }
    for (_, task) in fetches.lock().expect("fetches lock poisoned").drain() {
        task.abort();
    }
    tracing::info!(browser = %peer.name, "a browser stopped using this device");
}

/// Runs one command through Tauri's invoke path on the main window's webview, as this device's
/// own interface would.
async fn call(app: &AppHandle, command: &str, args: Value) -> Result<Value, Value> {
    let window = app.get_webview_window("main").ok_or_else(|| {
        Value::from("Raygun's window is closed on that device. Open Raygun there and try again.")
    })?;
    let webview: tauri::Webview = window.as_ref().clone();
    let url = webview
        .url()
        .map_err(|error| Value::from(error.to_string()))?;
    let (answered, answer) = oneshot::channel::<InvokeResponse>();
    let request = tauri::webview::InvokeRequest {
        cmd: command.to_owned(),
        callback: CallbackFn(0),
        error: CallbackFn(1),
        url,
        body: InvokeBody::Json(args),
        headers: Default::default(),
        invoke_key: app.invoke_key().to_owned(),
    };
    let target = webview.clone();
    app.run_on_main_thread(move || {
        target.on_message(
            request,
            Box::new(move |_, _, response, _, _| {
                let _ = answered.send(response);
            }),
        );
    })
    .map_err(|error| Value::from(error.to_string()))?;
    match answer.await {
        Ok(InvokeResponse::Ok(InvokeResponseBody::Json(json))) => {
            Ok(serde_json::from_str(&json).unwrap_or(Value::Null))
        }
        Ok(InvokeResponse::Ok(InvokeResponseBody::Raw(bytes))) => Ok(Value::from(bytes)),
        Ok(InvokeResponse::Err(InvokeError(error))) => Err(error),
        Err(_) => Err(Value::from(format!("{command} gave no answer"))),
    }
}

/// Fetches one proxy path (a range of it, when asked) and sends its head and body.
async fn fetch(
    client: &reqwest::Client,
    proxy: SocketAddr,
    id: u64,
    path: &str,
    range: Option<String>,
    outgoing: &mpsc::UnboundedSender<Outgoing>,
) -> Result<(), String> {
    let path = proxy_path(path).ok_or("that isn't something this device serves")?;
    let mut request = client.get(format!("http://{proxy}/{path}"));
    if let Some(range) = &range {
        request = request.header(reqwest::header::RANGE, range);
    }
    let mut response = request.send().await.map_err(|error| error.to_string())?;
    let mut headers = serde_json::Map::new();
    for name in [
        "content-type",
        "content-length",
        "content-range",
        "accept-ranges",
        "last-modified",
        "etag",
    ] {
        if let Some(value) = response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
        {
            headers.insert(name.to_owned(), Value::from(value));
        }
    }
    let _ = outgoing.send(Outgoing::Json(json!({
        "t": "head",
        "id": id,
        "status": response.status().as_u16(),
        "headers": headers,
    })));
    let body_id = u32::try_from(id).map_err(|_| "request id too large")?;
    let mut sent = 0usize;
    let mut piece = Vec::with_capacity(BODY_PART);
    while let Some(chunk) = response.chunk().await.map_err(|error| error.to_string())? {
        sent += chunk.len();
        if range.is_none() && sent > MAX_WHOLE {
            return Err("too large to send whole; ask for ranges".into());
        }
        piece.extend_from_slice(&chunk);
        if piece.len() >= BODY_PART {
            let bytes = std::mem::replace(&mut piece, Vec::with_capacity(BODY_PART));
            if outgoing
                .send(Outgoing::Body { id: body_id, bytes })
                .is_err()
            {
                return Ok(());
            }
        }
    }
    if !piece.is_empty() {
        let _ = outgoing.send(Outgoing::Body {
            id: body_id,
            bytes: piece,
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn commands() -> Vec<String> {
        let bindings = std::fs::read_to_string(crate::workspace_bindings_path()).unwrap();
        bindings
            .split("__TAURI_INVOKE")
            .skip(1)
            .filter_map(|rest| rest.split('"').nth(1))
            .map(str::to_owned)
            .collect()
    }

    #[test]
    fn this_devices_own_commands_are_refused() {
        for command in [
            "settings_update",
            "sync_join",
            "backup_restore",
            "data_erase_everything",
            "permission_request",
            "playback_control",
            "miniplayer_open",
            "local_folder_add_path",
            "library_add_root",
            "files_pick",
            "app_restart_safe",
            "favorite_set",
            "plugin:event|listen",
            "video_play",
            "",
        ] {
            assert!(refusal(command).is_some(), "{command} should be refused");
        }
    }

    #[test]
    fn sources_catalogues_and_downloads_are_allowed() {
        for command in [
            "online_search",
            "catalog_search",
            "catalog_global_search",
            "manga_title",
            "manga_chapter",
            "novel_chapter",
            "stremio_meta",
            "stremio_streams",
            "extensions_list",
            "extension_install",
            "sources_add",
            "songs_download",
            "manga_download",
            "video_source_download",
            "music_artist",
            "podcast_search",
            "answers_ask",
        ] {
            assert_eq!(refusal(command), None, "{command} should be allowed");
        }
    }

    #[test]
    fn every_denial_names_real_commands() {
        let commands = commands();
        assert!(commands.len() > 300, "the bindings list the commands");
        for name in DENIED {
            assert!(
                commands.iter().any(|command| command == name),
                "{name} isn't a command"
            );
        }
        for prefix in DENIED_PREFIXES {
            assert!(
                commands.iter().any(|command| command.starts_with(prefix)),
                "no command starts with {prefix}"
            );
        }
    }

    #[test]
    fn only_the_proxys_routes_are_fetched() {
        assert_eq!(proxy_path("cover/abc"), Some("cover/abc"));
        assert_eq!(proxy_path("/pdf/l00/3?w=800"), Some("pdf/l00/3?w=800"));
        assert_eq!(
            proxy_path("media/tok/https/example.org/a/b.m3u8"),
            Some("media/tok/https/example.org/a/b.m3u8")
        );
        for refused in [
            "embed/youtube",
            "cover/",
            "local/../secrets.db",
            "local/a/%2e%2e/b",
            "local\\..\\x",
            "nothing",
            "",
        ] {
            assert_eq!(proxy_path(refused), None, "{refused}");
        }
    }
}
