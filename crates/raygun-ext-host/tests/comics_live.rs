//! The first-party comics sources against their real sites (`comics_live`): lists, search with
//! filters, a title's details and chapters, and a chapter's pages, fetched the way the app's image
//! proxy fetches them (with the headers each page carries).
//!
//! `cargo test -p raygun-ext-host --test comics_live -- --ignored --nocapture`, or one source with
//! its name after `comics_live_`.

use std::collections::BTreeMap;
use std::path::Path;

use raygun_ext_host::manga::{MangaChapter, MangaList, MangaListing, PageImage};
use raygun_ext_host::{ExtensionHost, HostConfig, UserSettings, discover_installed};
use raygun_model::{ExtensionStatus, FilterValue};
use serde_json::json;

const XKCD: &str = "dev.raygun.xkcd/xkcd";
const PEPPER_CARROT: &str = "dev.raygun.peppercarrot/peppercarrot";
const ARCHIVE: &str = "dev.raygun.archive-comics/comics";

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

    // Every item listed is in the public domain by age, and marked public domain, openly
    // licensed or in the Archive's curated set, by its own record.
    let client = raygun_net::http_client().unwrap();
    let before = chrono_year() - 95;
    for (label, listing) in [
        ("most read", &popular),
        ("recently added", &latest),
        ("corriere", &found),
        ("Italian", &italian),
    ] {
        for item in listing.items.iter().take(10) {
            let body = client
                .get(format!("https://archive.org/metadata/{}/metadata", item.id))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            let record: serde_json::Value = serde_json::from_str(&body).unwrap();
            let meta = &record["result"];
            let year: i32 = meta["date"].as_str().unwrap_or_default()[..4]
                .parse()
                .unwrap();
            let licence = meta["licenseurl"].as_str().unwrap_or_default();
            let curated = meta["collection"].as_array().is_some_and(|names| {
                names
                    .iter()
                    .any(|name| name.as_str() == Some("corrieredelpiccoli"))
            });
            println!(
                "{label}: {} · {year} · {licence} · curated {curated}",
                item.id
            );
            assert!(year < before, "{} is from {year}", item.id);
            assert!(
                curated
                    || licence.contains("creativecommons.org")
                    || licence.contains("publicdomain"),
                "{} carries {licence:?}",
                item.id
            );
        }
    }
}

/// This year, from the clock (UTC), for the public-domain cut-off.
fn chrono_year() -> i32 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // Close enough for a cut-off that moves once a year: 365.2425 days a year since 1970.
    1970 + (seconds as f64 / 31_556_952.0) as i32
}
