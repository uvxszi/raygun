//! What manga and novels share (M31): browsing a source capability's catalogue, titles with
//! their chapters, the library of one kind, read state and progress, and reading order. Each
//! medium's service wraps a `Catalog` and adds what reading it takes (pages or text).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use raygun_db::titles::{self, TitleFields, TitleRow, UnitFields, UnitRow};
use raygun_ext_host::manga::MangaList;
use raygun_ext_host::video::VIDEO_SOURCE;
use raygun_model::{
    ChapterDownload, FilterValue, LibraryCategory, LibraryList, MangaBrowseMode, MangaCard,
    MangaChapterInfo, MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView,
    SourceFilter, SourceSearchResult,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// Sources searched at the same time (M46.7): each runs a script and makes requests, so a search
/// over many sources goes a few at a time instead of all at once.
const SEARCH_CONCURRENCY: usize = 4;

pub struct Catalog {
    app: AppHandle,
    pub images: ImageLinks,
    /// `title.kind`: `manga`, `novel` or `video`.
    kind: &'static str,
    /// The provider capability: `manga.source`, `novel.source` or `video.source`.
    capability: &'static str,
}

/// A chapter with its neighbours in reading order.
pub struct ChapterContext {
    pub title: TitleRow,
    pub unit: UnitRow,
    pub previous_id: Option<String>,
    pub next_id: Option<String>,
}

/// Prefix of stored covers that are files under the downloads folder.
pub const LOCAL_PREFIX: &str = "local:";

/// Sources of titles imported from the device (EPUB books, comics) start with this.
pub const LOCAL_SOURCE_PREFIX: &str = "local.";

impl Catalog {
    pub fn new(
        app: AppHandle,
        images: ImageLinks,
        kind: &'static str,
        capability: &'static str,
    ) -> Self {
        Self {
            app,
            images,
            kind,
            capability,
        }
    }

    pub async fn sources(&self) -> Vec<MangaSource> {
        let extensions = self.extensions();
        let (_ready, host) = extensions.ready().await;
        host.providers(self.capability)
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
        self.listing_page(source_id, listing).await
    }

    /// A source's listing as cards: adult items dropped while NSFW is off, library titles marked.
    async fn listing_page(
        &self,
        source_id: &str,
        mut listing: raygun_ext_host::manga::MangaListing,
    ) -> Result<MangaListingPage, String> {
        let show_nsfw = self.extensions().show_nsfw();
        listing.items.retain(|item| show_nsfw || !item.nsfw);
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
                        .map(|url| self.cover(url, &item.cover_headers)),
                    id: item.id,
                    title: item.title,
                })
                .collect(),
            has_next_page: listing.has_next_page,
        })
    }

    // ---- Search with filters, and across sources (M40) ----

    /// The filters a source offers for its search.
    pub async fn filters(&self, source_id: &str) -> Result<Vec<SourceFilter>, String> {
        let extensions = self.extensions();
        let (_ready, host) = extensions.ready().await;
        host.manga_filters(source_id).await
    }

    /// A source search with its filters; the query may be empty when filters are set.
    pub async fn search(
        &self,
        source_id: &str,
        query: &str,
        page: u32,
        filters: &BTreeMap<String, FilterValue>,
    ) -> Result<MangaListingPage, String> {
        let query = query.trim();
        // Nothing asked for yet: the source's popular titles, so Search never starts empty.
        if query.is_empty() && filters.is_empty() {
            return self
                .browse(source_id, MangaBrowseMode::Popular, None, page)
                .await;
        }
        let listing = {
            let extensions = self.extensions();
            let (_ready, host) = extensions.ready().await;
            host.manga_search_filtered(source_id, query, page.max(1), filters)
                .await
        }?;
        self.listing_page(source_id, listing).await
    }

    /// The first page of every source's results for `query`, a few sources at a time (Mihon's
    /// global search; never all at once, M46.7), or only of the ones in `only` (Search's source
    /// chips, M46.4) when it isn't empty. A blank query gives each source's popular titles. A
    /// source that fails reports its error and the others still show.
    pub async fn global_search(
        &self,
        query: &str,
        only: &[String],
    ) -> Result<Vec<SourceSearchResult>, String> {
        let query = query.trim();
        let mut sources = self.sources().await;
        if !only.is_empty() {
            sources.retain(|source| only.contains(&source.id));
        }
        let searches = sources.into_iter().map(|source| async move {
            let result = {
                let extensions = self.extensions();
                let (_ready, host) = extensions.ready().await;
                if query.is_empty() {
                    host.manga_list(&source.id, MangaList::Popular, 1).await
                } else {
                    host.manga_search(&source.id, query, 1).await
                }
            };
            let page = match result {
                Ok(listing) => self.listing_page(&source.id, listing).await,
                Err(error) => Err(error),
            };
            match page {
                Ok(page) => SourceSearchResult {
                    source,
                    items: page.items,
                    has_next_page: page.has_next_page,
                    error: None,
                },
                Err(error) => SourceSearchResult {
                    source,
                    items: Vec::new(),
                    has_next_page: false,
                    error: Some(error),
                },
            }
        });
        let mut results = crate::stremio::at_most(SEARCH_CONCURRENCY, searches).await;
        // Sources with results first, then empty ones, then failures.
        results.sort_by_key(|result| (result.error.is_some(), result.items.is_empty()));
        Ok(results)
    }

    /// A stored title, fetched first when `refresh` is set or it never was; fetching stores it,
    /// so progress has chapters to attach to.
    pub async fn title_row(
        &self,
        source_id: &str,
        remote_id: &str,
        refresh: bool,
    ) -> Result<TitleRow, String> {
        let stored = titles::find_title(&self.db(), source_id, remote_id)
            .await
            .map_err(to_message)?;
        match stored {
            Some(row) if !refresh && row.refreshed_at.is_some() => Ok(row),
            stored => {
                // A manual refresh fetches the cover again too, as Mihon's does.
                if refresh
                    && let Some(row) = &stored
                    && let Some(cover) = &row.fields.cover
                {
                    self.images
                        .forget_cover(cover, &row.fields.cover_headers)
                        .await;
                }
                Ok(self.fetch_title(source_id, remote_id).await?.0)
            }
        }
    }

    /// Fetches and stores a title with its chapters; returns it and how many chapters were new.
    pub async fn fetch_title(
        &self,
        source_id: &str,
        remote_id: &str,
    ) -> Result<(TitleRow, usize), String> {
        let video = self.capability == VIDEO_SOURCE;
        let (details, chapters) = {
            let extensions = self.extensions();
            let (_ready, host) = extensions.ready().await;
            let units = async {
                if video {
                    host.video_episodes(source_id, remote_id).await
                } else {
                    host.manga_chapters(source_id, remote_id).await
                }
            };
            futures_util::future::join(host.manga_details(source_id, remote_id), units).await
        };
        let details = details?;
        let chapters = chapters?;
        let db = self.db();
        let id = titles::upsert_title(
            &db,
            &TitleFields {
                kind: self.kind.into(),
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
                media_type: details.media_type,
                year: details.year,
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
        let unit_kind = if video { "episode" } else { "chapter" };
        let added = titles::sync_units(&db, &id, &units, unit_kind, now_ms())
            .await
            .map_err(to_message)?;
        tracing::info!(
            kind = self.kind,
            source = source_id,
            title = remote_id,
            chapters = units.len(),
            added,
            "title refreshed"
        );
        let row = titles::title(&db, &id)
            .await
            .map_err(to_message)?
            .ok_or_else(|| "The title went missing".to_owned())?;
        Ok((row, added))
    }

    /// The title page's view, with the download state of chapters that have one.
    pub async fn title_view(
        &self,
        row: TitleRow,
        downloads: &HashMap<String, ChapterDownload>,
    ) -> Result<MangaTitleView, String> {
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
        let hidden_nsfw = !self.extensions().show_nsfw() && is_adult(&row.fields.genres);
        Ok(MangaTitleView {
            hidden_nsfw,
            id: row.id,
            source_id: row.fields.source_id,
            source_name,
            remote_id: row.fields.remote_id,
            title: row.fields.title,
            cover_url: row.fields.cover.as_deref().map(|url| {
                if row.in_library_at.is_some() {
                    self.kept_cover(url, &row.fields.cover_headers)
                } else {
                    self.cover(url, &row.fields.cover_headers)
                }
            }),
            authors: row.fields.authors,
            artists: row.fields.artists,
            description: row.fields.description,
            genres: row.fields.genres,
            status: row.fields.status,
            url: row.fields.url,
            in_library: row.in_library_at.is_some(),
            chapters: units
                .iter()
                .map(|unit| chapter_info(unit, downloads))
                .collect(),
            continue_chapter_id,
        })
    }

    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        titles::set_in_library(&self.db(), title_id, in_library.then(now_ms))
            .await
            .map_err(to_message)
    }

    /// A source's name for people: its provider's, or what the app's own sources are called.
    pub async fn source_names(&self) -> HashMap<String, String> {
        let mut names: HashMap<String, String> = self
            .sources()
            .await
            .into_iter()
            .map(|source| (source.id, source.name))
            .collect();
        names.insert("stremio".into(), "Stremio".into());
        names.insert(crate::novel::LOCAL_EPUB.into(), "EPUB file".into());
        names.insert(crate::comics::LOCAL_COMICS.into(), "Local comics".into());
        names
    }

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        let entries = titles::library(&self.db(), self.kind)
            .await
            .map_err(to_message)?;
        let show_nsfw = self.extensions().show_nsfw();
        let mut memberships = raygun_db::categories::memberships(&self.db(), self.kind)
            .await
            .map_err(to_message)?;
        let names = self.source_names().await;
        Ok(entries
            .into_iter()
            .filter(|entry| show_nsfw || !is_adult(&entry.title.fields.genres))
            .map(|entry| MangaLibraryEntry {
                list: entry.title.list.as_deref().and_then(LibraryList::from_key),
                favorite: entry.title.favorite_at.is_some(),
                pinned: entry.title.pinned_at.is_some(),
                authors: entry.title.fields.authors.clone(),
                artists: entry.title.fields.artists.clone(),
                source_name: names
                    .get(&entry.title.fields.source_id)
                    .cloned()
                    .unwrap_or_else(|| entry.title.fields.source_id.clone()),
                refreshed_at: entry.title.refreshed_at.map(|ms| ms as f64),
                latest_fetched_at: entry.latest_found_at.map(|ms| ms as f64),
                local: entry
                    .title
                    .fields
                    .source_id
                    .starts_with(LOCAL_SOURCE_PREFIX),
                category_ids: memberships.remove(&entry.title.id).unwrap_or_default(),
                genres: entry.title.fields.genres.clone(),
                status: entry.title.fields.status.clone(),
                added_at: entry.title.in_library_at.unwrap_or(0) as f64,
                last_read_at: entry
                    .last_read_hlc
                    .map(|stamp| raygun_db::hlc::physical_ms(stamp as u64) as f64),
                latest_chapter_at: entry.latest_unit_at.map(|ms| ms as f64),
                started: entry.started > 0,
                downloaded: entry.downloaded as u32,
                cover_url: entry
                    .title
                    .fields
                    .cover
                    .as_deref()
                    .map(|url| self.kept_cover(url, &entry.title.fields.cover_headers)),
                title_id: entry.title.id,
                source_id: entry.title.fields.source_id,
                remote_id: entry.title.fields.remote_id,
                title: entry.title.fields.title,
                chapters: entry.units as u32,
                unread: entry.unread as u32,
            })
            .collect())
    }

    // ---- Categories (M39) ----

    pub async fn categories(&self) -> Result<Vec<LibraryCategory>, String> {
        Ok(raygun_db::categories::list(&self.db(), self.kind)
            .await
            .map_err(to_message)?
            .into_iter()
            .map(|category| LibraryCategory {
                display: category
                    .display
                    .as_deref()
                    .and_then(|json| serde_json::from_str(json).ok()),
                id: category.id,
                name: category.name,
            })
            .collect())
    }

    pub async fn create_category(&self, name: &str) -> Result<(), String> {
        let name = category_name(name)?;
        raygun_db::categories::create(&self.db(), self.kind, &name)
            .await
            .map(|_| ())
            .map_err(to_message)
    }

    pub async fn rename_category(&self, id: &str, name: &str) -> Result<(), String> {
        let name = category_name(name)?;
        raygun_db::categories::rename(&self.db(), id, &name)
            .await
            .map_err(to_message)
    }

    pub async fn delete_category(&self, id: &str) -> Result<(), String> {
        raygun_db::categories::delete(&self.db(), id)
            .await
            .map_err(to_message)
    }

    pub async fn reorder_categories(&self, ids: &[String]) -> Result<(), String> {
        raygun_db::categories::reorder(&self.db(), self.kind, ids)
            .await
            .map_err(to_message)
    }

    pub async fn set_title_categories(&self, title_id: &str, ids: &[String]) -> Result<(), String> {
        raygun_db::categories::set_title_categories(&self.db(), title_id, ids)
            .await
            .map_err(to_message)
    }

    /// A chapter of a stored title and its neighbours in reading order.
    pub async fn chapter_context(
        &self,
        title_id: &str,
        unit_id: &str,
    ) -> Result<ChapterContext, String> {
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
        Ok(ChapterContext {
            previous_id: index
                .checked_sub(1)
                .map(|previous| order[previous].id.clone()),
            next_id: order.get(index + 1).map(|next| next.id.clone()),
            unit: order[index].clone(),
            title,
        })
    }

    /// Stores how far a chapter was read; reaching `total - 1` completes it.
    pub async fn set_progress(
        &self,
        unit_id: &str,
        position: u32,
        total: u32,
    ) -> Result<(), String> {
        titles::set_progress(
            &self.db(),
            unit_id,
            position.into(),
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

    /// A cover's proxy URL: `local:<path>` covers (imported books) are files under the downloads
    /// folder; others are fetched with their headers.
    pub fn cover(&self, url: &str, headers: &std::collections::BTreeMap<String, String>) -> String {
        match url.strip_prefix(LOCAL_PREFIX) {
            Some(path) => self.images.local(path),
            None => self.images.cover(url, headers),
        }
    }

    /// A library title's cover: cached for good, whatever the cache's size limit (M46.2).
    pub fn kept_cover(
        &self,
        url: &str,
        headers: &std::collections::BTreeMap<String, String>,
    ) -> String {
        match url.strip_prefix(LOCAL_PREFIX) {
            Some(path) => self.images.local(path),
            None => self.images.kept_cover(url, headers),
        }
    }

    /// `title.kind` of this catalogue's titles.
    pub fn kind(&self) -> &'static str {
        self.kind
    }

    pub fn app(&self) -> &AppHandle {
        &self.app
    }

    pub fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    pub fn extensions(&self) -> tauri::State<'_, Arc<ExtensionService>> {
        self.app.state::<Arc<ExtensionService>>()
    }
}

/// A category name as typed: trimmed, not empty, at most 40 characters.
fn category_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the category a name".into());
    }
    Ok(name.chars().take(40).collect())
}

/// Genres that mark a title as adult (lower case, compared whole). Sources that know better mark
/// listings `nsfw` themselves; this catches stored titles and sources that only tag genres.
const ADULT_GENRES: [&str; 14] = [
    "adult",
    "hentai",
    "smut",
    "erotica",
    "erotic",
    "pornographic",
    "porn",
    "18+",
    "r18",
    "r-18",
    "nsfw",
    "mature",
    "lolicon",
    "shotacon",
];

/// Whether a title's genres mark it as adult (M35).
pub fn is_adult(genres: &[String]) -> bool {
    genres.iter().any(|genre| {
        let genre = genre.trim().to_lowercase();
        ADULT_GENRES.contains(&genre.as_str())
    })
}

/// Units in reading order: by chapter number when every unit has one, else the source's order
/// reversed (sources list newest first).
pub fn reading_order(units: &[UnitRow]) -> Vec<&UnitRow> {
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
pub fn continue_from(units: &[UnitRow]) -> Option<&UnitRow> {
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

pub fn chapter_info(
    unit: &UnitRow,
    downloads: &HashMap<String, ChapterDownload>,
) -> MangaChapterInfo {
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

pub fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

pub fn to_message(error: impl std::fmt::Display) -> String {
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
    fn adult_genres_mark_a_title() {
        let genres = |list: &[&str]| list.iter().map(|g| (*g).to_owned()).collect::<Vec<_>>();
        assert!(is_adult(&genres(&["Action", " Hentai "])));
        assert!(is_adult(&genres(&["SMUT"])));
        assert!(!is_adult(&genres(&["Ecchi", "Romance", "Adult Life"])));
        assert!(!is_adult(&[]));
    }
}
