//! Search through a source's listings: many sources (Miru's especially) answer a search with
//! nothing, or fail, while their listings work. For a search row with fewer than
//! [`FEW_RESULTS`] titles, the source's popular listing is paged through (at most [`MAX_PAGES`]
//! pages or [`MAX_TITLES`] titles, before a deadline) and its titles are matched against the query
//! with Search's ranking. Pages are kept for [`KEEP_FOR`], so typing on doesn't fetch them again,
//! and a few sources page at a time ([`AT_ONCE`]).

use std::collections::HashMap;
use std::future::Future;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_ext_host::manga::{MangaListing, MangaSummary};
use raygun_resolver::video_match::{TitleInfo, parse_title};
use tokio::sync::Semaphore;
use tokio::time::Instant;

/// A search with fewer results than this looks through the source's listings too.
pub const FEW_RESULTS: usize = 3;
/// Listing pages read at most per source.
pub const MAX_PAGES: u32 = 5;
/// Listed titles read at most per source.
pub const MAX_TITLES: usize = 150;
/// Matches added at most per row.
const MAX_MATCHES: usize = 30;
/// How long listed pages are reused.
pub const KEEP_FOR: Duration = Duration::from_secs(10 * 60);
/// How long a listing that failed isn't asked again.
const FAILED_FOR: Duration = Duration::from_secs(60);
/// Sources paging through their listings at once.
const AT_ONCE: usize = 3;
/// The least time paging gets after a slow search, so a source that took long to fail still
/// gets a look at its listings.
pub const LEAST_TIME: Duration = Duration::from_secs(6);
/// The lowest score that counts as a match: every word of the query in the title.
const MATCH_SCORE: u32 = 60;
/// A title of another season than the query names scores at most this (as in the UI's
/// `videoTitleScore`).
const OTHER_SEASON: u32 = 30;
/// Sources whose listings are kept at most.
const KEPT_SOURCES: usize = 64;

/// What was read of one source's listing.
#[derive(Debug, Default)]
pub struct Listed {
    pub items: Vec<MangaSummary>,
    /// Pages read so far.
    pub pages: u32,
    /// The listing has no more pages, or the bound is reached.
    pub done: bool,
    /// When the first page came; the pages are read again after [`KEEP_FOR`].
    at: Option<Instant>,
    /// The listing failed on its first page.
    pub failed: Option<String>,
}

impl Listed {
    /// Pages are kept [`KEEP_FOR`]; a failed listing is asked again after [`FAILED_FOR`].
    fn fresh(&self) -> bool {
        let keep = if self.failed.is_some() {
            FAILED_FOR
        } else {
            KEEP_FOR
        };
        self.at.is_some_and(|at| at.elapsed() < keep)
    }

    fn full(&self) -> bool {
        self.done || self.pages >= MAX_PAGES || self.items.len() >= MAX_TITLES
    }
}

/// Listed pages by source, and the turns sources take paging.
pub struct ListingCache {
    sources: Mutex<HashMap<String, Arc<tokio::sync::Mutex<Listed>>>>,
    turns: Semaphore,
}

impl Default for ListingCache {
    fn default() -> Self {
        Self {
            sources: Mutex::new(HashMap::new()),
            turns: Semaphore::new(AT_ONCE),
        }
    }
}

impl ListingCache {
    /// The titles of `source`'s listing that match `query`, best first, reading more pages with
    /// `fetch` (page number, from 1) until the bound or `deadline`. Pages read before stay; a
    /// listing that fails on its first page gives its error.
    pub async fn matches<F, Fut>(
        &self,
        source: &str,
        query: &str,
        video: bool,
        deadline: Instant,
        fetch: F,
    ) -> Result<Vec<MangaSummary>, String>
    where
        F: FnMut(u32) -> Fut,
        Fut: Future<Output = Result<MangaListing, String>>,
    {
        let entry = self.entry(source);
        let mut listed = match tokio::time::timeout_at(deadline, entry.lock()).await {
            Ok(listed) => listed,
            Err(_) => return Ok(Vec::new()),
        };
        if !listed.fresh() {
            *listed = Listed::default();
        }
        if !listed.full()
            && listed.failed.is_none()
            && let Ok(_turn) = tokio::time::timeout_at(deadline, self.turns.acquire()).await
        {
            read_pages(&mut listed, deadline, fetch).await;
        }
        if listed.items.is_empty()
            && let Some(error) = &listed.failed
        {
            return Err(error.clone());
        }
        Ok(best_matches(&listed.items, query, video))
    }

    fn entry(&self, source: &str) -> Arc<tokio::sync::Mutex<Listed>> {
        let mut sources = self
            .sources
            .lock()
            .unwrap_or_else(|poison| poison.into_inner());
        if sources.len() >= KEPT_SOURCES && !sources.contains_key(source) {
            // Old ones go first; ones being read stay.
            sources.retain(|_, entry| {
                entry
                    .try_lock()
                    .map_or(true, |listed| listed.fresh() && listed.pages > 0)
            });
        }
        sources.entry(source.to_owned()).or_default().clone()
    }
}

/// Reads pages after the ones in `listed` until the bound, the listing's end or `deadline`.
/// Dropping the future (a newer search) keeps the pages already read.
pub async fn read_pages<F, Fut>(listed: &mut Listed, deadline: Instant, mut fetch: F)
where
    F: FnMut(u32) -> Fut,
    Fut: Future<Output = Result<MangaListing, String>>,
{
    while !listed.full() {
        let page = listed.pages + 1;
        let answer = match tokio::time::timeout_at(deadline, fetch(page)).await {
            Ok(answer) => answer,
            // Out of time: what was read is kept, and the next search reads on.
            Err(_) => return,
        };
        match answer {
            Ok(listing) => {
                if listed.at.is_none() {
                    listed.at = Some(Instant::now());
                }
                listed.pages = page;
                let before = listed.items.len();
                for item in listing.items {
                    if listed.items.len() >= MAX_TITLES {
                        break;
                    }
                    if !listed.items.iter().any(|seen| seen.id == item.id) {
                        listed.items.push(item);
                    }
                }
                // A page with nothing new (sources that ignore the page number) is the end too.
                if !listing.has_next_page || listed.items.len() == before {
                    listed.done = true;
                }
            }
            Err(error) => {
                if page == 1 {
                    listed.at = Some(Instant::now());
                    listed.failed = Some(error);
                }
                // A later page failing ends the listing where it is.
                listed.done = true;
            }
        }
    }
}

/// The titles of `items` matching `query`, best first (a listing's own order among equals).
pub fn best_matches(items: &[MangaSummary], query: &str, video: bool) -> Vec<MangaSummary> {
    let wanted = Wanted::new(query);
    let mut scored: Vec<(u32, usize, &MangaSummary)> = items
        .iter()
        .enumerate()
        .filter_map(|(position, item)| {
            let score = if video {
                wanted.video_score(&item.title)
            } else {
                title_score(&item.title, &wanted.simple)
            };
            (score >= MATCH_SCORE).then_some((score, position, item))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(MAX_MATCHES)
        .map(|(_, _, item)| item.clone())
        .collect()
}

struct Wanted {
    simple: String,
    info: TitleInfo,
}

impl Wanted {
    fn new(query: &str) -> Self {
        Self {
            simple: raygun_resolver::normalize(query),
            info: parse_title(query),
        }
    }

    /// Search's `videoTitleScore`: the titles' bases compared, a title of another season than
    /// the one asked for scored low, and with no season asked the first season first.
    fn video_score(&self, title: &str) -> u32 {
        let found = parse_title(title);
        let base = |info: &TitleInfo, text: &str| {
            if info.base.is_empty() {
                raygun_resolver::normalize(text)
            } else {
                raygun_resolver::normalize(&info.base)
            }
        };
        let score = title_score(&base(&found, title), &base(&self.info, &self.simple));
        let asked = self.info.names_season();
        if asked && !found.same_season(&self.info) {
            return score.min(OTHER_SEASON);
        }
        if !asked && found.names_season() {
            return score.saturating_sub(found.season.unwrap_or(2).min(10));
        }
        score
    }
}

/// Search's `titleScore` (`search/rankResults.ts`) over a normalized query: exact, then starts
/// with it, then has every word of it, else the share of its words it has. A title holding the
/// query inside one word (titles without spaces, as in Chinese and Japanese) counts as having
/// every word.
pub fn title_score(title: &str, wanted: &str) -> u32 {
    let found = raygun_resolver::normalize(title);
    let wanted = raygun_resolver::normalize(wanted);
    if wanted.is_empty() || found.is_empty() {
        return 0;
    }
    if found == wanted {
        return 100;
    }
    if found.starts_with(&wanted) {
        return 80;
    }
    let words: Vec<&str> = wanted.split(' ').collect();
    let have: std::collections::HashSet<&str> = found.split(' ').collect();
    let shared = words.iter().filter(|word| have.contains(*word)).count();
    if shared == words.len() {
        return if found.contains(&wanted) { 70 } else { 60 };
    }
    if found.contains(&wanted) {
        return 65;
    }
    u32::try_from(40 * shared / words.len()).unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicU32, Ordering};

    use super::*;

    fn item(id: usize, title: &str) -> MangaSummary {
        MangaSummary {
            id: id.to_string(),
            title: title.to_owned(),
            cover_url: None,
            cover_headers: Default::default(),
            nsfw: false,
        }
    }

    /// A listing of `pages` pages of 20 titles; one in each page names "dragon".
    fn page_of(page: u32, pages: u32) -> MangaListing {
        let start = (page as usize - 1) * 20;
        MangaListing {
            items: (start..start + 20)
                .map(|id| {
                    if id % 20 == 7 {
                        item(id, &format!("The Dragon Story {id}"))
                    } else {
                        item(id, &format!("Title {id}"))
                    }
                })
                .collect(),
            has_next_page: page < pages,
        }
    }

    fn later(seconds: u64) -> Instant {
        Instant::now() + Duration::from_secs(seconds)
    }

    #[test]
    fn scores_like_search() {
        assert_eq!(title_score("One Piece", "one piece"), 100);
        assert_eq!(title_score("One Piece Film Red", "One Piece"), 80);
        assert_eq!(title_score("Film: One Piece", "one piece"), 70);
        assert_eq!(title_score("Piece of One", "one piece"), 60);
        assert_eq!(title_score("Piece of Cake", "one piece"), 20);
        assert_eq!(title_score("Café Terrace", "cafe"), 80);
        assert_eq!(title_score("进击的巨人第二季", "巨人"), 65);
        assert_eq!(title_score("anything", ""), 0);
    }

    #[test]
    fn video_matches_weigh_seasons() {
        let items = vec![
            item(1, "Overlord II"),
            item(2, "Overlord"),
            item(3, "Overlord III"),
            item(4, "Other Show"),
        ];
        let ids =
            |found: Vec<MangaSummary>| found.into_iter().map(|item| item.id).collect::<Vec<_>>();
        // No season asked: the first season first, the others after.
        assert_eq!(ids(best_matches(&items, "overlord", true)), ["2", "1", "3"]);
        // A season asked: only that season.
        assert_eq!(ids(best_matches(&items, "Overlord Season 2", true)), ["1"]);
        // Plain titles don't know seasons.
        assert_eq!(
            ids(best_matches(&items, "overlord", false)),
            ["2", "1", "3"]
        );
    }

    #[tokio::test]
    async fn pages_through_listings_to_the_bound() {
        let cache = ListingCache::default();
        let asked = Arc::new(AtomicU32::new(0));
        let fetch = |asked: Arc<AtomicU32>| {
            move |page: u32| {
                asked.fetch_add(1, Ordering::SeqCst);
                async move { Ok(page_of(page, 50)) }
            }
        };
        let found = cache
            .matches("a", "dragon story", false, later(10), fetch(asked.clone()))
            .await
            .unwrap();
        // At most MAX_PAGES pages: one match on each.
        assert_eq!(asked.load(Ordering::SeqCst), MAX_PAGES);
        assert_eq!(found.len(), MAX_PAGES as usize);
        assert!(
            found
                .iter()
                .all(|item| item.title.starts_with("The Dragon Story"))
        );
        // Typing on reuses the pages read.
        let again = cache
            .matches("a", "dragon", false, later(10), fetch(asked.clone()))
            .await
            .unwrap();
        assert_eq!(asked.load(Ordering::SeqCst), MAX_PAGES);
        assert_eq!(again.len(), MAX_PAGES as usize);
        // Nothing matches: nothing, still without asking.
        let none = cache
            .matches("a", "unknown words", false, later(10), fetch(asked.clone()))
            .await
            .unwrap();
        assert!(none.is_empty());
        assert_eq!(asked.load(Ordering::SeqCst), MAX_PAGES);
    }

    #[tokio::test]
    async fn stops_at_the_titles_bound_and_the_end() {
        let mut listed = Listed::default();
        // 60 titles a page: the third page reaches MAX_TITLES.
        read_pages(&mut listed, later(10), |page| async move {
            let start = (page as usize - 1) * 60;
            Ok(MangaListing {
                items: (start..start + 60).map(|id| item(id, "Title")).collect(),
                has_next_page: true,
            })
        })
        .await;
        assert_eq!(listed.items.len(), MAX_TITLES);
        assert_eq!(listed.pages, 3);

        // A source sending the same page again has ended.
        let mut listed = Listed::default();
        read_pages(&mut listed, later(10), |_| async { Ok(page_of(1, 9)) }).await;
        assert_eq!(listed.pages, 2);
        assert_eq!(listed.items.len(), 20);
        assert!(listed.done);
    }

    #[tokio::test]
    async fn stops_at_the_deadline_and_reads_on_later() {
        let cache = ListingCache::default();
        let slow = |page: u32| async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            Ok(page_of(page, 50))
        };
        // Time for two pages.
        let soon = Instant::now() + Duration::from_millis(750);
        let found = cache
            .matches("a", "dragon", false, soon, slow)
            .await
            .unwrap();
        assert_eq!(found.len(), 2);
        // The next search reads on from page 3.
        let found = cache
            .matches("a", "dragon", false, later(60), slow)
            .await
            .unwrap();
        assert_eq!(found.len(), MAX_PAGES as usize);
    }

    #[tokio::test]
    async fn a_failing_listing_says_why() {
        let cache = ListingCache::default();
        let failed = cache
            .matches("a", "dragon", false, later(10), |_| async {
                Err::<MangaListing, _>("no listing".to_owned())
            })
            .await;
        assert_eq!(failed, Err("no listing".to_owned()));
    }

    /// A Miru source whose search misses what it lists: iptv-org's search matches channel names
    /// case by case, so a lower-case name finds nothing there, while its listing has the channel.
    /// `cargo test -p raygun-app -- --ignored listing_search_live --nocapture`; the script comes
    /// from `MIRU_REPO`, else the reference/miru-repo checkout.
    #[tokio::test(flavor = "multi_thread")]
    #[ignore = "needs network access and a miru-repo checkout"]
    async fn listing_search_live() {
        use raygun_ext_host::manga::MangaList;
        use raygun_ext_host::{ExtensionHost, HostConfig};

        let root = std::env::var_os("MIRU_REPO").map_or_else(
            || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/miru-repo"),
            std::path::PathBuf::from,
        );
        let index = std::fs::read_to_string(root.join("index.json")).unwrap();
        let index = raygun_ext_host::repo::parse_index(&index).unwrap();
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == "miru.iptv-org")
            .unwrap();
        let file = entry.package.split('#').next().unwrap_or_default();
        let bytes = std::fs::read(root.join(file)).unwrap();
        let (code, header) = raygun_ext_host::miru::package::verify_script(&bytes, entry).unwrap();
        let package = raygun_ext_host::miru::package::package(entry, code, &header).unwrap();
        let temp = tempfile::TempDir::new().unwrap();
        raygun_ext_host::package::install(temp.path(), &package).unwrap();
        let host = ExtensionHost::start(HostConfig::default()).unwrap();
        let infos = host
            .load(raygun_ext_host::discover_installed(temp.path()))
            .await;
        let provider = infos[0].providers[0].id.clone();

        let listing = host
            .manga_list(&provider, MangaList::Popular, 1)
            .await
            .unwrap();
        let wanted = listing
            .items
            .iter()
            .take(MAX_TITLES)
            .find(|item| item.title.to_lowercase() != item.title)
            .expect("a channel with capitals")
            .clone();
        let query = wanted.title.to_lowercase();
        let searched = host.catalog_search_row(&provider, &query).await.unwrap();
        eprintln!("{query:?}: the search found {}", searched.items.len());

        let cache = ListingCache::default();
        let found = cache
            .matches(&provider, &query, true, later(30), |page| {
                host.manga_list(&provider, MangaList::Popular, page)
            })
            .await
            .unwrap();
        eprintln!(
            "its listings: {:?}",
            found
                .iter()
                .take(5)
                .map(|item| &item.title)
                .collect::<Vec<_>>()
        );
        assert!(found.iter().any(|item| item.id == wanted.id));
    }
}
