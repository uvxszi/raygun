//! What manga and novels share (M31): browsing a source capability's catalogue, titles with
//! their chapters, the library of one kind, read state and progress, and reading order. Each
//! medium's service wraps a `Catalog` and adds what reading it takes (pages or text).

use std::collections::HashMap;
use std::sync::Arc;

use raygun_db::titles::{self, TitleFields, TitleRow, UnitFields, UnitRow};
use raygun_ext_host::manga::MangaList;
use raygun_model::{
    ChapterDownload, MangaBrowseMode, MangaCard, MangaChapterInfo, MangaLibraryEntry,
    MangaListingPage, MangaSource, MangaTitleView,
};
use raygun_net::ImageLinks;
use tauri::{AppHandle, Manager};

use crate::extensions::ExtensionService;
use crate::startup::AppDb;

pub struct Catalog {
    app: AppHandle,
    pub images: ImageLinks,
    /// `title.kind`: `manga` or `novel`.
    kind: &'static str,
    /// The provider capability: `manga.source` or `novel.source`.
    capability: &'static str,
}

/// A chapter with its neighbours in reading order.
pub struct ChapterContext {
    pub title: TitleRow,
    pub unit: UnitRow,
    pub previous_id: Option<String>,
    pub next_id: Option<String>,
}

const UNIT_KIND: &str = "chapter";
/// Prefix of stored covers that are files under the downloads folder.
pub const LOCAL_PREFIX: &str = "local:";

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
        let show_nsfw = self.extensions().show_nsfw();
        let mut listing = listing;
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
            _ => Ok(self.fetch_title(source_id, remote_id).await?.0),
        }
    }

    /// Fetches and stores a title with its chapters; returns it and how many chapters were new.
    pub async fn fetch_title(
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
            cover_url: row
                .fields
                .cover
                .as_deref()
                .map(|url| self.cover(url, &row.fields.cover_headers)),
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

    pub async fn library(&self) -> Result<Vec<MangaLibraryEntry>, String> {
        let entries = titles::library(&self.db(), self.kind)
            .await
            .map_err(to_message)?;
        let show_nsfw = self.extensions().show_nsfw();
        Ok(entries
            .into_iter()
            .filter(|entry| show_nsfw || !is_adult(&entry.title.fields.genres))
            .map(|entry| MangaLibraryEntry {
                cover_url: entry
                    .title
                    .fields
                    .cover
                    .as_deref()
                    .map(|url| self.cover(url, &entry.title.fields.cover_headers)),
                title_id: entry.title.id,
                source_id: entry.title.fields.source_id,
                remote_id: entry.title.fields.remote_id,
                title: entry.title.fields.title,
                chapters: entry.units as u32,
                unread: entry.unread as u32,
            })
            .collect())
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
            None => self.images.link(url, headers),
        }
    }

    pub fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    pub fn extensions(&self) -> tauri::State<'_, Arc<ExtensionService>> {
        self.app.state::<Arc<ExtensionService>>()
    }
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
