//! More open sources against local fixture servers (the real bundles, their API bases pointed at
//! this machine): the Internet Archive's public-domain books as PDFs, Creative Commons webcomics,
//! PeerTube's openly licensed videos through Sepia Search, the Great 78 Project's and UC Santa
//! Barbara's records old enough to be in the public domain, NASA's Scientific Visualization
//! Studio, and ESO's and NOIRLab's CC BY videos.

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
use crate::novel::{Part, PassageBlock};
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
        "/tag/minifantasytheater" => html(
            r#"<html><body><div class="thumbbox col sml-6 med-4"><figure class="thumbnail">
            <a href="https://www.davidrevoy.com/article1184/the-avian-intelligence-agent" title="The Avian Intelligence Agent"><img src="https://www.davidrevoy.com/plugins/vignette/plxthumbnailer.php?src=https://www.davidrevoy.com/data/images/blog/2026/2026-11-11_en_078_minifantasytheater.jpg&amp;w=430&amp;h=240&amp;s=1&amp;q=80" alt="The Avian Intelligence Agent"></a>
            <figcaption class="text-center"><a href="https://www.davidrevoy.com/article1184/the-avian-intelligence-agent">The Avian Intelligence Agent</a><br/>
            <span class="detail"> 11 november 2026<a href="https://www.davidrevoy.com/article1184/the-avian-intelligence-agent/show#comments">, 44 comments</a></span></figcaption></figure></div>
            <div class="thumbbox col sml-6 med-4"><figure class="thumbnail">
            <a href="https://www.davidrevoy.com/article1024/morning-routine" title="Morning routine"><img src="https://www.davidrevoy.com/plugins/vignette/plxthumbnailer.php?src=https://www.davidrevoy.com/data/images/blog/2024/2024-04-21_morning-routine.jpg&amp;w=430" alt="Morning routine"></a>
            <figcaption><span class="detail"> 21 april 2024</span></figcaption></figure></div></body></html>"#
                .to_owned(),
        ),
        "/article1184/the-avian-intelligence-agent" => html(
            r#"<html><body><a href="https://www.davidrevoy.com/data/images/blog/2026/2026-11-11_en_078_minifantasytheater.jpg"><img src="https://www.davidrevoy.com/plugins/vignette/plxthumbnailer.php?src=x"></a>
            <a href="https://www.davidrevoy.com/data/images/blog/2026/2026-11-11_sketches.jpg">sketches</a></body></html>"#
                .to_owned(),
        ),
        "/article1024/morning-routine" => html(
            r#"<html><body><a href="https://www.davidrevoy.com/data/images/blog/2024/2024-04-21_morning-routine.jpg">strip</a></body></html>"#
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
async fn open_webcomics_lists_every_series_and_their_strips() {
    let (app, asked) = recording(webcomics_answer);
    let address = serve(app);
    let host = local_host(
        "open-webcomics",
        &[
            "https://www.sandraandwoo.com",
            "https://turnoff.us",
            "https://www.davidrevoy.com",
        ],
        address,
    )
    .await;

    let popular = host
        .manga_list(WEBCOMICS, MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["sandra-and-woo", "turnoff", "mini-fantasy-theater"]);
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

    // MiniFantasyTheater: the tag's pages until one is missing, numbered from the first episode;
    // a post's own strip, not its other pictures; an early post's only picture.
    let chapters = host
        .manga_chapters(WEBCOMICS, "mini-fantasy-theater")
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
                "mini-fantasy-theater/1184/the-avian-intelligence-agent",
                "The Avian Intelligence Agent",
                Some(2.0)
            ),
            (
                "mini-fantasy-theater/1024/morning-routine",
                "Morning routine",
                Some(1.0)
            ),
        ]
    );
    assert_eq!(chapters[1].date_upload, Some(1_713_657_600_000));
    assert!(
        asked
            .lock()
            .unwrap()
            .iter()
            .any(|path| path == "/tag/minifantasytheater/page2"),
        "reads on until a page is missing"
    );
    let pages = host
        .manga_pages(
            WEBCOMICS,
            "mini-fantasy-theater/1184/the-avian-intelligence-agent",
        )
        .await
        .unwrap();
    let urls: Vec<_> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://www.davidrevoy.com/data/images/blog/2026/2026-11-11_en_078_minifantasytheater.jpg"
        ]
    );
    let pages = host
        .manga_pages(WEBCOMICS, "mini-fantasy-theater/1024/morning-routine")
        .await
        .unwrap();
    assert_eq!(
        pages[0].url,
        "https://www.davidrevoy.com/data/images/blog/2024/2024-04-21_morning-routine.jpg"
    );
    let details = host
        .manga_details(WEBCOMICS, "mini-fantasy-theater")
        .await
        .unwrap();
    assert_eq!(details.authors, ["David Revoy"]);
    assert!(details.description.unwrap().contains("CC BY-SA 4.0"));
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

// ---- Wikibooks ----

const WIKIBOOKS: &str = "dev.raygun.wikibooks/books";

/// `LaTeX/Document_Structure` as MediaWiki renders it: the book's navigation box, a highlighted
/// example, and the sectioning commands' table, whose placeholders are in wikitext italics that
/// MediaWiki leaves as written inside highlighted code.
const DOCUMENT_STRUCTURE: &str = r#"<div class="mw-content-ltr mw-parser-output" lang="en" dir="ltr"><table class="wikitable collapsible noprint"><tbody><tr><th>LaTeX</th></tr><tr><td><a href="/wiki/LaTeX/Basics" title="LaTeX/Basics">Basics</a></td></tr></tbody></table>
<p>The area between <code class="mw-highlight mw-highlight-lang-latex mw-content-ltr" dir="ltr"><span class="k">\documentclass</span><span class="nb">{</span>...<span class="nb">}</span></code> and the document is the <i>preamble</i>.
</p>
<table style="margin-top: 1.5em; margin-bottom: 1.5em">
<tbody><tr>
<td style="padding-left:3em;">
<div class="mw-highlight mw-highlight-lang-latex mw-content-ltr" dir="ltr"><pre><span></span><span class="k">\documentclass</span><span class="nb">{</span>article<span class="nb">}</span>
<span class="k">\begin</span><span class="nb">{</span>document<span class="nb">}</span>
<span class="k">\end</span><span class="nb">{</span>document<span class="nb">}</span>
</pre></div>
</td></tr></tbody></table>
<table class="wikitable">
<tbody><tr>
<th>Command
</th>
<th width="10%">Level
</th>
<th>Comment
</th></tr>
<tr>
<td><code class="mw-highlight mw-highlight-lang-latex mw-content-ltr" dir="ltr"><span class="k">\part</span><span class="nb">{</span>''part''<span class="nb">}</span></code>
</td>
<td style="text-align: center">-1
</td>
<td>not in letters
</td></tr>
<tr>
<td><code class="mw-highlight mw-highlight-lang-latex mw-content-ltr" dir="ltr"><span class="k">\chapter</span><span class="nb">{</span>''chapter''<span class="nb">}</span></code>
</td>
<td style="text-align: center">0
</td>
<td>only books and reports
</td></tr></tbody></table>
<p>A quotation closes with <code class="mw-highlight mw-highlight-lang-latex">''</code> and a double prime is <code class="mw-highlight mw-highlight-lang-latex">a''</code>.</p>
</div>"#;

#[tokio::test]
async fn wikibooks_reads_tables_of_commands_with_their_placeholders_in_italics() {
    let (app, asked) = recording(|address| {
        if address.contains("action=parse") {
            json_response(
                StatusCode::OK,
                json!({ "parse": { "title": "LaTeX/Document Structure", "text": DOCUMENT_STRUCTURE } }),
            )
        } else {
            json_response(StatusCode::NOT_FOUND, json!({}))
        }
    });
    let address = serve(app);
    let host = local_host("wikibooks", &["https://en.wikibooks.org"], address).await;
    let blocks = host
        .novel_passage(WIKIBOOKS, "LaTeX/Document Structure")
        .await
        .unwrap();
    // The rendered page, not its wikitext.
    assert!(last_query(&asked, "/w/api.php").contains("action=parse"));
    // The navigation box is left out; the example is LaTeX code, line by line.
    assert!(
        !blocks
            .iter()
            .any(|block| block.plain().is_some_and(|text| text.contains("Basics")))
    );
    assert!(blocks.iter().any(|block| matches!(
        block,
        PassageBlock::Code(code) if code.language.as_deref() == Some("latex")
            && code.text == "\\documentclass{article}\n\\begin{document}\n\\end{document}"
    )));
    let table = blocks
        .iter()
        .find_map(|block| match block {
            PassageBlock::Table(table) => Some(table),
            _ => None,
        })
        .expect("the commands' table");
    assert_eq!(
        table.plain(),
        "Command\tLevel\tComment\n\\part{part}\t-1\tnot in letters\n\\chapter{chapter}\t0\tonly books and reports"
    );
    assert!(table.rows[2][0].parts.iter().any(|part| matches!(
        part,
        Part::Styled(styled) if styled.italic && styled.code && styled.text == "chapter"
    )));
    // Quotes TeX means stay as written.
    assert_eq!(
        blocks.last().and_then(PassageBlock::plain).as_deref(),
        Some("A quotation closes with '' and a double prime is a''.")
    );
}

// ---- Great 78 Project ----

const GREAT_78: &str = "dev.raygun.great-78";
const GREAT_78_CHARTS: &str = "dev.raygun.great-78/charts";

fn great_78_record(date: &str, title: &str, description: &str, files: Value) -> Value {
    json!({
        "metadata": {
            "title": title,
            "date": date,
            "creator": ["Billy Murray", "Geo. M. Cohan"],
            "description": description,
            "publisher": "EDISON",
            "publisher-catalog-number": "50443",
            "collection": ["78rpm", "georgeblood", "audio_music"],
        },
        "files": files,
    })
}

fn great_78_answer(path: &str) -> Response {
    let ok = |value: Value| json_response(StatusCode::OK, value);
    if path.starts_with("/advancedsearch.php") {
        // Charts and genre rows ask for a genre or for every record; a search names titles.
        let docs = if path.contains("genre%3A") || !path.contains("title") {
            json!([{ "identifier": "78_band-contest" }, { "identifier": "78_over-there" }])
        } else {
            json!([
                { "identifier": "78_over-there" },
                { "identifier": "78_lomax-set" },
                { "identifier": "78_test-pressing" },
                { "identifier": "78_opera-set" },
            ])
        };
        return ok(json!({ "response": { "numFound": 4, "docs": docs } }));
    }
    let mp3 = |name: &str, length: &str, title: &str| json!({ "name": name, "format": "VBR MP3", "length": length, "title": title });
    match path {
        "/metadata/78_over-there" => ok(great_78_record(
            "1917",
            "OVER THERE",
            "<p><b>Performer:</b> Billy Murray<br /><b>Writer:</b> Geo. M. Cohan</p><p>Matrix number: 5609-C-2014</p>",
            json!([
                mp3("Over There - Billy Murray.mp3", "02:54", "Over There"),
                mp3("Over There - Billy Murray-restored.mp3", "174.2", "Over There (restored)"),
                { "name": "Over There - Billy Murray.flac", "format": "24bit Flac" },
            ]),
        )),
        "/metadata/78_lomax-set" => ok(great_78_record(
            "1907 - 1936",
            "Folk Music of the United States",
            "<p><b>Performer:</b> Mrs. Texas Gladden</p><p>Sung at Salem, Va., 1941</p>",
            json!([mp3(
                "01 - The House Carpenter.mp3",
                "180",
                "The House Carpenter"
            )]),
        )),
        "/metadata/78_test-pressing" => ok(great_78_record(
            "1921",
            "Some Song",
            "<p>Unissued test pressing.</p>",
            json!([mp3("Some Song.mp3", "180", "Some Song")]),
        )),
        "/metadata/78_opera-set" => ok(great_78_record(
            "1920",
            "CARMEN (Volumes I and II)",
            "<p><b>Performer:</b> ENRICO CARUSO; Victor Orchestra</p>",
            json!([
                mp3("02 - Habanera.mp3", "200", "Habanera"),
                mp3("01 - Prelude.mp3", "100", "Prelude"),
            ]),
        )),
        "/metadata/78_band-contest" => ok(great_78_record(
            "1901",
            "A Coon Band Contest",
            "<p><b>Performer:</b> Sousa's Band</p>",
            json!([mp3("A Coon Band Contest.mp3", "150", "A Coon Band Contest")]),
        )),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::test]
async fn great_78_offers_issued_records_old_enough_for_the_public_domain() {
    let (app, asked) = recording(great_78_answer);
    let address = serve(app);
    let host = local_host("great-78", &["https://archive.org"], address).await;
    let cutoff = chrono::Utc::now().year() - 102;

    let found = host.search_music("over there", 10).await;
    let found = found
        .iter()
        .find(|result| result.provider.extension_id == GREAT_78)
        .unwrap();
    assert_eq!(found.error, None);
    let query = last_query(&asked, "/advancedsearch.php");
    assert!(
        query.contains(&format!(
            "q=(title:(over* there*) OR creator:(over* there*)) AND collection:georgeblood AND mediatype:audio AND year:[1880 TO {cutoff}]"
        )),
        "{query}"
    );
    assert!(query.contains("sort[]=downloads desc"), "{query}");
    let songs: Vec<_> = found
        .tracks
        .iter()
        .map(|track| {
            (
                track.title.as_str(),
                track.artist.as_deref(),
                track.album.as_deref(),
                track.duration_ms,
            )
        })
        .collect();
    // A set dated up to 1936 (with a note from 1941) and a test pressing are left out; a set's
    // recordings come in order, under the set's title.
    assert_eq!(
        songs,
        [
            (
                "Over There",
                Some("Billy Murray"),
                Some("Edison 50443"),
                Some(174_200)
            ),
            (
                "Prelude",
                Some("Enrico Caruso; Victor Orchestra"),
                Some("CARMEN (Volumes I and II)"),
                Some(100_000)
            ),
            (
                "Habanera",
                Some("Enrico Caruso; Victor Orchestra"),
                Some("CARMEN (Volumes I and II)"),
                Some(200_000)
            ),
        ]
    );
    let Some(raygun_model::MediaSource::Url { url, .. }) = &found.tracks[0].source else {
        panic!("a stream");
    };
    assert_eq!(
        url,
        &format!(
            "http://{address}/download/78_over-there/Over%20There%20-%20Billy%20Murray-restored.mp3"
        )
    );

    // Charts: the slur in a title of the time keeps that record out of rows shown unasked.
    let charts: Vec<_> = host
        .music_charts(None)
        .await
        .into_iter()
        .filter(|(provider, _)| provider.id == GREAT_78_CHARTS)
        .flat_map(|(_, charts)| charts.unwrap())
        .collect();
    assert!(
        charts
            .iter()
            .any(|chart| chart.id == "decade:1910" && chart.name == "The 1910s")
    );
    let top = host
        .music_chart(GREAT_78_CHARTS, "top", None, 10)
        .await
        .unwrap();
    let titles: Vec<_> = top.iter().map(|track| track.title.as_str()).collect();
    assert_eq!(titles, ["Over There"]);
    let rows = host
        .music_tag_songs("Country", &[], "genre", None, 10)
        .await;
    let (name, tracks) = rows
        .into_iter()
        .find(|(provider, _)| provider.id == GREAT_78_CHARTS)
        .unwrap()
        .1
        .unwrap();
    assert_eq!(name.as_deref(), Some("Folk and old time on 78s"));
    assert_eq!(tracks.len(), 1);
    let query = last_query(&asked, "/advancedsearch.php");
    assert!(query.contains(r#"genre:("Folk" OR "Hillbilly""#), "{query}");
}
// ---- UCSB Cylinder Audio Archive ----

const CYLINDERS: &str = "dev.raygun.ucsb-cylinders";

/// A search result row as the archive draws it.
fn cylinder_row(mms: &str, title: &str, year: &str) -> String {
    format!(
        r#"<div class="result-row"><div class="result-container">
        <div class="row-link"><a href="detail.php?query_type=mms_id&query={mms}&r=1&of=4">1</a></div>
        <div class="pure-g"><div class="pure-u-1-3"><div class="data-label">Title:</div></div>
        <div class="pure-u-2-3"><div class="data-entry"><a href="detail.php?query_type=mms_id&query={mms}&r=1&of=4">{title}</a></div></div></div>
        <div class="pure-g"><div class="pure-u-1-3"><div class="data-label">Release Year:</div></div>
        <div class="pure-u-2-3"><div class="data-entry">{year}</div></div></div>
        </div><hr /></div>"#
    )
}

/// A cylinder's page: its title, rows (label and entries) and MP3.
fn cylinder_page(title: &str, rows: &[(&str, &str)], number: &str) -> String {
    let rows: String = rows
        .iter()
        .map(|(label, entry)| {
            format!(
                r#"<div class="pure-g detail-row"><div class="pure-u-1-3"><div class=" data-label">{label}: </div></div>
                <div class="pure-u-2-3"><div class="data-entry">{entry}</div></div></div>"#
            )
        })
        .collect();
    format!(
        r#"<html><body><div id="main"><h1 id="uniform_title"> {title} </h1>{rows}
        <div class="detail-audio audio-wrapper"><h3>Cylinder {number}</h3>
        <audio controls src="https://cylinders.s3-us-west-2.amazonaws.com/0000/{number}/cusb-cyl{number}d.mp3" type="audio/m4a"></audio>
        <div class="download"><a href="/download.php?number={number}" download>Download</a></div></div></div></body></html>"#
    )
}

fn cylinders_answer(path: &str) -> Response {
    if path.starts_with("/search.php") {
        let rows = if path.contains("query_type=subject") {
            cylinder_row("111", "Alexander's ragtime band", "1911")
        } else {
            [
                cylinder_row("111", "Alexander's ragtime band", "1911"),
                cylinder_row("222", "A later song", "1950"),
                cylinder_row("333", "[Brown wax home recording]", "1905"),
                cylinder_row(
                    "444",
                    "[Brown wax commercial recording of a march]",
                    "between 1900 and 1902",
                ),
            ]
            .concat()
        };
        return html(format!(
            "<html><body><div class=\"search-results\">{rows}</div></body></html>"
        ));
    }
    match path {
        "/detail.php?query_type=mms_id&query=111" => html(cylinder_page(
            "Alexander's ragtime band /",
            &[
                ("Performers", "[Billy] Murray."),
                ("Issue Number", "Edison Blue Amberol: 2048."),
                (
                    "Release year",
                    "<a href=\"/search.php?query=1913\">1913</a>",
                ),
            ],
            "0400",
        )),
        "/detail.php?query_type=mms_id&query=333" => html(cylinder_page(
            "[Brown wax home recording]",
            &[("Release year", "1905"), ("Notes", "Home recording.")],
            "3333",
        )),
        "/detail.php?query_type=mms_id&query=444" => html(cylinder_page(
            "[Brown wax commercial recording of a march]",
            &[
                ("Performers", "Performers not identified."),
                ("Release year", "between 1900 and 1902"),
                (
                    "Notes",
                    "Commercial brown wax cylinder; no information on cylinder.",
                ),
            ],
            "4444",
        )),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::test]
async fn ucsb_cylinders_offers_issued_cylinders_released_long_enough_ago() {
    let (app, asked) = recording(cylinders_answer);
    let address = serve(app);
    let host = local_host(
        "ucsb-cylinders",
        &["https://cylinders.library.ucsb.edu"],
        address,
    )
    .await;

    let results = host.search_music("rag", 10).await;
    let found = results
        .iter()
        .find(|result| result.provider.extension_id == CYLINDERS)
        .unwrap();
    assert_eq!(found.error, None);
    assert_eq!(
        last_query(&asked, "/search.php"),
        "nq=1&query_type=keyword&query=rag"
    );
    let songs: Vec<_> = found
        .tracks
        .iter()
        .map(|track| {
            (
                track.title.as_str(),
                track.artist.as_deref(),
                track.album.as_deref(),
            )
        })
        .collect();
    // A cylinder released too recently isn't even opened; a home recording was never issued.
    assert_eq!(
        songs,
        [
            (
                "Alexander's ragtime band",
                Some("Billy Murray"),
                Some("Edison Blue Amberol 2048")
            ),
            ("Brown wax commercial recording of a march", None, None),
        ]
    );
    let Some(raygun_model::MediaSource::Url { url, .. }) = &found.tracks[0].source else {
        panic!("a stream");
    };
    assert_eq!(
        url,
        "https://cylinders.s3-us-west-2.amazonaws.com/0000/0400/cusb-cyl0400d.mp3"
    );
    assert!(
        !asked
            .lock()
            .unwrap()
            .iter()
            .any(|path| path.contains("query=222")),
        "released in 1950"
    );

    // A chart is a subject search; the cylinder's page was kept.
    let before = asked.lock().unwrap().len();
    let vaudeville = host
        .music_chart("dev.raygun.ucsb-cylinders/charts", "vaudeville", None, 10)
        .await
        .unwrap();
    assert_eq!(vaudeville.len(), 1);
    let requests = asked.lock().unwrap()[before..].to_vec();
    assert_eq!(
        requests,
        ["/search.php?nq=1&query_type=subject&query=Vaudeville"]
    );
}
// ---- NASA Scientific Visualization Studio ----

const SVS: &str = "dev.raygun.nasa-svs/videos";

/// A result card as the SVS search draws it.
fn svs_card(id: u32, title: &str, picture: &str, kind: &str) -> String {
    format!(
        r#"<li
            id="card_search_result_{id}"
            class="list-item card flex-shrink-0"
        ><div class="hstack"><div class="list-item-image flex-shrink-0"><a href="/{id}/" title="Go to this page"><img
            class="w-100 h-100" src="{picture}" alt="A picture" loading="lazy"></a><span class="badge"> ID: {id} </span><span
            class="d-none d-lg-block badge"><span class="bi bi-pie-chart-fill me-1" aria-hidden="true"></span> {kind}
            </span></div><div class="list-item-body"><h3 id="card_search_result_{id}_body_title" class="mb-0"><a href="/{id}/">
            {title}
            </a></h3></div><a id="card_search_result_{id}_body_link_button" href="/{id}/">Go to this page <span class="bi bi-chevron-right mx-2" aria-hidden="true"></span></a></div></li>"#
    )
}

fn svs_movie(name: &str, height: u32) -> Value {
    json!({ "type": "media", "instance": {
        "url": format!("http://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/{name}"),
        "filename": name,
        "media_type": if name.ends_with(".vtt") || name.ends_with(".srt") { "Captions" } else { "Movie" },
        "width": height * 16 / 9,
        "height": height,
    }})
}

fn svs_answer(path: &str) -> Response {
    let ok = |value: Value| json_response(StatusCode::OK, value);
    if path.starts_with("/api/search/results/list") {
        return html(
            [
                svs_card(
                    4720,
                    "CGI Moon Kit",
                    "/vis/a000000/a004700/a004720/lroc_color_2k.jpg",
                    "Visualization",
                ),
                svs_card(
                    15103,
                    "Impact Flashes &amp; the Moon",
                    "/vis/a010000/a015100/a015103/flash_print.jpg",
                    "Produced Video",
                ),
            ]
            .concat(),
        );
    }
    match path {
        "/api/4720/" => ok(json!({
            "id": 4720,
            "title": "CGI Moon Kit",
            "description": "<p>These color and elevation maps are designed for use in 3D rendering software.</p>",
            "release_date": "2019-09-06T10:00:00-04:00",
            "main_image": { "url": "http://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/lroc_color_2k.jpg" },
            "credits": [{ "role": "Visualizer", "people": [{ "name": "Ernie Wright" }] }],
            "nasa_science_categories": ["Planets & Moons"],
            "missions": [],
            "keywords": ["Moon"],
            "media_groups": [
                { "id": 322782, "widget": "Single image", "items": [] },
                { "id": 322787, "widget": "Video player", "title": "How These Maps Are Used", "items": [
                    svs_movie("moon_mapping_720p30.webm", 720),
                    svs_movie("moon_mapping_360p30.mp4", 360),
                    svs_movie("moon_mapping_1080p30.mp4", 1080),
                    svs_movie("moon_mapping_720p30.mp4", 720),
                    svs_movie("moon_mapping.en_US.srt", 0),
                    svs_movie("moon_mapping.en_US.vtt", 0),
                ] },
            ],
        })),
        "/api/15103/" => ok(json!({
            "id": 15103,
            "title": "Impact Flashes",
            "description": "Music is \"Bio Rhythms\" by Adam Paul Courtenay Burns of Universal Production Music.",
            "media_groups": [{ "id": 1, "widget": "Video player", "items": [svs_movie("flash.mp4", 2160)] }],
        })),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::test]
async fn nasa_svs_lists_public_domain_videos_and_leaves_out_licensed_music() {
    let (app, asked) = recording(svs_answer);
    let address = serve(app);
    let host = local_host("nasa-svs", &["https://svs.gsfc.nasa.gov"], address).await;

    let popular = host.manga_list(SVS, MangaList::Popular, 1).await.unwrap();
    let cards: Vec<_> = popular
        .items
        .iter()
        .map(|item| {
            (
                item.id.as_str(),
                item.title.as_str(),
                item.subtitle.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        cards,
        [
            ("4720", "CGI Moon Kit", Some("Visualization")),
            ("15103", "Impact Flashes & the Moon", Some("Produced Video")),
        ]
    );
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("https://{address}/vis/a000000/a004700/a004720/lroc_color_2k.jpg").as_str())
    );
    assert!(!popular.has_next_page, "eight cards make a full chunk");
    assert_eq!(
        last_query(&asked, "/api/search/results/list"),
        "sort_by_1=popularity&media=movies&sort_by_0=DESC&chunk=0"
    );

    // Licensed music: refused once opened, and left out of lists from then on.
    let refused = host.manga_details(SVS, "15103").await.unwrap_err();
    assert!(refused.contains("licensed music"), "{refused}");
    let popular = host.manga_list(SVS, MangaList::Popular, 1).await.unwrap();
    assert_eq!(popular.items.len(), 1);

    host.manga_search_filtered(
        SVS,
        "moon",
        2,
        &filters(&[
            ("type", "visualization"),
            ("category", "Earth"),
            ("studio", "SVS"),
        ]),
    )
    .await
    .unwrap();
    assert_eq!(
        last_query(&asked, "/api/search/results/list"),
        "sort_by_1=relevance&search=moon&result_type=visualization&nasa_science_categories=Earth&studio=SVS&media=movies&sort_by_0=DESC&chunk=1"
    );

    let details = host.manga_details(SVS, "4720").await.unwrap();
    assert_eq!(details.year, Some(2019));
    assert_eq!(
        details.cover_url.as_deref(),
        Some("https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/lroc_color_2k.jpg")
    );
    let description = details.description.unwrap();
    assert!(
        description.contains("Visualizer: Ernie Wright"),
        "{description}"
    );
    assert!(
        description.contains("Public domain (NASA media)"),
        "{description}"
    );

    let episodes = host.video_episodes(SVS, "4720").await.unwrap();
    let names: Vec<_> = episodes
        .iter()
        .map(|episode| (episode.id.as_str(), episode.name.as_str()))
        .collect();
    assert_eq!(names, [("4720:322787", "How These Maps Are Used")]);
    let servers = host.video_servers(SVS, "4720:322787").await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    let files: Vec<_> = videos
        .iter()
        .map(|video| (video.label.as_deref(), video.quality))
        .collect();
    assert_eq!(
        files,
        [
            (Some("1080p"), Some(1080)),
            (Some("720p"), Some(720)),
            (Some("720p WebM"), Some(720)),
            (Some("360p"), Some(360)),
        ]
    );
    assert_eq!(
        videos[0].url,
        "https://svs.gsfc.nasa.gov/vis/a000000/a004700/a004720/moon_mapping_1080p30.mp4"
    );
    let captions: Vec<_> = videos[0]
        .subtitles
        .iter()
        .map(|track| (track.lang.as_deref(), track.url.rsplit('/').next().unwrap()))
        .collect();
    assert_eq!(captions, [(Some("en"), "moon_mapping.en_US.vtt")]);
}
// ---- ESO and NOIRLab ----

const ESO: &str = "dev.raygun.eso/eso";
const NOIRLAB: &str = "dev.raygun.eso/noirlab";

fn eso_thumbnails(videos: &[(&str, &str)], next: &str) -> String {
    let cards: String = videos
        .iter()
        .map(|(id, title)| {
            format!(
                r##"<div class="video-thumbnail"><div class="img-wrapper"><a class="popup-youtube" href="#popup-youtube" data-youtube-id="x" data-title="{title}" data-url="/public/videos/{id}/"><span class="fa fa-play"></span><img src="https://cdn.eso.org/videos/videoframe/{id}.jpg" class="img-responsive" alt="{title}"></a></div></div>"##
            )
        })
        .collect();
    format!(
        r#"<html><body>{cards}<ul class="pagination"><li><a href="{next}">2</a></li></ul></body></html>"#
    )
}

fn eso_video(credit: &str) -> String {
    format!(
        r#"<html><head><meta property="og:image" content="https://cdn.eso.org/videos/videoframe/eso2613a.jpg" /></head><body>
        <h1>Astronomers find unique disc of debris around dead star | ESO News</h1><div class="youtube-wrapper"></div>
        <p>Using ESO&#39;s Very Large Telescope, astronomers found a unique disc.</p>
        <p>Members of the media can request a version of this video without the overset text.</p>
        <strong>Credit:</strong><br><div class="credit">{credit}</div>
        <div class="object-info"><h3>About the Video</h3><div><table>
        <tr><td class="title">Id:</td><td>eso2613a</td></tr>
        <tr><td class="title">Release date:</td><td>27 October 2026, 14:00</td></tr>
        <tr><td class="title">Duration:</td><td>01 m 21 s</td></tr></table></div></div>
        <span class="archive_dl_text"><a href="https://cdn.eso.org/videos/ultra_hd/eso2613a.mp4">4K Ultra HD Preview H.264</a></span>
        <span class="archive_dl_text"><a href="https://cdn.eso.org/videos/ultra_hd_h265/eso2613a.mp4">4K Ultra HD Preview H.265</a></span>
        <span class="archive_dl_text"><a href="https://cdn.eso.org/videos/hd_and_apple/eso2613a.m4v">HD &amp; Apple TV Preview</a></span>
        <span class="archive_dl_text"><a href="https://cdn.eso.org/videos/hd_1080p25_screen/eso2613a.mp4">Full HD Preview 1080p</a></span>
        <span class="archive_dl_text"><a href="https://cdn.eso.org/videos/medium_podcast/eso2613a.mp4">Video Podcast</a></span>
        <span class="archive_dl_text"><a href="https://cdn2.eso.org/videos/ultra_hd_broadcast/eso2613a.mov">4K Ultra HD Broadcast</a></span>
        <span class="archive_dl_text"><a href="https://cdn.eso.org/videos/srt/eso2613a_en.srt">English subtitles</a></span>
        </body></html>"#
    )
}

fn eso_answer(path: &str) -> Response {
    match path {
        "/public/videos/" => html(eso_thumbnails(
            &[("eso2613a", "Debris around a dead star"), ("eso1234a", "A borrowed video")],
            "/public/videos/list/2/",
        )),
        "/public/videos/page/2/" => html(eso_thumbnails(
            &[("noirlab2011c", "Lucky Imaging")],
            "/public/videos/page/3/",
        )),
        "/public/videos/viewall/?sort=-release_date" => html(
            r#"<html><body><table class="archive_normal"><tr>
            <td class="imagerow" width="25%"><a href="/public/videos/cs0032a/"><img src="/public/archives/videos/thumb/cs0032a.jpg" width="122" alt="Chatting science" /></a></td>
            </tr></table></body></html>"#
                .to_owned(),
        ),
        "/public/videos/archive/category/solarsystem/?search=jupiter" => html(eso_thumbnails(
            &[("eso1623a", "Jupiter imaged using VISIR")],
            "",
        )),
        "/public/videos/eso2613a/" => html(eso_video(
            "<p>ESO</p> <p><strong>Directed by</strong>: Angelos Tsaousis.<br /><strong>Music</strong>: Stellardrone – Rendezvous with Rama.</p>",
        )),
        "/public/videos/eso1234a/" => html(eso_video(
            "<p>© Someone Else, used with permission</p>",
        )),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

#[tokio::test]
async fn eso_and_noirlab_list_cc_by_videos_and_play_their_files() {
    let (app, asked) = recording(eso_answer);
    let address = serve(app);
    let host = local_host(
        "eso",
        &["https://www.eso.org", "https://noirlab.edu"],
        address,
    )
    .await;

    let popular = host.manga_list(ESO, MangaList::Popular, 1).await.unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["eso2613a", "eso1234a"]);
    assert!(popular.has_next_page);
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://cdn.eso.org/videos/videoframe/eso2613a.jpg")
    );
    // NOIRLab numbers its pages `/page/<n>/`.
    let second = host
        .manga_list(NOIRLAB, MangaList::Popular, 2)
        .await
        .unwrap();
    assert_eq!(second.items[0].id, "noirlab2011c");
    assert!(second.has_next_page);
    let latest = host.manga_list(ESO, MangaList::Latest, 1).await.unwrap();
    assert_eq!(latest.items[0].id, "cs0032a");
    assert_eq!(
        latest.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/public/archives/videos/thumb/cs0032a.jpg").as_str())
    );
    let found = host
        .manga_search_filtered(ESO, "jupiter", 1, &filters(&[("category", "solarsystem")]))
        .await
        .unwrap();
    assert_eq!(found.items[0].id, "eso1623a");
    assert!(!found.has_next_page);

    let details = host.manga_details(ESO, "eso2613a").await.unwrap();
    assert_eq!(details.year, Some(2026));
    assert_eq!(
        details.cover_url.as_deref(),
        Some("https://cdn.eso.org/videos/videoframe/eso2613a.jpg")
    );
    let description = details.description.unwrap();
    assert!(
        description.starts_with("Using ESO's Very Large Telescope"),
        "{description}"
    );
    assert!(
        !description.contains("Members of the media"),
        "{description}"
    );
    assert!(description.contains("1:21"), "{description}");
    assert!(description.contains("Music: Stellardrone"), "{description}");
    assert!(description.contains("Licence: CC BY 4.0"), "{description}");
    assert!(
        description.contains("The music isn't part of that licence"),
        "{description}"
    );

    let episodes = host.video_episodes(ESO, "eso2613a").await.unwrap();
    assert_eq!(episodes[0].date_upload, Some(1_793_109_600_000));
    let servers = host.video_servers(ESO, "eso2613a").await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    let files: Vec<_> = videos
        .iter()
        .map(|video| {
            (
                video.label.as_deref(),
                video.url.rsplit('/').nth(1).unwrap(),
            )
        })
        .collect();
    // H.265 and broadcast copies are left out.
    assert_eq!(
        files,
        [
            (Some("4K"), "ultra_hd"),
            (Some("1080p"), "hd_1080p25_screen"),
            (Some("720p"), "hd_and_apple"),
            (Some("360p"), "medium_podcast"),
        ]
    );
    assert_eq!(videos[0].subtitles[0].lang.as_deref(), Some("en"));

    // Another licence: refused once opened, and left out of lists from then on.
    let refused = host.manga_details(ESO, "eso1234a").await.unwrap_err();
    assert!(refused.contains("licence other than CC BY"), "{refused}");
    let popular = host.manga_list(ESO, MangaList::Popular, 1).await.unwrap();
    assert_eq!(popular.items.len(), 1);
    assert_eq!(
        asked
            .lock()
            .unwrap()
            .iter()
            .filter(|path| path.as_str() == "/public/videos/")
            .count(),
        1,
        "list pages are kept"
    );
}
