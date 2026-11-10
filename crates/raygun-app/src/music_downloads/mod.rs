//! Songs saved into the app to play offline (M46.10). A queue in `local_music_download`, worked
//! one song at a time: a source's own file downloads with Range resume to
//! `downloads/music/<id>/track.<ext>`, else what the resolver finds, else YouTube's own audio
//! file (M46.51, M4A at the Downloads quality), else YouTube's copy with yt-dlp on desktop; when
//! none gives a file, a stream that plays is captured in the background (M46.49, [`capture`]). Playback swaps in the saved copy by the song's key, so a downloaded song
//! plays offline wherever it appears. Saved songs can also be written out as files, as
//! downloaded, WAV, FLAC or MP3.
//!
//! The queue can be paused, put in any order, and songs tried again or given another source go
//! first. Every change is pushed at once (`SongDownloadsEvent`, and the download itself as
//! `SongDownloadProgressEvent`); progress at most four times a second, and only when what it shows
//! moved on. Each saved copy remembers where it came from, so Check downloads ([`check`]) can
//! tell one made from another source or for other names, or a file gone or damaged; downloading
//! it again keeps the copy until the new one is done.
//!
//! Songs only YouTube's player plays are recorded (M46.52): captured from a hidden YouTube player
//! of their own, in the background and silent, beside whatever plays ([`capture::youtube`]). They
//! wait in a queue of their own (`kind` `recording`), worked by a second worker one at a time, so
//! files keep downloading meanwhile. What a recording captured is kept: one that missed parts
//! tries again later by itself and captures only those. A copy with less than 90% of the song is
//! never played instead of the stream.

pub mod capture;
mod check;
pub mod transcode;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use raygun_db::music_downloads::{self as store, NewSong, Origin, SongDownloadRow};
use raygun_model::{
    AudioFileFormat, MediaKind, MediaSource, OnlineTrack, QueueItem, SaveLocation, SongDestination,
    SongDownload, SongDownloadKind, SongDownloadProblem, SongDownloadState, SongDownloadsChecked,
    SongsSaved, TrackInput, TrackQuery,
};
use raygun_net::ImageLinks;
use reqwest::header::{CONTENT_RANGE, CONTENT_TYPE, RANGE};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;

use crate::catalog::{now_ms, to_message};
use crate::events::{SongDownloadProgressEvent, SongDownloadsEvent};
use crate::export::output::Place;
use crate::resolution::Exclusions;
use crate::resolver::ResolverService;
use crate::startup::AppDb;

/// Folder under the downloads root.
const FOLDER: &str = "music";
/// A stream's file while it downloads, and the address it comes from: a partial file only
/// continues from the stream that started it.
const PARTIAL: &str = "track.part";
const PARTIAL_SOURCE: &str = "track.part.url";
/// The UI hears a download's progress at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);
/// How often progress is written down, for a restart to continue from it.
const PROGRESS_SAVE_EVERY: Duration = Duration::from_secs(2);
/// Progress without a known size is told in steps of this many bytes.
const UNSIZED_STEP: u64 = 100 * 1024;
/// The speed is measured over at least this long.
const SPEED_WINDOW: Duration = Duration::from_millis(500);
/// Streams tried before giving up on finding a file (YouTube picks and other songs by that name
/// don't count).
const RESOLVE_TRIES: usize = 10;
/// Picks looked at in all, so a resolver offering only skipped ones still ends.
const RESOLVE_PICKS: usize = 30;
/// A stream that drops or answers 5xx/429/408 is tried again after these waits, continuing from
/// what it saved (M46.46, as Kreate's downloads retry).
const STREAM_RETRIES: [Duration; 3] = [
    Duration::from_secs(1),
    Duration::from_secs(3),
    Duration::from_secs(8),
];
/// A stream that sends nothing for this long has dropped.
const STALL: Duration = Duration::from_secs(30);
/// A song that failed for network reasons goes back to the queue after these waits.
const AUTO_RETRIES: [Duration; 3] = [
    Duration::from_secs(30),
    Duration::from_secs(120),
    Duration::from_secs(600),
];
/// Biggest song file accepted.
const MAX_SONG_BYTES: u64 = 1024 * 1024 * 1024;
/// Songs queued at once that are announced one by one; more change the whole list.
const ANNOUNCED: usize = 20;
/// The name a YouTube copy saved by yt-dlp is shown under.
const YTDLP_SOURCE: &str = "YouTube (yt-dlp)";
/// The name YouTube's own audio files are shown under (M46.51).
const YOUTUBE: &str = "YouTube";
/// YouTube videos of a song whose audio files are tried, besides the one it was asked with.
const YOUTUBE_TRIES: usize = 3;
/// Streams a capture is tried from before giving up (M46.49).
const CAPTURE_TRIES: usize = 3;
/// The provider recorded for the source a song was asked with, and for one picked for it.
const OWN: &str = "own";
const PICKED: &str = "picked";

/// Why the running download must stop now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// Downloads were paused: it waits in its place, keeping what it saved.
    Paused,
    /// Another source was picked: it starts again from that one, first in the queue.
    Restarted,
    /// Its recording was stopped: it waits to be recorded again (Retry), keeping what it has.
    Held,
    /// Downloading a saved song again was called off: the copy it kept stays (M46.49).
    Kept,
    /// Taken out of the downloads: its files are deleted.
    Removed,
}

/// What a song whose recording was stopped says; it waits for Retry.
const RECORDING_STOPPED: &str = "Recording stopped. Tap Retry to record it.";

/// The notification saying songs wait for Raygun to be open to be recorded.
const WINDOW_NOTICE: &str = "downloads.recordWhenOpen";

/// The folder in a download's own where a recording keeps what it captured between tries.
const KEPT_SOUND: &str = "sound";

/// A recording that missed parts tries again by itself after these waits (and when Raygun starts).
const PARTIAL_RETRIES: [Duration; 3] = [
    Duration::from_secs(60),
    Duration::from_secs(5 * 60),
    Duration::from_secs(30 * 60),
];

/// The two workers: files one at a time, and recordings one at a time beside them (M46.52).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
enum Lane {
    Files,
    Recordings,
}

/// Why a song only YouTube's player plays can't be recorded right now: it waits in its place.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum RecordWait {
    /// Raygun's window is gone (Android: Raygun was swiped away while downloads carried on), and
    /// with it the webview its hidden player plays in.
    Window,
    /// This device can only hear YouTube's player while Raygun's own player is quiet (its webview
    /// gives no sound from a muted player, and an unmuted one would take the audio focus).
    Quiet,
}

impl RecordWait {
    /// What the song says while it waits.
    fn note(self) -> &'static str {
        match self {
            Self::Window => "Waits until Raygun is open: it's recorded in Raygun's window.",
            Self::Quiet => {
                "Waits until nothing plays: on this device YouTube's player can only be recorded \
                 while Raygun's player is quiet."
            }
        }
    }
}

/// The download running now, and what wakes it when it must stop.
struct Running {
    id: String,
    stop: Arc<Notify>,
}

/// How fast the running download goes, smoothed.
#[derive(Debug, Default, Clone)]
struct Speed {
    sample: Option<(Instant, u64)>,
    bytes_per_second: Option<f64>,
}

impl Speed {
    fn update(&mut self, done: u64, now: Instant) {
        match self.sample {
            Some((at, from)) if done >= from => {
                let elapsed = now.duration_since(at);
                if elapsed < SPEED_WINDOW {
                    return;
                }
                let rate = (done - from) as f64 / elapsed.as_secs_f64();
                self.bytes_per_second = Some(match self.bytes_per_second {
                    Some(before) => before * 0.6 + rate * 0.4,
                    None => rate,
                });
                self.sample = Some((now, done));
            }
            // The first look, or another stream starting over.
            _ => {
                *self = Self {
                    sample: Some((now, done)),
                    bytes_per_second: None,
                }
            }
        }
    }

    /// Time left in ms, when the size and a speed are known.
    fn eta_ms(&self, done: u64, total: Option<u64>) -> Option<f64> {
        let speed = self.bytes_per_second.filter(|speed| *speed >= 1.0)?;
        Some(total?.saturating_sub(done) as f64 / speed * 1000.0)
    }
}

/// What the running download is doing now; lists and events show it.
struct Live {
    /// The download as it was when it started.
    item: SongDownload,
    /// A source's file, or a capture.
    kind: SongDownloadKind,
    done: u64,
    total: Option<u64>,
    /// A capture's share done (its bytes don't say).
    share: Option<f64>,
    /// The source being tried now.
    source: Option<String>,
    speed: Speed,
    /// The step of progress last told, and when.
    told: Option<(u64, Instant)>,
    /// The recordings' worker runs it (M46.52): it shows as a recording whatever it tries.
    recording: bool,
}

impl Live {
    /// `item` as it is now.
    fn apply(&self, item: SongDownload) -> SongDownload {
        let fraction = self.share.or_else(|| fraction(self.done, self.total));
        SongDownload {
            state: SongDownloadState::Downloading,
            kind: Some(if self.recording {
                SongDownloadKind::Recording
            } else {
                self.kind
            }),
            provider: self.source.clone().or(item.provider),
            done_bytes: self.done as f64,
            total_bytes: self.total.map(|total| total as f64),
            fraction,
            bytes_per_second: self.speed.bytes_per_second,
            // A capture's bytes don't tell its time left.
            eta_ms: match self.share {
                Some(_) => None,
                None => self.speed.eta_ms(self.done, self.total),
            },
            ..item
        }
    }
}

pub struct SongDownloadService {
    app: AppHandle,
    /// The downloads folder.
    root: PathBuf,
    client: reqwest::Client,
    images: ImageLinks,
    /// Wakes the files' worker.
    wake: Notify,
    /// Wakes the recordings' worker (M46.52).
    wake_recordings: Notify,
    paused: AtomicBool,
    /// The download each worker runs now.
    running: Mutex<HashMap<Lane, Running>>,
    /// Stops asked of the running downloads.
    stops: Mutex<HashMap<String, Stop>>,
    live: Mutex<HashMap<String, Live>>,
    /// Finished songs by key: the saved file. Playback asks it for every song it loads.
    offline: Mutex<HashMap<String, PathBuf>>,
    /// Network retries already made, by download (M46.46); reset when one succeeds.
    auto_retries: Mutex<HashMap<String, usize>>,
    /// The YouTube video the files' worker found for a song only YouTube's player plays, and
    /// where it came from: the recordings' worker records it without looking again.
    to_record: Mutex<HashMap<String, (String, Origin)>>,
    /// Tries already made by recordings that missed parts, by download.
    partial_retries: Mutex<HashMap<String, usize>>,
}

impl SongDownloadService {
    pub fn new(
        app: AppHandle,
        downloads: PathBuf,
        images: ImageLinks,
        client: reqwest::Client,
    ) -> Arc<Self> {
        Arc::new(Self {
            app,
            root: downloads,
            client,
            images,
            wake: Notify::new(),
            wake_recordings: Notify::new(),
            paused: AtomicBool::new(false),
            running: Mutex::new(HashMap::new()),
            stops: Mutex::new(HashMap::new()),
            live: Mutex::new(HashMap::new()),
            offline: Mutex::new(HashMap::new()),
            auto_retries: Mutex::new(HashMap::new()),
            to_record: Mutex::new(HashMap::new()),
            partial_retries: Mutex::new(HashMap::new()),
        })
    }

    /// Loads the finished songs and starts the workers. Downloads cut off by the last exit go back
    /// to the queue first and continue where they stopped; recordings that waited or missed parts
    /// are queued again.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let db = service.db();
            if let Err(error) = store::reset_interrupted(&db).await {
                tracing::warn!(%error, "could not reset interrupted song downloads");
            }
            // YouTube's own audio files download now (M46.51).
            match store::requeue_youtube_failures(&db, now_ms()).await {
                Ok(0) => {}
                Ok(count) => {
                    tracing::info!(count, "song downloads YouTube had are queued again")
                }
                Err(error) => tracing::warn!(%error, "could not queue YouTube's songs again"),
            }
            // Recordings waiting (for a consent or Raygun's window before M46.52) or that missed
            // parts go on; ones the person stopped wait for Retry.
            match store::requeue_recordings_except(&db, RECORDING_STOPPED, now_ms()).await {
                Ok(0) => {}
                Ok(count) => tracing::info!(count, "songs waiting to be recorded are queued"),
                Err(error) => {
                    tracing::warn!(%error, "could not queue the songs waiting to be recorded")
                }
            }
            match store::done(&db).await {
                Ok(rows) => {
                    let mut short = Vec::new();
                    {
                        let mut offline = service.offline.lock().expect("offline index poisoned");
                        for row in rows {
                            let Some(file) = row.file.clone() else {
                                continue;
                            };
                            if service.plays_whole(&row) {
                                offline.insert(
                                    row.song.track_key.clone(),
                                    service.dir(&row.id).join(file),
                                );
                            } else {
                                short.push(row);
                            }
                        }
                    }
                    for row in short {
                        service.mark_partial(&row).await;
                    }
                }
                Err(error) => tracing::warn!(%error, "song downloads unreadable"),
            }
            let recordings = service.clone();
            tauri::async_runtime::spawn(async move { recordings.work(Lane::Recordings).await });
            service.work(Lane::Files).await;
        });
    }

    /// Whether a saved copy has enough of the song to play instead of streaming it (M46.52): at
    /// least 90% of what plays of it (its trims), when both lengths are known.
    fn plays_whole(&self, row: &SongDownloadRow) -> bool {
        let (Some(file_ms), Some(duration_ms)) = (row.file_ms, row.song.duration_ms) else {
            return true;
        };
        let (_, end) = self.played_part(row);
        let full = u64::try_from(duration_ms).unwrap_or(0);
        let needed = end.map_or(full, |end| end.min(full));
        needed == 0 || capture::covers_enough(u64::try_from(file_ms).unwrap_or(0), needed)
    }

    /// A saved copy too short to play instead of the stream: it says so (Check downloads), and
    /// the song streams until it's downloaded again.
    async fn mark_partial(&self, row: &SongDownloadRow) {
        let problem = partial_problem(row);
        let json = serde_json::to_string(&problem).ok();
        tracing::info!(song = %row.song.track_key, "a saved copy holds too little of its song; it streams instead");
        if matches!(
            store::set_problem(&self.db(), &row.id, json.as_deref()).await,
            Ok(true)
        ) {
            self.announce(&row.id).await;
        }
    }

    /// The saved copy of a song, by its key.
    pub fn offline_file(&self, key: &str) -> Option<PathBuf> {
        self.offline
            .lock()
            .expect("offline index poisoned")
            .get(key)
            .cloned()
    }

    /// Keys of every finished song, for marking them in lists.
    pub fn downloaded_keys(&self) -> Vec<String> {
        self.offline
            .lock()
            .expect("offline index poisoned")
            .keys()
            .cloned()
            .collect()
    }

    pub fn paused(&self) -> bool {
        self.paused.load(Ordering::SeqCst)
    }

    /// Pausing stops the running downloads where they are (they continue from there on resume).
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        if paused {
            let running: Vec<String> = self
                .running
                .lock()
                .expect("running poisoned")
                .values()
                .map(|running| running.id.clone())
                .collect();
            for id in running {
                self.stop_running(&id, Stop::Paused);
            }
        } else {
            self.wake_workers();
        }
        self.changed("");
    }

    /// Wakes both workers: something was queued.
    fn wake_workers(&self) {
        self.wake.notify_one();
        self.wake_recordings.notify_one();
    }

    async fn work(&self, lane: Lane) {
        let wake = match lane {
            Lane::Files => &self.wake,
            Lane::Recordings => &self.wake_recordings,
        };
        loop {
            // Registered before checking, so a wake-up between the check and the wait isn't lost.
            let woken = wake.notified();
            if self.paused() {
                woken.await;
                continue;
            }
            // Battery saver holds the next download (unless allowed); one under way finishes.
            crate::device_state::wait_for_downloads().await;
            match store::next_queued(&self.db(), lane == Lane::Recordings).await {
                Ok(Some(row)) => {
                    if lane == Lane::Recordings && !self.wait_to_record(&row).await {
                        continue;
                    }
                    self.download(row, lane).await
                }
                Ok(None) => woken.await,
                Err(error) => {
                    tracing::warn!(%error, "song download queue unreadable");
                    woken.await;
                }
            }
        }
    }

    /// Queues songs; ones already queued, running or saved are left alone, and ones that failed
    /// go back to the queue.
    pub async fn enqueue(&self, tracks: Vec<TrackInput>) -> Result<(), String> {
        if let Some(collections) = self
            .app
            .try_state::<crate::collections::CollectionsService>()
        {
            // Downloaded lists mark them too.
            collections.note_explicit(&tracks).await;
        }
        let db = self.db();
        let mut queued = Vec::new();
        for track in tracks {
            if matches!(track.source, Some(MediaSource::LocalFile { .. })) {
                // Already on this device.
                continue;
            }
            let song = NewSong {
                track_key: raygun_resolver::track_key(&query_for(&track)),
                title: track.title,
                artist: track.artist,
                album: track.album,
                duration_ms: track.duration_ms.map(i64::from),
                artwork_url: track
                    .artwork_url
                    .as_deref()
                    .map(|url| self.images.original(url)),
                // A lookup isn't a stream: the resolver finds one by the song's key.
                source: track
                    .source
                    .as_ref()
                    .filter(|source| !matches!(source, MediaSource::Lookup { .. }))
                    .and_then(|source| serde_json::to_string(source).ok()),
            };
            let (id, changed) = store::enqueue(&db, &song, now_ms())
                .await
                .map_err(to_message)?;
            if changed {
                self.auto_retries
                    .lock()
                    .expect("retries poisoned")
                    .remove(&id);
                queued.push(id);
            }
        }
        if queued.is_empty() {
            return Ok(());
        }
        // A song whose recording was stopped goes back to the recordings.
        self.wake_workers();
        if queued.len() <= ANNOUNCED {
            for id in &queued {
                self.announce(id).await;
            }
        } else {
            self.changed("");
        }
        Ok(())
    }

    async fn download(&self, row: SongDownloadRow, lane: Lane) {
        let db = self.db();
        if let Err(error) = store::set_state(&db, &row.id, store::DOWNLOADING).await {
            tracing::warn!(%error, "could not start a song download");
            return;
        }
        // Removed between the queue and now.
        let Ok(Some(row)) = store::get(&db, &row.id).await else {
            return;
        };
        let stop = Arc::new(Notify::new());
        self.running.lock().expect("running poisoned").insert(
            lane,
            Running {
                id: row.id.clone(),
                stop: stop.clone(),
            },
        );
        let item = self.to_download(row.clone());
        self.live.lock().expect("live poisoned").insert(
            row.id.clone(),
            Live {
                kind: match lane {
                    Lane::Files => SongDownloadKind::File,
                    Lane::Recordings => SongDownloadKind::Recording,
                },
                done: row.done_bytes.max(0) as u64,
                total: row.total_bytes.map(|total| total.max(0) as u64),
                share: None,
                source: row.provider.clone(),
                speed: Speed::default(),
                told: None,
                recording: lane == Lane::Recordings,
                item,
            },
        );
        self.announce(&row.id).await;
        // Paused or removed while it was being picked up.
        let result = if self.paused() || self.stopped(&row.id) {
            Err(Failure::Stopped)
        } else {
            self.fetch(&row, &stop, lane).await
        };
        self.running.lock().expect("running poisoned").remove(&lane);
        let live = self.live.lock().expect("live poisoned").remove(&row.id);
        let asked = self.stops.lock().expect("stops poisoned").remove(&row.id);
        // The video found for a recording is kept while it waits to be recorded.
        let waits = matches!(
            (&asked, &result),
            (
                None,
                Err(Failure::ToRecord(_) | Failure::Recording(_) | Failure::Partial(_))
            ) | (Some(Stop::Held | Stop::Paused), _)
                | (None, Err(Failure::Stopped))
        );
        if !waits {
            self.to_record
                .lock()
                .expect("recordings poisoned")
                .remove(&row.id);
        }
        let outcome = match (asked, result) {
            (Some(Stop::Removed), _) => {
                self.delete_files(&row.id).await;
                Ok(())
            }
            (Some(Stop::Kept), _) => {
                // Downloading again was called off: the copy it kept stays.
                self.clear_partials(&row.id, row.replacing.as_slice()).await;
                self.keep_copy(&row.id).await
            }
            (Some(Stop::Restarted), _) => {
                // Whatever came from the other stream goes; the picked one starts first.
                self.clear_partials(&row.id, row.replacing.as_slice()).await;
                let front = self.front().await;
                store::set_queued(&db, &row.id, Some(front)).await
            }
            (Some(Stop::Held), _) => self.recording_stopped(&row).await,
            // A pause that came too late doesn't undo a finished song.
            (_, Ok(saved)) => self.finished(&row, saved).await,
            // Paused: it waits in its place, keeping what it saved, whatever the stop cut short.
            (Some(Stop::Paused), Err(_)) | (None, Err(Failure::Stopped)) => {
                if let Some(live) = &live
                    && live.done > 0
                {
                    let _ = store::set_progress(
                        &db,
                        &row.id,
                        live.done as i64,
                        live.total.map(|total| total as i64),
                    )
                    .await;
                }
                store::set_queued(&db, &row.id, None).await
            }
            (None, Err(Failure::Error(message))) => {
                tracing::warn!(song = %row.song.track_key, summary = %message.summary, detail = ?message.detail, "song download failed");
                // A recording that failed for good is a failed download like any other.
                if lane == Lane::Recordings {
                    let _ = store::clear_kind(&db, &row.id).await;
                }
                self.failed(&row, &message).await
            }
            (None, Err(Failure::Transient(message))) => {
                tracing::warn!(song = %row.song.track_key, summary = %message.summary, detail = ?message.detail, "song download failed for now");
                let failed = self.failed(&row, &message).await;
                // A song downloading again keeps its copy; it's tried again when asked.
                if row.replacing.is_none() {
                    self.retry_later(&row.id);
                }
                failed
            }
            (None, Err(Failure::ToRecord(found))) => {
                let (video_id, origin) = *found;
                tracing::info!(song = %row.song.track_key, %video_id, "only YouTube's player plays a song: it waits to be recorded");
                self.queue_recording(&row, video_id, origin).await
            }
            (None, Err(Failure::Recording(wait))) => {
                // It waits in its place, keeping what it recorded.
                tracing::info!(song = %row.song.track_key, ?wait, "a recording waits");
                store::set_queued(&db, &row.id, None).await
            }
            (None, Err(Failure::Partial(message))) => {
                tracing::info!(song = %row.song.track_key, summary = %message.summary, detail = ?message.detail, "a recording missed parts");
                self.recorded_part(&row, &message).await
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not record a song download's outcome");
        }
        self.announce(&row.id).await;
    }

    /// What a recording keeps in its folder besides the copy it replaces: what it captured.
    fn kept_by_recording(row: &SongDownloadRow) -> Vec<String> {
        let mut kept = vec![KEPT_SOUND.to_owned()];
        kept.extend(row.replacing.clone());
        kept
    }

    /// A song only YouTube's player plays goes to the recordings' queue, keeping its place: the
    /// recordings' worker records `video_id` without looking again.
    async fn queue_recording(
        &self,
        row: &SongDownloadRow,
        video_id: String,
        origin: Origin,
    ) -> Result<(), raygun_db::DbError> {
        self.to_record
            .lock()
            .expect("recordings poisoned")
            .insert(row.id.clone(), (video_id, origin));
        self.clear_partials(&row.id, &Self::kept_by_recording(row))
            .await;
        let db = self.db();
        store::set_kind(&db, &row.id, Some(store::RECORDING)).await?;
        store::set_queued(&db, &row.id, None).await?;
        self.wake_recordings.notify_one();
        Ok(())
    }

    /// The person stopped a song's recording: it waits for Retry, keeping what it recorded.
    async fn recording_stopped(&self, row: &SongDownloadRow) -> Result<(), raygun_db::DbError> {
        self.clear_partials(&row.id, &Self::kept_by_recording(row))
            .await;
        let message = Message {
            summary: RECORDING_STOPPED.into(),
            detail: Some("What was recorded of it is kept: Retry records the rest.".into()),
        };
        self.failed(row, &message).await?;
        store::set_kind(&self.db(), &row.id, Some(store::RECORDING)).await
    }

    /// A recording that missed parts keeps what it has and tries again by itself a little later,
    /// recording only what's missing; meanwhile the song streams.
    async fn recorded_part(
        &self,
        row: &SongDownloadRow,
        message: &Message,
    ) -> Result<(), raygun_db::DbError> {
        self.clear_partials(&row.id, &Self::kept_by_recording(row))
            .await;
        self.failed(row, message).await?;
        store::set_kind(&self.db(), &row.id, Some(store::RECORDING)).await?;
        if row.replacing.is_none() {
            self.retry_partial_later(&row.id);
        }
        Ok(())
    }

    /// Puts a recording that missed parts back in the recordings' queue later, a few times (and
    /// when Raygun starts), unless it was stopped, retried or removed meanwhile.
    fn retry_partial_later(&self, id: &str) {
        let attempt = {
            let mut retries = self.partial_retries.lock().expect("retries poisoned");
            let count = retries.entry(id.to_owned()).or_insert(0);
            *count += 1;
            *count
        };
        let Some(wait) = PARTIAL_RETRIES.get(attempt - 1).copied() else {
            return;
        };
        let (app, id) = (self.app.clone(), id.to_owned());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(wait).await;
            let service = app.state::<Arc<SongDownloadService>>().inner().clone();
            let still_partial = store::get(&service.db(), &id)
                .await
                .ok()
                .flatten()
                .is_some_and(|row| {
                    row.state == store::FAILED
                        && row
                            .error
                            .as_deref()
                            .is_some_and(|error| error.starts_with(PARTLY_RECORDED))
                });
            if still_partial
                && store::set_queued(&service.db(), &id, Some(now_ms()))
                    .await
                    .is_ok()
            {
                tracing::info!(%id, attempt, "recording what a song's recording missed");
                service.wake_recordings.notify_one();
                service.announce(&id).await;
            }
        });
    }

    /// Waits until the recordings' worker can record `row`: Raygun's window is open (Android:
    /// the hidden player plays in its webview), and, where hearing YouTube's player takes the
    /// audio focus, Raygun's player is quiet. The song says why meanwhile, and a notification asks
    /// to open Raygun. False when downloads were paused or the song left the queue meanwhile.
    async fn wait_to_record(&self, row: &SongDownloadRow) -> bool {
        use capture::youtube::{self, Purpose, Tap};

        let mut said: Option<RecordWait> = None;
        let ready = loop {
            let wait = if !youtube::window_open(&self.app) {
                Some(RecordWait::Window)
            } else if Tap::for_purpose(Purpose::Song).needs_quiet()
                && youtube::player_busy(&self.app)
            {
                Some(RecordWait::Quiet)
            } else {
                None
            };
            if wait != said {
                let _ = store::set_note(&self.db(), &row.id, wait.map(RecordWait::note)).await;
                self.announce(&row.id).await;
                if wait == Some(RecordWait::Window) {
                    self.notify_waiting_for_window().await;
                } else if said == Some(RecordWait::Window) {
                    let id = crate::notifications::notice_id(WINDOW_NOTICE);
                    crate::notifications::cancel_notice(&self.app, id).await;
                }
                said = wait;
            }
            if wait.is_none() {
                break true;
            }
            let woken = self.wake_recordings.notified();
            tokio::select! {
                () = tokio::time::sleep(Duration::from_secs(2)) => {}
                () = woken => {}
            }
            let queued = store::get(&self.db(), &row.id)
                .await
                .ok()
                .flatten()
                .is_some_and(|now| now.state == store::QUEUED);
            if self.paused() || !queued {
                break false;
            }
        };
        if said == Some(RecordWait::Window) {
            let id = crate::notifications::notice_id(WINDOW_NOTICE);
            crate::notifications::cancel_notice(&self.app, id).await;
        }
        ready
    }

    /// A notification saying songs wait for Raygun to be open to be recorded.
    async fn notify_waiting_for_window(&self) {
        let songs = store::recordings(&self.db(), true)
            .await
            .unwrap_or(1)
            .max(1);
        let (title, text) = window_notice(songs);
        crate::notifications::show_notice(
            &self.app,
            crate::notifications::Notice {
                id: crate::notifications::notice_id(WINDOW_NOTICE),
                channel: &crate::notifications::PLAYBACK,
                title,
                text,
                route: "/downloads?tab=music".to_owned(),
            },
        )
        .await;
    }

    /// Stops recording songs (M46.52): `id`'s, else every one queued, being recorded or waiting
    /// to record what it missed. They keep what they recorded and wait for Retry.
    pub async fn cancel_recordings(&self, id: Option<&str>) -> Result<(), String> {
        let running = self
            .running
            .lock()
            .expect("running poisoned")
            .get(&Lane::Recordings)
            .map(|running| running.id.clone());
        if let Some(running) = running.filter(|running| id.is_none_or(|id| id == running)) {
            self.stop_running(&running, Stop::Held);
        }
        let stopped =
            store::stop_recordings(&self.db(), id, RECORDING_STOPPED, PARTLY_RECORDED, now_ms())
                .await
                .map_err(to_message)?;
        if stopped > 0 {
            self.changed("");
        }
        Ok(())
    }

    /// A saved song: its file goes in the offline index and the download is done.
    async fn finished(
        &self,
        row: &SongDownloadRow,
        saved: Saved,
    ) -> Result<(), raygun_db::DbError> {
        tracing::info!(song = %row.song.track_key, file = %saved.file, kind = saved.kind, "song downloaded");
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(&row.id);
        let dir = self.dir(&row.id);
        let path = dir.join(&saved.file);
        // What another stream left half done goes, and the copy this one replaces.
        let replaced = row.replacing.as_deref().filter(|old| *old != saved.file);
        self.clear_partials(
            &row.id,
            &[
                saved.file.clone(),
                row.replacing.clone().unwrap_or_default(),
            ],
        )
        .await;
        let bytes = tokio::fs::metadata(&path)
            .await
            .ok()
            .map(|meta| meta.len() as i64);
        let file_ms = match saved.file_ms {
            Some(ms) => Some(ms),
            None => {
                let measured = path.clone();
                tokio::task::spawn_blocking(move || transcode::length_ms(&measured))
                    .await
                    .ok()
                    .flatten()
            }
        };
        let kept = store::set_done(
            &self.db(),
            &row.id,
            &store::Saved {
                file: saved.file.clone(),
                kind: saved.kind.into(),
                provider: saved.provider.clone(),
                bytes,
                file_ms: file_ms.map(|ms| ms as i64),
                origin: saved.origin.clone(),
            },
            now_ms(),
        )
        .await?;
        // Removed meanwhile: nothing to play from.
        if kept {
            self.partial_retries
                .lock()
                .expect("retries poisoned")
                .remove(&row.id);
            let mut saved_row = row.clone();
            saved_row.file_ms = file_ms.map(|ms| ms as i64);
            let whole = self.plays_whole(&saved_row);
            if whole {
                // Lists show the length of what plays: the file.
                if let (Some(ms), Some(durations)) = (
                    file_ms,
                    self.app
                        .try_state::<Arc<crate::durations::DurationService>>(),
                ) {
                    durations.learn_file(row.song.track_key.clone(), ms as i64);
                }
                self.offline
                    .lock()
                    .expect("offline index poisoned")
                    .insert(row.song.track_key.clone(), path);
            } else {
                // Too little of the song to play instead of the stream (M46.52).
                self.offline
                    .lock()
                    .expect("offline index poisoned")
                    .remove(&row.song.track_key);
                self.mark_partial(&saved_row).await;
            }
            if let Some(old) = replaced {
                let _ = tokio::fs::remove_file(dir.join(old)).await;
            }
            if whole {
                self.save_finished(&row.id);
            }
        }
        Ok(())
    }

    /// A download that didn't work; one downloading again keeps its copy, sized as it is.
    async fn failed(
        &self,
        row: &SongDownloadRow,
        message: &Message,
    ) -> Result<(), raygun_db::DbError> {
        let db = self.db();
        store::set_failed(
            &db,
            &row.id,
            &message.summary,
            message.detail.as_deref(),
            now_ms(),
        )
        .await?;
        if let Some(kept) = &row.replacing {
            self.clear_partials(&row.id, std::slice::from_ref(kept))
                .await;
            self.size_copy(&row.id, kept).await?;
        }
        Ok(())
    }

    /// Stops downloading a song again: the copy it kept stays as it was.
    async fn keep_copy(&self, id: &str) -> Result<(), raygun_db::DbError> {
        if let Some(kept) = store::keep_replaced(&self.db(), id).await? {
            self.size_copy(id, &kept).await?;
        }
        Ok(())
    }

    /// Writes down a kept copy's size again (what downloaded meanwhile counted over it).
    async fn size_copy(&self, id: &str, file: &str) -> Result<(), raygun_db::DbError> {
        let bytes = tokio::fs::metadata(self.dir(id).join(file))
            .await
            .map(|meta| meta.len() as i64)
            .unwrap_or(0);
        store::set_progress(&self.db(), id, bytes, Some(bytes)).await
    }

    /// Deletes what a download left in its folder, except the file `keep`.
    async fn clear_partials(&self, id: &str, keep: &[String]) {
        let Ok(mut entries) = tokio::fs::read_dir(self.dir(id)).await else {
            return;
        };
        while let Ok(Some(entry)) = entries.next_entry().await {
            let name = entry.file_name().to_string_lossy().into_owned();
            if keep.contains(&name) {
                continue;
            }
            let path = entry.path();
            let removed = if entry.file_type().await.is_ok_and(|kind| kind.is_dir()) {
                tokio::fs::remove_dir_all(&path).await
            } else {
                tokio::fs::remove_file(&path).await
            };
            if let Err(error) = removed {
                tracing::debug!(%error, "couldn't delete what a song download left");
            }
        }
    }

    /// Finds a file for the song and saves it: the source it was asked with first, then what the
    /// resolver finds, then YouTube's own audio files (M46.51), then YouTube's copy with yt-dlp on
    /// desktop. When nothing gives a file, a stream that plays it is captured (M46.49): first when
    /// the download was queued to be. A song only YouTube's player plays then goes to the
    /// recordings' queue (M46.52); the recordings' worker records the video found, or (after a
    /// restart) looks again, recording it once nothing else saves it.
    async fn fetch(
        &self,
        row: &SongDownloadRow,
        stop: &Notify,
        lane: Lane,
    ) -> Result<Saved, Failure> {
        if lane == Lane::Recordings {
            let found = self
                .to_record
                .lock()
                .expect("recordings poisoned")
                .get(&row.id)
                .cloned();
            if let Some((video_id, origin)) = found {
                tokio::fs::create_dir_all(self.dir(&row.id))
                    .await
                    .map_err(|error| Failure::Error(device_message(&error.to_string())))?;
                let quality = crate::settings::load(&self.db())
                    .await
                    .unwrap_or_default()
                    .audio_quality
                    .download;
                return self
                    .record(row, &video_id, origin, quality, stop)
                    .await
                    .map_err(|failure| self.recording_failure(failure));
            }
        }
        let asked: Option<MediaSource> = row
            .song
            .source
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok());
        // Searched by the names the song shows (Edit details), as it plays.
        let (title, artist) = self.shown_names(row);
        let query = TrackQuery {
            title,
            artist,
            album: row.song.album.clone(),
            duration_ms: row.song.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
        };
        let resolver = self.app.state::<Arc<ResolverService>>().inner().clone();
        let dir = self.dir(&row.id);
        tokio::fs::create_dir_all(&dir)
            .await
            .map_err(|error| Failure::Error(device_message(&error.to_string())))?;
        let quality = crate::settings::load(&self.db())
            .await
            .unwrap_or_default()
            .audio_quality
            .download;
        let mut exclusions = Exclusions {
            fresh_only: true,
            quality: Some(quality),
            decodable: true,
            ..Exclusions::default()
        };
        let using = resolver.stream_use(Some(quality), true).await;
        // What each source said, the streams that play it but gave no file (captured when nothing
        // else works), and the YouTube videos of the song (saved from after the other sources).
        let mut attempts = Attempts::default();
        let own = |source: &MediaSource| Origin {
            provider: Some(if row.provider.is_some() { PICKED } else { OWN }.into()),
            stream: None,
            source: serde_json::to_string(source).ok(),
            title: Some(row.song.title.clone()),
            artist: row.song.artist.clone(),
        };

        // 0. A download queued to be captured (a song an earlier version waited to record).
        if row.kind.as_deref() == Some("capture")
            && let Some(source @ MediaSource::Url { url, .. }) = &asked
        {
            let candidate = Candidate {
                name: row.provider.clone().unwrap_or_else(|| source_name(url)),
                source: source.clone(),
                origin: own(source),
            };
            match self.capture(row, &candidate, quality, stop).await {
                Ok(saved) => return Ok(saved),
                Err(CaptureFailure::Stopped) => return Err(Failure::Stopped),
                Err(CaptureFailure::Device(error)) => {
                    return Err(Failure::Error(device_message(&error)));
                }
                Err(CaptureFailure::Failed(error) | CaptureFailure::Partial { why: error, .. }) => {
                    attempts
                        .report
                        .captured
                        .push((candidate.name.clone(), error));
                }
                Err(CaptureFailure::Interrupted) => {}
            }
            let _ = store::clear_kind(&self.db(), &row.id).await;
        }

        // 1. The source it was asked with, or the one picked for it: a YouTube video's own audio
        // file (M46.51).
        match &asked {
            Some(source @ MediaSource::YouTube { video_id }) => {
                let mut file = None;
                match self
                    .save_youtube(row, &dir, video_id, &mut file, &resolver, using, stop)
                    .await
                {
                    Ok(saved) => return Ok(saved.from(own(source))),
                    Err(StreamError::Stopped) => return Err(Failure::Stopped),
                    Err(StreamError::Device(error)) => {
                        return Err(Failure::Error(device_message(&error)));
                    }
                    Err(error) => attempts.own_video_failed(video_id, file, own(source), &error),
                }
            }
            Some(source @ MediaSource::Url { url, headers })
                if attempts.report.captured.is_empty() =>
            {
                let name = row.provider.clone().unwrap_or_else(|| source_name(url));
                self.trying(&row.id, Some(name.clone()));
                match self
                    .save_retrying(row, &dir, url, headers, Some(name.clone()), stop)
                    .await
                {
                    Ok(saved) => return Ok(saved.from(own(source))),
                    Err(StreamError::Stopped) => return Err(Failure::Stopped),
                    Err(StreamError::Device(error)) => {
                        return Err(Failure::Error(device_message(&error)));
                    }
                    Err(error) => attempts.file_failed(
                        Candidate {
                            name,
                            source: source.clone(),
                            origin: own(source),
                        },
                        &error,
                    ),
                }
                exclusions.failed_sources.push(source.clone());
                tracing::info!(song = %row.song.track_key, "the song's own stream gave no file; trying others");
            }
            Some(source @ MediaSource::Url { .. }) => {
                exclusions.failed_sources.push(source.clone())
            }
            _ => {}
        }

        // 2. Whatever else the resolver finds; YouTube's videos wait for step 3.
        let mut tries = 0;
        for _ in 0..RESOLVE_PICKS {
            if tries >= RESOLVE_TRIES || self.stopped(&row.id) {
                break;
            }
            let pick = tokio::select! {
                pick = resolver.resolve(&query, &exclusions) => pick,
                () = stop.notified() => return Err(Failure::Stopped),
            };
            let pick = match pick {
                Ok(pick) => pick,
                Err(error) => {
                    attempts.report.resolver = Some(error);
                    break;
                }
            };
            exclusions.tried.push(pick.key.clone());
            // A saved copy must be this song: every word of its title in the stream's, and its
            // artist named in the stream's artist or title (an exact "Swim" can be anyone's).
            let credited = format!(
                "{} {}",
                pick.artist.as_deref().unwrap_or_default(),
                pick.title
            );
            if !covers_title(&query.title, &pick.title)
                || !names_artist(query.artist.as_deref(), &credited)
            {
                tracing::info!(song = %row.song.track_key, offered = %pick.title, "skipping a loose match");
                attempts
                    .report
                    .loose
                    .push((pick.provider_name.clone(), pick.title.clone()));
                continue;
            }
            let origin = Origin {
                provider: Some(pick.key.provider_id.clone()),
                stream: Some(pick.key.candidate_id.clone()),
                source: serde_json::to_string(&pick.source).ok(),
                title: Some(pick.title.clone()),
                artist: pick.artist.clone(),
            };
            // A YouTube video, its audio file or its embed: saved from after the others.
            let video = crate::resolver::youtube_video(&pick.key).or(match &pick.source {
                MediaSource::YouTube { video_id } => Some(video_id.as_str()),
                _ => None,
            });
            if let Some(video_id) = video {
                let file =
                    matches!(pick.source, MediaSource::Url { .. }).then(|| pick.source.clone());
                attempts.video_offered(video_id, file, origin);
                continue;
            }
            if let MediaSource::Url { url, headers } = &pick.source {
                tries += 1;
                let name = pick.provider_name.clone();
                self.trying(&row.id, Some(name.clone()));
                match self
                    .save_retrying(row, &dir, url, headers, Some(name.clone()), stop)
                    .await
                {
                    Ok(saved) => {
                        resolver.mark_verified(&pick).await;
                        return Ok(saved.from(origin));
                    }
                    Err(StreamError::Stopped) => return Err(Failure::Stopped),
                    Err(StreamError::Device(error)) => {
                        return Err(Failure::Error(device_message(&error)));
                    }
                    // In pieces, on a busy network or failing: noted, and kept to capture. Only a
                    // stream's own error counts against it for playback.
                    Err(error) => {
                        if let StreamError::Error(message) = &error {
                            resolver.mark_failed(&pick, message).await;
                        }
                        attempts.file_failed(
                            Candidate {
                                name,
                                source: pick.source.clone(),
                                origin,
                            },
                            &error,
                        );
                    }
                }
            }
        }

        // 3. YouTube's own audio files (M46.51), a few videos of the song.
        for index in attempts.videos_to_save() {
            if self.stopped(&row.id) {
                return Err(Failure::Stopped);
            }
            let copy = &attempts.youtube[index];
            let (video_id, origin, mut file) = (
                copy.video_id.clone(),
                copy.origin.clone(),
                copy.file.clone(),
            );
            match self
                .save_youtube(row, &dir, &video_id, &mut file, &resolver, using, stop)
                .await
            {
                Ok(saved) => return Ok(saved.from(origin)),
                Err(StreamError::Stopped) => return Err(Failure::Stopped),
                Err(StreamError::Device(error)) => {
                    return Err(Failure::Error(device_message(&error)));
                }
                Err(error) => attempts.video_failed(index, file, &error),
            }
        }

        // 4. YouTube's copy as a file with yt-dlp on desktop (M46.47).
        if let Some(copy) = attempts.youtube.first()
            && crate::ytdlp::supported()
            && !self.stopped(&row.id)
        {
            let (video_id, origin) = (copy.video_id.clone(), copy.origin.clone());
            self.trying(&row.id, Some(YOUTUBE.into()));
            match self.save_with_ytdlp(row, &dir, &video_id, stop).await {
                Ok(saved) => return Ok(saved.from(origin)),
                Err(StreamError::Stopped) => return Err(Failure::Stopped),
                Err(StreamError::Device(error)) => {
                    return Err(Failure::Error(device_message(&error)));
                }
                Err(error) => attempts.report.ytdlp = Some(error.text()),
            }
        }

        // 5. The last resort: a stream that plays is captured (M46.49). Without yt-dlp (Android)
        // nothing else saves a YouTube song whose audio file wouldn't download.
        let to_capture = attempts.capture_order();
        let captured = capture_in_turn(
            to_capture,
            &mut attempts.report,
            || self.stopped(&row.id),
            |candidate| async move { self.capture(row, &candidate, quality, stop).await },
        )
        .await?;
        if let Some(saved) = captured {
            return Ok(saved);
        }

        // 6. When nothing else saved it, a song YouTube's player plays is recorded from a hidden
        // player of its own (M46.52): in the recordings' queue, beside the files.
        if let Some((video_id, origin)) = attempts.video_to_record()
            && !self.stopped(&row.id)
        {
            if lane == Lane::Files {
                return Err(Failure::ToRecord(Box::new((video_id, origin))));
            }
            match self.record(row, &video_id, origin, quality, stop).await {
                Ok(saved) => return Ok(saved),
                Err(CaptureFailure::Failed(error)) => {
                    tracing::info!(song = %row.song.track_key, %error, "couldn't record a song");
                    attempts.report.captured.push((YOUTUBE.into(), error));
                }
                Err(failure) => return Err(self.recording_failure(failure)),
            }
        }
        if self.stopped(&row.id) {
            return Err(Failure::Stopped);
        }
        let report = &attempts.report;
        let message = report
            .message(report.transient && row.replacing.is_none() && self.retries_left(&row.id));
        if report.transient {
            Err(Failure::Transient(message))
        } else {
            Err(Failure::Error(message))
        }
    }

    /// Captures a stream that plays the song (M46.49): its bytes, else its sound.
    async fn capture(
        &self,
        row: &SongDownloadRow,
        candidate: &Candidate,
        quality: raygun_model::AudioQuality,
        stop: &Notify,
    ) -> Result<Saved, CaptureFailure> {
        self.capturing(&row.id, &candidate.name, SongDownloadKind::Capture);
        let (start_ms, end_ms) = self.played_part(row);
        let cover = match row.song.artwork_url.as_deref() {
            Some(url) => self.cover_bytes(url).await,
            None => None,
        };
        let tags = transcode::SongTags {
            title: row.song.title.clone(),
            artist: row.song.artist.clone(),
            album: row.song.album.clone(),
            cover,
        };
        let id = row.id.clone();
        let progress = |bytes: u64, share: Option<f64>| self.capture_progress(&id, bytes, share);
        let dir = self.dir(&row.id);
        let stem = format!("capture-{}", uuid_name());
        let captured = capture::capture(
            &self.app,
            &self.client,
            capture::Request {
                source: &candidate.source,
                dir: &dir,
                stem: &stem,
                expected: capture::Expected {
                    duration_ms: row.song.duration_ms.and_then(|ms| u64::try_from(ms).ok()),
                    start_ms,
                    end_ms,
                },
                quality,
                tags: Some(tags),
            },
            &progress,
            stop,
        )
        .await
        .map_err(capture_failure)?;
        // Saved as `track.<ext>`, as every download is.
        let extension = captured
            .file
            .rsplit_once('.')
            .map_or("flac", |(_, extension)| extension)
            .to_owned();
        let file = format!("track.{extension}");
        tokio::fs::rename(dir.join(&captured.file), dir.join(&file))
            .await
            .map_err(|error| CaptureFailure::Device(error.to_string()))?;
        Ok(Saved {
            file,
            provider: Some(candidate.name.clone()),
            kind: "capture",
            file_ms: captured.duration_ms,
            origin: candidate.origin.clone(),
        })
    }

    /// Records a song that plays only in YouTube's player (M46.52): its video `video_id` plays in a
    /// hidden YouTube player of its own, silent and beside whatever plays, its sound captured and
    /// saved as a captured song is. What it captured is kept until it's whole.
    async fn record(
        &self,
        row: &SongDownloadRow,
        video_id: &str,
        origin: Origin,
        quality: raygun_model::AudioQuality,
        stop: &Notify,
    ) -> Result<Saved, CaptureFailure> {
        self.capturing(&row.id, YOUTUBE, SongDownloadKind::Recording);
        let (start_ms, end_ms) = self.played_part(row);
        let cover = match row.song.artwork_url.as_deref() {
            Some(url) => self.cover_bytes(url).await,
            None => None,
        };
        let tags = transcode::SongTags {
            title: row.song.title.clone(),
            artist: row.song.artist.clone(),
            album: row.song.album.clone(),
            cover,
        };
        let id = row.id.clone();
        let progress = |bytes: u64, share: Option<f64>| self.capture_progress(&id, bytes, share);
        let dir = self.dir(&row.id);
        let stem = format!("recording-{}", uuid_name());
        let source = MediaSource::YouTube {
            video_id: video_id.to_owned(),
        };
        let recorded = capture::youtube(
            &self.app,
            capture::Request {
                source: &source,
                dir: &dir,
                stem: &stem,
                expected: capture::Expected {
                    duration_ms: row.song.duration_ms.and_then(|ms| u64::try_from(ms).ok()),
                    start_ms,
                    end_ms,
                },
                quality,
                tags: Some(tags),
            },
            &progress,
            stop,
        )
        .await
        .map_err(capture_failure)?;
        let extension = recorded
            .file
            .rsplit_once('.')
            .map_or("flac", |(_, extension)| extension)
            .to_owned();
        let file = format!("track.{extension}");
        tokio::fs::rename(dir.join(&recorded.file), dir.join(&file))
            .await
            .map_err(|error| CaptureFailure::Device(error.to_string()))?;
        Ok(Saved {
            file,
            provider: Some(YOUTUBE.into()),
            kind: store::RECORDING,
            file_ms: recorded.duration_ms,
            origin,
        })
    }

    /// What a recording that couldn't finish comes to: it waits (Raygun's window, or quiet), keeps
    /// what it has for later, or fails.
    fn recording_failure(&self, failure: CaptureFailure) -> Failure {
        match failure {
            CaptureFailure::Stopped => Failure::Stopped,
            CaptureFailure::Device(error) => Failure::Error(device_message(&error)),
            CaptureFailure::Interrupted => {
                Failure::Recording(if capture::youtube::window_open(&self.app) {
                    RecordWait::Quiet
                } else {
                    RecordWait::Window
                })
            }
            CaptureFailure::Partial {
                covered_ms,
                needed_ms,
                why,
            } => Failure::Partial(partial_message(covered_ms, needed_ms, &why)),
            CaptureFailure::Failed(error) => Failure::Error(Message {
                summary: "Couldn't record it from YouTube's player.".into(),
                detail: Some(error),
            }),
        }
    }

    /// The names the song shows now: its own, or the ones Edit details gave it.
    fn shown_names(&self, row: &SongDownloadRow) -> (String, Option<String>) {
        self.app
            .try_state::<Arc<crate::details::DetailService>>()
            .and_then(|details| {
                details.shown_names(
                    &row.song.track_key,
                    &row.song.title,
                    row.song.artist.as_deref(),
                )
            })
            .unwrap_or_else(|| (row.song.title.clone(), row.song.artist.clone()))
    }

    /// A YouTube video's own audio file (M46.51): `file` when the resolver already gave it, else
    /// asked for now (M4A, at the Downloads quality) and left in `file`, saved by ranges as
    /// YouTube's servers want. A file that doesn't save stays in `file`: it plays, so it's
    /// captured when nothing else works (M46.49).
    #[allow(clippy::too_many_arguments)]
    async fn save_youtube(
        &self,
        row: &SongDownloadRow,
        dir: &Path,
        video_id: &str,
        file: &mut Option<MediaSource>,
        resolver: &ResolverService,
        using: raygun_ext_host::StreamUse,
        stop: &Notify,
    ) -> Result<Saved, StreamError> {
        self.trying(&row.id, Some(YOUTUBE.into()));
        let source = match file.clone() {
            Some(source) => source,
            None => tokio::select! {
                resolved = resolver.youtube_audio(video_id, using) => {
                    file.insert(resolved.map_err(StreamError::Error)?.source).clone()
                }
                () = stop.notified() => return Err(StreamError::Stopped),
            },
        };
        let MediaSource::Url { url, headers } = &source else {
            return Err(StreamError::Error("YouTube gave no file for it".into()));
        };
        self.save_retrying(row, dir, url, headers, Some(YOUTUBE.into()), stop)
            .await
    }

    /// A YouTube video's sound saved with yt-dlp (M46.47), checked like any other file.
    async fn save_with_ytdlp(
        &self,
        row: &SongDownloadRow,
        dir: &Path,
        video_id: &str,
        stop: &Notify,
    ) -> Result<Saved, StreamError> {
        let ytdlp = self
            .app
            .state::<Arc<crate::ytdlp::YtDlpService>>()
            .inner()
            .clone();
        // yt-dlp stops when the download must; it waits on its own signal.
        let cancel = Arc::new(Notify::new());
        let fetch = {
            let (app, id) = (self.app.clone(), row.id.clone());
            ytdlp.fetch_song(video_id, dir, cancel.clone(), move |done, total| {
                if let Some(service) = app.try_state::<Arc<SongDownloadService>>() {
                    service.progress(&id, done, total);
                }
            })
        };
        tokio::pin!(fetch);
        let file = tokio::select! {
            file = &mut fetch => file,
            () = stop.notified() => {
                cancel.notify_one();
                let _ = fetch.await;
                return Err(StreamError::Stopped);
            }
        };
        if self.stopped(&row.id) {
            return Err(StreamError::Stopped);
        }
        let file = file.map_err(StreamError::Error)?;
        let path = dir.join(&file);
        let check = path.clone();
        let audio = tokio::task::spawn_blocking(move || transcode::probe(&check))
            .await
            .map_err(|error| StreamError::Error(error.to_string()))?;
        if let Err(error) = audio {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(StreamError::Error(format!(
                "yt-dlp saved something that can't be read ({error})"
            )));
        }
        Ok(Saved::file(file, Some(YTDLP_SOURCE.into())))
    }

    /// [`Self::save_stream`], tried again when the stream drops or the server is busy, each time
    /// continuing from what it saved (M46.46).
    async fn save_retrying(
        &self,
        row: &SongDownloadRow,
        dir: &Path,
        url: &str,
        headers: &std::collections::BTreeMap<String, String>,
        provider: Option<String>,
        stop: &Notify,
    ) -> Result<Saved, StreamError> {
        let mut waits = STREAM_RETRIES.iter();
        loop {
            match self
                .save_stream(row, dir, url, headers, provider.clone(), stop)
                .await
            {
                Err(StreamError::Transient(message)) => match waits.next() {
                    Some(wait) if !self.stopped(&row.id) => {
                        tracing::info!(song = %row.song.track_key, %message, "trying the stream again");
                        tokio::select! {
                            () = tokio::time::sleep(*wait) => {}
                            () = stop.notified() => return Err(StreamError::Stopped),
                        }
                    }
                    _ if self.stopped(&row.id) => return Err(StreamError::Stopped),
                    _ => return Err(StreamError::Transient(message)),
                },
                other => return other,
            }
        }
    }

    /// Whether a song that fails for network reasons now still goes back to the queue by itself.
    fn retries_left(&self, id: &str) -> bool {
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .get(id)
            .copied()
            .unwrap_or(0)
            < AUTO_RETRIES.len()
    }

    /// Puts a song that failed for network reasons back in the queue later, a few times.
    fn retry_later(&self, id: &str) {
        let attempt = {
            let mut retries = self.auto_retries.lock().expect("retries poisoned");
            let count = retries.entry(id.to_owned()).or_insert(0);
            *count += 1;
            *count
        };
        let Some(wait) = AUTO_RETRIES.get(attempt - 1).copied() else {
            return;
        };
        let (app, id) = (self.app.clone(), id.to_owned());
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(wait).await;
            let service = app.state::<Arc<SongDownloadService>>().inner().clone();
            // Only if it's still failed (not removed, retried or recorded meanwhile).
            let still_failed = store::get(&service.db(), &id)
                .await
                .ok()
                .flatten()
                .is_some_and(|row| row.state == store::FAILED);
            if still_failed {
                tracing::info!(%id, attempt, "trying a failed song download again");
                // At the back: the songs waiting meanwhile go first.
                if store::set_queued(&service.db(), &id, Some(now_ms()))
                    .await
                    .is_ok()
                {
                    service.wake_workers();
                    service.announce(&id).await;
                }
            }
        });
    }

    /// Downloads a stream into `dir`, continuing a partial one from the same stream with a Range
    /// request, and checks that it's audio. Returns the file's name.
    async fn save_stream(
        &self,
        row: &SongDownloadRow,
        dir: &Path,
        url: &str,
        headers: &std::collections::BTreeMap<String, String>,
        provider: Option<String>,
        stop: &Notify,
    ) -> Result<Saved, StreamError> {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(StreamError::Error(
                "Only web addresses can be downloaded".into(),
            ));
        }
        let partial = dir.join(PARTIAL);
        let marker = dir.join(PARTIAL_SOURCE);
        let started_from = tokio::fs::read_to_string(&marker).await.ok();
        let mut have = tokio::fs::metadata(&partial)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);
        if !continues(started_from.as_deref(), url, have) {
            // What another stream saved doesn't fit this one.
            let _ = tokio::fs::remove_file(&partial).await;
            have = 0;
        }
        if have == 0 {
            tokio::fs::write(&marker, url)
                .await
                .map_err(|error| StreamError::Device(error.to_string()))?;
        }
        let mut request = self.client.get(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        } else if raygun_net::wants_range(url) {
            // YouTube sends a file at full speed only when it's asked for by range (M46.51).
            request = request.header(RANGE, "bytes=0-");
        }
        let response = tokio::select! {
            response = tokio::time::timeout(STALL, request.send()) => response,
            () = stop.notified() => return Err(StreamError::Stopped),
        }
        .map_err(|_| StreamError::Transient("The server didn't answer".into()))?
        .map_err(network_failure)?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if content_type.contains("mpegurl") || url.to_ascii_lowercase().contains(".m3u8") {
            return Err(StreamError::Pieces);
        }
        let already_whole = have > 0 && status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE;
        if !already_whole {
            if !status.is_success() {
                let message = format!("The server answered {status}");
                // Busy or briefly down: worth another try (M46.46).
                let busy = status.is_server_error()
                    || status == reqwest::StatusCode::TOO_MANY_REQUESTS
                    || status == reqwest::StatusCode::REQUEST_TIMEOUT;
                return Err(if busy {
                    StreamError::Transient(message)
                } else {
                    StreamError::Error(message)
                });
            }
            let resumed = status == reqwest::StatusCode::PARTIAL_CONTENT;
            let start = if resumed { have } else { 0 };
            let total = if resumed {
                response
                    .headers()
                    .get(CONTENT_RANGE)
                    .and_then(|value| value.to_str().ok())
                    .and_then(|range| range.rsplit('/').next())
                    .and_then(|total| total.parse::<u64>().ok())
            } else {
                response.content_length()
            };
            if total.is_some_and(|total| total > MAX_SONG_BYTES) {
                return Err(StreamError::Error(
                    "The file is too big to be a song".into(),
                ));
            }
            self.progress(&row.id, start, total);
            let written = self
                .write_body(row, &partial, resumed, response, start, total, stop)
                .await;
            let done = match &written {
                Ok(done) => *done,
                Err(_) => self
                    .live
                    .lock()
                    .expect("live poisoned")
                    .get(&row.id)
                    .map_or(start, |live| live.done),
            };
            let _ = store::set_progress(
                &self.db(),
                &row.id,
                done as i64,
                total.map(|total| total as i64),
            )
            .await;
            written?;
        }
        let extension = audio_extension(&content_type, url);
        let file = format!("track.{extension}");
        let path = dir.join(&file);
        tokio::fs::rename(&partial, &path)
            .await
            .map_err(|error| StreamError::Device(error.to_string()))?;
        let _ = tokio::fs::remove_file(&marker).await;
        // An error page or a video saved as a song would fail later, when it plays.
        let check = path.clone();
        let audio = tokio::task::spawn_blocking(move || transcode::probe(&check))
            .await
            .map_err(|error| StreamError::Error(error.to_string()))?;
        if let Err(error) = audio {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(StreamError::Error(format!(
                "It gave something that isn't a song ({error})"
            )));
        }
        Ok(Saved::file(file, provider))
    }

    /// Writes the body into the partial file; returns how many bytes the file has.
    #[allow(clippy::too_many_arguments)]
    async fn write_body(
        &self,
        row: &SongDownloadRow,
        partial: &Path,
        resumed: bool,
        response: reqwest::Response,
        start: u64,
        total: Option<u64>,
        stop: &Notify,
    ) -> Result<u64, StreamError> {
        let device = |error: std::io::Error| StreamError::Device(error.to_string());
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(resumed)
            .write(true)
            .truncate(!resumed)
            .open(partial)
            .await
            .map_err(device)?;
        let mut body = response.bytes_stream();
        let mut done = start;
        let mut saved_at = Instant::now();
        loop {
            let next = tokio::select! {
                next = tokio::time::timeout(STALL, body.next()) => next,
                () = stop.notified() => {
                    file.flush().await.map_err(device)?;
                    return Err(StreamError::Stopped);
                }
            };
            // A stream that goes quiet has dropped (M46.46); what came is kept for the retry.
            let Ok(next) = next else {
                file.flush().await.map_err(device)?;
                return Err(StreamError::Transient("The download stalled".into()));
            };
            let Some(chunk) = next else {
                break;
            };
            if self.stopped(&row.id) {
                file.flush().await.map_err(device)?;
                return Err(StreamError::Stopped);
            }
            let chunk = match chunk {
                Ok(chunk) => chunk,
                Err(error) => {
                    file.flush().await.map_err(device)?;
                    return Err(network_failure(error));
                }
            };
            file.write_all(&chunk).await.map_err(device)?;
            done += chunk.len() as u64;
            if done > MAX_SONG_BYTES {
                return Err(StreamError::Error(
                    "The file is too big to be a song".into(),
                ));
            }
            self.progress(&row.id, done, total);
            if saved_at.elapsed() >= PROGRESS_SAVE_EVERY {
                saved_at = Instant::now();
                file.flush().await.map_err(device)?;
                let _ = store::set_progress(
                    &self.db(),
                    &row.id,
                    done as i64,
                    total.map(|total| total as i64),
                )
                .await;
            }
        }
        file.flush().await.map_err(device)?;
        Ok(done)
    }

    /// The running download moved on: kept for lists, and told when what it shows changed.
    fn progress(&self, id: &str, done: u64, total: Option<u64>) {
        let now = Instant::now();
        let item = {
            let mut live = self.live.lock().expect("live poisoned");
            let Some(entry) = live.get_mut(id) else {
                return;
            };
            entry.done = done;
            entry.total = total;
            entry.speed.update(done, now);
            let step = progress_step(done, total);
            if !worth_telling(entry.told, step, now) {
                return;
            }
            entry.told = Some((step, now));
            entry.apply(entry.item.clone())
        };
        self.emit_item(item);
    }

    /// The running download tries `source` now, from the start.
    fn trying(&self, id: &str, source: Option<String>) {
        self.start_attempt(id, SongDownloadKind::File, source);
    }

    /// The running download captures `source`'s stream now (M46.49), or records it from YouTube's
    /// player (`kind` recording, M46.52).
    fn capturing(&self, id: &str, source: &str, kind: SongDownloadKind) {
        self.start_attempt(id, kind, Some(source.to_owned()));
    }

    fn start_attempt(&self, id: &str, kind: SongDownloadKind, source: Option<String>) {
        let item = {
            let mut live = self.live.lock().expect("live poisoned");
            let Some(entry) = live.get_mut(id) else {
                return;
            };
            entry.kind = kind;
            entry.source = source;
            entry.done = 0;
            entry.total = None;
            entry.share = (kind != SongDownloadKind::File).then_some(0.0);
            entry.speed = Speed::default();
            entry.told = None;
            entry.apply(entry.item.clone())
        };
        self.emit_item(item);
    }

    /// A capture moved on: its bytes so far, and how much of the song it holds when known.
    fn capture_progress(&self, id: &str, bytes: u64, share: Option<f64>) {
        let now = Instant::now();
        let item = {
            let mut live = self.live.lock().expect("live poisoned");
            let Some(entry) = live.get_mut(id) else {
                return;
            };
            if bytes > 0 {
                entry.done = bytes;
                entry.speed.update(bytes, now);
            }
            if share.is_some() {
                entry.share = share;
            }
            let step = (entry.share.unwrap_or(0.0) * 100.0) as u64 + entry.done / UNSIZED_STEP;
            if !worth_telling(entry.told, step, now) {
                return;
            }
            entry.told = Some((step, now));
            entry.apply(entry.item.clone())
        };
        self.emit_item(item);
    }

    /// Asks the running download to stop.
    fn stop_running(&self, id: &str, why: Stop) {
        {
            let mut stops = self.stops.lock().expect("stops poisoned");
            // Removing outweighs restarting, which outweighs pausing.
            let stronger = stops.get(id).is_none_or(|asked| rank(why) >= rank(*asked));
            if stronger {
                stops.insert(id.to_owned(), why);
            }
        }
        if let Some(running) = self
            .running
            .lock()
            .expect("running poisoned")
            .values()
            .find(|running| running.id == id)
        {
            running.stop.notify_one();
        }
    }

    fn stopped(&self, id: &str) -> bool {
        self.stops.lock().expect("stops poisoned").contains_key(id)
    }

    fn is_running(&self, id: &str) -> bool {
        self.running
            .lock()
            .expect("running poisoned")
            .values()
            .any(|running| running.id == id)
    }

    /// Where a song put first in the queue goes: before every queued one.
    async fn front(&self) -> i64 {
        let now = now_ms();
        match store::queue_front(&self.db()).await {
            Ok(Some(front)) => front.min(now) - 1,
            _ => now,
        }
    }

    /// Takes a download out, stopping it if it runs; its files are deleted.
    pub async fn remove(&self, id: &str) -> Result<(), String> {
        self.remove_quietly(id).await?;
        self.changed(id);
        Ok(())
    }

    /// Takes several downloads out at once (cancels the waiting ones, clears the failed ones).
    pub async fn remove_many(&self, ids: &[String]) -> Result<(), String> {
        for id in ids {
            self.remove_quietly(id).await?;
        }
        self.changed("");
        Ok(())
    }

    /// Takes a download out. A saved song downloading again only stops doing so: the copy it
    /// kept stays (M46.49), removed as any other with a second go.
    async fn remove_quietly(&self, id: &str) -> Result<(), String> {
        let db = self.db();
        let Some(row) = store::get(&db, id).await.map_err(to_message)? else {
            return Ok(());
        };
        let running = self.is_running(id) || row.state == store::DOWNLOADING;
        if row.replacing.is_some() {
            if running {
                self.stop_running(id, Stop::Kept);
            } else {
                self.keep_copy(id).await.map_err(to_message)?;
            }
            return Ok(());
        }
        // Also when the worker has just picked it up and isn't running it yet.
        if running {
            self.stop_running(id, Stop::Removed);
        }
        self.offline
            .lock()
            .expect("offline index poisoned")
            .remove(&row.song.track_key);
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        self.partial_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        self.to_record
            .lock()
            .expect("recordings poisoned")
            .remove(id);
        store::remove(&db, id).await.map_err(to_message)?;
        self.delete_files(id).await;
        Ok(())
    }

    /// Downloads the song from a source picked for it (M46.47), first in the queue: that stream
    /// first, then the others as usual. What was saved from another stream is dropped, and a
    /// download running now starts again from the pick. A saved song downloads again from it,
    /// keeping its copy until the new one is done (M46.49).
    pub async fn use_source(
        &self,
        id: &str,
        track: OnlineTrack,
        picked: Option<String>,
    ) -> Result<(), String> {
        let db = self.db();
        let row = store::get(&db, id)
            .await
            .map_err(to_message)?
            .ok_or("That download was removed")?;
        // The copy that keeps playing meanwhile.
        let kept: Vec<String> = if row.state == store::DONE {
            let front = self.front().await;
            store::start_replacing(&db, id, None, front, true)
                .await
                .map_err(to_message)?;
            row.file.clone()
        } else {
            row.replacing.clone()
        }
        .into_iter()
        .collect();
        let source = track
            .source
            .ok_or("That answer has no stream of its own: pick one that plays")?;
        let name = picked
            .filter(|name| !name.trim().is_empty())
            .or_else(|| match &source {
                MediaSource::YouTube { .. } => Some(YOUTUBE.into()),
                MediaSource::Url { url, .. } => Some(source_name(url)),
                _ => None,
            });
        let json = serde_json::to_string(&source).map_err(|error| error.to_string())?;
        store::set_picked(&db, id, &json, name.as_deref())
            .await
            .map_err(to_message)?;
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        // Another source: what was found or recorded of the old one goes.
        self.to_record
            .lock()
            .expect("recordings poisoned")
            .remove(id);
        let _ = store::clear_kind(&db, id).await;
        if self.is_running(id) {
            // The worker starts it again from the pick.
            self.stop_running(id, Stop::Restarted);
        } else {
            self.clear_partials(id, &kept).await;
            let front = self.front().await;
            store::set_queued(&db, id, Some(front))
                .await
                .map_err(to_message)?;
            self.wake.notify_one();
        }
        self.announce(id).await;
        Ok(())
    }

    /// Queues a failed download again, first in the queue; it continues with what it saved.
    pub async fn retry(&self, id: &str) -> Result<(), String> {
        let db = self.db();
        let row = store::get(&db, id)
            .await
            .map_err(to_message)?
            .ok_or("That download was removed")?;
        match row.state.as_str() {
            // On its way already.
            store::QUEUED | store::DOWNLOADING => return Ok(()),
            store::DONE => return Err("That song is already downloaded".into()),
            _ => {}
        }
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        // A recording goes on from what it recorded, with its tries counted afresh.
        self.partial_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        let front = self.front().await;
        store::set_queued(&db, id, Some(front))
            .await
            .map_err(to_message)?;
        self.wake_workers();
        self.announce(id).await;
        Ok(())
    }

    /// Queues every failed download again, at the back in their order; returns how many.
    pub async fn retry_failed(&self) -> Result<u32, String> {
        let db = self.db();
        let ids = store::failed(&db).await.map_err(to_message)?;
        let base = now_ms();
        for (index, id) in ids.iter().enumerate() {
            self.auto_retries
                .lock()
                .expect("retries poisoned")
                .remove(id);
            self.partial_retries
                .lock()
                .expect("retries poisoned")
                .remove(id);
            store::set_queued(&db, id, Some(base + index as i64))
                .await
                .map_err(to_message)?;
        }
        if !ids.is_empty() {
            self.wake_workers();
            self.changed("");
        }
        Ok(ids.len() as u32)
    }

    /// Puts the queued downloads among `ids` in that order, ahead of any others.
    pub async fn reorder(&self, ids: &[String]) -> Result<(), String> {
        let db = self.db();
        let front = store::queue_front(&db)
            .await
            .map_err(to_message)?
            .unwrap_or_else(now_ms);
        store::reorder(&db, ids, front - ids.len() as i64)
            .await
            .map_err(to_message)?;
        self.changed("");
        Ok(())
    }

    /// The download of `track`'s song, whatever its state (M46.18: the download button).
    pub async fn find(&self, track: &TrackInput) -> Result<Option<SongDownload>, String> {
        Ok(self.row_for(track).await?.map(|row| self.to_download(row)))
    }

    /// One download as it is now.
    pub async fn get(&self, id: &str) -> Result<Option<SongDownload>, String> {
        Ok(store::get(&self.db(), id)
            .await
            .map_err(to_message)?
            .map(|row| self.to_download(row)))
    }

    /// Every download, newest first.
    pub async fn list(&self) -> Result<Vec<SongDownload>, String> {
        let rows = store::all(&self.db()).await.map_err(to_message)?;
        Ok(rows.into_iter().map(|row| self.to_download(row)).collect())
    }

    /// A row as the UI sees it, with the running download's progress.
    fn to_download(&self, row: SongDownloadRow) -> SongDownload {
        let state = SongDownloadState::parse(&row.state);
        let done = row.done_bytes.max(0) as u64;
        let total = row.total_bytes.map(|total| total.max(0) as u64);
        let item = SongDownload {
            state,
            kind: row.kind.as_deref().and_then(SongDownloadKind::parse),
            // Kept covers show offline (M46.26).
            artwork_url: row
                .song
                .artwork_url
                .as_deref()
                .map(|url| self.images.kept_cover(url, &Default::default())),
            format: row
                .file
                .as_deref()
                .and_then(|file| file.rsplit_once('.'))
                .map(|(_, extension)| extension.to_owned()),
            fraction: match state {
                SongDownloadState::Done => Some(1.0),
                SongDownloadState::Downloading => fraction(done, total),
                _ => None,
            },
            bytes_per_second: None,
            eta_ms: None,
            replacing: row.replacing.is_some(),
            problem: row
                .problem
                .as_deref()
                .filter(|_| state == SongDownloadState::Done)
                .and_then(|json| serde_json::from_str::<SongDownloadProblem>(json).ok()),
            id: row.id,
            track_key: row.song.track_key,
            title: row.song.title,
            artist: row.song.artist,
            album: row.song.album,
            duration_ms: row.song.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
            provider: row.provider,
            done_bytes: done as f64,
            total_bytes: total.map(|total| total as f64),
            error: row.error,
            error_detail: row.error_detail,
            queued_at: row.queued_at as f64,
            finished_at: row.finished_at.map(|at| at as f64),
        };
        if state != SongDownloadState::Downloading {
            return item;
        }
        let live = self.live.lock().expect("live poisoned");
        match live.get(&item.id) {
            Some(live) => live.apply(item),
            None => SongDownload {
                kind: Some(SongDownloadKind::File),
                ..item
            },
        }
    }

    /// What plays of a song, in ms: from its start trim to its end trim, if it has one.
    fn played_part(&self, row: &SongDownloadRow) -> (u64, Option<u64>) {
        let Some(sound) = self.app.try_state::<Arc<crate::sound::SoundService>>() else {
            return (0, None);
        };
        let edit = sound.edit(&crate::sound::key(&TrackQuery {
            title: row.song.title.clone(),
            artist: row.song.artist.clone(),
            album: row.song.album.clone(),
            duration_ms: None,
        }));
        (u64::from(edit.start_ms), edit.end_ms.map(u64::from))
    }

    // ---- Checking saved songs (M46.49) ----

    /// Checks saved songs (`ids`, else all): files gone or damaged, songs given another source or
    /// other names since they downloaded. Each says what's wrong in its `problem`.
    pub async fn check(&self, ids: Option<Vec<String>>) -> Result<SongDownloadsChecked, String> {
        let rows: Vec<SongDownloadRow> = store::all(&self.db())
            .await
            .map_err(to_message)?
            .into_iter()
            .filter(|row| row.state == store::DONE)
            .filter(|row| ids.as_ref().is_none_or(|ids| ids.contains(&row.id)))
            .collect();
        let mut checked = SongDownloadsChecked {
            checked: 0,
            problems: 0,
        };
        let names = self.source_names().await;
        let mut changed = Vec::new();
        for row in rows {
            let problem = self.problem_of(&row, &names).await;
            checked.checked += 1;
            if problem.is_some() {
                checked.problems += 1;
            }
            // A file that's gone, or holds too little of the song, doesn't play offline.
            if problem.as_ref().is_some_and(|problem| {
                matches!(
                    problem.kind,
                    raygun_model::SongDownloadProblemKind::Missing
                        | raygun_model::SongDownloadProblemKind::Partial
                )
            }) {
                self.offline
                    .lock()
                    .expect("offline index poisoned")
                    .remove(&row.song.track_key);
            }
            let json = problem
                .as_ref()
                .and_then(|problem| serde_json::to_string(problem).ok());
            if store::set_problem(&self.db(), &row.id, json.as_deref())
                .await
                .map_err(to_message)?
            {
                changed.push(row.id);
            }
        }
        if changed.len() <= ANNOUNCED {
            for id in &changed {
                self.announce(id).await;
            }
        } else {
            self.changed("");
        }
        Ok(checked)
    }

    /// Checks the saved copy of `track`'s song, if it has one (after its source changed).
    pub async fn check_song(&self, track: &TrackInput) {
        let row = match self.row_for(track).await {
            Ok(Some(row)) if row.state == store::DONE => row,
            _ => return,
        };
        if let Err(error) = self.check(Some(vec![row.id])).await {
            tracing::info!(%error, "couldn't check a downloaded song");
        }
    }

    /// What's wrong with a saved song now, if anything.
    async fn problem_of(
        &self,
        row: &SongDownloadRow,
        names: &check::Names,
    ) -> Option<SongDownloadProblem> {
        let file = row.file.as_deref()?;
        let path = self.dir(&row.id).join(file);
        let recorded = row.total_bytes;
        let damaged = tokio::task::spawn_blocking(move || check::file_problem(&path, recorded))
            .await
            .ok()
            .flatten();
        if damaged.is_some() {
            return damaged;
        }
        if !self.plays_whole(row) {
            return Some(partial_problem(row));
        }
        let picks = raygun_db::streams::candidates(&self.db(), &row.song.track_key)
            .await
            .unwrap_or_default();
        if let Some(problem) = check::source_problem(row, &picks, names) {
            return Some(problem);
        }
        let shown = self
            .app
            .try_state::<Arc<crate::details::DetailService>>()
            .and_then(|details| {
                details.shown_names(
                    &row.song.track_key,
                    &row.song.title,
                    row.song.artist.as_deref(),
                )
            });
        check::details_problem(row, shown)
    }

    /// Downloads saved songs again (`ids`): from the source the song has now when it was given
    /// another, else as any download. A copy that still plays is kept until the new one is done.
    /// Returns how many were queued.
    pub async fn redownload(&self, ids: Vec<String>) -> Result<u32, String> {
        use raygun_model::SongDownloadProblemKind as Kind;

        let db = self.db();
        let front = self.front().await;
        let names = self.source_names().await;
        let mut queued = 0u32;
        for (index, id) in ids.iter().enumerate() {
            let Some(row) = store::get(&db, id).await.map_err(to_message)? else {
                continue;
            };
            if row.state != store::DONE {
                continue;
            }
            let kind = row
                .problem
                .as_deref()
                .and_then(|json| serde_json::from_str::<SongDownloadProblem>(json).ok())
                .map(|problem| problem.kind);
            // The stream the song was given since, and what it's called.
            let picked = match kind {
                Some(Kind::SourceChanged) => {
                    let picks = raygun_db::streams::candidates(&db, &row.song.track_key)
                        .await
                        .unwrap_or_default();
                    check::current_pick(&row, &picks).and_then(|pick| {
                        Some((pick.source.clone()?, check::pick_name(pick, &names)))
                    })
                }
                _ => None,
            };
            let place = front - (ids.len() - index) as i64;
            // A file gone or damaged isn't worth keeping: it downloads as a new song.
            let gone = matches!(kind, Some(Kind::Missing | Kind::Damaged));
            let source = picked.as_ref().map(|(source, _)| source.as_str());
            if !store::start_replacing(&db, id, source, place, !gone)
                .await
                .map_err(to_message)?
            {
                continue;
            }
            match (&picked, kind) {
                (Some((source, name)), _) => {
                    store::set_picked(&db, id, source, Some(name))
                        .await
                        .map_err(to_message)?;
                }
                // Other names: the stream it was asked with was for the old ones; the resolver
                // finds one by the new.
                (None, Some(Kind::DetailsChanged)) => {
                    store::set_source(&db, id, None).await.map_err(to_message)?;
                }
                _ => {}
            }
            if gone {
                self.offline
                    .lock()
                    .expect("offline index poisoned")
                    .remove(&row.song.track_key);
            }
            let _ = store::clear_kind(&db, id).await;
            self.auto_retries
                .lock()
                .expect("retries poisoned")
                .remove(id);
            queued += 1;
        }
        if queued > 0 {
            self.wake.notify_one();
            self.changed("");
        }
        Ok(queued)
    }

    /// Every source's name by its provider id, for saying which one a song has now.
    async fn source_names(&self) -> check::Names {
        let Some(extensions) = self
            .app
            .try_state::<Arc<crate::extensions::ExtensionService>>()
        else {
            return check::Names::new();
        };
        extensions
            .list()
            .await
            .into_iter()
            .flat_map(|extension| extension.providers)
            .map(|provider| (provider.id, provider.name))
            .collect()
    }

    /// The download of `track`'s song: by the names it shows, else by its own (Edit details).
    async fn row_for(&self, track: &TrackInput) -> Result<Option<SongDownloadRow>, String> {
        let db = self.db();
        let key = raygun_resolver::track_key(&query_for(track));
        if let Some(row) = store::by_key(&db, &key).await.map_err(to_message)? {
            return Ok(Some(row));
        }
        let Some(original) = self
            .app
            .try_state::<Arc<crate::details::DetailService>>()
            .and_then(|details| details.original_song_key(&track.title, track.artist.as_deref()))
        else {
            return Ok(None);
        };
        store::by_key(&db, &original).await.map_err(to_message)
    }

    // ---- Writing songs out as files ----

    /// Writes saved songs as files: as downloaded, WAV or FLAC, to `Music/Raygun/<artist>/` or
    /// the picked folder, with their tags and cover.
    pub async fn save(
        &self,
        ids: Vec<String>,
        format: AudioFileFormat,
        destination: SongDestination,
    ) -> Result<SongsSaved, String> {
        let place = self.place(destination).await?;
        // MP3s are encoded at the Downloads audio quality (M46.18); Best is LAME's 320 kbps.
        let mp3_kbps = crate::settings::load(&self.db())
            .await
            .unwrap_or_default()
            .audio_quality
            .download
            .target_kbps()
            .unwrap_or(320);
        let mut saved = SongsSaved {
            saved: Vec::new(),
            failed: Vec::new(),
        };
        for id in ids {
            match self.save_one(&id, format, &place, mp3_kbps).await {
                Ok(shown) => saved.saved.push(shown),
                Err((title, error)) => saved.failed.push(format!("{title}: {error}")),
            }
        }
        Ok(saved)
    }

    async fn place(&self, destination: SongDestination) -> Result<Place, String> {
        match destination {
            SongDestination::Music => Ok(Place::Music),
            SongDestination::Folder => {
                let prefs = crate::settings::load(&self.db())
                    .await
                    .map_err(to_message)?
                    .save;
                crate::export::place_for(&prefs, raygun_model::ExportDestination::Folder)
            }
        }
    }

    async fn save_one(
        &self,
        id: &str,
        format: AudioFileFormat,
        place: &Place,
        mp3_kbps: u32,
    ) -> Result<String, (String, String)> {
        let row = store::get(&self.db(), id)
            .await
            .map_err(|error| (id.to_owned(), error.to_string()))?
            .ok_or_else(|| (id.to_owned(), "This download was removed".to_owned()))?;
        let title = row.song.title.clone();
        let fail = |error: String| (title.clone(), error);
        let file = row
            .file
            .as_deref()
            .filter(|_| row.state == store::DONE)
            .ok_or_else(|| fail("It isn't downloaded yet".into()))?;
        let source = self.dir(id).join(file);
        let cover = match row.song.artwork_url.as_deref() {
            Some(url) => self.cover_bytes(url).await,
            None => None,
        };
        let tags = transcode::SongTags {
            title: row.song.title.clone(),
            artist: row.song.artist.clone(),
            album: row.song.album.clone(),
            cover,
        };
        let outputs = self
            .app
            .state::<Arc<crate::export::ExportService>>()
            .outputs()
            .clone();
        let scratch = self.root.join(FOLDER).join(".out");
        let place = place.clone();
        let artist = row
            .song
            .artist
            .clone()
            .filter(|artist| !artist.trim().is_empty())
            .unwrap_or_else(|| "Unknown artist".into());
        tokio::task::spawn_blocking(move || {
            std::fs::create_dir_all(&scratch).map_err(|error| error.to_string())?;
            let (temp, extension) =
                transcode::write_file(&source, format, &scratch.join(uuid_name()), mp3_kbps)?;
            let tagged = transcode::tag(&temp, &tags);
            if let Err(error) = &tagged {
                tracing::info!(%error, "couldn't tag a saved song");
            }
            let name = crate::export::names::file_name(
                &format!(
                    "{} - {}",
                    tags.artist.as_deref().unwrap_or("Unknown artist"),
                    tags.title
                ),
                &extension,
            );
            let folder = vec![crate::export::names::safe_name(&artist)];
            let mut output = outputs.create(&place, &folder, &name, transcode::mime(&extension))?;
            let copied = std::fs::File::open(&temp)
                .and_then(|mut input| std::io::copy(&mut input, &mut output.file));
            let _ = std::fs::remove_file(&temp);
            let ok = copied.is_ok();
            let saved = outputs.finish(output, ok)?;
            copied.map_err(|error| error.to_string())?;
            Ok::<_, String>(saved.shown)
        })
        .await
        .map_err(|error| fail(error.to_string()))?
        .map_err(fail)
    }

    async fn cover_bytes(&self, url: &str) -> Option<Vec<u8>> {
        let response = self.client.get(url).send().await.ok()?;
        if !response.status().is_success() {
            return None;
        }
        let bytes = response.bytes().await.ok()?;
        (bytes.len() < 8 * 1024 * 1024).then(|| bytes.to_vec())
    }

    /// Keeps a finished song's cover for offline (M46.26), and also writes the song out when
    /// Settings → Downloads asks for it.
    fn save_finished(&self, id: &str) {
        let app = self.app.clone();
        let id = id.to_owned();
        tauri::async_runtime::spawn(async move {
            let service = app.state::<Arc<SongDownloadService>>().inner().clone();
            service.keep_cover(&id).await;
            let Ok(settings) = crate::settings::load(&service.db()).await else {
                return;
            };
            let destination = match settings.save.location {
                SaveLocation::App => return,
                SaveLocation::Downloads => SongDestination::Music,
                SaveLocation::Folder => SongDestination::Folder,
            };
            match service
                .save(vec![id], settings.save.song_format, destination)
                .await
            {
                Ok(saved) if saved.failed.is_empty() => {}
                Ok(saved) => {
                    tracing::warn!(failed = ?saved.failed, "couldn't save a song as a file")
                }
                Err(error) => tracing::warn!(%error, "couldn't save a song as a file"),
            }
        });
    }

    /// Fetches the song's cover into the image cache's kept part, which is never evicted, so it
    /// shows offline. Covers are kept by address, and an album's songs share one, so an album
    /// keeps one file whatever number of its songs is downloaded.
    async fn keep_cover(&self, id: &str) {
        let Ok(Some(row)) = store::get(&self.db(), id).await else {
            return;
        };
        let Some(url) = row.song.artwork_url.as_deref() else {
            return;
        };
        let link = self.images.kept_cover(url, &Default::default());
        if let Err(error) = self
            .client
            .get(&link)
            .send()
            .await
            .and_then(|response| response.error_for_status())
        {
            tracing::info!(%error, "couldn't keep a downloaded song's cover");
        }
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.root.join(FOLDER).join(id)
    }

    async fn delete_files(&self, id: &str) {
        let dir = self.dir(id);
        if tokio::fs::try_exists(&dir).await.unwrap_or(false)
            && let Err(error) = tokio::fs::remove_dir_all(&dir).await
        {
            tracing::warn!(%error, "could not delete a downloaded song");
        }
    }

    /// Something changed: one download (its id), or the list (empty).
    fn changed(&self, id: &str) {
        if let Err(error) = SongDownloadsEvent(id.to_owned()).emit(&self.app) {
            tracing::debug!(%error, "failed to emit song downloads event");
        }
    }

    fn emit_item(&self, item: SongDownload) {
        if let Err(error) = SongDownloadProgressEvent(item).emit(&self.app) {
            tracing::debug!(%error, "failed to emit a song download");
        }
    }

    /// One download changed: says so, with the download as it is now (unless it's gone).
    async fn announce(&self, id: &str) {
        self.changed(id);
        if let Ok(Some(row)) = store::get(&self.db(), id).await {
            self.emit_item(self.to_download(row));
        }
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }
}

/// The saved copy of `item` when its song is downloaded: the item with that file as its source.
/// Library files already are local.
pub fn offline_item(app: &AppHandle, item: &QueueItem) -> Option<QueueItem> {
    if matches!(item.source, MediaSource::LocalFile { .. }) {
        return None;
    }
    let service = app.try_state::<Arc<SongDownloadService>>()?;
    let key = raygun_resolver::track_key(&TrackQuery::for_item(item));
    // A song given other names (Edit details) is saved under its own.
    let path = service.offline_file(&key).or_else(|| {
        let original = app
            .try_state::<Arc<crate::details::DetailService>>()?
            .original_song_key(&item.title, item.artist.as_deref())?;
        service.offline_file(&original)
    })?;
    Some(QueueItem {
        source: MediaSource::LocalFile {
            path: path.to_string_lossy().into_owned(),
        },
        media_kind: MediaKind::Audio,
        ..item.clone()
    })
}

/// Whether `offered` (a stream's title) has every word of the song's own title.
fn covers_title(title: &str, offered: &str) -> bool {
    let offered: HashSet<String> = raygun_resolver::normalize(offered)
        .split(' ')
        .map(str::to_owned)
        .collect();
    raygun_resolver::core_title(title)
        .split(' ')
        .filter(|word| !word.is_empty())
        .all(|word| offered.contains(word))
}

/// Whether `offered` names the song's (first) artist, when it has one: most of its words.
fn names_artist(artist: Option<&str>, offered: &str) -> bool {
    let Some(artist) = artist else {
        return true;
    };
    let first = artist
        .split([',', '&', ';'])
        .next()
        .unwrap_or(artist)
        .split(" feat")
        .next()
        .unwrap_or(artist);
    let wanted: Vec<String> = raygun_resolver::normalize(first)
        .split(' ')
        .filter(|word| !word.is_empty())
        .map(str::to_owned)
        .collect();
    if wanted.is_empty() {
        return true;
    }
    let offered: HashSet<String> = raygun_resolver::normalize(offered)
        .split(' ')
        .map(str::to_owned)
        .collect();
    let named = wanted.iter().filter(|word| offered.contains(*word)).count();
    named * 2 > wanted.len()
}

fn query_for(track: &TrackInput) -> TrackQuery {
    TrackQuery {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: track.album.clone(),
        duration_ms: track.duration_ms,
    }
}

fn uuid_name() -> String {
    uuid::Uuid::now_v7().to_string()
}

/// How far a download is, 0–1, when its size is known.
fn fraction(done: u64, total: Option<u64>) -> Option<f64> {
    total
        .filter(|total| *total > 0)
        .map(|total| (done as f64 / total as f64).min(1.0))
}

/// The step of progress a listener can see: a percent of the file, or 100 KB without a size.
fn progress_step(done: u64, total: Option<u64>) -> u64 {
    match total.filter(|total| *total > 0) {
        Some(total) => done.min(total) * 100 / total,
        None => done / UNSIZED_STEP,
    }
}

/// Whether progress at `step` is worth telling: the first, or another step at least
/// [`PROGRESS_EVERY`] after the last one told.
fn worth_telling(told: Option<(u64, Instant)>, step: u64, now: Instant) -> bool {
    told.is_none_or(|(last, at)| last != step && now.duration_since(at) >= PROGRESS_EVERY)
}

/// Whether a partial file of `have` bytes, started from `started_from`, continues from `url`.
fn continues(started_from: Option<&str>, url: &str, have: u64) -> bool {
    have > 0 && started_from.is_some_and(|from| same_file(from.trim(), url))
}

/// Whether two addresses give one file: the same address, or two of YouTube's for one file
/// (M46.51: they change with every lookup; the format, size and file date they name don't).
fn same_file(a: &str, b: &str) -> bool {
    let identity = |url: &str| -> Option<Vec<(String, String)>> {
        if !raygun_net::wants_range(url) {
            return None;
        }
        let url = reqwest::Url::parse(url).ok()?;
        let mut named: Vec<(String, String)> = url
            .query_pairs()
            .filter(|(name, _)| matches!(name.as_ref(), "itag" | "clen" | "lmt"))
            .map(|(name, value)| (name.into_owned(), value.into_owned()))
            .collect();
        named.sort();
        (named.len() == 3).then_some(named)
    };
    a == b || matches!((identity(a), identity(b)), (Some(was), Some(now)) if was == now)
}

fn rank(stop: Stop) -> u8 {
    match stop {
        Stop::Paused => 0,
        Stop::Restarted => 1,
        Stop::Held => 2,
        Stop::Kept => 3,
        Stop::Removed => 4,
    }
}

/// A stream's name when its source didn't give one: its site, as `audius.co` or `archive.org`.
fn source_name(url: &str) -> String {
    let host = reqwest::Url::parse(url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .unwrap_or_default();
    let labels: Vec<&str> = host.split('.').filter(|label| !label.is_empty()).collect();
    match labels.as_slice() {
        [] => "Its own source".into(),
        [.., site, top] if top.chars().all(|char| char.is_ascii_alphabetic()) => {
            format!("{site}.{top}")
        }
        _ => host,
    }
}

/// What a listener reads when something went wrong: a line, and the sources' own errors.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Message {
    summary: String,
    detail: Option<String>,
}

/// What a download's attempts came to.
#[derive(Debug, Default)]
struct Report {
    /// Sources whose file failed: (name, what happened).
    failed: Vec<(String, String)>,
    /// Sources that stream it only in HLS pieces.
    pieces: Vec<String>,
    /// Streams that couldn't be captured either (M46.49): (name, what happened).
    captured: Vec<(String, String)>,
    /// Sources that only had other songs by that name: (name, what they offered).
    loose: Vec<(String, String)>,
    /// Why the resolver stopped looking.
    resolver: Option<String>,
    /// Why yt-dlp couldn't save YouTube's copy.
    ytdlp: Option<String>,
    /// A failure was the network's: worth trying again later.
    transient: bool,
}

impl Report {
    /// Folds in how one source's file went.
    fn note(&mut self, name: &str, error: &StreamError) {
        match error {
            StreamError::Pieces => self.pieces.push(name.to_owned()),
            StreamError::Transient(message) => {
                self.transient = true;
                self.failed.push((name.to_owned(), message.clone()));
            }
            other => self.failed.push((name.to_owned(), other.text())),
        }
    }

    /// Says in a line what happened: which sources had it, or that none did; `retrying` when it
    /// goes back to the queue by itself. What each source said goes in the detail.
    fn message(&self, retrying: bool) -> Message {
        let failed = unique(self.failed.iter().map(|(name, _)| name.as_str()));
        let captured = unique(self.captured.iter().map(|(name, _)| name.as_str()));
        // Streams that play it but couldn't be saved: in pieces, or their capture failed.
        let playing: Vec<&str> = unique(
            self.pieces
                .iter()
                .map(String::as_str)
                .chain(captured.iter().copied())
                .chain(self.ytdlp.iter().map(|_| YOUTUBE))
                .filter(|name| !failed.contains(name)),
        );
        let tried = unique(failed.iter().chain(playing.iter()).copied());
        let mut summary = if !tried.is_empty() {
            format!("Couldn't download it from {}.", either(&tried))
        } else if !self.loose.is_empty() {
            "No source has this exact song.".to_owned()
        } else {
            "No source has this song.".to_owned()
        };
        if retrying {
            summary.push_str(" Trying again soon.");
        }
        let mut lines: Vec<String> = self
            .failed
            .iter()
            .map(|(name, error)| format!("{name}: {error}"))
            .collect();
        lines.extend(
            unique(self.pieces.iter().map(String::as_str))
                .iter()
                .map(|name| format!("{name}: streams in HLS pieces")),
        );
        lines.extend(
            self.captured
                .iter()
                .map(|(name, error)| format!("{name}, captured: {error}")),
        );
        lines.extend(
            self.loose
                .iter()
                .map(|(name, offered)| format!("{name} offered \u{201c}{offered}\u{201d}")),
        );
        if let Some(error) = &self.ytdlp {
            lines.push(format!("yt-dlp: {error}"));
        }
        if let Some(error) = &self.resolver {
            lines.push(format!("Search: {error}"));
        }
        Message {
            summary,
            detail: (!lines.is_empty()).then(|| lines.join("\n")),
        }
    }
}

/// "Audius", "Audius or YouTube", "Audius, YouTube or 2 more".
fn either(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [first, second] => format!("{first} or {second}"),
        [first, second, rest @ ..] => format!("{first}, {second} or {} more", rest.len()),
    }
}

/// Names in their order, each once.
fn unique<'a>(names: impl Iterator<Item = &'a str>) -> Vec<&'a str> {
    let mut seen = Vec::new();
    for name in names {
        if !seen.contains(&name) {
            seen.push(name);
        }
    }
    seen
}

/// The notification's title and text while `songs` wait for Raygun's window to be recorded.
fn window_notice(songs: u32) -> (String, String) {
    if songs == 1 {
        (
            "1 song waits to be recorded".to_owned(),
            "It's recorded from YouTube's player in Raygun's window. Open Raygun to record it."
                .to_owned(),
        )
    } else {
        (
            format!("{songs} songs wait to be recorded"),
            "They're recorded from YouTube's player in Raygun's window. Open Raygun to record \
             them."
                .to_owned(),
        )
    }
}

/// How a recording that missed parts starts what it says ([`partial_message`]).
const PARTLY_RECORDED: &str = "Partly recorded";

/// What a recording that missed parts says: how much it has, and that the rest comes later.
fn partial_message(covered_ms: u64, needed_ms: Option<u64>, why: &str) -> Message {
    let summary = match needed_ms {
        Some(needed) => format!(
            "{PARTLY_RECORDED}: {} of {}. The rest is recorded later.",
            clock(covered_ms),
            clock(needed)
        ),
        None => format!(
            "{PARTLY_RECORDED}: {} so far. The rest is recorded later.",
            clock(covered_ms)
        ),
    };
    Message {
        summary,
        detail: Some(format!(
            "What was recorded is kept, and only what's missing is recorded again, by itself in a \
             while or with Retry. Until it's whole the song streams. ({why})"
        )),
    }
}

/// What Check downloads says of a copy too short to play instead of the stream: its length and
/// the song's.
fn partial_problem(row: &SongDownloadRow) -> SongDownloadProblem {
    let length = |ms: Option<i64>| ms.and_then(|ms| u64::try_from(ms).ok()).map(clock);
    SongDownloadProblem {
        kind: raygun_model::SongDownloadProblemKind::Partial,
        was: length(row.file_ms),
        now: length(row.song.duration_ms),
        detail: None,
    }
}

/// `m:ss`.
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// What went wrong when the song couldn't be written on this device.
fn device_message(error: &str) -> Message {
    Message {
        summary: "Couldn't save it on this device. Check there's free space.".into(),
        detail: Some(error.to_owned()),
    }
}

/// A saved song file: its name in the download's folder, the name of where it came from, how it
/// was made (`file`, `capture`), its length when known, and its origin.
struct Saved {
    file: String,
    provider: Option<String>,
    kind: &'static str,
    file_ms: Option<u64>,
    origin: Origin,
}

impl Saved {
    fn file(file: String, provider: Option<String>) -> Self {
        Self {
            file,
            provider,
            kind: "file",
            file_ms: None,
            origin: Origin::default(),
        }
    }

    /// The file, from the stream `origin` describes.
    fn from(self, origin: Origin) -> Self {
        Self { origin, ..self }
    }
}

/// A YouTube video of the song (M46.51), saved from after the other sources.
struct YouTubeCopy {
    video_id: String,
    /// Its audio file, when the resolver already asked for it.
    file: Option<MediaSource>,
    origin: Origin,
    /// Its file was tried already: the song was asked with it.
    tried: bool,
}

/// A stream that plays the song but gave no file: captured when nothing else works (M46.49).
#[derive(Debug, Clone, PartialEq)]
struct Candidate {
    name: String,
    source: MediaSource,
    origin: Origin,
}

/// What a download's attempts found (M46.49, M46.51): what each source said, the streams that
/// play the song but gave no file, and the YouTube videos of the song.
#[derive(Default)]
struct Attempts {
    report: Report,
    /// Streams to capture when nothing else works, in the order they came.
    capturable: Vec<Candidate>,
    youtube: Vec<YouTubeCopy>,
}

impl Attempts {
    /// A stream's file didn't save: noted, and the stream kept to capture.
    fn file_failed(&mut self, candidate: Candidate, error: &StreamError) {
        self.report.note(&candidate.name, error);
        self.capture_later(candidate);
    }

    /// The song's own YouTube video didn't save: its audio file, when YouTube gave one (`file`),
    /// plays all the same, so it's captured when nothing else works.
    fn own_video_failed(
        &mut self,
        video_id: &str,
        file: Option<MediaSource>,
        origin: Origin,
        error: &StreamError,
    ) {
        self.report.note(YOUTUBE, error);
        if let Some(source) = &file {
            self.capture_later(Candidate {
                name: YOUTUBE.into(),
                source: source.clone(),
                origin: origin.clone(),
            });
        }
        self.youtube.push(YouTubeCopy {
            video_id: video_id.to_owned(),
            file,
            origin,
            tried: true,
        });
    }

    /// The resolver offered a YouTube video of the song, with its audio file when it asked for
    /// it. A new one is saved after the other sources; for one tried already (the song's own),
    /// the file is captured instead.
    fn video_offered(&mut self, video_id: &str, file: Option<MediaSource>, origin: Origin) {
        let Some(index) = self
            .youtube
            .iter()
            .position(|copy| copy.video_id == video_id)
        else {
            self.youtube.push(YouTubeCopy {
                video_id: video_id.to_owned(),
                file,
                origin,
                tried: false,
            });
            return;
        };
        if self.youtube[index].tried
            && let Some(source) = &file
        {
            let origin = self.youtube[index].origin.clone();
            self.capture_later(Candidate {
                name: YOUTUBE.into(),
                source: source.clone(),
                origin,
            });
        }
        let copy = &mut self.youtube[index];
        copy.file = copy.file.take().or(file);
    }

    /// The YouTube video to record the song from as it plays, when nothing else saved it: the
    /// one it was asked with, else the first the search found.
    fn video_to_record(&self) -> Option<(String, Origin)> {
        self.youtube
            .first()
            .map(|copy| (copy.video_id.clone(), copy.origin.clone()))
    }

    /// The YouTube videos whose audio files are saved now: a few not tried yet, as they came.
    fn videos_to_save(&self) -> Vec<usize> {
        (0..self.youtube.len())
            .filter(|index| !self.youtube[*index].tried)
            .take(YOUTUBE_TRIES)
            .collect()
    }

    /// The video at `index` didn't save; `file` is the audio file it was tried with, if any.
    fn video_failed(&mut self, index: usize, file: Option<MediaSource>, error: &StreamError) {
        self.report.note(YOUTUBE, error);
        let copy = &mut self.youtube[index];
        copy.tried = true;
        copy.file = file.clone();
        let origin = copy.origin.clone();
        if let Some(source) = file {
            self.capture_later(Candidate {
                name: YOUTUBE.into(),
                source,
                origin,
            });
        }
    }

    /// Keeps a stream to capture, once: a web stream (only those can be captured), not kept
    /// already (YouTube gives one file another address with every lookup).
    fn capture_later(&mut self, candidate: Candidate) {
        let MediaSource::Url { url, .. } = &candidate.source else {
            return;
        };
        let kept = self.capturable.iter().any(|known| match &known.source {
            MediaSource::Url { url: known, .. } => same_file(known, url),
            _ => false,
        });
        if !kept {
            self.capturable.push(candidate);
        }
    }

    /// The streams to capture, in turn: streams in pieces first (they play, and were never
    /// files), then as they came (the song's own source first); at most [`CAPTURE_TRIES`].
    fn capture_order(&self) -> Vec<Candidate> {
        let mut order: Vec<&Candidate> = self.capturable.iter().collect();
        order.sort_by_key(|candidate| !self.report.pieces.contains(&candidate.name));
        order.into_iter().take(CAPTURE_TRIES).cloned().collect()
    }
}

/// Captures the streams `to_try` in turn until one is saved (M46.49): a stream whose capture
/// fails gives its turn to the next and is noted in `report`; a stop, or a device that can't write
/// the file (nothing else would either), ends it. `None` when none could be captured.
async fn capture_in_turn<Captured>(
    to_try: Vec<Candidate>,
    report: &mut Report,
    stopped: impl Fn() -> bool,
    mut capture: impl FnMut(Candidate) -> Captured,
) -> Result<Option<Saved>, Failure>
where
    Captured: std::future::Future<Output = Result<Saved, CaptureFailure>>,
{
    for candidate in to_try {
        if stopped() {
            return Err(Failure::Stopped);
        }
        let name = candidate.name.clone();
        match capture(candidate).await {
            Ok(saved) => return Ok(Some(saved)),
            Err(CaptureFailure::Stopped) => return Err(Failure::Stopped),
            Err(CaptureFailure::Device(error)) => {
                return Err(Failure::Error(device_message(&error)));
            }
            // A stream's capture keeps nothing between tries, so it never ends partial.
            Err(CaptureFailure::Failed(error) | CaptureFailure::Partial { why: error, .. }) => {
                tracing::info!(source = %name, %error, "couldn't capture a stream");
                report.captured.push((name, error));
            }
            // Only YouTube's player's captures are interrupted; a stream's never is.
            Err(CaptureFailure::Interrupted) => {
                report
                    .captured
                    .push((name, "The capture stopped".to_owned()));
            }
        }
    }
    Ok(None)
}

enum CaptureFailure {
    Stopped,
    Device(String),
    Failed(String),
    /// A recording had to stop: Raygun's window went, or Raygun's player started where hearing
    /// YouTube's player takes the audio focus.
    Interrupted,
    /// A recording has some of the song, kept for the next try (M46.52).
    Partial {
        covered_ms: u64,
        needed_ms: Option<u64>,
        why: String,
    },
}

enum Failure {
    Error(Message),
    /// The network or a busy server (M46.46): tried again later.
    Transient(Message),
    /// Removed, paused or given another source while it ran.
    Stopped,
    /// It plays only in YouTube's player: it goes to the recordings' queue, to be recorded from
    /// this video, found there (M46.52).
    ToRecord(Box<(String, Origin)>),
    /// A recording can't go on now: it waits in its place.
    Recording(RecordWait),
    /// A recording has some of the song, kept; the rest is recorded later.
    Partial(Message),
}

/// How one stream went.
enum StreamError {
    /// It streams in HLS pieces: it plays, but can't be saved as one file.
    Pieces,
    Error(String),
    /// The network or a busy server: worth another try.
    Transient(String),
    /// The file couldn't be written here (no room, no access): other sources would fail too.
    Device(String),
    /// The download was asked to stop.
    Stopped,
}

impl StreamError {
    fn text(&self) -> String {
        match self {
            Self::Pieces => "It streams in HLS pieces".into(),
            Self::Error(message) | Self::Transient(message) | Self::Device(message) => {
                message.clone()
            }
            Self::Stopped => "Stopped".into(),
        }
    }
}

fn capture_failure(error: capture::CaptureError) -> CaptureFailure {
    match error {
        capture::CaptureError::Stopped => CaptureFailure::Stopped,
        capture::CaptureError::Device(error) => CaptureFailure::Device(error),
        capture::CaptureError::Failed(error) => CaptureFailure::Failed(error),
        capture::CaptureError::Interrupted => CaptureFailure::Interrupted,
        capture::CaptureError::Partial {
            covered_ms,
            needed_ms,
            why,
        } => CaptureFailure::Partial {
            covered_ms,
            needed_ms,
            why,
        },
    }
}

fn network_failure(error: reqwest::Error) -> StreamError {
    StreamError::Transient(format!("The download stopped: {error}"))
}

/// A song file's extension from its type, else its address; MP3 when neither says.
fn audio_extension(content_type: &str, url: &str) -> &'static str {
    const BY_TYPE: &[(&str, &str)] = &[
        ("audio/mpeg", "mp3"),
        ("audio/mp3", "mp3"),
        ("audio/mp4", "m4a"),
        ("audio/x-m4a", "m4a"),
        ("audio/aac", "aac"),
        ("audio/aacp", "aac"),
        ("audio/flac", "flac"),
        ("audio/x-flac", "flac"),
        ("audio/ogg", "ogg"),
        ("audio/vorbis", "ogg"),
        ("audio/opus", "opus"),
        ("audio/wav", "wav"),
        ("audio/x-wav", "wav"),
        ("audio/wave", "wav"),
        ("audio/webm", "webm"),
    ];
    if let Some((_, extension)) = BY_TYPE
        .iter()
        .find(|(kind, _)| content_type.starts_with(kind))
    {
        return extension;
    }
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    ["mp3", "m4a", "aac", "flac", "ogg", "opus", "wav", "webm"]
        .into_iter()
        .find(|extension| path.ends_with(&format!(".{extension}")))
        .unwrap_or("mp3")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn saved_copies_must_carry_the_whole_title() {
        assert!(covers_title(
            "Ghostbusters",
            "01 - Ray Parker Jr - Ghostbusters"
        ));
        assert!(covers_title(
            "Hey Jude (Remastered)",
            "The Beatles - Hey Jude"
        ));
        assert!(!covers_title("Get Lucky", "Lucky Star"));
        assert!(names_artist(
            Some("Ray Parker Jr."),
            "01 - Ray Parker Jr - Ghostbusters"
        ));
        assert!(names_artist(Some("BTS"), "BTS - SWIM"));
        assert!(names_artist(
            Some("Daft Punk, Pharrell Williams"),
            "Daft Punk - Get Lucky"
        ));
        assert!(!names_artist(Some("BTS"), "Swim"));
        assert!(names_artist(None, "Swim"));
    }

    #[test]
    fn extensions_come_from_the_type_then_the_address() {
        assert_eq!(audio_extension("audio/mpeg", "https://x/a"), "mp3");
        assert_eq!(
            audio_extension("audio/mp4; codecs=mp4a", "https://x/a"),
            "m4a"
        );
        assert_eq!(
            audio_extension("application/octet-stream", "https://x/a.flac?x=1"),
            "flac"
        );
        assert_eq!(audio_extension("", "https://x/a"), "mp3");
    }

    #[test]
    fn progress_is_told_four_times_a_second_at_most_and_only_when_it_shows() {
        let start = Instant::now();
        let later = |ms: u64| start + Duration::from_millis(ms);
        // A percent of a known size, else 100 KB.
        assert_eq!(progress_step(450, Some(1_000)), 45);
        assert_eq!(progress_step(2_000, Some(1_000)), 100);
        assert_eq!(progress_step(250 * 1024, None), 2);
        assert_eq!(progress_step(10, Some(0)), 0);

        assert!(worth_telling(None, 0, start), "the first is told");
        assert!(
            !worth_telling(Some((45, start)), 46, later(100)),
            "too soon after the last"
        );
        assert!(worth_telling(Some((45, start)), 46, later(260)));
        assert!(
            !worth_telling(Some((45, start)), 45, later(5_000)),
            "nothing new to show"
        );

        // Bytes of 1% steps arriving every 10 ms for 2 s are told 8 times or so, not 200.
        let mut told = None;
        let mut count = 0;
        for tick in 0..200_u64 {
            let now = later(tick * 10);
            let step = progress_step(tick * 10, Some(2_000));
            if worth_telling(told, step, now) {
                told = Some((step, now));
                count += 1;
            }
        }
        assert!((8..=9).contains(&count), "told {count} times");
    }

    #[test]
    fn speed_and_time_left_follow_the_bytes() {
        let start = Instant::now();
        let mut speed = Speed::default();
        speed.update(0, start);
        assert_eq!(speed.bytes_per_second, None, "nothing measured yet");
        speed.update(100_000, start + Duration::from_millis(200));
        assert_eq!(speed.bytes_per_second, None, "too short to measure");
        speed.update(500_000, start + Duration::from_secs(1));
        assert_eq!(speed.bytes_per_second, Some(500_000.0));
        assert_eq!(speed.eta_ms(500_000, Some(1_500_000)), Some(2_000.0));
        assert_eq!(speed.eta_ms(500_000, None), None, "no size, no time left");
        // Another stream starts over.
        speed.update(0, start + Duration::from_secs(2));
        assert_eq!(speed.bytes_per_second, None);
    }

    #[test]
    fn the_running_download_shows_its_source_progress_and_time_left() {
        let item = SongDownload {
            id: "d".into(),
            track_key: "a|song".into(),
            title: "Song".into(),
            artist: Some("A".into()),
            album: None,
            duration_ms: Some(200_000),
            artwork_url: None,
            state: SongDownloadState::Queued,
            kind: None,
            provider: Some("Picked".into()),
            format: None,
            done_bytes: 0.0,
            total_bytes: None,
            fraction: None,
            bytes_per_second: None,
            eta_ms: None,
            error: None,
            error_detail: None,
            replacing: false,
            problem: None,
            queued_at: 1.0,
            finished_at: None,
        };
        let start = Instant::now();
        let mut live = Live {
            item: item.clone(),
            kind: SongDownloadKind::File,
            done: 0,
            total: None,
            share: None,
            source: None,
            speed: Speed::default(),
            told: None,
            recording: false,
        };
        // Before a source answers: the picked one, nothing known yet.
        let shown = live.apply(item.clone());
        assert_eq!(shown.state, SongDownloadState::Downloading);
        assert_eq!(shown.kind, Some(SongDownloadKind::File));
        assert_eq!(shown.provider.as_deref(), Some("Picked"));
        assert_eq!((shown.fraction, shown.eta_ms), (None, None));

        live.source = Some("Indie Tracks".into());
        live.total = Some(4_000_000);
        live.speed.update(0, start);
        live.done = 1_000_000;
        live.speed.update(1_000_000, start + Duration::from_secs(1));
        let shown = live.apply(item.clone());
        assert_eq!(shown.provider.as_deref(), Some("Indie Tracks"));
        assert_eq!(shown.fraction, Some(0.25));
        assert_eq!(shown.bytes_per_second, Some(1_000_000.0));
        assert_eq!(shown.eta_ms, Some(3_000.0));
        assert_eq!(
            (shown.done_bytes, shown.total_bytes),
            (1_000_000.0, Some(4_000_000.0))
        );

        // A capture shows how much of the song it holds (M46.49).
        live.kind = SongDownloadKind::Capture;
        live.share = Some(0.4);
        let shown = live.apply(item.clone());
        assert_eq!(shown.kind, Some(SongDownloadKind::Capture));
        assert_eq!(shown.state, SongDownloadState::Downloading);
        assert_eq!((shown.fraction, shown.eta_ms), (Some(0.4), None));

        // The recordings' worker's song shows as a recording, whatever it tries (M46.52).
        live.recording = true;
        live.kind = SongDownloadKind::File;
        assert_eq!(live.apply(item).kind, Some(SongDownloadKind::Recording));
    }

    #[test]
    fn partial_files_continue_only_from_their_own_stream() {
        assert!(continues(Some("https://a/x.mp3"), "https://a/x.mp3", 10));
        assert!(continues(Some("https://a/x.mp3\n"), "https://a/x.mp3", 10));
        assert!(!continues(Some("https://b/y.mp3"), "https://a/x.mp3", 10));
        assert!(
            !continues(None, "https://a/x.mp3", 10),
            "a partial file of unknown origin starts over"
        );
        assert!(!continues(Some("https://a/x.mp3"), "https://a/x.mp3", 0));
        // YouTube's addresses change with each lookup; one file's go on from each other.
        let youtube = |host: &str, lmt: &str| {
            format!(
                "https://{host}.googlevideo.com/videoplayback?expire=1&itag=140&clen=5983881&lmt={lmt}&sig=x"
            )
        };
        assert!(continues(
            Some(&youtube("rr1---sn-a", "1735")),
            &youtube("rr5---sn-b", "1735"),
            10
        ));
        assert!(!continues(
            Some(&youtube("rr1---sn-a", "1735")),
            &youtube("rr1---sn-a", "1999"),
            10
        ));
    }

    #[test]
    fn a_recording_that_missed_parts_says_what_it_has_and_that_the_rest_comes_later() {
        let message = partial_message(
            130_000,
            Some(220_000),
            "The stream stops at 2:10 every time",
        );
        assert_eq!(
            message.summary,
            "Partly recorded: 2:10 of 3:40. The rest is recorded later."
        );
        assert!(message.summary.starts_with(PARTLY_RECORDED));
        let detail = message.detail.unwrap();
        assert!(detail.contains("only what's missing is recorded again"));
        assert!(detail.ends_with("(The stream stops at 2:10 every time)"));
        assert_eq!(
            partial_message(65_000, None, "x").summary,
            "Partly recorded: 1:05 so far. The rest is recorded later."
        );
        // Check downloads says how much a copy too short to play holds.
        let row = SongDownloadRow {
            id: "1".into(),
            song: NewSong {
                track_key: "a|b".into(),
                title: "B".into(),
                artist: Some("A".into()),
                album: None,
                duration_ms: Some(200_000),
                artwork_url: None,
                source: None,
            },
            state: store::DONE.into(),
            kind: Some(store::RECORDING.into()),
            provider: None,
            file: Some("track.flac".into()),
            done_bytes: 0,
            total_bytes: None,
            error: None,
            error_detail: None,
            created_at: 0,
            queued_at: 0,
            finished_at: Some(1),
            origin: Origin::default(),
            file_ms: Some(30_000),
            replacing: None,
            problem: None,
        };
        assert_eq!(
            partial_problem(&row),
            SongDownloadProblem {
                kind: raygun_model::SongDownloadProblemKind::Partial,
                was: Some("0:30".into()),
                now: Some("3:20".into()),
                detail: None,
            }
        );
    }

    #[test]
    fn streams_are_named_by_their_site() {
        assert_eq!(
            source_name("https://api.indie.example/v1/tracks/x/stream"),
            "indie.example"
        );
        assert_eq!(
            source_name("https://archive.org/download/a/b.mp3"),
            "archive.org"
        );
        assert_eq!(source_name("http://10.0.2.2:8765/a.mp3"), "10.0.2.2");
        assert_eq!(source_name("not an address"), "Its own source");
    }

    /// A YouTube audio file as song downloads save it (M46.51): it reads, and an M4A also takes
    /// tags and converts. `YOUTUBE_AUDIO_FILE=<file> cargo test -p raygun-app -- --ignored
    /// youtube_file_live --nocapture`.
    #[test]
    #[ignore = "needs a YouTube audio file (YOUTUBE_AUDIO_FILE)"]
    fn youtube_file_live_reads_tags_and_converts() {
        let source =
            PathBuf::from(std::env::var("YOUTUBE_AUDIO_FILE").expect("YOUTUBE_AUDIO_FILE"));
        let extension = source
            .extension()
            .and_then(|extension| extension.to_str())
            .unwrap()
            .to_owned();
        let temp = tempfile::TempDir::new().unwrap();
        let saved = temp.path().join(format!("track.{extension}"));
        std::fs::copy(&source, &saved).unwrap();
        transcode::probe(&saved).unwrap();
        // Opus has no decoder here: it reads by its container, its length unknown to Raygun.
        let measured = transcode::measure(&saved).unwrap();
        println!("{extension}: {measured:?} ms");
        if extension == "m4a" {
            let ms = measured.expect("its length");
            assert!(ms > 60_000, "{ms}");
            let tags = transcode::SongTags {
                title: "Get Lucky".into(),
                artist: Some("Daft Punk".into()),
                album: None,
                cover: None,
            };
            transcode::tag(&saved, &tags).unwrap();
            transcode::probe(&saved).unwrap();
            let (wav, _) =
                transcode::write_file(&saved, AudioFileFormat::Wav, &temp.path().join("out"), 320)
                    .unwrap();
            let wav_ms = transcode::measure(&wav).unwrap().unwrap();
            println!("as WAV: {wav_ms} ms");
            assert!(wav_ms.abs_diff(ms) < 1_000, "{wav_ms} vs {ms}");
        }
    }

    #[test]
    fn failures_say_in_a_line_what_happened() {
        // Sources had it and their files failed, YouTube's among them.
        let mut report = Report {
            resolver: Some("No playable source found for \u{201c}Swim\u{201d}".into()),
            ..Report::default()
        };
        report.note(
            "Indie Tracks",
            &StreamError::Error("The server answered 403 Forbidden".into()),
        );
        report.note(
            "Indie Tracks",
            &StreamError::Error("The server answered 404 Not Found".into()),
        );
        report.note(
            YOUTUBE,
            &StreamError::Error("YouTube didn't give a stream for it".into()),
        );
        let message = report.message(false);
        assert_eq!(
            message.summary,
            "Couldn't download it from Indie Tracks or YouTube."
        );
        assert_eq!(
            message.detail.as_deref(),
            Some(
                "Indie Tracks: The server answered 403 Forbidden\nIndie Tracks: The server answered 404 Not \
                 Found\nYouTube: YouTube didn't give a stream for it\nSearch: No playable source \
                 found for \u{201c}Swim\u{201d}"
            )
        );

        // yt-dlp couldn't save YouTube's copy either (desktop).
        let report = Report {
            ytdlp: Some("Sign in to confirm you're not a bot".into()),
            ..Report::default()
        };
        let message = report.message(false);
        assert_eq!(message.summary, "Couldn't download it from YouTube.");
        assert_eq!(
            message.detail.as_deref(),
            Some("yt-dlp: Sign in to confirm you're not a bot")
        );

        // Different songs by that name, and nothing else.
        let report = Report {
            loose: vec![("Indie Tracks".into(), "Swim (cover)".into())],
            ..Report::default()
        };
        let message = report.message(false);
        assert_eq!(message.summary, "No source has this exact song.");
        assert_eq!(
            message.detail.as_deref(),
            Some("Indie Tracks offered \u{201c}Swim (cover)\u{201d}")
        );

        // A stream in pieces that couldn't be captured (M46.49).
        let mut report = Report {
            captured: vec![("Radio".into(), "3 of 40 pieces wouldn't download".into())],
            ..Report::default()
        };
        report.note("Radio", &StreamError::Pieces);
        let message = report.message(false);
        assert_eq!(message.summary, "Couldn't download it from Radio.");
        assert_eq!(
            message.detail.as_deref(),
            Some("Radio: streams in HLS pieces\nRadio, captured: 3 of 40 pieces wouldn't download")
        );

        // The network: it goes back to the queue while tries are left.
        let mut report = Report::default();
        report.note(
            "Indie Tracks",
            &StreamError::Transient("The download stalled".into()),
        );
        assert!(report.transient);
        assert_eq!(
            report.message(true).summary,
            "Couldn't download it from Indie Tracks. Trying again soon."
        );
        assert_eq!(
            report.message(false).summary,
            "Couldn't download it from Indie Tracks."
        );

        // Nothing anywhere.
        assert_eq!(
            Report::default().message(false).summary,
            "No source has this song."
        );
        assert_eq!(either(&["A", "B", "C", "D"]), "A, B or 2 more");

        // No message ever says a song is only on YouTube or can't be saved from it.
        for report in [Report::default(), report] {
            let summary = report.message(false).summary.to_lowercase();
            assert!(!summary.contains("only youtube"), "{summary}");
            assert!(!summary.contains("phones"), "{summary}");
        }
    }

    fn web(url: &str) -> MediaSource {
        MediaSource::Url {
            url: url.into(),
            headers: Default::default(),
        }
    }

    fn candidate(name: &str, url: &str) -> Candidate {
        Candidate {
            name: name.into(),
            source: web(url),
            origin: Origin::default(),
        }
    }

    /// One of YouTube's addresses for a video's audio file (they differ with each lookup).
    fn googlevideo(host: &str) -> String {
        format!(
            "https://{host}.googlevideo.com/videoplayback?expire=1&itag=140&clen=5983881&lmt=1735&sig=x"
        )
    }

    fn saved(file: &str) -> Saved {
        Saved {
            file: file.into(),
            provider: None,
            kind: "capture",
            file_ms: None,
            origin: Origin::default(),
        }
    }

    #[test]
    fn the_songs_own_youtube_audio_is_captured_when_its_file_wont_save() {
        // Without yt-dlp (Android), capture is all that's left once the audio file of the video
        // the song was asked with doesn't save: that file plays, so it's captured, as the song's.
        let mut attempts = Attempts::default();
        let own = Origin {
            provider: Some(OWN.into()),
            ..Origin::default()
        };
        attempts.own_video_failed(
            "dQw4w9WgXcQ",
            Some(web(&googlevideo("rr1---sn-a"))),
            own.clone(),
            &StreamError::Transient("The download stalled".into()),
        );
        // The resolver offers that video again, its file at another address: captured once.
        attempts.video_offered(
            "dQw4w9WgXcQ",
            Some(web(&googlevideo("rr5---sn-b"))),
            Origin::default(),
        );
        // Another video of the song is saved after the other sources, not captured yet.
        attempts.video_offered("another", None, Origin::default());
        assert_eq!(attempts.videos_to_save(), [1]);
        let order = attempts.capture_order();
        assert_eq!(order.len(), 1, "{order:?}");
        assert_eq!(order[0].name, YOUTUBE);
        assert_eq!(order[0].source, web(&googlevideo("rr1---sn-a")));
        assert_eq!(order[0].origin, own);
        assert!(attempts.report.transient);
        assert_eq!(
            attempts.report.message(false).summary,
            "Couldn't download it from YouTube."
        );
    }

    #[test]
    fn a_file_found_later_for_the_songs_own_video_is_captured() {
        // YouTube gave no file when the song's own video was asked for; the search found one.
        let mut attempts = Attempts::default();
        attempts.own_video_failed(
            "v",
            None,
            Origin::default(),
            &StreamError::Error("YouTube didn't answer".into()),
        );
        assert!(attempts.capture_order().is_empty());
        attempts.video_offered("v", Some(web(&googlevideo("rr1"))), Origin::default());
        assert!(
            attempts.videos_to_save().is_empty(),
            "the video tried already isn't saved again"
        );
        assert_eq!(attempts.capture_order().len(), 1);
    }

    #[test]
    fn other_videos_files_are_captured_once_they_fail_to_save() {
        let mut attempts = Attempts::default();
        attempts.video_offered("a", None, Origin::default());
        attempts.video_offered("b", None, Origin::default());
        assert_eq!(attempts.videos_to_save(), [0, 1]);
        // "a"'s file was asked for as it was saved, and didn't save: captured. "b" had none.
        attempts.video_failed(
            0,
            Some(web("https://a.example/audio.m4a")),
            &StreamError::Transient("The download stalled".into()),
        );
        attempts.video_failed(
            1,
            None,
            &StreamError::Error("YouTube gave no file for it".into()),
        );
        assert!(attempts.videos_to_save().is_empty());
        let order = attempts.capture_order();
        assert_eq!(
            order.iter().map(|each| &each.source).collect::<Vec<_>>(),
            [&web("https://a.example/audio.m4a")]
        );
    }

    #[test]
    fn streams_are_captured_in_pieces_first_then_as_they_came_each_once() {
        let mut attempts = Attempts::default();
        attempts.file_failed(
            candidate("Own", "https://own.example/a.mp3"),
            &StreamError::Error("The server answered 403 Forbidden".into()),
        );
        attempts.file_failed(
            candidate("Indie", "https://indie.example/a.mp3"),
            &StreamError::Transient("The download stalled".into()),
        );
        attempts.file_failed(
            candidate("Radio", "https://radio.example/a.m3u8"),
            &StreamError::Pieces,
        );
        // The same stream again, and an embed, which can't be captured: neither is kept.
        attempts.file_failed(
            candidate("Indie", "https://indie.example/a.mp3"),
            &StreamError::Transient("The download stalled".into()),
        );
        attempts.file_failed(
            Candidate {
                name: YOUTUBE.into(),
                source: MediaSource::YouTube {
                    video_id: "v".into(),
                },
                origin: Origin::default(),
            },
            &StreamError::Error("It plays only in YouTube's player".into()),
        );
        attempts.file_failed(
            candidate("More", "https://more.example/a.mp3"),
            &StreamError::Error("The server answered 404 Not Found".into()),
        );
        let names: Vec<String> = attempts
            .capture_order()
            .into_iter()
            .map(|each| each.name)
            .collect();
        assert_eq!(names, ["Radio", "Own", "Indie"], "at most {CAPTURE_TRIES}");
        assert_eq!(attempts.report.pieces, ["Radio"]);
        assert_eq!(attempts.report.failed.len(), 5);
    }

    #[tokio::test]
    async fn a_capture_that_fails_gives_its_turn_to_the_next_stream() {
        let mut report = Report::default();
        let tried = std::cell::RefCell::new(Vec::new());
        let result = capture_in_turn(
            vec![
                candidate("Radio", "https://radio.example/a.m3u8"),
                candidate("Own", "https://own.example/a.mp3"),
                candidate("Indie", "https://indie.example/a.mp3"),
            ],
            &mut report,
            || false,
            |candidate| {
                tried.borrow_mut().push(candidate.name.clone());
                let result = if candidate.name == "Radio" {
                    Err(CaptureFailure::Failed(
                        "No sound came from the stream".into(),
                    ))
                } else {
                    Ok(saved("track.flac"))
                };
                async move { result }
            },
        )
        .await;
        assert!(matches!(result, Ok(Some(saved)) if saved.file == "track.flac"));
        assert_eq!(*tried.borrow(), ["Radio", "Own"], "the third isn't needed");
        assert_eq!(
            report.captured,
            [(
                "Radio".to_owned(),
                "No sound came from the stream".to_owned()
            )]
        );
        assert_eq!(
            report.message(false).detail.as_deref(),
            Some("Radio, captured: No sound came from the stream")
        );
    }

    #[tokio::test]
    async fn captures_end_on_a_stop_or_a_device_that_cant_write() {
        let streams = || {
            vec![
                candidate("Radio", "https://radio.example/a.m3u8"),
                candidate("Own", "https://own.example/a.mp3"),
            ]
        };
        // The device can't write: the next stream would fail the same way.
        let mut report = Report::default();
        let calls = std::cell::Cell::new(0);
        let result = capture_in_turn(
            streams(),
            &mut report,
            || false,
            |_| {
                calls.set(calls.get() + 1);
                async { Err(CaptureFailure::Device("No space left on device".into())) }
            },
        )
        .await;
        match result {
            Err(Failure::Error(message)) => {
                assert_eq!(message.detail.as_deref(), Some("No space left on device"));
            }
            _ => panic!("a device error ends the download"),
        }
        assert_eq!(calls.get(), 1);

        // Stopped during a capture, or before the next one starts.
        let mut report = Report::default();
        let result = capture_in_turn(
            streams(),
            &mut report,
            || false,
            |_| async { Err(CaptureFailure::Stopped) },
        )
        .await;
        assert!(matches!(result, Err(Failure::Stopped)));
        let result = capture_in_turn(
            streams(),
            &mut report,
            || true,
            |_| async { Ok(saved("track.flac")) },
        )
        .await;
        assert!(matches!(result, Err(Failure::Stopped)));

        // Every capture failed: the download says so (nothing saved, nothing stopped).
        let mut report = Report::default();
        let result = capture_in_turn(
            streams(),
            &mut report,
            || false,
            |candidate| {
                let error = format!("{} didn't start", candidate.name);
                async move { Err(CaptureFailure::Failed(error)) }
            },
        )
        .await;
        assert!(matches!(result, Ok(None)));
        assert_eq!(report.captured.len(), 2);
        assert_eq!(
            report.message(false).summary,
            "Couldn't download it from Radio or Own."
        );
        // Nothing to capture.
        let result = capture_in_turn(
            Vec::new(),
            &mut report,
            || false,
            |_| async { Ok(saved("never")) },
        )
        .await;
        assert!(matches!(result, Ok(None)));
    }
}
