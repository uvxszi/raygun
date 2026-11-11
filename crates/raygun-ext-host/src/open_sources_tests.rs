//! The public-domain and openly licensed sources (Standard Ebooks, Wikisource, Wikimedia Commons,
//! NASA, Blender open movies, the Internet Archive's open audio and public-domain films, OpenStax)
//! and the keyed ones (National Archives, Europeana, Last.fm) against local fixture servers: the
//! real bundles, their API bases pointed at this machine.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use raygun_model::{ChartGroup, ExtensionStatus, FilterValue, MediaSource, TrackQuery};
use serde_json::{Value, json};

use crate::fixture_tests::{json_response, query_params, serve};
use crate::manga::MangaList;
use crate::novel::{NOVEL_SOURCE, PassageBlock};
use crate::video::VIDEO_SOURCE;
use crate::{ExtensionHost, ExtensionSource, HostConfig};

/// The requests a fixture server saw, as their query parameters.
type Seen = Arc<Mutex<Vec<HashMap<String, String>>>>;

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

fn xhtml(body: String) -> Response {
    (
        [(header::CONTENT_TYPE, "application/xhtml+xml; charset=utf-8")],
        body,
    )
        .into_response()
}

fn select(value: &str) -> FilterValue {
    FilterValue::Select(value.to_owned())
}

fn filters(pairs: &[(&str, &str)]) -> BTreeMap<String, FilterValue> {
    pairs
        .iter()
        .map(|(key, value)| ((*key).to_owned(), select(value)))
        .collect()
}

fn last(seen: &Seen) -> HashMap<String, String> {
    seen.lock().unwrap().last().cloned().unwrap()
}

/// Extended metadata as Commons gives it: `{ key: { value } }`.
fn meta(fields: &[(&str, &str)]) -> Value {
    Value::Object(
        fields
            .iter()
            .map(|(key, value)| ((*key).to_owned(), json!({ "value": value })))
            .collect(),
    )
}

// ---- Standard Ebooks ----

fn ebook_item(id: &str, title: &str, author: &str) -> String {
    let slug = id.replace('/', "_");
    format!(
        r#"<li typeof="schema:Book" about="/ebooks/{id}">
        <div class="thumbnail-container" aria-hidden="true"><a href="/ebooks/{id}" property="schema:url"><picture>
        <img src="/images/covers/{slug}/abc/cover@2x.jpg" alt="" property="schema:image"/></picture></a></div>
        <p><a href="/ebooks/{id}" property="schema:url"><span property="schema:name">{title}</span></a></p>
        <p class="author" typeof="schema:Person" property="schema:author" resource="/ebooks/someone"><a href="https://standardebooks.org/ebooks/someone" property="schema:url"><span property="schema:name">{author}</span></a></p>
        </li>"#
    )
}

const WAR_AND_PEACE: &str = "leo-tolstoy/war-and-peace/louise-maude_aylmer-maude";

fn standard_ebooks_page(path: &str) -> Option<String> {
    let page = match path {
        "/ebooks/leo-tolstoy/war-and-peace/louise-maude_aylmer-maude" => {
            r#"<main>
            <article class="ebook" typeof="schema:Book" about="/ebooks/leo-tolstoy/war-and-peace/louise-maude_aylmer-maude">
            <meta property="schema:abstract" content="Five families in the Napoleonic wars."/>
            <header><hgroup><h1 property="schema:name">War and Peace</h1>
            <p><a property="schema:author" typeof="schema:Person" href="/ebooks/leo-tolstoy" resource="/ebooks/leo-tolstoy">
            <span property="schema:name">Leo Tolstoy</span><meta property="schema:url" content="x"/></a></p></hgroup></header>
            <aside id="reading-ease"><p>563,500 words</p>
            <p>Translated by <a href="https://en.wikipedia.org/wiki/Louise_Maude">Louise Maude</a> and <a href="https://en.wikipedia.org/wiki/Aylmer_Maude">Aylmer Maude</a>.</p>
            <ul class="tags"><li><a href="/subjects/fiction">Fiction</a></li><li><a href="/subjects/war">War</a></li></ul></aside>
            <section id="description"><aside class="donation"><p>We rely on your support.</p></aside>
            <div property="schema:description"><p><i>War and Peace</i> follows five families.</p><p>It is long &amp; famous.</p></div></section>
            <section id="read-free"><meta property="schema:thumbnailUrl" content="https://standardebooks.org/ebooks/leo-tolstoy/war-and-peace/louise-maude_aylmer-maude/downloads/cover-thumbnail.jpg"/></section>
            </article></main>"#
        }
        "/ebooks/charles-dickens/barnaby-rudge/text" => {
            r#"<body><header><a href="/honeypot" hidden="hidden">Following this link will ban your IP</a></header><main>
            <nav aria-labelledby="toc-title" id="toc" epub:type="toc"><h2 id="toc-title">Table of Contents</h2><ol>
            <li><a href="text/titlepage">Titlepage</a></li>
            <li><a href="text/imprint">Imprint</a></li>
            <li><a href="text/preface">Preface</a></li>
            <li><a href="text/halftitlepage">Barnaby Rudge</a><ol>
              <li><a href="text/chapter-1">Chapter 1</a></li>
              <li><a href="text/chapter-1#section-2">Chapter 1, part two</a></li>
              <li><a href="text/chapter-82">Chapter the Last</a></li>
            </ol></li>
            <li><a href="text/colophon">Colophon</a></li>
            <li><a href="text/uncopyright">Uncopyright</a></li>
            </ol></nav>
            <nav id="landmarks" epub:type="landmarks"><ol><li><a href="text/chapter-1">Barnaby Rudge</a></li></ol></nav>
            </main></body>"#
        }
        "/ebooks/leo-tolstoy/war-and-peace/louise-maude_aylmer-maude/text" => {
            r#"<main><nav id="toc"><ol>
            <li><a href="text/titlepage">Titlepage</a></li>
            <li><a href="text/dramatis-personae">Dramatis Personae</a></li>
            <li><a href="text/halftitlepage">War and Peace</a><ol>
              <li><a href="text/book-1">Book <span epub:type="z3998:roman">I</span></a><ol>
                <li><a href="text/part-1-1">Part <span epub:type="z3998:roman">I</span>: 1805</a><ol>
                  <li><a href="text/chapter-1-1-1" epub:type="z3998:roman">I</a></li>
                  <li><a href="text/chapter-1-1-2" epub:type="z3998:roman">II</a></li>
                </ol></li>
              </ol></li>
            </ol></li>
            <li><a href="text/endnotes">Endnotes</a></li>
            </ol></nav></main>"#
        }
        "/ebooks/charles-dickens/barnaby-rudge/text/chapter-1" => {
            r#"<html><body>
            <header><a href="/honeypot" hidden="hidden">Following this link will ban your IP</a><nav><a href="/">Standard Ebooks</a></nav></header>
            <main epub:type="bodymatter z3998:fiction"><section id="chapter-1" epub:type="chapter">
            <h2><span epub:type="se:label">Chapter</span> <span epub:type="z3998:ordinal">1</span></h2>
            <p>In the year 1775, there stood a house.<a href="endnotes#note-1" id="noteref-1" epub:type="noteref">1</a></p>
            <img src="../images/map.jpg" alt="A map"/>
            </section></main>
            <footer><a href="/ebooks/charles-dickens/barnaby-rudge/text/chapter-2" rel="next">Next: Chapter 2</a></footer></body></html>"#
        }
        _ => return None,
    };
    Some(page.to_owned())
}

#[tokio::test]
async fn standard_ebooks_lists_books_and_reads_their_chapters() {
    let seen: Seen = Arc::default();
    let listed = seen.clone();
    let app = Router::new()
        .route(
            "/ebooks",
            get(move |uri: Uri| {
                let listed = listed.clone();
                async move {
                    let params = query_params(&uri);
                    listed.lock().unwrap().push(params.clone());
                    let page = params.get("page").cloned().unwrap_or_default();
                    let books = match (params.get("query").map(String::as_str), page.as_str()) {
                        (Some("nothing"), _) => String::new(),
                        (_, "2") => ebook_item("mary-shelley/frankenstein", "Frankenstein", "Mary Shelley"),
                        _ => [
                            ebook_item("charles-dickens/barnaby-rudge", "Barnaby Rudge", "Charles Dickens"),
                            ebook_item(WAR_AND_PEACE, "War &amp; Peace", "Leo Tolstoy"),
                            ebook_item("Not An Id", "Broken", "Nobody"),
                        ]
                        .concat(),
                    };
                    let next = if page == "1" && !books.is_empty() {
                        r#"<a href="/ebooks?page=2" rel="next">Next</a>"#
                    } else {
                        ""
                    };
                    xhtml(format!(
                        r#"<?xml version="1.0" encoding="utf-8"?><html><body><main>
                        <ol class="ebooks-list grid">{books}</ol><nav class="pagination">{next}</nav></main></body></html>"#
                    ))
                }
            }),
        )
        .fallback(|uri: Uri| async move {
            match standard_ebooks_page(uri.path()) {
                Some(page) => xhtml(page),
                None => StatusCode::NOT_FOUND.into_response(),
            }
        });
    let address = serve(app);
    let host = local_host("standard-ebooks", &["https://standardebooks.org"], address).await;
    let provider = "dev.raygun.standard-ebooks/books";
    assert_eq!(host.providers(NOVEL_SOURCE).await.len(), 1);

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let titles: Vec<_> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["Barnaby Rudge", "War & Peace"],
        "a malformed id is left out"
    );
    // The catalogue page names each book's author: the cards have them without asking more.
    assert_eq!(popular.items[0].authors, ["Charles Dickens"]);
    assert_eq!(popular.items[1].authors, ["Leo Tolstoy"]);
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(
            format!(
                "http://{address}/images/covers/charles-dickens_barnaby-rudge/abc/cover@2x.jpg"
            )
            .as_str()
        )
    );
    assert!(popular.has_next_page);
    let asked = last(&seen);
    assert_eq!(
        (
            asked["sort"].as_str(),
            asked["per-page"].as_str(),
            asked["page"].as_str()
        ),
        ("popularity", "48", "1")
    );
    let second = host
        .manga_list(provider, MangaList::Latest, 2)
        .await
        .unwrap();
    assert_eq!(second.items[0].id, "mary-shelley/frankenstein");
    assert!(!second.has_next_page);
    assert_eq!(last(&seen)["sort"], "newest");

    host.manga_search(provider, "dickens", 1).await.unwrap();
    let asked = last(&seen);
    assert_eq!(
        (asked["query"].as_str(), asked["sort"].as_str()),
        ("dickens", "relevance")
    );
    assert!(!asked.contains_key("tags%5B%5D"));
    host.manga_search_filtered(
        provider,
        "",
        1,
        &filters(&[("subject", "poetry"), ("sort", "newest")]),
    )
    .await
    .unwrap();
    let asked = last(&seen);
    assert_eq!(
        (asked["tags%5B%5D"].as_str(), asked["sort"].as_str()),
        ("poetry", "newest")
    );
    assert!(!asked.contains_key("query"), "no words, no query");
    let none = host.manga_search(provider, "nothing", 1).await.unwrap();
    assert!(none.items.is_empty() && !none.has_next_page);
    assert_eq!(host.manga_filters(provider).await.unwrap().len(), 2);

    let details = host.manga_details(provider, WAR_AND_PEACE).await.unwrap();
    assert_eq!(details.title, "War and Peace");
    assert_eq!(details.authors, ["Leo Tolstoy"]);
    assert_eq!(details.genres, ["Fiction", "War"]);
    assert_eq!(
        details.description.as_deref(),
        Some(
            "War and Peace follows five families.\n\nIt is long & famous.\n\nTranslated by Louise Maude and Aylmer Maude."
        ),
        "the description and the translators, not the donation box"
    );
    assert_eq!(
        details.cover_url.as_deref(),
        Some(
            "https://standardebooks.org/ebooks/leo-tolstoy/war-and-peace/louise-maude_aylmer-maude/downloads/cover-thumbnail.jpg"
        )
    );
    assert_eq!(details.status, "completed");
    assert!(
        host.manga_details(provider, "../../honeypot")
            .await
            .is_err()
    );

    let chapters = host
        .manga_chapters(provider, "charles-dickens/barnaby-rudge")
        .await
        .unwrap();
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Preface", "Chapter 1", "Chapter the Last"],
        "no title pages, imprint or licence; a page once however often it is linked"
    );
    assert_eq!(
        chapters[1].id,
        "charles-dickens/barnaby-rudge/text/chapter-1"
    );
    assert_eq!(chapters[2].number, Some(3.0));
    let nested = host.manga_chapters(provider, WAR_AND_PEACE).await.unwrap();
    let names: Vec<_> = nested.iter().map(|chapter| chapter.name.as_str()).collect();
    assert_eq!(
        names,
        [
            "Dramatis Personae",
            "Book I · Part I: 1805 · I",
            "Book I · Part I: 1805 · II",
            "Endnotes"
        ],
        "chapters are named with the parts they sit in; the parts' own pages aren't chapters"
    );

    let text = host.novel_passage(provider, &chapters[1].id).await.unwrap();
    assert_eq!(
        text,
        [
            PassageBlock::Heading("Chapter 1".into()),
            PassageBlock::Paragraph("In the year 1775, there stood a house.".into()),
            PassageBlock::Image(format!(
                "http://{address}/ebooks/charles-dickens/barnaby-rudge/images/map.jpg"
            )),
        ],
        "the chapter alone: no site header or footer, no note markers"
    );
    assert!(
        host.novel_passage(
            provider,
            "charles-dickens/barnaby-rudge/text/../../honeypot"
        )
        .await
        .is_err()
    );
}

// ---- Wikisource ----

const PRIDE: &str = "Pride and Prejudice (1817)";

fn wikisource_page(title: &str, round: usize) -> Value {
    let category = |name: &str| json!({ "ns": 14, "title": format!("Category:{name}") });
    match title {
        // Its categories arrive in two rounds, the licence in the second.
        PRIDE if round == 0 => json!({
            "title": PRIDE, "categories": [category("Novels")],
            "thumbnail": { "source": "https://upload.wikimedia.org/p.jpg?utm_source=en.wikisource.org" },
        }),
        PRIDE => json!({ "title": PRIDE, "categories": [category("PD-old")] }),
        "Gettysburg Address" => json!({
            "title": title, "categories": [category("Versions pages"), category("Featured texts")],
        }),
        "Legislation Act 2020" => json!({ "title": title, "categories": [category("OGL v3.0")] }),
        "Hamlet" => json!({
            "title": title, "categories": [category("PD-old")],
            "pageprops": { "disambiguation": "" },
        }),
        "Emma (Austen)" => json!({ "title": title, "categories": [category("PD-old-80-US")] }),
        _ => json!({ "title": title, "missing": true }),
    }
}

fn wikisource_parse(title: &str) -> Option<Value> {
    let header = |data: &str| {
        format!(
            r#"<div class="ws-header wst-header-structure"><div class="wst-header-mainblock"><a href="/wiki/Pride_and_Prejudice_(1817)" title="{PRIDE}">← Contents</a></div>{data}</div>"#
        )
    };
    let licence = r#"<div class="licenseContainer licenseBanner"><p>This work is in the public domain.</p>
        <a href="/wiki/Pride_and_Prejudice_(1817)/Licence" title="Pride and Prejudice (1817)/Licence">more</a></div>"#;
    let (text, categories) = match title {
        PRIDE => (
            format!(
                r#"<div class="mw-parser-output">{}
                <p>PRIDE AND PREJUDICE: A NOVEL.</p>
                <ul><li><a href="/wiki/Pride_and_Prejudice_(1817)/Chapter_1" title="Pride and Prejudice (1817)/Chapter 1">Chapter 1</a></li>
                <li><a href="/wiki/Pride_and_Prejudice_(1817)/Chapter_2" class="mw-redirect" title="Pride and Prejudice (1817)/Chapter 2">Chapter 2</a></li>
                <li><a href="/wiki/Pride_and_Prejudice_(1817)/Volume_2" title="Pride and Prejudice (1817)/Volume 2">Volume 2</a></li>
                <li><a href="/wiki/Pride_and_Prejudice_(1817)/Chapter_1#top" title="Pride and Prejudice (1817)/Chapter 1">again</a></li>
                <li><a href="/w/index.php?title=Pride_and_Prejudice_(1817)/Missing&amp;action=edit&amp;redlink=1" class="new" title="Pride and Prejudice (1817)/Missing (page does not exist)">Missing</a></li>
                <li><a href="/wiki/Emma_(Austen)" title="Emma (Austen)">Emma</a></li></ul>{licence}</div>"#,
                header(
                    r#"<div class="wst-header-notes"><ul class="plainSister"><li>sister projects</li></ul>
                    <div class="wst-header-content header&#95;notes&#95;content"><p>the most famous of Jane Austen's <i>novels</i>.
                    <br /><i>one or more chapters are available in a spoken word format.</i></p></div></div>
                    <div class="ws-noexport" id="ws-data" style="speak:none"><span id="ws-article-id">942</span><span id="ws-title">Pride and Prejudice</span><span id="ws-year">1817</span><span id="ws-author">Jane Austen</span></div></div>"#,
                )
            ),
            json!([
                { "category": "Novels" }, { "category": "British_novels" },
                { "category": "PD-old" }, { "category": "1817_works" },
                { "category": "Validated_texts" }, { "category": "Hidden_tracking", "hidden": true },
            ]),
        ),
        "Pride and Prejudice (1817)/Volume 2" => (
            format!(
                r#"<div class="mw-parser-output">{}<ul>
                <li><a href="/wiki/Pride_and_Prejudice_(1817)/Volume_2/Chapter_1" title="Pride and Prejudice (1817)/Volume 2/Chapter 1">Chapter 1</a></li>
                <li><a href="/wiki/Pride_and_Prejudice_(1817)/Volume_2/Chapter_2" title="Pride and Prejudice (1817)/Volume 2/Chapter 2">Chapter 2</a></li></ul>{licence}</div>"#,
                header(r#"<div id="ws-data"><span id="ws-title">Pride and Prejudice</span></div></div>"#)
            ),
            json!([]),
        ),
        "Pride and Prejudice (1817)/Chapter 1" => (
            format!(
                r#"<div class="mw-parser-output">{}
                <p><span><span class="pagenum ws-pagenum" id="9" data-page-number="9"><span class="pagenum-inner ws-noexport">&#8203;</span></span></span>It is a truth universally acknowledged, that a single man in possession of a good fortune must be in want of a wife.</p>{licence}</div>"#,
                header(r#"<div id="ws-data"><span id="ws-title">Pride and Prejudice</span></div></div>"#)
            ),
            json!([]),
        ),
        "The Raven (Poe)" => (
            r#"<div class="mw-parser-output"><p>Once upon a midnight dreary, while I pondered, weak and weary.</p></div>"#.to_owned(),
            json!([{ "category": "PD-old" }, { "category": "Poems" }]),
        ),
        _ => return None,
    };
    Some(json!({ "parse": { "title": title, "text": text, "categories": categories } }))
}

#[tokio::test]
async fn wikisource_lists_open_works_and_reads_them_chapter_by_chapter() {
    let seen: Seen = Arc::default();
    let searches = seen.clone();
    let app = Router::new().route(
        "/w/api.php",
        get(move |uri: Uri| {
            let searches = searches.clone();
            async move {
                let params = query_params(&uri);
                match (params.get("action").map(String::as_str), params.get("list").map(String::as_str)) {
                    (Some("query"), Some("search")) => {
                        searches.lock().unwrap().push(params.clone());
                        let hits = if params["srsearch"].contains("emma") {
                            vec!["Emma", "Hamlet"]
                        } else {
                            vec![
                                "Pride and Prejudice (1817)/Chapter 5",
                                PRIDE,
                                "Gettysburg Address",
                                "Legislation Act 2020",
                                "Missing work",
                            ]
                        };
                        let hits: Vec<_> = hits.iter().map(|title| json!({ "ns": 0, "title": title })).collect();
                        json_response(StatusCode::OK, json!({
                            "query": { "searchinfo": { "totalhits": 100 }, "search": hits },
                        }))
                    }
                    (Some("query"), Some("allpages")) => {
                        assert_eq!(params["apprefix"], "Pride and Prejudice (1817)/");
                        let titles = ["Chapter 1", "Chapter 2", "Volume 2", "Volume 2/Chapter 1", "Volume 2/Chapter 2"];
                        let pages: Vec<_> = titles.iter().map(|title| json!({ "title": format!("{PRIDE}/{title}") })).collect();
                        json_response(StatusCode::OK, json!({ "query": { "allpages": pages } }))
                    }
                    (Some("query"), _) => {
                        let round = usize::from(params.contains_key("clcontinue"));
                        let asked: Vec<&str> = params["titles"].split('|').collect();
                        let redirects: Vec<_> = asked
                            .iter()
                            .filter(|title| **title == "Emma")
                            .map(|_| json!({ "from": "Emma", "to": "Emma (Austen)" }))
                            .collect();
                        let pages: Vec<_> = asked
                            .iter()
                            .map(|title| if *title == "Emma" { "Emma (Austen)" } else { title })
                            .map(|title| wikisource_page(title, round))
                            .collect();
                        let mut answer = json!({ "query": { "pages": pages, "redirects": redirects } });
                        if round == 0 && asked.contains(&PRIDE) {
                            answer["continue"] = json!({ "clcontinue": "942|PD-old", "continue": "||" });
                        }
                        json_response(StatusCode::OK, answer)
                    }
                    (Some("parse"), _) => match wikisource_parse(&params["page"]) {
                        Some(answer) => json_response(StatusCode::OK, answer),
                        None => json_response(StatusCode::OK, json!({
                            "error": { "code": "missingtitle", "info": "The page you specified doesn't exist." },
                        })),
                    },
                    _ => StatusCode::BAD_REQUEST.into_response(),
                }
            }
        }),
    );
    let address = serve(app);
    let host = local_host("wikisource", &["https://en.wikisource.org"], address).await;
    let provider = "dev.raygun.wikisource/texts";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        [PRIDE],
        "a chapter found stands for its work, once; versions pages, works without an open \
         licence and missing pages are left out"
    );
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://upload.wikimedia.org/p.jpg")
    );
    assert!(popular.has_next_page, "5 of 100 hits seen");
    let asked = last(&seen);
    assert!(asked["srsearch"].starts_with("incategory:\"Featured_texts\""));
    assert!(asked["srsearch"].contains("-incategory:\"Versions_pages\""));
    assert_eq!(
        (asked["srsort"].as_str(), asked["sroffset"].as_str()),
        ("incoming_links_desc", "0")
    );

    host.manga_list(provider, MangaList::Latest, 1)
        .await
        .unwrap();
    let asked = last(&seen);
    assert!(asked["srsearch"].contains("incategory:\"Validated_texts\""));
    assert!(asked["srsearch"].contains("Novels|Short_stories"));
    assert_eq!(asked["srsort"], "create_timestamp_desc");

    let found = host.manga_search(provider, "emma", 2).await.unwrap();
    let ids: Vec<_> = found.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        ["Emma (Austen)"],
        "redirects are followed; disambiguation pages left out"
    );
    let asked = last(&seen);
    assert_eq!(
        (asked["srsort"].as_str(), asked["sroffset"].as_str()),
        ("relevance", "30")
    );
    host.manga_search_filtered(
        provider,
        "",
        1,
        &filters(&[("genre", "Collections of poetry")]),
    )
    .await
    .unwrap();
    let asked = last(&seen);
    assert!(asked["srsearch"].contains("incategory:\"Collections_of_poetry\""));
    assert_eq!(asked["srsort"], "incoming_links_desc");

    let details = host.manga_details(provider, PRIDE).await.unwrap();
    assert_eq!(details.title, "Pride and Prejudice");
    assert_eq!(details.authors, ["Jane Austen"]);
    assert_eq!(
        details.description.as_deref(),
        Some("The most famous of Jane Austen's novels.\n\nPublished 1817.")
    );
    assert_eq!(
        details.genres,
        ["Novels", "British novels"],
        "no licence, upkeep, date or hidden categories"
    );
    assert_eq!(
        details.cover_url.as_deref(),
        Some("https://upload.wikimedia.org/p.jpg")
    );
    assert_eq!(
        details.url.as_deref(),
        Some(format!("http://{address}/wiki/Pride_and_Prejudice_(1817)").as_str())
    );

    let chapters = host.manga_chapters(provider, PRIDE).await.unwrap();
    let listed: Vec<_> = chapters
        .iter()
        .map(|chapter| (chapter.id.as_str(), chapter.name.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("Pride and Prejudice (1817)/Chapter 1", "Chapter 1"),
            ("Pride and Prejudice (1817)/Chapter 2", "Chapter 2"),
            (
                "Pride and Prejudice (1817)/Volume 2/Chapter 1",
                "Volume 2 · Chapter 1"
            ),
            (
                "Pride and Prejudice (1817)/Volume 2/Chapter 2",
                "Volume 2 · Chapter 2"
            ),
        ],
        "in the main page's order, a volume's chapters in its place; red links, other works \
         and the licence box's links aren't chapters"
    );
    let text = host.novel_passage(provider, &chapters[0].id).await.unwrap();
    assert_eq!(
        text,
        [PassageBlock::Paragraph(
            "It is a truth universally acknowledged, that a single man in possession of a good \
             fortune must be in want of a wife."
                .into()
        )],
        "no header, page number or licence box"
    );

    let poem = host
        .manga_chapters(provider, "The Raven (Poe)")
        .await
        .unwrap();
    assert_eq!(
        (poem.len(), poem[0].id.as_str(), poem[0].name.as_str()),
        (1, "The Raven (Poe)", "Full text"),
        "a work on one page is one chapter"
    );
    assert!(host.manga_details(provider, "No such work").await.is_err());
}

// ---- Wikimedia Commons ----

fn commons_audio(title: &str, index: u32, info: Value) -> Value {
    json!({ "ns": 6, "title": format!("File:{title}"), "index": index, "videoinfo": [info] })
}

fn commons_songs() -> Vec<Value> {
    let licensed = |extra: &[(&str, &str)]| {
        let mut fields = vec![("License", "pd"), ("LicenseShortName", "Public domain")];
        fields.extend_from_slice(extra);
        meta(&fields)
    };
    vec![
        commons_audio(
            "Moonlight Sonata Presto.ogg",
            1,
            json!({
                "mediatype": "AUDIO", "mime": "application/ogg", "duration": 493.7,
                "url": "https://upload.wikimedia.org/1/1d/Moonlight_Sonata_Presto.ogg?utm_source=commons.wikimedia.org&utm_campaign=imageinfo",
                "extmetadata": licensed(&[
                    ("ObjectName", "Moonlight Sonata Presto"),
                    ("Artist", "<a href=\"https://en.wikipedia.org/wiki/Beethoven\">Ludwig van Beethoven</a>"),
                    ("Categories", "Piano sonatas|Ludwig van Beethoven"),
                ]),
                "derivatives": [{ "src": "https://upload.wikimedia.org/transcoded/Presto.ogg.mp3", "type": "audio/mpeg", "transcodekey": "mp3", "bandwidth": 179_128 }],
            }),
        ),
        commons_audio(
            "Sonata.mid",
            2,
            json!({
                "mediatype": "AUDIO", "mime": "audio/midi", "duration": 300.0,
                "url": "https://upload.wikimedia.org/Sonata.mid", "extmetadata": licensed(&[]),
            }),
        ),
        commons_audio(
            "En-us-presto.ogg",
            3,
            json!({
                "mediatype": "AUDIO", "mime": "application/ogg", "duration": 1.2,
                "url": "https://upload.wikimedia.org/En-us-presto.ogg", "extmetadata": licensed(&[]),
            }),
        ),
        commons_audio(
            "Presto (GFDL).ogg",
            4,
            json!({
                "mediatype": "AUDIO", "mime": "application/ogg", "duration": 200.0,
                "url": "https://upload.wikimedia.org/gfdl.ogg",
                "extmetadata": meta(&[("License", "gfdl"), ("LicenseShortName", "GFDL")]),
            }),
        ),
        commons_audio(
            "Presto in the nude.ogg",
            5,
            json!({
                "mediatype": "AUDIO", "mime": "application/ogg", "duration": 200.0,
                "url": "https://upload.wikimedia.org/nude.ogg", "extmetadata": licensed(&[("Categories", "Nudity|Music")]),
            }),
        ),
        commons_audio(
            "Piano_etude.flac",
            6,
            json!({
                "mediatype": "AUDIO", "mime": "audio/flac", "duration": 200.0,
                "url": "https://upload.wikimedia.org/Piano_etude.flac?utm_source=commons.wikimedia.org",
                "extmetadata": meta(&[("License", "cc-by-sa-4.0"), ("LicenseShortName", "CC BY-SA 4.0")]),
            }),
        ),
    ]
}

fn commons_video_info() -> Value {
    let webm = |key: &str, height: u32| {
        json!({
            "src": format!("https://upload.wikimedia.org/transcoded/Sprite.webm.{key}"),
            "type": "video/webm; codecs=\"vp9, opus\"", "transcodekey": key, "height": height,
        })
    };
    json!({
        "mediatype": "VIDEO", "mime": "video/webm", "width": 2048, "height": 858, "duration": 629.8,
        "url": "https://upload.wikimedia.org/7/76/Sprite.webm?utm_source=commons.wikimedia.org&utm_content=original",
        "thumburl": "https://upload.wikimedia.org/thumb/Sprite.webm.jpg?utm_source=commons.wikimedia.org",
        "extmetadata": meta(&[
            ("ObjectName", "Sprite Fright"),
            ("ImageDescription", "Short film <i>Sprite Fright</i> by the Blender Foundation"),
            ("Artist", "<bdi><a href=\"https://www.wikidata.org/wiki/Q66424857\">Blender Studio</a></bdi>"),
            ("LicenseShortName", "CC BY 4.0"),
            ("License", "cc-by-4.0"),
            ("DateTimeOriginal", "Published on 29 October 2021"),
            ("Categories", "Sprite Fright (Blender Foundation Shortfilm)|WebM videos|Videos of 2021|Media from YouTube|Blender movies|Files with closed captioning in English|CC-BY-4.0"),
        ]),
        "derivatives": [
            webm("240p.vp9.webm", 178),
            { "src": "https://upload.wikimedia.org/transcoded/Sprite.webm.360p.mpeg4.mov", "type": "video/quicktime; codecs=\"mp4v\"", "transcodekey": "360p.mpeg4.mov" },
            webm("480p.vp9.webm", 358),
            { "src": "https://upload.wikimedia.org/transcoded/Sprite.webm.160p.ogv", "type": "video/ogg; codecs=\"theora, vorbis\"", "transcodekey": "160p.ogv" },
            webm("1080p.vp9.webm", 804),
        ],
        "timedtext": [
            { "src": "https://commons.wikimedia.org/w/api.php?action=timedtext&title=File%3ASprite.webm&lang=en&trackformat=vtt", "srclang": "en", "label": "English", "kind": "subtitles" },
            { "src": "https://commons.wikimedia.org/w/api.php?action=timedtext&title=File%3ASprite.webm&lang=de&trackformat=vtt", "srclang": "de", "label": "Deutsch", "kind": "subtitles" },
        ],
    })
}

#[tokio::test]
async fn wikimedia_commons_offers_openly_licensed_songs_and_videos() {
    let seen: Seen = Arc::default();
    let searches = seen.clone();
    let app = Router::new().route(
        "/w/api.php",
        get(move |uri: Uri| {
            let searches = searches.clone();
            async move {
                let params = query_params(&uri);
                if params.get("generator").map(String::as_str) == Some("search") {
                    searches.lock().unwrap().push(params.clone());
                    let search = &params["gsrsearch"];
                    if search.starts_with("filetype:audio") {
                        return json_response(StatusCode::OK, json!({ "query": { "pages": commons_songs() } }));
                    }
                    let sprite = json!({ "ns": 6, "title": "File:Sprite.webm", "index": 1, "videoinfo": [commons_video_info()] });
                    let mut explicit = commons_video_info();
                    explicit["extmetadata"]["Categories"] = json!({ "value": "Sexual intercourse in humans" });
                    let explicit = json!({ "ns": 6, "title": "File:Explicit.webm", "index": 2, "videoinfo": [explicit] });
                    let mut song = commons_songs()[0].clone();
                    song["index"] = json!(3);
                    return json_response(StatusCode::OK, json!({
                        "continue": { "gsroffset": 24, "continue": "gsroffset||" },
                        "query": { "pages": [explicit, sprite, song] },
                    }));
                }
                match params.get("titles").map(String::as_str) {
                    Some("File:Sprite.webm") => json_response(StatusCode::OK, json!({
                        "query": { "pages": [{ "ns": 6, "title": "File:Sprite.webm", "videoinfo": [commons_video_info()] }] },
                    })),
                    _ => json_response(StatusCode::OK, json!({
                        "query": { "pages": [{ "ns": 6, "title": "File:Gone.webm", "missing": true }] },
                    })),
                }
            }
        }),
    );
    let address = serve(app);
    let host = local_host(
        "wikimedia-commons",
        &["https://commons.wikimedia.org"],
        address,
    )
    .await;

    let results = host.search_music("moonlight", 10).await;
    assert_eq!(results.len(), 1);
    let tracks = &results[0].tracks;
    let titles: Vec<_> = tracks.iter().map(|track| track.title.as_str()).collect();
    assert_eq!(
        titles,
        ["Moonlight Sonata Presto", "Piano etude"],
        "no MIDI, no clips, no files without a public-domain or CC licence, nothing filed under nudity"
    );
    assert_eq!(tracks[0].artist.as_deref(), Some("Ludwig van Beethoven"));
    assert_eq!(tracks[0].duration_ms, Some(493_700));
    let Some(MediaSource::Url { url, headers }) = &tracks[0].source else {
        panic!("{:?}", tracks[0].source);
    };
    assert_eq!(
        url, "https://upload.wikimedia.org/transcoded/Presto.ogg.mp3",
        "Commons' MP3 copy"
    );
    assert!(headers["User-Agent"].starts_with("Raygun/"));
    assert!(
        matches!(&tracks[1].source, Some(MediaSource::Url { url, .. }) if url == "https://upload.wikimedia.org/Piano_etude.flac"),
        "the file itself without its tracking query: {:?}",
        tracks[1].source
    );
    let asked = last(&seen);
    assert_eq!(
        asked["gsrsearch"],
        "filetype:audio -filemime:audio/midi filesize:>300 moonlight"
    );

    let query = TrackQuery {
        title: "Moonlight Sonata: Presto (Live)".into(),
        artist: Some("Ludwig van Beethoven & Orchestra".into()),
        album: None,
        duration_ms: None,
    };
    let answers = host
        .stream_candidates(&query, 10, raygun_resolver::QualityTarget::Any)
        .await;
    let candidates = answers[0].result.as_ref().unwrap();
    let offered: Vec<_> = candidates
        .iter()
        .map(|candidate| {
            (
                candidate.title.as_str(),
                candidate.quality.as_deref(),
                candidate.bitrate_kbps,
            )
        })
        .collect();
    assert_eq!(
        offered,
        [("Moonlight Sonata Presto", Some("MP3"), Some(179))],
        "only recordings named with every word of the title"
    );
    let searched: Vec<_> = seen
        .lock()
        .unwrap()
        .iter()
        .rev()
        .take(2)
        .map(|params| params["gsrsearch"].clone())
        .collect();
    assert!(searched.contains(&"filetype:audio -filemime:audio/midi filesize:>300 Moonlight Sonata Presto Ludwig van Beethoven".to_owned()));
    assert!(searched.contains(
        &"filetype:audio -filemime:audio/midi filesize:>300 Moonlight Sonata Presto".to_owned()
    ));

    let provider = "dev.raygun.wikimedia-commons/videos";
    assert_eq!(host.providers(VIDEO_SOURCE).await.len(), 1);
    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["Sprite.webm"], "no audio, nothing filed under sex");
    assert_eq!(popular.items[0].title, "Sprite Fright");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://upload.wikimedia.org/thumb/Sprite.webm.jpg")
    );
    assert!(popular.has_next_page);
    let asked = last(&seen);
    assert_eq!(
        asked["gsrsearch"],
        "filetype:video incategory:\"Media_of_the_day\""
    );
    assert_eq!(
        (asked["gsrsort"].as_str(), asked["gsroffset"].as_str()),
        ("incoming_links_desc", "0")
    );
    host.manga_list(provider, MangaList::Latest, 2)
        .await
        .unwrap();
    let asked = last(&seen);
    assert_eq!(
        (asked["gsrsort"].as_str(), asked["gsroffset"].as_str()),
        ("create_timestamp_desc", "24")
    );
    host.manga_search(provider, "apollo 11", 1).await.unwrap();
    let asked = last(&seen);
    assert_eq!(
        (asked["gsrsearch"].as_str(), asked["gsrsort"].as_str()),
        ("filetype:video apollo 11", "relevance")
    );
    host.manga_search_filtered(
        provider,
        "moon",
        1,
        &filters(&[
            ("collection", "featured"),
            ("sort", "create_timestamp_desc"),
        ]),
    )
    .await
    .unwrap();
    let asked = last(&seen);
    assert_eq!(
        asked["gsrsearch"],
        "filetype:video incategory:\"Media_of_the_day\" moon"
    );
    assert_eq!(asked["gsrsort"], "create_timestamp_desc");

    let details = host.manga_details(provider, "Sprite.webm").await.unwrap();
    assert_eq!(details.title, "Sprite Fright");
    assert_eq!(
        details.description.as_deref(),
        Some(
            "Short film Sprite Fright by the Blender Foundation\n\nLicence: CC BY 4.0 · By Blender Studio"
        ),
        "the licence and whom it asks to credit"
    );
    assert_eq!(details.year, Some(2021));
    assert_eq!(
        details.genres,
        [
            "Sprite Fright (Blender Foundation Shortfilm)",
            "Blender movies"
        ]
    );
    assert_eq!(details.media_type.as_deref(), Some("other"));

    let episodes = host.video_episodes(provider, "Sprite.webm").await.unwrap();
    let servers = host.video_servers(provider, &episodes[0].id).await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    let links: Vec<_> = videos
        .iter()
        .map(|video| (video.quality, video.label.as_deref().unwrap_or_default()))
        .collect();
    assert_eq!(
        links,
        [
            (Some(1080), "1080p · VP9"),
            (Some(858), "Original · 2048×858"),
            (Some(480), "480p · VP9"),
            (Some(240), "240p · VP9"),
        ],
        "WebM copies by their names' heights and the original, best first; no Theora or MPEG-4 Part 2"
    );
    assert_eq!(
        videos[1].url,
        "https://upload.wikimedia.org/7/76/Sprite.webm"
    );
    let languages: Vec<_> = videos[0]
        .subtitles
        .iter()
        .map(|track| track.lang.as_deref())
        .collect();
    assert_eq!(languages, [Some("en"), Some("de")]);
    assert!(host.video_servers(provider, "Gone.webm").await.is_err());
}

// ---- NASA ----

fn nasa_item(id: &str, title: &str, date: &str, description: &str) -> Value {
    let picture = |size: &str, width: u32| {
        json!({
            "href": format!("https://images-assets.nasa.gov/video/{id}/{id}~{size}.jpg"),
            "rel": "alternate", "render": "image", "width": width,
        })
    };
    json!({
        "href": format!("https://images-assets.nasa.gov/video/{id}/collection.json"),
        "data": [{
            "nasa_id": id, "title": title, "date_created": date, "description": description,
            "media_type": "video", "center": "JPL", "keywords": ["Crew", "Mars"],
            "photographer": "NASA/Joel Kowsky",
        }],
        "links": [picture("orig", 1920), picture("large", 800), picture("medium", 400),
            { "href": format!("https://images-assets.nasa.gov/video/{id}/{id}.vtt"), "rel": "captions" }],
    })
}

const NASA_ID: &str = "jsc2026 crew";

#[tokio::test]
async fn nasa_lists_public_domain_videos_and_their_encodes() {
    let seen: Seen = Arc::default();
    let searches = seen.clone();
    let app = Router::new()
        .route(
            "/popular.json",
            get(|| async {
                let mut image = nasa_item("picture", "A picture", "2020-01-01T00:00:00Z", "");
                image["data"][0]["media_type"] = json!("image");
                json_response(StatusCode::OK, json!({ "collection": { "items": [
                    nasa_item("reentry one", "Reentry", "2022-12-11T00:00:00Z", "Orion returns."),
                    image,
                    nasa_item("news", "News", "2022-12-11T00:00:00Z", "© Getty Images. Used by NASA."),
                ] } }))
            }),
        )
        .route(
            "/search",
            get(move |uri: Uri| {
                let searches = searches.clone();
                async move {
                    let params = query_params(&uri);
                    searches.lock().unwrap().push(params.clone());
                    let items = if params.contains_key("nasa_id") {
                        vec![nasa_item(NASA_ID, "jsc2026m000132_Meet_the_Crew", "2026-03-25T00:00:00Z", "<p>Crew &amp; rocket.</p>")]
                    } else {
                        vec![
                            nasa_item("older", "JPL-20200210-MARSf-0001-Mars Sample Return-1080", "2025-01-01T00:00:00Z", "Samples."),
                            nasa_item(NASA_ID, "jsc2026m000132_Meet_the_Crew", "2026-03-25T00:00:00Z", "Crew."),
                        ]
                    };
                    json_response(StatusCode::OK, json!({ "collection": {
                        "items": items, "metadata": { "total_hits": 100 }, "links": [{ "rel": "next" }],
                    } }))
                }
            }),
        )
        .fallback(|uri: Uri| async move {
            match uri.path() {
                "/asset/jsc2026%20crew" => {
                    let file = |name: &str| json!({ "href": format!("http://images-assets.nasa.gov/video/{NASA_ID}/{name}") });
                    json_response(StatusCode::OK, json!({ "collection": { "items": [
                        file(&format!("{NASA_ID}~orig.mp4")), file(&format!("{NASA_ID}.srt")),
                        file(&format!("{NASA_ID}~mobile.mp4")), file(&format!("{NASA_ID}~large.mp4")),
                        file(&format!("{NASA_ID}~preview.mp4")), file(&format!("{NASA_ID}~small.mp4")),
                        file(&format!("{NASA_ID}~medium.mp4")), file(&format!("{NASA_ID}.vtt")),
                        file("metadata.json"), file(&format!("{NASA_ID}~thumb.jpg")),
                    ] } }))
                }
                "/video/jsc2026%20crew/metadata.json" => {
                    json_response(StatusCode::OK, json!({ "XMP:VideoFrameSize": { "Unit": "pixel", "H": 2160, "W": 3840 } }))
                }
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        });
    let address = serve(app);
    let host = local_host(
        "nasa",
        &[
            "https://images-api.nasa.gov",
            "https://images-assets.nasa.gov",
        ],
        address,
    )
    .await;
    let provider = "dev.raygun.nasa/videos";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        ["reentry one"],
        "videos only, none crediting another rights holder"
    );
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/video/reentry%20one/reentry%20one~large.jpg").as_str()),
        "the largest picture up to 800 pixels, its address escaped"
    );
    assert!(!popular.has_next_page);

    let latest = host
        .manga_list(provider, MangaList::Latest, 1)
        .await
        .unwrap();
    let titles: Vec<_> = latest
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        titles,
        ["Meet the Crew", "Mars Sample Return"],
        "newest first, file names made into titles"
    );
    assert!(latest.has_next_page);
    let asked = last(&seen);
    let year = chrono::Datelike::year(&chrono::Utc::now());
    assert_eq!(asked["year_start"], (year - 1).to_string());
    assert_eq!(
        (asked["media_type"].as_str(), asked["page_size"].as_str()),
        ("video", "30")
    );

    host.manga_search_filtered(
        provider,
        "mars",
        2,
        &filters(&[("center", "JPL"), ("decade", "2010")]),
    )
    .await
    .unwrap();
    let asked = last(&seen);
    assert_eq!(
        (
            asked["q"].as_str(),
            asked["center"].as_str(),
            asked["year_start"].as_str(),
            asked["year_end"].as_str(),
            asked["page"].as_str()
        ),
        ("mars", "JPL", "2010", "2019", "2")
    );
    host.manga_search(provider, "", 1).await.unwrap();
    assert_eq!(
        last(&seen)["q"],
        "nasa",
        "the library needs something to search by"
    );
    assert_eq!(host.manga_filters(provider).await.unwrap().len(), 2);

    let details = host.manga_details(provider, NASA_ID).await.unwrap();
    assert_eq!(details.title, "Meet the Crew");
    assert_eq!(
        details.description.as_deref(),
        Some("Crew & rocket.\n\nNASA Jet Propulsion Laboratory · By NASA/Joel Kowsky")
    );
    assert_eq!(
        (details.year, details.media_type.as_deref()),
        (Some(2026), Some("other"))
    );
    assert_eq!(details.genres, ["Crew", "Mars"]);
    assert_eq!(details.artists, ["NASA/Joel Kowsky"]);

    let servers = host.video_servers(provider, NASA_ID).await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    let labels: Vec<_> = videos
        .iter()
        .map(|video| video.label.as_deref().unwrap_or_default())
        .collect();
    assert_eq!(
        labels,
        ["1080p", "720p", "360p", "270p", "180p", "Original · 2160p"]
    );
    assert_eq!(
        videos[0].url,
        format!("http://{address}/video/jsc2026%20crew/jsc2026%20crew~large.mp4")
    );
    let captions: Vec<_> = videos[0]
        .subtitles
        .iter()
        .map(|track| track.url.as_str())
        .collect();
    assert_eq!(
        captions,
        [format!("http://{address}/video/jsc2026%20crew/jsc2026%20crew.vtt").as_str()],
        "one caption file, WebVTT over SubRip"
    );
}

// ---- Blender open movies ----

#[tokio::test]
async fn blender_movies_lists_the_films_and_plays_them_from_commons() {
    let batches = Arc::new(AtomicUsize::new(0));
    let counted = batches.clone();
    let app = Router::new().route(
        "/w/api.php",
        get(move |uri: Uri| {
            counted.fetch_add(1, Ordering::SeqCst);
            async move {
                let params = query_params(&uri);
                let pages: Vec<_> = params["titles"]
                    .split('|')
                    .map(|title| {
                        if title.starts_with("File:Glass Half") {
                            return json!({ "title": title, "missing": true });
                        }
                        let slug = title.trim_start_matches("File:").replace(' ', "_");
                        json!({ "title": title, "videoinfo": [{
                            "url": format!("https://upload.wikimedia.org/{slug}?utm_source=commons.wikimedia.org"),
                            "thumburl": format!("https://upload.wikimedia.org/thumb/{slug}.jpg?utm_source=commons.wikimedia.org"),
                            "mime": "video/webm", "width": 4096, "height": 1744,
                            "extmetadata": meta(&[
                                ("ImageDescription", "An open movie."),
                                ("LicenseShortName", "CC BY 3.0"),
                                ("Artist", "Durian Open Movie project; about page (archived link)"),
                            ]),
                            "derivatives": [
                                { "src": format!("https://upload.wikimedia.org/transcoded/{slug}.480p.vp9.webm"), "type": "video/webm; codecs=\"vp9, opus\"", "transcodekey": "480p.vp9.webm" },
                                { "src": format!("https://upload.wikimedia.org/transcoded/{slug}.1080p.vp9.webm"), "type": "video/webm; codecs=\"vp9, opus\"", "transcodekey": "1080p.vp9.webm" },
                            ],
                            "timedtext": [{ "src": "https://commons.wikimedia.org/w/api.php?action=timedtext&lang=en", "srclang": "en", "label": "English" }],
                        }] })
                    })
                    .collect();
                json_response(StatusCode::OK, json!({ "query": { "pages": pages } }))
            }
        }),
    );
    let address = serve(app);
    let host = local_host(
        "blender-movies",
        &["https://commons.wikimedia.org"],
        address,
    )
    .await;
    let provider = "dev.raygun.blender-movies/movies";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(popular.items.len(), 17);
    assert_eq!(popular.items[0].title, "Big Buck Bunny");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://upload.wikimedia.org/thumb/Big_Buck_Bunny_4K.webm.jpg")
    );
    let glass = popular
        .items
        .iter()
        .find(|item| item.id == "glass-half")
        .unwrap();
    assert_eq!(
        glass.cover_url, None,
        "a film gone from Commons has no cover"
    );
    assert!(!popular.has_next_page);
    let latest = host
        .manga_list(provider, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_eq!(latest.items[0].id, "singularity", "newest first");
    let llamas = host.manga_search(provider, "caminandes", 1).await.unwrap();
    assert_eq!(llamas.items.len(), 3);
    let sintel = host.manga_search(provider, "sint 2010", 1).await.unwrap();
    let ids: Vec<_> = sintel.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(ids, ["sintel"]);

    let details = host.manga_details(provider, "sintel").await.unwrap();
    assert_eq!(
        (details.title.as_str(), details.year),
        ("Sintel", Some(2010))
    );
    assert_eq!(
        details.description.as_deref(),
        Some("An open movie.\n\nLicence: CC BY 3.0 · By Durian Open Movie project")
    );
    assert_eq!(details.media_type.as_deref(), Some("movie"));
    assert!(host.manga_details(provider, "glass-half").await.is_err());
    assert!(host.manga_details(provider, "not-a-film").await.is_err());

    let servers = host.video_servers(provider, "sintel").await.unwrap();
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    let qualities: Vec<_> = videos.iter().map(|video| video.quality).collect();
    assert_eq!(qualities, [Some(1744), Some(1080), Some(480)]);
    assert_eq!(
        videos[0].url,
        "https://upload.wikimedia.org/Sintel_movie_4K.webm"
    );
    assert_eq!(videos[1].subtitles.len(), 1);
    assert_eq!(
        batches.load(Ordering::SeqCst),
        1,
        "every film's file is asked for in one request, once"
    );
}

// ---- Internet Archive ----

#[tokio::test]
async fn internet_archive_asks_for_open_audio_and_public_domain_films_only() {
    let seen: Seen = Arc::default();
    let searches = seen.clone();
    let app = Router::new().route(
        "/advancedsearch.php",
        get(move |uri: Uri| {
            let searches = searches.clone();
            async move {
                searches.lock().unwrap().push(query_params(&uri));
                json_response(
                    StatusCode::OK,
                    json!({ "response": { "numFound": 0, "docs": [] } }),
                )
            }
        }),
    );
    let address = serve(app);
    let host = local_host("internet-archive", &["https://archive.org"], address).await;

    host.search_music("ambient", 5).await;
    let audio = last(&seen)["q"].clone();
    assert!(audio.starts_with("(title:(ambient) OR creator:(ambient)) AND mediatype:audio"));
    assert!(audio.contains(
        "collection:(netlabels OR musopen) AND (licenseurl:*creativecommons.org* OR licenseurl:*publicdomain*)"
    ));
    assert!(
        audio.contains("-collection:(78rpm OR georgeblood)"),
        "{audio}"
    );
    assert!(
        !audio.contains("opensource_audio"),
        "open uploads declare their own licences"
    );
    let query = TrackQuery {
        title: "Calls and Echoes".into(),
        artist: Some("Kai Engel".into()),
        album: None,
        duration_ms: None,
    };
    host.stream_candidates(&query, 5, raygun_resolver::QualityTarget::Any)
        .await;
    assert!(
        seen.lock()
            .unwrap()
            .iter()
            .rev()
            .take(3)
            .all(|params| params["q"].contains("-collection:(78rpm OR georgeblood)"))
    );

    let provider = "dev.raygun.internet-archive/films";
    host.manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let films = last(&seen)["q"].clone();
    let before = chrono::Datelike::year(&chrono::Utc::now()) - 95;
    assert!(films.starts_with("collection:(feature_films) AND mediatype:(movies) AND "));
    assert!(
        films.contains(&format!("year:[1800 TO {}]", before - 1)),
        "public domain by age: {films}"
    );
    assert!(films.contains(
        "((licenseurl:*publicdomain\\/mark* OR licenseurl:*licenses\\/publicdomain*) AND year:[1800 TO 1977])"
    ));
    host.manga_search_filtered(provider, "", 1, &filters(&[("collection", "prelinger")]))
        .await
        .unwrap();
    let prelinger = last(&seen)["q"].clone();
    assert_eq!(
        prelinger,
        "collection:(prelinger) AND mediatype:(movies) AND licenseurl:*publicdomain*"
    );
}

// ---- Library of Congress ----

/// A film as loc.gov's lists give it.
fn loc_film(id: &str, title: &str, date: &str) -> Value {
    json!({
        "id": format!("http://www.loc.gov/item/{id}/"),
        "url": format!("https://www.loc.gov/item/{id}/"),
        "title": title, "date": date, "access_restricted": false,
        "partof": ["national screening room", "catalog"],
        "resources": [{
            "caption": "Digital viewing copy", "height": 1080, "width": 1440,
            "video": format!("https://tile.loc.gov/storage-services/service/mbrs/ntscrm/{id}/{id}.mp4"),
            "video_stream": format!("https://tile.loc.gov/streaming-services/iiif/service:mbrs:ntscrm:{id}:{id}/full/full/0/full/default.m3u8"),
            "poster": format!("//tile.loc.gov/storage-services/service/mbrs/ntscrm/{id}/{id}.jpg"),
        }],
    })
}

/// A film's own page (`at=item,resources`) with its rights statement and advisory.
fn loc_item(id: &str, date: &str, advisory: Option<&str>) -> Value {
    let mut item = json!({
        "title": "Pack mules with ammunition on the Santiago Trail, Cuba", "date": date,
        "access_restricted": false,
        "partof": [{ "title": "national screening room", "count": 1294 }],
        "rights": ["<p>The Library of Congress is not aware of any U.S. copyright or other restrictions in the vast majority of motion pictures in these collections. Absent any such restrictions, these materials are free to use and reuse.</p>\r\n<p>In rare cases, copyrighted motion pictures are made available by special permission.</p>\r\n<p>Credit Line: Library of Congress, Motion Picture, Broadcasting, and Recorded Sound Division.</p>\r\n<p>More about <a href=\"/legal/\">Copyright and other Restrictions</a>.</p>"],
        "summary": ["A great pack train approaches rapidly."],
        "created_published": ["United States : Edison Manufacturing Co., 1898."],
        "contributor_names": [
            "Paley, William Daly, 1857-1924, camera", "Thomas A. Edison, Inc",
            "Theodore Roosevelt Association Collection (Library of Congress)",
        ],
        "contributors": [{ "paley, william daly": "https://www.loc.gov/search/?fa=contributor:paley" }],
        "genre": ["Actualities (Motion pictures)", "Silent films"],
    });
    if let Some(advisory) = advisory {
        item["rights_advisory"] = json!([advisory]);
    }
    json!({ "item": item, "resources": loc_film(id, "", date)["resources"] })
}

const LOC_EDISON: &str = "/collections/edison-company-motion-pictures-and-sound-recordings/";

#[tokio::test]
async fn loc_keeps_to_the_api_limit_and_offers_public_domain_films_only() {
    let times: Arc<Mutex<Vec<std::time::Instant>>> = Arc::default();
    let seen: Seen = Arc::default();
    let (timed, searches) = (times.clone(), seen.clone());
    let app = Router::new().fallback(move |uri: Uri| {
        let (timed, searches) = (timed.clone(), searches.clone());
        async move {
            timed.lock().unwrap().push(std::time::Instant::now());
            let mut params = query_params(&uri);
            params.insert("path".to_owned(), uri.path().to_owned());
            searches.lock().unwrap().push(params);
            match uri.path() {
                LOC_EDISON => {
                    let mut later = loc_film("later", "A later film", "1955");
                    later["partof"] = json!(["national screening room"]);
                    let mut closed = loc_film("closed", "Closed", "1910");
                    closed["access_restricted"] = json!(true);
                    let mut jukebox = loc_film("jukebox", "Lent", "1910");
                    jukebox["partof"] = json!(["national jukebox"]);
                    let mut silent = loc_film("silent", "Pictures only", "1910");
                    silent["resources"] = json!([{ "image": "//tile.loc.gov/a.jpg" }]);
                    json_response(StatusCode::OK, json!({
                        "results": [
                            loc_film("98501077", "Pack mules with ammunition on the Santiago Trail, Cuba", "1898"),
                            later, closed, jukebox, silent,
                            loc_film("mp76000243", "Oscar Straus", "191?"),
                        ],
                        "pagination": { "current": 1, "next": "https://www.loc.gov/collections/x/?sp=2" },
                    }))
                }
                "/item/98501077/" => json_response(StatusCode::OK, loc_item("98501077", "1898", None)),
                "/item/mp76000243/" => json_response(
                    StatusCode::OK,
                    loc_item("mp76000243", "191?", Some("Rights status not evaluated.")),
                ),
                "/film-and-videos/" => StatusCode::TOO_MANY_REQUESTS.into_response(),
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    });
    let address = serve(app);
    let host = local_host("loc", &["https://www.loc.gov"], address).await;
    let provider = "dev.raygun.loc/films";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        ["98501077", "mp76000243"],
        "published before 1931, open, not the Jukebox's, with a video"
    );
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://tile.loc.gov/storage-services/service/mbrs/ntscrm/98501077/98501077.jpg")
    );
    assert!(popular.has_next_page);
    let asked = last(&seen);
    let cutoff = chrono::Datelike::year(&chrono::Utc::now()) - 96;
    assert_eq!(asked["dates"], format!("1800/{cutoff}"));
    assert_eq!(asked["fa"], "online-format:video");
    assert_eq!(asked["at"], "results,pagination");
    assert_eq!(asked["fo"], "json");

    let details = host.manga_details(provider, "98501077").await.unwrap();
    assert_eq!(details.year, Some(1898));
    let description = details.description.unwrap();
    assert!(
        description.contains("Made by: William Daly Paley (camera), Thomas A. Edison, Inc"),
        "{description}"
    );
    assert!(description.ends_with(
        "Public domain in the United States. Credit: Library of Congress, Motion Picture, Broadcasting, and Recorded Sound Division."
    ));
    let episodes = host.video_episodes(provider, "98501077").await.unwrap();
    assert_eq!(episodes.len(), 1);
    let servers = host.video_servers(provider, &episodes[0].id).await.unwrap();
    let videos = servers[0].videos.as_ref().unwrap();
    assert_eq!(videos[0].quality, Some(1080));
    assert!(videos[0].url.ends_with("/98501077.mp4"));
    assert_eq!(videos[1].format.as_deref(), Some("hls"));
    let refused = host
        .manga_details(provider, "mp76000243")
        .await
        .unwrap_err();
    assert!(refused.contains("rights restrictions"), "{refused}");
    assert_eq!(
        seen.lock().unwrap().len(),
        3,
        "a film's episodes and videos come from its page, read once"
    );

    // Too many requests: the API shuts the client out for an hour, and nothing is asked meanwhile.
    let blocked = host.manga_search(provider, "fire", 1).await.unwrap_err();
    assert!(blocked.contains("slow down"), "{blocked}");
    let again = host.manga_search(provider, "parade", 1).await.unwrap_err();
    assert!(again.contains("slow down"), "{again}");
    let times = times.lock().unwrap();
    assert_eq!(times.len(), 4, "asked nothing while shut out");
    for pair in times.windows(2) {
        assert!(
            pair[1] - pair[0] >= std::time::Duration::from_millis(3000),
            "one request at a time, 3.1 s apart: {:?}",
            pair[1] - pair[0]
        );
    }
}

// ---- National Archives ----

/// A first-party extension with `bases` pointed at `address`, its network access limited to it,
/// and its `apiKey` setting given.
async fn keyed_local_host(
    name: &str,
    id: &str,
    bases: &[&str],
    address: SocketAddr,
    key: &str,
) -> ExtensionHost {
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
        .load_with_settings(
            vec![ExtensionSource::built_in(&[
                ("raygun-extension.json", &manifest.to_string()),
                ("dist/index.js", &code),
            ])],
            crate::UserSettings::from([(
                id.to_owned(),
                BTreeMap::from([("apiKey".to_owned(), json!(key))]),
            )]),
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

/// A moving-image record as the catalogue describes it (shapes from its open data).
fn nara_record(id: u64, title: &str, use_status: &str, objects: Value) -> Value {
    json!({
        "naId": id, "title": title, "levelOfDescription": "item",
        "generalRecordsTypes": ["Moving Images"],
        "productionDates": [{ "year": 1936, "logicalDate": "1936-01-01" }],
        "scopeAndContentNote": "Horses of the Spanish Riding School perform.",
        "shotList": "LS, riders enter the hall. MS, a horse in levade.",
        "useRestriction": { "status": use_status },
        "accessRestriction": { "status": "Unrestricted" },
        "digitalObjects": objects,
        "ancestors": [
            { "distance": 2, "levelOfDescription": "recordGroup", "naId": 440, "recordGroupNumber": 111,
              "title": "Records of the Office of the Chief Signal Officer" },
            { "distance": 1, "levelOfDescription": "series", "naId": 25061,
              "title": "Motion Picture Films from the Army Library Copy Collection",
              "creators": [{ "creatorType": "Most Recent",
                "heading": "Department of Defense. Department of the Army. Office of the Chief Signal Officer. (1964 - )" }] },
        ],
        "localIdentifier": format!("111-M-{id}"),
    })
}

fn nara_file(designator: Option<&str>, url: &str) -> Value {
    let mut object = json!({
        "objectType": "Audio/Visual File (MP4)",
        "objectFilename": url.rsplit('/').next().unwrap(),
        "objectUrl": url,
        "objectDescription": "Download this film.",
    });
    if let Some(designator) = designator {
        object["objectDesignator"] = json!(designator);
    }
    object
}

/// `records/search`'s answer, wrapped as the API wraps it.
fn nara_answer(records: &[Value], total: u64) -> Value {
    let hits: Vec<Value> = records
        .iter()
        .map(|record| json!({ "_id": record["naId"].to_string(), "_source": { "record": record } }))
        .collect();
    json!({
        "body": { "hits": { "total": { "value": total, "relation": "eq" }, "hits": hits } },
        "statusCode": 200,
    })
}

const NARA_KEY: &str = "a-key";

#[tokio::test]
async fn nara_offers_unrestricted_films_with_their_reels() {
    let lz = "https://catalog.archives.gov/medialz/mopix/111/m";
    let records = vec![
        nara_record(
            35486,
            "DIE GANGARTEN DES PFERDES (SPANISCHE REITSCHULE)",
            "Unrestricted",
            json!([
                nara_file(Some("Reel 2 of 2"), &format!("{lz}/111-m-111-r2.mp4")),
                nara_file(
                    Some("Reel 1 of 2"),
                    "https://s3.amazonaws.com/NARAprodstorage/lz/mopix/111/m/111-m-111-r1.mp4",
                ),
                nara_file(
                    Some("Reel 1 of 2"),
                    "https://s3.dualstack.us-east-1.amazonaws.com/NARAprodstorage/lz/mopix/111/m/111-m-111-r1.mp4",
                ),
                { "objectType": "Portable Document File (PDF)", "objectFilename": "sheet.pdf",
                  "objectUrl": "https://catalog.archives.gov/medialz/sheet.pdf" },
            ]),
        ),
        nara_record(
            14117,
            "TEHERAN CONFERENCE, IRAN",
            "Restricted - Possibly",
            json!([nara_file(None, &format!("{lz}/111-adc-310.mp4"))]),
        ),
        nara_record(
            75411,
            "SECRETARY KNOX INSPECTS NOUMEA",
            "Undetermined",
            json!([nara_file(None, &format!("{lz}/428-npc-193.mp4"))]),
        ),
        nara_record(
            102045538,
            "NASA SYNCOM PROJECT",
            "Unrestricted",
            json!([{ "objectType": "Image (JPG)", "objectDesignator": "111-DD Release Sheet",
                     "objectFilename": "sheet.jpg", "objectUrl": "https://catalog.archives.gov/medialz/sheet.jpg" }]),
        ),
        nara_record(
            75465,
            "CARRIER ACTIVITY ABOARD USS PHILIPPINE SEA (CVA-47)",
            "Unrestricted",
            json!([nara_file(None, &format!("{lz}/428-npc-328.mp4"))]),
        ),
    ];
    let seen: Seen = Arc::default();
    let searches = seen.clone();
    let app = Router::new().route(
        "/records/search",
        get(move |uri: Uri, headers: axum::http::HeaderMap| {
            let (searches, records) = (searches.clone(), records.clone());
            async move {
                let mut params = query_params(&uri);
                let key = headers
                    .get("x-api-key")
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or_default()
                    .to_owned();
                params.insert("key".to_owned(), key.clone());
                searches.lock().unwrap().push(params.clone());
                match key.as_str() {
                    NARA_KEY => {}
                    // The catalogue's web front answers a refused key with its own page.
                    "web-page" => {
                        return (
                            [(header::CONTENT_TYPE, "text/html")],
                            "<!doctype html><html></html>",
                        )
                            .into_response();
                    }
                    _ => {
                        return json_response(
                            StatusCode::FORBIDDEN,
                            json!({ "message": "Forbidden" }),
                        );
                    }
                }
                let found: Vec<Value> = match params.get("naId") {
                    Some(id) => records
                        .iter()
                        .filter(|record| record["naId"].as_u64() == id.parse().ok())
                        .cloned()
                        .collect(),
                    None => records.clone(),
                };
                json_response(StatusCode::OK, nara_answer(&found, 130))
            }
        }),
    );
    let address = serve(app);
    let base = "https://catalog.archives.gov/api/v2";
    let host = keyed_local_host("nara", "dev.raygun.nara", &[base], address, NARA_KEY).await;
    let provider = "dev.raygun.nara/films";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let listed: Vec<_> = popular
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.title.as_str()))
        .collect();
    assert_eq!(
        listed,
        [
            ("35486", "Die Gangarten Des Pferdes (Spanische Reitschule)"),
            (
                "75465",
                "Carrier Activity Aboard USS Philippine Sea (CVA-47)"
            ),
        ],
        "unrestricted in use and access, with an MP4"
    );
    assert!(popular.has_next_page, "60 of 130");
    let asked = last(&seen);
    assert_eq!(asked["key"], NARA_KEY);
    assert_eq!(asked["q"], "*");
    assert_eq!(asked["ancestorNaId"], "25061");
    assert_eq!(asked["typeOfMaterials"], "Moving Images");
    assert_eq!(asked["availableOnline"], "true");
    assert_eq!(asked["limit"], "60");
    assert_eq!(asked["page"], "1");

    host.manga_search_filtered(
        provider,
        "horses",
        2,
        &filters(&[("recordGroup", "111"), ("decade", "1930")]),
    )
    .await
    .unwrap();
    let asked = last(&seen);
    assert_eq!(asked["q"], "horses");
    assert_eq!(asked["recordGroupNumber"], "111");
    assert_eq!(
        (asked["startDate"].as_str(), asked["endDate"].as_str()),
        ("1930", "1939")
    );
    assert_eq!(asked["page"], "2");

    // A listed film opens without another request; its reels are its episodes, in order.
    let before = seen.lock().unwrap().len();
    let details = host.manga_details(provider, "35486").await.unwrap();
    assert_eq!(details.year, Some(1936));
    let description = details.description.unwrap();
    for part in [
        "Horses of the Spanish Riding School perform.",
        "Shot list: LS, riders enter the hall.",
        "Made by: Department of Defense. Department of the Army. Office of the Chief Signal Officer",
        "Series: Motion Picture Films from the Army Library Copy Collection",
        "Record Group 111: Records of the Office of the Chief Signal Officer",
        "National Archives Identifier 35486 (111-M-35486). Unrestricted use and access.",
    ] {
        assert!(description.contains(part), "{part}: {description}");
    }
    let episodes = host.video_episodes(provider, "35486").await.unwrap();
    let reels: Vec<_> = episodes
        .iter()
        .map(|episode| (episode.id.as_str(), episode.name.as_str()))
        .collect();
    assert_eq!(
        reels,
        [("35486/0", "Reel 1 of 2"), ("35486/1", "Reel 2 of 2")],
        "each file once, by reel"
    );
    let servers = host.video_servers(provider, "35486/0").await.unwrap();
    assert_eq!(
        servers[0].videos.as_ref().unwrap()[0].url,
        "https://catalog.archives.gov/medialz/mopix/111/m/111-m-111-r1.mp4",
        "the storage bucket's address becomes the catalogue's own"
    );
    let single = host.video_episodes(provider, "75465").await.unwrap();
    assert_eq!(single[0].name, "Film");
    assert_eq!(seen.lock().unwrap().len(), before, "all from the lists");

    let restricted = host.manga_details(provider, "14117").await.unwrap_err();
    assert!(restricted.contains("restricted"), "{restricted}");
    assert_eq!(last(&seen)["naId"], "14117");

    // A key the catalogue refuses (a 403, or its web page where JSON was asked for).
    for key in ["refused", "web-page"] {
        let host = keyed_local_host("nara", "dev.raygun.nara", &[base], address, key).await;
        let error = host
            .manga_list(provider, MangaList::Popular, 1)
            .await
            .unwrap_err();
        assert_eq!(
            crate::needs_setup_in(&error),
            Some("dev.raygun.nara"),
            "{key}: {error}"
        );
        assert!(error.contains("didn't accept the API key"), "{error}");
        assert!(host.needs_setup("dev.raygun.nara").is_some());
    }
}

// ---- Europeana ----

const EUROPEANA_KEY: &str = "a-key";
const SOUND_AND_VISION: &str = "2051906/data_euscreenXL_media_10692";

fn europeana_record() -> Value {
    let file = "https://www.openbeelden.nl/files/10/661368.mp4";
    json!({ "success": true, "object": {
        "proxies": [
            { "europeanaProxy": true, "dctermsCreated": { "def": ["#1950-11-28"] } },
            { "europeanaProxy": false,
              "dcTitle": { "en": ["Displaced persons find asylum"], "nl": ["Ontheemden vinden asiel"] },
              "dcDescription": { "en": ["Elderly people from camps arrive in Eindhoven."] },
              "dcCreator": { "en": ["Polygoon-Profilti (producer)", "Netherlands Institute for Sound and Vision (curator)"] },
              "dctermsCreated": { "def": ["1950-11-28"] },
              "dcLanguage": { "def": ["nl"] } },
        ],
        "aggregations": [{
            "edmIsShownBy": file,
            "hasView": ["https://www.openbeelden.nl/files/10/661368.ogv"],
            "edmRights": { "def": ["http://creativecommons.org/licenses/by-sa/4.0/"] },
            "edmDataProvider": { "def": ["http://data.europeana.eu/organization/4538"] },
            "edmProvider": { "def": ["http://data.europeana.eu/organization/1607"] },
            "webResources": [
                { "about": file, "ebucoreHasMimeType": "video/mp4", "ebucoreHeight": 576 },
                { "about": "https://www.openbeelden.nl/files/10/661368.ogv", "ebucoreHasMimeType": "video/ogg" },
            ],
        }],
        "organizations": [
            { "about": "http://data.europeana.eu/organization/4538", "prefLabel": { "en": ["Netherlands Institute for Sound & Vision"] } },
            { "about": "http://data.europeana.eu/organization/1607", "prefLabel": { "en": ["EUscreen"] } },
        ],
        "concepts": [{ "about": "http://data.europeana.eu/concept/3286", "prefLabel": { "en": ["World War II"] } }],
        "europeanaAggregation": {
            "edmPreview": "https://api.europeana.eu/thumbnail/v2/url.json?uri=x&type=VIDEO",
            "edmLandingPage": format!("https://www.europeana.eu/item/{SOUND_AND_VISION}"),
        },
    } })
}

#[tokio::test]
async fn europeana_offers_openly_licensed_files_and_credits_their_holders() {
    let seen: Seen = Arc::default();
    let searches = seen.clone();
    let app = Router::new().fallback(move |uri: Uri, headers: axum::http::HeaderMap| {
        let searches = searches.clone();
        async move {
            let mut params = query_params(&uri);
            let key = headers
                .get("x-api-key")
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_owned();
            params.insert("key".to_owned(), key.clone());
            params.insert("path".to_owned(), uri.path().to_owned());
            params.insert(
                "refinements".to_owned(),
                uri.query().unwrap_or_default().matches("qf=").count().to_string(),
            );
            searches.lock().unwrap().push(params.clone());
            if key != EUROPEANA_KEY {
                return json_response(
                    StatusCode::UNAUTHORIZED,
                    json!({ "success": false, "error": "API key is invalid" }),
                );
            }
            let item = |id: &str, rights: &str, file: &str| {
                json!({ "id": format!("/{id}"), "title": [id], "rights": [rights],
                        "edmIsShownBy": [file], "edmPreview": [format!("https://api.europeana.eu/thumbnail/v2/url.json?uri={id}")],
                        "dataProvider": ["Romanian Radio Broadcasting Company"], "language": ["ro"] })
            };
            match uri.path() {
                "/search.json" if params.get("theme").map(String::as_str) == Some("music") => {
                    let mut hora = item(
                        "2059208/hora",
                        "http://creativecommons.org/licenses/by-sa/4.0/",
                        "https://resource.culturalia.ro/x/P_15042_01.mp3",
                    );
                    hora["dcTitleLangAware"] = json!({ "en": ["Hora beaten"], "ro": ["Hora bătută"] });
                    json_response(StatusCode::OK, json!({ "success": true, "items": [
                        hora,
                        item("2059219/page", "http://creativecommons.org/publicdomain/mark/1.0/",
                            "http://gallica.bnf.fr/ark:/12148/bpt6k13111729/f1.audio"),
                        item("2059201/closed", "http://creativecommons.org/licenses/by-nc/4.0/",
                            "https://example.org/a.mp3"),
                    ] }))
                }
                "/search.json" => {
                    let mut film = item(SOUND_AND_VISION, "http://creativecommons.org/licenses/by-sa/4.0/",
                        "https://www.openbeelden.nl/files/10/661368.mp4");
                    film["dcTitleLangAware"] = json!({ "nl": ["Ontheemden vinden asiel"], "en": ["Displaced persons find asylum"] });
                    json_response(StatusCode::OK, json!({ "success": true, "totalResults": 7584, "items": [
                        film,
                        item("08604/closed", "http://rightsstatements.org/vocab/InC/1.0/", "https://example.org/b.mp4"),
                        item("08604/vimeo", "http://creativecommons.org/publicdomain/mark/1.0/", "https://vimeo.com/438840139"),
                    ] }))
                }
                path if path == format!("/{SOUND_AND_VISION}.json") => {
                    json_response(StatusCode::OK, europeana_record())
                }
                _ => StatusCode::NOT_FOUND.into_response(),
            }
        }
    });
    let address = serve(app);
    let base = "https://api.europeana.eu/record/v2";
    let host = keyed_local_host(
        "europeana",
        "dev.raygun.europeana",
        &[base],
        address,
        EUROPEANA_KEY,
    )
    .await;
    let provider = "dev.raygun.europeana/videos";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let listed: Vec<_> = popular
        .items
        .iter()
        .map(|item| (item.id.as_str(), item.title.as_str()))
        .collect();
    assert_eq!(
        listed,
        [(SOUND_AND_VISION, "Displaced persons find asylum")],
        "openly licensed files only"
    );
    assert!(popular.has_next_page);
    let asked = last(&seen);
    assert_eq!(asked["key"], EUROPEANA_KEY, "the key goes in a header");
    assert!(!asked.contains_key("wskey"), "never in the address");
    assert_eq!(asked["reusability"], "open");
    assert_eq!(asked["refinements"], "2", "videos with an MP4");

    host.manga_search_filtered(
        provider,
        "voetbal: ajax",
        1,
        &filters(&[
            ("country", "netherlands"),
            ("provider", "Netherlands Institute for Sound & Vision"),
        ]),
    )
    .await
    .unwrap();
    let asked = last(&seen);
    assert_eq!(asked["query"], "voetbal ajax", "search operators left out");
    assert_eq!(asked["refinements"], "4");

    let details = host
        .manga_details(provider, SOUND_AND_VISION)
        .await
        .unwrap();
    assert_eq!(details.title, "Displaced persons find asylum");
    assert_eq!(details.year, Some(1950));
    assert_eq!(details.genres, ["World War II"]);
    let description = details.description.unwrap();
    assert!(
        description
            .contains("Licence: CC BY-SA 4.0 (http://creativecommons.org/licenses/by-sa/4.0/).")
    );
    assert!(description.ends_with(
        "Provided by Netherlands Institute for Sound & Vision through EUscreen and Europeana."
    ));
    let episodes = host
        .video_episodes(provider, SOUND_AND_VISION)
        .await
        .unwrap();
    assert_eq!(episodes.len(), 1, "the Ogg view doesn't play everywhere");
    let servers = host.video_servers(provider, &episodes[0].id).await.unwrap();
    let videos = servers[0].videos.as_ref().unwrap();
    assert_eq!(
        (videos[0].url.as_str(), videos[0].quality),
        ("https://www.openbeelden.nl/files/10/661368.mp4", Some(576))
    );

    let songs = host.search_music("hora", 10).await;
    let tracks = &songs[0].tracks;
    assert_eq!(
        tracks.len(),
        1,
        "audio files under an open licence: {tracks:?}"
    );
    assert_eq!(
        tracks[0].title, "Hora bătută",
        "in the recording's own language"
    );
    assert_eq!(
        tracks[0].artist.as_deref(),
        Some("Romanian Radio Broadcasting Company")
    );

    let refused = keyed_local_host("europeana", "dev.raygun.europeana", &[base], address, "old")
        .await
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap_err();
    assert_eq!(
        crate::needs_setup_in(&refused),
        Some("dev.raygun.europeana"),
        "{refused}"
    );
}

// ---- OpenStax ----

const PSYCHOLOGY: &str = "06aba565-9432-40f6-97ee-b8a361f118a8";
const CALCULUS: &str = "8b89d172-2927-466f-8661-01abc7ccdba4";
const CALCULO: &str = "8635f0be-e734-4cd1-a10e-8063e5b863b6";
const MICROBIOLOGY: &str = "e42bd376-624b-4c0f-972f-e0c57998e765";
const PHYSICS: &str = "cce64fde-f448-43b8-ae88-27705cceb0da";
const OPENSTAX_ARCHIVE: &str = "/apps/archive/20260604.144757";

/// A live book as OpenStax's CMS lists it (shapes from its pages API).
fn openstax_book(
    id: &str,
    title: &str,
    (slug, locale): (&str, &str),
    subject: &str,
    published: &str,
) -> Value {
    json!({
        "id": 417,
        "meta": { "slug": slug, "type": "books.Book", "locale": locale,
                  "html_url": format!("https://openstax.org/details/books/{slug}"),
                  "detail_url": "https://openstax.org/apps/cms/api/v2/pages/417/" },
        "title": title, "book_state": "live", "cnx_id": id,
        "book_subjects": [{ "id": 1, "meta": { "type": "books.BookSubjects" }, "subject_name": subject,
                            "subject_page_content": "Simple to use, simple to adopt." }],
        "book_categories": [{ "id": 2, "subject_name": subject, "subject_category": title }],
        "description": format!("<p data-block-key=\"0lgy4\"><i>{title}</i> is a textbook &amp; more.</p><p data-block-key=\"x\">Second paragraph.</p>"),
        "content_warning_text": null,
        "cover_url": format!("https://assets.openstax.org/oscms-prodcms/media/documents/{slug}.svg"),
        "publish_date": published,
        "license_name": "Creative Commons Attribution-NonCommercial-ShareAlike License",
        "license_url": "https://creativecommons.org/licenses/by-nc-sa/4.0/",
        "license_version": "4.0",
        "authors": [],
    })
}

fn openstax_author(name: &str, senior: bool, top: bool) -> Value {
    json!({ "type": "author", "id": name,
            "value": { "name": name, "university": "Somewhere", "country": "",
                       "senior_author": senior, "display_at_top": top } })
}

fn openstax_catalogue() -> Value {
    let mut psychology = openstax_book(
        PSYCHOLOGY,
        "Psychology 2e",
        ("psychology-2e", "en"),
        "Social Sciences",
        "2020-04-22",
    );
    psychology["book_categories"] =
        json!([{ "subject_name": "Social Sciences", "subject_category": "Psychology" }]);
    psychology["authors"] = json!([
        openstax_author("Rose M. Spielman", true, true),
        openstax_author("William J. Jenkins", true, false),
        openstax_author("Mara Aruguete", false, false),
    ]);
    let mut calculus = openstax_book(
        CALCULUS,
        "Calculus Volume 1",
        ("calculus-volume-1", "en"),
        "Math",
        "2016-03-30",
    );
    calculus["authors"] = json!([openstax_author("Gilbert Strang", true, true)]);
    let mut calculo = openstax_book(
        CALCULO,
        "Cálculo volumen 1",
        ("cálculo-volumen-1", "es"),
        "Matemáticas",
        "2022-03-01",
    );
    // Some Spanish books' licence is only in their own data.
    for field in ["license_name", "license_url", "license_version"] {
        calculo[field] = Value::Null;
    }
    calculo["authors"] = json!([openstax_author("Gilbert Strang", true, true)]);
    let mut microbiology = openstax_book(
        MICROBIOLOGY,
        "Microbiology",
        ("microbiology", "en"),
        "Science",
        "2016-11-01",
    );
    microbiology["content_warning_text"] =
        json!("This material includes descriptions of severe injury.");
    microbiology["license_url"] = json!("https://creativecommons.org/licenses/by/4.0/");
    let physics = openstax_book(
        PHYSICS,
        "Physics",
        ("physics", "en"),
        "Science",
        "2020-03-26",
    );
    let mut retired = openstax_book(
        "11111111-2222-4333-8444-555555555555",
        "Chemistry",
        ("chemistry", "en"),
        "Science",
        "2015-03-11",
    );
    retired["book_state"] = json!("retired");
    let mut without_id = openstax_book("", "Coming Soon", ("coming-soon", "en"), "Math", "");
    without_id["cnx_id"] = Value::Null;
    json!({ "meta": { "total_count": 7 }, "items": [
        psychology, calculus, calculo, microbiology, physics, retired, without_id,
    ] })
}

fn openstax_release() -> Value {
    json!({ "id": "v4/cd2a554", "code": "9ce9a1a8", "archiveUrl": OPENSTAX_ARCHIVE, "books": {
        PSYCHOLOGY: { "defaultVersion": "de7e40c" },
        CALCULUS: { "defaultVersion": "8dbc2ce" },
        CALCULO: { "defaultVersion": "da492e7" },
        MICROBIOLOGY: { "defaultVersion": "aa11bb2" },
        // Live in the CMS, retired from the reader: not listed.
        PHYSICS: { "defaultVersion": "cc33dd4", "retired": true },
    } })
}

/// The id of a page of the fixture books' contents.
fn openstax_page(number: u32) -> String {
    format!("00000000-0000-4000-8000-{number:012}")
}

fn openstax_leaf(number: u32, version: &str, title: &str, target: &str) -> Value {
    json!({ "id": format!("{}@{version}", openstax_page(number)), "title": title,
            "toc_type": "book-content", "toc_target_type": target, "slug": format!("page-{number}") })
}

/// A contents title as the archive writes it: "<number> <words>" in spans.
fn openstax_title(number: Option<&str>, text: &str) -> String {
    let number = number.map_or_else(String::new, |number| {
        format!(r#"<span class="os-number">{number}</span><span class="os-divider"> </span>"#)
    });
    format!(r#"{number}<span data-type="" itemprop="" class="os-text">{text}</span>"#)
}

fn openstax_part(label: &str, number: &str, text: &str) -> String {
    let number = format!(r#"<span class="os-part-text">{label} </span>{number}"#);
    openstax_title(Some(number.as_str()), text)
}

/// Psychology's contents: chapters in a unit, unnumbered parts, the answer key and the index.
fn openstax_contents(licence: &str) -> Value {
    let words = |text: &str| openstax_title(None, text);
    json!({
        "title": "Psychology 2e", "id": PSYCHOLOGY, "version": "de7e40c", "language": "en",
        "license": { "url": licence, "name": "Creative Commons Attribution-NonCommercial-ShareAlike License" },
        "tree": { "id": format!("{PSYCHOLOGY}@de7e40c"), "title": "Psychology 2e", "contents": [
            openstax_leaf(1, "", &words("Preface"), "preface"),
            { "id": "c1@de7e40c", "title": openstax_part("Chapter", "1", "Introduction to Psychology"),
              "toc_type": "chapter", "contents": [
                openstax_leaf(2, "", &words("Introduction"), "intro"),
                openstax_leaf(3, "", &openstax_title(Some("1.1"), "What Is Psychology?"), "numbered-section"),
                openstax_leaf(4, "de7e40c", &words("Key Terms"), "composite-page"),
                { "id": "r1@de7e40c", "title": words("Chapter Review"), "toc_type": "sub-book-tree", "contents": [
                    openstax_leaf(5, "de7e40c", &words("Key Equations"), "composite-page"),
                ] },
            ] },
            { "id": "u2@de7e40c", "title": openstax_part("Unit", "2", "Research"), "toc_type": "unit", "contents": [
                openstax_leaf(6, "", &openstax_part("Unit", "2", "Unit Introduction"), "intro"),
                { "id": "c2@de7e40c", "title": openstax_part("Chapter", "2", "Psychological Research"),
                  "toc_type": "chapter", "contents": [
                    openstax_leaf(7, "", &openstax_title(Some("2.1"), "Why Is Research Important?"), "numbered-section"),
                    // A page listed twice is one chapter.
                    openstax_leaf(3, "", &openstax_title(Some("1.1"), "What Is Psychology?"), "numbered-section"),
                ] },
            ] },
            openstax_leaf(8, "", &openstax_part("Appendix", "A", "Statistics"), "appendix"),
            { "id": "a@de7e40c", "title": words("Answer Key"), "toc_type": "sub-book-tree", "contents": [
                openstax_leaf(9, "de7e40c", &words("Chapter 1"), "answer-key"),
            ] },
            openstax_leaf(10, "de7e40c", &words("References"), "composite-page"),
            openstax_leaf(11, "de7e40c", &words("Index"), "index"),
        ] },
    })
}

/// A page as the archive serves it: a whole XHTML document, with figures and MathML.
fn openstax_page_answer() -> Value {
    let content = r#"<html xmlns="http://www.w3.org/1999/xhtml"><head><style>:target { background-color: #ffffcc; }</style><script>window.addEventListener('load', () => {})</script></head><body>
<div data-type="page" id="page_x" class="chapter-content-module"><h2 data-type="document-title"><span class="os-number">1.1</span>
<span class="os-divider"> </span>
<span data-type="" itemprop="" class="os-text">What Is Psychology?</span>
</h2><p id="fs-1"><span data-type="term">Psychology</span> refers to the scientific study of the mind and behavior.</p>
<div class="os-figure"><figure id="f1"><span data-type="media" data-alt="Wilhelm Wundt"><img src="../resources/206235878db6612ad174b6951718283e737b774d" data-media-type="image/jpeg" alt="Wilhelm Wundt" width="650" height="329"/></span></figure></div>
<div data-type="equation" class="unnumbered"><math display="block"><semantics><mrow><mi>f</mi><mo>=</mo><mfrac><mn>1</mn><mi>x</mi></mfrac></mrow><annotation-xml encoding="MathML-Content"><mi>f</mi></annotation-xml></semantics></math></div>
</div></body></html>"#;
    json!({ "slug": "1-1-what-is-psychology", "title": "What Is Psychology?", "id": openstax_page(3),
            "abstract": "<p>Learn things.</p>", "revised": "2026-07-09T15:08:17+00:00", "noindex": false,
            "content": content })
}

#[tokio::test]
async fn openstax_lists_live_books_and_reads_their_pages_with_figures_and_formulas() {
    let catalogue_asks = Arc::new(AtomicUsize::new(0));
    let release_asks = Arc::new(AtomicUsize::new(0));
    let contents_asks = Arc::new(AtomicUsize::new(0));
    let (catalogue, release, contents) = (
        catalogue_asks.clone(),
        release_asks.clone(),
        contents_asks.clone(),
    );
    let app = Router::new().fallback(move |uri: Uri| {
        let (catalogue, release, contents) = (catalogue.clone(), release.clone(), contents.clone());
        async move {
            let path = uri.path().to_owned();
            if path == "/apps/cms/api/v2/pages/" {
                let params = query_params(&uri);
                assert_eq!(params["type"], "books.Book");
                assert_eq!(params["limit"], "250");
                for field in [
                    "cnx_id",
                    "book_state",
                    "license_url",
                    "authors",
                    "description",
                ] {
                    assert!(
                        params["fields"].split(',').any(|asked| asked == field),
                        "{field}"
                    );
                }
                catalogue.fetch_add(1, Ordering::SeqCst);
                return json_response(StatusCode::OK, openstax_catalogue());
            }
            if path == "/rex/release.json" {
                release.fetch_add(1, Ordering::SeqCst);
                return json_response(StatusCode::OK, openstax_release());
            }
            let Some(file) = path.strip_prefix(&format!("{OPENSTAX_ARCHIVE}/contents/")) else {
                return StatusCode::NOT_FOUND.into_response();
            };
            let page_three = format!("{PSYCHOLOGY}@de7e40c:{}.json", openstax_page(3));
            match file {
                file if file == format!("{PSYCHOLOGY}@de7e40c.json") => {
                    contents.fetch_add(1, Ordering::SeqCst);
                    json_response(
                        StatusCode::OK,
                        openstax_contents("http://creativecommons.org/licenses/by-nc-sa/4.0/"),
                    )
                }
                file if file == format!("{CALCULO}@da492e7.json") => json_response(
                    StatusCode::OK,
                    openstax_contents("http://creativecommons.org/licenses/by/4.0/"),
                ),
                file if file == page_three => json_response(StatusCode::OK, openstax_page_answer()),
                _ => json_response(StatusCode::NOT_FOUND, json!({ "message": "Not Found" })),
            }
        }
    });
    let address = serve(app);
    let host = local_host("openstax", &["https://openstax.org"], address).await;
    let provider = "dev.raygun.openstax/books";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let titles: Vec<_> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "Calculus Volume 1",
            "Microbiology",
            "Psychology 2e",
            "Cálculo volumen 1"
        ],
        "live books the reader serves, by subject in OpenStax's order, English first; retired \
         ones and one without an id are left out"
    );
    assert!(!popular.has_next_page);
    let psychology = &popular.items[2];
    assert_eq!(psychology.id, PSYCHOLOGY);
    assert_eq!(
        psychology.authors,
        ["Rose M. Spielman"],
        "the authors shown first"
    );
    assert_eq!(
        psychology.cover_url.as_deref(),
        Some("https://assets.openstax.org/oscms-prodcms/media/documents/psychology-2e.svg")
    );
    let latest = host
        .manga_list(provider, MangaList::Latest, 1)
        .await
        .unwrap();
    let titles: Vec<_> = latest
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(
        titles,
        [
            "Cálculo volumen 1",
            "Psychology 2e",
            "Microbiology",
            "Calculus Volume 1"
        ],
        "the newest first"
    );

    let search = |query: &'static str, pairs: &'static [(&'static str, &'static str)]| {
        let host = &host;
        async move {
            host.manga_search_filtered(provider, query, 1, &filters(pairs))
                .await
                .unwrap()
                .items
                .into_iter()
                .map(|item| item.title)
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(
        search("psych", &[]).await,
        ["Psychology 2e"],
        "a word's start"
    );
    assert_eq!(
        search("strang", &[]).await,
        ["Calculus Volume 1", "Cálculo volumen 1"],
        "authors count"
    );
    assert_eq!(
        search("calc", &[]).await,
        ["Calculus Volume 1", "Cálculo volumen 1"],
        "without accents"
    );
    assert_eq!(
        search("", &[("subject", "Math"), ("language", "es")]).await,
        ["Cálculo volumen 1"],
        "Spanish subjects count as their English ones"
    );
    assert_eq!(
        search("", &[("sort", "newest")]).await[0],
        "Cálculo volumen 1"
    );
    assert!(search("nothing like it", &[]).await.is_empty());
    let offered = host.manga_filters(provider).await.unwrap();
    assert_eq!(offered.len(), 3, "subject, language and sort");

    let details = host.manga_details(provider, PSYCHOLOGY).await.unwrap();
    assert_eq!(details.title, "Psychology 2e");
    assert_eq!(details.authors, ["Rose M. Spielman", "William J. Jenkins"]);
    assert_eq!(details.genres, ["Social Sciences", "Psychology"]);
    assert_eq!(details.status, "completed");
    assert_eq!(
        details.url.as_deref(),
        Some(format!("http://{address}/details/books/psychology-2e").as_str())
    );
    assert_eq!(
        details.description.as_deref(),
        Some(
            "Psychology 2e is a textbook & more.\n\nSecond paragraph.\n\n\
             Senior contributing authors: Rose M. Spielman, William J. Jenkins.\n\
             Contributing authors: Mara Aruguete.\n\n\
             Subjects: Social Sciences.\n\n\
             Licence: CC BY-NC-SA 4.0 (from OpenStax's book data)."
        )
    );
    let calculo = host.manga_details(provider, CALCULO).await.unwrap();
    let description = calculo.description.unwrap();
    assert!(
        description.ends_with("Licence: CC BY 4.0 (from OpenStax's book data)."),
        "the book's own data, when the catalogue has no licence: {description}"
    );
    assert_eq!(
        calculo.url.as_deref(),
        Some(format!("http://{address}/details/books/c%C3%A1lculo-volumen-1").as_str())
    );
    let microbiology = host.manga_details(provider, MICROBIOLOGY).await.unwrap();
    let description = microbiology.description.unwrap();
    assert!(
        description.contains("Content note: This material includes descriptions of severe injury."),
        "{description}"
    );
    assert!(
        description.ends_with("Licence: CC BY 4.0 (from OpenStax's book data)."),
        "the catalogue's licence when the contents can't be read: {description}"
    );

    let chapters = host.manga_chapters(provider, PSYCHOLOGY).await.unwrap();
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(
        names,
        [
            "Preface",
            "Chapter 1 · Introduction",
            "1.1 What Is Psychology?",
            "Chapter 1 · Key Terms",
            "Chapter 1 · Chapter Review · Key Equations",
            "Unit 2: Unit Introduction",
            "2.1 Why Is Research Important?",
            "Appendix A: Statistics",
            "Answer Key · Chapter 1",
            "References",
        ],
        "pages in reading order, named with their chapter; each page once, no index"
    );
    assert_eq!(
        chapters[2].id,
        format!("{PSYCHOLOGY}:{}", openstax_page(3)),
        "the book's and the page's ids, without versions"
    );
    assert_eq!(chapters[9].number, Some(10.0));
    assert_eq!(
        contents_asks.load(Ordering::SeqCst),
        1,
        "details and chapters share the contents"
    );

    let text = host.novel_passage(provider, &chapters[2].id).await.unwrap();
    assert!(
        matches!(&text[0], PassageBlock::Heading(heading) if heading == "1.1 What Is Psychology?"),
        "{text:?}"
    );
    assert_eq!(
        text[1],
        PassageBlock::Paragraph(
            "Psychology refers to the scientific study of the mind and behavior.".into()
        )
    );
    assert!(
        text.contains(&PassageBlock::Image(format!(
            "http://{address}{OPENSTAX_ARCHIVE}/resources/206235878db6612ad174b6951718283e737b774d"
        ))),
        "figures from the archive's resources: {text:?}"
    );
    assert!(
        text.iter().any(
            |block| matches!(block, PassageBlock::Math(formula) if formula.display && formula.mathml.contains("<mfrac>"))
        ),
        "{text:?}"
    );
    let raw = host
        .call(provider, "passage", json!({ "chapterId": chapters[2].id }))
        .await
        .unwrap();
    let html = raw["html"].as_str().unwrap();
    assert!(
        html.contains("<math display=\"block\">"),
        "formulas as they are"
    );
    assert!(
        !html.contains("<head") && !html.contains("STYLING") && !html.contains("addEventListener"),
        "no head"
    );
    assert_eq!(
        raw["baseUrl"],
        format!("http://{address}{OPENSTAX_ARCHIVE}/contents/")
    );

    let missing = host
        .novel_passage(provider, &chapters[1].id)
        .await
        .unwrap_err();
    assert!(missing.contains("current version"), "{missing}");
    for wrong in ["../../rex/release", &format!("{PSYCHOLOGY}:../x")] {
        let error = host.novel_passage(provider, wrong).await.unwrap_err();
        assert!(error.contains("no page with this id"), "{error}");
    }
    let retired = host.manga_chapters(provider, PHYSICS).await.unwrap_err();
    assert!(retired.contains("no longer offers"), "{retired}");
    assert_eq!(
        (
            catalogue_asks.load(Ordering::SeqCst),
            release_asks.load(Ordering::SeqCst)
        ),
        (1, 1),
        "the catalogue and the versions are kept"
    );
}

// ---- Last.fm ----

const LASTFM_KEY: &str = "a-key";

/// A song in a Last.fm chart (shapes from its API's JSON answers): numbers as text, a duration in
/// seconds or "0", images Raygun doesn't use.
fn lastfm_track(name: &str, artist: &str, duration: Value) -> Value {
    let page = |text: &str| text.replace(' ', "+");
    json!({
        "name": name, "duration": duration, "playcount": "25146553", "listeners": "1371049", "mbid": "",
        "url": format!("https://www.last.fm/music/{}/_/{}", page(artist), page(name)),
        "streamable": { "#text": "0", "fulltrack": "0" },
        "artist": { "name": artist, "mbid": "", "url": format!("https://www.last.fm/music/{}", page(artist)) },
        "image": [{ "#text": "https://lastfm.freetls.fastly.net/i/u/34s/2a96cbd8b46e442fc41c2b86b821562f.png", "size": "small" }],
        "@attr": { "rank": "0" },
    })
}

fn lastfm_tracks(root: &str, tracks: Value) -> Value {
    json!({ root: { "track": tracks, "@attr": { "page": "1", "perPage": "50", "totalPages": "1", "total": "2" } } })
}

fn lastfm_tags(tags: &[(&str, u32)]) -> Value {
    let tags: Vec<Value> = tags
        .iter()
        .map(|(name, count)| json!({ "count": count, "name": name, "url": format!("https://www.last.fm/tag/{name}") }))
        .collect();
    json!({ "toptags": { "tag": tags, "@attr": { "artist": "Cher" } } })
}

fn lastfm_error(status: StatusCode, code: u32, message: &str) -> Response {
    json_response(
        status,
        json!({ "error": code, "message": message, "links": [] }),
    )
}

/// What Last.fm answers a request with a valid key.
fn lastfm_answer(params: &HashMap<String, String>) -> Response {
    let param = |name: &str| params.get(name).map_or("", String::as_str);
    let not_found = |what: &str| lastfm_error(StatusCode::NOT_FOUND, 6, what);
    let answer = match param("method") {
        "chart.getTopTracks" => lastfm_tracks(
            "tracks",
            json!([
                lastfm_track("Die With A Smile", "Lady Gaga", json!("0")),
                lastfm_track("APT.", "ROSÉ", json!("170")),
                lastfm_track("APT.", "ROSÉ", json!("170")),
                { "name": "", "artist": { "name": "Nobody" } },
            ]),
        ),
        "geo.getTopTracks" => match param("country") {
            "United States" | "Russia" => lastfm_tracks(
                "tracks",
                json!([lastfm_track(
                    "Birds of a Feather",
                    "Billie Eilish",
                    json!("210")
                )]),
            ),
            _ => return lastfm_error(StatusCode::BAD_REQUEST, 6, "country param invalid"),
        },
        "tag.getTopTracks" => match param("tag") {
            // One song comes as an object, not a list.
            "rock" => lastfm_tracks(
                "tracks",
                lastfm_track("Bohemian Rhapsody", "Queen", json!("355")),
            ),
            "hip hop" => lastfm_tracks(
                "tracks",
                json!([
                    lastfm_track("HUMBLE.", "Kendrick Lamar", json!("177")),
                    lastfm_track("Juicy", "The Notorious B.I.G.", json!("0")),
                ]),
            ),
            tag => {
                json!({ "tracks": { "track": [], "@attr": { "tag": tag, "page": "1", "perPage": "50", "totalPages": "0", "total": "0" } } })
            }
        },
        "track.getTopTags" => match (param("artist"), param("track")) {
            ("Cher", "Believe") => lastfm_tags(&[
                ("pop", 100),
                ("dance", 88),
                ("seen live", 50),
                ("1998", 40),
                ("cher", 30),
                ("Pop", 20),
                ("electronic", 9),
            ]),
            ("Cher", _) => {
                json!({ "toptags": { "tag": [], "@attr": { "artist": "Cher", "track": "B-side" } } })
            }
            _ => return not_found("Track not found"),
        },
        "artist.getTopTags" => match param("artist") {
            "Cher" => lastfm_tags(&[("pop", 100), ("female vocalists", 60), ("seen live", 12)]),
            artist => json!({ "toptags": { "@attr": { "artist": artist } } }),
        },
        "artist.getSimilar" => match param("artist") {
            "Cher" => json!({ "similarartists": { "artist": [
                { "name": "Sonny & Cher", "mbid": "3d6a2b54-4a0a-4d8b-a2c8-3a2e6c4e1f10", "match": "1",
                  "url": "https://www.last.fm/music/Sonny+&+Cher", "image": [], "streamable": "0" },
                { "name": "Madonna", "mbid": "", "match": 0.82, "url": "https://www.last.fm/music/Madonna", "image": [] },
                { "name": "Cher", "mbid": "", "match": "0.5", "url": "https://www.last.fm/music/Cher" },
            ], "@attr": { "artist": "Cher" } } }),
            artist => json!({ "similarartists": { "artist": [], "@attr": { "artist": artist } } }),
        },
        "artist.getTopTracks" => match param("artist") {
            "Cher" => lastfm_tracks(
                "toptracks",
                json!([
                    lastfm_track("Believe", "Cher", Value::Null),
                    lastfm_track("Strong Enough", "Cher", Value::Null),
                ]),
            ),
            "Sonny & Cher" => lastfm_tracks(
                "toptracks",
                json!([
                    lastfm_track("I Got You Babe", "Sonny & Cher", Value::Null),
                    lastfm_track("The Beat Goes On", "Sonny & Cher", Value::Null),
                ]),
            ),
            "Madonna" => lastfm_tracks(
                "toptracks",
                lastfm_track("Like a Prayer", "Madonna", Value::Null),
            ),
            _ => return not_found("The artist you supplied could not be found"),
        },
        "track.getSimilar" => match (param("artist"), param("track")) {
            // Numbers as numbers here.
            ("Cher", "Believe") => json!({ "similartracks": { "track": [{
                "name": "Strong Enough", "playcount": 866_023, "mbid": "", "match": 1,
                "url": "https://www.last.fm/music/Cher/_/Strong+Enough",
                "streamable": { "#text": "0", "fulltrack": "0" }, "duration": 223,
                "artist": { "name": "Cher", "mbid": "", "url": "https://www.last.fm/music/Cher" },
                "image": [],
            }], "@attr": { "artist": "Cher" } } }),
            _ => return not_found("Track not found"),
        },
        "artist.getInfo" => match param("artist") {
            "Cher" => json!({ "artist": {
                "name": "Cher", "mbid": "bfcc6d75-a6a5-4bc6-8282-47aec8531818",
                "url": "https://www.last.fm/music/Cher", "image": [], "streamable": "0", "ontour": "0",
                "stats": { "listeners": "1612343", "playcount": "26791548" },
                "similar": { "artist": [] }, "tags": { "tag": [] },
                "bio": {
                    "links": { "link": { "#text": "", "rel": "original", "href": "https://last.fm/music/Cher/+wiki" } },
                    "published": "27 Feb 2006, 15:18",
                    "summary": "Cher (born Cherilyn Sarkisian; May 20, 1946) is an American singer &amp; actress. <a href=\"https://www.last.fm/music/Cher\">Read more on Last.fm</a>",
                    "content": "Cher (born Cherilyn Sarkisian; May 20, 1946) is an American singer &amp; actress.\n\nShe rose to fame in 1965. <a href=\"https://www.last.fm/music/Cher\">Read more on Last.fm</a>. User-contributed text is available under the Creative Commons By-SA License; additional terms may apply.",
                },
            } }),
            // Last.fm has a page for any name it has heard, but nothing to say of it.
            "Nobody" => json!({ "artist": {
                "name": "Nobody", "url": "https://www.last.fm/music/Nobody",
                "bio": { "summary": " <a href=\"https://www.last.fm/music/Nobody\">Read more on Last.fm</a>", "content": "" },
            } }),
            _ => return not_found("The artist you supplied could not be found"),
        },
        _ => {
            return lastfm_error(
                StatusCode::BAD_REQUEST,
                3,
                "Invalid Method - No method with that name in this package",
            );
        }
    };
    json_response(StatusCode::OK, answer)
}

#[tokio::test]
async fn lastfm_reads_charts_tags_recommendations_and_biographies_with_a_key() {
    let seen: Seen = Arc::default();
    let asked = seen.clone();
    let app = Router::new().fallback(move |uri: Uri| {
        let asked = asked.clone();
        async move {
            let params = query_params(&uri);
            asked.lock().unwrap().push(params.clone());
            assert_eq!(uri.path(), "/");
            assert_eq!(params["format"], "json");
            match params.get("api_key").map(String::as_str) {
                Some(LASTFM_KEY) => lastfm_answer(&params),
                Some("suspended") => lastfm_error(
                    StatusCode::FORBIDDEN,
                    26,
                    "Suspended API key - Access for your account has been suspended, please contact Last.fm",
                ),
                Some("busy") => {
                    lastfm_error(StatusCode::TOO_MANY_REQUESTS, 29, "Rate Limit Exceded")
                }
                _ => lastfm_error(
                    StatusCode::FORBIDDEN,
                    10,
                    "Invalid API key - You must be granted a valid key by last.fm",
                ),
            }
        }
    });
    let address = serve(app);
    let base = "https://ws.audioscrobbler.com/2.0";
    let id = "dev.raygun.lastfm";
    let host = keyed_local_host("lastfm", id, &[base], address, LASTFM_KEY).await;
    let charts = "dev.raygun.lastfm/charts";
    let asked = |name: &str| last(&seen).get(name).cloned().unwrap_or_default();

    // Charts: Last.fm's, the country's and the genres'.
    let listed: Vec<_> = host
        .music_charts(Some("us"))
        .await
        .into_iter()
        .flat_map(|(_, answer)| answer.unwrap())
        .collect();
    assert_eq!(
        (
            listed[0].id.as_str(),
            listed[0].name.as_str(),
            listed[0].group
        ),
        ("top", "Top songs on Last.fm", ChartGroup::Top)
    );
    assert_eq!(
        (listed[1].id.as_str(), listed[1].name.as_str()),
        ("country:us", "Top songs in the United States")
    );
    let hip_hop = listed
        .iter()
        .find(|chart| chart.id == "tag:hip-hop")
        .unwrap();
    assert_eq!(
        (hip_hop.tag.as_deref(), hip_hop.group),
        (Some("Hip-Hop"), ChartGroup::Genre)
    );
    assert!(
        listed.iter().all(|chart| chart.artwork_url.is_none()),
        "no Last.fm images"
    );
    let countries = |region: &'static str| {
        let host = &host;
        async move {
            host.music_charts(Some(region))
                .await
                .into_iter()
                .flat_map(|(_, answer)| answer.unwrap())
                .filter(|chart| chart.id.starts_with("country:"))
                .map(|chart| chart.id)
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(countries("uk").await, ["country:gb"]);
    assert!(
        countries("xx").await.is_empty(),
        "a country Last.fm has no chart for"
    );
    assert!(
        seen.lock().unwrap().is_empty(),
        "listing charts asks nothing"
    );

    let top = host.music_chart(charts, "top", None, 10).await.unwrap();
    let songs: Vec<_> = top
        .iter()
        .map(|track| {
            (
                track.title.as_str(),
                track.artist.as_deref(),
                track.duration_ms,
            )
        })
        .collect();
    assert_eq!(
        songs,
        [
            ("Die With A Smile", Some("Lady Gaga"), None),
            ("APT.", Some("ROSÉ"), Some(170_000))
        ],
        "seconds to milliseconds, 0 for unknown; a repeat and a song without a name left out"
    );
    assert!(
        top.iter()
            .all(|track| track.artwork_url.is_none() && track.source.is_none())
    );
    assert_eq!(
        (asked("method"), asked("limit"), asked("api_key")),
        (
            "chart.getTopTracks".to_owned(),
            "10".to_owned(),
            LASTFM_KEY.to_owned()
        )
    );
    let country = host
        .music_chart(charts, "country:us", None, 10)
        .await
        .unwrap();
    assert_eq!(country[0].title, "Birds of a Feather");
    assert_eq!(asked("country"), "United States");
    let before = seen.lock().unwrap().len();
    let russia = host
        .music_chart(charts, "country:ru", None, 10)
        .await
        .unwrap();
    assert_eq!(russia.len(), 1);
    let names: Vec<_> = seen.lock().unwrap()[before..]
        .iter()
        .map(|params| params["country"].clone())
        .collect();
    assert_eq!(
        names,
        ["Russian Federation", "Russia"],
        "each name Last.fm may know"
    );
    let rock = host
        .music_chart(charts, "tag:rock", None, 10)
        .await
        .unwrap();
    assert_eq!(rock[0].title, "Bohemian Rhapsody", "one song as an object");
    assert_eq!(rock[0].duration_ms, Some(355_000));

    let rows = host
        .music_tag_songs(
            "Hip-Hop",
            &["hip hop".to_owned(), "rap".to_owned()],
            "genre",
            None,
            10,
        )
        .await;
    let (_, answer) = rows
        .iter()
        .find(|(provider, _)| provider.extension_id == id)
        .unwrap();
    let (name, tracks) = answer.as_ref().unwrap();
    assert_eq!(*name, None);
    assert_eq!(tracks[0].title, "HUMBLE.", "the first spelling with songs");
    assert_eq!(asked("tag"), "hip hop");

    // Tags.
    let song = |title: &str, artist: Option<&str>| TrackQuery {
        title: title.to_owned(),
        artist: artist.map(str::to_owned),
        album: None,
        duration_ms: None,
    };
    let tags_of = |query: TrackQuery| {
        let host = &host;
        async move {
            host.music_tags(&query)
                .await
                .into_iter()
                .find(|(provider, _)| provider.extension_id == "dev.raygun.lastfm")
                .unwrap()
                .1
                .unwrap()
                .into_iter()
                .map(|(name, weight)| (name, (weight * 100.0).round() / 100.0))
                .collect::<Vec<_>>()
        }
    };
    assert_eq!(
        tags_of(song("Believe", Some("Cher"))).await,
        [("pop".to_owned(), 1.0), ("dance".to_owned(), 0.88)],
        "no \"seen live\", years, the artist's name, repeats or rare tags"
    );
    assert_eq!(asked("autocorrect"), "1");
    assert_eq!(
        tags_of(song("B-side", Some("Cher"))).await,
        [
            ("pop".to_owned(), 0.5),
            ("female vocalists".to_owned(), 0.3)
        ],
        "an untagged song gets its artist's tags at half weight"
    );
    assert_eq!(asked("method"), "artist.getTopTags");
    assert!(tags_of(song("Song", Some("Nobody"))).await.is_empty());
    let before = seen.lock().unwrap().len();
    assert!(tags_of(song("Song", None)).await.is_empty());
    assert_eq!(seen.lock().unwrap().len(), before, "no artist, no request");

    // Recommendations.
    assert_eq!(
        host.similar_artists("Cher", 5).await,
        ["Sonny & Cher", "Madonna"],
        "not the artist itself"
    );
    let titles = |tracks: Vec<raygun_model::OnlineTrack>| {
        tracks
            .into_iter()
            .map(|track| track.title)
            .collect::<Vec<_>>()
    };
    assert_eq!(
        titles(
            host.recommended_tracks("topTracks", json!({ "artist": "Cher", "limit": 5 }))
                .await
        ),
        ["Believe", "Strong Enough"]
    );
    let like_believe = host
        .recommended_tracks(
            "radio",
            json!({ "artist": "Cher", "title": "Believe", "limit": 5 }),
        )
        .await;
    assert_eq!(
        (like_believe[0].title.as_str(), like_believe[0].duration_ms),
        ("Strong Enough", Some(223_000))
    );
    for title in [json!("Unknown B-side"), Value::Null] {
        let mix = host
            .recommended_tracks(
                "radio",
                json!({ "artist": "Cher", "title": title, "limit": 6 }),
            )
            .await;
        assert_eq!(
            titles(mix),
            ["I Got You Babe", "Like a Prayer", "The Beat Goes On"],
            "the closest artists' most played songs, taking turns"
        );
    }

    // Artist facts.
    let cher = host.artist_info("Cher").await.unwrap();
    assert_eq!(cher.name, "Cher");
    assert_eq!(
        cher.bio.as_deref(),
        Some("Cher (born Cherilyn Sarkisian; May 20, 1946) is an American singer & actress."),
        "the summary, without the link"
    );
    assert_eq!(cher.url.as_deref(), Some("https://www.last.fm/music/Cher"));
    assert_eq!(cher.image_url, None);
    assert_eq!(asked("autocorrect"), "1");
    assert_eq!(host.artist_info("Nobody").await, None, "nothing but a name");
    assert_eq!(host.artist_info("Someone Unknown").await, None);

    // Keys Last.fm refuses: the source waits for a new one.
    for (key, said) in [("refused", "didn't accept"), ("suspended", "suspended")] {
        let host = keyed_local_host("lastfm", id, &[base], address, key).await;
        let error = host
            .music_chart(charts, "top", None, 10)
            .await
            .unwrap_err()
            .to_string();
        assert_eq!(crate::needs_setup_in(&error), Some(id), "{key}: {error}");
        assert!(error.contains(said), "{error}");
        assert!(host.needs_setup(id).is_some());
    }
    // Too many requests: a plain failure, then nothing asked for a while.
    let busy = keyed_local_host("lastfm", id, &[base], address, "busy").await;
    let error = busy
        .music_chart(charts, "top", None, 10)
        .await
        .unwrap_err()
        .to_string();
    assert_eq!(crate::needs_setup_in(&error), None, "{error}");
    assert!(busy.needs_setup(id).is_none());
    let before = seen.lock().unwrap().len();
    let again = busy
        .music_chart(charts, "tag:rock", None, 10)
        .await
        .unwrap_err()
        .to_string();
    assert!(again.contains("limiting requests"), "{again}");
    assert_eq!(seen.lock().unwrap().len(), before, "it rests");
}
