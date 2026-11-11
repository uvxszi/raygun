//! Search through a source's listings (M46.49): many sources answer a search with nothing, or
//! fail, while their listings work. When a source's search for something typed fails or finds
//! nothing (and `Settings::search_listings` is on), its listings are read instead, in turn and a
//! page at a time: a comic, book or video source's popular and latest titles, then blank searches
//! sorted by its own sort filters ([`sort_listings`]); a music source's charts. What they hold is
//! matched against the query with Search's own title ranking ([`raygun_resolver::title_score`]).
//! Reading stops once enough match, at the pages allowed or at the search's deadline. What was
//! read is kept for [`KEEP_FOR`], so typing on doesn't fetch it again and a later search reads on
//! from there; a few sources read at a time ([`AT_ONCE`]).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::future::Future;
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raygun_model::{
    FilterValue, OnlineTrack, ProviderInfo, SearchListingsPrefs, SourceFilter, TrackQuery,
};
use raygun_resolver::title_score;
use raygun_resolver::video_match::{TitleInfo, parse_title};
use tokio::sync::Semaphore;
use tokio::time::Instant;

use crate::ExtensionHost;
use crate::manga::{MangaList, MangaListing, MangaSummary};

/// How long what was read is reused.
pub const KEEP_FOR: Duration = Duration::from_secs(10 * 60);
/// How long a source whose listings all failed isn't asked again.
const FAILED_FOR: Duration = Duration::from_secs(60);
/// Sources reading their listings at once.
const AT_ONCE: usize = 3;
/// The least time reading gets after a slow search, so a source that took long to fail still
/// gets a look at its listings.
pub const LEAST_TIME: Duration = Duration::from_secs(6);
/// The lowest score that counts as a match: every word of the query in the title.
const MATCH_SCORE: u32 = 60;
/// A title of another season than the query names scores at most this (as in the UI's
/// `videoTitleScore`).
const OTHER_SEASON: u32 = 30;
/// Sources whose listings are kept at most.
const KEPT_SOURCES: usize = 32;
/// Listed items kept at most per source, however many pages are allowed.
const MAX_ITEMS: usize = 1_000;
/// Sorts taken at most from a source's filters.
const SORTS: usize = 2;
/// Songs asked of each chart.
const CHART_SONGS: u32 = 50;

/// One page of a listing.
#[derive(Debug, Clone, PartialEq)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub has_next: bool,
}

impl From<MangaListing> for Page<MangaSummary> {
    fn from(listing: MangaListing) -> Self {
        Self {
            items: listing.items,
            has_next: listing.has_next_page,
        }
    }
}

/// How far a search reads one source's listings.
#[derive(Debug, Clone, Copy)]
pub struct Bounds {
    /// Pages read at most, all its listings together.
    pub pages: u32,
    /// Matches enough to stop reading, and given at most.
    pub entries: usize,
    pub deadline: Instant,
}

impl Bounds {
    pub fn new(prefs: SearchListingsPrefs, deadline: Instant) -> Self {
        Self {
            pages: prefs.pages,
            entries: prefs.entries as usize,
            deadline,
        }
    }
}

/// How far one listing was read.
struct Reading<L> {
    listing: L,
    /// Pages read.
    read: u32,
    /// Read to its end, or it failed.
    done: bool,
    /// The last page's ids, to tell a source that sends the same page whatever the number.
    last: Vec<String>,
}

impl<L> Reading<L> {
    fn new(listing: L) -> Self {
        Self {
            listing,
            read: 0,
            done: false,
            last: Vec::new(),
        }
    }
}

/// What was read of one source's listings.
struct Listed<T, L> {
    items: Vec<T>,
    ids: HashSet<String>,
    lists: Vec<Reading<L>>,
    /// The listings known at once were added.
    seeded: bool,
    /// More listings were looked for.
    discovered: bool,
    /// Where the next turn starts.
    next: usize,
    /// Pages read, every listing together.
    pages: u32,
    /// When the first answer came; it's all read again after [`KEEP_FOR`].
    at: Option<Instant>,
    /// The first failure, given when nothing could be read.
    failed: Option<String>,
}

impl<T, L> Default for Listed<T, L> {
    fn default() -> Self {
        Self {
            items: Vec::new(),
            ids: HashSet::new(),
            lists: Vec::new(),
            seeded: false,
            discovered: false,
            next: 0,
            pages: 0,
            at: None,
            failed: None,
        }
    }
}

impl<T, L: Clone> Listed<T, L> {
    /// What was read is kept [`KEEP_FOR`]; listings that all failed are asked again after
    /// [`FAILED_FOR`].
    fn fresh(&self) -> bool {
        let keep = if self.items.is_empty() && self.failed.is_some() {
            FAILED_FOR
        } else {
            KEEP_FOR
        };
        self.at.is_some_and(|at| at.elapsed() < keep)
    }

    /// Whether reading on could find more within `bounds`.
    fn open(&self, bounds: &Bounds) -> bool {
        self.pages < bounds.pages
            && self.items.len() < MAX_ITEMS
            && (!self.discovered || self.lists.iter().any(|list| !list.done))
    }

    fn matching(&self, score: &impl Fn(&T) -> u32) -> usize {
        self.items
            .iter()
            .filter(|item| score(item) >= MATCH_SCORE)
            .count()
    }

    /// The next listing in turn that isn't done.
    fn turn(&self) -> Option<usize> {
        let count = self.lists.len();
        (0..count)
            .map(|step| (self.next + step) % count)
            .find(|&index| !self.lists[index].done)
    }

    /// Reads pages in turn, one of each listing at a time, until enough match, the bounds or
    /// the end of every listing. Once each listing had a page read, `discover` adds more (once).
    /// Out of time, what was read stays and the next search reads on from there.
    async fn read<S, D, DF, F, FF>(
        &mut self,
        bounds: &Bounds,
        score: &S,
        discover: D,
        mut fetch: F,
        id: fn(&T) -> String,
    ) where
        S: Fn(&T) -> u32,
        D: FnOnce() -> DF,
        DF: Future<Output = Vec<L>>,
        F: FnMut(L, u32) -> FF,
        FF: Future<Output = Result<Page<T>, String>>,
    {
        let mut discover = Some(discover);
        while self.open(bounds) && self.matching(score) < bounds.entries {
            let due = !self.discovered && self.lists.iter().all(|list| list.read > 0 || list.done);
            if due {
                let Some(discover) = discover.take() else {
                    return;
                };
                let Ok(found) = tokio::time::timeout_at(bounds.deadline, discover()).await else {
                    return;
                };
                self.discovered = true;
                self.lists.extend(found.into_iter().map(Reading::new));
                continue;
            }
            let Some(index) = self.turn() else {
                return;
            };
            let page = self.lists[index].read + 1;
            let listing = self.lists[index].listing.clone();
            let Ok(answer) = tokio::time::timeout_at(bounds.deadline, fetch(listing, page)).await
            else {
                return;
            };
            self.next = index + 1;
            self.at.get_or_insert_with(Instant::now);
            let reading = &mut self.lists[index];
            match answer {
                Ok(found) => {
                    self.pages += 1;
                    reading.read = page;
                    let ids: Vec<String> = found.items.iter().map(id).collect();
                    // A page like the last one (sources that ignore the number) is the end too.
                    reading.done = !found.has_next || ids.is_empty() || ids == reading.last;
                    reading.last = ids;
                    for item in found.items {
                        if self.items.len() >= MAX_ITEMS {
                            break;
                        }
                        if self.ids.insert(id(&item)) {
                            self.items.push(item);
                        }
                    }
                }
                Err(error) => {
                    // A failing listing ends where it is; the others go on.
                    reading.done = true;
                    self.failed.get_or_insert(error);
                }
            }
        }
    }
}

/// One source's listings as read, locked while they're read.
type Entry<T, L> = Arc<tokio::sync::Mutex<Listed<T, L>>>;

/// What was read of sources' listings, by source, and the turns sources take reading.
pub struct ListingCache<T, L> {
    sources: Mutex<HashMap<String, Entry<T, L>>>,
    turns: Semaphore,
    /// An item's identity, so listings that share items list each once.
    id: fn(&T) -> String,
}

impl<T: Clone, L: Clone> ListingCache<T, L> {
    pub fn new(id: fn(&T) -> String) -> Self {
        Self {
            sources: Mutex::new(HashMap::new()),
            turns: Semaphore::new(AT_ONCE),
            id,
        }
    }

    /// The items of `source`'s listings that `score` rates as matches, best first (a listing's
    /// own order among equals), at most `bounds.entries`. Reads on first while fewer match: the
    /// `first` listings, then those `discover` finds, each a page at a time with `fetch` (page
    /// numbers from 1), within `bounds`. Nothing read and a listing failed: that failure.
    pub async fn matches<S, D, DF, F, FF>(
        &self,
        source: &str,
        bounds: Bounds,
        score: S,
        first: Vec<L>,
        discover: D,
        fetch: F,
    ) -> Result<Vec<T>, String>
    where
        S: Fn(&T) -> u32,
        D: FnOnce() -> DF,
        DF: Future<Output = Vec<L>>,
        F: FnMut(L, u32) -> FF,
        FF: Future<Output = Result<Page<T>, String>>,
    {
        let entry = self.entry(source);
        let Ok(mut listed) = tokio::time::timeout_at(bounds.deadline, entry.lock()).await else {
            return Ok(Vec::new());
        };
        if !listed.fresh() {
            *listed = Listed::default();
        }
        if !listed.seeded {
            listed.seeded = true;
            listed.lists = first.into_iter().map(Reading::new).collect();
        }
        if listed.open(&bounds)
            && listed.matching(&score) < bounds.entries
            && let Ok(_turn) = tokio::time::timeout_at(bounds.deadline, self.turns.acquire()).await
        {
            listed.read(&bounds, &score, discover, fetch, self.id).await;
        }
        if listed.items.is_empty()
            && let Some(error) = &listed.failed
        {
            return Err(error.clone());
        }
        Ok(best(&listed.items, &score, bounds.entries))
    }

    fn entry(&self, source: &str) -> Entry<T, L> {
        let mut sources = self.sources.lock().unwrap_or_else(PoisonError::into_inner);
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

/// The items `score` rates as matches, best first (their order among equals), at most `entries`.
fn best<T: Clone>(items: &[T], score: &impl Fn(&T) -> u32, entries: usize) -> Vec<T> {
    let mut scored: Vec<(u32, usize, &T)> = items
        .iter()
        .enumerate()
        .filter_map(|(position, item)| {
            let score = score(item);
            (score >= MATCH_SCORE).then_some((score, position, item))
        })
        .collect();
    scored.sort_by(|a, b| b.0.cmp(&a.0).then(a.1.cmp(&b.1)));
    scored
        .into_iter()
        .take(entries)
        .map(|(_, _, item)| item.clone())
        .collect()
}

/// A comic, book or video source's listing that a search reads.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TitleListing {
    Popular,
    Latest,
    /// A blank search sorted by the source's own filter `key` (a sort or order select).
    Sorted {
        key: String,
        value: String,
    },
}

/// A chart a search reads: the `music.charts` provider keeping it, and its id there.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ChartRef {
    pub provider: String,
    pub id: String,
}

/// What searches read of sources' listings lately, shared by every search.
pub(crate) struct Listings {
    titles: ListingCache<MangaSummary, TitleListing>,
    songs: ListingCache<OnlineTrack, ChartRef>,
}

impl Default for Listings {
    fn default() -> Self {
        Self {
            titles: ListingCache::new(|item: &MangaSummary| item.id.clone()),
            songs: ListingCache::new(song_id),
        }
    }
}

fn song_id(track: &OnlineTrack) -> String {
    raygun_resolver::track_key(&TrackQuery {
        title: track.title.clone(),
        artist: track.artist.clone(),
        album: None,
        duration_ms: None,
    })
}

impl ExtensionHost {
    /// The titles of a comic, book or video source's listings that match `query`, best first,
    /// for a search of it that failed or found nothing: its popular and latest titles, then
    /// blank searches by its own sorts. `video` weighs seasons as Search does. Reads at most
    /// what `prefs` allows before `deadline`; fails only when nothing could be read.
    pub async fn listing_titles(
        &self,
        provider: &str,
        query: &str,
        video: bool,
        prefs: SearchListingsPrefs,
        deadline: Instant,
    ) -> Result<Vec<MangaSummary>, String> {
        // A source waiting for its key isn't asked for its listings either.
        if self.provider_needs_setup(provider) {
            return Ok(Vec::new());
        }
        let wanted = Wanted::new(query);
        self.listings()
            .titles
            .matches(
                provider,
                Bounds::new(prefs, deadline),
                |item: &MangaSummary| wanted.score(&item.title, video),
                vec![TitleListing::Popular, TitleListing::Latest],
                || async move {
                    // A source whose filters can't be listed has no sorts to read.
                    sort_listings(&self.manga_filters(provider).await.unwrap_or_default())
                },
                |listing, page| async move {
                    let answer = match listing {
                        TitleListing::Popular => {
                            self.manga_list(provider, MangaList::Popular, page).await
                        }
                        TitleListing::Latest => {
                            self.manga_list(provider, MangaList::Latest, page).await
                        }
                        TitleListing::Sorted { key, value } => {
                            let filters = BTreeMap::from([(key, FilterValue::Select(value))]);
                            self.manga_search_filtered(provider, "", page, &filters)
                                .await
                        }
                    };
                    answer.map(Page::from)
                },
            )
            .await
    }

    /// The songs of the charts of a music source's extension that match `query`, best first,
    /// for a search of it (a `music.metadata` provider) that failed or found nothing: the main
    /// charts first, a chart a page. Nothing when the extension keeps no charts. Reads at most
    /// what `prefs` allows before `deadline`; fails only when nothing could be read.
    pub async fn listing_songs(
        &self,
        provider: &ProviderInfo,
        query: &str,
        prefs: SearchListingsPrefs,
        deadline: Instant,
    ) -> Result<Vec<OnlineTrack>, String> {
        let keepers: Vec<ProviderInfo> = self
            .providers(crate::MUSIC_CHARTS)
            .await
            .into_iter()
            .filter(|charts| charts.extension_id == provider.extension_id)
            .collect();
        if keepers.is_empty() {
            return Ok(Vec::new());
        }
        let wanted = raygun_resolver::normalize(query);
        self.listings()
            .songs
            .matches(
                &provider.id,
                Bounds::new(prefs, deadline),
                |track: &OnlineTrack| song_score(track, &wanted),
                Vec::new(),
                || async move {
                    let mut charts = Vec::new();
                    for keeper in &keepers {
                        let args = serde_json::json!({ "region": null });
                        match self.call(&keeper.id, "charts", args).await {
                            Ok(value) => charts.extend(crate::host::parse_charts(keeper, &value)),
                            Err(error) => {
                                tracing::debug!(provider = %keeper.id, %error, "no charts to search");
                            }
                        }
                    }
                    // The main charts first, genre and mood ones after; each source's order
                    // among equals.
                    charts.sort_by_key(|chart| chart.group);
                    charts
                        .into_iter()
                        .map(|chart| ChartRef {
                            provider: chart.provider.id,
                            id: chart.id,
                        })
                        .collect()
                },
                |chart: ChartRef, _page| async move {
                    self.music_chart(&chart.provider, &chart.id, None, CHART_SONGS)
                        .await
                        .map(|items| Page {
                            items,
                            has_next: false,
                        })
                        .map_err(|error| error.to_string())
                },
            )
            .await
    }
}

/// How well a song answers a query (normalized): its title alone, or with its artist before or
/// after, as people type them.
fn song_score(track: &OnlineTrack, wanted: &str) -> u32 {
    let artist = track.artist.as_deref().unwrap_or_default();
    [
        title_score(&track.title, wanted),
        title_score(&format!("{artist} {}", track.title), wanted),
        title_score(&format!("{} {artist}", track.title), wanted),
    ]
    .into_iter()
    .max()
    .unwrap_or(0)
}

/// What a title search asked for.
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

    fn score(&self, title: &str, video: bool) -> u32 {
        if video {
            self.video_score(title)
        } else {
            title_score(title, &self.simple)
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

/// Blank searches sorted by a source's own sort or order filters, the most useful for finding
/// titles first (trending, then popularity, views and ratings, then the newest), at most
/// [`SORTS`]. What a filter sorts by unasked (its default, else its first option: what the
/// source lists anyway) and sorts by name are left out.
pub fn sort_listings(filters: &[SourceFilter]) -> Vec<TitleListing> {
    let mut found: Vec<(u8, usize, TitleListing)> = Vec::new();
    for filter in filters {
        let SourceFilter::Select {
            key,
            label,
            options,
            default,
        } = filter
        else {
            continue;
        };
        if !names_sort(key) && !names_sort(label) {
            continue;
        }
        let unasked = default
            .as_deref()
            .or(options.first().map(|option| option.value.as_str()));
        for (position, option) in options.iter().enumerate() {
            if Some(option.value.as_str()) == unasked {
                continue;
            }
            if let Some(rank) = sort_rank(&option.label).or_else(|| sort_rank(&option.value)) {
                found.push((
                    rank,
                    position,
                    TitleListing::Sorted {
                        key: key.clone(),
                        value: option.value.clone(),
                    },
                ));
            }
        }
    }
    found.sort_by_key(|(rank, position, _)| (*rank, *position));
    found
        .into_iter()
        .take(SORTS)
        .map(|(_, _, listing)| listing)
        .collect()
}

/// Whether a filter's key or label names a sort ("Sort by", "orderBy", "Ordenar", "Ordre"...).
fn names_sort(text: &str) -> bool {
    let text = text.to_lowercase();
    ["sort", "order", "orden", "ordre", "ordin"]
        .iter()
        .any(|word| text.contains(word))
}

/// How useful sorting by an option is for finding titles: trending (0), popular (1), newest (2);
/// `None` for the rest (by name, by relevance, a direction).
fn sort_rank(text: &str) -> Option<u8> {
    const RANKS: [(u8, &[&str]); 3] = [
        (0, &["trend", "hot", "rising"]),
        (
            1,
            &[
                "popular", "view", "rating", "rated", "score", "top", "follow", "favorit",
                "favourit", "bookmark", "like",
            ],
        ),
        (
            2,
            &[
                "latest", "newest", "new", "update", "recent", "added", "release",
            ],
        ),
    ];
    let text = raygun_resolver::normalize(text);
    let words: Vec<&str> = text.split(' ').collect();
    RANKS
        .iter()
        .find(|(_, stems)| {
            words
                .iter()
                .any(|word| stems.iter().any(|stem| word.starts_with(stem)))
        })
        .map(|(rank, _)| *rank)
}

#[cfg(test)]
mod tests;
