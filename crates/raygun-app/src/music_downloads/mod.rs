//! Songs saved into the app to play offline (M46.10). A queue in `local_music_download`, worked
//! one song at a time: a source's own file downloads with Range resume to
//! `downloads/music/<id>/track.<ext>`; a song no source gives a file of (YouTube's embed has none
//! to fetch) can be recorded as it plays, where the platform allows. Playback swaps in the saved
//! copy by the song's key, so a downloaded song plays offline wherever it appears. Saved songs can
//! also be written out as files, as downloaded, WAV or FLAC.
//!
//! The queue can be paused, put in any order, and songs tried again or given another source go
//! first. Every change is pushed at once (`SongDownloadsEvent`, and the download itself as
//! `SongDownloadProgressEvent`); progress at most four times a second, and only when what it shows
//! moved on.

pub mod record;
pub mod transcode;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use raygun_db::music_downloads::{self as store, NewSong, SongDownloadRow};
use raygun_model::{
    AudioFileFormat, MediaKind, MediaSource, OnlineTrack, PlaybackControl, PlaybackState,
    PlaybackStatus, QueueItem, QueueItemInput, SaveLocation, SongDestination, SongDownload,
    SongDownloadKind, SongDownloadState, SongsSaved, TrackInput, TrackQuery,
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
use crate::playback::PlaybackService;
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

/// Why the running download must stop now.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Stop {
    /// Downloads were paused: it waits in its place, keeping what it saved.
    Paused,
    /// Another source was picked: it starts again from that one, first in the queue.
    Restarted,
    /// Taken out of the downloads: its files are deleted.
    Removed,
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
    done: u64,
    total: Option<u64>,
    /// The source being tried now.
    source: Option<String>,
    speed: Speed,
    /// The step of progress last told, and when.
    told: Option<(u64, Instant)>,
}

impl Live {
    /// `item` as it is now.
    fn apply(&self, item: SongDownload) -> SongDownload {
        SongDownload {
            state: SongDownloadState::Downloading,
            kind: Some(SongDownloadKind::File),
            provider: self.source.clone().or(item.provider),
            done_bytes: self.done as f64,
            total_bytes: self.total.map(|total| total as f64),
            fraction: fraction(self.done, self.total),
            bytes_per_second: self.speed.bytes_per_second,
            eta_ms: self.speed.eta_ms(self.done, self.total),
            ..item
        }
    }
}

/// The recording running now: which download, the capture, and the queue item it follows.
struct Recording {
    id: String,
    capture: record::Capture,
    item_id: String,
    /// The speed to go back to when it's done (recordings play at 1x).
    rate: f64,
    /// Whether the item has started playing.
    started: bool,
    /// The download as it was when recording started.
    item: SongDownload,
    /// What plays of the song (its trims), in ms.
    from_ms: f64,
    to_ms: Option<f64>,
    /// Where the song is now.
    position_ms: f64,
    told: Option<(u64, Instant)>,
}

impl Recording {
    /// How much of what plays has played, 0–1, and the time left.
    fn progress(&self) -> (Option<f64>, Option<f64>) {
        let Some(to) = self.to_ms.filter(|to| *to > self.from_ms) else {
            return (None, None);
        };
        let played = (self.position_ms - self.from_ms).clamp(0.0, to - self.from_ms);
        (
            Some(played / (to - self.from_ms)),
            Some((to - self.from_ms - played).max(0.0)),
        )
    }

    fn apply(&self, item: SongDownload) -> SongDownload {
        let (fraction, eta_ms) = self.progress();
        SongDownload {
            state: SongDownloadState::Recording,
            kind: Some(SongDownloadKind::Recording),
            fraction,
            eta_ms,
            bytes_per_second: None,
            ..item
        }
    }
}

/// How a recording ended.
#[derive(Debug, Clone, PartialEq, Eq)]
enum RecordingEnd {
    /// The song played to its end: the recording is kept.
    Ended,
    /// Stopped from its window.
    Cancelled,
    /// The song didn't play (its error, when the player said).
    Failed(Option<String>),
    /// The download was removed.
    Removed,
}

pub struct SongDownloadService {
    app: AppHandle,
    /// The downloads folder.
    root: PathBuf,
    client: reqwest::Client,
    images: ImageLinks,
    wake: Notify,
    paused: AtomicBool,
    running: Mutex<Option<Running>>,
    /// Stops asked of the running download.
    stops: Mutex<HashMap<String, Stop>>,
    live: Mutex<HashMap<String, Live>>,
    /// Finished songs by key: the saved file. Playback asks it for every song it loads.
    offline: Mutex<HashMap<String, PathBuf>>,
    recording: Mutex<Option<Recording>>,
    /// Network retries already made, by download (M46.46); reset when one succeeds.
    auto_retries: Mutex<HashMap<String, usize>>,
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
            paused: AtomicBool::new(false),
            running: Mutex::new(None),
            stops: Mutex::new(HashMap::new()),
            live: Mutex::new(HashMap::new()),
            offline: Mutex::new(HashMap::new()),
            recording: Mutex::new(None),
            auto_retries: Mutex::new(HashMap::new()),
        })
    }

    /// Loads the finished songs and starts the worker. Downloads cut off by the last exit go back
    /// to the queue first and continue where they stopped.
    pub fn start(self: &Arc<Self>) {
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let db = service.db();
            if let Err(error) = store::reset_interrupted(&db).await {
                tracing::warn!(%error, "could not reset interrupted song downloads");
            }
            match store::done(&db).await {
                Ok(rows) => {
                    let mut offline = service.offline.lock().expect("offline index poisoned");
                    for (id, key, file) in rows {
                        offline.insert(key, service.dir(&id).join(file));
                    }
                }
                Err(error) => tracing::warn!(%error, "song downloads unreadable"),
            }
            service.work().await;
        });
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

    /// Pausing stops the running download where it is (it continues from there on resume).
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        if paused {
            let running = self
                .running
                .lock()
                .expect("running poisoned")
                .as_ref()
                .map(|running| running.id.clone());
            if let Some(id) = running {
                self.stop_running(&id, Stop::Paused);
            }
        } else {
            self.wake.notify_one();
        }
        self.changed("");
    }

    async fn work(&self) {
        loop {
            // Registered before checking, so a wake-up between the check and the wait isn't lost.
            let woken = self.wake.notified();
            if self.paused() {
                woken.await;
                continue;
            }
            // Battery saver holds the next download (unless allowed); one under way finishes.
            crate::device_state::wait_for_downloads().await;
            match store::next_queued(&self.db()).await {
                Ok(Some(row)) => self.download(row).await,
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
        self.wake.notify_one();
        if queued.len() <= ANNOUNCED {
            for id in &queued {
                self.announce(id).await;
            }
        } else {
            self.changed("");
        }
        Ok(())
    }

    async fn download(&self, row: SongDownloadRow) {
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
        *self.running.lock().expect("running poisoned") = Some(Running {
            id: row.id.clone(),
            stop: stop.clone(),
        });
        let item = self.to_download(row.clone());
        self.live.lock().expect("live poisoned").insert(
            row.id.clone(),
            Live {
                done: row.done_bytes.max(0) as u64,
                total: row.total_bytes.map(|total| total.max(0) as u64),
                source: row.provider.clone(),
                speed: Speed::default(),
                told: None,
                item,
            },
        );
        self.announce(&row.id).await;
        // Paused or removed while it was being picked up.
        let result = if self.paused() || self.stopped(&row.id) {
            Err(Failure::Stopped)
        } else {
            self.fetch(&row, &stop).await
        };
        *self.running.lock().expect("running poisoned") = None;
        let live = self.live.lock().expect("live poisoned").remove(&row.id);
        let asked = self.stops.lock().expect("stops poisoned").remove(&row.id);
        let outcome = match (asked, result) {
            (Some(Stop::Removed), _) => {
                self.delete_files(&row.id).await;
                Ok(())
            }
            (Some(Stop::Restarted), _) => {
                // Whatever came from the other stream goes; the picked one starts first.
                self.delete_files(&row.id).await;
                let front = self.front().await;
                store::set_queued(&db, &row.id, Some(front)).await
            }
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
            (None, Err(Failure::NeedsRecording { source, message })) => {
                if let Ok(source) = serde_json::to_string(&source) {
                    let _ = store::set_source(&db, &row.id, &source).await;
                }
                tracing::info!(song = %row.song.track_key, summary = %message.summary, "song download needs recording");
                store::set_failed(
                    &db,
                    &row.id,
                    store::NEEDS_RECORDING,
                    &message.summary,
                    message.detail.as_deref(),
                    now_ms(),
                )
                .await
            }
            (None, Err(Failure::Error(message))) => {
                tracing::warn!(song = %row.song.track_key, summary = %message.summary, detail = ?message.detail, "song download failed");
                store::set_failed(
                    &db,
                    &row.id,
                    store::FAILED,
                    &message.summary,
                    message.detail.as_deref(),
                    now_ms(),
                )
                .await
            }
            (None, Err(Failure::Transient(message))) => {
                tracing::warn!(song = %row.song.track_key, summary = %message.summary, detail = ?message.detail, "song download failed for now");
                let failed = store::set_failed(
                    &db,
                    &row.id,
                    store::FAILED,
                    &message.summary,
                    message.detail.as_deref(),
                    now_ms(),
                )
                .await;
                self.retry_later(&row.id);
                failed
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not record a song download's outcome");
        }
        self.announce(&row.id).await;
    }

    /// A saved song: its file goes in the offline index and the download is done.
    async fn finished(
        &self,
        row: &SongDownloadRow,
        saved: Saved,
    ) -> Result<(), raygun_db::DbError> {
        tracing::info!(song = %row.song.track_key, file = %saved.file, "song downloaded");
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(&row.id);
        let dir = self.dir(&row.id);
        let path = dir.join(&saved.file);
        // What another stream left half done goes.
        for leftover in [PARTIAL, PARTIAL_SOURCE] {
            let _ = tokio::fs::remove_file(dir.join(leftover)).await;
        }
        let bytes = tokio::fs::metadata(&path)
            .await
            .ok()
            .map(|meta| meta.len() as i64);
        let kept = store::set_done(
            &self.db(),
            &row.id,
            &saved.file,
            "file",
            saved.provider.as_deref(),
            bytes,
            now_ms(),
        )
        .await?;
        // Removed meanwhile: nothing to play from.
        if kept {
            self.offline
                .lock()
                .expect("offline index poisoned")
                .insert(row.song.track_key.clone(), path);
            self.save_finished(&row.id);
        }
        Ok(())
    }

    /// Finds a file for the song and saves it: the source it was asked with first, then what the
    /// resolver finds, then YouTube's copy with yt-dlp on desktop. When nothing gives a file but
    /// something plays it, it can be recorded.
    async fn fetch(&self, row: &SongDownloadRow, stop: &Notify) -> Result<Saved, Failure> {
        let asked: Option<MediaSource> = row
            .song
            .source
            .as_deref()
            .and_then(|json| serde_json::from_str(json).ok());
        let query = TrackQuery {
            title: row.song.title.clone(),
            artist: row.song.artist.clone(),
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
            ..Exclusions::default()
        };
        let mut report = Report::default();
        // What plays it, for a recording when no file comes: an HLS stream, else YouTube.
        let mut pieces: Option<MediaSource> = None;
        let mut youtube: Option<MediaSource> = None;

        // 1. The source it was asked with, or the one picked for it.
        match &asked {
            Some(source @ MediaSource::YouTube { video_id }) => {
                youtube = Some(source.clone());
                if crate::ytdlp::supported() {
                    self.trying(&row.id, Some("YouTube".into()));
                    match self.save_with_ytdlp(row, &dir, video_id, stop).await {
                        Ok(saved) => return Ok(saved),
                        Err(StreamError::Stopped) => return Err(Failure::Stopped),
                        Err(StreamError::Device(error)) => {
                            return Err(Failure::Error(device_message(&error)));
                        }
                        Err(error) => report.ytdlp = Some(error.text()),
                    }
                }
            }
            Some(source @ MediaSource::Url { url, headers }) => {
                let name = row.provider.clone().unwrap_or_else(|| source_name(url));
                self.trying(&row.id, Some(name.clone()));
                match self
                    .save_retrying(row, &dir, url, headers, Some(name.clone()), stop)
                    .await
                {
                    Ok(saved) => return Ok(saved),
                    Err(StreamError::Stopped) => return Err(Failure::Stopped),
                    Err(StreamError::Device(error)) => {
                        return Err(Failure::Error(device_message(&error)));
                    }
                    Err(StreamError::Pieces) => {
                        report.pieces.push(name);
                        pieces = Some(source.clone());
                    }
                    Err(StreamError::Transient(message)) => {
                        report.transient = true;
                        report.failed.push((name, message));
                    }
                    Err(StreamError::Error(message)) => report.failed.push((name, message)),
                }
                exclusions.failed_sources.push(source.clone());
                tracing::info!(song = %row.song.track_key, "the song's own stream gave no file; trying others");
            }
            _ => {}
        }

        // 2. Whatever else the resolver finds, YouTube aside (its embed has no file to fetch).
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
                    report.resolver = Some(error);
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
                report
                    .loose
                    .push((pick.provider_name.clone(), pick.title.clone()));
                continue;
            }
            match &pick.source {
                source @ MediaSource::YouTube { .. } => {
                    youtube.get_or_insert_with(|| source.clone());
                }
                MediaSource::Url { url, headers } => {
                    tries += 1;
                    let name = pick.provider_name.clone();
                    self.trying(&row.id, Some(name.clone()));
                    match self
                        .save_retrying(row, &dir, url, headers, Some(name.clone()), stop)
                        .await
                    {
                        Ok(saved) => {
                            resolver.mark_verified(&pick).await;
                            return Ok(saved);
                        }
                        Err(StreamError::Stopped) => return Err(Failure::Stopped),
                        Err(StreamError::Device(error)) => {
                            return Err(Failure::Error(device_message(&error)));
                        }
                        // It plays, just not as one file: not a failure for playback.
                        Err(StreamError::Pieces) => {
                            report.pieces.push(name);
                            pieces.get_or_insert_with(|| pick.source.clone());
                        }
                        // The network's fault, not the stream's.
                        Err(StreamError::Transient(message)) => {
                            report.transient = true;
                            report.failed.push((name, message));
                        }
                        Err(StreamError::Error(message)) => {
                            resolver.mark_failed(&pick, &message).await;
                            report.failed.push((name, message));
                        }
                    }
                }
                _ => {}
            }
        }

        // 3. YouTube's copy as a file with yt-dlp on desktop (M46.47), unless that just failed.
        if let Some(MediaSource::YouTube { video_id }) = &youtube
            && crate::ytdlp::supported()
            && report.ytdlp.is_none()
            && !self.stopped(&row.id)
        {
            self.trying(&row.id, Some("YouTube".into()));
            match self.save_with_ytdlp(row, &dir, video_id, stop).await {
                Ok(saved) => return Ok(saved),
                Err(StreamError::Stopped) => return Err(Failure::Stopped),
                Err(StreamError::Device(error)) => {
                    return Err(Failure::Error(device_message(&error)));
                }
                Err(error) => report.ytdlp = Some(error.text()),
            }
        }
        if self.stopped(&row.id) {
            return Err(Failure::Stopped);
        }
        let message = report.message(Outlook {
            youtube: youtube.is_some(),
            can_record: record::SUPPORTED,
            ytdlp: crate::ytdlp::supported(),
            retrying: report.transient && self.retries_left(&row.id),
        });
        match pieces.or(youtube) {
            Some(source) if record::SUPPORTED => Err(Failure::NeedsRecording {
                source: Box::new(source),
                message,
            }),
            _ if report.transient => Err(Failure::Transient(message)),
            _ => Err(Failure::Error(message)),
        }
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
        Ok(Saved {
            file,
            provider: Some(YTDLP_SOURCE.into()),
        })
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
                    service.wake.notify_one();
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
        Ok(Saved { file, provider })
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
        let item = {
            let mut live = self.live.lock().expect("live poisoned");
            let Some(entry) = live.get_mut(id) else {
                return;
            };
            entry.source = source;
            entry.done = 0;
            entry.total = None;
            entry.speed = Speed::default();
            entry.told = None;
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
            .as_ref()
            .filter(|running| running.id == id)
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
            .as_ref()
            .is_some_and(|running| running.id == id)
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

    async fn remove_quietly(&self, id: &str) -> Result<(), String> {
        let db = self.db();
        let Some(row) = store::get(&db, id).await.map_err(to_message)? else {
            return Ok(());
        };
        // Also when the worker has just picked it up and isn't running it yet.
        if self.is_running(id) || row.state == store::DOWNLOADING {
            self.stop_running(id, Stop::Removed);
        }
        if row.state == store::RECORDING {
            self.stop_recording(RecordingEnd::Removed);
        }
        self.offline
            .lock()
            .expect("offline index poisoned")
            .remove(&row.song.track_key);
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        store::remove(&db, id).await.map_err(to_message)?;
        self.delete_files(id).await;
        Ok(())
    }

    /// Downloads the song from a source picked for it (M46.47), first in the queue: that stream
    /// first, then the others as usual. What was saved from another stream is dropped, and a
    /// download running now starts again from the pick.
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
        if row.state == store::DONE {
            return Err("That song is already downloaded".into());
        }
        if row.state == store::RECORDING {
            return Err("That song is being recorded now".into());
        }
        let source = track
            .source
            .ok_or("That answer has no stream of its own: pick one that plays")?;
        let name = picked
            .filter(|name| !name.trim().is_empty())
            .or_else(|| match &source {
                MediaSource::YouTube { .. } => Some("YouTube".into()),
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
        if self.is_running(id) {
            // The worker starts it again from the pick.
            self.stop_running(id, Stop::Restarted);
        } else {
            self.delete_files(id).await;
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
            store::RECORDING => return Err("That song is being recorded now".into()),
            store::DONE => return Err("That song is already downloaded".into()),
            _ => {}
        }
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        let front = self.front().await;
        store::set_queued(&db, id, Some(front))
            .await
            .map_err(to_message)?;
        self.wake.notify_one();
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
            store::set_queued(&db, id, Some(base + index as i64))
                .await
                .map_err(to_message)?;
        }
        if !ids.is_empty() {
            self.wake.notify_one();
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
        let key = raygun_resolver::track_key(&query_for(track));
        Ok(store::by_key(&self.db(), &key)
            .await
            .map_err(to_message)?
            .map(|row| self.to_download(row)))
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

    /// A row as the UI sees it, with the running download's or recording's progress.
    fn to_download(&self, row: SongDownloadRow) -> SongDownload {
        let state = match SongDownloadState::parse(&row.state) {
            // Where nothing can be recorded (one left from another version), it simply failed.
            SongDownloadState::NeedsRecording if !record::SUPPORTED => SongDownloadState::Failed,
            state => state,
        };
        let done = row.done_bytes.max(0) as u64;
        let total = row.total_bytes.map(|total| total.max(0) as u64);
        let item = SongDownload {
            state,
            kind: match state {
                SongDownloadState::Downloading => Some(SongDownloadKind::File),
                SongDownloadState::Recording => Some(SongDownloadKind::Recording),
                _ => row.kind.as_deref().and_then(SongDownloadKind::parse),
            },
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
            can_record: record::SUPPORTED,
            queued_at: row.queued_at as f64,
            finished_at: row.finished_at.map(|at| at as f64),
        };
        match state {
            SongDownloadState::Downloading => {
                let live = self.live.lock().expect("live poisoned");
                match live.get(&item.id) {
                    Some(live) => live.apply(item),
                    None => item,
                }
            }
            SongDownloadState::Recording => {
                let recording = self.recording.lock().expect("recording poisoned");
                match recording
                    .as_ref()
                    .filter(|recording| recording.id == item.id)
                {
                    Some(recording) => recording.apply(item),
                    None => item,
                }
            }
            _ => item,
        }
    }

    // ---- Recording (no source gave a file) ----

    /// Plays the song from its start, next in the queue, and records Raygun's own sound until it
    /// ends; the recording becomes the saved copy.
    pub async fn record(&self, id: &str) -> Result<(), String> {
        if !record::SUPPORTED {
            return Err("Recording what plays needs the desktop app for now".into());
        }
        if self.recording.lock().expect("recording poisoned").is_some() {
            return Err("A song is being recorded already; wait for it to finish".into());
        }
        let db = self.db();
        let row = store::get(&db, id)
            .await
            .map_err(to_message)?
            .ok_or("This download was removed")?;
        if row.state == store::DONE {
            return Err("That song is already downloaded".into());
        }
        let source = row
            .song
            .source
            .as_deref()
            .and_then(|json| serde_json::from_str::<MediaSource>(json).ok())
            .unwrap_or_else(|| MediaSource::Lookup {
                title: row.song.title.clone(),
                artist: row.song.artist.clone(),
                album: row.song.album.clone(),
                duration_ms: row.song.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
            });
        // What plays is the trimmed song (M46.46).
        let (from_ms, to_ms) = self.played_part(&row);
        let dir = self.dir(id);
        tokio::fs::create_dir_all(&dir).await.map_err(to_message)?;
        let process = record::sound_process(&self.app).await?;
        let capture = record::Capture::start(&dir.join("capture.wav"), process)?;

        let playback = self.app.state::<PlaybackService>();
        // What was playing stops; the recorded song plays alone (M46.46).
        playback.control_here(PlaybackControl::Pause);
        let rate = playback.state().rate;
        if (rate - 1.0).abs() > f64::EPSILON {
            playback.control(PlaybackControl::SetRate { rate: 1.0 });
        }
        let item = QueueItemInput {
            media_kind: match source {
                MediaSource::YouTube { .. } => MediaKind::Video,
                _ => MediaKind::Audio,
            },
            title: row.song.title.clone(),
            artist: row.song.artist.clone(),
            album: row.song.album.clone(),
            duration_ms: row.song.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
            artwork_url: row.song.artwork_url.clone(),
            source,
        };
        playback.enqueue_next(vec![item]);
        // The item just put after the current one.
        let queue = playback.queue();
        let current = playback.state().current_item_id;
        let at = current
            .as_ref()
            .and_then(|current| {
                queue
                    .order
                    .iter()
                    .position(|index| queue.items[*index as usize].id == *current)
            })
            .map_or(0, |position| position + 1);
        let item_id = queue
            .order
            .get(at)
            .map(|index| queue.items[*index as usize].id.clone())
            .ok_or("The song didn't reach the queue")?;
        store::set_state(&db, id, store::RECORDING)
            .await
            .map_err(to_message)?;
        let snapshot = self.to_download(SongDownloadRow {
            state: store::RECORDING.into(),
            error: None,
            error_detail: None,
            ..row
        });
        *self.recording.lock().expect("recording poisoned") = Some(Recording {
            id: id.to_owned(),
            capture,
            item_id: item_id.clone(),
            rate,
            started: false,
            item: snapshot,
            from_ms,
            to_ms,
            position_ms: from_ms,
            told: None,
        });
        self.announce(id).await;
        playback.control(PlaybackControl::PlayItem { item_id });
        Ok(())
    }

    /// The part of a song that plays, in ms: from its start trim to its end trim or its end.
    fn played_part(&self, row: &SongDownloadRow) -> (f64, Option<f64>) {
        let edit = self
            .app
            .state::<Arc<crate::sound::SoundService>>()
            .edit(&crate::sound::key(&TrackQuery {
                title: row.song.title.clone(),
                artist: row.song.artist.clone(),
                album: row.song.album.clone(),
                duration_ms: None,
            }));
        let full = row.song.duration_ms.filter(|full| *full > 0);
        let end = match (edit.end_ms, full) {
            (Some(end), Some(full)) => Some(i64::from(end).min(full)),
            (Some(end), None) => Some(i64::from(end)),
            (None, full) => full,
        };
        (f64::from(edit.start_ms), end.map(|end| end as f64))
    }

    /// Follows playback while a recording runs: sound is kept only while the recorded song
    /// plays, and the recording ends when another item loads or playback stops.
    pub fn on_playback(&self, state: &PlaybackState, _item: Option<&QueueItem>) {
        let (finished, progress) = {
            let mut guard = self.recording.lock().expect("recording poisoned");
            let Some(recording) = guard.as_mut() else {
                return;
            };
            let ours = state.current_item_id.as_deref() == Some(recording.item_id.as_str());
            let playing = ours && state.status == PlaybackStatus::Playing;
            recording.capture.set_writing(playing);
            if playing {
                recording.started = true;
            }
            if ours {
                recording.position_ms = state.position_ms;
            }
            let end = if ours && state.status == PlaybackStatus::Error {
                Some(RecordingEnd::Failed(state.error.clone()))
            } else if (recording.started && !ours)
                || (ours && matches!(state.status, PlaybackStatus::Ended | PlaybackStatus::Idle))
            {
                Some(RecordingEnd::Ended)
            } else {
                None
            };
            let now = Instant::now();
            let step = (recording.progress().0.unwrap_or(0.0) * 100.0) as u64;
            let progress = (end.is_none() && ours && worth_telling(recording.told, step, now))
                .then(|| {
                    recording.told = Some((step, now));
                    recording.apply(recording.item.clone())
                });
            (end, progress)
        };
        if let Some(item) = progress {
            self.emit_item(item);
        }
        if let Some(end) = finished {
            if let RecordingEnd::Failed(error) = &end {
                tracing::warn!(?error, "the recorded song failed to play");
            }
            self.stop_recording(end);
        }
    }

    /// Whether `control` is refused because a recording runs (M46.46): it can't be paused,
    /// sought, skipped or changed; volume, repeat, shuffle and the sleep timer still work.
    pub fn locks(&self, control: &PlaybackControl) -> bool {
        let guard = self.recording.lock().expect("recording poisoned");
        let Some(recording) = guard.as_ref() else {
            return false;
        };
        match control {
            PlaybackControl::PlayItem { item_id } => *item_id != recording.item_id,
            PlaybackControl::Play
            | PlaybackControl::SetVolume { .. }
            | PlaybackControl::SetRepeat { .. }
            | PlaybackControl::SetShuffle { .. }
            | PlaybackControl::SetSleepTimer { .. } => false,
            _ => true,
        }
    }

    /// Stops the running recording without keeping it (its window's Cancel).
    pub fn cancel_recording(&self) {
        self.stop_recording(RecordingEnd::Cancelled);
    }

    /// Ends the running recording: one that played to its end is encoded into the saved copy,
    /// else it's dropped. The recorded song leaves the queue and the player stays paused (M46.46).
    fn stop_recording(&self, end: RecordingEnd) {
        let Some(recording) = self.recording.lock().expect("recording poisoned").take() else {
            return;
        };
        let playback = self.app.state::<PlaybackService>();
        playback.control_here(PlaybackControl::Pause);
        playback.control_here(PlaybackControl::Remove {
            item_id: recording.item_id.clone(),
        });
        if (recording.rate - 1.0).abs() > f64::EPSILON {
            playback.control(PlaybackControl::SetRate {
                rate: recording.rate,
            });
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let service = app.state::<Arc<SongDownloadService>>().inner().clone();
            service.finish_recording(recording, end).await;
        });
    }

    async fn finish_recording(&self, recording: Recording, end: RecordingEnd) {
        let db = self.db();
        let id = recording.id.clone();
        let dir = self.dir(&id);
        let row = store::get(&db, &id).await.ok().flatten();
        let expected_ms = recording
            .to_ms
            .map_or(0, |to| (to - recording.from_ms).max(0.0) as u64);
        let ending = end.clone();
        let result = tokio::task::spawn_blocking(move || {
            let stats = recording.capture.finish().map_err(|error| Message {
                summary: "The recording couldn't be saved.".into(),
                detail: Some(error),
            })?;
            match ending {
                RecordingEnd::Ended => {}
                RecordingEnd::Cancelled | RecordingEnd::Removed => {
                    let _ = std::fs::remove_file(&stats.path);
                    return Err(Message {
                        summary: "The recording was stopped before the song ended, so nothing \
                                  was kept."
                            .into(),
                        detail: None,
                    });
                }
                RecordingEnd::Failed(error) => {
                    let _ = std::fs::remove_file(&stats.path);
                    return Err(Message {
                        summary: "The song didn't play to its end, so nothing was kept. Record \
                                  it again, or choose another source."
                            .into(),
                        detail: error,
                    });
                }
            }
            let flac = dir.join("track.flac");
            let kept_ms =
                transcode::finish_recording(&stats.path, &flac).map_err(|error| Message {
                    summary: "The recording couldn't be saved.".into(),
                    detail: Some(error),
                })?;
            let _ = std::fs::remove_file(&stats.path);
            // A recording far shorter than the song was cut off (skipped, or the player failed).
            if expected_ms > 0 && kept_ms < expected_ms * 6 / 10 {
                let _ = std::fs::remove_file(&flac);
                return Err(Message {
                    summary: format!(
                        "Only {} of {} was recorded. Record it again and let it play to the end.",
                        clock(kept_ms),
                        clock(expected_ms)
                    ),
                    detail: None,
                });
            }
            Ok(flac)
        })
        .await
        .unwrap_or_else(|error| {
            Err(Message {
                summary: "The recording couldn't be saved.".into(),
                detail: Some(error.to_string()),
            })
        });
        let outcome = match (result, row) {
            (_, None) => {
                // Removed meanwhile.
                self.delete_files(&id).await;
                Ok(())
            }
            (Ok(flac), Some(row)) => {
                let bytes = tokio::fs::metadata(&flac)
                    .await
                    .ok()
                    .map(|meta| meta.len() as i64);
                let done = store::set_done(
                    &db,
                    &id,
                    "track.flac",
                    "recording",
                    Some("Recording"),
                    bytes,
                    now_ms(),
                )
                .await;
                if matches!(done, Ok(true)) {
                    self.offline
                        .lock()
                        .expect("offline index poisoned")
                        .insert(row.song.track_key.clone(), flac);
                    self.save_finished(&id);
                }
                done.map(|_| ())
            }
            (Err(message), Some(_)) => {
                tracing::warn!(summary = %message.summary, detail = ?message.detail, "recording failed");
                let _ = tokio::fs::remove_file(self.dir(&id).join("capture.wav")).await;
                store::set_failed(
                    &db,
                    &id,
                    store::NEEDS_RECORDING,
                    &message.summary,
                    message.detail.as_deref(),
                    now_ms(),
                )
                .await
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not record a recording's outcome");
        }
        if end != RecordingEnd::Removed {
            self.announce(&id).await;
        }
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
    let path = service.offline_file(&key)?;
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
    have > 0 && started_from.is_some_and(|from| from.trim() == url)
}

fn rank(stop: Stop) -> u8 {
    match stop {
        Stop::Paused => 0,
        Stop::Restarted => 1,
        Stop::Removed => 2,
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

/// `m:ss`, as a song's length reads.
fn clock(ms: u64) -> String {
    let seconds = ms / 1000;
    format!("{}:{:02}", seconds / 60, seconds % 60)
}

/// What a listener reads when something went wrong: a line, and the sources' own errors.
#[derive(Debug, Clone, PartialEq, Eq)]
struct Message {
    summary: String,
    detail: Option<String>,
}

/// What this device can do with a song no source gave a file of.
#[derive(Debug, Clone, Copy)]
struct Outlook {
    /// YouTube plays it.
    youtube: bool,
    /// What plays can be recorded here.
    can_record: bool,
    /// yt-dlp runs here.
    ytdlp: bool,
    /// It goes back to the queue by itself later (a network failure, with tries left).
    retrying: bool,
}

/// What a download's attempts came to.
#[derive(Debug, Default)]
struct Report {
    /// Sources whose file failed: (name, what happened).
    failed: Vec<(String, String)>,
    /// Sources that stream it only in HLS pieces.
    pieces: Vec<String>,
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
    /// Says what happened (which sources had it, never "only YouTube" when others did), and what
    /// can be done.
    fn message(&self, outlook: Outlook) -> Message {
        let failed = unique(self.failed.iter().map(|(name, _)| name.as_str()));
        let pieces = unique(self.pieces.iter().map(String::as_str));
        let only_youtube =
            failed.is_empty() && pieces.is_empty() && self.loose.is_empty() && outlook.youtube;
        let mut sentences = vec![if !failed.is_empty() {
            format!(
                "{} had it, but the {} didn't work.",
                names(&failed),
                if failed.len() == 1 {
                    "download"
                } else {
                    "downloads"
                }
            )
        } else if !pieces.is_empty() {
            format!(
                "{} {} it only in pieces, which can't be saved as a file.",
                names(&pieces),
                if pieces.len() == 1 {
                    "streams"
                } else {
                    "stream"
                }
            )
        } else if !self.loose.is_empty() {
            "Other sources only had different songs by that name.".to_owned()
        } else if outlook.youtube {
            "Only YouTube has this song.".to_owned()
        } else {
            "No source has this song.".to_owned()
        }];
        // Why YouTube, which plays it, gave no file.
        if outlook.youtube {
            if !outlook.ytdlp {
                sentences.push("Songs can't be saved from YouTube on phones.".to_owned());
            } else if self.ytdlp.is_some() {
                sentences.push(
                    if only_youtube {
                        "It couldn't be saved from there."
                    } else {
                        "YouTube's copy couldn't be saved either."
                    }
                    .to_owned(),
                );
            }
        }
        let recordable = outlook.youtube || !pieces.is_empty();
        sentences.push(
            if recordable && outlook.can_record {
                "Record it as it plays, or choose another source."
            } else if outlook.retrying {
                "It's tried again in a while, or choose another source."
            } else if failed.is_empty() && !recordable {
                "Choose another source, or check the song's title and artist."
            } else {
                "Choose another source."
            }
            .to_owned(),
        );
        let mut lines: Vec<String> = self
            .failed
            .iter()
            .map(|(name, error)| format!("{name}: {error}"))
            .collect();
        lines.extend(
            pieces
                .iter()
                .map(|name| format!("{name}: streams in HLS pieces")),
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
            summary: sentences.join(" "),
            detail: (!lines.is_empty()).then(|| lines.join("\n")),
        }
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

/// "Audius", "Audius and Jamendo", "Audius, Jamendo and 2 more".
fn names(names: &[&str]) -> String {
    match names {
        [] => String::new(),
        [one] => (*one).to_owned(),
        [first, second] => format!("{first} and {second}"),
        [first, second, rest @ ..] => format!("{first}, {second} and {} more", rest.len()),
    }
}

/// What went wrong when the song couldn't be written on this device.
fn device_message(error: &str) -> Message {
    Message {
        summary: "The song couldn't be saved on this device. Check that there's room, then try \
                  again."
            .into(),
        detail: Some(error.to_owned()),
    }
}

struct Saved {
    file: String,
    provider: Option<String>,
}

enum Failure {
    /// No file anywhere, but `source` plays it: it can be recorded as it plays.
    NeedsRecording {
        source: Box<MediaSource>,
        message: Message,
    },
    Error(Message),
    /// The network or a busy server (M46.46): tried again later.
    Transient(Message),
    /// Removed, paused or given another source while it ran.
    Stopped,
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
            can_record: false,
            queued_at: 1.0,
            finished_at: None,
        };
        let start = Instant::now();
        let mut live = Live {
            item: item.clone(),
            done: 0,
            total: None,
            source: None,
            speed: Speed::default(),
            told: None,
        };
        // Before a source answers: the picked one, nothing known yet.
        let shown = live.apply(item.clone());
        assert_eq!(shown.state, SongDownloadState::Downloading);
        assert_eq!(shown.kind, Some(SongDownloadKind::File));
        assert_eq!(shown.provider.as_deref(), Some("Picked"));
        assert_eq!((shown.fraction, shown.eta_ms), (None, None));

        live.source = Some("Audius".into());
        live.total = Some(4_000_000);
        live.speed.update(0, start);
        live.done = 1_000_000;
        live.speed.update(1_000_000, start + Duration::from_secs(1));
        let shown = live.apply(item);
        assert_eq!(shown.provider.as_deref(), Some("Audius"));
        assert_eq!(shown.fraction, Some(0.25));
        assert_eq!(shown.bytes_per_second, Some(1_000_000.0));
        assert_eq!(shown.eta_ms, Some(3_000.0));
        assert_eq!(
            (shown.done_bytes, shown.total_bytes),
            (1_000_000.0, Some(4_000_000.0))
        );
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
    }

    #[test]
    fn streams_are_named_by_their_site() {
        assert_eq!(
            source_name("https://discoveryprovider3.audius.co/v1/tracks/x/stream"),
            "audius.co"
        );
        assert_eq!(
            source_name("https://archive.org/download/a/b.mp3"),
            "archive.org"
        );
        assert_eq!(source_name("http://10.0.2.2:8765/a.mp3"), "10.0.2.2");
        assert_eq!(source_name("not an address"), "Its own source");
        assert_eq!(clock(200_000), "3:20");
        assert_eq!(clock(59_000), "0:59");
    }

    #[test]
    fn failures_say_what_really_happened() {
        let windows = Outlook {
            youtube: true,
            can_record: true,
            ytdlp: true,
            retrying: false,
        };
        let phone = Outlook {
            youtube: true,
            can_record: false,
            ytdlp: false,
            retrying: false,
        };
        let no_youtube = |outlook: Outlook| Outlook {
            youtube: false,
            ..outlook
        };

        // Only YouTube said it had it.
        assert_eq!(
            Report::default().message(phone).summary,
            "Only YouTube has this song. Songs can't be saved from YouTube on phones. Choose \
             another source."
        );

        // Other sources had it and their files failed: never "only YouTube".
        let report = Report {
            failed: vec![
                ("Audius".into(), "The server answered 403 Forbidden".into()),
                ("Audius".into(), "The server answered 404 Not Found".into()),
                ("Internet Archive".into(), "The download stalled".into()),
            ],
            resolver: Some("No playable source found for \u{201c}Swim\u{201d}".into()),
            ..Report::default()
        };
        let message = report.message(phone);
        assert_eq!(
            message.summary,
            "Audius and Internet Archive had it, but the downloads didn't work. Songs can't be \
             saved from YouTube on phones. Choose another source."
        );
        assert_eq!(
            message.detail.as_deref(),
            Some(
                "Audius: The server answered 403 Forbidden\nAudius: The server answered 404 Not \
                 Found\nInternet Archive: The download stalled\nSearch: No playable source found \
                 for \u{201c}Swim\u{201d}"
            )
        );

        // yt-dlp couldn't save YouTube's copy on Windows: it can be recorded.
        let ytdlp = Some("Sign in to confirm you're not a bot".to_owned());
        let report = Report {
            ytdlp: ytdlp.clone(),
            ..Report::default()
        };
        let message = report.message(windows);
        assert_eq!(
            message.summary,
            "Only YouTube has this song. It couldn't be saved from there. Record it as it plays, \
             or choose another source."
        );
        assert_eq!(
            message.detail.as_deref(),
            Some("yt-dlp: Sign in to confirm you're not a bot")
        );
        let report = Report {
            failed: vec![("Audius".into(), "The server answered 410 Gone".into())],
            ytdlp,
            ..Report::default()
        };
        assert_eq!(
            report.message(windows).summary,
            "Audius had it, but the download didn't work. YouTube's copy couldn't be saved \
             either. Record it as it plays, or choose another source."
        );

        // Different songs by that name, and nothing else.
        let report = Report {
            loose: vec![("Audius".into(), "Swim (cover)".into())],
            ..Report::default()
        };
        let message = report.message(no_youtube(windows));
        assert_eq!(
            message.summary,
            "Other sources only had different songs by that name. Choose another source, or check \
             the song's title and artist."
        );
        assert_eq!(
            message.detail.as_deref(),
            Some("Audius offered \u{201c}Swim (cover)\u{201d}")
        );

        // HLS pieces can be recorded.
        let report = Report {
            pieces: vec!["Radio".into()],
            ..Report::default()
        };
        assert_eq!(
            report.message(no_youtube(windows)).summary,
            "Radio streams it only in pieces, which can't be saved as a file. Record it as it \
             plays, or choose another source."
        );

        // The network: it's tried again while tries are left.
        let report = Report {
            failed: vec![("Audius".into(), "The download stalled".into())],
            transient: true,
            ..Report::default()
        };
        let retrying = Outlook {
            retrying: true,
            ..no_youtube(phone)
        };
        assert_eq!(
            report.message(retrying).summary,
            "Audius had it, but the download didn't work. It's tried again in a while, or choose \
             another source."
        );
        assert_eq!(
            report.message(no_youtube(phone)).summary,
            "Audius had it, but the download didn't work. Choose another source."
        );
        assert_eq!(
            Report::default().message(no_youtube(phone)).summary,
            "No source has this song. Choose another source, or check the song's title and \
             artist."
        );
        assert_eq!(names(&["A", "B", "C", "D"]), "A, B and 2 more");
    }
}
