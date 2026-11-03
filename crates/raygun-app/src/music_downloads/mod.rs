//! Songs saved into the app to play offline (M46.10). A queue in `local_music_download`, worked
//! one song at a time: a source's own file downloads with Range resume to
//! `downloads/music/<id>/track.<ext>`; a song that only YouTube has (played in YouTube's embed,
//! with no file to fetch) can be recorded as it plays, where the platform allows. Playback swaps
//! in the saved copy by the song's key, so a downloaded song plays offline wherever it appears.
//! Saved songs can also be written out as files, as downloaded, WAV or FLAC.

pub mod record;
pub mod transcode;

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use raygun_db::music_downloads::{self as store, NewSong, SongDownloadRow};
use raygun_model::{
    AudioFileFormat, MediaKind, MediaSource, PlaybackControl, PlaybackState, PlaybackStatus,
    QueueItem, QueueItemInput, SaveLocation, SongDestination, SongDownload, SongDownloadState,
    SongsSaved, TrackInput, TrackQuery,
};
use raygun_net::ImageLinks;
use reqwest::header::{CONTENT_RANGE, CONTENT_TYPE, RANGE};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;

use crate::catalog::{now_ms, to_message};
use crate::events::SongDownloadsEvent;
use crate::export::output::Place;
use crate::playback::PlaybackService;
use crate::resolution::Exclusions;
use crate::resolver::ResolverService;
use crate::startup::AppDb;

/// Folder under the downloads root.
const FOLDER: &str = "music";
/// How often progress is written and reported while a song downloads.
const PROGRESS_EVERY: Duration = Duration::from_millis(700);
/// Streams tried before giving up on finding a file (YouTube picks are skipped, not counted).
const RESOLVE_TRIES: usize = 10;
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

/// The recording running now: which download, the capture, and the queue item it follows.
struct Recording {
    id: String,
    capture: record::Capture,
    item_id: String,
    /// The speed to go back to when it's done (recordings play at 1x).
    rate: f64,
    /// Whether the item has started playing.
    started: bool,
}

pub struct SongDownloadService {
    app: AppHandle,
    /// The downloads folder.
    root: PathBuf,
    client: reqwest::Client,
    images: ImageLinks,
    wake: Notify,
    /// Downloads removed while they run; the worker deletes what it fetched.
    cancelled: Mutex<HashSet<String>>,
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
            cancelled: Mutex::new(HashSet::new()),
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

    async fn work(&self) {
        loop {
            // Registered before checking, so a wake-up between the check and the wait isn't lost.
            let woken = self.wake.notified();
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

    /// Queues songs; ones already queued, running or saved are left alone.
    pub async fn enqueue(&self, tracks: Vec<TrackInput>) -> Result<(), String> {
        let db = self.db();
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
                source: track
                    .source
                    .as_ref()
                    .and_then(|source| serde_json::to_string(source).ok()),
            };
            store::enqueue(&db, &song, now_ms())
                .await
                .map_err(to_message)?;
        }
        self.wake.notify_one();
        self.changed("");
        Ok(())
    }

    async fn download(&self, row: SongDownloadRow) {
        let db = self.db();
        if let Err(error) = store::set_state(&db, &row.id, store::DOWNLOADING).await {
            tracing::warn!(%error, "could not start a song download");
            return;
        }
        self.changed(&row.id);
        let result = self.fetch(&row).await;
        let cancelled = self
            .cancelled
            .lock()
            .expect("cancel set poisoned")
            .remove(&row.id);
        let outcome = match result {
            _ if cancelled => {
                self.delete_files(&row.id).await;
                Ok(())
            }
            Ok(saved) => {
                tracing::info!(song = %row.song.track_key, file = %saved.file, "song downloaded");
                self.auto_retries
                    .lock()
                    .expect("retries poisoned")
                    .remove(&row.id);
                self.offline.lock().expect("offline index poisoned").insert(
                    row.song.track_key.clone(),
                    self.dir(&row.id).join(&saved.file),
                );
                let done = store::set_done(
                    &db,
                    &row.id,
                    &saved.file,
                    "file",
                    saved.provider.as_deref(),
                    now_ms(),
                )
                .await;
                if done.is_ok() {
                    self.save_finished(&row.id);
                }
                done
            }
            Err(Failure::NeedsRecording(source, message)) => {
                if let Some(source) = source.and_then(|source| serde_json::to_string(&source).ok())
                {
                    let _ = store::set_source(&db, &row.id, &source).await;
                }
                store::set_failed(&db, &row.id, store::NEEDS_RECORDING, &message).await
            }
            Err(Failure::Error(message)) => {
                tracing::warn!(song = %row.song.track_key, %message, "song download failed");
                store::set_failed(&db, &row.id, store::FAILED, &message).await
            }
            Err(Failure::Transient(message)) => {
                tracing::warn!(song = %row.song.track_key, %message, "song download failed for now");
                let failed = store::set_failed(&db, &row.id, store::FAILED, &message).await;
                self.retry_later(&row.id);
                failed
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not record a song download's outcome");
        }
        self.changed(&row.id);
    }

    /// Finds a file for the song and saves it.
    async fn fetch(&self, row: &SongDownloadRow) -> Result<Saved, Failure> {
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
        tokio::fs::create_dir_all(&dir).await.map_err(io_failure)?;

        // A source's own stream first, then whatever the resolver finds, skipping YouTube (its
        // embed has no file to fetch). The source's own failing falls back to the others (M46.46).
        let mut last_error = None;
        let mut transient = false;
        if let Some(MediaSource::Url { url, headers }) = &asked {
            match self.save_retrying(row, &dir, url, headers, None).await {
                Ok(saved) => return Ok(saved),
                Err(Failure::Error(message)) if message.contains("HLS") => {
                    return Err(Failure::NeedsRecording(asked.clone(), message));
                }
                Err(Failure::Transient(message)) => {
                    transient = true;
                    last_error = Some(message);
                }
                Err(Failure::Error(message)) => last_error = Some(message),
                Err(other) => return Err(other),
            }
            tracing::info!(song = %row.song.track_key, "the source's own stream failed; trying others");
        }
        let mut youtube = match &asked {
            Some(source @ MediaSource::YouTube { .. }) => Some(source.clone()),
            _ => None,
        };
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
        for _ in 0..RESOLVE_TRIES {
            let pick = match resolver.resolve(&query, &exclusions).await {
                Ok(pick) => pick,
                Err(error) => {
                    last_error = Some(error);
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
                continue;
            }
            match &pick.source {
                MediaSource::Url { url, headers } => {
                    match self
                        .save_retrying(row, &dir, url, headers, Some(pick.provider_name.clone()))
                        .await
                    {
                        Ok(saved) => {
                            resolver.mark_verified(&pick).await;
                            return Ok(saved);
                        }
                        Err(Failure::Error(message)) => {
                            resolver.mark_failed(&pick, &message).await;
                            last_error = Some(message);
                        }
                        Err(Failure::Transient(message)) => {
                            resolver.mark_failed(&pick, &message).await;
                            transient = true;
                            last_error = Some(message);
                        }
                        Err(other) => return Err(other),
                    }
                }
                source @ MediaSource::YouTube { .. } => {
                    youtube.get_or_insert_with(|| source.clone());
                }
                _ => {}
            }
        }
        if youtube.is_some() {
            return Err(Failure::NeedsRecording(
                youtube,
                "Only YouTube has this song, and its player gives no file: record it as it plays"
                    .into(),
            ));
        }
        let message = last_error.unwrap_or_else(|| "No source has a file of this song".into());
        Err(if transient {
            Failure::Transient(message)
        } else {
            Failure::Error(message)
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
    ) -> Result<Saved, Failure> {
        let mut waits = STREAM_RETRIES.iter();
        loop {
            match self
                .save_stream(row, dir, url, headers, provider.clone())
                .await
            {
                Err(Failure::Transient(message)) => {
                    let removed = self
                        .cancelled
                        .lock()
                        .expect("cancel set poisoned")
                        .contains(&row.id);
                    match waits.next() {
                        Some(wait) if !removed => {
                            tracing::info!(song = %row.song.track_key, %message, "trying the stream again");
                            tokio::time::sleep(*wait).await;
                        }
                        _ => return Err(Failure::Transient(message)),
                    }
                }
                other => return other,
            }
        }
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
                let _ = service.requeue(&id).await;
            }
        });
    }

    async fn requeue(&self, id: &str) -> Result<(), String> {
        store::set_state(&self.db(), id, store::QUEUED)
            .await
            .map_err(to_message)?;
        self.wake.notify_one();
        self.changed(id);
        Ok(())
    }

    /// Downloads a stream into `dir`, continuing a partial one with a Range request, and checks
    /// that it's audio. Returns the file's name.
    async fn save_stream(
        &self,
        row: &SongDownloadRow,
        dir: &Path,
        url: &str,
        headers: &std::collections::BTreeMap<String, String>,
        provider: Option<String>,
    ) -> Result<Saved, Failure> {
        if !url.starts_with("https://") && !url.starts_with("http://") {
            return Err(Failure::Error(
                "Only web addresses can be downloaded".into(),
            ));
        }
        let partial = dir.join("track.part");
        let have = tokio::fs::metadata(&partial)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);
        let mut request = self.client.get(url);
        for (name, value) in headers {
            request = request.header(name.as_str(), value.as_str());
        }
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let response = tokio::time::timeout(STALL, request.send())
            .await
            .map_err(|_| Failure::Transient("The server didn't answer".into()))?
            .map_err(network_failure)?;
        let status = response.status();
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if content_type.contains("mpegurl") || url.to_ascii_lowercase().contains(".m3u8") {
            return Err(Failure::Error(
                "This source streams in HLS pieces, which can't be saved yet".into(),
            ));
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
                    Failure::Transient(message)
                } else {
                    Failure::Error(message)
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
                return Err(Failure::Error("The file is too big to be a song".into()));
            }
            let done = Arc::new(AtomicU64::new(start));
            let reporter = {
                let (done, id, db, app) =
                    (done.clone(), row.id.clone(), self.db(), self.app.clone());
                let total = total.map(|total| total as i64);
                tauri::async_runtime::spawn(async move {
                    let mut written = u64::MAX;
                    loop {
                        tokio::time::sleep(PROGRESS_EVERY).await;
                        let now = done.load(Ordering::SeqCst);
                        if now != written {
                            written = now;
                            let _ = store::set_progress(&db, &id, now as i64, total).await;
                            let _ = SongDownloadsEvent(id.clone()).emit(&app);
                        }
                    }
                })
            };
            let written = self
                .write_body(row, &partial, resumed, response, &done)
                .await;
            reporter.abort();
            let _ = store::set_progress(
                &self.db(),
                &row.id,
                done.load(Ordering::SeqCst) as i64,
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
            .map_err(io_failure)?;
        // An error page or a video saved as a song would fail later, when it plays.
        let check = path.clone();
        let audio = tokio::task::spawn_blocking(move || transcode::probe(&check))
            .await
            .map_err(|error| Failure::Error(error.to_string()))?;
        if let Err(error) = audio {
            let _ = tokio::fs::remove_file(&path).await;
            return Err(Failure::Error(format!(
                "The source gave something that isn't a song ({error})"
            )));
        }
        Ok(Saved { file, provider })
    }

    async fn write_body(
        &self,
        row: &SongDownloadRow,
        partial: &Path,
        resumed: bool,
        response: reqwest::Response,
        done: &AtomicU64,
    ) -> Result<(), Failure> {
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(resumed)
            .write(true)
            .truncate(!resumed)
            .open(partial)
            .await
            .map_err(io_failure)?;
        let mut body = response.bytes_stream();
        loop {
            // A stream that goes quiet has dropped (M46.46); what came is kept for the retry.
            let Ok(next) = tokio::time::timeout(STALL, body.next()).await else {
                file.flush().await.map_err(io_failure)?;
                return Err(Failure::Transient("The download stalled".into()));
            };
            let Some(chunk) = next else {
                break;
            };
            if self
                .cancelled
                .lock()
                .expect("cancel set poisoned")
                .contains(&row.id)
            {
                file.flush().await.map_err(io_failure)?;
                return Err(Failure::Error("Removed".into()));
            }
            let chunk = chunk.map_err(network_failure)?;
            file.write_all(&chunk).await.map_err(io_failure)?;
            let total = done.fetch_add(chunk.len() as u64, Ordering::SeqCst) + chunk.len() as u64;
            if total > MAX_SONG_BYTES {
                return Err(Failure::Error("The file is too big to be a song".into()));
            }
        }
        file.flush().await.map_err(io_failure)
    }

    /// Takes a download out, stopping it if it runs; its files are deleted.
    pub async fn remove(&self, id: &str) -> Result<(), String> {
        let db = self.db();
        let Some(row) = store::get(&db, id).await.map_err(to_message)? else {
            return Ok(());
        };
        if row.state == store::DOWNLOADING {
            self.cancelled
                .lock()
                .expect("cancel set poisoned")
                .insert(id.to_owned());
        }
        if row.state == store::RECORDING {
            self.stop_recording(false);
        }
        self.offline
            .lock()
            .expect("offline index poisoned")
            .remove(&row.song.track_key);
        store::remove(&db, id).await.map_err(to_message)?;
        self.delete_files(id).await;
        self.changed("");
        Ok(())
    }

    /// Queues a failed download again; it continues with what it saved.
    pub async fn retry(&self, id: &str) -> Result<(), String> {
        self.auto_retries
            .lock()
            .expect("retries poisoned")
            .remove(id);
        self.requeue(id).await
    }

    /// Every download, newest first.
    /// The download of `track`'s song, whatever its state (M46.18: the download button).
    pub async fn find(&self, track: &TrackInput) -> Result<Option<SongDownload>, String> {
        let key = raygun_resolver::track_key(&query_for(track));
        Ok(self
            .list()
            .await?
            .into_iter()
            .find(|download| download.track_key == key))
    }

    pub async fn list(&self) -> Result<Vec<SongDownload>, String> {
        let rows = store::all(&self.db()).await.map_err(to_message)?;
        Ok(rows
            .into_iter()
            .map(|row| SongDownload {
                state: SongDownloadState::parse(&row.state),
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
                id: row.id,
                track_key: row.song.track_key,
                title: row.song.title,
                artist: row.song.artist,
                album: row.song.album,
                duration_ms: row.song.duration_ms.and_then(|ms| u32::try_from(ms).ok()),
                kind: row.kind,
                provider: row.provider,
                done_bytes: row.done_bytes as f64,
                total_bytes: row.total_bytes.map(|total| total as f64),
                error: row.error,
                can_record: record::SUPPORTED,
            })
            .collect())
    }

    // ---- Recording (only YouTube had the song) ----

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
        *self.recording.lock().expect("recording poisoned") = Some(Recording {
            id: id.to_owned(),
            capture,
            item_id: item_id.clone(),
            rate,
            started: false,
        });
        self.changed(id);
        playback.control(PlaybackControl::PlayItem { item_id });
        Ok(())
    }

    /// Follows playback while a recording runs: sound is kept only while the recorded song
    /// plays, and the recording ends when another item loads or playback stops.
    pub fn on_playback(&self, state: &PlaybackState, _item: Option<&QueueItem>) {
        let finished = {
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
            let ended = (recording.started && !ours)
                || (ours && matches!(state.status, PlaybackStatus::Ended | PlaybackStatus::Idle))
                || (ours && state.status == PlaybackStatus::Error);
            ended.then(|| (state.status == PlaybackStatus::Error, state.error.clone()))
        };
        if let Some((failed, error)) = finished {
            if failed {
                tracing::warn!(?error, "the recorded song failed to play");
            }
            self.stop_recording(!failed);
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
        self.stop_recording(false);
    }

    /// Ends the running recording: `keep` encodes it into the saved copy, else it's dropped. The
    /// recorded song leaves the queue and the player stays paused (M46.46).
    fn stop_recording(&self, keep: bool) {
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
            service.finish_recording(recording, keep).await;
        });
    }

    async fn finish_recording(&self, recording: Recording, keep: bool) {
        let db = self.db();
        let id = recording.id.clone();
        let dir = self.dir(&id);
        let row = store::get(&db, &id).await.ok().flatten();
        // What plays is the trimmed song (M46.46).
        let expected_ms = row.as_ref().map_or(0, |row| {
            let edit =
                self.app
                    .state::<Arc<crate::sound::SoundService>>()
                    .edit(&crate::sound::key(&TrackQuery {
                        title: row.song.title.clone(),
                        artist: row.song.artist.clone(),
                        album: row.song.album.clone(),
                        duration_ms: None,
                    }));
            let full = row.song.duration_ms.unwrap_or(0);
            let end = edit.end_ms.map_or(full, i64::from).min(full.max(1));
            (end - i64::from(edit.start_ms)).max(0)
        });
        let result = tokio::task::spawn_blocking(move || {
            let stats = recording.capture.finish()?;
            if !keep {
                return Err("The song stopped before it ended".to_owned());
            }
            let flac = dir.join("track.flac");
            let kept_ms = transcode::finish_recording(&stats.path, &flac)?;
            let _ = std::fs::remove_file(&stats.path);
            // A recording far shorter than the song was cut off (skipped, or the player failed).
            if expected_ms > 0 && (kept_ms as i64) < expected_ms * 6 / 10 {
                let _ = std::fs::remove_file(&flac);
                return Err(format!(
                    "Only {} s of {} s were recorded; play it to the end to record it",
                    kept_ms / 1000,
                    expected_ms / 1000
                ));
            }
            Ok(flac)
        })
        .await
        .unwrap_or_else(|error| Err(error.to_string()));
        let outcome = match (result, row) {
            (Ok(flac), Some(row)) => {
                self.offline
                    .lock()
                    .expect("offline index poisoned")
                    .insert(row.song.track_key.clone(), flac);
                let done = store::set_done(
                    &db,
                    &id,
                    "track.flac",
                    "recording",
                    Some("Recording"),
                    now_ms(),
                )
                .await;
                if done.is_ok() {
                    self.save_finished(&id);
                }
                done
            }
            (Ok(_), None) => Ok(()),
            (Err(message), _) => {
                tracing::warn!(%message, "recording failed");
                let _ = tokio::fs::remove_file(self.dir(&id).join("capture.wav")).await;
                store::set_failed(&db, &id, store::NEEDS_RECORDING, &message).await
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not record a recording's outcome");
        }
        self.changed(&id);
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

    fn changed(&self, id: &str) {
        if let Err(error) = SongDownloadsEvent(id.to_owned()).emit(&self.app) {
            tracing::debug!(%error, "failed to emit song downloads event");
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
    let offered: std::collections::HashSet<String> = raygun_resolver::normalize(offered)
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
    let offered: std::collections::HashSet<String> = raygun_resolver::normalize(offered)
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

struct Saved {
    file: String,
    provider: Option<String>,
}

enum Failure {
    /// No file anywhere; the song can be recorded as `source` plays.
    NeedsRecording(Option<MediaSource>, String),
    Error(String),
    /// The network or a busy server (M46.46): tried again.
    Transient(String),
}

fn io_failure(error: std::io::Error) -> Failure {
    Failure::Error(format!("Couldn't save the song: {error}"))
}

fn network_failure(error: reqwest::Error) -> Failure {
    Failure::Transient(format!("The download stopped: {error}"))
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
}
