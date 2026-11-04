//! Keeps Raygun's player going when it leaves the screen (Android).
//!
//! The native engine's songs play in the `raygun-audio` plugin's media session service, which
//! Android runs in the foreground while it plays. Songs YouTube plays live in the webview's
//! embedded player instead, and between songs nothing plays while the next stream is found: with
//! nothing in that session, Android would freeze Raygun in the background, and the webview pauses
//! its media when the screen goes off. So the plugin is told what Rust's player is doing, and its
//! session shows that (the notification, the lock screen, headsets) whenever the native engine
//! isn't the one playing; while it shows that, the webview is kept awake. Commands from the
//! session come back as controls, like the native engine's.
//!
//! Which engine plays is read from the webview's engine commands (`EngineCommandEvent`, which only
//! the webview engine gets on Android); the player's state from `PlaybackStateEvent`.

use std::sync::Mutex;
use std::time::Instant;

use raygun_model::{EngineCommand, PlaybackState, PlaybackStatus, QueueItem};
use serde::Serialize;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::events::{EngineCommandEvent, PlaybackStateEvent, QueueEvent};
use crate::playback::PlaybackService;

/// A position this far from where the last report says it should be is a seek, ms.
const SEEK_SLACK_MS: f64 = 1500.0;

/// What the plugin's session shows for Rust's player.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MediaSnapshot {
    pub item_id: String,
    pub title: String,
    pub artist: Option<String>,
    pub album: Option<String>,
    pub artwork_url: Option<String>,
    /// `loading`, `buffering`, `playing`, `paused`, `ended` or `error`.
    pub status: &'static str,
    pub position_ms: f64,
    pub duration_ms: Option<f64>,
    pub rate: f64,
    /// The webview's engine holds the item (YouTube): it must stay awake.
    pub web: bool,
}

fn status_name(status: PlaybackStatus) -> Option<&'static str> {
    Some(match status {
        PlaybackStatus::Idle => return None,
        PlaybackStatus::Loading => "loading",
        PlaybackStatus::Buffering => "buffering",
        PlaybackStatus::Playing => "playing",
        PlaybackStatus::Paused => "paused",
        PlaybackStatus::Ended => "ended",
        PlaybackStatus::Error => "error",
    })
}

/// The snapshot for `state`, or `None` when nothing is loaded.
pub fn snapshot(
    state: &PlaybackState,
    item: Option<&QueueItem>,
    web: bool,
) -> Option<MediaSnapshot> {
    let status = status_name(state.status)?;
    let item = item?;
    Some(MediaSnapshot {
        item_id: item.id.clone(),
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        artwork_url: item.artwork_url.clone(),
        status,
        position_ms: state.position_ms.max(0.0),
        duration_ms: state.duration_ms,
        rate: state.rate,
        web,
    })
}

/// Whether `next` says something the plugin can't work out from `sent`, sent `elapsed_ms` ago:
/// a new item, status, length, speed or engine, or a position that isn't where playing on from
/// `sent` would be (a seek). Progress alone isn't sent: the session moves the position itself.
pub fn worth_sending(
    sent: Option<&MediaSnapshot>,
    next: Option<&MediaSnapshot>,
    elapsed_ms: f64,
) -> bool {
    let (sent, next) = match (sent, next) {
        (None, None) => return false,
        (Some(sent), Some(next)) => (sent, next),
        _ => return true,
    };
    if sent.item_id != next.item_id
        || sent.status != next.status
        || sent.web != next.web
        || sent.title != next.title
        || sent.artist != next.artist
        || sent.artwork_url != next.artwork_url
        || sent.duration_ms.map(f64::round) != next.duration_ms.map(f64::round)
        || (sent.rate - next.rate).abs() > f64::EPSILON
    {
        return true;
    }
    let expected = if sent.status == "playing" {
        sent.position_ms + elapsed_ms * sent.rate
    } else {
        sent.position_ms
    };
    (next.position_ms - expected).abs() > SEEK_SLACK_MS
}

#[derive(Default)]
struct Latest {
    state: Option<PlaybackState>,
    /// The webview's engine holds the current load.
    web: bool,
    /// The queue changed since the current item was looked up.
    queue_changed: bool,
}

/// Tells the plugin what Rust's player does, as it changes.
pub struct BackgroundPlayback {
    latest: Mutex<Latest>,
    wake: Notify,
}

impl BackgroundPlayback {
    /// Starts following the player (Android only: elsewhere the webview's own media session and
    /// the window being open cover this).
    pub fn start(app: &AppHandle) {
        if !cfg!(target_os = "android") {
            return;
        }
        app.manage(Self {
            latest: Mutex::new(Latest::default()),
            wake: Notify::new(),
        });
        let handle = app.clone();
        EngineCommandEvent::listen(app, move |event| {
            let web = match event.payload.0 {
                EngineCommand::LoadYouTube { .. } => true,
                EngineCommand::Load { .. }
                | EngineCommand::Stop
                | EngineCommand::FadeOut { .. } => false,
                _ => return,
            };
            if let Some(background) = handle.try_state::<BackgroundPlayback>() {
                background.latest.lock().expect("latest poisoned").web = web;
                background.wake.notify_one();
            }
        });
        let handle = app.clone();
        PlaybackStateEvent::listen(app, move |event| {
            if let Some(background) = handle.try_state::<BackgroundPlayback>() {
                background.latest.lock().expect("latest poisoned").state = Some(event.payload.0);
                background.wake.notify_one();
            }
        });
        let handle = app.clone();
        QueueEvent::listen(app, move |_| {
            if let Some(background) = handle.try_state::<BackgroundPlayback>() {
                background
                    .latest
                    .lock()
                    .expect("latest poisoned")
                    .queue_changed = true;
                background.wake.notify_one();
            }
        });
        let app = app.clone();
        tauri::async_runtime::spawn(async move { follow(app).await });
    }
}

async fn follow(app: AppHandle) {
    let mut sent: Option<(MediaSnapshot, Instant)> = None;
    let mut item: Option<QueueItem> = None;
    loop {
        let background = app.state::<BackgroundPlayback>();
        background.wake.notified().await;
        let (state, web, queue_changed) = {
            let mut latest = background.latest.lock().expect("latest poisoned");
            let changed = std::mem::take(&mut latest.queue_changed);
            (latest.state.clone(), latest.web, changed)
        };
        let Some(state) = state else {
            continue;
        };
        let current = state.current_item_id.as_deref();
        if queue_changed || item.as_ref().map(|item| item.id.as_str()) != current {
            item = current.and_then(|id| {
                app.state::<PlaybackService>()
                    .queue()
                    .items
                    .into_iter()
                    .find(|item| item.id == id)
            });
        }
        let next = snapshot(&state, item.as_ref(), web);
        let elapsed = sent
            .as_ref()
            .map(|(_, at)| at.elapsed().as_secs_f64() * 1000.0)
            .unwrap_or(0.0);
        if !worth_sending(
            sent.as_ref().map(|(snapshot, _)| snapshot),
            next.as_ref(),
            elapsed,
        ) {
            continue;
        }
        platform::send(&app, next.as_ref()).await;
        sent = next.map(|snapshot| (snapshot, Instant::now()));
    }
}

#[cfg(target_os = "android")]
mod platform {
    use tauri::{AppHandle, Manager};

    use super::MediaSnapshot;

    pub async fn send(app: &AppHandle, snapshot: Option<&MediaSnapshot>) {
        let value = match snapshot {
            Some(snapshot) => serde_json::json!({ "state": snapshot }),
            None => serde_json::json!({ "state": null }),
        };
        let app = app.clone();
        let sent = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .media_state(&value)
        })
        .await;
        if let Ok(Err(error)) = sent {
            tracing::warn!(%error, "the media session didn't take the player's state");
        }
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use tauri::AppHandle;

    use super::MediaSnapshot;

    pub async fn send(_app: &AppHandle, _snapshot: Option<&MediaSnapshot>) {}
}

#[cfg(test)]
mod tests {
    use raygun_model::{MediaKind, MediaSource, RepeatMode};

    use super::*;

    fn state(status: PlaybackStatus, position_ms: f64) -> PlaybackState {
        PlaybackState {
            status,
            current_item_id: Some("a".into()),
            position_ms,
            duration_ms: Some(200_000.0),
            volume: 1.0,
            repeat: RepeatMode::Off,
            shuffle: false,
            error: None,
            source_label: None,
            rate: 1.0,
            sleep_at: None,
            stop_after_current: false,
        }
    }

    fn item() -> QueueItem {
        QueueItem {
            explicit: false,
            id: "a".into(),
            title: "Song".into(),
            artist: Some("Artist".into()),
            album: None,
            duration_ms: Some(200_000),
            artwork_url: None,
            media_kind: MediaKind::Audio,
            source: MediaSource::YouTube {
                video_id: "x".into(),
            },
        }
    }

    #[test]
    fn nothing_loaded_is_no_snapshot() {
        let idle = state(PlaybackStatus::Idle, 0.0);
        assert_eq!(snapshot(&idle, Some(&item()), false), None);
        let playing = state(PlaybackStatus::Playing, 0.0);
        assert_eq!(snapshot(&playing, None, false), None);
        let shown = snapshot(&playing, Some(&item()), true).unwrap();
        assert_eq!(shown.status, "playing");
        assert_eq!(shown.title, "Song");
        assert!(shown.web);
    }

    #[test]
    fn progress_alone_is_not_sent_but_seeks_are() {
        let item = item();
        let first = snapshot(&state(PlaybackStatus::Playing, 10_000.0), Some(&item), true);
        // Two seconds later, two seconds on: the session worked that out itself.
        let later = snapshot(&state(PlaybackStatus::Playing, 12_000.0), Some(&item), true);
        assert!(!worth_sending(first.as_ref(), later.as_ref(), 2_000.0));
        // A jump to 1:30 is a seek.
        let sought = snapshot(&state(PlaybackStatus::Playing, 90_000.0), Some(&item), true);
        assert!(worth_sending(first.as_ref(), sought.as_ref(), 2_000.0));
        // Paused, the position stays put.
        let paused = snapshot(&state(PlaybackStatus::Paused, 10_000.0), Some(&item), true);
        let still = snapshot(&state(PlaybackStatus::Paused, 10_200.0), Some(&item), true);
        assert!(!worth_sending(paused.as_ref(), still.as_ref(), 5_000.0));
    }

    #[test]
    fn status_engine_and_item_changes_are_sent() {
        let item = item();
        let playing = snapshot(&state(PlaybackStatus::Playing, 0.0), Some(&item), false);
        let paused = snapshot(&state(PlaybackStatus::Paused, 0.0), Some(&item), false);
        assert!(worth_sending(playing.as_ref(), paused.as_ref(), 0.0));
        let web = snapshot(&state(PlaybackStatus::Playing, 0.0), Some(&item), true);
        assert!(worth_sending(playing.as_ref(), web.as_ref(), 0.0));
        assert!(worth_sending(playing.as_ref(), None, 0.0));
        assert!(worth_sending(None, playing.as_ref(), 0.0));
        assert!(!worth_sending(None, None, 0.0));
    }
}
