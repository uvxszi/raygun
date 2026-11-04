//! The public-domain and openly licensed sources against their real services (`open_sources_live`):
//! lists, search with filters, details, chapters or episodes, and that what they hand out plays
//! (or reads) and carries an open licence.
//!
//! `cargo test -p raygun-ext-host --test open_sources_live -- --ignored --nocapture`, or one source
//! with its name after `open_sources_live_`.

use std::collections::BTreeMap;
use std::path::Path;

use raygun_ext_host::manga::{MangaList, MangaListing};
use raygun_ext_host::novel::{NOVEL_SOURCE, PassageBlock};
use raygun_ext_host::video::VIDEO_SOURCE;
use raygun_ext_host::{ExtensionHost, HostConfig, discover_installed};
use raygun_model::{ExtensionStatus, FilterValue, MediaSource, TrackQuery};

const STANDARD_EBOOKS: &str = "dev.raygun.standard-ebooks/books";
const WIKISOURCE: &str = "dev.raygun.wikisource/texts";
const COMMONS_VIDEOS: &str = "dev.raygun.wikimedia-commons/videos";
const NASA: &str = "dev.raygun.nasa/videos";
const BLENDER: &str = "dev.raygun.blender-movies/movies";
const ARCHIVE_FILMS: &str = "dev.raygun.internet-archive/films";

/// Every first-party extension, loaded.
async fn host() -> ExtensionHost {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    for info in host.load(discover_installed(&dir)).await {
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

fn select(value: &str) -> FilterValue {
    FilterValue::Select(value.to_owned())
}

fn assert_listing(label: &str, listing: &MangaListing) {
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

fn paragraphs(blocks: &[PassageBlock]) -> usize {
    blocks
        .iter()
        .filter(|block| matches!(block, PassageBlock::Paragraph(text) if text.len() > 40))
        .count()
}

/// Asks for the first bytes of a file the way a player does and checks it is served.
async fn assert_served(label: &str, url: &str, headers: &BTreeMap<String, String>) {
    let client = raygun_net::http_client().unwrap();
    let mut request = client.get(url).header("Range", "bytes=0-1023");
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = request
        .send()
        .await
        .unwrap_or_else(|error| panic!("{label}: {url} failed: {error}"));
    let status = response.status();
    let kind = response
        .headers()
        .get("content-type")
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    println!("{label}: {url} → {status} {kind}");
    assert!(status.is_success(), "{label}: {url} answered {status}");
    assert!(
        !kind.starts_with("text/html"),
        "{label}: {url} is a web page"
    );
}

#[tokio::test]
#[ignore = "needs network access to standardebooks.org"]
async fn open_sources_live_standard_ebooks() {
    let host = host().await;
    assert!(
        host.providers(NOVEL_SOURCE)
            .await
            .iter()
            .any(|provider| provider.id == STANDARD_EBOOKS)
    );
    let popular = host
        .manga_list(STANDARD_EBOOKS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("most popular", &popular);
    assert!(popular.has_next_page);
    let second = host
        .manga_list(STANDARD_EBOOKS, MangaList::Popular, 2)
        .await
        .unwrap();
    assert_listing("most popular, page 2", &second);
    assert!(
        second
            .items
            .iter()
            .all(|item| popular.items.iter().all(|first| first.id != item.id)),
        "page 2 repeats page 1"
    );
    let latest = host
        .manga_list(STANDARD_EBOOKS, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("newest editions", &latest);
    let found = host
        .manga_search(STANDARD_EBOOKS, "dickens", 1)
        .await
        .unwrap();
    assert_listing("search dickens", &found);
    assert!(
        found
            .items
            .iter()
            .all(|item| item.id.starts_with("charles-dickens/"))
    );
    assert_eq!(host.manga_filters(STANDARD_EBOOKS).await.unwrap().len(), 2);
    let poetry = host
        .manga_search_filtered(
            STANDARD_EBOOKS,
            "",
            1,
            &BTreeMap::from([
                ("subject".to_owned(), select("poetry")),
                ("sort".to_owned(), select("newest")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("poetry, newest", &poetry);

    let details = host
        .manga_details(STANDARD_EBOOKS, "charles-dickens/barnaby-rudge")
        .await
        .unwrap();
    println!(
        "details: {:?} by {:?}, {:?}, cover {:?}",
        details.title, details.authors, details.genres, details.cover_url
    );
    assert_eq!(details.title, "Barnaby Rudge");
    assert_eq!(details.authors, ["Charles Dickens"]);
    assert!(details.description.is_some_and(|text| text.len() > 200));
    let chapters = host
        .manga_chapters(STANDARD_EBOOKS, "charles-dickens/barnaby-rudge")
        .await
        .unwrap();
    println!(
        "{} chapters: {:?} … {:?}",
        chapters.len(),
        chapters.first().map(|chapter| &chapter.name),
        chapters.last().map(|chapter| &chapter.name)
    );
    assert!(chapters.len() >= 80, "the preface and 82 chapters");
    assert!(
        chapters
            .iter()
            .all(|chapter| !chapter.name.contains("Titlepage"))
    );
    let text = host
        .novel_passage(STANDARD_EBOOKS, &chapters[1].id)
        .await
        .unwrap();
    println!(
        "{:?}: {} blocks, first {:?}",
        chapters[1].name,
        text.len(),
        text.first()
    );
    assert!(paragraphs(&text) > 10);

    // Parts within books: each chapter is named with them.
    let war = host
        .manga_chapters(
            STANDARD_EBOOKS,
            "leo-tolstoy/war-and-peace/louise-maude_aylmer-maude",
        )
        .await
        .unwrap();
    println!(
        "War and Peace: {} chapters, first {:?}",
        war.len(),
        war.iter()
            .take(3)
            .map(|chapter| &chapter.name)
            .collect::<Vec<_>>()
    );
    assert!(war.len() > 300);
    assert!(war.iter().any(|chapter| chapter.name.contains(" · ")));
}

#[tokio::test]
#[ignore = "needs network access to en.wikisource.org"]
async fn open_sources_live_wikisource() {
    let host = host().await;
    let popular = host
        .manga_list(WIKISOURCE, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("featured texts", &popular);
    let latest = host
        .manga_list(WIKISOURCE, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("newly validated literature", &latest);
    let found = host
        .manga_search(WIKISOURCE, "sherlock holmes", 1)
        .await
        .unwrap();
    assert_listing("search sherlock holmes", &found);
    assert!(
        found.items.iter().all(|item| !item.id.contains('/')),
        "works, not chapters"
    );
    let poems = host
        .manga_search_filtered(
            WIKISOURCE,
            "",
            1,
            &BTreeMap::from([("genre".to_owned(), select("Collections of poetry"))]),
        )
        .await
        .unwrap();
    assert_listing("poetry collections", &poems);

    let id = "Pride and Prejudice (1817)";
    let details = host.manga_details(WIKISOURCE, id).await.unwrap();
    println!(
        "details: {:?} by {:?}, {:?}, {:?}",
        details.title, details.authors, details.genres, details.description
    );
    assert_eq!(details.title, "Pride and Prejudice");
    assert_eq!(details.authors, ["Jane Austen"]);
    assert!(!details.genres.iter().any(|genre| genre.starts_with("PD")));
    let chapters = host.manga_chapters(WIKISOURCE, id).await.unwrap();
    println!(
        "{} chapters, first {:?}",
        chapters.len(),
        chapters
            .iter()
            .take(3)
            .map(|chapter| &chapter.name)
            .collect::<Vec<_>>()
    );
    assert!(chapters.len() >= 61);
    let text = host
        .novel_passage(WIKISOURCE, &chapters[0].id)
        .await
        .unwrap();
    println!(
        "{:?}: {} blocks, first {:?}",
        chapters[0].name,
        text.len(),
        text.first()
    );
    assert!(paragraphs(&text) > 5);
    assert!(
        !text.iter().any(
            |block| matches!(block, PassageBlock::Paragraph(text) if text.contains("public domain"))
        ),
        "the licence box is left out"
    );

    // A work in books, whose pages list the chapters.
    let farewell = host
        .manga_chapters(WIKISOURCE, "A Farewell to Arms")
        .await
        .unwrap();
    println!(
        "A Farewell to Arms: {} chapters, first {:?}",
        farewell.len(),
        farewell
            .iter()
            .take(3)
            .map(|chapter| &chapter.name)
            .collect::<Vec<_>>()
    );
    assert!(farewell.len() > 20);
    // A single page is one chapter.
    let poem = host.manga_chapters(WIKISOURCE, "The Raven (Poe)").await;
    println!("The Raven: {poem:?}");
}

#[tokio::test]
#[ignore = "needs network access to commons.wikimedia.org"]
async fn open_sources_live_wikimedia_commons() {
    let host = host().await;
    let results = host.search_music("moonlight sonata", 10).await;
    let commons = results
        .iter()
        .find(|result| result.provider.extension_id == "dev.raygun.wikimedia-commons")
        .unwrap();
    assert_eq!(commons.error, None);
    for track in &commons.tracks {
        println!(
            "{} — {:?} ({:?} ms) {:?}",
            track.title, track.artist, track.duration_ms, track.source
        );
    }
    assert!(!commons.tracks.is_empty());
    let Some(MediaSource::Url { url, headers }) = &commons.tracks[0].source else {
        panic!("a playable file");
    };
    assert_served("song", url, headers).await;

    let query = TrackQuery {
        title: "Clair de lune".into(),
        artist: Some("Claude Debussy".into()),
        album: None,
        duration_ms: None,
    };
    let candidates: Vec<_> = host
        .stream_candidates(&query, 10, raygun_resolver::QualityTarget::Any)
        .await
        .into_iter()
        .filter(|answer| answer.provider.extension_id == "dev.raygun.wikimedia-commons")
        .flat_map(|answer| answer.result.unwrap())
        .collect();
    for candidate in &candidates {
        println!("candidate: {} | {:?}", candidate.title, candidate.artist);
    }
    assert!(!candidates.is_empty());

    let popular = host
        .manga_list(COMMONS_VIDEOS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("media of the day", &popular);
    let latest = host
        .manga_list(COMMONS_VIDEOS, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("newest media of the day", &latest);
    let found = host
        .manga_search(COMMONS_VIDEOS, "apollo 11", 1)
        .await
        .unwrap();
    assert_listing("search apollo 11", &found);
    let id = "Sprite Fright - Blender Open Movie-full movie.webm";
    let details = host.manga_details(COMMONS_VIDEOS, id).await.unwrap();
    println!(
        "details: {:?}, {:?}, {:?}, {:?}",
        details.title, details.year, details.genres, details.description
    );
    assert!(
        details
            .description
            .is_some_and(|text| text.contains("CC BY 4.0"))
    );
    let servers = host.video_servers(COMMONS_VIDEOS, id).await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    for video in videos {
        println!("  {:?} {:?} {}", video.label, video.quality, video.url);
    }
    assert!(videos.iter().any(|video| video.quality == Some(1080)));
    assert!(!videos[0].subtitles.is_empty(), "the film has subtitles");
    assert_served("video", &videos[0].url, &videos[0].headers).await;
    let episodes = host
        .video_episodes(COMMONS_VIDEOS, &found.items[0].id)
        .await
        .unwrap();
    let servers = host
        .video_servers(COMMONS_VIDEOS, &episodes[0].id)
        .await
        .unwrap();
    assert!(
        servers[0]
            .videos
            .as_ref()
            .is_some_and(|videos| !videos.is_empty())
    );
}

#[tokio::test]
#[ignore = "needs network access to images-api.nasa.gov"]
async fn open_sources_live_nasa() {
    let host = host().await;
    assert!(
        host.providers(VIDEO_SOURCE)
            .await
            .iter()
            .any(|provider| provider.id == NASA)
    );
    let popular = host.manga_list(NASA, MangaList::Popular, 1).await.unwrap();
    assert_listing("most viewed", &popular);
    let latest = host.manga_list(NASA, MangaList::Latest, 1).await.unwrap();
    assert_listing("this year and last", &latest);
    assert!(latest.has_next_page);
    let found = host.manga_search(NASA, "artemis", 1).await.unwrap();
    assert_listing("search artemis", &found);
    let kennedy = host
        .manga_search_filtered(
            NASA,
            "launch",
            1,
            &BTreeMap::from([
                ("center".to_owned(), select("KSC")),
                ("decade".to_owned(), select("2020")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("launches at Kennedy in the 2020s", &kennedy);

    let id = &found.items[0].id;
    let details = host.manga_details(NASA, id).await.unwrap();
    println!(
        "details: {:?}, {:?}, {:?}, {:?}",
        details.title, details.year, details.genres, details.description
    );
    assert!(details.year.is_some());
    let episodes = host.video_episodes(NASA, id).await.unwrap();
    let servers = host.video_servers(NASA, &episodes[0].id).await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    for video in videos {
        println!(
            "  {:?} {:?} {} subtitles {}",
            video.label,
            video.quality,
            video.url,
            video.subtitles.len()
        );
    }
    assert!(
        videos
            .iter()
            .all(|video| video.url.starts_with("https://images-assets.nasa.gov/"))
    );
    let small = videos
        .iter()
        .find(|video| video.quality == Some(360))
        .unwrap_or(&videos[0]);
    assert_served("video", &small.url, &small.headers).await;
    if let Some(captions) = videos[0].subtitles.first() {
        assert_served("captions", &captions.url, &BTreeMap::new()).await;
    }
}

#[tokio::test]
#[ignore = "needs network access to commons.wikimedia.org"]
async fn open_sources_live_blender_movies() {
    let host = host().await;
    let popular = host
        .manga_list(BLENDER, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("open movies", &popular);
    assert!(
        popular.items.iter().all(|item| item.cover_url.is_some()),
        "every film is on Commons"
    );
    let latest = host
        .manga_list(BLENDER, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_eq!(latest.items.len(), popular.items.len());
    let llamas = host.manga_search(BLENDER, "caminandes", 1).await.unwrap();
    assert_eq!(llamas.items.len(), 3);
    for film in &popular.items {
        let details = host.manga_details(BLENDER, &film.id).await.unwrap();
        let servers = host.video_servers(BLENDER, &film.id).await.unwrap();
        let videos = servers[0].videos.as_ref().expect("resolved at once");
        println!(
            "{} ({:?}): {} videos, best {:?}, {} subtitles, {:?}",
            details.title,
            details.year,
            videos.len(),
            videos.first().map(|video| &video.label),
            videos[0].subtitles.len(),
            details
                .description
                .as_deref()
                .map(|text| text.lines().last())
        );
        assert!(
            videos.iter().any(|video| video.quality == Some(1080)),
            "{}",
            film.id
        );
        assert!(
            details
                .description
                .as_deref()
                .is_some_and(|text| text.contains("Licence: CC BY")),
            "{}",
            film.id
        );
    }
    let servers = host.video_servers(BLENDER, "big-buck-bunny").await.unwrap();
    let video = &servers[0].videos.as_ref().unwrap()[0];
    assert_served("Big Buck Bunny", &video.url, &video.headers).await;
}

/// The Internet Archive's songs and films stay within open licences and the public domain.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn open_sources_live_internet_archive() {
    let host = host().await;
    let client = raygun_net::http_client().unwrap();
    let metadata = |identifier: String| {
        let client = client.clone();
        async move {
            let text = client
                .get(format!(
                    "https://archive.org/metadata/{identifier}/metadata"
                ))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap();
            serde_json::from_str::<serde_json::Value>(&text).unwrap()["result"].clone()
        }
    };
    let collections = |item: &serde_json::Value| -> Vec<String> {
        match &item["collection"] {
            serde_json::Value::String(one) => vec![one.clone()],
            serde_json::Value::Array(many) => many
                .iter()
                .filter_map(|value| value.as_str().map(str::to_owned))
                .collect(),
            _ => Vec::new(),
        }
    };

    for query in ["kai engel", "ambient", "beethoven"] {
        let results = host.search_music(query, 12).await;
        let archive = results
            .iter()
            .find(|result| result.provider.extension_id == "dev.raygun.internet-archive")
            .unwrap();
        assert_eq!(archive.error, None, "{query}");
        println!("{query}: {} tracks", archive.tracks.len());
        assert!(!archive.tracks.is_empty(), "{query}");
        let mut items: Vec<String> = archive
            .tracks
            .iter()
            .filter_map(|track| match &track.source {
                Some(MediaSource::Url { url, .. }) => url
                    .strip_prefix("https://archive.org/download/")
                    .and_then(|rest| rest.split('/').next())
                    .map(str::to_owned),
                _ => None,
            })
            .collect();
        items.sort_unstable();
        items.dedup();
        for identifier in items {
            let item = metadata(identifier.clone()).await;
            let licence = item["licenseurl"].as_str().unwrap_or_default().to_owned();
            let kept = collections(&item);
            println!("  {identifier}: {licence} {kept:?}");
            assert!(
                kept.iter()
                    .any(|name| name == "musopen" || name == "netlabels")
                    && (licence.contains("creativecommons.org")
                        || licence.contains("publicdomain")),
                "{identifier} isn't an openly licensed netlabel or Musopen recording"
            );
            assert!(
                !kept
                    .iter()
                    .any(|name| name == "78rpm" || name == "georgeblood")
            );
        }
    }

    let popular = host
        .manga_list(ARCHIVE_FILMS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("feature films", &popular);
    let cutoff = chrono_year() - 95;
    for film in popular.items.iter().take(12) {
        let item = metadata(film.id.clone()).await;
        let year: i32 = item["year"]
            .as_str()
            .and_then(|year| year.get(..4))
            .and_then(|year| year.parse().ok())
            .or_else(|| {
                item["date"]
                    .as_str()
                    .and_then(|date| date.get(..4)?.parse().ok())
            })
            .unwrap_or(9999);
        let licence = item["licenseurl"].as_str().unwrap_or_default();
        println!("  {} ({year}): {licence}", film.title);
        assert!(
            year < cutoff
                || (year <= 1977
                    && (licence.contains("publicdomain/mark")
                        || licence.contains("licenses/publicdomain"))),
            "{} isn't in the public domain by the extension's rule",
            film.id
        );
    }
    let prelinger = host
        .manga_search_filtered(
            ARCHIVE_FILMS,
            "",
            1,
            &BTreeMap::from([("collection".to_owned(), select("prelinger"))]),
        )
        .await
        .unwrap();
    assert_listing("Prelinger Archives", &prelinger);
}

/// This year, from the system clock (no date crate here).
fn chrono_year() -> i32 {
    let seconds = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap()
        .as_secs();
    // 365.2425 days a year since 1970 is exact enough for a year.
    1970 + (seconds as f64 / 31_556_952.0) as i32
}
