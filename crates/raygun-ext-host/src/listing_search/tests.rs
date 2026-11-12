use std::sync::Mutex as StdMutex;

use raygun_model::FilterOption;
use serde_json::json;

use super::*;
use crate::{ExtensionSource, HostConfig};

fn item(id: usize, title: &str) -> MangaSummary {
    MangaSummary {
        id: id.to_string(),
        title: title.to_owned(),
        cover_url: None,
        cover_headers: Default::default(),
        authors: Vec::new(),
        nsfw: false,
        live: None,
        subtitle: None,
        folder: false,
    }
}

/// Page `page` of a listing named `name` with `pages` pages of 20 titles; one in each page names
/// "dragon".
fn page_of(name: &str, page: u32, pages: u32) -> Page<MangaSummary> {
    let start = (page as usize - 1) * 20;
    Page {
        items: (start..start + 20)
            .map(|at| {
                let id = format!("{name}{at}");
                let title = if at % 20 == 7 {
                    format!("The Dragon Story {name} {at}")
                } else {
                    format!("Title {name} {at}")
                };
                MangaSummary {
                    id,
                    ..item(0, &title)
                }
            })
            .collect(),
        has_next: page < pages,
    }
}

fn cache() -> ListingCache<MangaSummary, &'static str> {
    ListingCache::new(|item: &MangaSummary| item.id.clone())
}

fn bounds(pages: u32, entries: usize, seconds: u64) -> Bounds {
    Bounds {
        pages,
        entries,
        deadline: Instant::now() + Duration::from_secs(seconds),
    }
}

fn dragon(item: &MangaSummary) -> u32 {
    title_score(&item.title, "dragon story")
}

/// Every page asked, in order, as `listing:page`.
type Asked = Arc<StdMutex<Vec<String>>>;

fn fetcher(
    asked: &Asked,
    pages: u32,
) -> impl FnMut(&'static str, u32) -> std::future::Ready<Result<Page<MangaSummary>, String>> {
    let asked = asked.clone();
    move |listing, page| {
        asked.lock().unwrap().push(format!("{listing}:{page}"));
        std::future::ready(Ok(page_of(listing, page, pages)))
    }
}

fn nothing_more() -> std::future::Ready<Vec<&'static str>> {
    std::future::ready(Vec::new())
}

#[tokio::test]
async fn listings_are_read_in_turn_up_to_the_pages_allowed() {
    let listings = cache();
    let asked = Asked::default();
    let found = listings
        .matches(
            "a",
            bounds(5, 100, 10),
            dragon,
            vec!["popular", "latest"],
            nothing_more,
            fetcher(&asked, 50),
        )
        .await
        .unwrap();
    // A page of each in turn, five in all: one match on each.
    assert_eq!(
        *asked.lock().unwrap(),
        [
            "popular:1",
            "latest:1",
            "popular:2",
            "latest:2",
            "popular:3"
        ]
    );
    assert_eq!(found.len(), 5);
    assert!(
        found
            .iter()
            .all(|item| item.title.starts_with("The Dragon Story"))
    );

    // Typing on reuses what was read: nothing is asked again within the pages allowed.
    let again = listings
        .matches(
            "a",
            bounds(5, 100, 10),
            |item: &MangaSummary| title_score(&item.title, "dragon"),
            vec!["popular", "latest"],
            nothing_more,
            fetcher(&asked, 50),
        )
        .await
        .unwrap();
    assert_eq!(asked.lock().unwrap().len(), 5);
    assert_eq!(again.len(), 5);
    // More pages allowed (the setting changed): reading goes on where it stopped.
    listings
        .matches(
            "a",
            bounds(6, 100, 10),
            dragon,
            vec!["popular", "latest"],
            nothing_more,
            fetcher(&asked, 50),
        )
        .await
        .unwrap();
    assert_eq!(asked.lock().unwrap().last().unwrap(), "latest:3");
}

#[tokio::test]
async fn reading_stops_once_enough_match_and_gives_that_many() {
    let listings = cache();
    let asked = Asked::default();
    let found = listings
        .matches(
            "a",
            bounds(20, 2, 10),
            dragon,
            vec!["popular"],
            nothing_more,
            fetcher(&asked, 50),
        )
        .await
        .unwrap();
    assert_eq!(*asked.lock().unwrap(), ["popular:1", "popular:2"]);
    assert_eq!(found.len(), 2);

    // Best first: an exact title before titles that only hold the words.
    let mut exact = page_of("x", 1, 1);
    exact.items.push(item(99, "Dragon Story"));
    let ranked = cache()
        .matches(
            "b",
            bounds(5, 1, 10),
            dragon,
            vec!["x"],
            nothing_more,
            move |_, _| std::future::ready(Ok(exact.clone())),
        )
        .await
        .unwrap();
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].title, "Dragon Story");
}

#[tokio::test]
async fn more_listings_are_found_once_each_first_one_had_a_page() {
    let listings = cache();
    let asked = Asked::default();
    let discovered = Asked::default();
    let found = {
        let discovered = discovered.clone();
        let asked_then = asked.clone();
        listings
            .matches(
                "a",
                bounds(5, 100, 10),
                dragon,
                vec!["popular", "latest"],
                move || {
                    discovered
                        .lock()
                        .unwrap()
                        .push(asked_then.lock().unwrap().join(","));
                    std::future::ready(vec!["trending", "views"])
                },
                fetcher(&asked, 50),
            )
            .await
            .unwrap()
    };
    // Sorts are looked for after the first pages of popular and latest, then read in turn.
    assert_eq!(*discovered.lock().unwrap(), ["popular:1,latest:1"]);
    assert_eq!(
        *asked.lock().unwrap(),
        [
            "popular:1",
            "latest:1",
            "trending:1",
            "views:1",
            "popular:2"
        ]
    );
    assert_eq!(found.len(), 5);

    // A source with no listings known at once (a music source's charts) looks for them first.
    let charts = cache();
    let asked = Asked::default();
    charts
        .matches(
            "m",
            bounds(2, 100, 10),
            dragon,
            Vec::new(),
            || std::future::ready(vec!["top", "new", "genre"]),
            fetcher(&asked, 1),
        )
        .await
        .unwrap();
    assert_eq!(*asked.lock().unwrap(), ["top:1", "new:1"]);
}

#[tokio::test]
async fn listings_end_at_their_last_page_or_a_repeated_one() {
    let listings = cache();
    let asked = Asked::default();
    // Popular has two pages; latest sends its first page whatever the number.
    let fetch = {
        let asked = asked.clone();
        move |listing: &'static str, page: u32| {
            asked.lock().unwrap().push(format!("{listing}:{page}"));
            let answer = if listing == "latest" {
                Page {
                    has_next: true,
                    ..page_of(listing, 1, 9)
                }
            } else {
                page_of(listing, page, 2)
            };
            std::future::ready(Ok(answer))
        }
    };
    let found = listings
        .matches(
            "a",
            bounds(20, 100, 10),
            dragon,
            vec!["popular", "latest"],
            nothing_more,
            fetch,
        )
        .await
        .unwrap();
    assert_eq!(
        *asked.lock().unwrap(),
        ["popular:1", "latest:1", "popular:2", "latest:2"]
    );
    assert_eq!(found.len(), 3);
}

#[tokio::test(start_paused = true)]
async fn reading_stops_at_the_deadline_and_reads_on_later() {
    let listings = cache();
    let slow = |listing: &'static str, page: u32| async move {
        tokio::time::sleep(Duration::from_millis(300)).await;
        Ok(page_of(listing, page, 50))
    };
    // Time for two pages.
    let soon = Bounds {
        deadline: Instant::now() + Duration::from_millis(750),
        ..bounds(5, 100, 0)
    };
    let found = listings
        .matches("a", soon, dragon, vec!["popular"], nothing_more, slow)
        .await
        .unwrap();
    assert_eq!(found.len(), 2);
    // The next search reads on from page 3.
    let found = listings
        .matches(
            "a",
            bounds(5, 100, 60),
            dragon,
            vec!["popular"],
            nothing_more,
            slow,
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 5);
}

#[tokio::test]
async fn a_failure_is_given_only_when_nothing_could_be_read() {
    let failing = |listing: &'static str, page: u32| {
        std::future::ready(if listing == "popular" {
            Err(format!("{listing} is down"))
        } else {
            Ok(page_of(listing, page, 1))
        })
    };
    // Popular fails, latest works: its matches, and no failure.
    let found = cache()
        .matches(
            "a",
            bounds(5, 100, 10),
            dragon,
            vec!["popular", "latest"],
            nothing_more,
            failing,
        )
        .await
        .unwrap();
    assert_eq!(found.len(), 1);
    // Failed pages don't count against the pages allowed.
    let asked = Asked::default();
    let fetch = {
        let asked = asked.clone();
        move |listing: &'static str, page: u32| {
            asked.lock().unwrap().push(format!("{listing}:{page}"));
            failing(listing, page)
        }
    };
    cache()
        .matches(
            "b",
            bounds(1, 100, 10),
            dragon,
            vec!["popular", "latest"],
            nothing_more,
            fetch,
        )
        .await
        .unwrap();
    assert_eq!(*asked.lock().unwrap(), ["popular:1", "latest:1"]);

    let listings = cache();
    let down = listings
        .matches(
            "c",
            bounds(5, 100, 10),
            dragon,
            vec!["popular"],
            nothing_more,
            |_, _| std::future::ready(Err::<Page<MangaSummary>, _>("no listing".to_owned())),
        )
        .await;
    assert_eq!(down, Err("no listing".to_owned()));
}

#[test]
fn video_matches_weigh_seasons() {
    let items = [
        item(1, "Overlord II"),
        item(2, "Overlord"),
        item(3, "Overlord III"),
        item(4, "Other Show"),
    ];
    let ids = |query: &str, video: bool| {
        let wanted = Wanted::new(query);
        best(
            &items,
            &|item: &MangaSummary| wanted.score(&item.title, video),
            30,
        )
        .into_iter()
        .map(|item| item.id)
        .collect::<Vec<_>>()
    };
    // No season asked: the first season first, the others after.
    assert_eq!(ids("overlord", true), ["2", "1", "3"]);
    // A season asked: only that season.
    assert_eq!(ids("Overlord Season 2", true), ["1"]);
    // Plain titles don't know seasons.
    assert_eq!(ids("overlord", false), ["2", "1", "3"]);
}

#[test]
fn songs_match_by_title_with_or_without_their_artist() {
    let song = |title: &str, artist: &str| OnlineTrack {
        external_id: None,
        title: title.into(),
        artist: Some(artist.into()),
        album: None,
        duration_ms: None,
        artwork_url: None,
        source: None,
        explicit: false,
    };
    let wanted = raygun_resolver::normalize("daft punk get lucky");
    assert_eq!(song_score(&song("Get Lucky", "Daft Punk"), &wanted), 100);
    assert_eq!(
        song_score(&song("Get Lucky", "Daft Punk"), "get lucky daft punk"),
        100
    );
    assert_eq!(
        song_score(&song("Get Lucky", "Daft Punk"), "get lucky"),
        100
    );
    assert!(song_score(&song("Lose Yourself", "Eminem"), &wanted) < MATCH_SCORE);
}

fn option(value: &str, label: &str) -> FilterOption {
    FilterOption {
        value: value.into(),
        label: label.into(),
    }
}

#[test]
fn sorts_come_from_sort_filters_most_useful_first() {
    let filters = [
        SourceFilter::Select {
            key: "status".into(),
            label: "Status".into(),
            options: vec![option("hot", "Hot"), option("any", "Any")],
            default: None,
        },
        SourceFilter::Select {
            key: "sort".into(),
            label: "Sort by".into(),
            options: vec![
                option("relevance", "Relevance"),
                option("name", "Name (A-Z)"),
                option("latest", "Latest update"),
                option("views", "Most viewed"),
                option("trending", "Trending"),
            ],
            default: None,
        },
        SourceFilter::Toggle {
            key: "order".into(),
            label: "Ascending".into(),
            default: false,
        },
    ];
    // Trending, then views; the status filter isn't a sort, and relevance (the first option,
    // what it sorts by unasked) and names are left out.
    assert_eq!(
        sort_listings(&filters),
        [
            TitleListing::Sorted {
                key: "sort".into(),
                value: "trending".into()
            },
            TitleListing::Sorted {
                key: "sort".into(),
                value: "views".into()
            },
        ]
    );
    // A default is what the source lists anyway; options known by their value alone count too.
    let ordered = [SourceFilter::Select {
        key: "orderBy".into(),
        label: "Ordenar por".into(),
        options: vec![
            option("popular", "1"),
            option("new", "2"),
            option("az", "3"),
        ],
        default: Some("popular".into()),
    }];
    assert_eq!(
        sort_listings(&ordered),
        [TitleListing::Sorted {
            key: "orderBy".into(),
            value: "new".into()
        }]
    );
    assert!(sort_listings(&[]).is_empty());
}

// ---- Through the host, with sources whose search is broken ----

fn source(id: &str, provides: &[&str], code: &str) -> ExtensionSource {
    let manifest = json!({
        "id": id,
        "name": id,
        "version": "1.0.0",
        "apiVersion": 1,
        "runtime": "js",
        "entry": "index.js",
        "provides": provides,
        "permissions": { "net": ["127.0.0.1"] },
    })
    .to_string();
    ExtensionSource::built_in(&[("raygun-extension.json", &manifest), ("index.js", code)])
}

/// A comic source whose search always fails. Popular has three pages, latest one, and a blank
/// search sorted by `trending` one; each page holds one "Dragon" title. Every call is counted
/// in `calls` (read by `calls()`).
const BROKEN_COMICS: &str = r#"
    globalThis.calls = [];
    const page = (name, page, more) => ({
      items: [
        { id: `${name}-${page}-a`, title: `Dragon Tale ${name} ${page}` },
        { id: `${name}-${page}-b`, title: `Something Else ${name} ${page}` },
      ],
      hasNextPage: more,
    });
    raygun.providers.register({
      kind: 'manga.source', id: 'comics', name: 'Broken Comics',
      popular: async ({ page: number }) => {
        calls.push(`popular:${number}`);
        return page('popular', number, number < 3);
      },
      latest: async ({ page: number }) => {
        calls.push(`latest:${number}`);
        return page('latest', number, false);
      },
      search: async ({ query, page: number, filters }) => {
        if (query) { calls.push('search'); throw new Error('search is broken'); }
        calls.push(`sorted:${filters.sort}:${number}`);
        return page(filters.sort, number, false);
      },
      filters: async () => [
        { kind: 'select', key: 'sort', label: 'Sort', default: 'popular', options: [
          { value: 'popular', label: 'Popular' },
          { value: 'trending', label: 'Trending' },
          { value: 'title', label: 'Title' },
        ] },
      ],
      calls: async () => ({ calls }),
    });
"#;

async fn host_with(sources: Vec<ExtensionSource>) -> ExtensionHost {
    let host = ExtensionHost::start(HostConfig {
        call_timeout: Duration::from_secs(10),
        ..HostConfig::default()
    })
    .unwrap();
    let infos = host.load(sources).await;
    for info in &infos {
        assert_eq!(
            info.status,
            raygun_model::ExtensionStatus::Loaded,
            "{:?}",
            info.error
        );
    }
    host
}

async fn calls(host: &ExtensionHost, provider: &str) -> Vec<String> {
    let value = host.call(provider, "calls", json!({})).await.unwrap();
    serde_json::from_value(value["calls"].clone()).unwrap()
}

fn prefs(entries: u32, pages: u32) -> SearchListingsPrefs {
    SearchListingsPrefs {
        enabled: true,
        entries,
        pages,
    }
}

fn later(seconds: u64) -> Instant {
    Instant::now() + Duration::from_secs(seconds)
}

#[tokio::test]
async fn a_source_whose_search_fails_is_searched_through_its_listings() {
    let host = host_with(vec![source(
        "dev.raygun.broken",
        &["manga.source"],
        BROKEN_COMICS,
    )])
    .await;
    let provider = "dev.raygun.broken/comics";
    assert!(host.catalog_search_row(provider, "dragon").await.is_err());

    // Five pages: popular's and latest's first, then the trending sort's (the default sort and
    // the sort by title are left out), then popular's second and third.
    let found = host
        .listing_titles(provider, "dragon", false, prefs(20, 5), later(10))
        .await
        .unwrap();
    let titles: Vec<&str> = found.iter().map(|item| item.title.as_str()).collect();
    assert_eq!(
        titles,
        [
            "Dragon Tale popular 1",
            "Dragon Tale latest 1",
            "Dragon Tale trending 1",
            "Dragon Tale popular 2",
            "Dragon Tale popular 3",
        ]
    );
    assert_eq!(
        calls(&host, provider).await,
        [
            "search",
            "popular:1",
            "latest:1",
            "sorted:trending:1",
            "popular:2",
            "popular:3"
        ]
    );

    // Everything was read: another query reads nothing more, and nothing matches.
    let none = host
        .listing_titles(provider, "unknown words", false, prefs(20, 5), later(10))
        .await
        .unwrap();
    assert!(none.is_empty());
    assert_eq!(calls(&host, provider).await.len(), 6);
}

#[tokio::test]
async fn listings_are_read_no_further_than_the_entries_and_pages_allowed() {
    let host = host_with(vec![source(
        "dev.raygun.broken",
        &["manga.source"],
        BROKEN_COMICS,
    )])
    .await;
    let provider = "dev.raygun.broken/comics";
    // Two matches are enough: popular's and latest's first pages.
    let found = host
        .listing_titles(provider, "dragon tale", false, prefs(2, 5), later(10))
        .await
        .unwrap();
    assert_eq!(found.len(), 2);
    assert_eq!(calls(&host, provider).await, ["popular:1", "latest:1"]);

    // One more page allowed in all: the sorts are found and the first one read.
    let found = host
        .listing_titles(provider, "dragon tale", false, prefs(20, 3), later(10))
        .await
        .unwrap();
    assert_eq!(found.len(), 3);
    assert_eq!(
        calls(&host, provider).await,
        ["popular:1", "latest:1", "sorted:trending:1"]
    );
}

/// A music source whose song search fails, with charts beside it: Top holds the song, Genre
/// another.
const BROKEN_MUSIC: &str = r#"
    raygun.providers.register({
      kind: 'music.metadata', id: 'songs', name: 'Broken Songs',
      search: async () => { throw new Error('search is broken'); },
    });
    raygun.providers.register({
      kind: 'music.charts', id: 'charts', name: 'Broken Charts',
      charts: async () => ({ charts: [
        { id: 'jazz', name: 'Jazz', tag: 'Jazz' },
        { id: 'top', name: 'Top 50', group: 'top' },
      ] }),
      chart: async ({ id }) => ({ tracks: id === 'top'
        ? [{ title: 'Get Lucky', artist: 'Daft Punk' }, { title: 'Other Song', artist: 'Someone' }]
        : [{ title: 'So What', artist: 'Miles Davis' }] }),
    });
"#;

const GOOD_MUSIC: &str = r#"
    raygun.providers.register({
      kind: 'music.metadata', id: 'songs', name: 'Good Songs',
      search: async ({ query }) => ({ tracks: [{ title: query, artist: 'A' }] }),
    });
"#;

#[tokio::test]
async fn a_music_source_whose_search_fails_answers_from_its_charts() {
    let host = host_with(vec![
        source(
            "dev.raygun.brokenmusic",
            &["music.metadata", "music.charts"],
            BROKEN_MUSIC,
        ),
        source("dev.raygun.goodmusic", &["music.metadata"], GOOD_MUSIC),
    ])
    .await;
    let search = |listings: SearchListingsPrefs| {
        let host = &host;
        async move {
            let mut results = host
                .search_music_as_answered("daft punk get lucky", 10, listings, |_, _| {})
                .await;
            results.sort_by(|a, b| a.provider.name.cmp(&b.provider.name));
            results
        }
    };

    let results = search(prefs(20, 5)).await;
    let broken = &results[0];
    assert_eq!(broken.provider.name, "Broken Songs");
    assert!(broken.from_listings);
    assert_eq!(broken.error, None);
    let titles: Vec<&str> = broken
        .tracks
        .iter()
        .map(|track| track.title.as_str())
        .collect();
    assert_eq!(titles, ["Get Lucky"]);
    // A source whose search answered is left as it answered.
    let good = &results[1];
    assert!(!good.from_listings);
    assert_eq!(good.tracks[0].title, "daft punk get lucky");

    // Turned off, the failure stays.
    let off = search(SearchListingsPrefs {
        enabled: false,
        ..prefs(20, 5)
    })
    .await;
    assert!(!off[0].from_listings);
    assert!(
        off[0]
            .error
            .as_deref()
            .unwrap()
            .contains("search is broken")
    );
}

/// A Miru source whose search misses what it lists: iptv-org's search matches channel names
/// case by case, so a lower-case name finds nothing there, while its listing has the channel.
/// `cargo test -p raygun-ext-host -- --ignored listing_search_live --nocapture`; the script comes
/// from `MIRU_REPO`, else the reference/miru-repo checkout.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access and a miru-repo checkout"]
async fn listing_search_live() {
    let root = std::env::var_os("MIRU_REPO").map_or_else(
        || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/miru-repo"),
        std::path::PathBuf::from,
    );
    let index = std::fs::read_to_string(root.join("index.json")).unwrap();
    let index = crate::repo::parse_index(&index).unwrap();
    let entry = index
        .extensions
        .iter()
        .find(|entry| entry.id == "miru.iptv-org")
        .unwrap();
    let file = entry.package.split('#').next().unwrap_or_default();
    let bytes = std::fs::read(root.join(file)).unwrap();
    let (code, header) = crate::miru::package::verify_script(&bytes, entry).unwrap();
    let package = crate::miru::package::package(entry, code, &header).unwrap();
    let temp = tempfile::TempDir::new().unwrap();
    crate::package::install(temp.path(), &package).unwrap();
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(crate::discover_installed(temp.path())).await;
    let provider = infos[0].providers[0].id.clone();

    let listing = host
        .manga_list(&provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let wanted = listing
        .items
        .iter()
        .find(|item| item.title.to_lowercase() != item.title)
        .expect("a channel with capitals")
        .clone();
    let query = wanted.title.to_lowercase();
    let searched = host.catalog_search_row(&provider, &query).await.unwrap();
    eprintln!("{query:?}: the search found {}", searched.items.len());

    let found = host
        .listing_titles(
            &provider,
            &query,
            true,
            SearchListingsPrefs::default(),
            later(30),
        )
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
