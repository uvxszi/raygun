//! What manga and novels share (M31): browsing a source capability's catalogue, titles with
//! their chapters, the library of one kind, read state and progress, and reading order. Each
//! medium's service wraps a `Catalog` and adds what reading it takes (pages or text).

use std::collections::{BTreeMap, HashMap};
use std::sync::Arc;

use raygun_db::titles::{self, TitleFields, TitleRow, UnitFields, UnitRow};
use raygun_ext_host::listing_search::LEAST_TIME;
use raygun_ext_host::manga::{MangaList, MangaListing, MangaSummary};
use raygun_ext_host::video::VIDEO_SOURCE;
use raygun_model::{
    ChapterDownload, FilterValue, LibraryCategory, LibraryList, MangaBrowseMode, MangaCard,
    MangaChapterInfo, MangaLibraryEntry, MangaListingPage, MangaSource, MangaTitleView,
    SearchListingsPrefs, SourceFilter, SourceSearchResult,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::startup::AppDb;

/// How long a title (details and chapters) may take before its page says the source failed.
const TITLE_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(30);

pub struct Catalog {
    app: AppHandle,
    pub images: ImageLinks,
    /// `title.kind`: `manga`, `novel` or `video`.
    kind: &'static str,
    /// The provider capability: `manga.source`, `novel.source` or `video.source`.
    capability: &'static str,
    /// The latest search typed; a newer one stops reading listings for this one.
    searching: tokio::sync::watch::Sender<String>,
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
            searching: tokio::sync::watch::Sender::new(String::new()),
        }
    }

    pub async fn sources(&self) -> Vec<MangaSource> {
        let extensions = self.extensions();
        let (_ready, host) = extensions.ready().await;
        host.listed_providers(self.capability)
            .await
            .into_iter()
            .map(|provider| MangaSource {
                adult: provider.adult,
                enabled: !host.content().is_disabled(&provider.id),
                id: provider.id,
                name: provider.name,
                extension_id: provider.extension_id,
                search_pill: provider.search_pill,
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

    /// A source's listing as cards: adult items dropped while NSFW is off, library titles marked,
    /// and titles this device stored (opened or saved before) given the authors stored with them
    /// when the listing names none: one database query per page, no requests.
    async fn listing_page(
        &self,
        source_id: &str,
        mut listing: raygun_ext_host::manga::MangaListing,
    ) -> Result<MangaListingPage, String> {
        let show_nsfw = self.extensions().show_nsfw();
        listing.items.retain(|item| show_nsfw || !item.nsfw);
        let ids: Vec<String> = listing.items.iter().map(|item| item.id.clone()).collect();
        let known = titles::listed(&self.db(), source_id, &ids)
            .await
            .map_err(to_message)?;
        // A video's stored "authors" are its cast.
        let books = self.capability != VIDEO_SOURCE;
        Ok(MangaListingPage {
            items: listing_cards(source_id, listing.items, &known, books, |url, headers| {
                self.cover(url, headers)
            }),
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

    /// A source search with its filters; the query may be empty when filters are set. A first
    /// page without filters for a query its search fails at or finds nothing for gets the titles
    /// of its listings that match instead, as `listings` allows (M46.49), each marked
    /// `from_listing`.
    pub async fn search(
        &self,
        source_id: &str,
        query: &str,
        page: u32,
        filters: &BTreeMap<String, FilterValue>,
        listings: SearchListingsPrefs,
    ) -> Result<MangaListingPage, String> {
        let query = query.trim();
        // Nothing asked for yet: the source's popular titles, so Search never starts empty.
        if query.is_empty() && filters.is_empty() {
            return self
                .browse(source_id, MangaBrowseMode::Popular, None, page)
                .await;
        }
        let started = tokio::time::Instant::now();
        let (listing, listed) = {
            let extensions = self.extensions();
            let (_ready, host) = extensions.ready().await;
            let page = page.max(1);
            let answer = host
                .manga_search_filtered(source_id, query, page, filters)
                .await;
            // Later pages, and searches narrowed by filters, are the source's own.
            let listings = SearchListingsPrefs {
                enabled: listings.enabled && page == 1 && filters.is_empty(),
                ..listings
            };
            match self
                .listed_matches(host, source_id, query, &answer, listings, started)
                .await
            {
                Some(found) => (found, true),
                None => (answer?, false),
            }
        };
        let mut page = self.listing_page(source_id, listing).await?;
        for card in &mut page.items {
            card.from_listing = listed;
        }
        Ok(page)
    }

    /// The first page of every source's results for `query` (Mihon's global search), or only of
    /// the ones in `only` (Search's source chips, M46.4) when it isn't empty. A blank query gives
    /// each source's popular titles. Sources take turns, a few at a time and the good ones first
    /// (M46.48: the host's catalogue turns), none for longer than
    /// [`raygun_ext_host::CATALOG_SEARCH_TIMEOUT`]. A source that fails reports why and the
    /// others still show. Search's filters (`filter`) leave sources unasked and titles out
    /// (`search_filter::keep_cards`: "No 18+", "18+ only", "Not in library"). A source whose
    /// search fails or finds nothing gets matches from its listings as `listings` allows
    /// ([`Self::search_row`]).
    pub async fn global_search(
        &self,
        query: &str,
        only: &[String],
        filter: raygun_model::SearchFilterPrefs,
        listings: SearchListingsPrefs,
    ) -> Result<Vec<SourceSearchResult>, String> {
        let query = query.trim();
        let mut sources = self.sources().await;
        if only.is_empty() {
            sources.retain(|source| source.enabled);
        } else {
            sources.retain(|source| only.contains(&source.id));
        }
        sources.retain(|source| crate::search_filter::asks_source(filter, source));
        let medium = self.kind;
        let searches = sources.into_iter().map(|source| async move {
            match self.search_row(&source.id, query, listings).await {
                Ok((mut page, listed)) => {
                    crate::search_filter::keep_cards(filter, medium, &source, &mut page.items);
                    SourceSearchResult {
                        from_listings: listed && !page.items.is_empty(),
                        items: page.items,
                        source,
                        has_next_page: page.has_next_page,
                        error: None,
                    }
                }
                Err(error) => SourceSearchResult {
                    source,
                    items: Vec::new(),
                    has_next_page: false,
                    error: Some(error),
                    from_listings: false,
                },
            }
        });
        let mut results = futures_util::future::join_all(searches).await;
        // Sources with results first, then empty ones, then failures.
        results.sort_by_key(|result| (result.error.is_some(), result.items.is_empty()));
        Ok(results)
    }

    /// A source's first page for a search row, and whether it came from the source's listings:
    /// a search of `query` that fails or finds nothing gets the titles of its listings that match
    /// instead, as `listings` allows, each marked `from_listing` (M46.49).
    async fn search_row(
        &self,
        source_id: &str,
        query: &str,
        listings: SearchListingsPrefs,
    ) -> Result<(MangaListingPage, bool), String> {
        let started = tokio::time::Instant::now();
        let extensions = self.extensions();
        let (ready, host) = extensions.ready().await;
        let answer = host
            .catalog_search_row(source_id, query)
            .await
            .map_err(|error| error.to_string());
        let (listing, listed) = match self
            .listed_matches(host, source_id, query, &answer, listings, started)
            .await
        {
            Some(found) => (found, true),
            None => (answer?, false),
        };
        drop(ready);
        let mut page = self.listing_page(source_id, listing).await?;
        for card in &mut page.items {
            card.from_listing = listed;
        }
        Ok((page, listed))
    }

    /// For a search of `query` started at `started` that failed or found nothing (`answer`):
    /// the titles of the source's listings that match it, as `listings` allows. Reading them
    /// stops within the search's own time, but never sooner than [`LEAST_TIME`] from now, so a
    /// source that took long to fail still gets a look. `None` keeps the answer: the search
    /// answered, nothing in its listings matched, they failed too, or a newer search came.
    async fn listed_matches(
        &self,
        host: &raygun_ext_host::ExtensionHost,
        source_id: &str,
        query: &str,
        answer: &Result<MangaListing, String>,
        listings: SearchListingsPrefs,
        started: tokio::time::Instant,
    ) -> Option<MangaListing> {
        if !looks_through_listings(query, answer, listings) {
            return None;
        }
        let deadline = (started + raygun_ext_host::CATALOG_SEARCH_TIMEOUT)
            .max(tokio::time::Instant::now() + LEAST_TIME);
        let video = self.capability == VIDEO_SOURCE;
        let matching = host.listing_titles(source_id, query, video, listings, deadline);
        match self.searching(query, matching).await {
            Some(Ok(items)) if !items.is_empty() => {
                tracing::debug!(
                    source = source_id,
                    found = items.len(),
                    "search answered from the source's listings"
                );
                Some(MangaListing {
                    items,
                    has_next_page: false,
                })
            }
            _ => None,
        }
    }

    /// Runs `work` for a search of `query`; `None` once a search for another query starts here.
    async fn searching<T>(&self, query: &str, work: impl Future<Output = T>) -> Option<T> {
        self.searching.send_if_modified(|current| {
            if current == query {
                false
            } else {
                query.clone_into(current);
                true
            }
        });
        let mut changes = self.searching.subscribe();
        let replaced = async move {
            loop {
                if changes.changed().await.is_err() {
                    std::future::pending::<()>().await;
                }
                if *changes.borrow() != query {
                    return;
                }
            }
        };
        tokio::select! {
            done = work => Some(done),
            () = replaced => None,
        }
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
        tracing::info!(
            kind = self.kind,
            source = source_id,
            id = remote_id,
            "fetching a title"
        );
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
            // A source that never answers ends in an error, not a page loading forever (M46.45).
            tokio::time::timeout(
                TITLE_TIMEOUT,
                futures_util::future::join(host.manga_details(source_id, remote_id), units),
            )
            .await
            .map_err(|_| {
                tracing::warn!(
                    source = source_id,
                    id = remote_id,
                    "a title didn't load in time"
                );
                "The source didn't answer in time".to_owned()
            })?
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
        let source = self
            .sources()
            .await
            .into_iter()
            .find(|source| source.id == row.fields.source_id);
        let adult =
            is_adult(&row.fields.genres) || source.as_ref().is_some_and(|source| source.adult);
        let source_name = source.map_or_else(|| row.fields.source_id.clone(), |source| source.name);
        let continue_chapter_id = continue_from(&units).map(|unit| unit.id.clone());
        let hidden_nsfw = hidden_in_library(self.extensions().show_nsfw(), &row.fields.genres);
        Ok(MangaTitleView {
            hidden_nsfw,
            adult,
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

    /// Adds a title to the library or takes it out. Taking it out also drops its list, favorite
    /// and pin (M46.38): a favorite is always in the library, so it never goes missing from it.
    pub async fn set_in_library(&self, title_id: &str, in_library: bool) -> Result<(), String> {
        let db = self.db();
        titles::set_in_library(&db, title_id, in_library.then(now_ms))
            .await
            .map_err(to_message)?;
        if !in_library {
            let ids = [title_id.to_owned()];
            for mark in [
                titles::TitleMark::List(None),
                titles::TitleMark::Favorite(None),
                titles::TitleMark::Pinned(None),
            ] {
                titles::set_mark(&db, &ids, &mark)
                    .await
                    .map_err(to_message)?;
            }
        }
        // Quick access follows the library (M46.41).
        if let Ok(Some(row)) = titles::title(&db, title_id).await {
            let pin = crate::quick_access::title_pin(&row);
            if in_library {
                crate::quick_access::add(&self.app, pin).await;
            } else {
                crate::quick_access::remove(&self.app, &pin.to, &pin.search).await;
            }
        }
        Ok(())
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

    /// The library titles [`Self::library`] leaves out while 18+ content is off (none while it's
    /// on), for the library's "hidden entries" note.
    pub async fn hidden(&self) -> Result<Vec<raygun_model::HiddenLibraryEntry>, String> {
        let entries = titles::library(&self.db(), self.kind)
            .await
            .map_err(to_message)?;
        let (_shown, hidden) = split_hidden(self.extensions().show_nsfw(), entries);
        if hidden.is_empty() {
            return Ok(Vec::new());
        }
        let names = self.source_names().await;
        Ok(hidden
            .into_iter()
            .map(|entry| {
                let fields = entry.title.fields;
                raygun_model::HiddenLibraryEntry {
                    cover_url: fields
                        .cover
                        .as_deref()
                        .map(|url| self.kept_cover(url, &fields.cover_headers)),
                    source_name: names
                        .get(&fields.source_id)
                        .cloned()
                        .unwrap_or_else(|| fields.source_id.clone()),
                    title_id: entry.title.id,
                    source_id: fields.source_id,
                    remote_id: fields.remote_id,
                    title: fields.title,
                }
            })
            .collect())
    }

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        let entries = titles::library(&self.db(), self.kind)
            .await
            .map_err(to_message)?;
        self.entries(entries).await
    }

    /// Titles with downloaded chapters or episodes, in the library or not (Downloads).
    pub async fn downloaded(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        let entries = titles::downloaded(&self.db(), self.kind)
            .await
            .map_err(to_message)?;
        self.entries(entries).await
    }

    /// Titles as the library lists them: adult ones left out unless shown, with their
    /// categories, source names and kept covers.
    async fn entries(
        &self,
        entries: Vec<titles::LibraryTitle>,
    ) -> Result<Vec<MangaLibraryEntry>, String> {
        let (entries, _hidden) = split_hidden(self.extensions().show_nsfw(), entries);
        let mut memberships = raygun_db::categories::memberships(&self.db(), self.kind)
            .await
            .map_err(to_message)?;
        let names = self.source_names().await;
        Ok(entries
            .into_iter()
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
        self.name_free(&name, None).await?;
        raygun_db::categories::create(&self.db(), self.kind, &name)
            .await
            .map(|_| ())
            .map_err(to_message)
    }

    pub async fn rename_category(&self, id: &str, name: &str) -> Result<(), String> {
        let name = category_name(name)?;
        self.name_free(&name, Some(id)).await?;
        raygun_db::categories::rename(&self.db(), id, &name)
            .await
            .map_err(to_message)
    }

    /// Two categories of one library can't share a name (Mihon's rule): their tabs would look
    /// the same.
    async fn name_free(&self, name: &str, except: Option<&str>) -> Result<(), String> {
        let taken = raygun_db::categories::list(&self.db(), self.kind)
            .await
            .map_err(to_message)?
            .into_iter()
            .any(|category| {
                Some(category.id.as_str()) != except && same_name(&category.name, name)
            });
        if taken {
            return Err(format!("There's already a category called {name}"));
        }
        Ok(())
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

    /// A chapter of a stored title and its neighbours in reading order; `skip_duplicates` steps
    /// over other uploads of the same chapter number.
    pub async fn chapter_context(
        &self,
        title_id: &str,
        unit_id: &str,
        skip_duplicates: bool,
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
        let (previous, next) = neighbours(&order, index, skip_duplicates);
        Ok(ChapterContext {
            previous_id: previous.map(|unit| unit.id.clone()),
            next_id: next.map(|unit| unit.id.clone()),
            unit: order[index].clone(),
            title,
        })
    }

    /// Marks chapters read or unread together.
    pub async fn set_units_read(&self, unit_ids: &[String], read: bool) -> Result<(), String> {
        titles::set_units_completed(&self.db(), unit_ids, read, now_ms())
            .await
            .map_err(to_message)
    }

    /// Marks every chapter before `unit_id` in reading order read (Mihon's "Mark previous as
    /// read"); returns how many there were.
    pub async fn mark_previous_read(&self, title_id: &str, unit_id: &str) -> Result<u32, String> {
        let db = self.db();
        let units = titles::units(&db, title_id).await.map_err(to_message)?;
        let ids = previous_unread(&units, unit_id)
            .ok_or_else(|| "This chapter isn't part of the title".to_owned())?;
        titles::set_units_completed(&db, &ids, true, now_ms())
            .await
            .map_err(to_message)?;
        Ok(ids.len() as u32)
    }

    /// Stores how far a chapter was read; reaching `total - 1` completes it. Nothing is stored
    /// while incognito is on (M47).
    pub async fn set_progress(
        &self,
        unit_id: &str,
        position: u32,
        total: u32,
    ) -> Result<(), String> {
        if crate::privacy::incognito(&self.app) {
            return Ok(());
        }
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

/// Whether a source's search of `query` that gave `answer` is looked through its listings
/// instead (M46.49): only when looking is on and something was typed, and only when the search
/// failed or found nothing, never when it answered.
fn looks_through_listings(
    query: &str,
    answer: &Result<MangaListing, String>,
    listings: SearchListingsPrefs,
) -> bool {
    listings.enabled
        && !query.trim().is_empty()
        && answer
            .as_ref()
            .map_or(true, |listing| listing.items.is_empty())
}

/// A source's listed titles as cards: marked when in the library, and with the authors this
/// device stored for them (`known`, by the source's ids) where the listing names none and the
/// titles are books or comics (`books`). `cover` links a cover through the image proxy.
fn listing_cards(
    source_id: &str,
    items: Vec<MangaSummary>,
    known: &HashMap<String, titles::ListedTitle>,
    books: bool,
    cover: impl Fn(&str, &BTreeMap<String, String>) -> String,
) -> Vec<MangaCard> {
    items
        .into_iter()
        .map(|item| {
            let stored = known.get(&item.id);
            let authors = match stored {
                Some(stored) if books && item.authors.is_empty() => stored.authors.clone(),
                _ => item.authors,
            };
            MangaCard {
                adult: item.nsfw,
                source_id: source_id.to_owned(),
                in_library: stored.is_some_and(|stored| stored.in_library),
                cover_url: item
                    .cover_url
                    .as_deref()
                    .map(|url| cover(url, &item.cover_headers)),
                id: item.id,
                title: item.title,
                from_listing: false,
                authors,
            }
        })
        .collect()
}

/// A category name as typed: trimmed, not empty, at most 40 characters.
fn category_name(name: &str) -> Result<String, String> {
    let name = name.trim();
    if name.is_empty() {
        return Err("Give the category a name".into());
    }
    Ok(name.chars().take(40).collect())
}

/// Whether two category names read the same, whatever their case.
fn same_name(a: &str, b: &str) -> bool {
    a.trim().to_lowercase() == b.trim().to_lowercase()
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

/// Whether the library leaves a title out: its genres mark it adult and 18+ content is off.
pub fn hidden_in_library(show_nsfw: bool, genres: &[String]) -> bool {
    !show_nsfw && is_adult(genres)
}

/// Library titles split into those the library shows and those it leaves out
/// ([`hidden_in_library`]), each in their order.
pub fn split_hidden(
    show_nsfw: bool,
    entries: Vec<titles::LibraryTitle>,
) -> (Vec<titles::LibraryTitle>, Vec<titles::LibraryTitle>) {
    entries
        .into_iter()
        .partition(|entry| !hidden_in_library(show_nsfw, &entry.title.fields.genres))
}

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

/// The chapters before and after `index` in reading order. With `skip_duplicates`, other uploads
/// of the chapter's number are stepped over, and among the uploads of the number reached the one
/// by the same group is taken (Mihon's "skip duplicate chapters").
pub fn neighbours<'a>(
    order: &[&'a UnitRow],
    index: usize,
    skip_duplicates: bool,
) -> (Option<&'a UnitRow>, Option<&'a UnitRow>) {
    if !skip_duplicates {
        return (
            index.checked_sub(1).map(|previous| order[previous]),
            order.get(index + 1).copied(),
        );
    }
    let current = order[index];
    (
        step(current, order[..index].iter().rev().copied()),
        step(current, order[index + 1..].iter().copied()),
    )
}

/// The first chapter among `candidates` (in the direction of travel) that isn't another upload of
/// `current`: of the uploads of that number, the one by `current`'s group if there is one.
fn step<'a>(
    current: &UnitRow,
    candidates: impl Iterator<Item = &'a UnitRow>,
) -> Option<&'a UnitRow> {
    let mut candidates = candidates;
    let Some(number) = current.fields.number else {
        return candidates.next();
    };
    let mut group: Vec<&'a UnitRow> = Vec::new();
    for unit in candidates {
        match (unit.fields.number, group.first()) {
            (Some(other), _) if other == number => continue,
            (Some(_), None) => group.push(unit),
            (Some(other), Some(first)) if first.fields.number == Some(other) => group.push(unit),
            (None, None) => return Some(unit),
            _ => break,
        }
    }
    group
        .iter()
        .find(|unit| unit.fields.scanlator == current.fields.scanlator)
        .or(group.first())
        .copied()
}

/// The unread chapters before `unit_id` in reading order, or `None` when it isn't one of `units`.
pub fn previous_unread(units: &[UnitRow], unit_id: &str) -> Option<Vec<String>> {
    let order = reading_order(units);
    let index = order.iter().position(|unit| unit.id == unit_id)?;
    Some(
        order[..index]
            .iter()
            .filter(|unit| unit.completed_at.is_none())
            .map(|unit| unit.id.clone())
            .collect(),
    )
}

/// Where reading continues: the chapter read last while it's unfinished, else the next unread one
/// after it, else the first unread one, else the first chapter (see [`continue_index`]).
pub fn continue_from(units: &[UnitRow]) -> Option<&UnitRow> {
    let order = reading_order(units);
    let index = continue_index(
        &order,
        |unit| {
            (unit.completed_at.is_some() || unit.position.unwrap_or(0) > 0)
                .then(|| unit.progress_at.unwrap_or(0))
        },
        |unit| unit.completed_at.is_some(),
        |_| true,
        |_| false,
    )?;
    order.get(index).copied()
}

/// Where reading or watching a title continues, as an index into `order` (its chapters or
/// episodes in reading or watching order); the one rule for every Continue (M46.48):
/// - the one read or watched last (`touched` gives when, for those with progress or finished)
///   while it's unfinished, with its place;
/// - after a finished one, the next unfinished one that is `out`;
/// - else the first one out after the last one finished in order, else the first unfinished one
///   out (a gap), else the first.
///
/// Specials (`special`) only count when the title has nothing else, or when one was watched last.
/// Picking the first partly watched one instead sent Continue back to an episode opened by mistake
/// long before the one watched last.
pub fn continue_index<T>(
    order: &[T],
    touched: impl Fn(&T) -> Option<i64>,
    finished: impl Fn(&T) -> bool,
    out: impl Fn(&T) -> bool,
    special: impl Fn(&T) -> bool,
) -> Option<usize> {
    if order.is_empty() {
        return None;
    }
    let regular: Vec<usize> = (0..order.len())
        .filter(|&index| !special(&order[index]))
        .collect();
    let pool = if regular.is_empty() {
        (0..order.len()).collect()
    } else {
        regular
    };
    let open = |index: usize| !finished(&order[index]) && out(&order[index]);
    // Of two touched at the same moment (chapters marked read together), the later one.
    let last = order
        .iter()
        .enumerate()
        .filter_map(|(index, item)| touched(item).map(|at| (at, index)))
        .max();
    if let Some((_, index)) = last {
        if !finished(&order[index]) {
            return Some(index);
        }
        if let Some(&next) = pool.iter().find(|&&next| next > index && open(next)) {
            return Some(next);
        }
    }
    let after_last = pool
        .iter()
        .rposition(|&index| finished(&order[index]))
        .map_or(0, |position| position + 1);
    pool[after_last..]
        .iter()
        .find(|&&index| open(index))
        .or_else(|| pool.iter().find(|&&index| open(index)))
        .or(pool.first())
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
            progress_at: (done || position.is_some()).then_some(1),
        }
    }

    /// A unit read or watched at `at`.
    fn touched(mut unit: UnitRow, at: i64) -> UnitRow {
        unit.progress_at = Some(at);
        unit
    }

    fn ids(units: Vec<&UnitRow>) -> Vec<&str> {
        units.into_iter().map(|unit| unit.id.as_str()).collect()
    }

    fn library_title(id: &str, genres: &[&str]) -> titles::LibraryTitle {
        titles::LibraryTitle {
            title: TitleRow {
                id: id.into(),
                fields: TitleFields {
                    kind: "manga".into(),
                    source_id: "src".into(),
                    remote_id: id.into(),
                    title: id.into(),
                    cover: None,
                    cover_headers: Default::default(),
                    authors: Vec::new(),
                    artists: Vec::new(),
                    description: None,
                    genres: genres.iter().map(|genre| (*genre).to_owned()).collect(),
                    status: String::new(),
                    url: None,
                    media_type: None,
                    year: None,
                },
                in_library_at: Some(1),
                refreshed_at: None,
                reader_mode: String::new(),
                list: None,
                favorite_at: None,
                pinned_at: None,
            },
            units: 0,
            unread: 0,
            started: 0,
            downloaded: 0,
            last_read_hlc: None,
            latest_unit_at: None,
            latest_found_at: None,
        }
    }

    #[test]
    fn adult_titles_are_hidden_from_the_library_only_while_adult_content_is_off() {
        let entries = || {
            vec![
                library_title("a", &["Action"]),
                library_title("b", &["Hentai"]),
                library_title("c", &[]),
                library_title("d", &["Romance", " Smut "]),
            ]
        };
        let names = |list: &[titles::LibraryTitle]| {
            list.iter()
                .map(|entry| entry.title.id.clone())
                .collect::<Vec<_>>()
        };
        let (shown, hidden) = split_hidden(false, entries());
        assert_eq!(names(&shown), ["a", "c"]);
        assert_eq!(names(&hidden), ["b", "d"]);

        let (shown, hidden) = split_hidden(true, entries());
        assert_eq!(names(&shown), ["a", "b", "c", "d"]);
        assert!(hidden.is_empty());
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

    fn upload(id: &str, number: f64, order: i64, group: &str) -> UnitRow {
        let mut row = unit(id, Some(number), order, None, false);
        row.fields.scanlator = Some(group.into());
        row
    }

    #[test]
    fn neighbours_step_over_other_uploads_of_a_chapter() {
        // Newest first, as sources list them: two groups upload chapters 1 and 2, one chapter 3.
        let units = [
            upload("3a", 3.0, 0, "A"),
            upload("2b", 2.0, 1, "B"),
            upload("2a", 2.0, 2, "A"),
            upload("1b", 1.0, 3, "B"),
            upload("1a", 1.0, 4, "A"),
        ];
        let order = reading_order(&units);
        let at = |id: &str| order.iter().position(|unit| unit.id == id).unwrap();
        fn ids<'a>(
            (previous, next): (Option<&'a UnitRow>, Option<&'a UnitRow>),
        ) -> (Option<&'a str>, Option<&'a str>) {
            (
                previous.map(|unit| unit.id.as_str()),
                next.map(|unit| unit.id.as_str()),
            )
        }
        assert_eq!(
            ids(neighbours(&order, at("1a"), true)),
            (None, Some("2a")),
            "the next number, by the same group"
        );
        assert_eq!(
            ids(neighbours(&order, at("2b"), true)),
            (Some("1b"), Some("3a"))
        );
        assert_eq!(ids(neighbours(&order, at("3a"), true)), (Some("2a"), None));
        assert_eq!(
            ids(neighbours(&order, at("2a"), false)),
            (Some("1b"), Some("2b")),
            "every upload in order when duplicates aren't skipped"
        );

        let unnumbered = [
            unit("extra", None, 0, None, false),
            upload("c1", 1.0, 1, "A"),
        ];
        let order = reading_order(&unnumbered);
        assert_eq!(ids(neighbours(&order, 0, true)), (None, Some("extra")));
    }

    #[test]
    fn previous_chapters_are_the_unread_ones_before() {
        let units = [
            unit("c4", Some(4.0), 0, None, false),
            unit("c3", Some(3.0), 1, None, false),
            unit("c2", Some(2.0), 2, None, true),
            unit("c1", Some(1.0), 3, Some(3), false),
        ];
        assert_eq!(
            previous_unread(&units, "c4").unwrap(),
            ["c1", "c3"],
            "chapters read already are left alone"
        );
        assert!(previous_unread(&units, "c1").unwrap().is_empty());
        assert!(previous_unread(&units, "gone").is_none());
    }

    #[test]
    fn continue_follows_what_was_read_or_watched_last() {
        // Episode 1 was opened for a moment, then episode 7 watched well into it.
        let mut episodes: Vec<UnitRow> = (1..=10)
            .map(|number| {
                unit(
                    &format!("e{number}"),
                    Some(f64::from(number)),
                    i64::from(10 - number),
                    None,
                    false,
                )
            })
            .collect();
        episodes[0] = touched(unit("e1", Some(1.0), 9, Some(4_000), false), 100);
        episodes[6] = touched(unit("e7", Some(7.0), 3, Some(1_500_000), false), 900);
        assert_eq!(continue_from(&episodes).unwrap().id, "e7", "not episode 1");

        // Finished: the next one, even with an older episode left half watched.
        episodes[6] = touched(unit("e7", Some(7.0), 3, None, true), 950);
        assert_eq!(continue_from(&episodes).unwrap().id, "e8");

        // The last one finished: back to the first one left unfinished.
        episodes[9] = touched(unit("e10", Some(10.0), 0, None, true), 990);
        assert_eq!(continue_from(&episodes).unwrap().id, "e1");

        // Marked read together (one stamp): after the last of them.
        let marked = [
            touched(unit("c1", Some(1.0), 2, None, true), 50),
            touched(unit("c2", Some(2.0), 1, None, true), 50),
            unit("c3", Some(3.0), 0, None, false),
        ];
        assert_eq!(continue_from(&marked).unwrap().id, "c3");
    }

    #[test]
    fn continue_index_skips_what_is_not_out_yet() {
        // (watched at, finished, out)
        let items = [
            (Some(10), true, true),
            (None, false, false),
            (None, false, true),
        ];
        let pick = |items: &[(Option<i64>, bool, bool)]| {
            continue_index(
                items,
                |item| item.0,
                |item| item.1,
                |item| item.2,
                |_| false,
            )
        };
        assert_eq!(pick(&items), Some(2));
        assert_eq!(
            pick(&[(None, false, false)]),
            Some(0),
            "nothing out: the first"
        );
        assert_eq!(pick(&[]), None);
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
    fn only_searches_that_failed_or_found_nothing_look_through_listings() {
        let on = SearchListingsPrefs::default();
        let found = |count: usize| -> Result<MangaListing, String> {
            Ok(MangaListing {
                items: (0..count)
                    .map(|id| raygun_ext_host::manga::MangaSummary {
                        id: id.to_string(),
                        title: format!("Title {id}"),
                        cover_url: None,
                        cover_headers: BTreeMap::new(),
                        authors: Vec::new(),
                        nsfw: false,
                    })
                    .collect(),
                has_next_page: false,
            })
        };
        assert!(looks_through_listings("dragon", &found(0), on));
        assert!(looks_through_listings(
            "dragon",
            &Err("HTTP 500".into()),
            on
        ));
        // A source that answered keeps its answer, even a short one.
        assert!(!looks_through_listings("dragon", &found(1), on));
        // Nothing typed: the popular titles are the answer.
        assert!(!looks_through_listings("  ", &found(0), on));
        let off = SearchListingsPrefs {
            enabled: false,
            ..on
        };
        assert!(!looks_through_listings(
            "dragon",
            &Err("HTTP 500".into()),
            off
        ));
    }

    #[test]
    fn listed_titles_get_the_authors_this_device_stored() {
        let summary = |id: &str, authors: &[&str]| MangaSummary {
            id: id.to_owned(),
            title: format!("Book {id}"),
            cover_url: Some(format!("https://books.example/{id}.jpg")),
            cover_headers: BTreeMap::new(),
            authors: authors.iter().map(|name| (*name).to_owned()).collect(),
            nsfw: id == "adult",
        };
        let stored = |in_library: bool, authors: &[&str]| titles::ListedTitle {
            in_library,
            authors: authors.iter().map(|name| (*name).to_owned()).collect(),
        };
        let known = HashMap::from([
            ("saved".to_owned(), stored(true, &["Jane Austen"])),
            ("opened".to_owned(), stored(false, &["Bram Stoker"])),
            ("named".to_owned(), stored(false, &["Stored Name"])),
        ]);
        let items = || {
            vec![
                summary("saved", &[]),
                summary("opened", &[]),
                summary("named", &["Listed Name"]),
                summary("new", &[]),
                summary("adult", &["Someone"]),
            ]
        };
        let cover = |url: &str, _: &BTreeMap<String, String>| format!("proxy:{url}");
        let cards = listing_cards("src/books", items(), &known, true, cover);
        let authors: Vec<Vec<String>> = cards.iter().map(|card| card.authors.clone()).collect();
        assert_eq!(
            authors,
            [
                vec!["Jane Austen".to_owned()],
                vec!["Bram Stoker".to_owned()],
                vec!["Listed Name".to_owned()],
                vec![],
                vec!["Someone".to_owned()],
            ],
            "the listing's own authors first, then the stored ones"
        );
        assert_eq!(
            cards.iter().map(|card| card.in_library).collect::<Vec<_>>(),
            [true, false, false, false, false]
        );
        assert_eq!(
            cards[0].cover_url.as_deref(),
            Some("proxy:https://books.example/saved.jpg")
        );
        assert!(cards[4].adult);
        assert!(cards.iter().all(|card| card.source_id == "src/books"));

        // A video's stored authors are its cast: its cards keep only what the listing says.
        let videos = listing_cards("src/films", items(), &known, false, cover);
        assert!(videos[1].authors.is_empty());
        assert_eq!(videos[2].authors, ["Listed Name"]);
        assert!(videos[0].in_library);
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
