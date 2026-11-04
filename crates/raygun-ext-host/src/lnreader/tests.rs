//! LNReader plugins through the host as the app calls them: a plugin written here in the shape
//! LNReader's compiled plugins have (CommonJS, `require` for cheerio, `@libs/fetch`, dayjs and
//! htmlparser2) against a stand-in site with a public-domain book, and an `#[ignore]` live check
//! of plugins from LNReader's own repository.

use std::collections::HashMap;
use std::net::SocketAddr;

use axum::Router;
use axum::http::{Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serde_json::json;

use crate::manga::MangaList;
use crate::novel::PassageBlock;
use crate::{ExtensionHost, ExtensionSource, HostConfig};

const PLUGIN: &str = r#""use strict";
Object.defineProperty(exports, "__esModule", { value: true });
var cheerio_1 = require("cheerio");
var fetch_1 = require("@libs/fetch");
var novelStatus_1 = require("@libs/novelStatus");
var filterInputs_1 = require("@libs/filterInputs");
var storage_1 = require("@libs/storage");
var dayjs_1 = require("dayjs");
var htmlparser2_1 = require("htmlparser2");
var ExamplePlugin = (function () {
  function ExamplePlugin() {
    this.id = "examplenovels";
    this.name = "Example Novels";
    this.icon = "src/en/example/icon.png";
    this.site = "SITE/";
    this.version = "1.0.0";
    this.newestFirst = storage_1.storage.get("newestFirst");
    this.pluginSettings = { newestFirst: { value: false, label: "Newest chapters first", type: "Switch" } };
    this.imageRequestInit = { headers: { Referer: "SITE/" } };
    this.filters = {
      sort: { type: filterInputs_1.FilterTypes.Picker, label: "Sort", value: "popular",
        options: [{ label: "Popular", value: "popular" }, { label: "New", value: "new" }] },
      genres: { type: filterInputs_1.FilterTypes.ExcludableCheckboxGroup, label: "Genres", value: { include: [], exclude: [] },
        options: [{ label: "Adventure", value: "adventure" }, { label: "Mystery", value: "mystery" }] },
      completed: { type: filterInputs_1.FilterTypes.Switch, label: "Completed only", value: false },
      keyword: { type: filterInputs_1.FilterTypes.TextInput, label: "Keyword", value: "" },
    };
  }
  ExamplePlugin.prototype.popularNovels = async function (page, _a) {
    var filters = _a.filters;
    var sort = _a.showLatestNovels ? "new" : filters.sort.value;
    var url = this.site + "list?page=" + page + "&sort=" + sort + "&genres=" + filters.genres.value.include.join(",");
    var $ = (0, cheerio_1.load)(await (await (0, fetch_1.fetchApi)(url)).text());
    return $("div.novel").map(function (i, el) {
      var link = $(el).find("h3 a");
      return { name: link.text().trim(), path: link.attr("href").replace(/^\//, ""), cover: $(el).find("img").attr("src") };
    }).get();
  };
  ExamplePlugin.prototype.searchNovels = async function (searchTerm, page) {
    var form = new FormData();
    form.append("q", searchTerm);
    form.append("page", String(page));
    var answer = await (0, fetch_1.fetchApi)(this.site + "search", { method: "POST", body: form });
    var $ = (0, cheerio_1.load)(await answer.text());
    var novels = [];
    $("li.result a").each(function () {
      novels.push({ name: $(this).text(), path: $(this).attr("href").slice(1) });
    });
    return novels;
  };
  ExamplePlugin.prototype.parseNovel = async function (path) {
    var $ = (0, cheerio_1.load)(await (await (0, fetch_1.fetchApi)(this.site + path)).text());
    var novel = {
      path: path,
      name: $("h1").first().text().trim(),
      cover: $(".cover img").attr("src"),
      summary: $(".summary p").map(function (i, p) { return $(p).text(); }).get().join("\n\n"),
      author: $(".info span:contains(Author)").next().text(),
      genres: $(".genres a").map(function (i, a) { return $(a).text(); }).toArray().join(","),
      status: $(".status").text().includes("Completed") ? novelStatus_1.NovelStatus.Completed : novelStatus_1.NovelStatus.Ongoing,
      chapters: [],
    };
    $(".chapters li").each(function (i, el) {
      var link = $(el).find("a");
      novel.chapters.push({
        name: link.text(),
        path: link.attr("href").slice(1),
        releaseTime: (0, dayjs_1.default)($(el).find("time").attr("datetime")).toISOString(),
        chapterNumber: i + 1,
      });
    });
    novel.totalPages = Number($(".pages").attr("data-total")) || 1;
    return novel;
  };
  ExamplePlugin.prototype.parsePage = async function (path, page) {
    var $ = (0, cheerio_1.load)(await (await (0, fetch_1.fetchApi)(this.site + path + "?page=" + page)).text());
    var chapters = [];
    $(".chapters li a").each(function (i, a) {
      chapters.push({ name: $(a).text(), path: $(a).attr("href").slice(1), chapterNumber: 3 });
    });
    return { chapters: chapters };
  };
  ExamplePlugin.prototype.parseChapter = async function (path) {
    var $ = (0, cheerio_1.load)(await (await (0, fetch_1.fetchApi)(this.site + path)).text());
    $(".chapter-content .ad").remove();
    var paragraphs = 0;
    var parser = new htmlparser2_1.Parser({ onopentag: function (name) { if (name === "p") paragraphs++; } });
    parser.write($(".chapter-content").html());
    parser.end();
    return $(".chapter-content").html() + "<p>" + paragraphs + " paragraphs</p>";
  };
  return ExamplePlugin;
}());
exports.default = new ExamplePlugin();
"#;

fn page(body: &str) -> Response {
    (
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        format!(
            "<!doctype html><html><head><title>Example</title></head><body>{body}</body></html>"
        ),
    )
        .into_response()
}

fn params(uri: &Uri) -> HashMap<String, String> {
    url::form_urlencoded::parse(uri.query().unwrap_or_default().as_bytes())
        .into_owned()
        .collect()
}

/// A stand-in novel site with H. G. Wells's The Time Machine (1895, public domain).
fn site() -> Router {
    Router::new()
        .route(
            "/list",
            get(|uri: Uri| async move {
                let query = params(&uri);
                // A browser check, as a site behind one answers.
                if query.get("page").map(String::as_str) == Some("7") {
                    return (
                        axum::http::StatusCode::FORBIDDEN,
                        [(header::CONTENT_TYPE, "text/html")],
                        "<!DOCTYPE html><html><head><title>Just a moment...</title></head><body></body></html>",
                    )
                        .into_response();
                }
                if query.get("page").map(String::as_str) != Some("1") {
                    return page("");
                }
                let first = if query.get("sort").map(String::as_str) == Some("new") {
                    "The War of the Worlds"
                } else {
                    "The Time Machine"
                };
                page(&format!(
                    r#"<div class="novel"><img src="/covers/1.jpg"><h3><a href="/novel/time-machine">{first}</a></h3></div>
                    <div class="novel"><img src="https://img.example.org/2.jpg"><h3><a href="/novel/war-of-the-worlds">  The War of the Worlds </a></h3></div>
                    <p>genres: {}</p>"#,
                    query.get("genres").cloned().unwrap_or_default()
                ))
            }),
        )
        .route(
            "/search",
            post(|headers: axum::http::HeaderMap, body: String| async move {
                let form = header_text(&headers, "content-type");
                assert!(form.starts_with("application/x-www-form-urlencoded"), "{form}");
                let found: HashMap<String, String> = url::form_urlencoded::parse(body.as_bytes()).into_owned().collect();
                if found.get("q").map(String::as_str) == Some("time machine") {
                    page(r#"<ul><li class="result"><a href="/novel/time-machine">The Time Machine</a></li></ul>"#)
                } else {
                    page("<ul></ul>")
                }
            }),
        )
        .route(
            "/novel/time-machine",
            get(|uri: Uri| async move {
                if params(&uri).get("page").map(String::as_str) == Some("2") {
                    return page(r#"<ul class="chapters"><li><a href="/novel/time-machine/3">III</a></li></ul>"#);
                }
                page(
                    r#"<h1>The Time Machine</h1><h1>Ignored</h1><div class="cover"><img src="/covers/1.jpg"></div>
                    <div class="summary"><p>A scientist builds a machine.</p><p>He travels to the year 802,701.</p></div>
                    <div class="info"><span>Author</span><span>H. G. Wells</span></div>
                    <div class="genres"><a>Science fiction</a><a>Classic</a></div>
                    <div class="status">Completed</div>
                    <ul class="chapters">
                      <li><a href="/novel/time-machine/1">I</a><time datetime="2024-01-01T00:00:00Z"></time></li>
                      <li><a href="/novel/time-machine/2">II</a><time datetime="2024-02-01T00:00:00Z"></time></li>
                    </ul><div class="pages" data-total="2"></div>"#,
                )
            }),
        )
        .route(
            "/novel/time-machine/2",
            get(|| async {
                page(
                    r#"<div class="chapter-content"><h2>II</h2><p>I think that at that time none of us quite believed in the Time Machine.</p><div class="ad">Buy now</div><p>The fact is, the Time Traveller was one of those men who are too clever to be believed.</p></div>"#,
                )
            }),
        )
}

fn header_text(headers: &axum::http::HeaderMap, name: &str) -> String {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned()
}

fn serve(app: Router) -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    address
}

fn plugin_source(address: SocketAddr) -> ExtensionSource {
    let manifest = json!({
        "id": "lnreader.examplenovels", "name": "Example Novels", "version": "1.0.0", "apiVersion": 1,
        "runtime": "lnreader", "entry": "index.js", "provides": ["novel.source"],
        "permissions": { "net": ["127.0.0.1"] },
    });
    let code = PLUGIN.replace("SITE", &format!("http://{address}"));
    ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest.to_string()),
        ("index.js", &code),
    ])
}

#[tokio::test(flavor = "multi_thread")]
async fn a_plugin_lists_reads_and_searches_like_in_lnreader() {
    let address = serve(site());
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(vec![plugin_source(address)]).await;
    assert_eq!(
        infos[0].status,
        raygun_model::ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    assert_eq!(infos[0].providers[0].kind, "novel.source");
    assert_eq!(
        infos[0].settings[0].key, "newestFirst",
        "the plugin's settings"
    );
    let provider = "lnreader.examplenovels/lnreader";

    let popular = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    let titles: Vec<&str> = popular
        .items
        .iter()
        .map(|item| item.title.as_str())
        .collect();
    assert_eq!(titles, ["The Time Machine", "The War of the Worlds"]);
    assert_eq!(popular.items[0].id, "novel/time-machine");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/covers/1.jpg").as_str()),
        "covers resolve against the site"
    );
    assert_eq!(
        popular.items[0].cover_headers["Referer"],
        format!("http://{address}/")
    );
    assert!(popular.has_next_page);
    let latest = host
        .manga_list(provider, MangaList::Latest, 1)
        .await
        .unwrap();
    assert_eq!(
        latest.items[0].title, "The War of the Worlds",
        "Latest asks for new ones"
    );
    assert!(
        host.manga_list(provider, MangaList::Popular, 2)
            .await
            .unwrap()
            .items
            .is_empty()
    );
    // The plugin reads the check's page as an empty list; Raygun says what the site did.
    let checked = host
        .manga_list(provider, MangaList::Popular, 7)
        .await
        .unwrap_err();
    assert!(checked.contains("wants a browser check"), "{checked}");

    let found = host
        .manga_search(provider, "time machine", 1)
        .await
        .unwrap();
    assert_eq!(found.items.len(), 1, "searched with a posted form");

    let filters = host.manga_filters(provider).await.unwrap();
    assert_eq!(filters.len(), 3, "free text has no Raygun filter");
    let filtered = host
        .call(
            provider,
            "search",
            json!({ "query": "", "page": 1, "filters": { "genres": { "include": ["mystery"], "exclude": [] } } }),
        )
        .await
        .unwrap();
    assert_eq!(filtered["items"].as_array().unwrap().len(), 2);

    let details = host
        .manga_details(provider, "novel/time-machine")
        .await
        .unwrap();
    assert_eq!(details.title, "The Time Machine");
    assert_eq!(details.authors, ["H. G. Wells"]);
    assert_eq!(details.genres, ["Science fiction", "Classic"]);
    assert_eq!(details.status, "completed");
    assert_eq!(
        details.description.as_deref(),
        Some("A scientist builds a machine.\n\nHe travels to the year 802,701.")
    );
    let chapters = host
        .manga_chapters(provider, "novel/time-machine")
        .await
        .unwrap();
    let names: Vec<&str> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["I", "II", "III"],
        "the second page's chapters follow"
    );
    assert_eq!(
        chapters[1].date_upload,
        Some(1_706_745_600_000),
        "1 February 2024"
    );

    let passage = host
        .novel_passage(provider, "novel/time-machine/2")
        .await
        .unwrap();
    let texts: Vec<String> = passage
        .iter()
        .map(|block| match block {
            PassageBlock::Paragraph(text) | PassageBlock::Heading(text) => text.clone(),
            other => format!("{other:?}"),
        })
        .collect();
    assert_eq!(texts[0], "II");
    assert!(
        !texts.iter().any(|text| text.contains("Buy now")),
        "removed with cheerio"
    );
    assert_eq!(
        texts.last().map(String::as_str),
        Some("2 paragraphs"),
        "counted with htmlparser2"
    );
}

#[test]
fn the_cheerio_api_answers_as_cheerio_does() {
    // Runs a script against the prelude alone: each line must hold.
    let script = r#"
      const { load } = __lnrRequire('cheerio');
      const $ = load('<div id="a" class="x y"><p>One <b>two</b></p><p data-n="3">Three</p><ul><li>1</li><li>2</li><li>3</li></ul></div>');
      const check = (name, value, expected) => { if (JSON.stringify(value) !== JSON.stringify(expected)) throw new Error(name + ': ' + JSON.stringify(value)); };
      check('text', $('p').first().text(), 'One two');
      check('length', $('p').length, 2);
      check('attr', $('#a').attr('id'), 'a');
      check('missing attr', $('#a').attr('title'), undefined);
      check('hasClass', $('#a').hasClass('y'), true);
      check('data', $('p').eq(1).data('n'), 3);
      check('map', $('li').map((i, el) => $(el).text()).get(), ['1', '2', '3']);
      check('last', $('li:last').text(), '3');
      check('eq', $('li:eq(1)').text(), '2');
      check('first pseudo', $('li:first').text(), '1');
      check('next', $('p').first().next().text(), 'Three');
      check('parent', $('b').parent().prop('tagName'), 'P');
      check('closest', $('b').closest('div').attr('id'), 'a');
      check('children', $('ul').children().length, 3);
      check('contents', $('p').first().contents().length, 2);
      check('filter', $('li').filter((i) => i > 0).length, 2);
      check('not', $('li').not(':first').length, 2);
      check('is', $('p').is('[data-n]'), true);
      check('has', $('p').has('b').length, 1);
      check('own text', $('p').first().clone().children().remove().end().text(), 'One ');
      check('html', $('p').first().html(), 'One <b>two</b>');
      $('b').replaceWith('<i>2</i>');
      check('replaceWith', $('p').first().html(), 'One <i>2</i>');
      $('ul').append('<li>4</li>');
      check('append', $('li').length, 4);
      $('li').last().remove();
      check('remove', $('li').length, 3);
      $('#a').removeAttr('class').attr('title', 'T');
      check('attributes', $('#a').attr(), { id: 'a', title: 'T' });
      check('each', (() => { const seen = []; $('li').each(function (i) { seen.push(i + $(this).text()); }); return seen; })(), ['01', '12', '23']);
      check('node', $('li')[0].type + ':' + $('li')[0].name + ':' + $('li')[0].children[0].data, 'tag:li:1');
      check('created', $('<span class="c">new</span>').text(), 'new');
      check('dayjs', __lnrRequire('dayjs')('2023-12-02T00:00:00Z').toISOString(), '2023-12-02T00:00:00.000Z');
      check('dayjs format', __lnrRequire('dayjs')('05/02/2021', 'DD/MM/YYYY').format('YYYY-MM-DD'), '2021-02-05');
      check('dayjs subtract', __lnrRequire('dayjs')('2023-12-10T00:00:00Z').subtract(3, 'day').toISOString(), '2023-12-07T00:00:00.000Z');
      check('url', new URL('/b?x=1', 'https://example.org/a/').href, 'https://example.org/b?x=1');
      check('search params', new URLSearchParams({ a: '1 2', b: 'é' }).toString(), 'a=1+2&b=%C3%A9');
      check('urlencode', __lnrRequire('urlencode').encode('中文', 'gbk'), '%D6%D0%CE%C4');
      exports.default = {
        id: 'probe', name: 'Probe', site: 'https://example.org/', version: '1.0.0',
        popularNovels: async () => [], searchNovels: async () => [], parseNovel: async () => ({}), parseChapter: async () => '' };
    "#;
    let runtime = tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
        .unwrap();
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async {
        let manifest = crate::manifest::Manifest::parse(
            &json!({
                "id": "lnreader.probe", "name": "Probe", "version": "1.0.0", "apiVersion": 1,
                "runtime": "lnreader", "entry": "index.js", "provides": ["novel.source"],
            })
            .to_string(),
        )
        .unwrap();
        // Its checks throw on a mismatch, which fails the load.
        let loaded = crate::js::JsExtension::load(
            manifest,
            script.to_owned(),
            crate::storage::KvStore::open(None),
            crate::cookies::CookieJar::open(None),
            Default::default(),
            &crate::Limits::default(),
            crate::ContentFilter::default(),
        )
        .await;
        if let Err(error) = loaded {
            panic!("{error}");
        }
    });
}

/// LNReader's own repository, and plugins of sites that publish original or licensed fiction.
/// Sites behind a browser check answer with one (which Raygun never passes); at least one of
/// them must read from listing to chapter text.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "network"]
async fn lnreader_live() {
    let client = raygun_net::http_client().unwrap();
    let index_url = "https://raw.githubusercontent.com/LNReader/lnreader-plugins/plugins/v3.0.0/.dist/plugins.min.json";
    let text = client
        .get(index_url)
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let index = crate::repo::parse_index(&text).unwrap();
    println!("{} plugins", index.extensions.len());
    let folder = tempfile::tempdir().unwrap();
    let base: url::Url = index_url.parse().unwrap();
    for id in [
        "lnreader.royalroad",
        "lnreader.scribblehub",
        "lnreader.inkitt",
        "lnreader.wuxiaworld",
        "lnreader.webnovel",
    ] {
        let Some(entry) = index.extensions.iter().find(|entry| entry.id == id) else {
            println!("{id}: no longer listed");
            continue;
        };
        let url = crate::repo::package_url(&base, entry).unwrap();
        let bytes = client.get(url).send().await.unwrap().bytes().await.unwrap();
        let script = super::package::verify_script(&bytes, entry).unwrap();
        let verified = super::package::package(entry, script).unwrap();
        crate::package::install(folder.path(), &verified).unwrap();
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let mut read = 0;
    for info in host.load(crate::discover_installed(folder.path())).await {
        assert_eq!(
            info.status,
            raygun_model::ExtensionStatus::Loaded,
            "{}: {:?}",
            info.id,
            info.error
        );
        let provider = info.providers[0].id.clone();
        let outcome = async {
            let popular = host.manga_list(&provider, MangaList::Popular, 1).await?;
            let novel = popular.items.first().ok_or("no novels listed")?;
            let details = host.manga_details(&provider, &novel.id).await?;
            let chapters = host.manga_chapters(&provider, &novel.id).await?;
            let first = chapters.first().ok_or("no chapters")?;
            let passage = host.novel_passage(&provider, &first.id).await?;
            Ok::<_, String>(format!(
                "{} — {} chapters, the first {} blocks",
                details.title,
                chapters.len(),
                passage.len()
            ))
        }
        .await;
        match outcome {
            Ok(summary) => {
                println!("{provider}: {summary}");
                read += 1;
            }
            Err(error) => println!("{provider}: {}", error.lines().next().unwrap_or_default()),
        }
    }
    assert!(read > 0, "no plugin read a chapter");
}
