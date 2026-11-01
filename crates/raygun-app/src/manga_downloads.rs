//! Chapter downloads (M28): a queue in `local_manga_download` worked by one background task.
//! Pages land in `downloads/manga/<title>/<chapter>/` and are served back through the stream
//! proxy's `/local` route, so a downloaded chapter reads without the network.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use raygun_db::manga_downloads::{self, DownloadRow};
use raygun_db::titles;
use raygun_ext_host::novel::PassageBlock;
use raygun_model::{ChapterDownload, DownloadState, MangaDownloadItem, MangaDownloads};
use raygun_net::ImageLinks;
use raygun_net::images::{DownloadError, RemoteImage, download_images, downloaded_pages};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::events::MangaDownloadsEvent;
use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// Pages fetched at once; sources such as MangaDex limit how fast one client may go.
const PAGE_CONCURRENCY: usize = 2;
/// Folders under the downloads root; `manga` is also the first segment of page URLs.
const FOLDER: &str = "manga";
const NOVEL_FOLDER: &str = "novel";
/// A downloaded novel chapter's blocks, in its folder.
const PASSAGE_FILE: &str = "passage.json";

pub struct MangaDownloadService {
    app: AppHandle,
    /// The downloads folder.
    root: PathBuf,
    images: ImageLinks,
    client: reqwest::Client,
    wake: Notify,
    paused: AtomicBool,
    /// Chapters cancelled while their download runs; the worker drops what it fetched.
    cancelled: Mutex<HashSet<String>>,
}

impl MangaDownloadService {
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

    /// Starts the worker. Downloads cut off by the last exit go back to the queue first.
    pub fn start(self: &Arc<Self>) {
        let downloads = self.clone();
        tauri::async_runtime::spawn(async move {
            match manga_downloads::reset_interrupted(&downloads.db()).await {
                Ok(0) => {}
                Ok(count) => tracing::info!(count, "resuming interrupted downloads"),
                Err(error) => tracing::warn!(%error, "could not reset interrupted downloads"),
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
            match manga_downloads::next_queued(&self.db()).await {
                Ok(Some(row)) => self.download(row).await,
                Ok(None) => woken.await,
                Err(error) => {
                    tracing::warn!(%error, "download queue unreadable");
                    woken.await;
                }
            }
        }
    }

    async fn download(&self, row: DownloadRow) {
        let db = self.db();
        let unit_id = row.unit_id.clone();
        if let Err(error) = manga_downloads::start(&db, &unit_id).await {
            tracing::warn!(%error, "could not start a download");
            return;
        }
        self.changed(&unit_id);
        let started = std::time::Instant::now();
        let result = self.fetch(&row).await;
        let cancelled = self
            .cancelled
            .lock()
            .expect("cancel set poisoned")
            .remove(&unit_id);
        let outcome = match result {
            _ if cancelled => {
                self.delete_files(&row.title_id, &unit_id).await;
                Ok(())
            }
            Ok(pages) => {
                tracing::info!(
                    chapter = %unit_id,
                    pages,
                    elapsed_ms = started.elapsed().as_millis() as u64,
                    "chapter downloaded"
                );
                manga_downloads::finish(&db, &unit_id, now_ms()).await
            }
            Err(Failure::Stopped) => manga_downloads::requeue(&db, &unit_id).await,
            Err(Failure::Error(message)) => {
                tracing::warn!(chapter = %unit_id, %message, "chapter download failed");
                manga_downloads::fail(&db, &unit_id, &message).await
            }
        };
        if let Err(error) = outcome {
            tracing::warn!(%error, "could not record a download's outcome");
        }
        self.changed(&unit_id);
    }

    /// Fetches the page list and the pages; returns how many pages were saved.
    async fn fetch(&self, row: &DownloadRow) -> Result<usize, Failure> {
        let db = self.db();
        let failed = |error: raygun_db::DbError| Failure::Error(error.to_string());
        let title = titles::title(&db, &row.title_id)
            .await
            .map_err(failed)?
            .ok_or_else(|| Failure::Error("The title is no longer stored".into()))?;
        let unit = titles::unit(&db, &row.unit_id)
            .await
            .map_err(failed)?
            .ok_or_else(|| Failure::Error("The chapter is no longer stored".into()))?;
        if title.fields.kind == "novel" {
            return self
                .fetch_passage(row, &title.fields.source_id, &unit.fields.remote_id)
                .await;
        }
        let pages = {
            let extensions = self.app.state::<Arc<ExtensionService>>();
            let (_ready, host) = extensions.ready().await;
            host.manga_pages(&title.fields.source_id, &unit.fields.remote_id)
                .await
                .map_err(Failure::Error)?
        };
        if pages.is_empty() {
            return Err(Failure::Error("The source listed no pages".into()));
        }
        let total = pages.len();
        manga_downloads::set_progress(&db, &row.unit_id, 0, total as i64)
            .await
            .map_err(failed)?;
        self.changed(&row.unit_id);

        let images: Vec<RemoteImage> = pages
            .into_iter()
            .map(|page| RemoteImage {
                url: page.url,
                headers: page.headers,
            })
            .collect();
        // Progress is written from a side task so the page callback stays synchronous.
        let done = Arc::new(AtomicUsize::new(0));
        let progress = Arc::new(Notify::new());
        let reporter = {
            let (done, progress, db) = (done.clone(), progress.clone(), db.clone());
            let app = self.app.clone();
            let unit_id = row.unit_id.clone();
            tauri::async_runtime::spawn(async move {
                let mut written = 0;
                loop {
                    progress.notified().await;
                    let now = done.load(Ordering::SeqCst);
                    if now != written {
                        written = now;
                        let _ =
                            manga_downloads::set_progress(&db, &unit_id, now as i64, total as i64)
                                .await;
                        let _ = MangaDownloadsEvent(unit_id.clone()).emit(&app);
                    }
                }
            })
        };
        let result = download_images(
            &self.client,
            &images,
            &self.chapter_dir(&row.title_id, &row.unit_id),
            PAGE_CONCURRENCY,
            |count| {
                done.store(count, Ordering::SeqCst);
                progress.notify_one();
            },
            || self.paused.load(Ordering::SeqCst) || self.is_cancelled(&row.unit_id),
        )
        .await;
        reporter.abort();
        match result {
            Ok(()) => Ok(total),
            Err(DownloadError::Stopped) => Err(Failure::Stopped),
            Err(error) => Err(Failure::Error(error.to_string())),
        }
    }

    /// Downloads a novel chapter's text: its blocks, written through a `.part` folder.
    async fn fetch_passage(
        &self,
        row: &DownloadRow,
        source_id: &str,
        remote_id: &str,
    ) -> Result<usize, Failure> {
        if source_id == crate::novel::LOCAL_EPUB {
            return Err(Failure::Error("This book is already on the device".into()));
        }
        let failed = |error: raygun_db::DbError| Failure::Error(error.to_string());
        manga_downloads::set_progress(&self.db(), &row.unit_id, 0, 1)
            .await
            .map_err(failed)?;
        self.changed(&row.unit_id);
        let passage = {
            let extensions = self.app.state::<Arc<ExtensionService>>();
            let (_ready, host) = extensions.ready().await;
            host.novel_passage(source_id, remote_id)
                .await
                .map_err(Failure::Error)?
        };
        if self.is_cancelled(&row.unit_id) {
            return Err(Failure::Stopped);
        }
        let json =
            serde_json::to_vec(&passage).map_err(|error| Failure::Error(error.to_string()))?;
        let dir = self.novel_dir(&row.title_id, &row.unit_id);
        let partial = dir.with_extension("part");
        let io = |error: std::io::Error| Failure::Error(error.to_string());
        let _ = tokio::fs::remove_dir_all(&partial).await;
        tokio::fs::create_dir_all(&partial).await.map_err(io)?;
        tokio::fs::write(partial.join(PASSAGE_FILE), json)
            .await
            .map_err(io)?;
        let _ = tokio::fs::remove_dir_all(&dir).await;
        tokio::fs::rename(&partial, &dir).await.map_err(io)?;
        Ok(1)
    }

    /// A downloaded novel chapter's blocks, or None when it isn't on the device.
    pub async fn local_passage(
        &self,
        title_id: &str,
        chapter_id: &str,
    ) -> Option<Vec<PassageBlock>> {
        let db = self.db();
        let row = manga_downloads::get(&db, chapter_id).await.ok()??;
        if row.state != manga_downloads::DONE {
            return None;
        }
        let file = self.novel_dir(title_id, chapter_id).join(PASSAGE_FILE);
        match tokio::fs::read(&file).await {
            Ok(bytes) => serde_json::from_slice(&bytes).ok(),
            Err(_) => {
                tracing::info!(chapter = chapter_id, "downloaded chapter is gone from disk");
                let _ = manga_downloads::remove(&db, chapter_id).await;
                None
            }
        }
    }

    /// Removes a chapter's downloaded files, manga or novel.
    async fn delete_files(&self, title_id: &str, unit_id: &str) {
        for dir in [
            self.chapter_dir(title_id, unit_id),
            self.novel_dir(title_id, unit_id),
        ] {
            if tokio::fs::try_exists(&dir).await.unwrap_or(false)
                && let Err(error) = tokio::fs::remove_dir_all(&dir).await
            {
                tracing::warn!(%error, "could not delete a downloaded chapter");
            }
        }
    }

    /// Queues chapters (any already queued or downloaded are left alone; failed ones retry).
    pub async fn enqueue(&self, chapter_ids: &[String]) -> Result<(), String> {
        let db = self.db();
        let now = now_ms();
        for (offset, id) in chapter_ids.iter().enumerate() {
            let unit = titles::unit(&db, id)
                .await
                .map_err(to_message)?
                .ok_or_else(|| "This chapter isn't stored".to_owned())?;
            // Keeps the order chapters were picked in.
            manga_downloads::enqueue(&db, id, &unit.title_id, now + offset as i64)
                .await
                .map_err(to_message)?;
        }
        self.wake.notify_one();
        self.changed("");
        Ok(())
    }

    /// Takes chapters out of the queue, stopping a running one; downloaded files are deleted.
    pub async fn remove(&self, chapter_ids: &[String]) -> Result<(), String> {
        let db = self.db();
        for id in chapter_ids {
            let Some(row) = manga_downloads::get(&db, id).await.map_err(to_message)? else {
                continue;
            };
            if row.state == manga_downloads::DOWNLOADING {
                self.cancelled
                    .lock()
                    .expect("cancel set poisoned")
                    .insert(id.clone());
            }
            manga_downloads::remove(&db, id).await.map_err(to_message)?;
            self.delete_files(&row.title_id, id).await;
        }
        self.changed("");
        Ok(())
    }

    pub fn set_paused(&self, paused: bool) {
        self.paused.store(paused, Ordering::SeqCst);
        if !paused {
            self.wake.notify_one();
        }
        self.changed("");
    }

    /// The queue (waiting, running and failed chapters) and how many are downloaded.
    pub async fn queue(&self) -> Result<MangaDownloads, String> {
        let entries = manga_downloads::entries(&self.db())
            .await
            .map_err(to_message)?;
        let downloaded = entries
            .iter()
            .filter(|entry| entry.download.state == manga_downloads::DONE)
            .count() as u32;
        Ok(MangaDownloads {
            paused: self.paused.load(Ordering::SeqCst),
            items: entries
                .into_iter()
                .filter(|entry| entry.download.state != manga_downloads::DONE)
                .map(|entry| MangaDownloadItem {
                    chapter_id: entry.download.unit_id.clone(),
                    title_id: entry.download.title_id.clone(),
                    download: chapter_download(&entry.download),
                    title: entry.title,
                    chapter: entry.chapter,
                })
                .collect(),
            downloaded,
        })
    }

    /// Download states of a title's chapters, by chapter id.
    pub async fn for_title(&self, title_id: &str) -> HashMap<String, ChapterDownload> {
        rows_by_unit(manga_downloads::for_title(&self.db(), title_id).await)
    }

    /// Download states of the given chapters, by chapter id.
    pub async fn for_chapters(&self, chapter_ids: &[String]) -> HashMap<String, ChapterDownload> {
        rows_by_unit(manga_downloads::for_units(&self.db(), chapter_ids).await)
    }

    /// Proxy URLs of a downloaded chapter's pages, or None when it isn't (fully) on the device.
    /// A finished row whose folder went missing is dropped.
    pub async fn local_pages(&self, title_id: &str, chapter_id: &str) -> Option<Vec<String>> {
        let db = self.db();
        let row = manga_downloads::get(&db, chapter_id).await.ok()??;
        if row.state != manga_downloads::DONE {
            return None;
        }
        let dir = self.chapter_dir(title_id, chapter_id);
        let names = tokio::task::spawn_blocking(move || downloaded_pages(&dir))
            .await
            .ok()
            .and_then(Result::ok)
            .filter(|names| !names.is_empty());
        let Some(names) = names else {
            tracing::info!(chapter = chapter_id, "downloaded chapter is gone from disk");
            let _ = manga_downloads::remove(&db, chapter_id).await;
            return None;
        };
        Some(
            names
                .iter()
                .map(|name| {
                    self.images
                        .local(&format!("{FOLDER}/{title_id}/{chapter_id}/{name}"))
                })
                .collect(),
        )
    }

    fn is_cancelled(&self, unit_id: &str) -> bool {
        self.cancelled
            .lock()
            .expect("cancel set poisoned")
            .contains(unit_id)
    }

    fn chapter_dir(&self, title_id: &str, unit_id: &str) -> PathBuf {
        self.root.join(FOLDER).join(title_id).join(unit_id)
    }

    fn novel_dir(&self, title_id: &str, unit_id: &str) -> PathBuf {
        self.root.join(NOVEL_FOLDER).join(title_id).join(unit_id)
    }

    fn changed(&self, unit_id: &str) {
        if let Err(error) = MangaDownloadsEvent(unit_id.to_owned()).emit(&self.app) {
            tracing::debug!(%error, "failed to emit downloads event");
        }
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }
}

enum Failure {
    Stopped,
    Error(String),
}

fn rows_by_unit(
    rows: Result<Vec<DownloadRow>, raygun_db::DbError>,
) -> HashMap<String, ChapterDownload> {
    match rows {
        Ok(rows) => rows
            .iter()
            .map(|row| (row.unit_id.clone(), chapter_download(row)))
            .collect(),
        Err(error) => {
            tracing::warn!(%error, "could not read download states");
            HashMap::new()
        }
    }
}

pub fn chapter_download(row: &DownloadRow) -> ChapterDownload {
    ChapterDownload {
        state: DownloadState::parse(&row.state),
        pages_done: row.pages_done.max(0) as u32,
        pages_total: row.pages_total.map(|total| total.max(0) as u32),
        error: row.error.clone(),
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
