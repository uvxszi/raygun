//! The first-party comics sources against their real sites (`comics_live`): lists, search with
//! filters, a title's details and chapters, and a chapter's pages, fetched the way the app's image
//! proxy fetches them (with the headers each page carries).
//!
//! `cargo test -p raygun-ext-host --test comics_live -- --ignored --nocapture`, or one source with
//! its name after `comics_live_`.

use std::collections::BTreeMap;
use std::path::Path;

use raygun_ext_host::manga::{MANGA_SOURCE, MangaChapter, MangaList, MangaListing, PageImage};
use raygun_ext_host::{ExtensionHost, HostConfig, UserSettings, discover_installed};
use raygun_model::{ExtensionStatus, FilterValue};
use serde_json::json;

const MANGADEX: &str = "dev.raygun.mangadex/mangadex";
const WEBTOON: &str = "dev.raygun.webtoons/webtoons";
const TAPAS: &str = "dev.raygun.tapas/tapas";
const XKCD: &str = "dev.raygun.xkcd/xkcd";
const PEPPER_CARROT: &str = "dev.raygun.peppercarrot/peppercarrot";
const ARCHIVE: &str = "dev.raygun.archive-comics/comics";
const CREATORS: &str = "dev.raygun.mangaplus-creators/creators";
const NAVER: &str = "dev.raygun.naver-webtoon/naver";

/// Every first-party extension, with `settings` for some, all loaded.
async fn host_with(settings: UserSettings) -> ExtensionHost {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load_with_settings(discover_installed(&dir), settings)
        .await;
    for info in &infos {
        assert_eq!(
            info.status,
            ExtensionStatus::Loaded,
            "{}: {:?}",
            info.id,
            info.error
        );
    }
    host
}

async fn host() -> ExtensionHost {
    host_with(UserSettings::new()).await
}

fn settings(extension: &str, values: serde_json::Value) -> UserSettings {
    let values: BTreeMap<String, serde_json::Value> = serde_json::from_value(values).unwrap();
    UserSettings::from([(extension.to_owned(), values)])
}

fn show(label: &str, listing: &MangaListing) {
    println!(
        "{label}: {} titles (more: {}), first {:?}",
        listing.items.len(),
        listing.has_next_page,
        listing
            .items
            .iter()
            .take(4)
            .map(|item| (&item.id, &item.title))
            .collect::<Vec<_>>()
    );
}

fn assert_listing(label: &str, listing: &MangaListing) {
    show(label, listing);
    assert!(!listing.items.is_empty(), "{label} is empty");
    let mut ids: Vec<&str> = listing.items.iter().map(|item| item.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), listing.items.len(), "{label} repeats a title");
    assert!(
        listing.items.iter().any(|item| item.cover_url.is_some()),
        "{label} has no covers"
    );
}

/// Fetches pictures as the image proxy does, with the headers the source gave.
async fn assert_images(
    label: &str,
    urls: impl IntoIterator<Item = (String, BTreeMap<String, String>)>,
) {
    let client = raygun_net::http_client().unwrap();
    for (url, headers) in urls {
        let image = raygun_net::images::fetch_image(&client, &url, &headers)
            .await
            .unwrap_or_else(|error| panic!("{label}: {url} failed: {error}"));
        println!(
            "{label}: {url} → {} bytes .{}",
            image.bytes.len(),
            image.extension
        );
        // A picture, whatever its size: episodes end with small spacers and credits strips.
        assert_ne!(
            image.extension, "img",
            "{label}: {url} isn't a known picture type"
        );
    }
}

/// A title's details, chapters and the pages of one chapter (`pick` chooses it), checked.
async fn read_title(
    host: &ExtensionHost,
    provider: &str,
    id: &str,
    pick: impl Fn(&[MangaChapter]) -> Option<&MangaChapter>,
) -> (Vec<MangaChapter>, Vec<PageImage>) {
    let details = host.manga_details(provider, id).await.unwrap();
    println!(
        "details: {:?} by {:?}, {:?}, {} genres, cover {:?}, url {:?}",
        details.title,
        details.authors,
        details.status,
        details.genres.len(),
        details.cover_url,
        details.url
    );
    assert!(!details.title.is_empty() && details.title != "Untitled");
    assert!(details.url.is_some());
    if let Some(cover) = &details.cover_url {
        assert_images("cover", [(cover.clone(), details.cover_headers.clone())]).await;
    }
    let chapters = host.manga_chapters(provider, id).await.unwrap();
    println!(
        "{} chapters, first {:?}, last {:?}",
        chapters.len(),
        chapters
            .first()
            .map(|chapter| (&chapter.name, chapter.number)),
        chapters
            .last()
            .map(|chapter| (&chapter.name, chapter.number)),
    );
    let chapter = pick(&chapters).expect("a chapter to read");
    let pages = host.manga_pages(provider, &chapter.id).await.unwrap();
    println!("{:?}: {} pages", chapter.name, pages.len());
    // The first and last page, which load lazily in the reader.
    let ends = [pages.first(), pages.last()]
        .into_iter()
        .flatten()
        .map(|page| (page.url.clone(), page.headers.clone()));
    assert_images("page", ends).await;
    (chapters, pages)
}

fn select(value: &str) -> FilterValue {
    FilterValue::Select(value.to_owned())
}

#[tokio::test]
#[ignore = "needs network access to api.mangadex.org"]
async fn comics_live_mangadex() {
    let host = host().await;
    assert!(
        host.providers(MANGA_SOURCE)
            .await
            .iter()
            .any(|provider| provider.id == MANGADEX)
    );
    let popular = host
        .manga_list(MANGADEX, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("popular", &popular);
    assert!(popular.has_next_page);
    let latest = host
        .manga_list(MANGADEX, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("latest (chapters in English)", &latest);
    let second = host
        .manga_list(MANGADEX, MangaList::Latest, 2)
        .await
        .unwrap();
    show("latest page 2", &second);
    assert!(
        second
            .items
            .iter()
            .all(|item| latest.items.iter().all(|first| first.id != item.id)),
        "page 2 repeats page 1's series"
    );
    // Past MangaDex's 10,000-result window there's nothing more, rather than an error.
    let far = host
        .manga_list(MANGADEX, MangaList::Popular, 500)
        .await
        .unwrap();
    assert!(far.items.is_empty() && !far.has_next_page);

    let filters = host.manga_filters(MANGADEX).await.unwrap();
    assert!(
        filters.len() >= 5,
        "sort, status, demographic, origin, tags"
    );
    let manhwa = host
        .manga_search_filtered(
            MANGADEX,
            "",
            1,
            &BTreeMap::from([
                ("origin".to_owned(), select("ko")),
                ("sort".to_owned(), select("relevance")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("Korean originals, best match without words", &manhwa);

    let found = host.manga_search(MANGADEX, "frieren", 1).await.unwrap();
    assert_listing("search frieren", &found);
    assert!(
        found
            .items
            .iter()
            .all(|item| item.title.to_lowercase().contains("frieren")),
        "every match names it"
    );
    // A series with a chapter out this week: licensed series often lost theirs to takedowns.
    let recent = &latest.items[0];
    let (chapters, _) = read_title(&host, MANGADEX, &recent.id, |chapters| chapters.first()).await;
    assert!(chapters.iter().all(|chapter| !chapter.id.is_empty()));
    // Newest first, as the feed's volume and chapter order gives them.
    let numbers: Vec<f64> = chapters
        .iter()
        .filter_map(|chapter| chapter.number)
        .collect();
    println!(
        "chapter numbers from the top: {:?}",
        &numbers[..numbers.len().min(8)]
    );
}

#[tokio::test]
#[ignore = "needs network access to api.mangadex.org"]
async fn comics_live_mangadex_languages() {
    // The setting once held the code itself; both spellings read Spanish (Latin America).
    for value in ["Spanish (Latin America)", "es-la"] {
        let host = host_with(settings(
            "dev.raygun.mangadex",
            json!({ "language": value }),
        ))
        .await;
        let latest = host
            .manga_list(MANGADEX, MangaList::Latest, 1)
            .await
            .unwrap();
        assert_listing(value, &latest);
        let chapters = host
            .manga_chapters(MANGADEX, &latest.items[0].id)
            .await
            .unwrap();
        println!("{value}: {} chapters", chapters.len());
        assert!(
            !chapters.is_empty(),
            "a series just updated in Spanish has chapters in it"
        );
    }
}

#[tokio::test]
#[ignore = "needs network access to webtoons.com"]
async fn comics_live_webtoon() {
    let host = host().await;
    let popular = host
        .manga_list(WEBTOON, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("rankings", &popular);
    assert!(
        popular
            .items
            .iter()
            .all(|item| item.cover_headers.contains_key("Referer"))
    );
    let latest = host
        .manga_list(WEBTOON, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("today's Originals", &latest);

    let filters = host.manga_filters(WEBTOON).await.unwrap();
    assert_eq!(filters.len(), 3);
    let search = |query: &'static str, values: Vec<(&'static str, &'static str)>, page: u32| {
        let host = host.clone();
        async move {
            let chosen = values
                .into_iter()
                .map(|(key, value)| (key.to_owned(), select(value)))
                .collect::<BTreeMap<_, _>>();
            host.manga_search_filtered(WEBTOON, query, page, &chosen)
                .await
                .unwrap()
        }
    };
    let both = search("tower", vec![], 1).await;
    assert_listing("search tower", &both);
    assert!(both.items.iter().any(|item| item.id.contains(":webtoon:")));
    assert!(both.items.iter().any(|item| item.id.contains(":canvas:")));
    let canvas = search("tower", vec![("type", "canvas")], 2).await;
    assert_listing("Canvas search page 2", &canvas);
    let romance = search("", vec![("genre", "romance"), ("type", "webtoon")], 1).await;
    assert_listing("Originals romance", &romance);
    assert!(romance.has_next_page);
    let canvas_romance = search(
        "",
        vec![("genre", "romance"), ("type", "canvas"), ("sort", "UPDATE")],
        2,
    )
    .await;
    assert_listing("Canvas romance by date, page 2", &canvas_romance);

    let original = both
        .items
        .iter()
        .find(|item| item.id.contains(":webtoon:"))
        .unwrap();
    let (chapters, pages) =
        read_title(&host, WEBTOON, &original.id, |chapters| chapters.last()).await;
    assert_eq!(
        chapters.last().unwrap().number,
        Some(1.0),
        "the first episode is last"
    );
    assert!(
        pages
            .iter()
            .all(|page| page.headers.contains_key("Referer"))
    );
    let creator = canvas.items.first().unwrap();
    read_title(&host, WEBTOON, &creator.id, |chapters| chapters.first()).await;
}

#[tokio::test]
#[ignore = "needs network access to webtoons.com"]
async fn comics_live_webtoon_languages() {
    for language in [
        "Spanish",
        "French",
        "Indonesian",
        "Thai",
        "Chinese (Traditional)",
    ] {
        let host = host_with(settings(
            "dev.raygun.webtoons",
            json!({ "language": language }),
        ))
        .await;
        let popular = host
            .manga_list(WEBTOON, MangaList::Popular, 1)
            .await
            .unwrap();
        assert_listing(language, &popular);
        let id = &popular.items[0].id;
        assert!(!id.starts_with("en:"), "{language} lists {id}");
        let chapters = host.manga_chapters(WEBTOON, id).await.unwrap();
        assert!(!chapters.is_empty(), "{language}: {id} has no episodes");
    }
}

#[tokio::test]
#[ignore = "needs network access to tapas.io"]
async fn comics_live_tapas() {
    let host = host().await;
    let popular = host.manga_list(TAPAS, MangaList::Popular, 1).await.unwrap();
    assert_listing("ranking", &popular);
    assert!(popular.has_next_page);
    let latest = host.manga_list(TAPAS, MangaList::Latest, 2).await.unwrap();
    assert_listing("latest page 2", &latest);
    let found = host.manga_search(TAPAS, "solo", 1).await.unwrap();
    assert_listing("search solo", &found);
    assert!(
        found
            .items
            .iter()
            .all(|item| !item.title.contains("_h_i_g_h"))
    );

    let filters = host.manga_filters(TAPAS).await.unwrap();
    assert_eq!(filters.len(), 3);
    let free = host
        .manga_search_filtered(
            TAPAS,
            "",
            1,
            &BTreeMap::from([
                ("price".to_owned(), select("FREE")),
                ("genre".to_owned(), select("ROMANCE")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("free romance", &free);
    let (chapters, _) =
        read_title(&host, TAPAS, &free.items[0].id, |chapters| chapters.last()).await;
    assert!(chapters.len() > 1, "a free series lists its episodes");

    // A series that waits or charges for most episodes lists only the free ones.
    let paid = &popular.items[0];
    let chapters = host.manga_chapters(TAPAS, &paid.id).await.unwrap();
    println!("{}: {} free episodes", paid.title, chapters.len());
}

#[tokio::test]
#[ignore = "needs network access to xkcd.com"]
async fn comics_live_xkcd() {
    let host = host().await;
    let popular = host.manga_list(XKCD, MangaList::Popular, 1).await.unwrap();
    assert_eq!(popular.items.len(), 1);
    let found = host.manga_search(XKCD, "elevators", 1).await.unwrap();
    assert_eq!(found.items.len(), 1, "a comic's title finds the series");
    let none = host.manga_search(XKCD, "qqqzzzxxy", 1).await.unwrap();
    assert!(none.items.is_empty());
    let (chapters, _) = read_title(&host, XKCD, "xkcd", |chapters| chapters.first()).await;
    assert!(chapters.len() > 3000);
    assert_eq!(chapters.last().unwrap().number, Some(1.0));
    // An old comic, a large one that links to its full size, and an interactive one.
    for comic in ["1", "657"] {
        let pages = host.manga_pages(XKCD, comic).await.unwrap();
        println!("{comic}: {:?}", pages[0].url);
        assert_images(comic, [(pages[0].url.clone(), pages[0].headers.clone())]).await;
    }
    let interactive = host.manga_pages(XKCD, "1608").await.unwrap_err();
    assert!(interactive.contains("interactive"), "{interactive}");
}

#[tokio::test]
#[ignore = "needs network access to peppercarrot.com"]
async fn comics_live_pepper_carrot() {
    let host = host().await;
    let (chapters, pages) = read_title(&host, PEPPER_CARROT, "peppercarrot", |chapters| {
        chapters.last()
    })
    .await;
    assert!(chapters.len() >= 39);
    assert_eq!(chapters.last().unwrap().number, Some(1.0));
    assert!(chapters.iter().all(|chapter| chapter.date_upload.is_some()));
    assert!(pages.len() >= 5, "the title header and the pages");

    let host = host_with(settings(
        "dev.raygun.peppercarrot",
        json!({ "language": "French" }),
    ))
    .await;
    let chapters = host
        .manga_chapters(PEPPER_CARROT, "peppercarrot")
        .await
        .unwrap();
    println!(
        "in French: {:?}",
        chapters.first().map(|chapter| &chapter.name)
    );
    assert!(
        chapters[0].name.starts_with("Épisode"),
        "{}",
        chapters[0].name
    );
    let pages = host
        .manga_pages(PEPPER_CARROT, &chapters[0].id)
        .await
        .unwrap();
    assert!(pages[1].url.contains("/fr_Pepper-and-Carrot"));
    let ends = [&pages[0], pages.last().unwrap()]
        .into_iter()
        .map(|page| (page.url.clone(), page.headers.clone()));
    assert_images("French pages", ends).await;
}

#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn comics_live_internet_archive() {
    let host = host().await;
    let popular = host
        .manga_list(ARCHIVE, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("most read", &popular);
    let latest = host
        .manga_list(ARCHIVE, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("recently added", &latest);
    let found = host.manga_search(ARCHIVE, "corriere", 1).await.unwrap();
    assert_listing("search corriere", &found);
    let filters = host.manga_filters(ARCHIVE).await.unwrap();
    assert_eq!(filters.len(), 2);
    let italian = host
        .manga_search_filtered(
            ARCHIVE,
            "",
            1,
            &BTreeMap::from([
                ("language".to_owned(), select("ita")),
                ("sort".to_owned(), select("date asc")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("Italian, oldest first", &italian);
    let (_, pages) = read_title(&host, ARCHIVE, &found.items[0].id, |chapters| {
        chapters.first()
    })
    .await;
    assert!(pages.len() > 4);
}

#[tokio::test]
#[ignore = "needs network access to mangaplus-creators.jp"]
async fn comics_live_mangaplus_creators() {
    let host = host().await;
    let popular = host
        .manga_list(CREATORS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("popular this month", &popular);
    let latest = host
        .manga_list(CREATORS, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("latest episodes", &latest);
    let found = host.manga_search(CREATORS, "dragon", 1).await.unwrap();
    assert_listing("search dragon", &found);
    let fantasy = host
        .manga_search_filtered(
            CREATORS,
            "",
            1,
            &BTreeMap::from([
                ("genre".to_owned(), select("fantasy")),
                ("sort".to_owned(), select("like_desc")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("fantasy, most liked", &fantasy);
    let top = popular.items.first().unwrap();
    read_title(&host, CREATORS, &top.id, |chapters| chapters.last()).await;
    // A long series (Dragon Tofu has over 60 episodes): its episodes come 20 to a page.
    let long = found.items.first().unwrap();
    let chapters = host.manga_chapters(CREATORS, &long.id).await.unwrap();
    println!("{}: {} episodes", long.title, chapters.len());
    assert!(chapters.len() > 40, "every page of the episode list");
    let numbers: Vec<_> = chapters
        .iter()
        .filter_map(|chapter| chapter.number)
        .collect();
    assert!(
        numbers.windows(2).all(|pair| pair[0] >= pair[1]),
        "newest first"
    );
    let mut ids: Vec<_> = chapters.iter().map(|chapter| chapter.id.as_str()).collect();
    ids.sort_unstable();
    ids.dedup();
    assert_eq!(ids.len(), chapters.len(), "each episode once");

    let host = host_with(settings(
        "dev.raygun.mangaplus-creators",
        json!({ "language": "Spanish" }),
    ))
    .await;
    let spanish = host
        .manga_list(CREATORS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("popular in Spanish", &spanish);
}

#[tokio::test]
#[ignore = "needs network access to comic.naver.com"]
async fn comics_live_naver_webtoon() {
    let host = host().await;
    let popular = host.manga_list(NAVER, MangaList::Popular, 1).await.unwrap();
    assert_listing("weekly series by popularity", &popular);
    assert!(popular.has_next_page);
    assert!(
        popular
            .items
            .iter()
            .all(|item| item.cover_headers.contains_key("Referer"))
    );
    let latest = host.manga_list(NAVER, MangaList::Latest, 1).await.unwrap();
    assert_listing("today's series", &latest);
    // 신의 탑 (Tower of God).
    let found = host.manga_search(NAVER, "신의 탑", 1).await.unwrap();
    assert_listing("search", &found);
    let filters = host.manga_filters(NAVER).await.unwrap();
    assert_eq!(filters.len(), 3);
    let romance = host
        .manga_search_filtered(
            NAVER,
            "",
            2,
            &BTreeMap::from([
                ("genre".to_owned(), select("PURE")),
                ("sort".to_owned(), select("UPDATE")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("romance by latest episode, page 2", &romance);
    let finished = host
        .manga_search_filtered(
            NAVER,
            "",
            1,
            &BTreeMap::from([("status".to_owned(), select("finished"))]),
        )
        .await
        .unwrap();
    assert_listing("finished", &finished);

    let series = found
        .items
        .iter()
        .find(|item| !item.nsfw)
        .expect("a series for everyone");
    let (chapters, pages) = read_title(&host, NAVER, &series.id, |chapters| chapters.last()).await;
    assert!(chapters.len() > 100, "every page of a long series");
    assert_eq!(chapters.last().unwrap().number, Some(1.0));
    assert!(
        pages
            .iter()
            .all(|page| page.headers.contains_key("Referer"))
    );
}
