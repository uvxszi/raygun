//! Manga (M24): browsing `manga.source` extensions, titles with their chapters, the manga library
//! and reading progress. Covers and pages are linked through the stream proxy's image route.
//! Library updates and the updates feed arrived in M28.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::{StreamExt, stream};
use raygun_db::titles::{self, TitleFields, TitleRow, UnitFields, UnitRow};
use raygun_ext_host::manga::{MANGA_SOURCE, MangaList};
use raygun_model::{
    ChapterDownload, MangaBrowseMode, MangaCard, MangaChapterInfo, MangaChapterPages,
    MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView, MangaUpdateItem,
    MangaUpdateStatus, ReaderMode,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::MangaUpdateEvent;
use crate::extensions::ExtensionService;
use crate::manga_downloads::MangaDownloadService;
use crate::startup::AppDb;

const KIND: &str = "manga";
const UNIT_KIND: &str = "chapter";
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
    images: ImageLinks,
    update: Mutex<MangaUpdateStatus>,
}

impl MangaService {
    pub fn new(app: AppHandle, images: ImageLinks) -> Self {
        Self {
            app,
            images,
            update: Mutex::new(MangaUpdateStatus::default()),
        }
    }

    pub async fn sources(&self) -> Vec<MangaSource> {
        let extensions = self.extensions();
        let (_ready, host) = extensions.ready().await;
        host.providers(MANGA_SOURCE)
            .await
            .into_iter()
            .map(|provider| MangaSource {
                id: provider.id,
                name: provider.name,
                extension_id: provider.extension_id,
            })
            .collect()
    }

    pub async fn browse(
        &self,
        source_id: &str,
        mode: MangaBrowseMode,
        query: Option<&str>,
        page: u32,
    ) -> Result<MangaListingPage, String> {
        let page = page.max(1);
        let listing = {
            let extensions = self.extensions();
            let (_ready, host) = extensions.ready().await;
            match mode {
                MangaBrowseMode::Popular => {
                    host.manga_list(source_id, MangaList::Popular, page).await
                }
                MangaBrowseMode::Latest => {
                    host.manga_list(source_id, MangaList::Latest, page).await
                }
                MangaBrowseMode::Search => {
                    let query = query.map(str::trim).filter(|query| !query.is_empty());
                    let query = query.ok_or_else(|| "Type something to search for".to_owned())?;
                    host.manga_search(source_id, query, page).await
                }
            }
        }?;
        let ids: Vec<String> = listing.items.iter().map(|item| item.id.clone()).collect();
        let in_library = titles::in_library(&self.db(), source_id, &ids)
            .await
            .map_err(to_message)?;
        Ok(MangaListingPage {
            items: listing
                .items
                .into_iter()
                .map(|item| MangaCard {
                    source_id: source_id.to_owned(),
                    in_library: in_library.contains(&item.id),
                    cover_url: item
                        .cover_url
                        .as_deref()
                        .map(|url| self.images.link(url, &item.cover_headers)),
                    id: item.id,
                    title: item.title,
                })
                .collect(),
            has_next_page: listing.has_next_page,
        })
    }

    /// A title with its chapters. Stored data is used unless `refresh` is set or the title has
    /// never been fetched; fetching stores it, so progress has chapters to attach to.
    pub async fn title(
        &self,
        source_id: &str,
        remote_id: &str,
        refresh: bool,
    ) -> Result<MangaTitleView, String> {
        let db = self.db();
        let stored = titles::find_title(&db, source_id, remote_id)
            .await
            .map_err(to_message)?;
        let row = match stored {
            Some(row) if !refresh && row.refreshed_at.is_some() => row,
            _ => self.fetch_title(source_id, remote_id).await?.0,
        };
        self.title_view(row).await
    }

    /// Fetches and stores a title with its chapters; returns it and how many chapters were new.
    async fn fetch_title(
        &self,
        source_id: &str,
        remote_id: &str,
    ) -> Result<(TitleRow, usize), String> {
        let (details, chapters) = {
            let extensions = self.extensions();
            let (_ready, host) = extensions.ready().await;
            futures_util::future::join(
                host.manga_details(source_id, remote_id),
                host.manga_chapters(source_id, remote_id),
            )
            .await
        };
        let details = details?;
        let chapters = chapters?;
        let db = self.db();
        let id = titles::upsert_title(
            &db,
            &TitleFields {
                kind: KIND.into(),
                source_id: source_id.into(),
                remote_id: remote_id.into(),
                title: details.title,
                cover: details.cover_url,
                cover_headers: details.cover_headers,
                authors: details.authors,
                artists: details.artists,
                description: details.description,
                genres: details.genres,
                status: details.status,
                url: details.url,
            },
            now_ms(),
        )
        .await
        .map_err(to_message)?;
        let units: Vec<UnitFields> = chapters
            .into_iter()
            .map(|chapter| UnitFields {
                remote_id: chapter.id,
                name: chapter.name,
                number: chapter.number,
                volume: chapter.volume,
                scanlator: chapter.scanlator,
                date: chapter.date_upload,
            })
            .collect();
        let added = titles::sync_units(&db, &id, &units, UNIT_KIND, now_ms())
            .await
            .map_err(to_message)?;
        tracing::info!(
            source = source_id,
            title = remote_id,
            chapters = units.len(),
            added,
            "manga title refreshed"
        );
        let row = titles::title(&db, &id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "The title went missing".to_owned())?;
        Ok((row, added))
    }

    async fn title_view(&self, row: TitleRow) -> Result<MangaTitleView, String> {
        let units = titles::units(&self.db(), &row.id)
            .await
            .map_err(to_message)?;
        let source_name = self
            .sources()
            .await
            .into_iter()
            .find(|source| source.id == row.fields.source_id)
            .map_or_else(|| row.fields.source_id.clone(), |source| source.name);
        let continue_chapter_id = continue_from(&units).map(|unit| unit.id.clone());
        let downloads = self.downloads().for_title(&row.id).await;
        Ok(MangaTitleView {
            id: row.id,
            source_id: row.fields.source_id,
            source_name,
            remote_id: row.fields.remote_id,
            title: row.fields.title,
            cover_url: row
                .fields
                .cover
                .as_deref()
                .map(|url| self.images.link(url, &row.fields.cover_headers)),
            authors: row.fields.authors,
            artists: row.fields.artists,
            description: row.fields.description,
            genres: row.fields.genres,
            status: row.fields.status,
            url: row.fields.url,
            in_library: row.in_library_at.is_some(),
            chapters: units
                .iter()
                .map(|unit| chapter_info(unit, &downloads))
                .collect(),
            continue_chapter_id,
        })
    }

    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        titles::set_in_library(&self.db(), title_id, in_library.then(now_ms))
            .await
            .map_err(to_message)
    }

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        let entries = titles::library(&self.db(), KIND)
            .await
            .map_err(to_message)?;
        Ok(entries
            .into_iter()
            .map(|entry| MangaLibraryEntry {
                cover_url: entry
                    .title
                    .fields
                    .cover
                    .as_deref()
                    .map(|url| self.images.link(url, &entry.title.fields.cover_headers)),
                title_id: entry.title.id,
                source_id: entry.title.fields.source_id,
                remote_id: entry.title.fields.remote_id,
                title: entry.title.fields.title,
                chapters: entry.units as u32,
                unread: entry.unread as u32,
            })
            .collect())
    }

    /// One chapter's pages, with its neighbours in reading order.
    pub async fn chapter(
        &self,
        title_id: &str,
        unit_id: &str,
    ) -> Result<MangaChapterPages, String> {
        let db = self.db();
        let title = titles::title(&db, title_id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "This title isn't stored".to_owned())?;
        let units = titles::units(&db, title_id).await.map_err(to_message)?;
        let order = reading_order(&units);
        let index = order
            .iter()
            .position(|unit| unit.id == unit_id)
            .ok_or_else(|| "This chapter isn't part of the title".to_owned())?;
        let unit = order[index];
        let downloads = self.downloads();
        // A downloaded chapter reads from disk, without asking the source.
        let pages = match downloads.local_pages(title_id, unit_id).await {
            Some(pages) => pages,
            None => {
                let extensions = self.extensions();
                let (_ready, host) = extensions.ready().await;
                host.manga_pages(&title.fields.source_id, &unit.fields.remote_id)
                    .await?
                    .iter()
                    .map(|page| self.images.link(&page.url, &page.headers))
                    .collect()
            }
        };
        let download = downloads.for_chapters(std::slice::from_ref(&unit.id)).await;
        Ok(MangaChapterPages {
            reader_mode: ReaderMode::parse(&title.reader_mode),
            title_id: title.id,
            title: title.fields.title,
            chapter: chapter_info(unit, &download),
            pages,
            previous_chapter_id: index
                .checked_sub(1)
                .map(|previous| order[previous].id.clone()),
            next_chapter_id: order.get(index + 1).map(|next| next.id.clone()),
        })
    }

    pub async fn set_reader_mode(&self, title_id: &str, mode: ReaderMode) -> Result<(), String> {
        titles::set_reader_mode(&self.db(), title_id, mode.as_str())
            .await
            .map_err(to_message)
    }

    pub async fn set_progress(&self, unit_id: &str, page: u32, total: u32) -> Result<(), String> {
        titles::set_progress(
            &self.db(),
            unit_id,
            page.into(),
            Some(total.into()),
            now_ms(),
        )
        .await
        .map_err(to_message)
    }

    pub async fn set_read(&self, unit_id: &str, read: bool) -> Result<(), String> {
        titles::set_completed(&self.db(), unit_id, read, now_ms())
            .await
            .map_err(to_message)
    }

    /// Chapters library updates found, newest first.
    pub async fn updates(&self) -> Result<Vec<MangaUpdateItem>, String> {
        let rows = titles::updates(&self.db(), KIND, FEED_LIMIT)
            .await
            .map_err(to_message)?;
        let ids: Vec<String> = rows.iter().map(|row| row.unit.id.clone()).collect();
        let downloads = self.downloads().for_chapters(&ids).await;
        Ok(rows
            .into_iter()
            .map(|row| MangaUpdateItem {
                chapter: chapter_info(&row.unit, &downloads),
                cover_url: row
                    .title
                    .fields
                    .cover
                    .as_deref()
                    .map(|url| self.images.link(url, &row.title.fields.cover_headers)),
                title_id: row.title.id,
                source_id: row.title.fields.source_id,
                remote_id: row.title.fields.remote_id,
                title: row.title.fields.title,
                found_at: row.found_at as f64,
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
            if let Err(message) = manga.run_update(manual).await {
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

    async fn run_update(&self, manual: bool) -> Result<(), String> {
        let started = std::time::Instant::now();
        let db = self.db();
        let library = titles::library(&db, KIND).await.map_err(to_message)?;
        let due: Vec<TitleRow> = library
            .into_iter()
            .map(|entry| entry.title)
            .filter(|title| manual || wants_update(&title.fields.status))
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

        let mut results = stream::iter(due)
            .map(|title| async move {
                let result = self
                    .fetch_title(&title.fields.source_id, &title.fields.remote_id)
                    .await;
                if let Err(message) = &result {
                    tracing::warn!(title = %title.fields.title, %message, "title update failed");
                }
                result.map(|(_, added)| added)
            })
            .buffer_unordered(UPDATE_CONCURRENCY);
        while let Some(result) = results.next().await {
            let snapshot = {
                let mut status = self.update.lock().expect("update status poisoned");
                status.checked += 1;
                match result {
                    Ok(added) => status.new_chapters += added as u32,
                    Err(_) => status.failed += 1,
                }
                status.clone()
            };
            self.emit_update(snapshot);
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

    /// Runs scheduled library updates while the app is open, per the `mangaUpdateHours` setting.
    pub fn start_scheduler(&self) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(SCHEDULER_DELAY).await;
            loop {
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
        self.app.state::<AppDb>().inner().0.clone()
    }

    fn extensions(&self) -> tauri::State<'_, Arc<ExtensionService>> {
        self.app.state::<Arc<ExtensionService>>()
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

/// Units in reading order: by chapter number when every unit has one, else the source's order
/// reversed (sources list newest first).
fn reading_order(units: &[UnitRow]) -> Vec<&UnitRow> {
    let mut order: Vec<&UnitRow> = units.iter().collect();
    if units.iter().all(|unit| unit.fields.number.is_some()) {
        order.sort_by(|a, b| {
            a.fields
                .number
                .partial_cmp(&b.fields.number)
                .unwrap_or(std::cmp::Ordering::Equal)
                .then(b.source_order.cmp(&a.source_order))
        });
    } else {
        order.reverse();
    }
    order
}

/// Where reading continues: a partly read chapter, else the first unread one after the last read
/// chapter, else the first chapter.
fn continue_from(units: &[UnitRow]) -> Option<&UnitRow> {
    let order = reading_order(units);
    if let Some(partly) = order
        .iter()
        .find(|unit| unit.completed_at.is_none() && unit.position.unwrap_or(0) > 0)
    {
        return Some(partly);
    }
    let after_last_read = order
        .iter()
        .rposition(|unit| unit.completed_at.is_some())
        .map_or(0, |index| index + 1);
    order
        .get(after_last_read)
        .or_else(|| order.first())
        .copied()
}

fn chapter_info(unit: &UnitRow, downloads: &HashMap<String, ChapterDownload>) -> MangaChapterInfo {
    let read = unit.completed_at.is_some();
    MangaChapterInfo {
        id: unit.id.clone(),
        name: unit.fields.name.clone(),
        number: unit.fields.number,
        volume: unit.fields.volume.clone(),
        scanlator: unit.fields.scanlator.clone(),
        date_upload: unit.fields.date.map(|date| date as f64),
        read,
        last_page: unit
            .position
            .filter(|position| !read && *position > 0)
            .map(|position| position as u32),
        total_pages: unit.total.map(|total| total as u32),
        download: downloads.get(&unit.id).cloned(),
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

#[cfg(test)]
mod tests {
    use super::*;

    fn unit(
        id: &str,
        number: Option<f64>,
        order: i64,
        position: Option<i64>,
        done: bool,
    ) -> UnitRow {
        UnitRow {
            id: id.into(),
            title_id: "t".into(),
            fields: UnitFields {
                remote_id: id.into(),
                name: id.into(),
                number,
                volume: None,
                scanlator: None,
                date: None,
            },
            source_order: order,
            position,
            total: Some(20),
            completed_at: done.then_some(1),
        }
    }

    fn ids(units: Vec<&UnitRow>) -> Vec<&str> {
        units.into_iter().map(|unit| unit.id.as_str()).collect()
    }

    #[test]
    fn reading_order_follows_numbers_or_reverses_the_source() {
        let numbered = [
            unit("c3", Some(3.0), 0, None, false),
            unit("c1", Some(1.0), 2, None, false),
            unit("c2", Some(2.0), 1, None, false),
        ];
        assert_eq!(ids(reading_order(&numbered)), ["c1", "c2", "c3"]);
        let unnumbered = [
            unit("new", None, 0, None, false),
            unit("old", Some(1.0), 1, None, false),
        ];
        assert_eq!(ids(reading_order(&unnumbered)), ["old", "new"]);
    }

    #[test]
    fn continue_picks_the_partly_read_or_next_unread_chapter() {
        let fresh = [
            unit("c2", Some(2.0), 0, None, false),
            unit("c1", Some(1.0), 1, None, false),
        ];
        assert_eq!(
            continue_from(&fresh).unwrap().id,
            "c1",
            "start at the beginning"
        );

        let read_one = [
            unit("c2", Some(2.0), 0, None, false),
            unit("c1", Some(1.0), 1, None, true),
        ];
        assert_eq!(continue_from(&read_one).unwrap().id, "c2");

        let partly = [
            unit("c3", Some(3.0), 0, Some(5), false),
            unit("c2", Some(2.0), 1, None, false),
            unit("c1", Some(1.0), 2, None, true),
        ];
        assert_eq!(continue_from(&partly).unwrap().id, "c3");

        let all_read = [unit("c1", Some(1.0), 0, None, true)];
        assert_eq!(continue_from(&all_read).unwrap().id, "c1");
        assert!(continue_from(&[]).is_none());
    }

    #[test]
    fn chapter_info_shows_partial_progress_only() {
        let none = HashMap::new();
        let partly = chapter_info(&unit("c", Some(1.0), 0, Some(4), false), &none);
        assert_eq!((partly.read, partly.last_page), (false, Some(4)));
        let done = chapter_info(&unit("c", Some(1.0), 0, Some(19), true), &none);
        assert_eq!((done.read, done.last_page), (true, None));
        assert_eq!(done.download, None);
    }

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
