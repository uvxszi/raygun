//! Manga (M24): browsing `manga.source` extensions, titles with their chapters, the manga library
//! and reading progress. Covers and pages are linked through the stream proxy's image route.
//! Library updates and the updates feed arrived in M28; the catalogue code is shared with novels
//! through `Catalog` (M31). Comics imported from the device (M46, `comics`) are manga of the
//! `local.comics` source, read from their unpacked files.

use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{StreamExt, stream};
use raygun_db::titles::{self, TitleFields, TitleRow, UnitFields};
use raygun_ext_host::manga::MANGA_SOURCE;
use raygun_model::{
    MangaBrowseMode, MangaChapterInfo, MangaChapterPages, MangaLibraryEntry, MangaListingPage,
    MangaSource, MangaTitleView, MangaUpdateItem, MangaUpdateStatus, ReaderMode, UpdateFailure,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::catalog::{
    Catalog, LOCAL_PREFIX, LOCAL_SOURCE_PREFIX, chapter_info, now_ms, to_message,
};
use crate::comics::{self, COMICS, LOCAL_COMICS};
use crate::events::MangaUpdateEvent;
use crate::manga_downloads::MangaDownloadService;
use crate::notifications::news::{self, NewsKind};
use crate::novel::NovelService;
use crate::stremio::StremioService;
use crate::video_library;
use crate::video_sources::VideoSourceService;

const KIND: &str = "manga";
/// Books from sources get new chapters too, so library updates and the feed cover them.
const NOVEL_KIND: &str = "novel";
/// Titles refreshed at once during a library update.
const UPDATE_CONCURRENCY: usize = 2;
/// The newest updates the feed shows.
const FEED_LIMIT: u32 = 300;
/// `local_meta` key holding when the last library update finished (unix ms).
const LAST_UPDATE_KEY: &str = "manga.lastUpdate";
/// The scheduler's first look after launch, then how often it checks.
const SCHEDULER_DELAY: Duration = Duration::from_secs(60);
const SCHEDULER_PERIOD: Duration = Duration::from_secs(15 * 60);

pub struct MangaService {
    app: AppHandle,
    catalog: Catalog,
    update: Mutex<MangaUpdateStatus>,
    /// Where imported comics are unpacked (under `comics/`).
    downloads_dir: PathBuf,
}

/// A local comic series' folder and title id: a hash of its name as duplicates are matched, so
/// later volumes join the same title and the path stays plain ASCII.
pub fn series_key(series: &str) -> String {
    let key = crate::library_actions::duplicate_key(series);
    raygun_ext_host::package::sha256_hex(key.as_bytes())[..16].to_owned()
}

impl MangaService {
    /// The shared catalog behind this library (categories, M39).
    pub fn catalog(&self) -> &Catalog {
        &self.catalog
    }

    pub fn new(app: AppHandle, images: ImageLinks, downloads_dir: PathBuf) -> Self {
        Self {
            catalog: Catalog::new(app.clone(), images, KIND, MANGA_SOURCE),
            app,
            update: Mutex::new(MangaUpdateStatus::default()),
            downloads_dir,
        }
    }

    pub async fn sources(&self) -> Vec<MangaSource> {
        self.catalog.sources().await
    }

    pub async fn browse(
        &self,
        source_id: &str,
        mode: MangaBrowseMode,
        query: Option<&str>,
        page: u32,
    ) -> Result<MangaListingPage, String> {
        self.catalog.browse(source_id, mode, query, page).await
    }

    /// A title with its chapters and their download states.
    pub async fn title(
        &self,
        source_id: &str,
        remote_id: &str,
        refresh: bool,
    ) -> Result<MangaTitleView, String> {
        let row = if source_id == LOCAL_COMICS {
            // Imported comics have nothing to refresh from.
            titles::find_title(&self.db(), source_id, remote_id)
                .await
                .map_err(to_message)?
                .ok_or_else(|| "This comic was removed".to_owned())?
        } else {
            self.catalog
                .title_row(source_id, remote_id, refresh)
                .await?
        };
        let downloads = self.downloads().for_title(&row.id).await;
        let mut view = self.catalog.title_view(row, &downloads).await?;
        if view.source_id == LOCAL_COMICS {
            view.source_name = "Local comics".into();
        }
        Ok(view)
    }

    /// Imports a comic archive (an upload's file, deleted afterwards) as a chapter of a local
    /// series, adding the series to the library. Returns the source's id and the series' id, as the
    /// title page wants them.
    pub async fn import_comic(
        &self,
        archive: PathBuf,
        file_name: &str,
    ) -> Result<(String, String), String> {
        let comics_dir = self.downloads_dir.join(COMICS);
        let name = file_name.to_owned();
        let (unpacked, _) = tauri::async_runtime::spawn_blocking(move || {
            let result = comics::unpack(&archive, &name, &comics_dir, series_key);
            let _ = std::fs::remove_file(&archive);
            result
        })
        .await
        .map_err(to_message)??;
        let key = series_key(&unpacked.series);
        let db = self.db();
        let now = now_ms();
        let existing = titles::find_title(&db, LOCAL_COMICS, &key)
            .await
            .map_err(to_message)?;
        let mut units: Vec<UnitFields> = match &existing {
            Some(row) => titles::units(&db, &row.id)
                .await
                .map_err(to_message)?
                .into_iter()
                .map(|unit| unit.fields)
                .filter(|fields| fields.remote_id != unpacked.hash)
                .collect(),
            None => Vec::new(),
        };
        units.push(UnitFields {
            remote_id: unpacked.hash.clone(),
            name: unpacked.name.clone(),
            number: unpacked.number,
            volume: None,
            scanlator: None,
            date: None,
        });
        // Newest first, as sources list them; unnumbered chapters last.
        units.sort_by(|a, b| match (a.number, b.number) {
            (Some(x), Some(y)) => y.partial_cmp(&x).unwrap_or(std::cmp::Ordering::Equal),
            (Some(_), None) => std::cmp::Ordering::Less,
            (None, Some(_)) => std::cmp::Ordering::Greater,
            (None, None) => comics::natural_cmp(&a.name, &b.name),
        });
        // The cover is the first page of the first chapter.
        let first = units.last().map(|unit| unit.remote_id.clone());
        let cover = match first {
            Some(chapter) => {
                let dir = self.downloads_dir.join(COMICS).join(&key).join(&chapter);
                comics::pages(&dir)
                    .ok()
                    .and_then(|pages| pages.into_iter().next())
                    .map(|page| format!("{LOCAL_PREFIX}{COMICS}/{key}/{chapter}/{page}"))
            }
            None => None,
        };
        let info = &unpacked.info;
        let keep = |new: &Option<String>, old: Option<&str>| {
            new.clone().or_else(|| old.map(str::to_owned))
        };
        let old = existing.as_ref().map(|row| &row.fields);
        let id = titles::upsert_title(
            &db,
            &TitleFields {
                kind: KIND.into(),
                source_id: LOCAL_COMICS.into(),
                remote_id: key.clone(),
                title: unpacked.series.clone(),
                cover,
                cover_headers: Default::default(),
                authors: keep(
                    &info.writer,
                    old.and_then(|old| old.authors.first().map(String::as_str)),
                )
                .into_iter()
                .collect(),
                artists: keep(
                    &info.penciller,
                    old.and_then(|old| old.artists.first().map(String::as_str)),
                )
                .into_iter()
                .collect(),
                description: keep(
                    &info.summary,
                    old.and_then(|old| old.description.as_deref()),
                ),
                genres: match &info.genre {
                    Some(genres) => genres
                        .split(',')
                        .map(str::trim)
                        .filter(|genre| !genre.is_empty())
                        .map(str::to_owned)
                        .collect(),
                    None => old.map(|old| old.genres.clone()).unwrap_or_default(),
                },
                status: "unknown".into(),
                url: None,
                media_type: None,
                year: None,
            },
            now,
        )
        .await
        .map_err(to_message)?;
        titles::sync_units(&db, &id, &units, "chapter", now)
            .await
            .map_err(to_message)?;
        let stored = titles::title(&db, &id).await.map_err(to_message)?;
        if stored.is_some_and(|title| title.in_library_at.is_none()) {
            titles::set_in_library(&db, &id, Some(now))
                .await
                .map_err(to_message)?;
        }
        tracing::info!(series = %unpacked.series, pages = unpacked.pages, "comic imported");
        Ok((LOCAL_COMICS.to_owned(), key))
    }

    /// An imported comic chapter's pages, from its unpacked folder.
    async fn local_comic_pages(&self, series: &str, chapter: &str) -> Result<Vec<String>, String> {
        let dir = self.downloads_dir.join(COMICS).join(series).join(chapter);
        let names = tauri::async_runtime::spawn_blocking(move || comics::pages(&dir))
            .await
            .map_err(to_message)?
            .map_err(|_| "This chapter is missing from the device".to_owned())?;
        Ok(names
            .iter()
            .map(|name| {
                self.catalog
                    .images
                    .local(&format!("{COMICS}/{series}/{chapter}/{name}"))
            })
            .collect())
    }

    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        self.catalog.set_in_library(title_id, in_library).await
    }

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        self.catalog.library().await
    }

    /// A stored title's chapters as its page lists them, with their read and download states,
    /// without asking the source: the reader's chapter list.
    pub async fn chapters(&self, title_id: &str) -> Result<Vec<MangaChapterInfo>, String> {
        let units = titles::units(&self.db(), title_id)
            .await
            .map_err(to_message)?;
        let ids: Vec<String> = units.iter().map(|unit| unit.id.clone()).collect();
        let downloads = self.downloads().for_chapters(&ids).await;
        Ok(units
            .iter()
            .map(|unit| chapter_info(unit, &downloads))
            .collect())
    }

    /// One chapter's pages, with its neighbours in reading order.
    pub async fn chapter(
        &self,
        title_id: &str,
        unit_id: &str,
    ) -> Result<MangaChapterPages, String> {
        let prefs = match crate::settings::load(&self.db()).await {
            Ok(settings) => settings.manga_reader,
            Err(error) => {
                tracing::warn!(%error, "settings unreadable; reading with the defaults");
                raygun_model::MangaReaderPrefs::default()
            }
        };
        let context = self
            .catalog
            .chapter_context(title_id, unit_id, prefs.skip_duplicates)
            .await?;
        let downloads = self.downloads();
        // A downloaded chapter reads from disk, without asking the source.
        let pages = match downloads.local_pages(title_id, unit_id).await {
            Some(pages) => pages,
            None if context.title.fields.source_id == LOCAL_COMICS => {
                self.local_comic_pages(
                    &context.title.fields.remote_id,
                    &context.unit.fields.remote_id,
                )
                .await?
            }
            None => {
                let extensions = self.catalog.extensions();
                let (_ready, host) = extensions.ready().await;
                host.manga_pages(
                    &context.title.fields.source_id,
                    &context.unit.fields.remote_id,
                )
                .await?
                .iter()
                .map(|page| self.catalog.images.link(&page.url, &page.headers))
                .collect()
            }
        };
        let download = downloads
            .for_chapters(std::slice::from_ref(&context.unit.id))
            .await;
        let own_reader_mode = ReaderMode::chosen(&context.title.reader_mode);
        Ok(MangaChapterPages {
            reader_mode: own_reader_mode.unwrap_or(prefs.default_mode),
            own_reader_mode,
            chapter: chapter_info(&context.unit, &download),
            title_id: context.title.id,
            title: context.title.fields.title,
            pages,
            previous_chapter_id: context.previous_id,
            next_chapter_id: context.next_id,
        })
    }

    /// A title's own reading mode, or `None` to follow the reader's default.
    pub async fn set_reader_mode(
        &self,
        title_id: &str,
        mode: Option<ReaderMode>,
    ) -> Result<(), String> {
        titles::set_reader_mode(&self.db(), title_id, mode.map_or("", ReaderMode::as_str))
            .await
            .map_err(to_message)
    }

    pub async fn set_progress(&self, unit_id: &str, page: u32, total: u32) -> Result<(), String> {
        self.catalog.set_progress(unit_id, page, total).await
    }

    pub async fn set_read(&self, unit_id: &str, read: bool) -> Result<(), String> {
        self.catalog.set_read(unit_id, read).await
    }

    /// Chapters library updates found, comics' and books', newest first.
    pub async fn updates(&self) -> Result<Vec<MangaUpdateItem>, String> {
        let db = self.db();
        let mut rows = titles::updates(&db, KIND, FEED_LIMIT)
            .await
            .map_err(to_message)?;
        rows.extend(
            titles::updates(&db, NOVEL_KIND, FEED_LIMIT)
                .await
                .map_err(to_message)?,
        );
        // Stable, so each title's chapters keep the order the source gave them.
        rows.sort_by_key(|row| std::cmp::Reverse(row.found_at));
        rows.truncate(FEED_LIMIT as usize);
        let ids: Vec<String> = rows.iter().map(|row| row.unit.id.clone()).collect();
        let downloads = self.downloads().for_chapters(&ids).await;
        Ok(rows
            .into_iter()
            .map(|row| MangaUpdateItem {
                medium: row.title.fields.kind.clone(),
                chapter: chapter_info(&row.unit, &downloads),
                cover_url: row.title.fields.cover.as_deref().map(|url| {
                    self.catalog
                        .kept_cover(url, &row.title.fields.cover_headers)
                }),
                title_id: row.title.id,
                source_id: row.title.fields.source_id,
                remote_id: row.title.fields.remote_id,
                title: row.title.fields.title,
                found_at: row.found_at as f64,
                authors: row.title.fields.authors,
            })
            .collect())
    }

    /// Where the library update stands (with the last finished run, even from a past session).
    pub async fn update_status(&self) -> MangaUpdateStatus {
        let mut status = self.update.lock().expect("update status poisoned").clone();
        if status.last_run_at.is_none() {
            status.last_run_at = self.last_update().await.map(|at| at as f64);
        }
        status
    }

    /// Starts a library update in the background unless one is running. A manual update also
    /// checks finished and cancelled series, which scheduled ones skip.
    pub async fn start_update(&self, manual: bool) -> MangaUpdateStatus {
        self.start_update_of(manual, None).await
    }

    /// Like `start_update`, for some titles only (one category, M46).
    pub async fn start_update_of(
        &self,
        manual: bool,
        only: Option<HashSet<String>>,
    ) -> MangaUpdateStatus {
        {
            let mut status = self.update.lock().expect("update status poisoned");
            if status.running {
                return status.clone();
            }
            status.running = true;
        }
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let manga = app.state::<MangaService>();
            if let Err(message) = manga.run_update(manual, only).await {
                tracing::warn!(%message, "library update failed");
            }
            let finished = {
                let mut status = manga.update.lock().expect("update status poisoned");
                status.running = false;
                status.clone()
            };
            manga.emit_update(finished);
        });
        self.update_status().await
    }

    async fn run_update(&self, manual: bool, only: Option<HashSet<String>>) -> Result<(), String> {
        raygun_net::network_mode::check_online()?;
        let started = std::time::Instant::now();
        let db = self.db();
        let mut library = titles::library(&db, KIND).await.map_err(to_message)?;
        // Books from sources, and followed series and saved movies (M43).
        for kind in [NOVEL_KIND, video_library::KIND] {
            library.extend(titles::library(&db, kind).await.map_err(to_message)?);
        }
        let extensions = self.catalog.extensions();
        let due: Vec<TitleRow> = library
            .into_iter()
            .map(|entry| entry.title)
            .filter(|title| manual || wants_update(&title.fields.status))
            // Imported titles have no source to ask.
            .filter(|title| !title.fields.source_id.starts_with(LOCAL_SOURCE_PREFIX))
            // Nor do sources waiting for their key or sign-in, until they're set up.
            .filter(|title| !extensions.needs_setup(&title.fields.source_id))
            .filter(|title| only.as_ref().is_none_or(|only| only.contains(&title.id)))
            .collect();
        let snapshot = {
            let mut status = self.update.lock().expect("update status poisoned");
            *status = MangaUpdateStatus {
                running: true,
                total: due.len() as u32,
                last_run_at: status.last_run_at,
                ..MangaUpdateStatus::default()
            };
            status.clone()
        };
        self.emit_update(snapshot);
        // Which source each failure was at, as people know it.
        let names = if due.is_empty() {
            HashMap::new()
        } else {
            self.source_names().await
        };

        let mut results = stream::iter(due)
            .map(|title| async move {
                let result = self.refresh_title(&title).await;
                if let Err(message) = &result {
                    tracing::warn!(title = %title.fields.title, %message, "title update failed");
                }
                (title, result)
            })
            .buffer_unordered(UPDATE_CONCURRENCY);
        // One notification per title with something new.
        let mut news = Vec::new();
        while let Some((title, result)) = results.next().await {
            if let (Ok(added @ 1..), Some(kind)) = (&result, NewsKind::of_title(&title.fields.kind))
            {
                let fields = &title.fields;
                news.push((
                    kind,
                    news::library_notice(
                        kind,
                        &title.id,
                        &fields.title,
                        *added,
                        news::title_route(kind, &fields.source_id, &fields.remote_id),
                    ),
                ));
            }
            let snapshot = {
                let mut status = self.update.lock().expect("update status poisoned");
                status.checked += 1;
                match result {
                    Ok(added) => status.new_chapters += added as u32,
                    Err(message) => {
                        status.failed += 1;
                        status.failures.push(UpdateFailure {
                            medium: title.fields.kind.clone(),
                            source_name: names
                                .get(&title.fields.source_id)
                                .cloned()
                                .unwrap_or_else(|| title.fields.source_id.clone()),
                            source_id: title.fields.source_id,
                            remote_id: title.fields.remote_id,
                            title: title.fields.title,
                            message,
                        });
                    }
                }
                status.clone()
            };
            self.emit_update(snapshot);
        }

        if !news.is_empty() {
            let prefs = crate::settings::load(&db)
                .await
                .map(|settings| settings.notifications)
                .unwrap_or_default();
            news::announce(&self.app, &prefs, news).await;
        }

        let now = now_ms();
        raygun_db::set_local_value(&db, LAST_UPDATE_KEY, &now.to_string())
            .await
            .map_err(to_message)?;
        let status = {
            let mut status = self.update.lock().expect("update status poisoned");
            status.last_run_at = Some(now as f64);
            status.clone()
        };
        tracing::info!(
            titles = status.total,
            new_chapters = status.new_chapters,
            failed = status.failed,
            elapsed_ms = started.elapsed().as_millis() as u64,
            "library update finished"
        );
        Ok(())
    }

    /// Sources' names across the libraries an update covers.
    async fn source_names(&self) -> HashMap<String, String> {
        let mut names = self.catalog.source_names().await;
        names.extend(
            self.app
                .state::<NovelService>()
                .catalog()
                .source_names()
                .await,
        );
        names.extend(
            self.app
                .state::<VideoSourceService>()
                .catalog()
                .source_names()
                .await,
        );
        names
    }

    /// Fetches a library title again, through its own catalogue (books through novel sources,
    /// videos through video sources or Stremio); returns how many chapters or episodes were new.
    async fn refresh_title(&self, title: &TitleRow) -> Result<usize, String> {
        let (source_id, remote_id) = (&title.fields.source_id, &title.fields.remote_id);
        if title.fields.kind == NOVEL_KIND {
            let novels = self.app.state::<NovelService>();
            return Ok(novels.catalog().fetch_title(source_id, remote_id).await?.1);
        }
        if title.fields.kind != video_library::KIND {
            return Ok(self.catalog.fetch_title(source_id, remote_id).await?.1);
        }
        if source_id == video_library::STREMIO_SOURCE {
            return self.app.state::<Arc<StremioService>>().refresh(title).await;
        }
        let sources = self.app.state::<VideoSourceService>();
        Ok(sources.catalog().fetch_title(source_id, remote_id).await?.1)
    }

    /// Runs scheduled library updates while the app is open, per the `mangaUpdateHours` setting.
    pub fn start_scheduler(&self) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SCHEDULER_DELAY).await;
            loop {
                // Library updates wait for Offline Mode to end.
                raygun_net::network_mode::wait_online().await;
                let manga = app.state::<MangaService>();
                let hours = match crate::settings::load(&manga.db()).await {
                    Ok(settings) => settings.manga_update_hours,
                    Err(error) => {
                        tracing::warn!(%error, "settings unreadable; skipping the update check");
                        0
                    }
                };
                if update_due(hours, manga.last_update().await, now_ms()) {
                    manga.start_update(false).await;
                }
                tokio::time::sleep(SCHEDULER_PERIOD).await;
            }
        });
    }

    async fn last_update(&self) -> Option<i64> {
        raygun_db::local_value(&self.db(), LAST_UPDATE_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|value| value.parse().ok())
    }

    fn emit_update(&self, status: MangaUpdateStatus) {
        if let Err(error) = MangaUpdateEvent(status).emit(&self.app) {
            tracing::debug!(%error, "failed to emit update event");
        }
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.catalog.db()
    }

    fn downloads(&self) -> tauri::State<'_, Arc<MangaDownloadService>> {
        self.app.state::<Arc<MangaDownloadService>>()
    }
}

/// Scheduled updates skip series that won't get new chapters.
fn wants_update(status: &str) -> bool {
    !matches!(status, "completed" | "cancelled")
}

/// Whether a scheduled update is due: on, and never run or run at least `hours` ago.
fn update_due(hours: u32, last_run_ms: Option<i64>, now_ms: i64) -> bool {
    hours > 0 && last_run_ms.is_none_or(|last| now_ms - last >= i64::from(hours) * 3_600_000)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scheduled_updates_skip_finished_series_and_wait_their_interval() {
        assert!(wants_update("ongoing"));
        assert!(wants_update("hiatus"));
        assert!(wants_update("unknown"));
        assert!(!wants_update("completed"));
        assert!(!wants_update("cancelled"));

        let hour = 3_600_000;
        assert!(!update_due(0, None, 10 * hour), "off");
        assert!(update_due(12, None, 10 * hour), "never ran");
        assert!(!update_due(12, Some(hour), 12 * hour));
        assert!(update_due(12, Some(hour), 13 * hour));
    }
}
