//! More open sources against local fixture servers (the real bundles, their API bases pointed at
//! this machine): the Internet Archive's public-domain books as PDFs, Creative Commons webcomics,
//! and PeerTube's openly licensed videos through Sepia Search.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::path::Path;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use chrono::Datelike;
use raygun_model::{ExtensionStatus, FilterValue};
use serde_json::{Value, json};

use crate::fixture_tests::{json_response, serve};
use crate::manga::MangaList;
use crate::novel::PassageBlock;
use crate::{ExtensionHost, ExtensionSource, HostConfig};

/// The requests a fixture server saw, as their paths and queries.
type Asked = Arc<Mutex<Vec<String>>>;

/// A first-party extension with every address in `bases` pointed at `address` and its network
/// access limited to it, loaded into a new host.
async fn local_host(name: &str, bases: &[&str], address: SocketAddr) -> ExtensionHost {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions")
        .join(name);
    let mut manifest: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("raygun-extension.json")).unwrap())
            .unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    let mut code = std::fs::read_to_string(dir.join("dist/index.js")).unwrap();
    for base in bases {
        assert!(code.contains(base), "{name} bundle doesn't use {base}");
        code = code.replace(base, &format!("http://{address}"));
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load(vec![ExtensionSource::built_in(&[
            ("raygun-extension.json", &manifest.to_string()),
            ("dist/index.js", &code),
        ])])
        .await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    host
}

fn filters(pairs: &[(&str, &str)]) -> BTreeMap<String, FilterValue> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), FilterValue::Select((*value).to_owned())))
        .collect()
}

fn html(body: String) -> Response {
    ([(header::CONTENT_TYPE, "text/html; charset=utf-8")], body).into_response()
}

/// A router that answers `answer(path and query)` and records every request.
fn recording(answer: impl Fn(&str) -> Response + Clone + Send + Sync + 'static) -> (Router, Asked) {
    let asked: Asked = Arc::default();
    let requests = asked.clone();
    let app = Router::new().fallback(move |uri: Uri| {
        let requests = requests.clone();
        let answer = answer.clone();
        async move {
            let address = uri
                .path_and_query()
                .map(ToString::to_string)
                .unwrap_or_default();
            requests.lock().unwrap().push(address.clone());
            answer(&address)
        }
    });
    (app, asked)
}

/// The decoded query of the last request whose path starts with `path`.
fn last_query(asked: &Asked, path: &str) -> String {
    let requests = asked.lock().unwrap();
    let address = requests
        .iter()
        .rev()
        .find(|address| address.starts_with(path))
        .unwrap_or_else(|| panic!("nothing asked for {path}: {requests:?}"));
    let query = address
        .split_once('?')
        .map(|(_, query)| query)
        .unwrap_or("");
    url::form_urlencoded::parse(query.as_bytes())
        .map(|(key, value)| format!("{key}={value}"))
        .collect::<Vec<_>>()
        .join("&")
}

// ---- Internet Archive books ----

const ARCHIVE_BOOKS: &str = "dev.raygun.archive-books/books";
const WHITE_NIGHTS: &str = "whitenightsother00dostiala";
const DICTIONARY: &str = "anewenglishhind00fallgoog";

fn archive_item(date: &str, collections: &[&str], extra: Value, files: Value) -> Value {
    let mut metadata = json!({
        "identifier": "x",
        "mediatype": "texts",
        "date": date,
        "collection": collections,
        "possible-copyright-status": "NOT_IN_COPYRIGHT",
    });
    for (key, value) in extra.as_object().unwrap() {
        metadata[key] = value.clone();
    }
    json!({ "metadata": metadata, "files": files })
}

fn archive_answer(path: &str) -> Response {
    let ok = |value: Value| json_response(StatusCode::OK, value);
    if path.starts_with("/advancedsearch.php") {
        return ok(json!({ "response": { "numFound": 61, "docs": [
            {
                "identifier": WHITE_NIGHTS,
                "title": "White nights, and other stories",
                "creator": ["Dostoyevsky, Fyodor, 1821-1881", "Garnett, Constance Black, 1862-1946"],
            },
            {
                "identifier": "meditationsofmar00marc",
                "title": "The meditations of Marcus Aurelius",
                "creator": "Marcus Aurelius, Emperor of Rome, 121-180",
            },
            { "identifier": DICTIONARY, "title": "A new English-Hindustani dictionary" },
        ] } }));
    }
    let item = match path.strip_prefix("/metadata/").unwrap_or_default() {
        WHITE_NIGHTS => archive_item(
            "1918",
            &["cdl", "americana"],
            json!({
                "title": "White nights, and other stories",
                "creator": ["Dostoyevsky, Fyodor, 1821-1881", "Garnett, Constance Black, 1862-1946"],
                "description": "<p>Translated from the Russian by Constance Garnett.</p>",
                "subject": "Russian fiction; Short stories",
                "publisher": "New York : The Macmillan Company",
                "contributor": "University of California Libraries",
                "language": "eng",
                "imagecount": "308",
                "volume": "c. 2",
            }),
            json!([
                { "name": "whitenightsother00dostiala.epub", "format": "EPUB" },
                { "name": "whitenightsother00dostiala.pdf", "format": "Text PDF" },
                { "name": "whitenightsother00dostiala_bw.pdf", "format": "Grayscale PDF" },
                { "name": "whitenightsother00dostiala_djvu.txt", "format": "DjVuTXT" },
            ]),
        ),
        DICTIONARY => archive_item(
            "1883",
            &["europeanlibraries"],
            json!({ "title": "A new English-Hindustani dictionary", "language": ["eng", "hin"] }),
            json!([
                { "name": "anewenglishhind00fallgoog.pdf", "format": "Image Container PDF" },
                { "name": "anewenglishhind00fallgoog_text.pdf", "format": "Additional Text PDF" },
            ]),
        ),
        "historyofengland03hume" => archive_item(
            "[c1880]",
            &["toronto"],
            json!({ "title": "The history of England", "volume": "v. 3" }),
            json!([
                { "name": "part2/historyofengland03hume_2.pdf", "format": "Text PDF" },
                { "name": "part10/historyofengland03hume_10.pdf", "format": "Text PDF", "title": "Part ten" },
            ]),
        ),
        "recentbook" => archive_item(
            "1950",
            &["americana"],
            json!({ "title": "Recent" }),
            json!([]),
        ),
        "lentbook" => archive_item(
            "1920",
            &["americana"],
            json!({ "title": "Lent", "access-restricted-item": "true" }),
            json!([]),
        ),
        "uploadedbook" => archive_item(
            "1900",
            &["opensource"],
            json!({ "title": "Mine" }),
            json!([]),
        ),
        "nopdf" => archive_item(
            "1900",
            &["americana"],
            json!({ "title": "Text only" }),
            json!([{ "name": "nopdf_djvu.txt", "format": "DjVuTXT" }]),
        ),
        _ => json!({}),
    };
    ok(item)
}

#[tokio::test]
async fn archive_books_lists_public_domain_scans_and_reads_their_pdfs() {
    let (app, asked) = recording(archive_answer);
    let address = serve(app);
    let host = local_host("archive-books", &["https://archive.org"], address).await;
    let cutoff = chrono::Utc::now().year() - 96;

    let popular = host
        .manga_list(ARCHIVE_BOOKS, MangaList::Popular, 1)
        .await
        .unwrap();
    let listed: Vec<_> = popular
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.authors.clone()))
        .collect();
    assert_eq!(
        listed,
        [
            (
                WHITE_NIGHTS,
                vec![
                    "Fyodor Dostoyevsky".to_owned(),
                    "Constance Black Garnett".to_owned()
                ]
            ),
            (
                "meditationsofmar00marc",
                vec!["Marcus Aurelius, Emperor of Rome".to_owned()]
            ),
            (DICTIONARY, vec![]),
        ]
    );
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/services/img/{WHITE_NIGHTS}").as_str())
    );
    assert!(popular.has_next_page, "61 found, 30 a page");
    let asked_for = last_query(&asked, "/advancedsearch.php");
    for part in [
        "mediatype:texts",
        "possible-copyright-status:\"NOT_IN_COPYRIGHT\"",
        &format!("year:[1 TO {cutoff}]"),
        "NOT access-restricted-item:true",
        "format:PDF",
        "collection:(americana OR toronto OR europeanlibraries",
        "sort[]=downloads desc",
    ] {
        assert!(asked_for.contains(part), "{part}: {asked_for}");
    }

    host.manga_search_filtered(
        ARCHIVE_BOOKS,
        "white nights",
        2,
        &filters(&[("language", "eng"), ("library", "cdl")]),
    )
    .await
    .unwrap();
    let asked_for = last_query(&asked, "/advancedsearch.php");
    for part in [
        "(title:(white nights) OR creator:(white nights) OR subject:(white nights))",
        "language:(eng OR \"English\")",
        "collection:(cdl)",
        "page=2",
    ] {
        assert!(asked_for.contains(part), "{part}: {asked_for}");
    }
    assert!(!asked_for.contains("sort[]"), "words go by relevance");

    // A book's page: its facts, its text and why it's free to read.
    let details = host
        .manga_details(ARCHIVE_BOOKS, WHITE_NIGHTS)
        .await
        .unwrap();
    assert_eq!(details.title, "White nights, and other stories");
    assert_eq!(
        details.authors,
        ["Fyodor Dostoyevsky", "Constance Black Garnett"]
    );
    let description = details.description.unwrap();
    for part in [
        "Published 1918 · New York : The Macmillan Company · 308 scanned pages\n\n",
        "Translated from the Russian by Constance Garnett.",
        "Public domain in the United States: published more than 95 years ago, and marked not in \
         copyright by the Internet Archive. Scanned from a copy at University of California \
         Libraries.",
    ] {
        assert!(description.contains(part), "{part}: {description}");
    }
    assert!(
        !description.contains("Volume"),
        "\"c. 2\" is a copy, not a volume"
    );
    assert_eq!(
        details.genres,
        ["Russian fiction", "Short stories", "English"]
    );

    // One PDF, the one with the scans' text; the passage is the document itself.
    let chapters = host
        .manga_chapters(ARCHIVE_BOOKS, WHITE_NIGHTS)
        .await
        .unwrap();
    assert_eq!(chapters.len(), 1);
    assert_eq!(chapters[0].name, "White nights, and other stories");
    let blocks = host
        .novel_passage(ARCHIVE_BOOKS, &chapters[0].id)
        .await
        .unwrap();
    let [PassageBlock::Pdf(pdf)] = blocks.as_slice() else {
        panic!("a PDF passage: {blocks:?}");
    };
    assert_eq!(
        pdf.url,
        format!("http://{address}/download/{WHITE_NIGHTS}/whitenightsother00dostiala.pdf")
    );

    // A Google scan: its PDF with text rather than the picture-only one.
    let chapters = host
        .manga_chapters(ARCHIVE_BOOKS, DICTIONARY)
        .await
        .unwrap();
    assert_eq!(
        chapters
            .iter()
            .map(|chapter| chapter.id.as_str())
            .collect::<Vec<_>>(),
        [format!("{DICTIONARY}|anewenglishhind00fallgoog_text.pdf").as_str()]
    );

    // Volumes in one item: each a chapter, in order, named by its title or its file.
    let details = host
        .manga_details(ARCHIVE_BOOKS, "historyofengland03hume")
        .await
        .unwrap();
    assert!(
        details
            .description
            .unwrap()
            .starts_with("Published 1880 · Volume 3")
    );
    let chapters = host
        .manga_chapters(ARCHIVE_BOOKS, "historyofengland03hume")
        .await
        .unwrap();
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| (chapter.name.as_str(), chapter.number))
        .collect();
    assert_eq!(
        names,
        [
            ("historyofengland03hume 2", Some(1.0)),
            ("Part ten", Some(2.0))
        ]
    );
    let blocks = host
        .novel_passage(ARCHIVE_BOOKS, &chapters[1].id)
        .await
        .unwrap();
    assert!(
        matches!(&blocks[..], [PassageBlock::Pdf(pdf)] if pdf.url.ends_with("/download/historyofengland03hume/part10/historyofengland03hume_10.pdf")),
        "{blocks:?}"
    );

    // Books that aren't free to read are refused, however they're reached.
    for (id, why) in [
        ("recentbook", "doesn't list this book as public domain"),
        ("uploadedbook", "doesn't list this book as public domain"),
        ("lentbook", "lends this book only to signed-in readers"),
        ("missing", "has no such book"),
    ] {
        let refused = host.manga_details(ARCHIVE_BOOKS, id).await.unwrap_err();
        assert!(refused.contains(why), "{id}: {refused}");
        let refused = host
            .novel_passage(ARCHIVE_BOOKS, &format!("{id}|{id}.pdf"))
            .await
            .unwrap_err();
        assert!(refused.contains(why), "{id}: {refused}");
    }
    let none = host
        .manga_chapters(ARCHIVE_BOOKS, "nopdf")
        .await
        .unwrap_err();
    assert!(none.contains("has no PDF of this book"), "{none}");
    let gone = host
        .novel_passage(ARCHIVE_BOOKS, &format!("{WHITE_NIGHTS}|elsewhere.pdf"))
        .await
        .unwrap_err();
    assert!(gone.contains("no longer in the book"), "{gone}");
}

// ---- Open webcomics ----

const WEBCOMICS: &str = "dev.raygun.open-webcomics/webcomics";

fn sandra_and_woo_posts(page: u32) -> Value {
    match page {
        1 => json!([
            { "id": 3836, "date_gmt": "2000-01-01T00:00:00", "title": { "rendered": "Welcome to Sandra and Woo" } },
            { "id": 18, "date_gmt": "2008-10-19T00:00:00", "title": { "rendered": "[0001] A Sly Raccoon" } },
            { "id": 19, "date_gmt": "2008-10-26T00:00:00", "title": { "rendered": "[0002] Woo&#8217;s Day" } },
        ]),
        _ => json!([
            { "id": 12114, "date_gmt": "2023-08-12T19:15:44", "title": { "rendered": "[1340] MH370 until proven otherwise" } },
            { "id": 12115, "date_gmt": "2023-08-12T19:15:44", "title": { "rendered": "[1340] A repeat" } },
        ]),
    }
}

fn webcomics_answer(path: &str) -> Response {
    if path.starts_with("/wp-json/wp/v2/posts") {
        let page = if path.contains("&page=2&") { 2 } else { 1 };
        let mut response = json_response(StatusCode::OK, sandra_and_woo_posts(page));
        response
            .headers_mut()
            .insert("x-wp-totalpages", "2".parse().unwrap());
        return response;
    }
    match path {
        "/?p=12114" => html(
            r#"<html><body><div id="comic-head"></div><div id="comic">
            <img src="/comics/2023-08-12-1340-mh370-until-proven-otherwise.png" alt="[1340] MH370" />
            </div><div class="entry"><img src="/images/ad.png"></div></body></html>"#
                .to_owned(),
        ),
        "/?p=3836" => html("<html><body><p>Welcome!</p></body></html>".to_owned()),
        "/all/" => html(
            r#"<html><body><img src="/image/logo.png"><ul class="post-list">
            <li><a class="post-link" href="/geek/bsod/">Blue Screen of Death</a></li>
            <li><a class="post-link" href="/geek/dont-share-a-pipe/">don&#39;t share a pipe</a></li>
            <li><a class="post-link" href="/geek/tcp-buddies/">tcp buddies</a></li>
            <li><a class="post-link" href="https://store.turnoff.us/">Store</a></li>
            </ul></body></html>"#
                .to_owned(),
        ),
        "/geek/bsod/" => html(
            r#"<html><head><meta property="og:image" content="http://turnoff.us/image/en/bsod.jpeg"></head>
            <body><img src="/image/logo.png" width="60" /><div class="post-content">
            <img src="/image/en/bsod.jpeg" alt="Blue Screen of Death geek comic" />
            <img src="/image/en/bsod.jpeg" alt="again" /></div>
            <a href="https://store.turnoff.us"><img src="/image/store.png"/></a></body></html>"#
                .to_owned(),
        ),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::test]
async fn open_webcomics_lists_both_series_and_their_strips() {
    let (app, asked) = recording(webcomics_answer);
    let address = serve(app);
    let host = local_host(
        "open-webcomics",
        &["https://www.sandraandwoo.com", "https://turnoff.us"],
        address,
    )
    .await;

    let popular = host
        .manga_list(WEBCOMICS, MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["sandra-and-woo", "turnoff"]);
    assert!(!popular.has_next_page);
    let found = host.manga_search(WEBCOMICS, "raccoon", 1).await.unwrap();
    assert_eq!(found.items.len(), 1);
    assert_eq!(found.items[0].title, "Sandra and Woo");
    assert!(
        host.manga_search(WEBCOMICS, "screen of death", 1)
            .await
            .unwrap()
            .items
            .is_empty(),
        "strip names match once their list was read"
    );

    // Sandra and Woo: every page of the API, numbered strips only, newest first, each once.
    let chapters = host
        .manga_chapters(WEBCOMICS, "sandra-and-woo")
        .await
        .unwrap();
    let listed: Vec<_> = chapters
        .iter()
        .map(|chapter| (chapter.id.as_str(), chapter.name.as_str(), chapter.number))
        .collect();
    assert_eq!(
        listed,
        [
            (
                "sandra-and-woo/12114",
                "MH370 until proven otherwise",
                Some(1340.0)
            ),
            ("sandra-and-woo/19", "Woo\u{2019}s Day", Some(2.0)),
            ("sandra-and-woo/18", "A Sly Raccoon", Some(1.0)),
        ]
    );
    assert_eq!(chapters[2].date_upload, Some(1_224_374_400_000));
    let query = last_query(&asked, "/wp-json/wp/v2/posts");
    assert!(query.contains("categories=1"), "{query}");
    assert!(query.contains("per_page=100"), "{query}");
    let pages = host
        .manga_pages(WEBCOMICS, "sandra-and-woo/12114")
        .await
        .unwrap();
    let urls: Vec<_> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            format!("http://{address}/comics/2023-08-12-1340-mh370-until-proven-otherwise.png")
                .as_str()
        ]
    );
    let empty = host
        .manga_pages(WEBCOMICS, "sandra-and-woo/3836")
        .await
        .unwrap_err();
    assert!(empty.contains("no strip to show"), "{empty}");

    // turnoff.us: the list page's comics, the oldest numbered 1; a comic's own pictures.
    let chapters = host.manga_chapters(WEBCOMICS, "turnoff").await.unwrap();
    let listed: Vec<_> = chapters
        .iter()
        .map(|chapter| (chapter.id.as_str(), chapter.name.as_str(), chapter.number))
        .collect();
    assert_eq!(
        listed,
        [
            ("turnoff/bsod", "Blue Screen of Death", Some(3.0)),
            ("turnoff/dont-share-a-pipe", "don't share a pipe", Some(2.0)),
            ("turnoff/tcp-buddies", "tcp buddies", Some(1.0)),
        ]
    );
    let found = host
        .manga_search(WEBCOMICS, "screen of death", 1)
        .await
        .unwrap();
    assert_eq!(found.items.len(), 1);
    assert_eq!(found.items[0].id, "turnoff");
    let pages = host.manga_pages(WEBCOMICS, "turnoff/bsod").await.unwrap();
    let urls: Vec<_> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [format!("http://{address}/image/en/bsod.jpeg").as_str()]
    );
    let missing = host
        .manga_pages(WEBCOMICS, "turnoff/gone")
        .await
        .unwrap_err();
    assert!(missing.contains("no longer on the site"), "{missing}");

    let details = host.manga_details(WEBCOMICS, "turnoff").await.unwrap();
    assert_eq!(details.authors, ["Daniel Stori"]);
    assert!(
        details
            .description
            .unwrap()
            .contains("Creative Commons Attribution-NonCommercial-ShareAlike 4.0")
    );
    let unknown = host.manga_details(WEBCOMICS, "nothing").await.unwrap_err();
    assert!(unknown.contains("no such series"), "{unknown}");
}

// ---- PeerTube ----

const PEERTUBE: &str = "dev.raygun.peertube/videos";
const SPRING: &str = "3d95fb3d-c866-42c8-9db1-fe82f48ccb95";
const INTRO: &str = "9c9de5e8-0a1e-484a-b099-e80766180a6d";
const CLOSED: &str = "11111111-2222-4333-8444-555555555555";
const SENSITIVE: &str = "22222222-3333-4444-8555-666666666666";

fn sepia_video(origin: &str, uuid: &str, name: &str, licence: i64, nsfw: bool) -> Value {
    json!({
        "uuid": uuid,
        "url": format!("{origin}/videos/watch/{uuid}"),
        "name": name,
        "isLive": false,
        "nsfw": nsfw,
        "licence": { "id": licence, "label": "Attribution" },
        "thumbnailUrl": format!("{origin}/lazy-static/thumbnails/{uuid}.jpg"),
        "previewUrl": format!("{origin}/lazy-static/previews/{uuid}.jpg"),
        "channel": { "name": "blender", "displayName": "Blender Open Movies" },
    })
}

fn peertube_video(uuid: &str, licence: i64) -> Value {
    json!({
        "uuid": uuid,
        "name": "Spring - Blender Open Movie",
        "description": "**Spring** is the story of a shepherd girl and her dog.\r\n\r\nJoin the [Blender Development Fund](https://fund.blender.org/).\r\n![poster](/poster.png)",
        "duration": 464,
        "isLive": false,
        "nsfw": false,
        "publishedAt": "2019-06-26T14:54:03.643Z",
        "originallyPublishedAt": null,
        "privacy": { "id": 1, "label": "Public" },
        "licence": { "id": licence, "label": "Attribution - Share Alike" },
        "category": { "id": 2, "label": "Films" },
        "language": { "id": null, "label": "Unknown" },
        "tags": ["animation", "blender", "short film"],
        "previewPath": format!("/lazy-static/previews/{uuid}.jpg"),
        "channel": { "name": "blender_open_movies", "displayName": "Blender Open Movies", "host": "video.blender.org" },
        "account": { "name": "blender", "displayName": "Blender", "host": "video.blender.org" },
        "files": [
            { "resolution": { "id": 480, "label": "480p" }, "fileUrl": format!("https://cdn.example/{uuid}-480.mp4") },
            { "resolution": { "id": 1080, "label": "1080p" }, "fileUrl": format!("https://cdn.example/{uuid}-1080.mp4") },
            { "resolution": { "id": 0, "label": "Audio" }, "fileUrl": format!("https://cdn.example/{uuid}-0.mp4") },
        ],
        "streamingPlaylists": [{
            "type": 1,
            "playlistUrl": format!("https://cdn.example/hls/{uuid}/master.m3u8"),
            "files": [{ "resolution": { "id": 720 } }, { "resolution": { "id": 1080 } }],
        }],
    })
}

/// The fixture's answers, naming the server's own address (known once it's serving) as the
/// instance's.
fn peertube_answer(
    origin: Arc<Mutex<String>>,
) -> impl Fn(&str) -> Response + Clone + Send + Sync + 'static {
    move |path: &str| {
        let origin = origin.lock().unwrap().clone();
        let ok = |value: Value| json_response(StatusCode::OK, value);
        if path.starts_with("/api/v1/search/videos") {
            return ok(json!({ "total": 30, "data": [
                sepia_video(&origin, SPRING, "Spring - Blender Open Movie", 2, false),
                sepia_video(&origin, INTRO, "What is PeerTube?", 1, false),
                sepia_video(&origin, SENSITIVE, "Sensitive", 1, true),
                sepia_video(&origin, CLOSED, "Not open", 0, false),
                json!({ "uuid": "nothing", "url": "javascript:alert(1)", "name": "Bad", "licence": { "id": 1 } }),
            ] }));
        }
        let video = |uuid: &str, licence: i64| ok(peertube_video(uuid, licence));
        match path {
            p if p == format!("/api/v1/videos/{SPRING}") => video(SPRING, 2),
            p if p == format!("/api/v1/videos/{CLOSED}") => video(CLOSED, 0),
            p if p == format!("/api/v1/videos/{SENSITIVE}") => {
                let mut sensitive = peertube_video(SENSITIVE, 1);
                sensitive["nsfw"] = json!(true);
                ok(sensitive)
            }
            p if p == format!("/api/v1/videos/{SPRING}/captions") => {
                ok(json!({ "total": 2, "data": [
                { "language": { "id": "en", "label": "English" }, "captionPath": "/lazy-static/video-captions/a-en.vtt" },
                { "language": { "id": "fr", "label": "French" }, "fileUrl": "https://cdn.example/b-fr.vtt", "automaticallyGenerated": true },
            ] }))
            }
            p if p.starts_with(&format!("/api/v1/videos/{SPRING}/comment-threads")) => {
                ok(json!({ "total": 3, "data": [
                    {
                        "text": "<p><span class=\"h-card\"><a href=\"https://framatube.org/a/framasoft\">@<span>framasoft</span></a></span> Lovely &amp; calm</p><p>Thanks!</p>",
                        "isDeleted": false,
                        "createdAt": "2026-06-24T17:23:41.000Z",
                        "totalReplies": 2,
                        "account": { "name": "viewer", "displayName": "A viewer", "host": "mastodon.example", "avatars": [
                            { "path": "/lazy-static/avatars/big.png", "width": 600 },
                            { "path": "/lazy-static/avatars/small.png", "width": 48 },
                        ] },
                    },
                    { "text": "", "isDeleted": true, "createdAt": "2026-06-20T10:00:00.000Z", "totalReplies": 0 },
                ] }))
            }
            _ => StatusCode::NOT_FOUND.into_response(),
        }
    }
}

#[tokio::test]
async fn peertube_lists_openly_licensed_videos_and_plays_them_from_their_instance() {
    // Sepia Search and the instance are one local server, whose address is in the answers.
    let origin_cell: Arc<Mutex<String>> = Arc::default();
    let (app, asked) = recording(peertube_answer(origin_cell.clone()));
    let address = serve(app);
    let origin = format!("http://{address}");
    origin_cell.lock().unwrap().clone_from(&origin);
    let host = local_host("peertube", &["https://sepiasearch.org"], address).await;
    let spring = format!("{origin}/videos/watch/{SPRING}");

    let popular = host
        .manga_list(PEERTUBE, MangaList::Popular, 1)
        .await
        .unwrap();
    let listed: Vec<_> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        listed,
        ["Spring - Blender Open Movie", "What is PeerTube?"],
        "open licences only, no sensitive videos, http(s) addresses only"
    );
    assert_eq!(popular.items[0].id, spring);
    assert_eq!(popular.items[0].authors, ["Blender Open Movies"]);
    assert!(popular.has_next_page, "30 found, 24 a page");
    let query = last_query(&asked, "/api/v1/search/videos");
    for part in [
        "sort=-hot",
        "licenceOneOf=1&licenceOneOf=2&licenceOneOf=3&licenceOneOf=4&licenceOneOf=5&licenceOneOf=6&licenceOneOf=7",
        "start=0",
        "count=24",
        "nsfw=false",
        "isLive=false",
    ] {
        assert!(query.contains(part), "{part}: {query}");
    }
    host.manga_search_filtered(
        PEERTUBE,
        "blender",
        2,
        &filters(&[
            ("licence", "commercial"),
            ("category", "2"),
            ("language", "en"),
            ("duration", "long"),
        ]),
    )
    .await
    .unwrap();
    let query = last_query(&asked, "/api/v1/search/videos");
    for part in [
        "search=blender",
        "sort=-match",
        "licenceOneOf=1&licenceOneOf=2&licenceOneOf=3&licenceOneOf=7",
        "categoryOneOf=2",
        "languageOneOf=en",
        "durationMin=1201",
        "start=24",
    ] {
        assert!(query.contains(part), "{part}: {query}");
    }

    // The video's own instance: its text, who published it and under which licence.
    let details = host.manga_details(PEERTUBE, &spring).await.unwrap();
    assert_eq!(details.title, "Spring - Blender Open Movie");
    assert_eq!(details.media_type.as_deref(), Some("movie"));
    assert_eq!(details.year, Some(2019));
    assert_eq!(
        details.cover_url.as_deref(),
        Some(format!("{origin}/lazy-static/previews/{SPRING}.jpg").as_str())
    );
    assert_eq!(
        details.genres,
        ["Films", "animation", "blender", "short film"]
    );
    let description = details.description.unwrap();
    assert_eq!(
        description,
        format!(
            "Spring is the story of a shepherd girl and her dog.\n\nJoin the Blender Development \
             Fund.\n\nPublished 26 June 2019 · 7:44 · Films\n\nPublished on {} by Blender Open \
             Movies (blender_open_movies@video.blender.org), by Blender.\n\nLicence: Creative \
             Commons Attribution-ShareAlike (CC BY-SA), as its uploader set it.",
            origin.trim_start_matches("http://")
        )
    );
    let episodes = host.video_episodes(PEERTUBE, &spring).await.unwrap();
    assert_eq!(episodes.len(), 1);
    assert_eq!(episodes[0].name, "Film");
    assert_eq!(episodes[0].date_upload, Some(1_561_560_843_643));

    // One server, the instance: the HLS playlist first, then the files from the tallest.
    let servers = host.video_servers(PEERTUBE, &spring).await.unwrap();
    assert_eq!(servers.len(), 1);
    assert_eq!(servers[0].name, origin.trim_start_matches("http://"));
    let videos = servers[0].videos.as_ref().unwrap();
    let links: Vec<_> = videos
        .iter()
        .map(|video| (video.url.as_str(), video.format.as_deref(), video.quality))
        .collect();
    assert_eq!(
        links,
        [
            (
                format!("https://cdn.example/hls/{SPRING}/master.m3u8").as_str(),
                Some("hls"),
                Some(1080)
            ),
            (
                format!("https://cdn.example/{SPRING}-1080.mp4").as_str(),
                None,
                Some(1080)
            ),
            (
                format!("https://cdn.example/{SPRING}-480.mp4").as_str(),
                None,
                Some(480)
            ),
            (
                format!("https://cdn.example/{SPRING}-0.mp4").as_str(),
                None,
                None
            ),
        ]
    );
    let tracks: Vec<_> = videos[0]
        .subtitles
        .iter()
        .map(|track| {
            (
                track.url.as_str(),
                track.lang.as_deref(),
                track.label.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        tracks,
        [
            (
                format!("{origin}/lazy-static/video-captions/a-en.vtt").as_str(),
                Some("en"),
                Some("English")
            ),
            (
                "https://cdn.example/b-fr.vtt",
                Some("fr"),
                Some("French (automatic)")
            ),
        ]
    );

    // Comments: plain text, deleted ones left out, the next page while there are more.
    let comments = host.video_comments(PEERTUBE, &spring, None).await.unwrap();
    assert_eq!(comments.comments.len(), 1);
    let comment = &comments.comments[0];
    assert_eq!(comment.author, "A viewer");
    assert_eq!(comment.text, "@framasoft Lovely & calm\n\nThanks!");
    assert_eq!(comment.published.as_deref(), Some("2026-06-24"));
    assert_eq!(comment.replies, Some(2));
    assert_eq!(
        comment.avatar_url.as_deref(),
        Some(format!("{origin}/lazy-static/avatars/small.png").as_str())
    );
    assert_eq!(comments.next.as_deref(), Some("2"));
    let query = last_query(&asked, &format!("/api/v1/videos/{SPRING}/comment-threads"));
    assert!(
        query.contains("start=0&count=20&sort=-createdAt"),
        "{query}"
    );

    // A licence that isn't open any more, a sensitive video, an address that isn't a video.
    let closed = host
        .manga_details(PEERTUBE, &format!("{origin}/videos/watch/{CLOSED}"))
        .await
        .unwrap_err();
    assert!(
        closed.contains("doesn't publish it under an open licence"),
        "{closed}"
    );
    let sensitive = host
        .video_servers(PEERTUBE, &format!("{origin}/videos/watch/{SENSITIVE}"))
        .await
        .unwrap_err();
    assert!(sensitive.contains("marked sensitive"), "{sensitive}");
    let gone = host
        .manga_details(
            PEERTUBE,
            &format!("{origin}/videos/watch/33333333-4444-4555-8666-777777777777"),
        )
        .await
        .unwrap_err();
    assert!(gone.contains("no longer on its instance"), "{gone}");
    let odd = host
        .manga_details(PEERTUBE, "https://example.org/channel/x")
        .await
        .unwrap_err();
    assert!(odd.contains("no such video"), "{odd}");
}
