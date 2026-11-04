//! The public-domain and openly licensed sources (Standard Ebooks, Wikisource, Wikimedia Commons,
//! NASA, Blender open movies, and the Internet Archive's open audio and public-domain films)
//! against local fixture servers: the real bundles, their API bases pointed at this machine.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::Path;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use raygun_model::{ExtensionStatus, FilterValue, MediaSource, TrackQuery};
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

fn ebook_item(id: &str, title: &str) -> String {
    let slug = id.replace('/', "_");
    format!(
        r#"<li typeof="schema:Book" about="/ebooks/{id}">
        <div class="thumbnail-container" aria-hidden="true"><a href="/ebooks/{id}" property="schema:url"><picture>
        <img src="/images/covers/{slug}/abc/cover@2x.jpg" alt="" property="schema:image"/></picture></a></div>
        <p><a href="/ebooks/{id}" property="schema:url"><span property="schema:name">{title}</span></a></p>
        <p class="author" typeof="schema:Person" property="schema:author"><a href="/ebooks/someone"><span property="schema:name">Someone</span></a></p>
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
                        (_, "2") => ebook_item("mary-shelley/frankenstein", "Frankenstein"),
                        _ => [
                            ebook_item("charles-dickens/barnaby-rudge", "Barnaby Rudge"),
                            ebook_item(WAR_AND_PEACE, "War &amp; Peace"),
                            ebook_item("Not An Id", "Broken"),
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
