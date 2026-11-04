//! Song lengths (M46.26). Several sources send no length (Apple's charts, ListenBrainz, some tag
//! lists and plugins), and lists built from them kept the gap. Raygun now learns lengths from
//! playback, from every list a source returns with lengths, and from lookups for the rest, and
//! keeps them by track key in `local_track_duration`. Song lists ask with `lookup` and get what's
//! known at once; the rest arrive as `TrackDurationsEvent`s.

use std::collections::{HashSet, VecDeque};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_db::track_durations as store;
use raygun_model::{OnlineTrack, PlaybackState, QueueItem, TrackDuration, TrackQuery};
use raygun_resolver::{MIN_SCORE, track_key};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::events::TrackDurationsEvent;
use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// Songs waiting for a lookup, at most.
const QUEUE_LIMIT: usize = 200;
/// Between lookups, to stay polite to the sources.
const LOOKUP_GAP: Duration = Duration::from_millis(500);
/// A lookup that found nothing isn't tried again for a day.
const MISS_RETRY_MS: i64 = 24 * 60 * 60 * 1000;
/// Playback corrects a stored length only when it's this close (a music video's intro isn't the
/// song's length).
const PLAYBACK_TOLERANCE_MS: i64 = 15_000;
/// Playback's length counts once the song has played this long (streams settle their length).
const PLAYED_ENOUGH_MS: f64 = 5_000.0;

pub struct DurationService {
    app: AppHandle,
    queue: Mutex<VecDeque<(String, TrackQuery)>>,
    queued: Mutex<HashSet<String>>,
    wake: Notify,
    /// The queue item whose length playback already reported.
    reported: Mutex<Option<String>>,
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

impl DurationService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            queue: Mutex::new(VecDeque::new()),
            queued: Mutex::new(HashSet::new()),
            wake: Notify::new(),
            reported: Mutex::new(None),
        })
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    pub fn app(&self) -> &AppHandle {
        &self.app
    }

    /// The lengths known for `tracks`, in their order (`track_key` names each). Songs without one
    /// are queued for a lookup, unless one found nothing lately.
    pub async fn lookup(&self, tracks: Vec<TrackQuery>) -> Result<Vec<TrackDuration>, String> {
        let keys: Vec<String> = tracks.iter().map(track_key).collect();
        let stored: std::collections::HashMap<String, store::Stored> =
            store::get_many(self.db(), &keys)
                .await
                .map_err(|error| error.to_string())?
                .into_iter()
                .collect();
        let now = now_ms();
        let mut answers = Vec::with_capacity(tracks.len());
        let mut queued_any = false;
        for (track, key) in tracks.into_iter().zip(keys) {
            let known = stored.get(&key);
            let duration_ms = known
                .and_then(|row| row.duration_ms)
                .and_then(|ms| u32::try_from(ms).ok());
            let tried_lately = known.is_some_and(|row| {
                row.duration_ms.is_none() && now - row.updated_at < MISS_RETRY_MS
            });
            if duration_ms.is_none() && !tried_lately && !track.title.trim().is_empty() {
                queued_any |= self.enqueue(key.clone(), track);
            }
            answers.push(TrackDuration {
                track_key: key,
                duration_ms,
            });
        }
        if queued_any {
            self.wake.notify_one();
        }
        Ok(answers)
    }

    fn enqueue(&self, key: String, track: TrackQuery) -> bool {
        let mut queued = self.queued.lock().unwrap_or_else(|p| p.into_inner());
        let mut queue = self.queue.lock().unwrap_or_else(|p| p.into_inner());
        if queue.len() >= QUEUE_LIMIT || !queued.insert(key.clone()) {
            return false;
        }
        queue.push_back((key, track));
        true
    }

    /// Keeps the lengths a source sent with a list of songs (search, charts, albums). Known
    /// lengths stay; only gaps and remembered misses are filled.
    pub fn learn_tracks(self: &Arc<Self>, tracks: &[OnlineTrack]) {
        let lengths: Vec<(String, i64)> = tracks
            .iter()
            .filter_map(|track| {
                let ms = track.duration_ms.filter(|ms| *ms > 0)?;
                let key = track_key(&TrackQuery {
                    title: track.title.clone(),
                    artist: track.artist.clone(),
                    album: track.album.clone(),
                    duration_ms: None,
                });
                Some((key, i64::from(ms)))
            })
            .collect();
        if lengths.is_empty() {
            return;
        }
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            service.store(lengths, "source", false).await;
        });
    }

    /// Follows playback: once a song has played a few seconds, its stream's length is kept (it
    /// corrects a stored one only when close). Called with every state change.
    pub fn on_playback(self: &Arc<Self>, state: &PlaybackState, item: Option<&QueueItem>) {
        let (Some(item), Some(duration)) = (item, state.duration_ms) else {
            return;
        };
        if duration <= 0.0 || state.position_ms < PLAYED_ENOUGH_MS {
            return;
        }
        {
            let mut reported = self.reported.lock().unwrap_or_else(|p| p.into_inner());
            if reported.as_deref() == Some(item.id.as_str()) {
                return;
            }
            *reported = Some(item.id.clone());
        }
        let query = TrackQuery::for_item(item);
        if query.title.trim().is_empty() {
            return;
        }
        let ms = duration.round() as i64;
        // A song that says its own length keeps it, unless the stream is close to it.
        if let Some(said) = item.duration_ms
            && (i64::from(said) - ms).abs() > PLAYBACK_TOLERANCE_MS
        {
            return;
        }
        let key = track_key(&query);
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            // It played, so a source has it after all (M46.31).
            let _ = raygun_db::collections::clear_unavailable(service.db(), &key).await;
            service.store(vec![(key, ms)], "playback", true).await;
        });
    }

    async fn store(&self, lengths: Vec<(String, i64)>, source: &str, replace: bool) {
        if let Err(error) = store::put_many(self.db(), &lengths, source, replace, now_ms()).await {
            tracing::warn!(%error, "couldn't keep song lengths");
            return;
        }
        for (key, ms) in &lengths {
            if let Err(error) = store::fill_saved(self.db(), key, *ms).await {
                tracing::debug!(%error, "couldn't fill saved songs' length");
            }
        }
        self.announce(
            lengths
                .into_iter()
                .filter_map(|(key, ms)| {
                    Some(TrackDuration {
                        track_key: key,
                        duration_ms: Some(u32::try_from(ms).ok()?),
                    })
                })
                .collect(),
        );
    }

    fn announce(&self, durations: Vec<TrackDuration>) {
        if durations.is_empty() {
            return;
        }
        if let Err(error) = TrackDurationsEvent(durations).emit(&self.app) {
            tracing::debug!(%error, "couldn't announce song lengths");
        }
    }

    /// Works through queued lookups one at a time, in the background.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            loop {
                let next = service
                    .queue
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .pop_front();
                let Some((key, track)) = next else {
                    service.wake.notified().await;
                    continue;
                };
                service.look_up(&key, &track).await;
                service
                    .queued
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .remove(&key);
                crate::device_state::rest(LOOKUP_GAP).await;
            }
        });
    }

    async fn look_up(&self, key: &str, track: &TrackQuery) {
        let Some(extensions) = self.app.try_state::<Arc<ExtensionService>>() else {
            return;
        };
        let found = crate::import::find_song(&extensions, track).await;
        match found {
            Some((points, _, song)) if points >= MIN_SCORE => {
                if let Some(ms) = song.duration_ms.filter(|ms| *ms > 0) {
                    self.store(vec![(key.to_owned(), i64::from(ms))], "lookup", false)
                        .await;
                    return;
                }
                let _ = store::put_miss(self.db(), key, now_ms()).await;
            }
            _ => {
                let _ = store::put_miss(self.db(), key, now_ms()).await;
            }
        }
    }
}
