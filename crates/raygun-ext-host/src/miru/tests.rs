//! The Miru runtime against a local server: Miru's API as scripts use it, each kind of source
//! through the host's calls, and (ignored) surveys of real Miru repositories.

use std::collections::BTreeMap;
use std::net::SocketAddr;
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::routing::{get, post};
use raygun_model::{ExtensionStatus, FilterValue, SettingKind, SettingValue, SourceFilter};
use serde_json::{Value, json};

use crate::manga::MangaList;
use crate::novel::PassageBlock;
use crate::{ExtensionHost, ExtensionSource, HostConfig, UserSettings};

fn serve() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let json_body = |value: Value| {
        (
            [(header::CONTENT_TYPE, "application/json")],
            value.to_string(),
        )
    };
    let app = Router::new()
        .route("/api/json", get(move || async move { json_body(json!({ "hello": "world" })) }))
        .route("/alt/api/json", get(move || async move { json_body(json!({ "alt": true })) }))
        .route("/api/text", get(|| async { ([(header::CONTENT_TYPE, "text/plain")], "plain text") }))
        .route(
            "/api/list",
            get(move |uri: Uri| async move {
                let page: u32 = uri
                    .query()
                    .and_then(|query| query.strip_prefix("page="))
                    .and_then(|page| page.parse().ok())
                    .unwrap_or(1);
                // Past page 2 the "site" ignores the page and sends page 2 again.
                let page = page.min(2);
                json_body(json!({ "items": [
                    { "id": format!("show-{page}a"), "name": format!("Show {page}A"), "cover": "http://img.example/a.jpg" },
                    { "id": format!("show-{page}b"), "name": format!("Show {page}B") },
                ] }))
            }),
        )
        .route(
            "/echo",
            post(|headers: HeaderMap, body: String| async move {
                let kind = headers
                    .get(header::CONTENT_TYPE)
                    .and_then(|value| value.to_str().ok())
                    .unwrap_or("")
                    .to_owned();
                ([(header::CONTENT_TYPE, "application/json")], json!({ "type": kind, "body": body }).to_string())
            }),
        )
        .route("/echo-query", get(|uri: Uri| async move { uri.query().unwrap_or("").to_owned() }))
        .route("/missing", get(|| async { (StatusCode::NOT_FOUND, "no such page") }))
        .route(
            "/whoami",
            get(|headers: HeaderMap| async move {
                let value = |name: &str| {
                    headers
                        .get(name)
                        .and_then(|value| value.to_str().ok())
                        .unwrap_or("")
                        .to_owned()
                };
                ([(header::CONTENT_TYPE, "application/json")], json!({
                    "agent": value("user-agent"),
                    "miru": value("miru-url"),
                    "cookie": value("cookie"),
                }).to_string())
            }),
        )
        .route(
            "/cookie",
            get(|| async { [(header::SET_COOKIE, "visited=yes; Max-Age=3600; Path=/")] }),
        )
        .route(
            "/page",
            get(|| async {
                axum::response::Html(
                    r#"<html><body><h1> The Title </h1>
                    <ul><li class="item"><a href="/s/1" title="One">One</a></li><li class="item"><a href="/s/2">Two</a></li></ul>
                    <div class="box">Keep<span class="ad">Buy now</span> this</div></body></html>"#,
                )
            }),
        );
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

/// A Miru script as an installed extension; `net` stands for the `*` Miru extensions get (this
/// machine is never public, so tests name it).
fn source(script: &str, net: &[&str]) -> ExtensionSource {
    let header = crate::miru::header::parse(script).unwrap();
    let manifest = json!({
        "id": format!("miru.{}", header.package),
        "name": header.name,
        "version": "1.0.0",
        "apiVersion": 1,
        "runtime": "miru",
        "entry": "index.js",
        "provides": [header.kind.provider_kind()],
        "permissions": { "net": net },
    });
    ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest.to_string()),
        ("index.js", script),
    ])
}

fn script(package: &str, kind: &str, address: SocketAddr, body: &str) -> String {
    format!(
        "// ==MiruExtension==\n// @name         {package} test\n// @version      v1.0.0\n// @author       raygun\n// @lang         en\n// @license      MIT\n// @package      {package}\n// @type         {kind}\n// @webSite      http://{address}\n// ==/MiruExtension==\n\nexport default class extends Extension {{\n{body}\n}}\n"
    )
}

async fn host_with(sources: Vec<ExtensionSource>, settings: UserSettings) -> ExtensionHost {
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load_with_settings(sources, settings).await;
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

/// Runs one of the probe script's checks: its search answers with the result as a title.
async fn probe(host: &ExtensionHost, name: &str) -> Value {
    let listing = host
        .manga_search("miru.probe/source", name, 1)
        .await
        .unwrap_or_else(|error| panic!("{name}: {error}"));
    serde_json::from_str(&listing.items[0].title).unwrap()
}

const PROBES: &str = r#"
  async load() {
    await this.registerSetting({ title: 'Quality', key: 'quality', type: 'radio', defaultValue: 'hd', options: { High: 'hd', Low: 'sd' } });
    await this.registerSetting({ title: '  Mirror  ', key: 'mirror', type: 'input', defaultValue: 'main', description: 'Where from' });
    await this.registerSetting({ title: 'Subtitles', key: 'subs', type: 'toggle', defaultValue: 'false' });
  }
  async latest() { return []; }
  async search(kw) {
    const run = this[`probe_${kw}`];
    return [{ title: JSON.stringify(await run.call(this)), url: kw }];
  }
  async probe_requests() {
    const json = await this.request('/api/json');
    const text = await this.request('/api/text');
    const other = await this.request('/api/json', { headers: { 'Miru-Url': 'http://ADDR/alt' } });
    const absolute = await this.request('http://ADDR/alt/api/json');
    const posted = await this.request('/echo', { method: 'post', data: { a: 1, b: 'x y' } });
    const form = await this.request('/echo', { method: 'post', data: { a: 1, b: 'x y!' },
      headers: { 'Content-Type': 'application/x-www-form-urlencoded' } });
    const raw = await this.request('/echo', { method: 'post', data: 'as is', headers: { 'content-type': 'text/plain' } });
    const query = await this.request('/echo-query?x=1', { queryParameters: { q: 'a b', n: 2 } });
    let failed;
    try { await this.request('/missing'); } catch (error) { failed = error.message; }
    await this.request('/cookie');
    const who = await this.request('/whoami', { headers: { 'Miru-Url': 'http://ADDR' } });
    return { json, text, other, absolute, posted, form, raw, query, failed, who };
  }
  async probe_html() {
    const res = await this.request('/page');
    const title = await this.querySelector(res, 'h1').text;
    const items = await this.querySelectorAll(res, 'li.item');
    const names = await Promise.all(items.map((item) => this.querySelector(item.content, 'a').text));
    const hrefs = await Promise.all(items.map((item) => this.getAttributeText(item.content, 'a', 'href')));
    const outer = await items[0].outerHTML;
    const missing = await this.getAttributeText(res, 'li.item a', 'data-missing');
    const chained = await (await this.querySelector(res, 'ul').querySelector('li:nth-child(2)')).text;
    const viaElement = await this.querySelector(res, 'a').getAttributeText('href');
    // As in Miru: an element's own selector runs again on its HTML, so a path finds nothing.
    const viaPath = await this.querySelector(res, 'li.item a').getAttributeText('href');
    const cleaned = await (await this.querySelector(res, '.box').removeSelector('.ad')).text;
    const inner = await this.querySelector(res, '.box').innerHTML;
    let bad;
    try { await this.querySelector(res, 'li[').text; } catch (error) { bad = error.message; }
    const xpathTitle = await this.queryXPath(res, '//h1').text;
    const xpathHrefs = await this.queryXPath(res, '//li[@class="item"]/a/@href').attrs;
    const xpathHref = await this.queryXPath(res, '//li[@class="item"][2]/a/@href').attr;
    const all = await this.queryXPath(res, '//li[@class="item"]').allHTML;
    return { title, names, hrefs, outer, missing, chained, viaElement, viaPath, cleaned, inner, bad, xpathTitle,
      xpathHrefs, xpathHref, allCount: all.length, allFirst: all[0], allText: String(all).slice(0, 4) };
  }
  async probe_timers() {
    const order = [];
    setTimeout(() => order.push('b'), 40);
    setTimeout((value) => order.push(value), 10, 'a');
    const cancelled = setTimeout(() => order.push('never'), 5);
    clearTimeout(cancelled);
    await new Promise((resolve) => setTimeout(resolve, 100));
    const raced = await Promise.any([
      new Promise((_, reject) => setTimeout(() => reject(new Error('slow')), 50)),
      new Promise((resolve) => setTimeout(() => resolve('fast'), 5)),
    ]);
    return { order, raced };
  }
  async probe_crypto() {
    return {
      md5: md5('hello'),
      md5Utf8: md5('日本'),
      md5Bytes: md5('hello', { asBytes: true }).length,
      sha256: CryptoJS.SHA256('abc').toString(),
      aes: CryptoJS.AES.decrypt(CryptoJS.AES.encrypt('secret', 'key').toString(), 'key').toString(CryptoJS.enc.Utf8),
      base64: CryptoJS.enc.Base64.stringify(CryptoJS.enc.Utf8.parse('Raygun')),
      btoa: [btoa('Raygun'), btoa('ab'), btoa('a')],
      atob: [atob('UmF5Z3Vu'), atob('YWI='), atob(' YQ== ')],
      sameWindow: window === globalThis && global === globalThis,
    };
  }
  async probe_fetch() {
    const response = await fetch('http://ADDR/api/json');
    const posted = await fetch('http://ADDR/echo', { method: 'POST', headers: { 'Content-Type': 'text/plain' }, body: 'hi' });
    return { ok: response.ok, status: response.status, type: response.headers.get('Content-Type'),
      json: await response.json(), echo: await posted.json() };
  }
  async probe_settings() {
    return [await this.getSetting('quality'), await this.getSetting('subs'), await this.getSetting('mirror'),
      await this.getSetting('nope'), this.package, this.name];
  }
  async probe_webview() {
    try { await this.openWebView('https://example.org'); return 'opened'; } catch (error) { return error.message; }
  }
"#;

fn probe_source(address: SocketAddr) -> ExtensionSource {
    let body = PROBES.replace("ADDR", &address.to_string());
    source(&script("probe", "manga", address, &body), &["127.0.0.1"])
}

#[tokio::test]
async fn requests_follow_mirus_rules() {
    let address = serve();
    let host = host_with(vec![probe_source(address)], UserSettings::new()).await;
    let result = probe(&host, "requests").await;
    assert_eq!(
        result["json"],
        json!({ "hello": "world" }),
        "JSON answers are parsed"
    );
    assert_eq!(result["text"], "plain text", "others stay text");
    assert_eq!(
        result["other"],
        json!({ "alt": true }),
        "Miru-Url replaces the base"
    );
    assert_eq!(
        result["absolute"],
        json!({ "alt": true }),
        "a full address isn't put after the base"
    );
    assert_eq!(
        result["posted"],
        json!({ "type": "application/json; charset=utf-8", "body": r#"{"a":1,"b":"x y"}"# }),
        "an object goes as JSON"
    );
    assert_eq!(
        result["form"],
        json!({ "type": "application/x-www-form-urlencoded", "body": "a=1&b=x+y%21" }),
        "or as a form when the script says so"
    );
    assert_eq!(result["raw"]["body"], "as is");
    assert_eq!(result["query"], "x=1&q=a+b&n=2");
    let failed = result["failed"].as_str().unwrap();
    assert!(
        failed.contains("404") && failed.contains("127.0.0.1"),
        "{failed}"
    );
    assert_eq!(result["who"]["agent"], raygun_net::user_agent());
    assert_eq!(
        result["who"]["miru"],
        format!("http://{address}"),
        "the header still goes"
    );
    assert_eq!(
        result["who"]["cookie"], "visited=yes",
        "the extension keeps cookies"
    );
}

#[tokio::test]
async fn html_helpers_answer_like_miru() {
    let address = serve();
    let host = host_with(vec![probe_source(address)], UserSettings::new()).await;
    let result = probe(&host, "html").await;
    assert_eq!(
        result["title"], " The Title ",
        "text is the DOM's, untrimmed"
    );
    assert_eq!(result["names"], json!(["One", "Two"]));
    assert_eq!(result["hrefs"], json!(["/s/1", "/s/2"]));
    assert_eq!(
        result["outer"], r#"<li class="item"><a href="/s/1" title="One">One</a></li>"#,
        "attributes keep their order"
    );
    assert_eq!(result["missing"], Value::Null);
    assert_eq!(result["chained"], "Two");
    assert_eq!(result["viaElement"], "/s/1");
    assert_eq!(result["viaPath"], Value::Null);
    assert_eq!(result["cleaned"], "Keep this");
    assert_eq!(
        result["inner"],
        r#"Keep<span class="ad">Buy now</span> this"#
    );
    assert!(result["bad"].as_str().unwrap().contains("invalid selector"));
    assert_eq!(result["xpathTitle"], " The Title ");
    assert_eq!(
        result["xpathHrefs"], r#"["/s/1","/s/2"]"#,
        "a JSON string, as Miru sends"
    );
    assert_eq!(result["xpathHref"], "/s/2");
    assert_eq!(result["allCount"], 2);
    assert_eq!(
        result["allFirst"],
        r#"<li class="item"><a href="/s/1" title="One">One</a></li>"#
    );
    assert_eq!(result["allText"], "[<li", "printed as Dart prints a list");
}

#[tokio::test]
async fn timers_fetch_crypto_and_base64() {
    let address = serve();
    let host = host_with(vec![probe_source(address)], UserSettings::new()).await;
    assert_eq!(
        probe(&host, "timers").await,
        json!({ "order": ["a", "b"], "raced": "fast" })
    );
    assert_eq!(
        probe(&host, "crypto").await,
        json!({
            "md5": "5d41402abc4b2a76b9719d911017c592",
            "md5Utf8": "4dbed2e657457884e67137d3514119b3",
            "md5Bytes": 16,
            "sha256": "ba7816bf8f01cfea414140de5dae2223b00361a396177a9cb410ff61f20015ad",
            "aes": "secret",
            "base64": "UmF5Z3Vu",
            "btoa": ["UmF5Z3Vu", "YWI=", "YQ=="],
            "atob": ["Raygun", "ab", "a"],
            "sameWindow": true,
        })
    );
    assert_eq!(
        probe(&host, "fetch").await,
        json!({
            "ok": true, "status": 200, "type": "application/json",
            "json": { "hello": "world" },
            "echo": { "type": "text/plain", "body": "hi" },
        })
    );
    let refused = probe(&host, "webview").await;
    assert!(refused.as_str().unwrap().contains("bot check"), "{refused}");
}

#[tokio::test]
async fn settings_registered_while_loading_are_raygun_settings() {
    let address = serve();
    let host = host_with(vec![probe_source(address)], UserSettings::new()).await;
    let infos = host.list().await;
    let settings = &infos[0].settings;
    let keys: Vec<&str> = settings
        .iter()
        .map(|setting| setting.key.as_str())
        .collect();
    assert_eq!(keys, ["quality", "mirror", "subs"]);
    assert_eq!(settings[0].kind, SettingKind::Select);
    assert_eq!(settings[0].options, ["High", "Low"]);
    assert_eq!(settings[0].value, Some(SettingValue::Text("High".into())));
    assert_eq!(settings[1].label, "Mirror");
    assert_eq!(settings[2].kind, SettingKind::Boolean);
    assert_eq!(
        probe(&host, "settings").await,
        json!(["hd", "false", "main", null, "probe", "probe test"])
    );

    // The user's choices: a label stands for its value; a value of the wrong kind is dropped.
    let mut chosen = UserSettings::new();
    chosen.insert(
        "miru.probe".into(),
        BTreeMap::from([
            ("quality".to_owned(), json!("Low")),
            ("subs".to_owned(), json!(true)),
            ("mirror".to_owned(), json!(5)),
        ]),
    );
    let host = host_with(vec![probe_source(address)], chosen).await;
    assert_eq!(
        probe(&host, "settings").await,
        json!(["sd", "true", "main", null, "probe", "probe test"])
    );
    let infos = host.list().await;
    assert_eq!(
        infos[0].settings[1].value,
        Some(SettingValue::Text("main".into())),
        "the bad value shows as the default"
    );
}

const VIDEO: &str = r#"
  async latest(page) {
    const res = await this.request(`/api/list?page=${page}`);
    return res.items.map((item) => ({ title: item.name, url: item.id, cover: item.cover }));
  }
  async createFilter() {
    return {
      genre: { title: 'Genre', min: 0, max: 1, default: '', options: { action: 'Action', drama: 'Drama' } },
      'tags list': { title: 'Tags', min: 0, max: 2, default: 'all', options: { all: 'All', a: 'A', b: 'B', c: 'C' } },
    };
  }
  async search(kw, page, filter) {
    return [{ title: JSON.stringify({ kw, page, filter }), url: 'found' }];
  }
  async detail(url) {
    return { title: `Show ${url}`, cover: 'http://img.example/c.jpg', desc: 'About it',
      studio: ['video test', 'Studio Ghibli'], cast: [{ name: 'Ann' }, 'Bob'], episodes: [
      { title: 'Line 1', urls: [{ name: 'Episode 1', url: 'l1e1' }, { name: 'Episode 2', url: 'l1e2' }] },
      { title: 'Line 2', urls: [{ name: 'Episode 1', url: 'l2e1' }, { name: 'Episode 2', url: 'l2e2' }, { name: 'Episode 3', url: 'l2e3' }] },
    ] };
  }
  async watch(url) {
    if (url === 'l1e1') return { type: 'hls', url: 'http://ADDR/v/1.m3u8',
      subtitles: [{ title: 'English', url: 'http://ADDR/s/en.vtt', language: 'en' }] };
    if (url === 'l2e3') return { type: 'torrent', url: 'magnet:?xt=urn:btih:abc' };
    return { type: 'mp4', url: `http://ADDR/v/${url}.mp4`, headers: { Referer: 'http://ADDR/' } };
  }
"#;

#[tokio::test]
async fn video_sources_list_search_and_play() {
    let address = serve();
    let body = VIDEO.replace("ADDR", &address.to_string());
    let host = host_with(
        vec![source(
            &script("video", "bangumi", address, &body),
            &["127.0.0.1"],
        )],
        UserSettings::new(),
    )
    .await;
    let provider = "miru.video/source";
    let first = host
        .manga_list(provider, MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(first.items.len(), 2);
    assert!(first.has_next_page);
    assert_eq!(first.items[0].title, "Show 1A");
    assert_eq!(
        first.items[0].cover_url.as_deref(),
        Some("http://img.example/a.jpg")
    );
    // Miru's own headers for covers: the last request as the referrer, the user agent.
    let headers = &first.items[0].cover_headers;
    assert_eq!(
        headers.get("Referer").map(String::as_str),
        Some(format!("http://{address}/api/list?page=1").as_str())
    );
    assert_eq!(
        headers.get("User-Agent").map(String::as_str),
        Some(raygun_net::user_agent().as_str())
    );
    let second = host
        .manga_list(provider, MangaList::Latest, 2)
        .await
        .unwrap();
    assert_eq!(second.items[0].title, "Show 2A");
    let third = host
        .manga_list(provider, MangaList::Latest, 3)
        .await
        .unwrap();
    assert!(
        third.items.is_empty() && !third.has_next_page,
        "the same page again ends the list"
    );

    // Filters: Miru's createFilter, a select with an "All" for its empty default, and tags.
    let filters = host.manga_filters(provider).await.unwrap();
    assert_eq!(filters.len(), 2);
    let SourceFilter::Select {
        key,
        options,
        default,
        ..
    } = &filters[0]
    else {
        panic!("{filters:?}")
    };
    assert_eq!(key, "genre");
    assert_eq!(options[0].label, "All");
    assert_eq!(default.as_ref(), Some(&options[0].value));
    let SourceFilter::Tags { key: tags_key, .. } = &filters[1] else {
        panic!("{filters:?}")
    };
    assert_eq!(tags_key, "f1", "a key Raygun can't hold is renamed");
    let searched = |listing: crate::manga::MangaListing| -> Value {
        serde_json::from_str(&listing.items[0].title).unwrap()
    };
    let plain = host.manga_search(provider, "kw", 1).await.unwrap();
    assert_eq!(
        searched(plain),
        json!({ "kw": "kw", "page": 1, "filter": { "genre": [""], "tags list": ["all"] } }),
        "defaults, as Miru's search page sends"
    );
    let chosen = BTreeMap::from([
        ("genre".to_owned(), FilterValue::Select("drama".into())),
        (
            "f1".to_owned(),
            FilterValue::Tags {
                include: vec!["a".into(), "b".into(), "c".into()],
                exclude: Vec::new(),
            },
        ),
    ]);
    let filtered = host
        .manga_search_filtered(provider, "", 2, &chosen)
        .await
        .unwrap();
    assert_eq!(
        searched(filtered),
        json!({ "kw": "", "page": 2, "filter": { "genre": ["drama"], "tags list": ["b", "c"] } }),
        "at most `max` tags, the latest kept"
    );

    let details = host.manga_details(provider, "show-1a").await.unwrap();
    assert_eq!(details.title, "Show show-1a");
    assert_eq!(details.description.as_deref(), Some("About it"));
    // The studio is who made it (Raygun's directors), the cast as given; the source's own name
    // is never one of them.
    assert_eq!(details.artists, ["Studio Ghibli"]);
    assert_eq!(details.authors, ["Ann", "Bob"]);
    // Lines that list the same episodes become each episode's servers.
    let episodes = host.video_episodes(provider, "show-1a").await.unwrap();
    let names: Vec<(&str, Option<f64>)> = episodes
        .iter()
        .map(|episode| (episode.name.as_str(), episode.number))
        .collect();
    assert_eq!(
        names,
        [
            ("Episode 1", Some(1.0)),
            ("Episode 2", Some(2.0)),
            ("Episode 3", Some(3.0))
        ]
    );
    let servers = host.video_servers(provider, "l1e1").await.unwrap();
    let servers: Vec<(&str, &str)> = servers
        .iter()
        .map(|server| (server.id.as_str(), server.name.as_str()))
        .collect();
    assert_eq!(servers, [("l1e1", "Line 1"), ("l2e1", "Line 2")]);

    let hls = host.video_links(provider, "l1e1").await.unwrap();
    assert_eq!(hls[0].format.as_deref(), Some("hls"));
    assert_eq!(hls[0].subtitles[0].lang.as_deref(), Some("en"));
    assert_eq!(hls[0].subtitles[0].label.as_deref(), Some("English"));
    let mp4 = host.video_links(provider, "l1e2").await.unwrap();
    assert_eq!(mp4[0].format, None);
    assert_eq!(
        mp4[0].headers,
        BTreeMap::from([("Referer".to_owned(), format!("http://{address}/"))]),
        "a script's own headers replace Miru's"
    );
    let torrent = host.video_links(provider, "l2e3").await.unwrap();
    assert_eq!(torrent[0].format.as_deref(), Some(crate::video::TORRENT));
    assert_eq!(torrent[0].url, "magnet:?xt=urn:btih:abc");
}

const MANGA: &str = r#"
  async latest(page) { return page > 1 ? [] : [{ title: 'Comic', url: '/comic/1', cover: 'http://img.example/c.png' }]; }
  async detail(url) {
    return { title: 'Comic One', episodes: [{ title: 'Chapters', urls: [{ name: 'Chapter 1', url: 'c1' }, { name: 'Chapter 2.5', url: 'c2' }] }] };
  }
  async watch(url) {
    await this.request('/api/json');
    return { urls: [`http://img.example/${url}/1.jpg`, `http://img.example/${url}/2.jpg`] };
  }
"#;

const NOVEL: &str = r#"
  async latest() { return [{ title: 'Book', url: 'b1' }]; }
  async detail(url) {
    return { title: 'The Book', episodes: [
      { title: 'Volume 1', urls: [{ name: 'Prologue', url: 'p' }, { name: 'Interlude', url: 'i' }] },
      { title: 'Volume 2', urls: [{ name: 'Finale', url: 'f' }] },
    ] };
  }
  async watch(url) {
    return { title: 'Prologue', subtitle: 'Part one',
      content: ['First line\nsecond line', '<b>not bold</b>', 'https://img.example/plate.png'] };
  }
"#;

#[tokio::test]
async fn manga_and_novel_sources() {
    let address = serve();
    let host = host_with(
        vec![
            source(&script("comic", "manga", address, MANGA), &["127.0.0.1"]),
            source(&script("book", "fikushon", address, NOVEL), &["127.0.0.1"]),
        ],
        UserSettings::new(),
    )
    .await;
    let comic = "miru.comic/source";
    let chapters = host.manga_chapters(comic, "/comic/1").await.unwrap();
    let numbers: Vec<Option<f64>> = chapters.iter().map(|chapter| chapter.number).collect();
    assert_eq!(numbers, [Some(1.0), Some(2.5)], "numbers from the names");
    let pages = host.manga_pages(comic, "c1").await.unwrap();
    assert_eq!(pages.len(), 2);
    assert_eq!(pages[0].url, "http://img.example/c1/1.jpg");
    assert_eq!(
        pages[0].headers.get("Referer").map(String::as_str),
        Some(format!("http://{address}/api/json").as_str())
    );
    let details = host.manga_details(comic, "/comic/1").await.unwrap();
    assert_eq!(
        details.url.as_deref(),
        Some(format!("http://{address}/comic/1").as_str()),
        "a site path opens on the site"
    );

    let book = "miru.book/source";
    let chapters = host.manga_chapters(book, "b1").await.unwrap();
    let listed: Vec<(&str, Option<f64>, Option<&str>)> = chapters
        .iter()
        .map(|chapter| {
            (
                chapter.name.as_str(),
                chapter.number,
                chapter.scanlator.as_deref(),
            )
        })
        .collect();
    assert_eq!(
        listed,
        [
            ("Prologue", Some(1.0), Some("Volume 1")),
            ("Interlude", Some(2.0), Some("Volume 1")),
            ("Finale", Some(3.0), Some("Volume 2")),
        ],
        "names without numbers keep the list's order"
    );
    let passage = host.novel_passage(book, "p").await.unwrap();
    assert_eq!(
        passage,
        [
            PassageBlock::Heading("Part one".into()),
            PassageBlock::Paragraph("First line".into()),
            PassageBlock::Paragraph("second line".into()),
            PassageBlock::Paragraph("<b>not bold</b>".into()),
            PassageBlock::Image("https://img.example/plate.png".into()),
        ]
    );
}

#[tokio::test]
async fn slow_or_failing_setup_never_blocks_loading() {
    let address = serve();
    let slow = script(
        "slow",
        "manga",
        address,
        "async load() { await new Promise((resolve) => setTimeout(resolve, 1500)); this.ready = true; }\n  async latest() { return [{ title: this.ready ? 'ready' : 'early', url: 'x' }]; }",
    );
    let failing = script(
        "failing",
        "manga",
        address,
        "async load() { throw new Error('site down'); }\n  async latest() { return [{ title: 'still works', url: 'x' }]; }",
    );
    let started = Instant::now();
    let host = host_with(
        vec![
            source(&slow, &["127.0.0.1"]),
            source(&failing, &["127.0.0.1"]),
        ],
        UserSettings::new(),
    )
    .await;
    assert!(
        started.elapsed() < Duration::from_millis(1200),
        "{:?}",
        started.elapsed()
    );
    let slow = host
        .manga_list("miru.slow/source", MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(slow.items[0].title, "ready", "calls wait for the setup");
    let failing = host
        .manga_list("miru.failing/source", MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(failing.items[0].title, "still works");
}

#[tokio::test]
async fn missing_methods_and_broken_scripts_say_what_happened() {
    let address = serve();
    let bare = script(
        "bare",
        "bangumi",
        address,
        "async latest() { return [{ title: 'Only', url: 'x' }]; }",
    );
    let host = host_with(vec![source(&bare, &["127.0.0.1"])], UserSettings::new()).await;
    let provider = "miru.bare/source";
    assert_eq!(host.manga_filters(provider).await.unwrap(), Vec::new());
    let error = host.manga_search(provider, "x", 1).await.unwrap_err();
    assert_eq!(error, "This source can't search");
    let error = host.video_episodes(provider, "x").await.unwrap_err();
    assert_eq!(error, "This source can't open titles");
    assert_eq!(
        host.call(provider, "implemented", json!({})).await.unwrap(),
        json!(["latest"])
    );

    let no_class = script("noclass", "manga", address, "")
        .replace("export default class extends Extension {", "var x = {");
    let syntax = script("syntax", "manga", address, "async latest( {");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load(vec![
            source(&no_class, &["127.0.0.1"]),
            source(&syntax, &["127.0.0.1"]),
        ])
        .await;
    let error = |id: &str| {
        infos
            .iter()
            .find(|info| info.id == id)
            .and_then(|info| info.error.clone())
            .unwrap_or_default()
    };
    assert!(
        error("miru.noclass").contains("defines no source"),
        "{}",
        error("miru.noclass")
    );
    assert!(
        error("miru.syntax").contains("The Miru script stopped with an error")
            && error("miru.syntax").contains("Unexpected token"),
        "{}",
        error("miru.syntax")
    );
}

#[tokio::test]
async fn local_addresses_stay_out_of_reach() {
    let address = serve();
    let body = r#"
  async latest() {
    try { await this.request('/api/json'); return [{ title: 'reached', url: 'x' }]; }
    catch (error) { return [{ title: error.message, url: 'x' }]; }
  }"#;
    // With `*` alone, as installed: this machine isn't a public host.
    let host = host_with(
        vec![source(&script("local", "manga", address, body), &["*"])],
        UserSettings::new(),
    )
    .await;
    let listing = host
        .manga_list("miru.local/source", MangaList::Popular, 1)
        .await
        .unwrap();
    assert!(
        listing.items[0].title.contains("not permitted"),
        "{}",
        listing.items[0].title
    );
}

// ---- Real repositories (ignored: they read a checkout or the network) ----

/// Where miru-repo is checked out: `MIRU_REPO`, else the git-ignored `reference/miru-repo`.
fn miru_repo() -> std::path::PathBuf {
    std::env::var_os("MIRU_REPO").map_or_else(
        || std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../reference/miru-repo"),
        std::path::PathBuf::from,
    )
}

/// `MIRU_REPO=<checkout> cargo test -p raygun-ext-host -- --ignored miru_survey --nocapture`:
/// every script of a miru-repo checkout installed as the app installs it, loaded with the network
/// shut (setups that fetch fail, as offline), and asked which of Miru's methods it has. Prints
/// counts and each failure instead of stopping at the first.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "reads a miru-repo checkout (git-ignored reference/)"]
async fn miru_survey() {
    let root = miru_repo();
    let index = std::fs::read_to_string(root.join("index.json"))
        .unwrap_or_else(|error| panic!("{}: {error}", root.display()));
    let entries: Vec<Value> = serde_json::from_str(&index).unwrap();
    let parsed = crate::repo::parse_index(&index).unwrap();
    eprintln!(
        "index: {} entries, {} usable",
        entries.len(),
        parsed.extensions.len()
    );
    let temp = tempfile::TempDir::new().unwrap();
    let mut report: Vec<(String, &str, String)> = Vec::new();
    let mut installed = Vec::new();
    for entry in &parsed.extensions {
        let file = entry.package.split('#').next().unwrap_or_default();
        let bytes = match std::fs::read(root.join(file)) {
            Ok(bytes) => bytes,
            Err(error) => {
                report.push((entry.id.clone(), "read", error.to_string()));
                continue;
            }
        };
        let package = crate::miru::package::verify_script(&bytes, entry)
            .and_then(|(code, header)| crate::miru::package::package(entry, code, &header));
        match package {
            // Offline: only this machine, so nothing leaves it.
            Ok(package) => match crate::package::install(temp.path(), &package) {
                Ok(dir) => {
                    let manifest = dir.join(crate::MANIFEST_FILE);
                    let text = std::fs::read_to_string(&manifest).unwrap();
                    let mut json: Value = serde_json::from_str(&text).unwrap();
                    json["permissions"]["net"] = json!(["127.0.0.1"]);
                    std::fs::write(&manifest, json.to_string()).unwrap();
                    installed.push(entry.clone());
                }
                Err(error) => report.push((entry.id.clone(), "install", error.to_string())),
            },
            Err(error) => report.push((entry.id.clone(), "install", error.to_string())),
        }
    }
    let mut kinds: BTreeMap<String, usize> = BTreeMap::new();
    let mut methods: BTreeMap<String, usize> = BTreeMap::new();
    let mut loaded = 0;
    // A few at a time: each is its own QuickJS runtime.
    for batch in installed.chunks(20) {
        let host = ExtensionHost::start(HostConfig::default()).unwrap();
        let sources = batch
            .iter()
            .map(|entry| ExtensionSource::installed(temp.path().join(&entry.id)))
            .collect();
        let infos = host.load(sources).await;
        for info in infos {
            if info.status != ExtensionStatus::Loaded {
                report.push((info.id, "load", info.error.unwrap_or_default()));
                continue;
            }
            let Some(provider) = info.providers.first() else {
                report.push((info.id, "load", "no provider".into()));
                continue;
            };
            *kinds.entry(provider.kind.clone()).or_default() += 1;
            match host.call(&provider.id, "implemented", json!({})).await {
                Ok(Value::Array(names)) => {
                    loaded += 1;
                    for name in names.iter().filter_map(Value::as_str) {
                        *methods.entry(name.to_owned()).or_default() += 1;
                    }
                    let names: Vec<&str> = names.iter().filter_map(Value::as_str).collect();
                    if !names.contains(&"detail") || !names.contains(&"watch") {
                        report.push((info.id, "methods", format!("only {names:?}")));
                    }
                }
                other => report.push((info.id, "methods", format!("{other:?}"))),
            }
        }
    }
    let web_view = installed
        .iter()
        .filter(|entry| {
            let file = entry.package.split('#').next().unwrap_or_default();
            std::fs::read_to_string(root.join(file))
                .is_ok_and(|code| crate::miru::package::needs_web_view(&code))
        })
        .count();
    eprintln!(
        "{} installed, {loaded} loaded and answered; by kind {kinds:?}",
        installed.len()
    );
    eprintln!("methods: {methods:?}");
    eprintln!("{web_view} ask for a web view (bot checks), which Raygun doesn't open");
    for (id, stage, error) in &report {
        let line: String = error.chars().take(160).collect();
        eprintln!("{stage:>8}  {id}  {line}");
    }
    assert!(
        loaded * 10 >= installed.len() * 9,
        "most scripts load: {loaded} of {}",
        installed.len()
    );
}

/// The legal sources of miru-repo, on the network: `cargo test -p raygun-ext-host -- --ignored
/// miru_live --nocapture`. Scripts come from `MIRU_REPO` (or reference/miru-repo), else from
/// miru-project/repo on GitHub; each lists, opens a title and opens an episode or chapter.
/// Installs miru-repo scripts by id (from `MIRU_REPO` or reference/miru-repo, else GitHub) into a
/// temporary folder; returns it and the ids.
async fn install_live(wanted: &str) -> (tempfile::TempDir, Vec<String>) {
    const RAW: &str = "https://raw.githubusercontent.com/miru-project/repo/main/";
    let root = miru_repo();
    let client = raygun_net::http_client().unwrap();
    let read = |file: String| {
        let root = root.clone();
        let client = client.clone();
        async move {
            if let Ok(bytes) = std::fs::read(root.join(&file)) {
                return bytes;
            }
            client
                .get(format!("{RAW}{file}"))
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .to_vec()
        }
    };
    let index = String::from_utf8(read("index.json".into()).await).unwrap();
    let index = crate::repo::parse_index(&index).unwrap();
    let temp = tempfile::TempDir::new().unwrap();
    let mut ids = Vec::new();
    for id in wanted.split(',') {
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("{id} isn't in the index"));
        let file = entry
            .package
            .split('#')
            .next()
            .unwrap_or_default()
            .to_owned();
        let (code, header) = crate::miru::package::verify_script(&read(file).await, entry).unwrap();
        let package = crate::miru::package::package(entry, code, &header).unwrap();
        crate::package::install(temp.path(), &package).unwrap();
        ids.push(entry.id.clone());
    }
    (temp, ids)
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to the sources' sites"]
async fn miru_live() {
    // Also legal, failing on their sites' side (2026-11): miru.mangadex.org (MangaDex answers a
    // browser User-Agent from a non-browser with a bot check, and the script names none) and
    // miru.mangacross.jp (its site answers 403). Ask for them with MIRU_SOURCES.
    let wanted = std::env::var("MIRU_SOURCES")
        .unwrap_or_else(|_| "miru.de.ccc.media,miru.royalroad.com,miru.iptv-org".into());
    let (temp, ids) = install_live(&wanted).await;
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(crate::discover_installed(temp.path())).await;
    let mut failures = Vec::new();
    for info in &infos {
        if info.status != ExtensionStatus::Loaded {
            failures.push(format!("{}: {:?}", info.id, info.error));
            continue;
        }
        let provider = &info.providers[0];
        let checked: Result<String, String> = async {
            let listing = host.manga_list(&provider.id, MangaList::Popular, 1).await?;
            let first = listing.items.first().ok_or("the listing is empty")?.clone();
            let details = host.manga_details(&provider.id, &first.id).await?;
            let summary = match provider.kind.as_str() {
                "video.source" => {
                    let episodes = host.video_episodes(&provider.id, &first.id).await?;
                    let episode = episodes.first().ok_or("no episodes")?;
                    let servers = host.video_servers(&provider.id, &episode.id).await?;
                    let links = host.video_links(&provider.id, &servers[0].id).await?;
                    let link = links.first().ok_or("no video")?;
                    format!(
                        "{} episodes, {} servers, plays {} ({:?})",
                        episodes.len(),
                        servers.len(),
                        link.url,
                        link.format
                    )
                }
                "manga.source" => {
                    let chapters = host.manga_chapters(&provider.id, &first.id).await?;
                    let chapter = chapters.first().ok_or("no chapters")?;
                    let pages = host.manga_pages(&provider.id, &chapter.id).await?;
                    format!(
                        "{} chapters, {} pages in {}",
                        chapters.len(),
                        pages.len(),
                        chapter.name
                    )
                }
                _ => {
                    let chapters = host.manga_chapters(&provider.id, &first.id).await?;
                    let chapter = chapters.first().ok_or("no chapters")?;
                    let passage = host.novel_passage(&provider.id, &chapter.id).await?;
                    format!(
                        "{} chapters, {} blocks in {}",
                        chapters.len(),
                        passage.len(),
                        chapter.name
                    )
                }
            };
            Ok(format!(
                "{} titles; {:?}: {summary}",
                listing.items.len(),
                details.title
            ))
        }
        .await;
        match checked {
            Ok(summary) => eprintln!("{}: {summary}", info.id),
            Err(error) => failures.push(format!("{}: {error}", info.id)),
        }
    }
    assert_eq!(infos.len(), ids.len());
    assert!(failures.is_empty(), "{failures:#?}");
}

/// Searching legal Miru sources the way Search's rows do: `cargo test -p raygun-ext-host --
/// --ignored miru_search_live --nocapture`. Each query names titles the source lists.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to the sources' sites"]
async fn miru_search_live() {
    let cases = [
        ("miru.de.ccc.media", "congress"),
        ("miru.iptv-org", "news"),
        ("miru.royalroad.com", "dungeon"),
    ];
    let wanted: Vec<&str> = cases.iter().map(|(id, _)| *id).collect();
    let (temp, _) = install_live(&wanted.join(",")).await;
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(crate::discover_installed(temp.path())).await;
    let mut failures = Vec::new();
    for (id, query) in cases {
        let info = infos.iter().find(|info| info.id == id).unwrap();
        let provider = &info.providers[0].id;
        let started = Instant::now();
        match host.catalog_search_row(provider, query).await {
            Ok(listing) => {
                eprintln!(
                    "{id} {query:?}: {} in {:?}: {:?}",
                    listing.items.len(),
                    started.elapsed(),
                    listing
                        .items
                        .iter()
                        .take(3)
                        .map(|item| &item.title)
                        .collect::<Vec<_>>()
                );
                if listing.items.is_empty() {
                    failures.push(format!("{id}: nothing for {query:?}"));
                }
            }
            Err(error) => failures.push(format!("{id}: {error}")),
        }
    }
    assert!(failures.is_empty(), "{failures:#?}");
}
