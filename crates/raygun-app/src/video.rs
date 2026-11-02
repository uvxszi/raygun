//! Watching video (M38): the native player on Android (Media3, in the audio plugin) and the
//! progress it reports, for resuming and "continue watching". On desktop the webview plays it
//! (M46.4, `video_desktop`), with the same requests, updates and reports.
//! Progress also goes to the library's episodes, and finished downloads play instead of the
//! stream (M43). Episodes of a title play through a session (M44, `video_session`), which loads
//! videos into the open player and answers its requests.
//!
//! The player's controls (M45) follow `Settings::video_player`, and each title remembers its speed,
//! aspect, tracks and subtitle delay (`video.playerState.<group>`). The player saves changes made
//! in it through `request` reports, and asks for AniSkip's chapters once it knows the length.

use raygun_db::video::{self as store, VideoEntry};
use raygun_model::{
    AnimeEpisode, ContinueWatching, PlaybackControl, SettingChange, VideoPlayerPrefs, VideoRequest,
    VideoSubtitle,
};
use raygun_net::ImageLinks;
use serde_json::{Map, Value, json};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::now_ms;
use crate::events::{SettingsChangedEvent, WatchChangedEvent};
use crate::playback::PlaybackService;
use crate::startup::AppDb;
use crate::video_session::{EpisodeRef, VideoSessions};

/// The player settings video playback follows (Settings → Player, M42.1).
struct PlayerPrefs {
    resume: bool,
    /// Past this share a video counts as watched.
    watched_share: f64,
    seek_ms: u32,
    /// The subtitle language to turn on by itself; `Some("")` follows the device's language.
    auto_subtitles: Option<String>,
    /// The next episode plays by itself (M44).
    autoplay: bool,
    /// When the next-episode prompt shows, before the end.
    next_prompt_ms: u32,
    /// Controls, gestures, tracks and tools (M45).
    video_player: VideoPlayerPrefs,
}

/// Where a title's player memory is kept, after its group.
const STATE_PREFIX: &str = "video.playerState.";
/// The brightness the player last had, for "remember brightness".
const BRIGHTNESS_KEY: &str = "video.brightness";
/// What a title remembers: speed, aspect, subtitle and audio choices, subtitle delay, intro length.
const STATE_KEYS: [&str; 8] = [
    "speed",
    "aspect",
    "subtitle",
    "secondarySubtitle",
    "subtitleDelayMs",
    "audio",
    "introMs",
    // Subtitle lines changed by hand in the desktop player, per episode and track (M46.12).
    "subtitleEdits",
];

async fn player_prefs(db: &raygun_db::Db) -> PlayerPrefs {
    let settings = crate::settings::load(db).await.unwrap_or_default();
    // The app's language, unless it follows the device (the player knows the device's).
    let app_language = match settings.language.as_str() {
        raygun_model::SYSTEM_LANGUAGE => String::new(),
        tag => tag.to_owned(),
    };
    PlayerPrefs {
        resume: settings.resume_videos,
        watched_share: f64::from(settings.watched_percent) / 100.0,
        seek_ms: settings.seek_seconds * 1000,
        autoplay: settings.autoplay_next,
        next_prompt_ms: settings.next_prompt_seconds * 1000,
        video_player: settings.video_player.clone(),
        auto_subtitles: settings.auto_subtitles.then(|| {
            if settings.subtitle_language.is_empty() {
                app_language
            } else {
                settings.subtitle_language.clone()
            }
        }),
    }
}

/// A report from the player.
#[derive(Debug, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
struct PlayerReport {
    id: String,
    state: String,
    position_ms: f64,
    duration_ms: Option<f64>,
    error: Option<String>,
}

/// Plays `request` in the video player, resuming where it was left (from the start once watched).
/// A session that was playing ends.
pub async fn play(app: &AppHandle, request: VideoRequest) -> Result<(), String> {
    app.state::<VideoSessions>().end();
    let mut payload = prepare(app, &request, None, None).await?;
    payload["labels"] = app.state::<VideoSessions>().labels();
    open_player(app, &payload)
}

/// Plays `request` in another app (M45): the player hands it over at once and saves the position
/// that app gives back.
pub async fn play_elsewhere(app: &AppHandle, request: VideoRequest) -> Result<(), String> {
    app.state::<VideoSessions>().end();
    let mut payload = prepare(app, &request, None, None).await?;
    payload["labels"] = app.state::<VideoSessions>().labels();
    payload["external"] = json!(true);
    open_player(app, &payload)
}

/// The player's request for a video: where to start (saved progress unless `start_at`), after
/// recording that it started. `resume` replaces the request saved for resuming (a session saves
/// an episode reference). A finished download plays instead of the stream.
pub async fn prepare(
    app: &AppHandle,
    request: &VideoRequest,
    resume: Option<String>,
    start_at: Option<i64>,
) -> Result<serde_json::Value, String> {
    if !request.url.starts_with("https://") && !request.url.starts_with("http://") {
        return Err("Only web addresses can be played".into());
    }
    let db = app.state::<AppDb>().0.clone();
    let prefs = player_prefs(&db).await;
    let saved = store::progress(&db, &request.key)
        .await
        .map_err(|error| error.to_string())?;
    let resumed = saved
        .filter(|_| prefs.resume)
        .filter(|progress| progress.watched_at.is_none())
        .filter(|progress| {
            progress.duration_ms.is_none_or(|duration| {
                (progress.position_ms as f64) < duration as f64 * prefs.watched_share
            })
        })
        .map_or(0, |progress| progress.position_ms);
    let start_ms = start_at.unwrap_or(resumed);
    let entry = VideoEntry {
        key: request.key.clone(),
        title: request.title.clone(),
        subtitle: request.subtitle.clone(),
        // The source's address: a proxy link wouldn't outlive the app.
        poster: request
            .poster
            .as_deref()
            .map(|poster| app.state::<raygun_net::ImageLinks>().original(poster)),
        request: match resume {
            Some(resume) => resume,
            None => serde_json::to_string(&request).map_err(|error| error.to_string())?,
        },
    };
    store::started(&db, &entry, now_ms())
        .await
        .map_err(|error| error.to_string())?;
    // A finished download plays from the device, with its saved subtitles.
    let (url, headers, format, subtitles) =
        match crate::video_downloads::offline(app, &request.key).await {
            Some(offline) => (
                offline.url,
                Default::default(),
                offline.format,
                offline.subtitles,
            ),
            None => (
                request.url.clone(),
                request.headers.clone(),
                request.format.clone(),
                proxied_subtitles(app, &request.subtitles),
            ),
        };
    // The title's memory, and AniSkip unless the source's own chapters are enough.
    let group = request.group.clone().unwrap_or_else(|| request.key.clone());
    let title_state = title_state(&db, &group).await;
    let player = &prefs.video_player;
    let anime = request.anime.as_ref().filter(|_| {
        player.aniskip && !(player.aniskip_unless_chapters && !request.chapters.is_empty())
    });
    let settings = player_settings(&db, &prefs).await;
    // Music and video don't play over each other.
    app.state::<PlaybackService>()
        .control(PlaybackControl::Pause);
    Ok(serde_json::json!({
        "group": group,
        "titleState": title_state,
        "chapters": request.chapters,
        "anime": anime,
        "prefs": settings,
        // "Always use another app" (Settings → Player → Advanced).
        "external": player.external_player,
        "id": request.key,
        "url": url,
        "title": request.title,
        "subtitle": request.subtitle,
        "headers": headers,
        "format": format,
        "subtitles": subtitles,
        "startMs": start_ms,
        "seekStepMs": prefs.seek_ms,
        "autoSubtitleLanguage": prefs.auto_subtitles,
    }))
}

/// What opens the player for a session before any video is known (M44): it shows the title and
/// a spinner until the first `load`.
pub async fn session_shell(app: &AppHandle, title: &str, subtitle: &str) -> serde_json::Value {
    let db = app.state::<AppDb>().0.clone();
    let prefs = player_prefs(&db).await;
    let settings = player_settings(&db, &prefs).await;
    app.state::<PlaybackService>()
        .control(PlaybackControl::Pause);
    serde_json::json!({
        "session": true,
        "prefs": settings,
        "id": "",
        "title": title,
        "subtitle": subtitle,
        "seekStepMs": prefs.seek_ms,
        "autoSubtitleLanguage": prefs.auto_subtitles,
        "autoplay": prefs.autoplay,
        "nextPromptMs": prefs.next_prompt_ms,
        "labels": app.state::<VideoSessions>().labels(),
    })
}

/// The player's settings as it reads them, with the brightness to restore when it's remembered.
async fn player_settings(db: &raygun_db::Db, prefs: &PlayerPrefs) -> Value {
    let mut value = serde_json::to_value(&prefs.video_player).unwrap_or_default();
    if prefs.video_player.remember_brightness
        && let Ok(Some(saved)) = raygun_db::local_value(db, BRIGHTNESS_KEY).await
        && let Ok(level) = saved.parse::<f64>()
    {
        value["brightness"] = json!(level);
    }
    value
}

/// A title's player memory; null when it has none.
async fn title_state(db: &raygun_db::Db, group: &str) -> Value {
    raygun_db::local_value(db, &format!("{STATE_PREFIX}{group}"))
        .await
        .ok()
        .flatten()
        .and_then(|text| serde_json::from_str::<Value>(&text).ok())
        .filter(Value::is_object)
        .unwrap_or(Value::Null)
}

/// Merges what the player changed into a title's memory. Null forgets a key; keys a title doesn't
/// remember are ignored.
fn merge_state(saved: &mut Map<String, Value>, change: &Map<String, Value>) {
    for (key, value) in change {
        if !STATE_KEYS.contains(&key.as_str()) {
            continue;
        }
        if value.is_null() {
            saved.remove(key);
        } else {
            saved.insert(key.clone(), value.clone());
        }
    }
}

/// Applies a change made in the player to the saved player settings: known keys only, and the
/// subtitle style field by field.
fn merge_prefs(saved: &mut Value, change: &Map<String, Value>) {
    for (key, value) in change {
        match (saved.get_mut(key), value) {
            (Some(Value::Object(fields)), Value::Object(changed)) => {
                for (field, value) in changed {
                    if fields.contains_key(field) {
                        fields.insert(field.clone(), value.clone());
                    }
                }
            }
            (Some(slot), _) => *slot = value.clone(),
            (None, _) => {}
        }
    }
}

/// The player's own requests (M45), for sessions and single videos alike.
async fn player_request(app: &AppHandle, action: &str, raw: &Value) {
    let db = app.state::<AppDb>().0.clone();
    match action {
        "state" => {
            let group = raw.get("group").and_then(Value::as_str).unwrap_or_default();
            let Some(change) = raw.get("change").and_then(Value::as_object) else {
                return;
            };
            if group.is_empty() {
                return;
            }
            let key = format!("{STATE_PREFIX}{group}");
            let mut saved = match title_state(&db, group).await {
                Value::Object(saved) => saved,
                _ => Map::new(),
            };
            merge_state(&mut saved, change);
            let text = Value::Object(saved).to_string();
            if let Err(error) = raygun_db::set_local_value(&db, &key, &text).await {
                tracing::warn!(%error, "could not remember the player's state");
            }
        }
        "prefs" => {
            let Some(change) = raw.get("change").and_then(Value::as_object) else {
                return;
            };
            let saved = async {
                let settings = crate::settings::load(&db)
                    .await
                    .map_err(|error| error.to_string())?;
                let mut value = serde_json::to_value(&settings.video_player)
                    .map_err(|error| error.to_string())?;
                merge_prefs(&mut value, change);
                let prefs: VideoPlayerPrefs =
                    serde_json::from_value(value).map_err(|error| error.to_string())?;
                crate::settings::update(&db, SettingChange::VideoPlayer(Box::new(prefs)))
                    .await
                    .map_err(|error| error.to_string())
            };
            match saved.await {
                Ok(_) => {
                    let _ = SettingsChangedEvent("videoPlayer".into()).emit(app);
                }
                Err(error) => tracing::warn!(%error, "could not save a player setting"),
            }
        }
        "brightness" => {
            let Some(level) = raw
                .get("value")
                .and_then(Value::as_f64)
                .filter(|level| (0.0..=1.0).contains(level))
            else {
                return;
            };
            if let Err(error) =
                raygun_db::set_local_value(&db, BRIGHTNESS_KEY, &level.to_string()).await
            {
                tracing::warn!(%error, "could not remember the brightness");
            }
        }
        "duration" => {
            let id = raw
                .get("id")
                .and_then(Value::as_str)
                .unwrap_or_default()
                .to_owned();
            let duration = raw.get("durationMs").and_then(Value::as_f64).unwrap_or(0.0);
            let Some(anime) = raw
                .get("anime")
                .cloned()
                .and_then(|anime| serde_json::from_value::<AnimeEpisode>(anime).ok())
            else {
                return;
            };
            // Off the report queue: the lookup goes over the network.
            let app = app.clone();
            tauri::async_runtime::spawn(async move {
                match crate::aniskip::chapters(&db, &anime, duration).await {
                    Ok(chapters) if !chapters.is_empty() => {
                        let _ = update_player(
                            &app,
                            &json!({ "type": "chapters", "id": id, "items": chapters }),
                        );
                    }
                    Ok(_) => {}
                    Err(error) => tracing::warn!(%error, "AniSkip lookup failed"),
                }
            });
        }
        _ => {}
    }
}

/// The language subtitles are fetched in first: the subtitle language, else the app's.
pub async fn subtitle_language(db: &raygun_db::Db) -> String {
    let settings = crate::settings::load(db).await.unwrap_or_default();
    if !settings.subtitle_language.is_empty() {
        settings.subtitle_language
    } else if settings.language != raygun_model::SYSTEM_LANGUAGE {
        settings.language
    } else {
        String::new()
    }
}

/// Marks something watched or not, in the progress store and on its library episode.
pub async fn mark(db: &raygun_db::Db, entry: &VideoEntry, watched: bool) -> Result<(), String> {
    let now = now_ms();
    store::mark(db, entry, watched, now)
        .await
        .map_err(|error| error.to_string())?;
    crate::video_library::mirror_mark(db, &entry.key, watched, now).await
}

/// Subtitle files through the stream proxy, which serves an empty track for one that can't be
/// fetched (the player would give up on the whole video) and lets release builds load plain-HTTP
/// ones. The proxied URL has no extension, so the format is guessed from the original first.
fn proxied_subtitles(app: &AppHandle, subtitles: &[VideoSubtitle]) -> Vec<VideoSubtitle> {
    let Some(links) = app.try_state::<ImageLinks>() else {
        return subtitles.to_vec();
    };
    subtitles
        .iter()
        .filter(|subtitle| {
            subtitle.url.starts_with("https://") || subtitle.url.starts_with("http://")
        })
        .map(|subtitle| VideoSubtitle {
            url: links.subtitle(&subtitle.url, &Default::default()),
            format: subtitle
                .format
                .clone()
                .or_else(|| subtitle_format(&subtitle.url)),
            ..subtitle.clone()
        })
        .collect()
}

/// `srt`, `vtt`, `ass`... from a subtitle URL's file extension, when it has a known one.
fn subtitle_format(url: &str) -> Option<String> {
    let path = url.split(['?', '#']).next().unwrap_or_default();
    let name = path.rsplit('/').next().unwrap_or_default();
    let (_, extension) = name.rsplit_once('.')?;
    let extension = extension.to_ascii_lowercase();
    ["srt", "vtt", "webvtt", "ass", "ssa", "ttml", "dfxp"]
        .contains(&extension.as_str())
        .then_some(extension)
}

#[cfg(target_os = "android")]
pub fn open_player(app: &AppHandle, payload: &serde_json::Value) -> Result<(), String> {
    let player = app
        .try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .ok_or_else(|| "The video player isn't available".to_owned())?;
    player.video_open(payload)
}

#[cfg(not(target_os = "android"))]
pub fn open_player(app: &AppHandle, payload: &serde_json::Value) -> Result<(), String> {
    crate::video_desktop::open(app, payload)
}

/// Sends an update to the open player (M44); nothing happens when none is open.
#[cfg(target_os = "android")]
pub fn update_player(app: &AppHandle, update: &serde_json::Value) -> Result<(), String> {
    let player = app
        .try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .ok_or_else(|| "The video player isn't available".to_owned())?;
    player.video_update(update)
}

#[cfg(not(target_os = "android"))]
pub fn update_player(app: &AppHandle, update: &serde_json::Value) -> Result<(), String> {
    crate::video_desktop::update(app, update)
}

/// Where the player's reports go: from Android's player over JNI, and from the webview's player
/// on desktop (`video_player_report`, M46.4).
pub struct VideoReports(tokio::sync::mpsc::UnboundedSender<String>);

impl VideoReports {
    pub fn send(&self, json: String) {
        let _ = self.0.send(json);
    }
}

/// The player's reports become saved progress and `WatchChangedEvent`s.
pub fn install(app: &AppHandle) {
    // Reports are handled one at a time, in order: two at once (the end of a video sends several)
    // would race on the same rows.
    let (sender, mut reports) = tokio::sync::mpsc::unbounded_channel::<String>();
    app.manage(VideoReports(sender.clone()));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(json) = reports.recv().await {
            handle_report(&app, &json).await;
        }
    });
    tauri_plugin_raygun_audio::set_video_handler(move |json| {
        let _ = sender.send(json);
    });
}

async fn handle_report(app: &AppHandle, json: &str) {
    let Ok(raw) = serde_json::from_str::<serde_json::Value>(json) else {
        tracing::warn!(%json, "unreadable video player report");
        return;
    };
    // The player's own requests are handled here; the rest (next episode, another source...)
    // are the session's business.
    if raw.get("state").and_then(serde_json::Value::as_str) == Some("request") {
        let action = raw
            .get("action")
            .and_then(Value::as_str)
            .unwrap_or_default();
        match action {
            "state" | "prefs" | "brightness" | "duration" => {
                player_request(app, action, &raw).await;
            }
            _ => app.state::<VideoSessions>().on_report(&raw).await,
        }
        return;
    }
    let Ok(report) = serde_json::from_value::<PlayerReport>(raw.clone()) else {
        tracing::warn!(%raw, "unreadable video player report");
        return;
    };
    record(app, report).await;
    app.state::<VideoSessions>().on_report(&raw).await;
}

async fn record(app: &AppHandle, report: PlayerReport) {
    if let Some(error) = &report.error {
        tracing::warn!(key = %report.id, %error, "video playback failed");
    }
    let duration = report.duration_ms.filter(|duration| *duration > 0.0);
    let db = app.state::<AppDb>().0.clone();
    let watched_share = player_prefs(&db).await.watched_share;
    let watched = report.state == "ended"
        || duration.is_some_and(|duration| report.position_ms >= duration * watched_share);
    // A failed load reports position 0; don't let it wipe saved progress.
    if report.state == "error" && report.position_ms <= 0.0 {
        return;
    }
    let now = now_ms();
    let position_ms = report.position_ms as i64;
    let duration_ms = duration.map(|duration| duration as i64);
    if let Err(error) =
        store::set_progress(&db, &report.id, position_ms, duration_ms, watched, now).await
    {
        tracing::warn!(%error, "could not save watching progress");
    }
    if let Err(error) = crate::video_library::mirror_progress(
        &db,
        &report.id,
        position_ms,
        duration_ms,
        watched,
        now,
    )
    .await
    {
        tracing::warn!(%error, "could not save the episode's progress");
    }
    if watched {
        app.state::<VideoSessions>().on_watched(&report.id).await;
    }
    if matches!(report.state.as_str(), "closed" | "ended" | "switched") || watched {
        let _ = WatchChangedEvent(report.id).emit(app);
    }
}

pub async fn continue_watching(app: &AppHandle) -> Result<Vec<ContinueWatching>, String> {
    let db = app.state::<AppDb>().0.clone();
    let rows = store::continue_watching(&db, 30)
        .await
        .map_err(|error| error.to_string())?;
    Ok(to_view(app, rows))
}

/// Everything watched or started, most recent first.
pub async fn history(app: &AppHandle) -> Result<Vec<ContinueWatching>, String> {
    let db = app.state::<AppDb>().0.clone();
    let rows = store::history(&db, 200)
        .await
        .map_err(|error| error.to_string())?;
    Ok(to_view(app, rows))
}

fn to_view(app: &AppHandle, rows: Vec<store::VideoProgress>) -> Vec<ContinueWatching> {
    let images = app.state::<raygun_net::ImageLinks>();
    rows.into_iter()
        .map(|row| ContinueWatching {
            key: row.entry.key,
            title: row.entry.title,
            subtitle: row.entry.subtitle,
            // Posters seen again and again: through the lasting cache (M46.2).
            poster: row.entry.poster.map(|poster| {
                if images.is_remote(&poster) {
                    images.cover(&poster, &Default::default())
                } else {
                    poster
                }
            }),
            position_ms: row.position_ms as f64,
            duration_ms: row.duration_ms.map(|ms| ms as f64),
            updated_at: row.updated_at as f64,
            watched: row.watched_at.is_some(),
        })
        .collect()
}

/// Plays a "continue watching" entry again: an episode reference through a session (fresh links,
/// the last source first), else the request it was first played with.
pub async fn resume(app: &AppHandle, key: &str) -> Result<(), String> {
    let db = app.state::<AppDb>().0.clone();
    let saved = store::progress(&db, key)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "This was removed".to_owned())?;
    if let Ok(episode) = serde_json::from_str::<EpisodeRef>(&saved.entry.request) {
        return app
            .state::<VideoSessions>()
            .start(episode.origin, Some(episode.episode))
            .await;
    }
    let request: VideoRequest = serde_json::from_str(&saved.entry.request)
        .map_err(|_| "Open this from its page to choose a stream".to_owned())?;
    play(app, request).await
}

pub async fn remove(app: &AppHandle, key: &str) -> Result<(), String> {
    let db = app.state::<AppDb>().0.clone();
    store::remove(&db, key)
        .await
        .map_err(|error| error.to_string())
}

/// A video link as a request: its key is the address, its title the file name.
pub fn link_request(url: &str, title: Option<&str>, subtitle_url: Option<&str>) -> VideoRequest {
    let url = url.trim().to_owned();
    let title = title
        .map(str::trim)
        .filter(|title| !title.is_empty())
        .map(str::to_owned)
        .unwrap_or_else(|| {
            url.split(['?', '#'])
                .next()
                .and_then(|path| path.trim_end_matches('/').rsplit('/').next())
                .filter(|name| !name.is_empty())
                .unwrap_or("Video")
                .to_owned()
        });
    let lower = url.to_lowercase();
    let format = if lower.contains(".m3u8") {
        Some("hls".to_owned())
    } else if lower.contains(".mpd") {
        Some("dash".to_owned())
    } else {
        None
    };
    VideoRequest {
        group: None,
        chapters: Vec::new(),
        anime: None,
        key: format!("url:{url}"),
        title,
        subtitle: None,
        poster: None,
        format,
        headers: Default::default(),
        subtitles: subtitle_url
            .map(str::trim)
            .filter(|subtitle| !subtitle.is_empty())
            .map(|subtitle| raygun_model::VideoSubtitle {
                url: subtitle.to_owned(),
                lang: None,
                label: None,
                format: None,
            })
            .into_iter()
            .collect(),
        url,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_requests() {
        let hls = link_request(" https://cdn.example/show/master.m3u8?t=1 ", None, None);
        assert_eq!(hls.key, "url:https://cdn.example/show/master.m3u8?t=1");
        assert_eq!(hls.title, "master.m3u8");
        assert_eq!(hls.format.as_deref(), Some("hls"));
        assert!(hls.subtitles.is_empty());

        let mp4 = link_request(
            "https://cdn.example/film.mp4",
            Some("A film"),
            Some("https://cdn.example/film.srt"),
        );
        assert_eq!(mp4.title, "A film");
        assert_eq!(mp4.format, None);
        assert_eq!(mp4.subtitles[0].url, "https://cdn.example/film.srt");
    }

    #[test]
    fn subtitle_formats_from_urls() {
        let format = |url| subtitle_format(url);
        assert_eq!(
            format("https://cdn.example/a/film.SRT?t=1").as_deref(),
            Some("srt")
        );
        assert_eq!(
            format("https://cdn.example/subs.vtt#x").as_deref(),
            Some("vtt")
        );
        assert_eq!(format("https://subs.example/en/download/file/123"), None);
        assert_eq!(format("https://cdn.example.com/"), None);
    }

    fn object(value: Value) -> Map<String, Value> {
        match value {
            Value::Object(map) => map,
            _ => panic!("not an object"),
        }
    }

    #[test]
    fn a_titles_memory_takes_known_keys_and_forgets_nulls() {
        let mut saved =
            object(json!({ "speed": 125, "aspect": "crop", "audio": { "language": "ja" } }));
        merge_state(
            &mut saved,
            &object(json!({
                "speed": 150,
                "aspect": null,
                "subtitleDelayMs": -300,
                "volume": 1,
                "subtitleEdits": { "ep1|English|en": [{ "at": 1000, "startMs": 1200, "endMs": 3000, "text": "Hi" }] },
            })),
        );
        assert_eq!(
            Value::Object(saved),
            json!({
                "speed": 150,
                "audio": { "language": "ja" },
                "subtitleDelayMs": -300,
                "subtitleEdits": { "ep1|English|en": [{ "at": 1000, "startMs": 1200, "endMs": 3000, "text": "Hi" }] },
            })
        );
    }

    #[test]
    fn player_changes_merge_into_the_settings() {
        let mut saved = serde_json::to_value(VideoPlayerPrefs::default()).unwrap();
        merge_prefs(
            &mut saved,
            &object(json!({
                "defaultSpeed": 150,
                "showRemaining": true,
                "subtitleStyle": { "size": 120, "unknown": 1 },
                "notASetting": true
            })),
        );
        let prefs: VideoPlayerPrefs = serde_json::from_value(saved).unwrap();
        assert_eq!(prefs.default_speed, 150);
        assert!(prefs.show_remaining);
        assert_eq!(prefs.subtitle_style.size, 120);
        assert_eq!(
            prefs.subtitle_style.text_color,
            raygun_model::SubtitleStyle::default().text_color,
            "the rest of the style stays"
        );
        assert!(prefs.validate().is_ok());
    }
}
