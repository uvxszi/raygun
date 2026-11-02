//! Saving outside the app and exports (M46.1).
//!
//! Exports write a title (or several) as files the user keeps: CBZ, PDF or image folders for
//! manga, EPUB or text for novels, video files for downloaded episodes. Chapters that aren't on
//! the device go through the normal download queue first (one chapter at a time, so a big export
//! never hammers a source), and the job waits for them. Jobs report through [`ExportEvent`]s and
//! live in memory: Downloads → Exports lists them until the app closes.
//!
//! Finished downloads are also saved outside the app when Settings → Downloads asks for it
//! ([`save_finished_chapter`]); the app keeps its own copy for reading.

pub mod book;
pub mod cbz;
pub mod names;
pub mod output;
pub mod pages;
pub mod pdf;
pub mod video;

#[cfg(test)]
mod tests;

use std::collections::HashSet;
use std::io::BufWriter;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_db::titles::{self, TitleRow, UnitRow};
use raygun_model::{
    ChapterFormat, DownloadState, ExportDestination, ExportFormat, ExportJob, ExportPlan,
    ExportRequest, ExportScope, ExportState, SaveLocation, SavePrefs,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::{now_ms, reading_order, to_message};
use crate::comics::{self, COMICS, LOCAL_COMICS};
use crate::events::ExportEvent;
use crate::manga_downloads::MangaDownloadService;
use crate::startup::{AppDb, AppPaths};

pub use output::{Outputs, Place};

/// Finished jobs kept in the list.
const KEEP_FINISHED: usize = 30;
/// How often a job waiting on downloads looks at the queue.
const WAIT_STEP: Duration = Duration::from_secs(1);

const CBZ_MIME: &str = "application/vnd.comicbook+zip";
const PDF_MIME: &str = "application/pdf";
const EPUB_MIME: &str = "application/epub+zip";
const TEXT_MIME: &str = "text/plain";

pub struct ExportService {
    app: AppHandle,
    outputs: Outputs,
    client: reqwest::Client,
    jobs: Mutex<Vec<Job>>,
}

struct Job {
    job: ExportJob,
    cancel: Arc<AtomicBool>,
}

/// A title and the chapters an export takes from it, in reading order.
struct Chosen {
    title: TitleRow,
    units: Vec<UnitRow>,
}

/// What a request works out to.
struct Planned {
    titles: Vec<Chosen>,
    /// Chapters to download first.
    missing: Vec<String>,
    files: u32,
    problem: Option<String>,
}

/// The folder (and its sub-folders) where a medium's files go.
fn medium_folder(medium: &str) -> &'static str {
    match medium {
        "novel" => "Novels",
        "video" => "Videos",
        _ => "Manga",
    }
}

/// Where saved files go, from Settings → Downloads.
pub fn place_for(prefs: &SavePrefs, destination: ExportDestination) -> Result<Place, String> {
    match destination {
        ExportDestination::Downloads => Ok(Place::Downloads),
        ExportDestination::Share => Ok(Place::Share),
        ExportDestination::Folder => match &prefs.folder_uri {
            Some(uri) => Ok(Place::Folder {
                uri: uri.clone(),
                name: prefs.folder_name.clone().unwrap_or_else(|| "Folder".into()),
            }),
            None => Err("Choose a folder first".into()),
        },
    }
}

fn formats_for(medium: &str) -> &'static [ExportFormat] {
    match medium {
        "manga" => &[
            ExportFormat::Cbz,
            ExportFormat::Pdf,
            ExportFormat::PdfPerChapter,
            ExportFormat::Images,
        ],
        "novel" => &[ExportFormat::Epub, ExportFormat::Text],
        "video" => &[ExportFormat::Video],
        _ => &[],
    }
}

/// Picks the units a scope asks for, from units in reading order.
fn pick<'a>(
    ordered: Vec<&'a UnitRow>,
    scope: &ExportScope,
    downloaded: &HashSet<String>,
    local: bool,
) -> Vec<&'a UnitRow> {
    match scope {
        ExportScope::All => ordered,
        ExportScope::Downloaded => ordered
            .into_iter()
            .filter(|unit| local || downloaded.contains(&unit.id))
            .collect(),
        ExportScope::Unread => ordered
            .into_iter()
            .filter(|unit| unit.completed_at.is_none())
            .collect(),
        ExportScope::Range { from, to } => {
            let at = |id: &str| ordered.iter().position(|unit| unit.id == id);
            match (at(from), at(to)) {
                (Some(start), Some(end)) => {
                    let (start, end) = (start.min(end), start.max(end));
                    ordered[start..=end].to_vec()
                }
                _ => Vec::new(),
            }
        }
        ExportScope::Chosen { ids } => {
            let ids: HashSet<&String> = ids.iter().collect();
            ordered
                .into_iter()
                .filter(|unit| ids.contains(&unit.id))
                .collect()
        }
    }
}

impl ExportService {
    pub fn new(app: AppHandle, outputs: Outputs) -> Result<Self, reqwest::Error> {
        Ok(Self {
            app,
            outputs,
            client: raygun_net::http_client()?,
            jobs: Mutex::new(Vec::new()),
        })
    }

    pub fn outputs(&self) -> &Outputs {
        &self.outputs
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    fn downloads(&self) -> Arc<MangaDownloadService> {
        self.app
            .state::<Arc<MangaDownloadService>>()
            .inner()
            .clone()
    }

    async fn save_prefs(&self) -> SavePrefs {
        crate::settings::load(&self.db())
            .await
            .map(|settings| settings.save)
            .unwrap_or_default()
    }

    async fn plan_request(&self, request: &ExportRequest) -> Result<Planned, String> {
        if !formats_for(&request.medium).contains(&request.format) {
            return Err("That format doesn't fit this kind of title".into());
        }
        if request.medium == "video" {
            return video::plan(&self.app, request).await;
        }
        let db = self.db();
        let downloads = self.downloads();
        let mut titles = Vec::new();
        let mut missing = Vec::new();
        for title_id in &request.title_ids {
            let title = titles::title(&db, title_id)
                .await
                .map_err(to_message)?
                .ok_or_else(|| "A title is no longer stored".to_owned())?;
            let units = titles::units(&db, title_id).await.map_err(to_message)?;
            let ids: Vec<String> = units.iter().map(|unit| unit.id.clone()).collect();
            let states = downloads.for_chapters(&ids).await;
            let done: HashSet<String> = states
                .iter()
                .filter(|(_, download)| download.state == DownloadState::Done)
                .map(|(id, _)| id.clone())
                .collect();
            let local = title
                .fields
                .source_id
                .starts_with(crate::catalog::LOCAL_SOURCE_PREFIX);
            let chosen: Vec<UnitRow> = pick(reading_order(&units), &request.scope, &done, local)
                .into_iter()
                .cloned()
                .collect();
            if !local {
                missing.extend(
                    chosen
                        .iter()
                        .filter(|unit| !done.contains(&unit.id))
                        .map(|unit| unit.id.clone()),
                );
            }
            titles.push(Chosen {
                title,
                units: chosen,
            });
        }
        let chapters: usize = titles.iter().map(|chosen| chosen.units.len()).sum();
        let files = match request.format {
            ExportFormat::Pdf | ExportFormat::Epub | ExportFormat::Text => titles
                .iter()
                .filter(|chosen| !chosen.units.is_empty())
                .count(),
            _ => chapters,
        } as u32;
        let prefs = self.save_prefs().await;
        let problem = if chapters == 0 {
            Some("There's nothing to export".to_owned())
        } else if request.destination == ExportDestination::Share && files != 1 {
            Some("Sharing sends one file: pick PDF, EPUB or text, or a single chapter".to_owned())
        } else if request.format == ExportFormat::Images
            && request.destination == ExportDestination::Share
        {
            Some("A folder of images can't be shared".to_owned())
        } else {
            place_for(&prefs, request.destination).err()
        };
        Ok(Planned {
            titles,
            missing,
            files,
            problem,
        })
    }

    /// What an export would do, before it starts.
    pub async fn plan(&self, request: &ExportRequest) -> Result<ExportPlan, String> {
        let planned = self.plan_request(request).await?;
        Ok(ExportPlan {
            chapters: planned
                .titles
                .iter()
                .map(|chosen| chosen.units.len() as u32)
                .sum(),
            missing: planned.missing.len() as u32,
            files: planned.files,
            problem: planned.problem,
        })
    }

    /// Starts an export in the background.
    pub async fn start(self: &Arc<Self>, request: ExportRequest) -> Result<ExportJob, String> {
        let planned = self.plan_request(&request).await?;
        if let Some(problem) = planned.problem {
            return Err(problem);
        }
        let prefs = self.save_prefs().await;
        let place = place_for(&prefs, request.destination)?;
        let label = match planned.titles.as_slice() {
            [only] => only.title.fields.title.clone(),
            many => format!("{} titles", many.len()),
        };
        let to_write: u32 = planned
            .titles
            .iter()
            .map(|chosen| chosen.units.len() as u32)
            .sum();
        let job = ExportJob {
            id: uuid::Uuid::now_v7().to_string(),
            label,
            medium: request.medium.clone(),
            format: request.format,
            destination: request.destination,
            state: if planned.missing.is_empty() {
                ExportState::Writing
            } else {
                ExportState::Downloading
            },
            downloaded: 0,
            to_download: planned.missing.len() as u32,
            written: 0,
            to_write,
            files: Vec::new(),
            failed_chapters: 0,
            skipped_pages: 0,
            message: None,
            started_at: now_ms() as f64,
        };
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut jobs = self.jobs.lock().expect("export jobs poisoned");
            jobs.insert(
                0,
                Job {
                    job: job.clone(),
                    cancel: cancel.clone(),
                },
            );
        }
        self.emit(&job);
        let service = self.clone();
        let id = job.id.clone();
        tauri::async_runtime::spawn(async move {
            let result = service
                .run(
                    &id,
                    &request,
                    planned.titles,
                    planned.missing,
                    place,
                    &cancel,
                )
                .await;
            service.update(&id, |job| match result {
                Ok(()) if cancel.load(Ordering::SeqCst) => job.state = ExportState::Cancelled,
                Ok(()) => job.state = ExportState::Done,
                Err(message) if cancel.load(Ordering::SeqCst) => {
                    job.state = ExportState::Cancelled;
                    tracing::debug!(%message, "export cancelled");
                }
                Err(message) => {
                    tracing::warn!(%message, "export failed");
                    job.state = ExportState::Failed;
                    job.message = Some(message);
                }
            });
            service.trim();
        });
        Ok(job)
    }

    pub fn cancel(&self, id: &str) {
        let jobs = self.jobs.lock().expect("export jobs poisoned");
        if let Some(job) = jobs.iter().find(|job| job.job.id == id) {
            job.cancel.store(true, Ordering::SeqCst);
        }
    }

    /// Removes a finished job from the list.
    pub fn dismiss(&self, id: &str) {
        let mut jobs = self.jobs.lock().expect("export jobs poisoned");
        jobs.retain(|job| job.job.id != id || is_running(&job.job));
        drop(jobs);
        let _ = ExportEvent(None).emit(&self.app);
    }

    pub fn jobs(&self) -> Vec<ExportJob> {
        self.jobs
            .lock()
            .expect("export jobs poisoned")
            .iter()
            .map(|job| job.job.clone())
            .collect()
    }

    fn trim(&self) {
        let mut jobs = self.jobs.lock().expect("export jobs poisoned");
        let mut finished = 0;
        jobs.retain(|job| {
            if is_running(&job.job) {
                return true;
            }
            finished += 1;
            finished <= KEEP_FINISHED
        });
    }

    fn emit(&self, job: &ExportJob) {
        let _ = ExportEvent(Some(job.clone())).emit(&self.app);
    }

    fn update(&self, id: &str, change: impl FnOnce(&mut ExportJob)) {
        let job = {
            let mut jobs = self.jobs.lock().expect("export jobs poisoned");
            let Some(entry) = jobs.iter_mut().find(|job| job.job.id == id) else {
                return;
            };
            change(&mut entry.job);
            entry.job.clone()
        };
        self.emit(&job);
    }

    async fn run(
        &self,
        id: &str,
        request: &ExportRequest,
        titles: Vec<Chosen>,
        missing: Vec<String>,
        place: Place,
        cancel: &AtomicBool,
    ) -> Result<(), String> {
        let failed = self.wait_for_downloads(id, &missing, cancel).await?;
        self.update(id, |job| {
            job.state = ExportState::Writing;
            job.failed_chapters = failed.len() as u32;
            job.to_write = job.to_write.saturating_sub(failed.len() as u32);
        });
        if place == Place::Share {
            self.outputs.clear_shared();
        }
        let mut shared = Vec::new();
        for chosen in titles {
            let units: Vec<UnitRow> = chosen
                .units
                .iter()
                .filter(|unit| !failed.contains(&unit.id))
                .cloned()
                .collect();
            let chosen = Chosen {
                title: chosen.title,
                units,
            };
            if chosen.units.is_empty() {
                continue;
            }
            let saved = match request.format {
                ExportFormat::Cbz | ExportFormat::Images | ExportFormat::PdfPerChapter => {
                    self.write_chapters(id, &chosen, request.format, &place, cancel)
                        .await?
                }
                ExportFormat::Pdf => vec![self.write_pdf(id, &chosen, &place, cancel).await?],
                ExportFormat::Epub | ExportFormat::Text => {
                    vec![
                        self.write_book(id, &chosen, request.format, &place, cancel)
                            .await?,
                    ]
                }
                ExportFormat::Video => {
                    video::write(self, id, &chosen.title, &chosen.units, &place, cancel).await?
                }
            };
            for file in saved {
                if let Some(path) = file.path {
                    shared.push((path, mime_for(request.format), file.shown.clone()));
                }
                self.update(id, |job| job.files.push(file.shown));
            }
        }
        if let Some((path, mime, name)) = shared.into_iter().next() {
            let outputs = self.outputs.clone();
            tauri::async_runtime::spawn_blocking(move || outputs.share(&path, mime, &name))
                .await
                .map_err(to_message)??;
        }
        Ok(())
    }

    /// Queues `missing` and waits until each is downloaded or failed. Returns the failed ones.
    async fn wait_for_downloads(
        &self,
        id: &str,
        missing: &[String],
        cancel: &AtomicBool,
    ) -> Result<HashSet<String>, String> {
        if missing.is_empty() {
            return Ok(HashSet::new());
        }
        let downloads = self.downloads();
        downloads.enqueue(missing).await?;
        loop {
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelled".into());
            }
            let states = downloads.for_chapters(missing).await;
            let done = missing
                .iter()
                .filter(|unit| {
                    states
                        .get(*unit)
                        .is_some_and(|download| download.state == DownloadState::Done)
                })
                .count();
            // A chapter that failed, or was taken off the queue, is left out.
            let failed: HashSet<String> = missing
                .iter()
                .filter(|unit| {
                    states
                        .get(*unit)
                        .is_none_or(|download| download.state == DownloadState::Failed)
                })
                .cloned()
                .collect();
            let paused = downloads.is_paused();
            self.update(id, |job| {
                job.downloaded = done as u32;
                job.message = paused.then(|| "Downloads are paused".to_owned());
            });
            if done + failed.len() >= missing.len() {
                return Ok(failed);
            }
            tokio::time::sleep(WAIT_STEP).await;
        }
    }

    /// A chapter's page files: from its download, or for imported comics, where they were
    /// unpacked.
    async fn page_files(&self, title: &TitleRow, unit: &UnitRow) -> Result<Vec<PathBuf>, String> {
        if title.fields.source_id == LOCAL_COMICS {
            let dir = self
                .app
                .state::<AppPaths>()
                .0
                .downloads()
                .join(COMICS)
                .join(&title.fields.remote_id)
                .join(&unit.fields.remote_id);
            let listed = dir.clone();
            let names = tauri::async_runtime::spawn_blocking(move || comics::pages(&listed))
                .await
                .map_err(to_message)?
                .map_err(|_| "A chapter is missing from the device".to_owned())?;
            return Ok(names.iter().map(|name| dir.join(name)).collect());
        }
        self.downloads()
            .page_files(&title.id, &unit.id)
            .await
            .ok_or_else(|| format!("{} isn't on the device", unit.fields.name))
    }

    fn comic_info(&self, title: &TitleRow, unit: &UnitRow) -> cbz::ComicInfo {
        cbz::ComicInfo {
            series: title.fields.title.clone(),
            title: unit.fields.name.clone(),
            number: unit.fields.number,
            summary: title.fields.description.clone(),
            writers: title.fields.authors.clone(),
            pencillers: title.fields.artists.clone(),
            translator: unit.fields.scanlator.clone(),
            genres: title.fields.genres.clone(),
            web: title.fields.url.clone(),
            language: None,
            page_count: 0,
        }
    }

    /// One file (or folder, for images) per chapter.
    async fn write_chapters(
        &self,
        id: &str,
        chosen: &Chosen,
        format: ExportFormat,
        place: &Place,
        cancel: &AtomicBool,
    ) -> Result<Vec<output::Saved>, String> {
        let mut saved = Vec::new();
        let series = names::safe_name(&chosen.title.fields.title);
        for unit in &chosen.units {
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelled".into());
            }
            let pages = self.page_files(&chosen.title, unit).await?;
            let stem = names::chapter_stem(unit.fields.number, &unit.fields.name);
            let folder = vec![medium_folder("manga").to_owned(), series.clone()];
            let outputs = self.outputs.clone();
            let place = place.clone();
            let written = match format {
                ExportFormat::Cbz => {
                    let info = self.comic_info(&chosen.title, unit);
                    let name = names::file_name(&stem, "cbz");
                    tauri::async_runtime::spawn_blocking(move || {
                        write_one(&outputs, &place, &folder, &name, CBZ_MIME, |file| {
                            cbz::write(file, &pages, &info).map(|()| 0)
                        })
                    })
                    .await
                    .map_err(to_message)?
                    .map(|(file, _)| vec![file])?
                }
                ExportFormat::PdfPerChapter => {
                    let name = names::file_name(&stem, "pdf");
                    let title = format!("{} – {}", chosen.title.fields.title, unit.fields.name);
                    let (file, skipped) = tauri::async_runtime::spawn_blocking(move || {
                        write_one(&outputs, &place, &folder, &name, PDF_MIME, |file| {
                            let mut pdf = pdf::PdfWriter::new(BufWriter::new(file))?;
                            for page in &pages {
                                pdf.add_image(&std::fs::read(page)?)?;
                            }
                            let skipped = pdf.skipped();
                            pdf.finish(&title)?;
                            Ok(skipped)
                        })
                    })
                    .await
                    .map_err(to_message)??;
                    self.update(id, |job| job.skipped_pages += skipped);
                    vec![file]
                }
                _ => {
                    // A folder of images, named in page order.
                    let mut folder = folder;
                    folder.push(names::safe_name(&stem));
                    let width = pages.len().to_string().len().max(3);
                    tauri::async_runtime::spawn_blocking(move || {
                        let mut first = None;
                        for (index, page) in pages.iter().enumerate() {
                            let extension = page
                                .extension()
                                .and_then(|extension| extension.to_str())
                                .unwrap_or("jpg")
                                .to_owned();
                            let name = format!("{:0width$}.{extension}", index + 1);
                            let mime = image_mime(&extension);
                            let (file, _) =
                                write_one(&outputs, &place, &folder, &name, mime, |file| {
                                    std::io::copy(&mut std::fs::File::open(page)?, file).map(|_| 0)
                                })?;
                            first.get_or_insert(file);
                        }
                        Ok::<_, String>(first.into_iter().collect::<Vec<_>>())
                    })
                    .await
                    .map_err(to_message)??
                }
            };
            saved.extend(written);
            self.update(id, |job| job.written += 1);
        }
        Ok(saved)
    }

    /// One PDF for the whole title, a bookmark per chapter.
    async fn write_pdf(
        &self,
        id: &str,
        chosen: &Chosen,
        place: &Place,
        cancel: &AtomicBool,
    ) -> Result<output::Saved, String> {
        let mut chapters = Vec::new();
        for unit in &chosen.units {
            chapters.push((
                unit.fields.name.clone(),
                self.page_files(&chosen.title, unit).await?,
            ));
        }
        let name = names::file_name(&chosen.title.fields.title, "pdf");
        let title = chosen.title.fields.title.clone();
        let outputs = self.outputs.clone();
        let place = place.clone();
        let folder = vec![medium_folder("manga").to_owned()];
        // Progress and cancelling from inside the blocking writer.
        let (progress, mut ticks) = tokio::sync::mpsc::unbounded_channel::<()>();
        let stop = Arc::new(AtomicBool::new(false));
        let stopped = stop.clone();
        let writer = tauri::async_runtime::spawn_blocking(move || {
            write_one(&outputs, &place, &folder, &name, PDF_MIME, |file| {
                let mut pdf = pdf::PdfWriter::new(BufWriter::new(file))?;
                for (chapter, pages) in &chapters {
                    if stopped.load(Ordering::SeqCst) {
                        return Err(std::io::Error::other("Cancelled"));
                    }
                    pdf.bookmark(chapter);
                    for page in pages {
                        pdf.add_image(&std::fs::read(page)?)?;
                    }
                    let _ = progress.send(());
                }
                let skipped = pdf.skipped();
                pdf.finish(&title)?;
                Ok(skipped)
            })
        });
        let watcher = async {
            while ticks.recv().await.is_some() {
                self.update(id, |job| job.written += 1);
                if cancel.load(Ordering::SeqCst) {
                    stop.store(true, Ordering::SeqCst);
                }
            }
        };
        let (result, ()) = tokio::join!(writer, watcher);
        let (file, skipped) = result.map_err(to_message)??;
        self.update(id, |job| job.skipped_pages += skipped);
        Ok(file)
    }

    /// A novel as one EPUB or text file.
    async fn write_book(
        &self,
        id: &str,
        chosen: &Chosen,
        format: ExportFormat,
        place: &Place,
        cancel: &AtomicBool,
    ) -> Result<output::Saved, String> {
        let book = self
            .book(id, chosen, format == ExportFormat::Epub, cancel)
            .await?;
        let outputs = self.outputs.clone();
        let place = place.clone();
        let folder = vec![medium_folder("novel").to_owned()];
        tauri::async_runtime::spawn_blocking(move || {
            let (name, mime) = if format == ExportFormat::Epub {
                (names::file_name(&book.title, "epub"), EPUB_MIME)
            } else {
                (names::file_name(&book.title, "txt"), TEXT_MIME)
            };
            write_one(&outputs, &place, &folder, &name, mime, |file| {
                if format == ExportFormat::Epub {
                    book::write_epub(file, &book)?;
                } else {
                    std::io::Write::write_all(file, book::text(&book).as_bytes())?;
                }
                Ok(0)
            })
            .map(|(file, _)| file)
        })
        .await
        .map_err(to_message)?
    }

    async fn book(
        &self,
        id: &str,
        chosen: &Chosen,
        with_cover: bool,
        cancel: &AtomicBool,
    ) -> Result<book::Book, String> {
        let novels = self.app.state::<crate::novel::NovelService>();
        let mut chapters = Vec::new();
        for unit in &chosen.units {
            if cancel.load(Ordering::SeqCst) {
                return Err("Cancelled".into());
            }
            let chapter = novels.chapter(&chosen.title.id, &unit.id).await?;
            chapters.push(book::BookChapter {
                title: unit.fields.name.clone(),
                blocks: chapter.blocks,
            });
            self.update(id, |job| job.written += 1);
        }
        let cover = if with_cover {
            self.cover(&chosen.title).await
        } else {
            None
        };
        let fields = &chosen.title.fields;
        Ok(book::Book {
            id: format!("urn:raygun:{}", chosen.title.id),
            title: fields.title.clone(),
            authors: fields.authors.clone(),
            description: fields.description.clone(),
            language: "und".into(),
            genres: fields.genres.clone(),
            cover,
            chapters,
            modified: chrono::DateTime::from_timestamp_millis(now_ms())
                .unwrap_or_default()
                .format("%Y-%m-%dT%H:%M:%SZ")
                .to_string(),
        })
    }

    /// The title's cover image, through the image proxy (which knows its headers).
    async fn cover(&self, title: &TitleRow) -> Option<(Vec<u8>, String)> {
        let url = title.fields.cover.as_deref()?;
        let link = self
            .app
            .state::<crate::manga::MangaService>()
            .catalog()
            .cover(url, &title.fields.cover_headers);
        let bytes = self
            .client
            .get(&link)
            .send()
            .await
            .ok()?
            .bytes()
            .await
            .ok()?;
        let extension = match raygun_net::page_cache::sniff_content_type(&bytes) {
            "image/png" => "png",
            "image/gif" => "gif",
            "image/webp" => "webp",
            "image/jpeg" => "jpg",
            _ => return None,
        };
        Some((bytes.to_vec(), extension.into()))
    }
}

fn is_running(job: &ExportJob) -> bool {
    matches!(job.state, ExportState::Downloading | ExportState::Writing)
}

fn mime_for(format: ExportFormat) -> &'static str {
    match format {
        ExportFormat::Cbz => CBZ_MIME,
        ExportFormat::Pdf | ExportFormat::PdfPerChapter => PDF_MIME,
        ExportFormat::Epub => EPUB_MIME,
        ExportFormat::Text => TEXT_MIME,
        ExportFormat::Images => "image/*",
        ExportFormat::Video => "video/*",
    }
}

pub fn image_mime(extension: &str) -> &'static str {
    match extension {
        "png" => "image/png",
        "gif" => "image/gif",
        "webp" => "image/webp",
        "avif" => "image/avif",
        "jxl" => "image/jxl",
        "bmp" => "image/bmp",
        _ => "image/jpeg",
    }
}

/// Makes one file, lets `write` fill it, and publishes it (or removes it when writing failed).
/// Blocking. `write` returns a count passed back (pages a PDF skipped).
pub fn write_one(
    outputs: &Outputs,
    place: &Place,
    folder: &[String],
    name: &str,
    mime: &str,
    write: impl FnOnce(&mut std::fs::File) -> std::io::Result<u32>,
) -> Result<(output::Saved, u32), String> {
    let mut output = outputs.create(place, folder, name, mime)?;
    match write(&mut output.file) {
        Ok(count) => Ok((outputs.finish(output, true)?, count)),
        Err(error) => {
            let _ = outputs.finish(output, false);
            Err(error.to_string())
        }
    }
}

/// Saves a chapter that just finished downloading outside the app, when Settings → Downloads
/// asks for it. Runs in the background; a failure shows in Downloads → Exports.
pub fn save_finished_chapter(app: &AppHandle, title_id: &str, unit_id: &str) {
    let app = app.clone();
    let (title_id, unit_id) = (title_id.to_owned(), unit_id.to_owned());
    tauri::async_runtime::spawn(async move {
        let Some(service) = app.try_state::<Arc<ExportService>>() else {
            return;
        };
        let service = service.inner().clone();
        if let Err(message) = service.save_chapter(&title_id, &unit_id).await {
            tracing::warn!(%message, chapter = %unit_id, "could not save a download outside the app");
            service.record_failure(&title_id, message).await;
        }
    });
}

impl ExportService {
    async fn save_chapter(&self, title_id: &str, unit_id: &str) -> Result<(), String> {
        let prefs = self.save_prefs().await;
        let destination = match prefs.location {
            SaveLocation::App => return Ok(()),
            SaveLocation::Downloads => ExportDestination::Downloads,
            SaveLocation::Folder => ExportDestination::Folder,
        };
        let place = place_for(&prefs, destination)?;
        let db = self.db();
        let title = titles::title(&db, title_id)
            .await
            .map_err(to_message)?
            .ok_or("The title is no longer stored")?;
        let unit = titles::unit(&db, unit_id)
            .await
            .map_err(to_message)?
            .ok_or("The chapter is no longer stored")?;
        let chosen = Chosen {
            title,
            units: vec![unit],
        };
        let never = AtomicBool::new(false);
        if chosen.title.fields.kind == "novel" {
            // One text file per chapter, in a folder per title.
            let book = self.book("", &chosen, false, &never).await?;
            let unit = &chosen.units[0];
            let name = names::file_name(
                &names::chapter_stem(unit.fields.number, &unit.fields.name),
                "txt",
            );
            let folder = vec![
                medium_folder("novel").to_owned(),
                names::safe_name(&chosen.title.fields.title),
            ];
            let outputs = self.outputs.clone();
            tauri::async_runtime::spawn_blocking(move || {
                write_one(&outputs, &place, &folder, &name, TEXT_MIME, |file| {
                    std::io::Write::write_all(file, book::text(&book).as_bytes()).map(|()| 0)
                })
            })
            .await
            .map_err(to_message)??;
            return Ok(());
        }
        let format = match prefs.chapter_format {
            ChapterFormat::Cbz => ExportFormat::Cbz,
            ChapterFormat::Images => ExportFormat::Images,
        };
        self.write_chapters("", &chosen, format, &place, &never)
            .await
            .map(|_| ())
    }

    /// Lists a failed save so the user sees it.
    async fn record_failure(&self, title_id: &str, message: String) {
        let label = match titles::title(&self.db(), title_id).await {
            Ok(Some(title)) => title.fields.title,
            _ => "A download".into(),
        };
        let job = ExportJob {
            id: uuid::Uuid::now_v7().to_string(),
            label,
            medium: "manga".into(),
            format: ExportFormat::Cbz,
            destination: ExportDestination::Folder,
            state: ExportState::Failed,
            downloaded: 0,
            to_download: 0,
            written: 0,
            to_write: 1,
            files: Vec::new(),
            failed_chapters: 0,
            skipped_pages: 0,
            message: Some(message),
            started_at: now_ms() as f64,
        };
        self.jobs.lock().expect("export jobs poisoned").insert(
            0,
            Job {
                job: job.clone(),
                cancel: Arc::new(AtomicBool::new(false)),
            },
        );
        self.emit(&job);
        self.trim();
    }
}
