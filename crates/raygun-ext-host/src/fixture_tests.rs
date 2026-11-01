//! First-party extensions against local fixture servers (M15): the real bundles, with their API
//! base rewritten to point at a server on this machine.

use std::collections::HashMap;
use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use serde_json::json;

use crate::{ExtensionHost, ExtensionSource, HostConfig};

fn query_params(uri: &Uri) -> HashMap<String, String> {
    uri.query()
        .unwrap_or_default()
        .split('&')
        .filter_map(|pair| pair.split_once('='))
        .map(|(key, value)| {
            let value = percent_decode(&value.replace('+', " "));
            (key.to_owned(), value)
        })
        .collect()
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(byte) = u8::from_str_radix(&text[index + 1..index + 3], 16)
        {
            out.push(byte);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn json_response(status: StatusCode, body: serde_json::Value) -> Response {
    (
        status,
        [(header::CONTENT_TYPE, "application/json")],
        body.to_string(),
    )
        .into_response()
}

/// Serves `app` on a free local port from its own thread.
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

/// A first-party extension whose `api_base` now points at `address`, with network access
/// limited to that address.
fn patched_extension(name: &str, api_base: &str, address: SocketAddr) -> ExtensionSource {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions")
        .join(name);
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("raygun-extension.json")).unwrap())
            .unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    let code = std::fs::read_to_string(dir.join("dist/index.js")).unwrap();
    assert!(
        code.contains(api_base),
        "{name} bundle doesn't use {api_base}"
    );
    let code = code.replace(api_base, &format!("http://{address}"));
    ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest.to_string()),
        ("dist/index.js", &code),
    ])
}

fn query(title: &str, duration_ms: Option<u32>) -> raygun_model::TrackQuery {
    raygun_model::TrackQuery {
        title: title.into(),
        artist: Some("Band".into()),
        album: None,
        duration_ms,
    }
}

#[tokio::test]
async fn lrclib_finds_exact_and_searched_lyrics() {
    let app = Router::new()
        .route(
            "/api/get",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                match params.get("track_name").map(String::as_str) {
                    Some("Exact Song") => json_response(
                        StatusCode::OK,
                        json!({
                            "trackName": "Exact Song", "artistName": "Band", "duration": 200,
                            "instrumental": false, "plainLyrics": "Line one\nChorus",
                            // Two stamps on one line, a metadata tag, CRLF endings.
                            "syncedLyrics": "[ar:Band]\r\n[00:01.50]Line one\r\n[00:03.00][00:10.25] Chorus \r\n",
                        }),
                    ),
                    Some("Instrumental") => json_response(
                        StatusCode::OK,
                        json!({ "trackName": "Instrumental", "artistName": "Band", "instrumental": true }),
                    ),
                    Some("Plain Only") => json_response(
                        StatusCode::OK,
                        json!({ "trackName": "Plain Only", "artistName": "Band", "duration": 200,
                                "plainLyrics": "just words", "syncedLyrics": null }),
                    ),
                    _ => json_response(StatusCode::NOT_FOUND, json!({ "message": "not found" })),
                }
            }),
        )
        .route(
            "/api/search",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                if params.get("track_name").map(String::as_str) == Some("Unknown") {
                    return json_response(StatusCode::OK, json!([]));
                }
                json_response(
                    StatusCode::OK,
                    json!([
                        { "trackName": "x", "artistName": "Band", "duration": 300, "syncedLyrics": "[00:00.00]too long" },
                        { "trackName": "x", "artistName": "Band", "duration": 199, "syncedLyrics": null, "plainLyrics": "plain twin" },
                        { "trackName": "x", "artistName": "Band", "duration": 203, "syncedLyrics": "[00:02.00]closest synced" },
                    ]),
                )
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension(
        "lrclib",
        "https://lrclib.net",
        address,
    )])
    .await;

    let exact = host
        .lyrics(&query("Exact Song", Some(200_000)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(exact.provider_name, "LRCLIB");
    let lines: Vec<_> = exact
        .synced
        .iter()
        .map(|line| (line.time_ms, line.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        vec![(1_500, "Line one"), (3_000, "Chorus"), (10_250, "Chorus")]
    );

    assert_eq!(
        host.lyrics(&query("Instrumental", None)).await,
        Ok(None),
        "instrumentals have no lyrics"
    );

    // No exact match: search, keep synced over plain, closest length within 10 s.
    let searched = host
        .lyrics(&query("Search Song", Some(200_000)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(searched.synced[0].text, "closest synced");

    // An exact match without timings prefers a synced twin from search.
    let plain = host
        .lyrics(&query("Plain Only", Some(200_000)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(plain.synced[0].text, "closest synced");

    assert_eq!(
        host.lyrics(&query("Unknown", Some(200_000))).await,
        Ok(None)
    );
}

fn youtube_app() -> Router {
    let key_error = |params: &HashMap<String, String>| -> Option<Response> {
        match params.get("key").map(String::as_str) {
            Some("test-key") => None,
            Some("quota-key") => Some(json_response(
                StatusCode::FORBIDDEN,
                json!({ "error": { "code": 403, "message": "quota", "errors": [{ "reason": "quotaExceeded" }] } }),
            )),
            _ => Some(json_response(
                StatusCode::BAD_REQUEST,
                json!({ "error": { "code": 400, "message": "API key not valid", "errors": [{ "reason": "badRequest" }] } }),
            )),
        }
    };
    Router::new()
        .route(
            "/youtube/v3/search",
            get(move |uri: Uri| async move {
                let params = query_params(&uri);
                if let Some(error) = key_error(&params) {
                    return error;
                }
                assert_eq!(params["videoCategoryId"], "10");
                assert_eq!(params["type"], "video");
                json_response(
                    StatusCode::OK,
                    json!({ "items": [
                        { "id": { "videoId": "abc123def45" },
                          "snippet": { "title": "Band &amp; Friends - Song (Official Audio)", "channelTitle": "Band - Topic",
                                       "thumbnails": { "high": { "url": "https://i.ytimg.com/vi/abc123def45/hqdefault.jpg" } } } },
                        { "id": { "channelId": "not-a-video" }, "snippet": { "title": "Channel", "channelTitle": "x" } },
                        { "id": { "videoId": "zzz999yyy88" },
                          "snippet": { "title": "Song &#39;Live&#39;", "channelTitle": "Some Uploader" } },
                    ] }),
                )
            }),
        )
        .route(
            "/youtube/v3/videos",
            get(move |uri: Uri| async move {
                let params = query_params(&uri);
                if let Some(error) = key_error(&params) {
                    return error;
                }
                assert_eq!(params["id"], "abc123def45,zzz999yyy88");
                json_response(
                    StatusCode::OK,
                    json!({ "items": [
                        { "id": "abc123def45", "contentDetails": { "duration": "PT3M21S" } },
                        { "id": "zzz999yyy88", "contentDetails": { "duration": "PT1H2M3S" } },
                    ] }),
                )
            }),
        )
}

#[tokio::test]
async fn youtube_searches_and_offers_embed_streams_with_the_users_key() {
    let address = serve(youtube_app());
    let load = |key: &str| {
        let key = key.to_owned();
        async move {
            let host = ExtensionHost::start(HostConfig::default()).unwrap();
            let mut settings = crate::UserSettings::new();
            settings.insert(
                "dev.raygun.youtube".into(),
                [("apiKey".to_owned(), json!(key))].into(),
            );
            host.load_with_settings(
                vec![patched_extension(
                    "youtube",
                    "https://www.googleapis.com",
                    address,
                )],
                settings,
            )
            .await;
            host
        }
    };

    // Without a key, providers say what to do.
    let host = load("").await;
    let results = host.search_music("song", 10).await;
    assert!(
        results[0]
            .error
            .as_ref()
            .unwrap()
            .contains("Add your YouTube Data API key")
    );

    let host = load("test-key").await;
    let results = host.search_music("song", 10).await;
    assert_eq!(results[0].error, None);
    let tracks = &results[0].tracks;
    assert_eq!(tracks.len(), 2, "channels are skipped");
    assert_eq!(tracks[0].title, "Band & Friends - Song (Official Audio)");
    assert_eq!(tracks[0].artist.as_deref(), Some("Band"));
    assert_eq!(tracks[0].duration_ms, Some(201_000));
    assert_eq!(
        tracks[0].source,
        Some(raygun_model::MediaSource::YouTube {
            video_id: "abc123def45".into()
        })
    );
    assert_eq!(tracks[1].title, "Song 'Live'");
    assert_eq!(tracks[1].duration_ms, Some(3_723_000));

    let candidates = host
        .stream_candidates(&query("Song", Some(201_000)), 5)
        .await;
    let candidates = candidates[0].result.as_ref().unwrap();
    assert_eq!(candidates[0].id, "abc123def45");
    assert_eq!(candidates[0].quality.as_deref(), Some("YouTube"));

    // API errors become readable messages.
    let host = load("bad-key").await;
    assert!(
        host.search_music("song", 10).await[0]
            .error
            .as_ref()
            .unwrap()
            .contains("rejected the API key")
    );
    let host = load("quota-key").await;
    assert!(
        host.search_music("song", 10).await[0]
            .error
            .as_ref()
            .unwrap()
            .contains("quota is used up")
    );
}

#[test]
fn fixture_helpers_decode_queries() {
    let uri: Uri = "/api/get?track_name=Hello%2C%20Dolly&artist_name=A+B"
        .parse()
        .unwrap();
    let params = query_params(&uri);
    assert_eq!(params["track_name"], "Hello, Dolly");
    assert_eq!(params["artist_name"], "A B");
}

#[tokio::test]
async fn mangadex_lists_titles_chapters_and_pages() {
    let manga = |id: &str, title: serde_json::Value, alt: serde_json::Value| {
        json!({
            "id": id,
            "attributes": {
                "title": title, "altTitles": alt,
                "description": { "en": "  An elf mage outlives her party.  " },
                "status": "ongoing",
                "tags": [
                    { "attributes": { "name": { "en": "Fantasy" }, "group": "genre" } },
                    { "attributes": { "name": { "en": "Long Strip" }, "group": "format" } },
                ],
            },
            "relationships": [
                { "id": "c1", "type": "cover_art", "attributes": { "fileName": "cover.jpg" } },
                { "id": "a1", "type": "author", "attributes": { "name": "Yamada" } },
                { "id": "a2", "type": "artist", "attributes": { "name": "Abe" } },
            ],
        })
    };
    let app = Router::new()
        .route(
            "/manga",
            get(move |uri: Uri| async move {
                let params = query_params(&uri);
                let searched = params.get("title").cloned();
                assert_eq!(params.get("availableTranslatedLanguage%5B%5D").map(String::as_str), Some("en"));
                let items = match searched.as_deref() {
                    Some("nothing") => vec![],
                    _ => vec![
                        manga("m1", json!({ "ja-ro": "Sousou no Frieren" }), json!([{ "en": "Frieren" }])),
                        manga("m2", json!({ "en": "Plain English" }), json!([])),
                    ],
                };
                let offset: usize = params.get("offset").and_then(|offset| offset.parse().ok()).unwrap_or(0);
                json_response(StatusCode::OK, json!({ "result": "ok", "data": items, "total": if items.is_empty() { 0 } else { 30 }, "offset": offset, "limit": 24 }))
            }),
        )
        .route(
            "/manga/m1",
            get(move || async move {
                json_response(StatusCode::OK, json!({ "result": "ok", "data": manga("m1", json!({ "ja-ro": "Sousou no Frieren" }), json!([{ "en": "Frieren" }])) }))
            }),
        )
        .route(
            "/manga/m1/feed",
            get(|| async {
                let chapter = |id: &str, number: Option<&str>, pages: u32, external: Option<&str>| {
                    json!({
                        "id": id,
                        "attributes": { "volume": "1", "chapter": number, "title": if id == "ch2" { "The Journey" } else { "" },
                                        "publishAt": "2024-01-02T00:00:00+00:00", "pages": pages, "externalUrl": external },
                        "relationships": [{ "id": "g", "type": "scanlation_group", "attributes": { "name": "Group" } }],
                    })
                };
                json_response(StatusCode::OK, json!({
                    "result": "ok", "total": 4, "offset": 0, "limit": 500,
                    "data": [
                        chapter("ch2", Some("2"), 18, None),
                        chapter("ch1", Some("1"), 20, None),
                        chapter("ext", Some("3"), 0, Some("https://mangaplus.example/3")),
                        chapter("one", None, 12, None),
                    ],
                }))
            }),
        )
        .route(
            "/at-home/server/ch1",
            get(|| async {
                json_response(StatusCode::OK, json!({
                    "result": "ok", "baseUrl": "https://node.mangadex.network",
                    "chapter": { "hash": "abc", "data": ["1.png", "2.png"], "dataSaver": ["1.jpg", "2.jpg"] },
                }))
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension(
        "mangadex",
        "https://api.mangadex.org",
        address,
    )])
    .await;
    let provider = "dev.raygun.mangadex/mangadex";
    assert_eq!(host.providers(crate::manga::MANGA_SOURCE).await.len(), 1);

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert!(popular.has_next_page, "2 of 30 shown");
    assert_eq!(
        popular.items[0].title, "Frieren",
        "the English alternative title wins"
    );
    assert_eq!(popular.items[1].title, "Plain English");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://uploads.mangadex.org/covers/m1/cover.jpg.512.jpg")
    );
    assert!(
        popular.items[0].cover_headers["User-Agent"].starts_with("Raygun/"),
        "MangaDex challenges browser-like clients, covers included"
    );
    let none = host.manga_search(provider, "nothing", 2).await.unwrap();
    assert!(none.items.is_empty() && !none.has_next_page);

    let details = host.manga_details(provider, "m1").await.unwrap();
    assert_eq!(details.authors, ["Yamada"]);
    assert_eq!(details.artists, ["Abe"]);
    assert_eq!(details.genres, ["Fantasy"], "format tags aren't genres");
    assert_eq!(
        details.description.as_deref(),
        Some("An elf mage outlives her party.")
    );
    assert_eq!(details.status, "ongoing");

    let chapters = host.manga_chapters(provider, "m1").await.unwrap();
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Chapter 2: The Journey", "Chapter 1", "Oneshot"],
        "external chapters are left out"
    );
    assert_eq!(chapters[1].number, Some(1.0));
    assert_eq!(chapters[2].number, None);
    assert_eq!(chapters[0].scanlator.as_deref(), Some("Group"));
    assert_eq!(chapters[0].date_upload, Some(1_704_153_600_000));

    let pages = host.manga_pages(provider, "ch1").await.unwrap();
    let urls: Vec<_> = pages.iter().map(|page| page.url.as_str()).collect();
    assert!(
        pages
            .iter()
            .all(|page| page.headers["User-Agent"].starts_with("Raygun/"))
    );
    assert_eq!(
        urls,
        [
            "https://node.mangadex.network/data/abc/1.png",
            "https://node.mangadex.network/data/abc/2.png"
        ]
    );
}

#[tokio::test]
async fn gutenberg_lists_books_splits_chapters_and_reads_passages() {
    let filler =
        "It was on a dreary night of November that I beheld the accomplishment of my toils. "
            .repeat(3);
    let html = format!(
        "<html><body><h1>Frankenstein</h1><p>Title page</p>\
         <p>*** START OF THE PROJECT GUTENBERG EBOOK FRANKENSTEIN ***</p>\
         <h2>CONTENTS</h2><p>{filler}</p>\
         <h2><a id='l1'></a>Letter 1<span class='pagenum'>{{1}}</span></h2><p>{filler}</p>\
         <img src='images/fig.jpg'><script>steal()</script>\
         <h2>Chapter 1</h2><p>Short</p><p>{filler}</p>\
         <p>*** END OF THE PROJECT GUTENBERG EBOOK FRANKENSTEIN ***</p>\
         <h2>Licence</h2><p>{filler}</p></body></html>"
    );
    let book_requests = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let counted = book_requests.clone();
    let app = Router::new()
        .route(
            "/ebooks/search.opds/",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                let entries = match params.get("query").map(String::as_str) {
                    Some("nothing") => String::new(),
                    _ => "<entry><id>https://www.gutenberg.org/ebooks/authors/search.opds/?query=x</id><title>Authors</title></entry>\
                          <entry><id>https://www.gutenberg.org/ebooks/84.opds</id><title>Frankenstein &amp; more</title></entry>"
                        .to_owned(),
                };
                let start = params.get("start_index").cloned().unwrap_or_default();
                let sort = params.get("sort_order").cloned().unwrap_or_default();
                ([(header::CONTENT_TYPE, "application/atom+xml")], format!("<feed sort='{sort}' start='{start}'>{entries}</feed>")).into_response()
            }),
        )
        .route(
            "/ebooks/84.opds",
            get(|| async {
                let xml = r#"<feed><title>Frankenstein by Mary</title><entry>
                    <title>Frankenstein; or, The modern Prometheus</title>
                    <content type="xhtml"><div><p>Summary: A scientist builds a creature. (This is an automatically generated summary.)</p></div></content>
                    <author><name>Shelley, Mary Wollstonecraft</name></author>
                    <category scheme="http://purl.org/dc/terms/LCSH" term="Gothic fiction"/><category scheme="http://purl.org/dc/terms/LCSH" term="Monsters -- Fiction"/><category scheme="http://purl.org/dc/terms/LCSH" term="Gothic fiction"/><category scheme="http://purl.org/dc/terms/LCC" term="PR"/>
                    </entry></feed>"#;
                ([(header::CONTENT_TYPE, "application/atom+xml")], xml).into_response()
            }),
        )
        .route(
            "/ebooks/84.html.images",
            get(move || {
                counted.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
                async move { ([(header::CONTENT_TYPE, "text/html")], html).into_response() }
            }),
        );
    let address = serve(app);

    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions/gutenberg");
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("raygun-extension.json")).unwrap())
            .unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    let code = std::fs::read_to_string(dir.join("dist/index.js"))
        .unwrap()
        .replace("https://www.gutenberg.org", &format!("http://{address}"));
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest.to_string()),
        ("dist/index.js", &code),
    ])])
    .await;
    let provider = "dev.raygun.gutenberg/gutenberg";
    assert_eq!(host.providers(crate::novel::NOVEL_SOURCE).await.len(), 1);

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(popular.items.len(), 1, "author entries are skipped");
    assert_eq!(popular.items[0].id, "84");
    assert_eq!(popular.items[0].title, "Frankenstein & more");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/cache/epub/84/pg84.cover.medium.jpg").as_str())
    );
    assert!(!popular.has_next_page);
    let none = host.manga_search(provider, "nothing", 2).await.unwrap();
    assert!(none.items.is_empty());

    let details = host.manga_details(provider, "84").await.unwrap();
    assert_eq!(details.title, "Frankenstein; or, The modern Prometheus");
    assert_eq!(details.authors, ["Mary Wollstonecraft Shelley"]);
    assert_eq!(
        details.description.as_deref(),
        Some("A scientist builds a creature.")
    );
    assert_eq!(details.genres, ["Gothic fiction", "Monsters"]);
    assert_eq!(details.status, "completed");

    let chapters = host.manga_chapters(provider, "84").await.unwrap();
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(
        names,
        ["Letter 1", "Chapter 1"],
        "no contents, licence or front matter"
    );
    assert_eq!(chapters[1].id, "84/1");
    assert_eq!(chapters[1].number, Some(2.0));

    let letter = host.novel_passage(provider, "84/0").await.unwrap();
    use crate::novel::PassageBlock;
    assert_eq!(letter[0], PassageBlock::Heading("Letter 1".into()));
    assert!(
        matches!(&letter[1], PassageBlock::Paragraph(text) if text.starts_with("It was on a dreary night"))
    );
    assert_eq!(
        letter[2],
        PassageBlock::Image(format!("http://{address}/ebooks/images/fig.jpg"))
    );
    assert_eq!(letter.len(), 3, "the script is gone");
    assert!(host.novel_passage(provider, "84/7").await.is_err());
    assert_eq!(
        book_requests.load(std::sync::atomic::Ordering::SeqCst),
        1,
        "the book is downloaded once for its chapters and passages"
    );
}
