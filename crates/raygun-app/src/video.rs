//! Watching video (M38): the native player on Android (Media3, in the audio plugin) and the
//! progress it reports, for resuming and "continue watching". Elsewhere video isn't available yet.

use raygun_db::video::{self as store, VideoEntry};
use raygun_model::{ContinueWatching, PlaybackControl, VideoRequest};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::now_ms;
use crate::events::WatchChangedEvent;
use crate::playback::PlaybackService;
use crate::startup::AppDb;

/// Past this share a video counts as watched.
const WATCHED_SHARE: f64 = 0.9;

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
pub async fn play(app: &AppHandle, request: VideoRequest) -> Result<(), String> {
    if !request.url.starts_with("https://") && !request.url.starts_with("http://") {
        return Err("Only web addresses can be played".into());
    }
    let db = app.state::<AppDb>().0.clone();
    let saved = store::progress(&db, &request.key)
        .await
        .map_err(|error| error.to_string())?;
    let start_ms = saved
        .filter(|progress| progress.watched_at.is_none())
        .filter(|progress| {
            progress
                .duration_ms
                .is_none_or(|duration| (progress.position_ms as f64) < duration as f64 * 0.95)
        })
        .map_or(0, |progress| progress.position_ms);
    let entry = VideoEntry {
        key: request.key.clone(),
        title: request.title.clone(),
        subtitle: request.subtitle.clone(),
        poster: request.poster.clone(),
        request: serde_json::to_string(&request).map_err(|error| error.to_string())?,
    };
    store::started(&db, &entry, now_ms())
        .await
        .map_err(|error| error.to_string())?;
    // Music and video don't play over each other.
    app.state::<PlaybackService>()
        .control(PlaybackControl::Pause);
    let payload = serde_json::json!({
        "id": request.key,
        "url": request.url,
        "title": request.title,
        "subtitle": request.subtitle,
        "headers": request.headers,
        "format": request.format,
        "subtitles": request.subtitles,
        "startMs": start_ms,
    });
    open_player(app, &payload)
}

#[cfg(target_os = "android")]
fn open_player(app: &AppHandle, payload: &serde_json::Value) -> Result<(), String> {
    let player = app
        .try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .ok_or_else(|| "The video player isn't available".to_owned())?;
    player.video_open(payload)
}

#[cfg(not(target_os = "android"))]
fn open_player(_app: &AppHandle, _payload: &serde_json::Value) -> Result<(), String> {
    Err("Video playback is only available on Android for now".into())
}

/// The player's reports become saved progress and `WatchChangedEvent`s.
pub fn install(app: &AppHandle) {
    let app = app.clone();
    tauri_plugin_raygun_audio::set_video_handler(move |json| {
        let Ok(report) = serde_json::from_str::<PlayerReport>(&json) else {
            tracing::warn!(%json, "unreadable video player report");
            return;
        };
        let app = app.clone();
        tauri::async_runtime::spawn(async move { record(&app, report).await });
    });
}

async fn record(app: &AppHandle, report: PlayerReport) {
    if let Some(error) = &report.error {
        tracing::warn!(key = %report.id, %error, "video playback failed");
    }
    let duration = report.duration_ms.filter(|duration| *duration > 0.0);
    let watched = report.state == "ended"
        || duration.is_some_and(|duration| report.position_ms >= duration * WATCHED_SHARE);
    let db = app.state::<AppDb>().0.clone();
    // A failed load reports position 0; don't let it wipe saved progress.
    if report.state == "error" && report.position_ms <= 0.0 {
        return;
    }
    if let Err(error) = store::set_progress(
        &db,
        &report.id,
        report.position_ms as i64,
        duration.map(|duration| duration as i64),
        watched,
        now_ms(),
    )
    .await
    {
        tracing::warn!(%error, "could not save watching progress");
    }
    if matches!(report.state.as_str(), "closed" | "ended") || watched {
        let _ = WatchChangedEvent(report.id).emit(app);
    }
}

pub async fn continue_watching(app: &AppHandle) -> Result<Vec<ContinueWatching>, String> {
    let db = app.state::<AppDb>().0.clone();
    let rows = store::continue_watching(&db, 30)
        .await
        .map_err(|error| error.to_string())?;
    Ok(rows
        .into_iter()
        .map(|row| ContinueWatching {
            key: row.entry.key,
            title: row.entry.title,
            subtitle: row.entry.subtitle,
            poster: row.entry.poster,
            position_ms: row.position_ms as f64,
            duration_ms: row.duration_ms.map(|ms| ms as f64),
            updated_at: row.updated_at as f64,
        })
        .collect())
}

/// Plays a "continue watching" entry again with the request it was first played with.
pub async fn resume(app: &AppHandle, key: &str) -> Result<(), String> {
    let db = app.state::<AppDb>().0.clone();
    let saved = store::progress(&db, key)
        .await
        .map_err(|error| error.to_string())?
        .ok_or_else(|| "This was removed".to_owned())?;
    let request: VideoRequest =
        serde_json::from_str(&saved.entry.request).map_err(|error| error.to_string())?;
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
}
