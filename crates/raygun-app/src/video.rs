//! Watching video (M38): the native player on Android (Media3, in the audio plugin) and the
//! progress it reports, for resuming and "continue watching". Elsewhere video isn't available yet.
//! Progress also goes to the library's episodes, and finished downloads play instead of the
//! stream (M43). Episodes of a title play through a session (M44, `video_session`), which loads
//! videos into the open player and answers its requests.

use raygun_db::video::{self as store, VideoEntry};
use raygun_model::{ContinueWatching, PlaybackControl, VideoRequest, VideoSubtitle};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::now_ms;
use crate::events::WatchChangedEvent;
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
}

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
        poster: request.poster.clone(),
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
    // Music and video don't play over each other.
    app.state::<PlaybackService>()
        .control(PlaybackControl::Pause);
    Ok(serde_json::json!({
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
    app.state::<PlaybackService>()
        .control(PlaybackControl::Pause);
    serde_json::json!({
        "session": true,
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
pub fn open_player(_app: &AppHandle, _payload: &serde_json::Value) -> Result<(), String> {
    Err("Video playback is only available on Android for now".into())
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
pub fn update_player(_app: &AppHandle, _update: &serde_json::Value) -> Result<(), String> {
    Err("Video playback is only available on Android for now".into())
}

/// The player's reports become saved progress and `WatchChangedEvent`s.
pub fn install(app: &AppHandle) {
    // Reports are handled one at a time, in order: two at once (the end of a video sends several)
    // would race on the same rows.
    let (sender, mut reports) = tokio::sync::mpsc::unbounded_channel::<String>();
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
    // Requests (next episode, another source...) are the session's business.
    if raw.get("state").and_then(serde_json::Value::as_str) == Some("request") {
        app.state::<VideoSessions>().on_report(&raw).await;
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
    Ok(to_view(rows))
}

/// Everything watched or started, most recent first.
pub async fn history(app: &AppHandle) -> Result<Vec<ContinueWatching>, String> {
    let db = app.state::<AppDb>().0.clone();
    let rows = store::history(&db, 200)
        .await
        .map_err(|error| error.to_string())?;
    Ok(to_view(rows))
}

fn to_view(rows: Vec<store::VideoProgress>) -> Vec<ContinueWatching> {
    rows.into_iter()
        .map(|row| ContinueWatching {
            key: row.entry.key,
            title: row.entry.title,
            subtitle: row.entry.subtitle,
            poster: row.entry.poster,
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
}
