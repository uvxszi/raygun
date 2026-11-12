//! The public-domain and openly licensed sources against their real services (`open_sources_live`):
//! lists, search with filters, details, chapters or episodes, and that what they hand out plays
//! (or reads) and carries an open licence.
//!
//! `cargo test -p raygun-ext-host --test open_sources_live -- --ignored --nocapture`, or one source
//! with its name after `open_sources_live_`.

use std::collections::BTreeMap;
use std::path::Path;

use raygun_ext_host::manga::{MangaList, MangaListing};
use raygun_ext_host::novel::{NOVEL_SOURCE, Part, PassageBlock};
use raygun_ext_host::video::VIDEO_SOURCE;
use raygun_ext_host::{ExtensionHost, HostConfig, discover_installed};
use raygun_model::{
    ChartGroup, ExtensionStatus, FilterValue, MediaSource, MusicChart, OnlineTrack, ReleaseKind,
    TrackQuery,
};

const STANDARD_EBOOKS: &str = "dev.raygun.standard-ebooks/books";
const FYYD: &str = "dev.raygun.fyyd/podcasts";
const PODCAST_INDEX: &str = "dev.raygun.podcast-index/podcasts";
const INCOMPETECH_CHARTS: &str = "dev.raygun.incompetech/charts";
const GUTENBERG_AUDIOBOOKS: &str = "dev.raygun.gutenberg-audiobooks/books";
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
        .filter(|block| {
            !matches!(block, PassageBlock::Heading(_))
                && block.plain().is_some_and(|text| text.len() > 40)
        })
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
        !text.iter().any(|block| block
            .plain()
            .is_some_and(|text| text.contains("public domain"))),
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

/// The Internet Archive's songs (netlabels, Musopen, the Free Music Archive), films and MIT
/// lectures stay within open licences and the public domain.
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

    let mut free_music_archive = false;
    for query in ["kai engel", "ambient", "beethoven", "dj harrison"] {
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
                kept.iter().any(|name| name == "musopen"
                    || name == "netlabels"
                    || name == "freemusicarchive")
                    && (licence.contains("creativecommons.org")
                        || licence.contains("publicdomain")),
                "{identifier} isn't an openly licensed netlabel, Musopen or Free Music Archive recording"
            );
            assert!(
                !kept
                    .iter()
                    .any(|name| name == "78rpm" || name == "georgeblood")
            );
            free_music_archive |= kept.iter().any(|name| name == "freemusicarchive");
        }
    }
    assert!(
        free_music_archive,
        "the Free Music Archive's albums are searched"
    );

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

    // MIT OpenCourseWare's lectures: MIT's own uploads, each under a Creative Commons licence.
    let lectures = "dev.raygun.internet-archive/lectures";
    let courses = host
        .manga_list(lectures, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("MIT OpenCourseWare", &courses);
    for listed in courses.items.iter().take(6) {
        let item = metadata(listed.id.clone()).await;
        let uploader = item["uploader"].as_str().unwrap_or_default();
        let licence = item["licenseurl"].as_str().unwrap_or_default();
        println!("  {} ({uploader}): {licence}", listed.title);
        assert_eq!(uploader, "ocw-prod@mit.edu", "{}", listed.id);
        assert!(licence.contains("creativecommons.org/"), "{}", listed.id);
    }
    let found = host
        .manga_search(lectures, "linear algebra", 1)
        .await
        .unwrap();
    assert_listing("linear algebra", &found);
    let id = &courses.items[0].id;
    let details = host.manga_details(lectures, id).await.unwrap();
    let description = details.description.clone().unwrap_or_default();
    println!("{}:\n{description}", details.title);
    assert!(description.contains("Licence: CC "));
    let episodes = host.video_episodes(lectures, id).await.unwrap();
    println!(
        "  {} lectures, first {:?}",
        episodes.len(),
        episodes.first().map(|episode| &episode.name)
    );
    assert!(episodes.len() > 1);
    let servers = host.video_servers(lectures, &episodes[0].id).await.unwrap();
    let video = &servers[0].videos.as_ref().expect("resolved at once")[0];
    assert_served("lecture", &video.url, &video.headers).await;
    if let Some(subtitles) = video.subtitles.first() {
        assert_served("lecture subtitles", &subtitles.url, &BTreeMap::new()).await;
    }
}

/// The songs one extension found for a search, failing on its error.
async fn songs_from(host: &ExtensionHost, extension: &str, query: &str) -> Vec<OnlineTrack> {
    let results = host.search_music(query, 10).await;
    let found = results
        .iter()
        .find(|result| result.provider.extension_id == extension)
        .unwrap_or_else(|| panic!("{extension} isn't a music source"));
    assert_eq!(found.error, None, "{extension}: {query}");
    for track in &found.tracks {
        println!(
            "{query}: {} — {:?} ({:?} ms) {:?}",
            track.title, track.artist, track.duration_ms, track.source
        );
    }
    found.tracks.clone()
}

/// A track's playable address and the headers it goes with.
fn url_of(track: &OnlineTrack) -> (&str, &BTreeMap<String, String>) {
    match &track.source {
        Some(MediaSource::Url { url, headers }) => (url, headers),
        other => panic!("{} has no stream: {other:?}", track.title),
    }
}

/// The charts one extension lists.
async fn charts_of(host: &ExtensionHost, extension: &str) -> Vec<MusicChart> {
    let mut charts = Vec::new();
    for (provider, answer) in host.music_charts(None).await {
        if provider.extension_id == extension {
            charts.extend(answer.unwrap_or_else(|error| panic!("{extension}: {error}")));
        }
    }
    for chart in &charts {
        println!(
            "chart {} {:?} {:?} {:?}",
            chart.id, chart.name, chart.group, chart.tag
        );
    }
    charts
}

#[tokio::test]
#[ignore = "needs network access to api.fyyd.de"]
async fn open_sources_live_fyyd() {
    let host = host().await;
    let found = host.podcast_search(FYYD, "radiolab", 10).await.unwrap();
    for podcast in &found {
        println!(
            "{} — {:?} {}",
            podcast.title, podcast.author, podcast.feed_url
        );
    }
    assert!(
        found
            .first()
            .is_some_and(|podcast| podcast.title.to_lowercase().contains("radiolab")),
        "titles first"
    );
    let top = host.podcast_top(FYYD, 20).await.unwrap();
    println!(
        "{} hot podcasts, first {:?}",
        top.len(),
        top.first().map(|p| &p.title)
    );
    assert!(top.len() >= 10);
    let mut feeds: Vec<&str> = top
        .iter()
        .map(|podcast| podcast.feed_url.as_str())
        .collect();
    feeds.sort_unstable();
    feeds.dedup();
    assert_eq!(feeds.len(), top.len(), "a feed repeats");
    // A feed it names is a podcast feed.
    let feed = raygun_net::http_client()
        .unwrap()
        .get(&found[0].feed_url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    assert!(
        feed.contains("<rss") || feed.contains("<feed"),
        "not a feed"
    );
}

/// Podcast Index with the API key and secret in `PODCASTINDEX_KEY` and `PODCASTINDEX_SECRET` (each
/// person's own, free from api.podcastindex.org); without them it only checks the source asks for
/// them.
#[tokio::test]
#[ignore = "needs network access to api.podcastindex.org"]
async fn open_sources_live_podcast_index() {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions/podcast-index");
    let credentials = std::env::var("PODCASTINDEX_KEY")
        .ok()
        .zip(std::env::var("PODCASTINDEX_SECRET").ok());
    let mut settings = BTreeMap::new();
    if let Some((key, secret)) = &credentials {
        settings.insert("apiKey".to_owned(), serde_json::json!(key));
        settings.insert("apiSecret".to_owned(), serde_json::json!(secret));
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load_with_settings(
            vec![raygun_ext_host::ExtensionSource::installed(dir)],
            raygun_ext_host::UserSettings::from([(
                "dev.raygun.podcast-index".to_owned(),
                settings,
            )]),
        )
        .await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    let found = host.podcast_search(PODCAST_INDEX, "radiolab", 10).await;
    let Some(_) = credentials else {
        let error = found.unwrap_err();
        println!("without a key: {error}");
        assert!(error.contains("API key and secret"), "{error}");
        return;
    };
    let found = found.unwrap();
    for podcast in &found {
        println!(
            "{} — {:?} {}",
            podcast.title, podcast.author, podcast.feed_url
        );
    }
    assert!(
        found
            .iter()
            .any(|podcast| podcast.title.to_lowercase().contains("radiolab"))
    );
    let trending = host.podcast_top(PODCAST_INDEX, 20).await.unwrap();
    println!(
        "{} trending, first {:?}",
        trending.len(),
        trending.first().map(|podcast| &podcast.title)
    );
    assert!(trending.len() >= 10);
}

/// Kevin MacLeod's music under CC BY 4.0: search, charts, collections and his pieces' streams.
#[tokio::test]
#[ignore = "needs network access to incompetech.com"]
async fn open_sources_live_incompetech() {
    let host = host().await;
    let tracks = songs_from(&host, "dev.raygun.incompetech", "monkeys spinning").await;
    assert_eq!(tracks[0].title, "Monkeys Spinning Monkeys");
    assert_eq!(tracks[0].artist.as_deref(), Some("Kevin MacLeod"));
    let (url, headers) = url_of(&tracks[0]);
    assert_served("piece", url, headers).await;

    let charts = charts_of(&host, "dev.raygun.incompetech").await;
    assert!(
        charts
            .iter()
            .any(|chart| chart.tag.as_deref() == Some("Soundtrack"))
    );
    assert!(charts.iter().any(|chart| chart.group == ChartGroup::Mood));
    let known = host
        .music_chart(INCOMPETECH_CHARTS, "well-known", None, 10)
        .await
        .unwrap();
    assert_eq!(known.len(), 10);
    assert_eq!(known[0].title, "Monkeys Spinning Monkeys");
    let chill = host
        .music_tag_songs("Chill", &["calm".to_owned()], "mood", None, 10)
        .await;
    let (_, answer) = chill
        .iter()
        .find(|(provider, _)| provider.extension_id == "dev.raygun.incompetech")
        .unwrap();
    assert!(answer.as_ref().is_ok_and(|(_, tracks)| tracks.len() == 10));

    let albums: Vec<_> = host
        .search_albums("hard electronic", 5)
        .await
        .into_iter()
        .filter(|album| album.provider.extension_id == "dev.raygun.incompetech")
        .collect();
    assert_eq!(albums[0].title, "Hard Electronic");
    let album = host
        .music_album(&albums[0].provider.id, &albums[0].id)
        .await
        .unwrap();
    println!("{}: {} pieces", album.album.title, album.tracks.len());
    assert!(album.tracks.len() > 10);

    let query = TrackQuery {
        title: "Sneaky Snitch".into(),
        artist: Some("Kevin MacLeod".into()),
        album: None,
        duration_ms: None,
    };
    let candidates: Vec<_> = host
        .stream_candidates(&query, 5, raygun_resolver::QualityTarget::Any)
        .await
        .into_iter()
        .filter(|answer| answer.provider.extension_id == "dev.raygun.incompetech")
        .flat_map(|answer| answer.result.unwrap())
        .collect();
    assert_eq!(candidates.len(), 1, "{candidates:?}");
    assert_eq!(candidates[0].title, "Sneaky Snitch");
}

/// Concerts by artists who allow sharing them, never one they keep to streaming.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn open_sources_live_live_music_archive() {
    let host = host().await;
    let tracks = songs_from(
        &host,
        "dev.raygun.live-music-archive",
        "grateful dead scarlet begonias",
    )
    .await;
    assert!(tracks.len() >= 3);
    assert!(tracks.iter().all(|track| track.title == "Scarlet Begonias"));
    let (url, headers) = url_of(&tracks[0]);
    assert_served("song", url, headers).await;

    let albums: Vec<_> = host
        .search_albums("dark star orchestra 2007", 10)
        .await
        .into_iter()
        .filter(|album| album.provider.extension_id == "dev.raygun.live-music-archive")
        .collect();
    assert!(!albums.is_empty());
    let concert = host
        .music_album(&albums[0].provider.id, &albums[0].id)
        .await
        .unwrap();
    println!(
        "{}: {:?}",
        concert.album.title,
        concert
            .tracks
            .iter()
            .map(|track| &track.title)
            .collect::<Vec<_>>()
    );
    assert!(concert.tracks.len() > 5);
    assert_eq!(concert.album.kind, Some(ReleaseKind::Live));

    let releases: Vec<_> = host
        .artist_releases("Grateful Dead", 10)
        .await
        .into_iter()
        .filter(|album| album.provider.extension_id == "dev.raygun.live-music-archive")
        .collect();
    println!(
        "{:?}",
        releases
            .iter()
            .map(|album| &album.title)
            .collect::<Vec<_>>()
    );
    assert!(releases.len() >= 5);
    assert!(
        releases
            .iter()
            .all(|album| album.artist.as_deref() == Some("Grateful Dead"))
    );
    // Not one is kept to streaming only.
    let client = raygun_net::http_client().unwrap();
    for album in releases.iter().take(5) {
        let item: serde_json::Value = serde_json::from_str(
            &client
                .get(format!(
                    "https://archive.org/metadata/{}/metadata",
                    album.id
                ))
                .send()
                .await
                .unwrap()
                .text()
                .await
                .unwrap(),
        )
        .unwrap();
        let collections = item["result"]["collection"].to_string();
        assert!(collections.contains("etree"), "{}", album.id);
        assert!(!collections.contains("stream_only"), "{}", album.id);
    }
}

/// Project Gutenberg's books read by neural voices.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn open_sources_live_gutenberg_audiobooks() {
    let host = host().await;
    let popular = host
        .audiobooks(GUTENBERG_AUDIOBOOKS, "popular", "", 1)
        .await
        .unwrap();
    println!(
        "popular: {:?}",
        popular
            .books
            .iter()
            .take(5)
            .map(|book| (&book.title, &book.author))
            .collect::<Vec<_>>()
    );
    assert!(popular.books.len() >= 20);
    assert!(popular.has_more);
    let verne = host
        .audiobooks(GUTENBERG_AUDIOBOOKS, "search", "jules verne", 1)
        .await
        .unwrap();
    assert!(
        verne
            .books
            .iter()
            .any(|book| book.author.as_deref() == Some("Jules Verne"))
    );
    let (provider, id) = raygun_ext_host::audiobook::parse_listed_key(&verne.books[0].feed_url)
        .expect("books are listed by the source");
    let book = host.audiobook_details(provider, id).await.unwrap();
    println!("{} by {:?}: {:?}", book.title, book.author, book.chapters);
    assert!(!book.chapters.is_empty());
    assert_served("book", &book.chapters[0].audio_url, &BTreeMap::new()).await;
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

// ---- Library of Congress, National Archives, Europeana ----

const LOC_FILMS: &str = "dev.raygun.loc/films";
const LOC_RECORDINGS: &str = "dev.raygun.loc/recordings";

/// The Library of Congress source alone, with time for the API's stalls: an uncached query can
/// run until its request times out, and the source asks again once the API has answered it.
async fn loc_host() -> ExtensionHost {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions/loc");
    let host = ExtensionHost::start(HostConfig {
        call_timeout: std::time::Duration::from_secs(90),
        ..HostConfig::default()
    })
    .unwrap();
    let infos = host
        .load(vec![raygun_ext_host::ExtensionSource::installed(dir)])
        .await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    host
}

/// Library of Congress films published before 1931 and recordings published before 1926: about
/// eight requests, which the source spaces 3.1 s apart (the API allows 20 a minute).
#[tokio::test]
#[ignore = "needs network access to www.loc.gov"]
async fn open_sources_live_loc() {
    let host = loc_host().await;
    let popular = host
        .manga_list(LOC_FILMS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("Edison films", &popular);
    assert!(popular.has_next_page);
    let found = host.manga_search(LOC_FILMS, "fire", 1).await.unwrap();
    assert_listing("search fire", &found);
    let animation = host
        .manga_search_filtered(
            LOC_FILMS,
            "",
            1,
            &BTreeMap::from([(
                "collection".to_owned(),
                select("origins-of-american-animation"),
            )]),
        )
        .await
        .unwrap();
    assert_listing("early animation", &animation);

    // A film's page reads its rights statement first and refuses one with restrictions.
    let id = &animation.items[0].id;
    let details = host.manga_details(LOC_FILMS, id).await.unwrap();
    println!(
        "details: {:?}, {:?}, {:?}\n{}",
        details.title,
        details.year,
        details.genres,
        details.description.as_deref().unwrap_or_default()
    );
    assert!(details.year.is_some_and(|year| year <= chrono_year() - 96));
    assert!(
        details
            .description
            .as_deref()
            .is_some_and(|text| text.contains("Public domain in the United States"))
    );
    let episodes = host.video_episodes(LOC_FILMS, id).await.unwrap();
    let servers = host
        .video_servers(LOC_FILMS, &episodes[0].id)
        .await
        .unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    for video in videos {
        println!(
            "  {:?} {:?} {:?} {}",
            video.label, video.quality, video.format, video.url
        );
    }
    let mp4 = videos
        .iter()
        .find(|video| video.format.is_none())
        .expect("an MP4");
    assert!(mp4.url.starts_with("https://tile.loc.gov/"));
    assert_served("film", &mp4.url, &mp4.headers).await;
    assert!(
        videos
            .iter()
            .any(|video| video.format.as_deref() == Some("hls"))
    );

    // Recordings: two lists read once, searched on the device.
    let tracks = songs_from(&host, "dev.raygun.loc", "rag").await;
    assert!(!tracks.is_empty());
    let (url, headers) = url_of(&tracks[0]);
    assert!(url.starts_with("https://tile.loc.gov/"));
    assert_served("recording", url, headers).await;
    let albums = host.search_albums("edison", 5).await;
    assert!(
        albums
            .iter()
            .any(|album| album.provider.id == LOC_RECORDINGS && album.id == "edison")
    );
    let edison = host.music_album(LOC_RECORDINGS, "edison").await.unwrap();
    println!("{}: {} recordings", edison.album.title, edison.tracks.len());
    assert!(edison.tracks.len() > 50);
}

/// One first-party extension with its API key set (none when `key` is `None`).
async fn keyed_host(folder: &str, id: &str, key: Option<&str>) -> ExtensionHost {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions")
        .join(folder);
    let mut settings = BTreeMap::new();
    if let Some(key) = key {
        settings.insert("apiKey".to_owned(), serde_json::json!(key));
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load_with_settings(
            vec![raygun_ext_host::ExtensionSource::installed(dir)],
            raygun_ext_host::UserSettings::from([(id.to_owned(), settings)]),
        )
        .await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    host
}

/// The key in an environment variable, when one is set.
fn key_from(variable: &str) -> Option<String> {
    std::env::var(variable)
        .ok()
        .filter(|key| !key.trim().is_empty())
}

const NARA_FILMS: &str = "dev.raygun.nara/films";

/// National Archives films with the API key in `NARA_API_KEY` (each person's own, asked for by
/// email at Catalog_API@nara.gov); without one it checks the source waits for it, asked nothing.
#[tokio::test]
#[ignore = "needs network access to catalog.archives.gov"]
async fn open_sources_live_nara() {
    let key = key_from("NARA_API_KEY");
    let host = keyed_host("nara", "dev.raygun.nara", key.as_deref()).await;
    let popular = host.manga_list(NARA_FILMS, MangaList::Popular, 1).await;
    if key.is_none() {
        let error = popular.unwrap_err();
        println!("without a key: {error}");
        assert_eq!(
            raygun_ext_host::needs_setup_in(&error),
            Some("dev.raygun.nara")
        );
        assert!(host.needs_setup("dev.raygun.nara").is_some());
        return;
    }
    // The catalogue has no pictures of its films, so the lists have no covers to check.
    let popular = popular.unwrap();
    println!(
        "Army library copies: {} films (more: {}), first {:?}",
        popular.items.len(),
        popular.has_next_page,
        popular.items.first().map(|item| &item.title)
    );
    assert!(!popular.items.is_empty());
    let found = host
        .manga_search_filtered(
            NARA_FILMS,
            "navy",
            1,
            &BTreeMap::from([("decade".to_owned(), select("1940"))]),
        )
        .await
        .unwrap();
    println!(
        "navy in the 1940s: {} films, first {:?}",
        found.items.len(),
        found.items.first().map(|item| &item.title)
    );

    let id = &popular.items[0].id;
    let details = host.manga_details(NARA_FILMS, id).await.unwrap();
    println!(
        "details: {:?}, {:?}\n{}",
        details.title,
        details.year,
        details.description.as_deref().unwrap_or_default()
    );
    assert!(
        details
            .description
            .as_deref()
            .is_some_and(|text| text.contains("Unrestricted use and access"))
    );
    let episodes = host.video_episodes(NARA_FILMS, id).await.unwrap();
    println!(
        "reels: {:?}",
        episodes
            .iter()
            .map(|episode| &episode.name)
            .collect::<Vec<_>>()
    );
    let servers = host
        .video_servers(NARA_FILMS, &episodes[0].id)
        .await
        .unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    println!("  {}", videos[0].url);
    assert_served("film", &videos[0].url, &videos[0].headers).await;
}

const EUROPEANA_VIDEOS: &str = "dev.raygun.europeana/videos";

/// The rights of Europeana items by id, asked of Europeana's search directly (one request).
async fn europeana_rights(key: &str, ids: &[&str]) -> BTreeMap<String, String> {
    let query = format!(
        "europeana_id:({})",
        ids.iter()
            .map(|id| format!("\"/{id}\""))
            .collect::<Vec<_>>()
            .join(" OR ")
    );
    let rows = ids.len().to_string();
    let url = reqwest::Url::parse_with_params(
        "https://api.europeana.eu/record/v2/search.json",
        [
            ("query", query.as_str()),
            ("rows", rows.as_str()),
            ("profile", "minimal"),
        ],
    )
    .unwrap();
    let text = raygun_net::http_client()
        .unwrap()
        .get(url)
        .header("X-Api-Key", key)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let answer: serde_json::Value = serde_json::from_str(&text).unwrap();
    answer["items"]
        .as_array()
        .into_iter()
        .flatten()
        .filter_map(|item| {
            let id = item["id"].as_str()?.trim_start_matches('/').to_owned();
            Some((id, item["rights"][0].as_str()?.to_owned()))
        })
        .collect()
}

/// Public Domain Mark, CC0, CC BY and CC BY-SA, any version or port.
fn open_licence(rights: &str) -> bool {
    [
        "http://creativecommons.org/publicdomain/mark/",
        "http://creativecommons.org/publicdomain/zero/",
        "http://creativecommons.org/licenses/by/",
        "http://creativecommons.org/licenses/by-sa/",
    ]
    .iter()
    .any(|open| rights.replace("https://", "http://").starts_with(open))
}

/// Europeana's open films and recordings with the API key in `EUROPEANA_API_KEY` (each person's
/// own, from their europeana.eu account; Europeana's public demo key works for trying it);
/// without one it checks the source waits for it, asked nothing.
#[tokio::test]
#[ignore = "needs network access to api.europeana.eu"]
async fn open_sources_live_europeana() {
    let key = key_from("EUROPEANA_API_KEY");
    let host = keyed_host("europeana", "dev.raygun.europeana", key.as_deref()).await;
    let popular = host
        .manga_list(EUROPEANA_VIDEOS, MangaList::Popular, 1)
        .await;
    let Some(key) = key else {
        let error = popular.unwrap_err();
        println!("without a key: {error}");
        assert_eq!(
            raygun_ext_host::needs_setup_in(&error),
            Some("dev.raygun.europeana")
        );
        let music = host.search_music("hora", 10).await;
        assert!(
            music
                .iter()
                .all(|result| result.provider.extension_id != "dev.raygun.europeana"),
            "a source waiting for its key is left out of searches"
        );
        return;
    };
    let popular = popular.unwrap();
    assert_listing("open videos", &popular);
    let football = host
        .manga_search_filtered(
            EUROPEANA_VIDEOS,
            "football",
            1,
            &BTreeMap::from([("country".to_owned(), select("netherlands"))]),
        )
        .await
        .unwrap();
    assert_listing("football in the Netherlands", &football);
    let latest = host
        .manga_list(EUROPEANA_VIDEOS, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("recently added", &latest);
    for listing in [&popular, &football, &latest] {
        let ids: Vec<&str> = listing.items.iter().map(|item| item.id.as_str()).collect();
        let rights = europeana_rights(&key, &ids).await;
        for id in ids {
            let licence = rights.get(id).map_or("(none)", String::as_str);
            assert!(open_licence(licence), "{id} is under {licence}");
        }
    }

    // A film names its licence and the institution that holds it, and plays from its file.
    let id = &popular.items[0].id;
    let details = host.manga_details(EUROPEANA_VIDEOS, id).await.unwrap();
    let description = details.description.clone().unwrap_or_default();
    println!(
        "details: {:?}, {:?}\n{description}",
        details.title, details.year
    );
    assert!(description.contains("Licence: "));
    assert!(description.contains("Provided by "));
    let episodes = host.video_episodes(EUROPEANA_VIDEOS, id).await.unwrap();
    let servers = host
        .video_servers(EUROPEANA_VIDEOS, &episodes[0].id)
        .await
        .unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    println!("  {:?} {}", videos[0].label, videos[0].url);
    assert_served("film", &videos[0].url, &videos[0].headers).await;

    // Recordings play from the institutions' own MP3s, each under an open licence.
    let tracks = songs_from(&host, "dev.raygun.europeana", "hora").await;
    assert!(!tracks.is_empty());
    let ids: Vec<&str> = tracks
        .iter()
        .filter_map(|track| track.external_id.as_deref())
        .collect();
    assert_eq!(ids.len(), tracks.len(), "every recording has its id");
    let rights = europeana_rights(&key, &ids).await;
    for id in &ids {
        let licence = rights.get(*id).map_or("(none)", String::as_str);
        assert!(open_licence(licence), "{id} is under {licence}");
    }
    let (url, headers) = url_of(&tracks[0]);
    assert_served("recording", url, headers).await;
}

const OPENSTAX: &str = "dev.raygun.openstax/books";

/// How many formulas a passage holds, on lines of their own or in paragraphs.
fn formulas(blocks: &[PassageBlock]) -> usize {
    blocks
        .iter()
        .map(|block| match block {
            PassageBlock::Math(_) => 1,
            PassageBlock::Rich(parts) => parts
                .iter()
                .filter(|part| matches!(part, Part::Math(_)))
                .count(),
            PassageBlock::Item(item) => item
                .parts
                .iter()
                .filter(|part| matches!(part, Part::Math(_)))
                .count(),
            PassageBlock::Table(table) => table
                .rows
                .iter()
                .flatten()
                .flat_map(|cell| &cell.parts)
                .filter(|part| matches!(part, Part::Math(_)))
                .count(),
            _ => 0,
        })
        .sum()
}

/// OpenStax's textbooks: the catalogue, search, a book's licence and pages, a section's figures
/// served from the archive, and a Calculus section's formulas.
#[tokio::test]
#[ignore = "needs network access to openstax.org"]
async fn open_sources_live_openstax() {
    let host = host().await;
    let popular = host
        .manga_list(OPENSTAX, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("by subject", &popular);
    assert!(popular.has_next_page, "more books than a page");
    let latest = host
        .manga_list(OPENSTAX, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("newest", &latest);
    let spanish = host
        .manga_search_filtered(
            OPENSTAX,
            "",
            1,
            &BTreeMap::from([("language".to_owned(), select("es"))]),
        )
        .await
        .unwrap();
    assert_listing("Spanish books", &spanish);
    assert_eq!(host.manga_filters(OPENSTAX).await.unwrap().len(), 3);
    let found = host.manga_search(OPENSTAX, "psychology", 1).await.unwrap();
    assert_listing("search psychology", &found);
    let book = &found.items[0];
    assert!(book.title.starts_with("Psychology"), "{}", book.title);
    assert!(!book.authors.is_empty());

    let details = host.manga_details(OPENSTAX, &book.id).await.unwrap();
    let description = details.description.clone().unwrap_or_default();
    println!(
        "details: {:?} by {:?}, {:?}, {:?}\n{description}",
        details.title, details.authors, details.genres, details.url
    );
    assert!(
        description.contains("Licence: CC BY")
            && description.contains("(from OpenStax's book data)"),
        "a Creative Commons licence from the book's data"
    );
    assert!(!details.authors.is_empty());
    assert!(
        details
            .url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://openstax.org/details/books/"))
    );

    let chapters = host.manga_chapters(OPENSTAX, &book.id).await.unwrap();
    println!(
        "{} chapters, first {:?}",
        chapters.len(),
        chapters
            .iter()
            .take(4)
            .map(|chapter| &chapter.name)
            .collect::<Vec<_>>()
    );
    assert!(chapters.len() > 100);
    assert!(
        chapters
            .iter()
            .any(|chapter| chapter.name == "Chapter 1 · Introduction")
    );
    assert!(chapters.iter().all(|chapter| chapter.name != "Index"));
    let history = chapters
        .iter()
        .find(|chapter| chapter.name.starts_with("1.2 "))
        .expect("section 1.2");
    let text = host.novel_passage(OPENSTAX, &history.id).await.unwrap();
    println!(
        "{:?}: {} blocks, first {:?}",
        history.name,
        text.len(),
        text.first()
    );
    assert!(paragraphs(&text) > 5);
    let figure = text
        .iter()
        .find_map(|block| match block {
            PassageBlock::Image(url) => Some(url),
            _ => None,
        })
        .expect("a figure");
    assert!(
        figure.starts_with("https://openstax.org/apps/archive/"),
        "{figure}"
    );
    assert_served("figure", figure, &BTreeMap::new()).await;

    // Formulas come as MathML and are drawn.
    let calculus = host
        .manga_search(OPENSTAX, "calculus volume 1", 1)
        .await
        .unwrap();
    let calculus = calculus
        .items
        .iter()
        .find(|item| item.title == "Calculus Volume 1")
        .expect("Calculus Volume 1");
    let chapters = host.manga_chapters(OPENSTAX, &calculus.id).await.unwrap();
    let limits = chapters
        .iter()
        .find(|chapter| chapter.name.starts_with("2.2 "))
        .expect("section 2.2");
    let raw = host
        .call(
            OPENSTAX,
            "passage",
            serde_json::json!({ "chapterId": limits.id }),
        )
        .await
        .unwrap();
    assert!(
        raw["html"]
            .as_str()
            .is_some_and(|html| html.contains("<math"))
    );
    let text = host.novel_passage(OPENSTAX, &limits.id).await.unwrap();
    println!(
        "{:?}: {} blocks, {} formulas",
        limits.name,
        text.len(),
        formulas(&text)
    );
    assert!(formulas(&text) > 20);
}

const LASTFM: &str = "dev.raygun.lastfm";
const LASTFM_CHARTS: &str = "dev.raygun.lastfm/charts";

/// Last.fm with the API key in `LASTFM_API_KEY` (each person's own, free from
/// last.fm/api/account/create): charts, genres, recommendations and a biography. Without one it
/// checks the source waits for it, asked nothing.
#[tokio::test]
#[ignore = "needs network access to ws.audioscrobbler.com"]
async fn open_sources_live_lastfm() {
    let key = key_from("LASTFM_API_KEY");
    let host = keyed_host("lastfm", LASTFM, key.as_deref()).await;
    let top = host.music_chart(LASTFM_CHARTS, "top", None, 20).await;
    if key.is_none() {
        let error = top.unwrap_err().to_string();
        println!("without a key: {error}");
        assert_eq!(raygun_ext_host::needs_setup_in(&error), Some(LASTFM));
        assert!(host.needs_setup(LASTFM).is_some());
        assert!(
            host.music_charts(Some("us"))
                .await
                .iter()
                .all(|(provider, _)| provider.extension_id != LASTFM),
            "a source waiting for its key isn't asked"
        );
        assert_eq!(host.artist_info("Radiohead").await, None);
        return;
    }
    let top = top.unwrap();
    for track in top.iter().take(5) {
        println!(
            "top: {} — {:?} ({:?} ms)",
            track.title, track.artist, track.duration_ms
        );
    }
    assert_eq!(top.len(), 20);
    assert!(
        top.iter()
            .all(|track| track.artist.is_some() && track.artwork_url.is_none())
    );

    let charts = charts_of(&host, LASTFM).await;
    assert!(
        charts
            .iter()
            .any(|chart| chart.group == ChartGroup::Genre && chart.tag.as_deref() == Some("Rock"))
    );
    let regional: Vec<MusicChart> = host
        .music_charts(Some("de"))
        .await
        .into_iter()
        .filter(|(provider, _)| provider.extension_id == LASTFM)
        .flat_map(|(_, answer)| answer.unwrap())
        .collect();
    assert!(regional.iter().any(|chart| chart.id == "country:de"));
    for id in [
        "country:de",
        "country:ru",
        "country:kr",
        "tag:rock",
        "tag:hip-hop",
    ] {
        let songs = host.music_chart(LASTFM_CHARTS, id, None, 10).await.unwrap();
        println!(
            "{id}: {:?}",
            songs
                .iter()
                .take(3)
                .map(|track| &track.title)
                .collect::<Vec<_>>()
        );
        assert!(!songs.is_empty(), "{id} is empty");
    }
    let chill = host
        .music_tag_songs(
            "Chill",
            &["relaxing".to_owned(), "calm".to_owned()],
            "mood",
            None,
            10,
        )
        .await;
    let (_, answer) = chill
        .iter()
        .find(|(provider, _)| provider.extension_id == LASTFM)
        .unwrap();
    assert!(answer.as_ref().is_ok_and(|(_, tracks)| !tracks.is_empty()));

    let query = TrackQuery {
        title: "Get Lucky".into(),
        artist: Some("Daft Punk".into()),
        album: None,
        duration_ms: None,
    };
    let tags: Vec<(String, f64)> = host
        .music_tags(&query)
        .await
        .into_iter()
        .filter(|(provider, _)| provider.extension_id == LASTFM)
        .flat_map(|(_, answer)| answer.unwrap())
        .collect();
    println!("Get Lucky: {tags:?}");
    assert!(!tags.is_empty() && tags.len() <= 10);
    assert!(tags.iter().all(|(name, weight)| {
        name.to_lowercase() != "seen live" && (0.0..=1.0).contains(weight)
    }));

    let similar = host.similar_artists("Radiohead", 10).await;
    println!("like Radiohead: {similar:?}");
    assert!(!similar.is_empty() && !similar.iter().any(|name| name == "Radiohead"));
    let best = host
        .recommended_tracks(
            "topTracks",
            serde_json::json!({ "artist": "Radiohead", "limit": 10 }),
        )
        .await;
    assert!(
        !best.is_empty()
            && best
                .iter()
                .all(|track| track.artist.as_deref() == Some("Radiohead"))
    );
    let like_song = host
        .recommended_tracks(
            "radio",
            serde_json::json!({ "artist": "Radiohead", "title": "Karma Police", "limit": 20 }),
        )
        .await;
    assert!(!like_song.is_empty());
    let mix = host
        .recommended_tracks(
            "radio",
            serde_json::json!({ "artist": "Radiohead", "title": null, "limit": 20 }),
        )
        .await;
    let mut artists: Vec<_> = mix
        .iter()
        .filter_map(|track| track.artist.clone())
        .collect();
    artists.sort();
    artists.dedup();
    println!(
        "Radiohead radio: {} songs by {} artists",
        mix.len(),
        artists.len()
    );
    assert!(artists.len() > 2, "the closest artists, taking turns");

    let info = host.artist_info("Radiohead").await.expect("a biography");
    let bio = info.bio.unwrap_or_default();
    println!("{}: {bio}\n{:?}", info.name, info.url);
    assert!(
        !bio.is_empty()
            && !bio.contains("Read more on Last.fm")
            && !bio.contains("Creative Commons")
    );
    assert!(
        info.url
            .is_some_and(|url| url.starts_with("https://www.last.fm/music/"))
    );
    assert_eq!(info.image_url, None);
}

// ---- National Park Service, ESA/Hubble and ESA/Webb ----

const NPS_VIDEOS: &str = "dev.raygun.nps/videos";

/// National Park Service videos with the API key in `NPS_API_KEY` (each person's own, free from
/// nps.gov/subjects/developer/get-started.htm; the API's `DEMO_KEY` works for trying it, ten
/// requests a day): two API requests, the library's list and a park's search, then a video from
/// the list opened, played and captioned without another. Without a key it checks the source
/// waits for one, asked nothing.
#[tokio::test]
#[ignore = "needs network access to developer.nps.gov"]
async fn open_sources_live_nps() {
    let key = key_from("NPS_API_KEY");
    let host = keyed_host("nps", "dev.raygun.nps", key.as_deref()).await;
    let popular = host.manga_list(NPS_VIDEOS, MangaList::Popular, 1).await;
    if key.is_none() {
        let error = popular.unwrap_err();
        println!("without a key: {error}");
        assert_eq!(
            raygun_ext_host::needs_setup_in(&error),
            Some("dev.raygun.nps")
        );
        assert!(host.needs_setup("dev.raygun.nps").is_some());
        return;
    }
    let popular = popular.unwrap();
    assert_listing("the library's videos", &popular);
    assert!(popular.has_next_page);
    let bison = host
        .manga_search_filtered(
            NPS_VIDEOS,
            "bison",
            1,
            &BTreeMap::from([("park".to_owned(), select("yell"))]),
        )
        .await
        .unwrap();
    assert_listing("bison at Yellowstone", &bison);

    let id = &bison.items[0].id;
    let details = host.manga_details(NPS_VIDEOS, id).await.unwrap();
    let description = details.description.clone().unwrap_or_default();
    println!(
        "details: {:?}, {:?}\n{description}",
        details.title, details.genres
    );
    assert!(description.contains("Public domain in the United States"));
    let episodes = host.video_episodes(NPS_VIDEOS, id).await.unwrap();
    let servers = host
        .video_servers(NPS_VIDEOS, &episodes[0].id)
        .await
        .unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    for video in videos {
        println!(
            "  {:?} {:?} {} captions {}",
            video.label,
            video.quality,
            video.url,
            video.subtitles.len()
        );
    }
    assert!(
        videos
            .iter()
            .all(|video| video.url.starts_with("https://www.nps.gov/"))
    );
    let smallest = videos.last().unwrap();
    assert_served("video", &smallest.url, &smallest.headers).await;
    if let Some(captions) = videos[0].subtitles.first() {
        assert_served("captions", &captions.url, &BTreeMap::new()).await;
    }
}

const ESA_HUBBLE: &str = "dev.raygun.esa-hubble/hubble";
const ESA_WEBB: &str = "dev.raygun.esa-hubble/webb";

/// The first of a list's videos that opens (one under another licence is refused), with its
/// details' text.
async fn first_opening(
    host: &ExtensionHost,
    provider: &str,
    listing: &MangaListing,
) -> (String, String) {
    for item in listing.items.iter().take(8) {
        match host.manga_details(provider, &item.id).await {
            Ok(details) => return (item.id.clone(), details.description.unwrap_or_default()),
            Err(error) => println!("  {} refused: {error}", item.id),
        }
    }
    panic!("none of {provider}'s first videos opens");
}

/// ESA/Hubble's and ESA/Webb's videos: lists, search in a category, a video's credit and CC BY
/// 4.0 licence, its MP4 and subtitles served, and a video under another licence refused.
#[tokio::test]
#[ignore = "needs network access to esahubble.org and esawebb.org"]
async fn open_sources_live_esa_hubble() {
    let host = keyed_host("esa-hubble", "dev.raygun.esa-hubble", None).await;
    let popular = host
        .manga_list(ESA_HUBBLE, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("ESA/Hubble's videos", &popular);
    assert!(popular.has_next_page);
    let latest = host
        .manga_list(ESA_HUBBLE, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("ESA/Hubble's newest", &latest);
    let orion = host
        .manga_search_filtered(
            ESA_HUBBLE,
            "orion",
            1,
            &BTreeMap::from([("category".to_owned(), select("nebulae"))]),
        )
        .await
        .unwrap();
    assert_listing("Orion among the nebulae", &orion);

    let (id, description) = first_opening(&host, ESA_HUBBLE, &latest).await;
    println!("{id}:\n{description}");
    assert!(description.contains("Credit: "));
    assert!(description.contains("Licence: CC BY 4.0 (https://esahubble.org/copyright/)."));
    let servers = host.video_servers(ESA_HUBBLE, &id).await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    for video in videos {
        println!("  {:?} {:?} {}", video.label, video.quality, video.url);
    }
    assert!(
        videos
            .iter()
            .all(|video| video.url.starts_with("https://cdn.esahubble.org/"))
    );
    let full_hd = videos
        .iter()
        .find(|video| video.quality == Some(1080))
        .unwrap_or(&videos[0]);
    assert_served("video", &full_hd.url, &full_hd.headers).await;

    // A Hubblecast with subtitles in several languages.
    let hubblecast = host.video_servers(ESA_HUBBLE, "heic1717a").await.unwrap();
    let subtitles = &hubblecast[0].videos.as_ref().unwrap()[0].subtitles;
    println!(
        "heic1717a subtitles: {:?}",
        subtitles
            .iter()
            .map(|track| &track.label)
            .collect::<Vec<_>>()
    );
    let english = subtitles
        .iter()
        .find(|track| track.lang.as_deref() == Some("en"))
        .expect("English subtitles");
    assert_served("subtitles", &english.url, &BTreeMap::new()).await;

    // "Eyes on the Skies" is released under CC BY-ND.
    let refused = host
        .manga_details(ESA_HUBBLE, "eyes_skies_full")
        .await
        .unwrap_err();
    assert!(refused.contains("licence other than CC BY"), "{refused}");

    let webb = host
        .manga_list(ESA_WEBB, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("ESA/Webb's newest", &webb);
    let (id, description) = first_opening(&host, ESA_WEBB, &webb).await;
    println!("{id}:\n{description}");
    assert!(description.contains("Licence: CC BY 4.0 (https://esawebb.org/copyright/)."));
    let servers = host.video_servers(ESA_WEBB, &id).await.unwrap();
    let video = &servers[0].videos.as_ref().expect("resolved at once")[0];
    assert!(video.url.starts_with("https://cdn.esawebb.org/"));
    assert_served("Webb video", &video.url, &video.headers).await;
}

/// ListenBrainz's recommendations and genre radios, all without an account.
#[tokio::test]
#[ignore = "needs network access to api.listenbrainz.org, labs.api.listenbrainz.org and musicbrainz.org"]
async fn open_sources_live_listenbrainz() {
    let host = host().await;
    let similar = host.similar_artists("Radiohead", 8).await;
    println!("like Radiohead: {similar:?}");
    assert!(similar.len() >= 5);
    assert!(similar.iter().all(|name| name != "Radiohead"));

    let names = |tracks: &[OnlineTrack]| {
        tracks
            .iter()
            .map(|track| {
                format!(
                    "{} — {}",
                    track.artist.as_deref().unwrap_or("?"),
                    track.title
                )
            })
            .collect::<Vec<_>>()
    };
    let top = host
        .recommended_tracks(
            "topTracks",
            serde_json::json!({ "artist": "Radiohead", "limit": 10 }),
        )
        .await;
    println!("Radiohead's best known: {:?}", names(&top));
    assert!(top.len() >= 8);
    assert!(
        top.iter()
            .all(|track| track.artist.as_deref() == Some("Radiohead"))
    );
    let song_radio = host
        .recommended_tracks(
            "radio",
            serde_json::json!({ "artist": "Radiohead", "title": "Creep", "limit": 10 }),
        )
        .await;
    println!("like Creep: {:?}", names(&song_radio));
    assert!(song_radio.len() >= 8);
    let artist_radio = host
        .recommended_tracks(
            "radio",
            serde_json::json!({ "artist": "Björk", "title": null, "limit": 12 }),
        )
        .await;
    println!("Björk radio: {:?}", names(&artist_radio));
    assert!(artist_radio.len() >= 8);
    let mut artists: Vec<_> = artist_radio
        .iter()
        .filter_map(|track| track.artist.clone())
        .collect();
    artists.sort_unstable();
    artists.dedup();
    assert!(artists.len() >= 4, "a radio mixes artists");

    // An artist with a release this month: fresh releases, as the site lists them.
    let fresh: serde_json::Value = serde_json::from_str(
        &raygun_net::http_client()
            .unwrap()
            .get("https://api.listenbrainz.org/1/explore/fresh-releases/?days=14&past=true&future=false")
            .send()
            .await
            .unwrap()
            .text()
            .await
            .unwrap(),
    )
    .unwrap();
    let candidates: Vec<String> = fresh["payload"]["releases"]
        .as_array()
        .unwrap()
        .iter()
        .filter(|release| {
            release["artist_mbids"]
                .as_array()
                .is_some_and(|ids| ids.len() == 1)
        })
        .filter_map(|release| release["artist_credit_name"].as_str().map(str::to_owned))
        .filter(|name| name.len() > 6)
        .take(5)
        .collect();
    let mut found = Vec::new();
    for artist in &candidates {
        found = host
            .recommended_tracks(
                "newReleases",
                serde_json::json!({ "artist": artist, "days": 28, "limit": 6 }),
            )
            .await;
        println!("{artist}'s new songs: {:?}", names(&found));
        if !found.is_empty() {
            break;
        }
    }
    assert!(!found.is_empty(), "none of {candidates:?} had new songs");

    let radio = host
        .music_tag_songs("Jazz", &["jazz".to_owned()], "genre", None, 10)
        .await;
    let (_, answer) = radio
        .iter()
        .find(|(provider, _)| provider.extension_id == "dev.raygun.listenbrainz")
        .expect("ListenBrainz answers genres");
    let (_, songs) = answer.as_ref().unwrap();
    println!("jazz radio: {:?}", names(songs));
    assert!(songs.len() >= 8);
}

// ---- Internet Archive books, open webcomics, PeerTube ----

const ARCHIVE_BOOKS: &str = "dev.raygun.archive-books/books";
const WEBCOMICS: &str = "dev.raygun.open-webcomics/webcomics";
const PEERTUBE: &str = "dev.raygun.peertube/videos";

#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn open_sources_live_archive_books() {
    let host = host().await;
    let popular = host
        .manga_list(ARCHIVE_BOOKS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("most read", &popular);
    assert!(popular.has_next_page);
    let latest = host
        .manga_list(ARCHIVE_BOOKS, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("recently added", &latest);
    let found = host
        .manga_search(ARCHIVE_BOOKS, "pride and prejudice", 1)
        .await
        .unwrap();
    assert_listing("search pride and prejudice", &found);
    let french = host
        .manga_search_filtered(
            ARCHIVE_BOOKS,
            "",
            1,
            &BTreeMap::from([
                ("language".to_owned(), select("fre")),
                ("library".to_owned(), select("toronto")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("French books from Canadian libraries", &french);

    for id in [&popular.items[0].id, &found.items[0].id] {
        let details = host.manga_details(ARCHIVE_BOOKS, id).await.unwrap();
        let description = details.description.unwrap_or_default();
        println!(
            "{id}: {:?} by {:?}\n{description}",
            details.title, details.authors
        );
        assert!(description.contains("Public domain in the United States"));
        let chapters = host.manga_chapters(ARCHIVE_BOOKS, id).await.unwrap();
        assert!(!chapters.is_empty());
        let blocks = host
            .novel_passage(ARCHIVE_BOOKS, &chapters[0].id)
            .await
            .unwrap();
        let [PassageBlock::Pdf(pdf)] = blocks.as_slice() else {
            panic!("{id}: not a PDF: {blocks:?}");
        };
        assert_served(id, &pdf.url, &pdf.headers).await;
    }
}

#[tokio::test]
#[ignore = "needs network access to www.sandraandwoo.com, turnoff.us and www.davidrevoy.com"]
async fn open_sources_live_open_webcomics() {
    let host = host().await;
    let popular = host
        .manga_list(WEBCOMICS, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("series", &popular);
    for (series, at_least) in [
        ("sandra-and-woo", 1300),
        ("turnoff", 300),
        ("mini-fantasy-theater", 78),
    ] {
        let details = host.manga_details(WEBCOMICS, series).await.unwrap();
        assert!(
            details
                .description
                .unwrap_or_default()
                .contains("Licence: Creative Commons")
        );
        let chapters = host.manga_chapters(WEBCOMICS, series).await.unwrap();
        println!(
            "{series}: {} strips, newest {:?}",
            chapters.len(),
            chapters.first().map(|chapter| &chapter.name)
        );
        assert!(chapters.len() >= at_least, "{series}: {}", chapters.len());
        for chapter in [chapters.first().unwrap(), chapters.last().unwrap()] {
            let pages = host.manga_pages(WEBCOMICS, &chapter.id).await.unwrap();
            assert!(!pages.is_empty(), "{}", chapter.id);
            assert_served(&chapter.id, &pages[0].url, &pages[0].headers).await;
        }
    }
}

#[tokio::test]
#[ignore = "needs network access to sepiasearch.org and PeerTube instances"]
async fn open_sources_live_peertube() {
    let host = host().await;
    let popular = host
        .manga_list(PEERTUBE, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_listing("trending", &popular);
    assert!(popular.has_next_page);
    let latest = host
        .manga_list(PEERTUBE, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_listing("newest", &latest);
    let films = host
        .manga_search_filtered(
            PEERTUBE,
            "blender",
            1,
            &BTreeMap::from([
                ("licence".to_owned(), select("commercial")),
                ("category".to_owned(), select("2")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("Blender films anyone may use", &films);

    // Instances go down now and then: the first few videos that answer.
    let mut played = 0;
    for item in films.items.iter().chain(&popular.items).take(8) {
        let details = match host.manga_details(PEERTUBE, &item.id).await {
            Ok(details) => details,
            Err(error) => {
                println!("{}: {error}", item.id);
                continue;
            }
        };
        let description = details.description.unwrap_or_default();
        assert!(description.contains("Licence: "), "{description}");
        let servers = host.video_servers(PEERTUBE, &item.id).await.unwrap();
        let videos = servers[0].videos.as_ref().expect("resolved at once");
        println!(
            "{} ({}): {:?}",
            details.title,
            servers[0].name,
            videos
                .iter()
                .map(|video| (&video.label, video.subtitles.len()))
                .collect::<Vec<_>>()
        );
        assert_served(&item.id, &videos[0].url, &videos[0].headers).await;
        let comments = host.video_comments(PEERTUBE, &item.id, None).await.unwrap();
        println!("  {} comments", comments.comments.len());
        played += 1;
        if played == 3 {
            break;
        }
    }
    assert!(played >= 2, "only {played} videos answered");
}

// ---- Great 78 Project, UCSB cylinders, NASA SVS, ESO and NOIRLab ----

const GREAT_78: &str = "dev.raygun.great-78";
const GREAT_78_CHARTS: &str = "dev.raygun.great-78/charts";
const CYLINDERS: &str = "dev.raygun.ucsb-cylinders";
const CYLINDER_CHARTS: &str = "dev.raygun.ucsb-cylinders/charts";
const SVS: &str = "dev.raygun.nasa-svs/videos";
const ESO: &str = "dev.raygun.eso/eso";
const NOIRLAB: &str = "dev.raygun.eso/noirlab";

/// Great 78 Project sides recorded before the cutoff, found, charted and served.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn open_sources_live_great_78() {
    let host = host().await;
    let tracks = songs_from(&host, GREAT_78, "caruso").await;
    assert!(!tracks.is_empty());
    let (url, headers) = url_of(&tracks[0]);
    assert!(url.starts_with("https://archive.org/download/78_"), "{url}");
    assert_served("78", url, headers).await;
    let charts = charts_of(&host, GREAT_78).await;
    assert!(charts.iter().any(|chart| chart.group == ChartGroup::Decade));
    for chart in ["top", "genre:jazz", "decade:1910"] {
        let songs = host
            .music_chart(GREAT_78_CHARTS, chart, None, 10)
            .await
            .unwrap();
        println!(
            "{chart}: {:?}",
            songs.iter().map(|song| &song.title).collect::<Vec<_>>()
        );
        assert!(songs.len() >= 5, "{chart}: {}", songs.len());
    }
}

/// UCSB cylinders released before the cutoff: a search, a chart, and an MP3 served.
#[tokio::test]
#[ignore = "needs network access to cylinders.library.ucsb.edu"]
async fn open_sources_live_ucsb_cylinders() {
    let host = host().await;
    let tracks = songs_from(&host, CYLINDERS, "ragtime").await;
    assert!(tracks.len() >= 3, "{}", tracks.len());
    let (url, headers) = url_of(&tracks[0]);
    assert!(url.starts_with("https://cylinders.s3"), "{url}");
    assert_served("cylinder", url, headers).await;
    let charts = charts_of(&host, CYLINDERS).await;
    assert!(charts.len() >= 5);
    let opera = host
        .music_chart(CYLINDER_CHARTS, "opera", None, 10)
        .await
        .unwrap();
    assert!(opera.len() >= 5, "{}", opera.len());
}

/// NASA SVS: popular and filtered lists with pictures, a page's videos and captions.
#[tokio::test]
#[ignore = "needs network access to svs.gsfc.nasa.gov"]
async fn open_sources_live_nasa_svs() {
    let host = host().await;
    let popular = host.manga_list(SVS, MangaList::Popular, 1).await.unwrap();
    assert_listing("most viewed", &popular);
    assert!(popular.has_next_page);
    let earth = host
        .manga_search_filtered(
            SVS,
            "hurricane",
            1,
            &BTreeMap::from([
                ("category".to_owned(), select("Earth")),
                ("type".to_owned(), select("visualization")),
            ]),
        )
        .await
        .unwrap();
    assert_listing("hurricanes", &earth);
    // The first page that opens (music refuses some) plays from the SVS.
    let mut played = false;
    for item in popular.items.iter().take(6) {
        let Ok(details) = host.manga_details(SVS, &item.id).await else {
            continue;
        };
        assert!(
            details
                .description
                .unwrap_or_default()
                .contains("Public domain (NASA media)")
        );
        let episodes = host.video_episodes(SVS, &item.id).await.unwrap();
        let servers = host.video_servers(SVS, &episodes[0].id).await.unwrap();
        let videos = servers[0].videos.as_ref().expect("resolved at once");
        println!(
            "{}: {} files, best {}",
            item.id,
            videos.len(),
            videos[0].url
        );
        assert!(videos[0].url.starts_with("https://svs.gsfc.nasa.gov/vis/"));
        assert_served("svs", &videos[0].url, &videos[0].headers).await;
        played = true;
        break;
    }
    assert!(played, "no popular page opened");
}

/// ESO and NOIRLab: lists, a video's CC BY details and its files.
#[tokio::test]
#[ignore = "needs network access to www.eso.org and noirlab.edu"]
async fn open_sources_live_eso() {
    let host = host().await;
    for (provider, at_least) in [(ESO, 4), (NOIRLAB, 4)] {
        let popular = host
            .manga_list(provider, MangaList::Popular, 1)
            .await
            .unwrap();
        assert_listing(provider, &popular);
        assert!(popular.items.len() >= at_least);
        let latest = host
            .manga_list(provider, MangaList::Latest, 1)
            .await
            .unwrap();
        assert_listing(provider, &latest);
        let (_, description) = first_opening(&host, provider, &latest).await;
        assert!(description.contains("Licence: CC BY 4.0"), "{description}");
        // The first video with files of its own (some are on YouTube only) plays from the site.
        let mut played = false;
        for item in latest.items.iter().take(8) {
            let Ok(servers) = host.video_servers(provider, &item.id).await else {
                continue;
            };
            let videos = servers[0].videos.as_ref().expect("resolved at once");
            println!(
                "{provider} {}: {:?}",
                item.id,
                videos.iter().map(|video| &video.label).collect::<Vec<_>>()
            );
            assert_served(provider, &videos[0].url, &videos[0].headers).await;
            played = true;
            break;
        }
        assert!(played, "{provider}: none of the newest videos has a file");
    }
}
