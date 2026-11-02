//! Video downloads (M43): a queue in `local_video_download` worked one video at a time by a
//! background task. A plain file downloads with Range resume; an HLS stream saves its chosen
//! variant's segments (four at a time) with their keys and init maps, plus a playlist pointing at
//! them. Files land in `downloads/video/<id>/` and play back through the stream proxy's `/local`
//! route under the same progress key, so resuming and Continue watching carry over.

pub mod hls;

use std::collections::HashSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use futures_util::{StreamExt, stream};
use raygun_db::video_downloads::{self as store, NewDownload, VideoDownloadRow};
use raygun_model::{DownloadState, VideoDownloadItem, VideoDownloads, VideoRequest, VideoSubtitle};
use raygun_net::ImageLinks;
use reqwest::header::{CONTENT_RANGE, CONTENT_TYPE, RANGE};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::io::AsyncWriteExt;
use tokio::sync::Notify;

use crate::catalog::{now_ms, to_message};
use crate::events::VideoDownloadsEvent;
use crate::local_network::{self, LocalNetwork};
use crate::startup::AppDb;

/// Folder under the downloads root, and the first segment of the files' proxy paths.
const FOLDER: &str = "video";
/// HLS segments fetched at once.
const SEGMENT_CONCURRENCY: usize = 4;
/// Most subtitle tracks saved with a video.
const MAX_SUBTITLES: usize = 8;
/// Biggest playlist or subtitle file read.
const MAX_TEXT_BYTES: usize = 16 * 1024 * 1024;
/// How often progress is written and reported while a download runs.
const PROGRESS_EVERY: Duration = Duration::from_millis(700);
/// The playable file of an HLS download.
const HLS_INDEX: &str = "index.m3u8";

pub struct VideoDownloadService {
    app: AppHandle,
    /// The downloads folder.
    root: PathBuf,
    images: ImageLinks,
    client: reqwest::Client,
    wake: Notify,
    paused: AtomicBool,
    /// Downloads removed while they run; the worker deletes what it fetched.
    cancelled: Mutex<HashSet<String>>,
}

impl VideoDownloadService {
    pub fn new(
        app: AppHandle,
        downloads: PathBuf,
        images: ImageLinks,
        client: reqwest::Client,
    ) -> Arc<Self> {
        Arc::new(Self {
            app,
            root: downloads,
            images,
            client,
            wake: Notify::new(),
            paused: AtomicBool::new(false),
            cancelled: Mutex::new(HashSet::new()),
        })
    }

    /// Starts the worker. Downloads cut off by the last exit go back to the queue first and
    /// continue where they stopped.
    pub fn start(self: &Arc<Self>) {
        let downloads = self.clone();
        tauri::async_runtime::spawn(async move {
            match store::reset_interrupted(&downloads.db()).await {
                Ok(0) => {}
                Ok(count) => tracing::info!(count, "resuming interrupted video downloads"),
                Err(error) => tracing::warn!(%error, "could not reset interrupted video downloads"),
            }
            downloads.work().await;
        });
    }

    async fn work(&self) {
        loop {
            // Registered before checking, so a wake-up between the check and the wait isn't lost.
            let woken = self.wake.notified();
            if self.paused.load(Ordering::SeqCst) {
                woken.await;
                continue;
            }
            match store::next_queued(&self.db()).await {
                Ok(Some(row)) => self.download(row).await,
                Ok(None) => woken.await,
                Err(error) => {
                    tracing::warn!(%error, "video download queue unreadable");
                    woken.await;
                }
            }
        }
    }

    async fn download(&self, row: VideoDownloadRow) {
        let db = self.db();
        if let Err(error) = store::set_downloading(&db, &row.id).await {
            tracing::warn!(%error, "could not start a video download");
            return;
        }
        self.changed(&row.id);
        let started = Instant::now();
        let result = self.fetch(&row).await;
        let cancelled = self
            .cancelled
            .lock()
            .expect("cancel set poisoned")
            .remove(&row.id);
        let finished = !cancelled && result.is_ok();
        let outcome = match result {
            _ if cancelled => {
                self.delete_files(&row.id).await;
                Ok(())
            }
            Ok((file, subtitles)) => {
                tracing::info!(
                    key = %row.download.key,
                    file,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "video downloaded"
                );
                let subtitles = serde_json::to_string(&subtitles).unwrap_or_else(|_| "[]".into());
                store::set_done(&db, &row.id, &file, &subtitles, now_ms()).await
            }
            // Paused: back in the queue, to continue where it stopped.
            Err(Failure::Stopped) => store::requeue(&db, &row.id).await,
            Err(Failure::Error(message)) => {
                tracing::warn!(key = %row.download.key, %message, "video download failed");
                store::set_failed(&db, &row.id, &message).await
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not record a video download's outcome");
        } else if finished {
            // Also saved outside the app when Settings → Downloads asks for it (M46.1).
            crate::export::video::save_finished(&self.app, &row.id);
        }
        self.changed(&row.id);
    }

    /// Downloads the video and its subtitles; returns the playable file's name and the saved
    /// tracks.
    async fn fetch(&self, row: &VideoDownloadRow) -> Result<(String, Vec<VideoSubtitle>), Failure> {
        let dir = self.dir(&row.id);
        tokio::fs::create_dir_all(&dir).await.map_err(io_failure)?;
        let headers: std::collections::BTreeMap<String, String> =
            serde_json::from_str(&row.download.headers).unwrap_or_default();
        let progress = Arc::new(Progress::default());
        let stopped = || self.is_stopped(&row.id);
        let job = Job {
            client: &self.client,
            dir: &dir,
            headers: &headers,
            preferred: self.preferred_quality().await,
            stopped: &stopped,
            progress: &progress,
        };
        // Progress is written from a side task, so fetching never waits on the database.
        let reporter = {
            let (progress, id, db) = (progress.clone(), row.id.clone(), self.db());
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                let mut written = (0, 0);
                loop {
                    tokio::time::sleep(PROGRESS_EVERY).await;
                    let now = (
                        progress.bytes.load(Ordering::SeqCst),
                        progress.parts.load(Ordering::SeqCst),
                    );
                    if now != written {
                        written = now;
                        let _ = progress.save(&db, &id).await;
                        let _ = VideoDownloadsEvent(id.clone()).emit(&app);
                    }
                }
            })
        };
        let file = job
            .video(&row.download.url, row.download.format.as_deref())
            .await;
        reporter.abort();
        let _ = progress.save(&self.db(), &row.id).await;
        let file = file?;
        let remote: Vec<VideoSubtitle> =
            serde_json::from_str(&row.download.subtitles).unwrap_or_default();
        let subtitles = job.subtitles(remote, &self.subtitle_language().await).await;
        Ok((file, subtitles))
    }

    /// Queues a video (one already queued, downloading or downloaded is left alone; a failed one
    /// tries again with this link).
    pub async fn enqueue(
        &self,
        request: VideoRequest,
        title_id: Option<String>,
    ) -> Result<(), String> {
        if !request.url.starts_with("https://") && !request.url.starts_with("http://") {
            return Err("Only web addresses can be downloaded".into());
        }
        if request.format.as_deref() == Some("dash") {
            return Err("DASH streams can't be downloaded yet".into());
        }
        self.app
            .state::<LocalNetwork>()
            .check(&request.url, local_network::VIDEOS_DENIED)
            .await?;
        let new = NewDownload {
            key: request.key,
            title_id,
            title: request.title,
            subtitle: request.subtitle,
            // The source's address: a proxy link wouldn't outlive the app.
            poster: request
                .poster
                .as_deref()
                .map(|poster| self.images.original(poster)),
            url: request.url,
            headers: serde_json::to_string(&request.headers).map_err(to_message)?,
            format: request.format,
            subtitles: serde_json::to_string(&request.subtitles).map_err(to_message)?,
        };
        store::enqueue(&self.db(), &new, now_ms())
            .await
            .map_err(to_message)?;
        self.wake.notify_one();
        self.changed("");
        Ok(())
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
        store::remove(&db, id).await.map_err(to_message)?;
        self.delete_files(id).await;
        self.changed("");
        Ok(())
    }

    /// Queues a failed download again; it continues with what it saved.
    pub async fn retry(&self, id: &str) -> Result<(), String> {
        store::retry(&self.db(), id, now_ms())
            .await
            .map_err(to_message)?;
        self.wake.notify_one();
        self.changed(id);
        Ok(())
    }

    /// Pausing stops the running download (it continues from there on resume).
    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        if !paused {
            self.wake.notify_one();
        }
        self.changed("");
    }

    /// Plays a download: a finished one from the device, else its link.
    pub async fn play(&self, id: &str) -> Result<(), String> {
        let row = store::get(&self.db(), id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This download was removed".to_owned())?;
        let download = row.download;
        let request = VideoRequest {
            key: download.key,
            title: download.title,
            subtitle: download.subtitle,
            poster: download.poster,
            url: download.url,
            headers: serde_json::from_str(&download.headers).unwrap_or_default(),
            format: download.format,
            subtitles: serde_json::from_str(&download.subtitles).unwrap_or_default(),
            group: None,
            chapters: Vec::new(),
            anime: None,
        };
        crate::video::play(&self.app, request).await
    }

    /// Every download, newest first.
    pub async fn list(&self) -> Result<VideoDownloads, String> {
        let rows = store::all(&self.db()).await.map_err(to_message)?;
        Ok(VideoDownloads {
            paused: self.paused.load(Ordering::SeqCst),
            items: rows
                .into_iter()
                .map(|row| VideoDownloadItem {
                    state: DownloadState::parse(&row.state),
                    done_bytes: row.done_bytes as f64,
                    total_bytes: row.total_bytes.map(|total| total as f64),
                    done_parts: row.done_parts.max(0) as u32,
                    total_parts: row.total_parts.map(|total| total.max(0) as u32),
                    error: row.error,
                    poster: row
                        .download
                        .poster
                        .as_deref()
                        .map(|url| self.images.cover(url, &Default::default())),
                    id: row.id,
                    key: row.download.key,
                    title_id: row.download.title_id,
                    title: row.download.title,
                    subtitle: row.download.subtitle,
                })
                .collect(),
        })
    }

    /// The saved copy of what plays under `key`, when it's downloaded and still on disk. A
    /// finished row whose files went missing is dropped.
    pub async fn offline(&self, key: &str) -> Option<Offline> {
        let db = self.db();
        let row = store::done_for_key(&db, key).await.ok()??;
        let file = row.file.as_deref()?;
        if !tokio::fs::try_exists(self.dir(&row.id).join(file))
            .await
            .unwrap_or(false)
        {
            tracing::info!(%key, "downloaded video is gone from disk");
            let _ = store::remove(&db, &row.id).await;
            self.changed("");
            return None;
        }
        let saved: Vec<VideoSubtitle> =
            serde_json::from_str(&row.local_subtitles).unwrap_or_default();
        Some(Offline {
            url: self.local_url(&row.id, file),
            format: (file == HLS_INDEX).then(|| "hls".to_owned()),
            subtitles: saved
                .into_iter()
                .map(|track| VideoSubtitle {
                    url: self.local_url(&row.id, &track.url),
                    ..track
                })
                .collect(),
        })
    }

    async fn subtitle_language(&self) -> String {
        let settings = crate::settings::load(&self.db()).await.unwrap_or_default();
        if !settings.subtitle_language.is_empty() {
            settings.subtitle_language
        } else if settings.language != raygun_model::SYSTEM_LANGUAGE {
            settings.language
        } else {
            String::new()
        }
    }

    async fn preferred_quality(&self) -> Option<u32> {
        crate::settings::load(&self.db())
            .await
            .ok()
            .and_then(|settings| settings.preferred_video_quality)
    }

    fn local_url(&self, id: &str, file: &str) -> String {
        self.images.local(&format!("{FOLDER}/{id}/{file}"))
    }

    fn dir(&self, id: &str) -> PathBuf {
        self.root.join(FOLDER).join(id)
    }

    async fn delete_files(&self, id: &str) {
        let dir = self.dir(id);
        if tokio::fs::try_exists(&dir).await.unwrap_or(false)
            && let Err(error) = tokio::fs::remove_dir_all(&dir).await
        {
            tracing::warn!(%error, "could not delete a downloaded video");
        }
    }

    fn is_stopped(&self, id: &str) -> bool {
        self.paused.load(Ordering::SeqCst)
            || self
                .cancelled
                .lock()
                .expect("cancel set poisoned")
                .contains(id)
    }

    fn changed(&self, id: &str) {
        if let Err(error) = VideoDownloadsEvent(id.to_owned()).emit(&self.app) {
            tracing::debug!(%error, "failed to emit video downloads event");
        }
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }
}

/// A finished download as the player gets it.
pub struct Offline {
    pub url: String,
    pub format: Option<String>,
    pub subtitles: Vec<VideoSubtitle>,
}

/// The saved copy of what plays under `key`, if the download service has one.
pub async fn offline(app: &AppHandle, key: &str) -> Option<Offline> {
    let service = app.try_state::<Arc<VideoDownloadService>>()?;
    service.offline(key).await
}

enum Failure {
    /// Paused or removed while running.
    Stopped,
    Error(String),
}

fn io_failure(error: std::io::Error) -> Failure {
    Failure::Error(format!("Couldn't save the video: {error}"))
}

/// How far a download got: bytes for a file, segments for HLS.
#[derive(Default)]
struct Progress {
    bytes: AtomicU64,
    total_bytes: AtomicU64,
    parts: AtomicUsize,
    total_parts: AtomicUsize,
}

impl Progress {
    async fn save(&self, db: &raygun_db::Db, id: &str) -> Result<(), raygun_db::DbError> {
        let positive = |value: u64| (value > 0).then_some(value as i64);
        store::set_progress(
            db,
            id,
            self.bytes.load(Ordering::SeqCst) as i64,
            positive(self.total_bytes.load(Ordering::SeqCst)),
            self.parts.load(Ordering::SeqCst) as i64,
            positive(self.total_parts.load(Ordering::SeqCst) as u64),
        )
        .await
    }
}

/// One running download: where it saves, with what, and how far it got.
struct Job<'a> {
    client: &'a reqwest::Client,
    dir: &'a Path,
    headers: &'a std::collections::BTreeMap<String, String>,
    /// The preferred height, for choosing an HLS variant.
    preferred: Option<u32>,
    /// Paused or removed: stop, keeping what was saved.
    stopped: &'a (dyn Fn() -> bool + Sync),
    progress: &'a Progress,
}

impl Job<'_> {
    /// Downloads a link by its kind; returns the playable file's name.
    async fn video(&self, url: &str, format: Option<&str>) -> Result<String, Failure> {
        let lower = url.to_lowercase();
        let format = format.unwrap_or_default();
        if format == "dash" || lower.contains(".mpd") {
            Err(Failure::Error(
                "DASH streams can't be downloaded yet".into(),
            ))
        } else if format == "hls" || lower.contains(".m3u8") {
            self.hls(url).await
        } else {
            self.progressive(url).await
        }
    }

    /// A plain video file, continuing a partial one with a Range request. Returns its name.
    async fn progressive(&self, url: &str) -> Result<String, Failure> {
        let partial = self.dir.join("video.part");
        let have = tokio::fs::metadata(&partial)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);
        let mut request = self.get(url);
        if have > 0 {
            request = request.header(RANGE, format!("bytes={have}-"));
        }
        let response = request.send().await.map_err(network_failure)?;
        let status = response.status();
        let name = format!("video{}", video_extension(url));
        if have > 0 && status == reqwest::StatusCode::RANGE_NOT_SATISFIABLE {
            // Everything had arrived before the last run stopped.
            tokio::fs::rename(&partial, self.dir.join(&name))
                .await
                .map_err(io_failure)?;
            return Ok(name);
        }
        if !status.is_success() {
            return Err(Failure::Error(format!("The server answered {status}")));
        }
        let content_type = response
            .headers()
            .get(CONTENT_TYPE)
            .and_then(|value| value.to_str().ok())
            .unwrap_or_default()
            .to_ascii_lowercase();
        if content_type.contains("mpegurl") {
            drop(response);
            let _ = tokio::fs::remove_file(&partial).await;
            return self.hls(url).await;
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
        self.progress
            .total_bytes
            .store(total.unwrap_or(0), Ordering::SeqCst);
        self.progress.bytes.store(start, Ordering::SeqCst);
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .append(resumed)
            .write(true)
            .truncate(!resumed)
            .open(&partial)
            .await
            .map_err(io_failure)?;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            if (self.stopped)() {
                file.flush().await.map_err(io_failure)?;
                return Err(Failure::Stopped);
            }
            let chunk = chunk.map_err(network_failure)?;
            file.write_all(&chunk).await.map_err(io_failure)?;
            self.progress
                .bytes
                .fetch_add(chunk.len() as u64, Ordering::SeqCst);
        }
        file.flush().await.map_err(io_failure)?;
        drop(file);
        let written = self.progress.bytes.load(Ordering::SeqCst);
        if total.is_some_and(|total| written < total) {
            return Err(Failure::Error("The download broke off".into()));
        }
        tokio::fs::rename(&partial, self.dir.join(&name))
            .await
            .map_err(io_failure)?;
        Ok(name)
    }

    /// An HLS stream: the chosen variant (and its separate audio, if any) with every segment,
    /// key and init map. Returns the playlist's name.
    async fn hls(&self, url: &str) -> Result<String, Failure> {
        let text = self.text(url).await?;
        let mut plans = Vec::new();
        if hls::is_master(&text) {
            let master = hls::parse_master(&text, url).map_err(Failure::Error)?;
            let variant = hls::choose_variant(&master.variants, self.preferred).clone();
            let video = self.text(&variant.url).await?;
            let video = hls::plan_media(&video, &variant.url, "v").map_err(Failure::Error)?;
            match hls::audio_for(&master, &variant) {
                Some(audio) => {
                    let audio_text = self.text(&audio.url).await?;
                    let audio =
                        hls::plan_media(&audio_text, &audio.url, "a").map_err(Failure::Error)?;
                    let master = hls::local_master(&variant, "video.m3u8", "audio.m3u8");
                    plans.push(("video.m3u8", video));
                    plans.push(("audio.m3u8", audio));
                    self.write(HLS_INDEX, master.as_bytes()).await?;
                }
                None => plans.push((HLS_INDEX, video)),
            }
        } else {
            let plan = hls::plan_media(&text, url, "").map_err(Failure::Error)?;
            plans.push((HLS_INDEX, plan));
        }
        let files: Vec<hls::Fetch> = plans
            .iter()
            .flat_map(|(_, plan)| plan.files.iter().cloned())
            .collect();
        self.progress
            .total_parts
            .store(files.len(), Ordering::SeqCst);
        let mut fetches = stream::iter(files)
            .map(|file| async move { self.save(&file).await })
            .buffer_unordered(SEGMENT_CONCURRENCY);
        while let Some(result) = fetches.next().await {
            result?;
            self.progress.parts.fetch_add(1, Ordering::SeqCst);
        }
        drop(fetches);
        for (name, plan) in &plans {
            self.write(name, plan.playlist.as_bytes()).await?;
        }
        Ok(HLS_INDEX.to_owned())
    }

    /// Saves one segment, key or map, unless an earlier run already did.
    async fn save(&self, file: &hls::Fetch) -> Result<(), Failure> {
        if (self.stopped)() {
            return Err(Failure::Stopped);
        }
        let path = self.dir.join(&file.name);
        if let Ok(meta) = tokio::fs::metadata(&path).await {
            self.progress.bytes.fetch_add(meta.len(), Ordering::SeqCst);
            return Ok(());
        }
        let mut request = self.get(&file.url);
        if let Some((start, length)) = file.range {
            request = request.header(
                RANGE,
                format!("bytes={start}-{}", start + length.saturating_sub(1)),
            );
        }
        let response = request.send().await.map_err(network_failure)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Failure::Error(format!(
                "A part of the stream failed ({status})"
            )));
        }
        let partial = self.dir.join(format!("{}.part", file.name));
        let mut out = tokio::fs::File::create(&partial)
            .await
            .map_err(io_failure)?;
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(network_failure)?;
            out.write_all(&chunk).await.map_err(io_failure)?;
            self.progress
                .bytes
                .fetch_add(chunk.len() as u64, Ordering::SeqCst);
        }
        out.flush().await.map_err(io_failure)?;
        drop(out);
        tokio::fs::rename(&partial, &path).await.map_err(io_failure)
    }

    /// Up to eight subtitle tracks, the `language` ones first; tracks that fail are left out.
    async fn subtitles(
        &self,
        mut remote: Vec<VideoSubtitle>,
        language: &str,
    ) -> Vec<VideoSubtitle> {
        let language = language.to_ascii_lowercase();
        let matches = |track: &VideoSubtitle| {
            !language.is_empty()
                && track
                    .lang
                    .as_deref()
                    .is_some_and(|lang| lang.to_ascii_lowercase().starts_with(&language))
        };
        remote.sort_by_key(|track| !matches(track));
        let mut saved = Vec::new();
        for (index, track) in remote
            .into_iter()
            .filter(|track| track.url.starts_with("https://") || track.url.starts_with("http://"))
            .take(MAX_SUBTITLES)
            .enumerate()
        {
            let text = match self.text(&track.url).await {
                Ok(text) => text,
                Err(_) => {
                    tracing::info!(url = %track.url, "a subtitle track couldn't be saved");
                    continue;
                }
            };
            let format = track
                .format
                .clone()
                .or_else(|| subtitle_format(&track.url, &text));
            let name = format!("sub{}.{}", index + 1, format.as_deref().unwrap_or("srt"));
            if self.write(&name, text.as_bytes()).await.is_ok() {
                saved.push(VideoSubtitle {
                    url: name,
                    format,
                    ..track
                });
            }
        }
        saved
    }

    async fn text(&self, url: &str) -> Result<String, Failure> {
        let response = self.get(url).send().await.map_err(network_failure)?;
        let status = response.status();
        if !status.is_success() {
            return Err(Failure::Error(format!("The server answered {status}")));
        }
        let mut bytes = Vec::new();
        let mut body = response.bytes_stream();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(network_failure)?;
            if bytes.len() + chunk.len() > MAX_TEXT_BYTES {
                return Err(Failure::Error("The playlist is too large".into()));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    async fn write(&self, name: &str, bytes: &[u8]) -> Result<(), Failure> {
        tokio::fs::write(self.dir.join(name), bytes)
            .await
            .map_err(io_failure)
    }

    fn get(&self, url: &str) -> reqwest::RequestBuilder {
        let mut request = self.client.get(url);
        for (name, value) in self.headers {
            request = request.header(name, value);
        }
        request
    }
}

fn network_failure(error: reqwest::Error) -> Failure {
    Failure::Error(format!("The download broke off ({error})"))
}

/// `.mp4`, `.mkv`, `.webm`... from the URL when it's a video container; `.mp4` otherwise.
fn video_extension(url: &str) -> &'static str {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_lowercase();
    [".mkv", ".webm", ".mov", ".m4v", ".avi", ".ts"]
        .into_iter()
        .find(|extension| path.ends_with(extension))
        .unwrap_or(".mp4")
}

/// A subtitle file's format: from its URL, else `vtt` when it starts like WebVTT.
fn subtitle_format(url: &str, text: &str) -> Option<String> {
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_lowercase();
    ["srt", "vtt", "ass", "ssa", "ttml"]
        .into_iter()
        .find(|extension| path.ends_with(&format!(".{extension}")))
        .map(str::to_owned)
        .or_else(|| {
            text.trim_start_matches('\u{feff}')
                .starts_with("WEBVTT")
                .then(|| "vtt".to_owned())
        })
}

#[cfg(test)]
mod tests;
