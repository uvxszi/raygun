//! First-party extensions against local fixture servers (M15): the real bundles, with their API
//! base rewritten to point at a server on this machine.

use std::collections::{BTreeMap, HashMap};
use std::net::SocketAddr;
use std::path::Path;

use axum::Router;
use axum::http::{StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
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
    patched_extension_bases(name, &[api_base], address)
}

/// [`patched_extension`] for an extension that talks to more than one API.
fn patched_extension_bases(name: &str, api_bases: &[&str], address: SocketAddr) -> ExtensionSource {
    let dir = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../extensions")
        .join(name);
    let mut manifest: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("raygun-extension.json")).unwrap())
            .unwrap();
    manifest["permissions"]["net"] = json!(["127.0.0.1"]);
    let mut code = std::fs::read_to_string(dir.join("dist/index.js")).unwrap();
    for api_base in api_bases {
        assert!(
            code.contains(api_base),
            "{name} bundle doesn't use {api_base}"
        );
        code = code.replace(api_base, &format!("http://{address}"));
    }
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

/// KRC's XOR key, as KuGou's files use it.
const KRC_KEY: [u8; 16] = [
    64, 71, 97, 119, 94, 50, 116, 71, 81, 54, 49, 45, 206, 210, 110, 105,
];

/// A KRC file as KuGou's download sends it: `krc1`, then zlib data XORed with the key, all base64.
fn krc_content(text: &str) -> String {
    use base64::Engine;
    use std::io::Write;
    let mut encoder = flate2::write::ZlibEncoder::new(Vec::new(), flate2::Compression::default());
    encoder.write_all(text.as_bytes()).unwrap();
    let compressed = encoder.finish().unwrap();
    let mut bytes = b"krc1".to_vec();
    bytes.extend(
        compressed
            .iter()
            .enumerate()
            .map(|(index, byte)| byte ^ KRC_KEY[index % KRC_KEY.len()]),
    );
    base64::engine::general_purpose::STANDARD.encode(bytes)
}

#[tokio::test]
async fn kugou_reads_krc_with_translations_and_falls_back_to_lrc() {
    use base64::Engine;
    let base64 = |text: &str| base64::engine::general_purpose::STANDARD.encode(text);
    let language = base64(
        &json!({
            "content": [
                { "language": 0, "type": 1, "lyricContent": [[""], [""], ["你的名字"], ["奔向夜晚"]] },
                {
                    "language": 0, "type": 0,
                    "lyricContent": [["so", "ng"], [""], ["ki mi ", "no ", "na ma e"], ["yo ru ", "ni ", "ka ke ru"]]
                }
            ],
            "version": 1
        })
        .to_string(),
    );
    // The song's name and a credit first (both dropped), then two sung lines with word timings.
    let krc = format!(
        "[id:$00000000]\r\n[ar:Band]\r\n[ti:Song]\r\n[language:{language}]\r\n\
         [0,1000]<0,500,0>Song<500,500,0> - Band\r\n[1000,1000]<0,1000,0>作词：Someone\r\n\
         [2000,1500]<0,700,0>君の<700,800,0>名前\r\n[4000,1500]<0,700,0>夜に<700,800,0>駆ける\r\n"
    );
    let krc = krc_content(&krc);
    let lrc = base64("[00:01.00]Line one\n[00:02.50]Line two\n");
    let app = Router::new()
        .route(
            "/api/v3/search/song",
            get(|uri: Uri| async move {
                let keyword = query_params(&uri)
                    .get("keyword")
                    .cloned()
                    .unwrap_or_default();
                let hash = if keyword.contains("Broken") {
                    "h2"
                } else {
                    "h1"
                };
                json_response(
                    StatusCode::OK,
                    json!({ "data": { "info": [{ "hash": hash, "duration": 200 }] } }),
                )
            }),
        )
        .route(
            "/search",
            get(|uri: Uri| async move {
                let id = match query_params(&uri).get("hash").map(String::as_str) {
                    Some("h1") => 7,
                    _ => 8,
                };
                json_response(
                    StatusCode::OK,
                    json!({ "candidates": [{ "id": id, "accesskey": "k" }] }),
                )
            }),
        )
        .route(
            "/download",
            get(move |uri: Uri| {
                let (krc, lrc) = (krc.clone(), lrc.clone());
                async move {
                    let params = query_params(&uri);
                    let content = match (
                        params.get("id").map(String::as_str),
                        params.get("fmt").map(String::as_str),
                    ) {
                        (Some("7"), Some("krc")) => krc,
                        // A KRC that isn't one: the LRC is read instead.
                        (Some("8"), Some("krc")) => base64("not a krc file"),
                        _ => lrc,
                    };
                    json_response(StatusCode::OK, json!({ "content": content }))
                }
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension_bases(
        "kugou",
        &["https://krcs.kugou.com", "https://mobileservice.kugou.com"],
        address,
    )])
    .await;

    let lyrics = host
        .lyrics(&query("Song", Some(200_000)))
        .await
        .unwrap()
        .unwrap();
    assert_eq!(lyrics.provider_name, "KuGou");
    let lines: Vec<_> = lyrics
        .synced
        .iter()
        .map(|line| (line.time_ms, line.text.as_str()))
        .collect();
    assert_eq!(lines, [(2_000, "君の名前"), (4_000, "夜に駆ける")]);
    let translations: Vec<_> = lyrics
        .translations
        .iter()
        .map(|translation| {
            (
                translation.language.as_str(),
                translation.kind,
                translation.lines.clone(),
            )
        })
        .collect();
    assert_eq!(
        translations,
        [
            (
                "zh-Hans",
                raygun_model::LyricsTranslationKind::Translation,
                vec!["你的名字".to_owned(), "奔向夜晚".to_owned()]
            ),
            (
                "ja-Latn",
                raygun_model::LyricsTranslationKind::Romanization,
                vec![
                    "ki mi no na ma e".to_owned(),
                    "yo ru ni ka ke ru".to_owned()
                ]
            ),
        ]
    );

    let fallback = host
        .lyrics(&query("Broken", Some(200_000)))
        .await
        .unwrap()
        .unwrap();
    let texts: Vec<&str> = fallback
        .synced
        .iter()
        .map(|line| line.text.as_str())
        .collect();
    assert_eq!(texts, ["Line one", "Line two"]);
    assert!(fallback.translations.is_empty());
}

/// The web client version the fixture's music.youtube.com page names; others are refused.
const YOUTUBE_CLIENT: &str = "1.20990101.00.00";

/// A YouTube Music search result row, as InnerTube sends it.
fn youtube_music_row(video_id: &str, title: &str, details: serde_json::Value) -> serde_json::Value {
    json!({ "musicResponsiveListItemRenderer": {
        "playlistItemData": { "videoId": video_id },
        "thumbnail": { "musicThumbnailRenderer": { "thumbnail": { "thumbnails": [
            { "url": format!("https://lh3.googleusercontent.com/{video_id}=w60-h60-l90-rj"), "width": 60 },
            { "url": format!("https://lh3.googleusercontent.com/{video_id}=w120-h120-l90-rj"), "width": 120 },
        ] } } },
        "flexColumns": [
            { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": [{ "text": title }] } } },
            { "musicResponsiveListItemFlexColumnRenderer": { "text": { "runs": details } } },
        ],
    } })
}

fn music_page(text: &str, page_type: &str) -> serde_json::Value {
    json!({ "text": text, "navigationEndpoint": { "browseEndpoint": {
        "browseEndpointContextSupportedConfigs": {
            "browseEndpointContextMusicConfig": { "pageType": page_type },
        },
    } } })
}

fn youtube_music_search(headers: &axum::http::HeaderMap, body: &str) -> Response {
    let body: serde_json::Value = serde_json::from_str(body).unwrap();
    let client = &body["context"]["client"];
    if client["clientVersion"] != YOUTUBE_CLIENT
        || headers["x-youtube-client-version"] != YOUTUBE_CLIENT
    {
        return json_response(StatusCode::BAD_REQUEST, json!({ "error": { "code": 400 } }));
    }
    assert_eq!(client["clientName"], "WEB_REMIX");
    assert_eq!(client["gl"], "DE", "the region setting");
    assert_eq!(headers["x-youtube-client-name"], "67");
    let separator = || json!({ "text": " \u{2022} " });
    let rows = if body["params"] == "EgWKAQIIAWoKEAkQBRAKEAMQBA==" {
        vec![youtube_music_row(
            "abc123def45",
            "Song (feat. Guest)",
            json!([
                music_page("Band", "MUSIC_PAGE_TYPE_ARTIST"),
                { "text": " & " },
                music_page("Guest", "MUSIC_PAGE_TYPE_ARTIST"),
                separator(),
                music_page("The Album", "MUSIC_PAGE_TYPE_ALBUM"),
                separator(),
                { "text": "3:21" },
            ]),
        )]
    } else {
        vec![
            youtube_music_row(
                "abc123def45",
                "Song (Official Audio)",
                json!([music_page("Band", "MUSIC_PAGE_TYPE_ARTIST"), separator(), { "text": "3:21" }]),
            ),
            youtube_music_row(
                "zzz999yyy88",
                "Song live",
                json!([
                    music_page("Some Uploader - Topic", "MUSIC_PAGE_TYPE_USER_CHANNEL"),
                    separator(),
                    { "text": "58M views" },
                    separator(),
                    { "text": "1:02:03" },
                ]),
            ),
        ]
    };
    json_response(
        StatusCode::OK,
        json!({ "contents": { "tabbedSearchResultsRenderer": { "tabs": [{ "tabRenderer": {
            "content": { "sectionListRenderer": { "contents": [
                { "itemSectionRenderer": {} },
                { "musicShelfRenderer": { "contents": rows } },
            ] } },
        } }] } } }),
    )
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
            "/",
            get(|| async {
                format!(r#"<script>ytcfg.set({{"INNERTUBE_CLIENT_VERSION":"{YOUTUBE_CLIENT}"}})</script>"#)
            }),
        )
        .route(
            "/youtubei/v1/search",
            post(|headers: axum::http::HeaderMap, body: String| async move {
                youtube_music_search(&headers, &body)
            }),
        )
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
async fn youtube_searches_youtube_music_without_a_key_or_the_api_with_one() {
    let address = serve(youtube_app());
    let load = |key: &str| {
        let key = key.to_owned();
        async move {
            let host = ExtensionHost::start(HostConfig::default()).unwrap();
            let mut settings = crate::UserSettings::new();
            settings.insert(
                "dev.raygun.youtube".into(),
                [
                    ("apiKey".to_owned(), json!(key)),
                    ("region".to_owned(), json!("de")),
                ]
                .into(),
            );
            host.load_with_settings(
                vec![patched_extension_bases(
                    "youtube",
                    &["https://music.youtube.com", "https://www.googleapis.com"],
                    address,
                )],
                settings,
            )
            .await;
            host
        }
    };

    // No key: YouTube Music's own search, after reading the client version its page names.
    let host = load("").await;
    let results = host.search_music("song", 10).await;
    assert_eq!(results[0].error, None);
    let tracks = &results[0].tracks;
    assert_eq!(
        tracks.len(),
        2,
        "songs, then videos that aren't already there"
    );
    assert_eq!(tracks[0].title, "Song (feat. Guest)");
    assert_eq!(tracks[0].artist.as_deref(), Some("Band, Guest"));
    assert_eq!(tracks[0].album.as_deref(), Some("The Album"));
    assert_eq!(tracks[0].duration_ms, Some(201_000));
    assert_eq!(
        tracks[0].artwork_url.as_deref(),
        Some("https://lh3.googleusercontent.com/abc123def45=w544-h544-l90-rj")
    );
    assert_eq!(
        tracks[0].source,
        Some(raygun_model::MediaSource::YouTube {
            video_id: "abc123def45".into()
        })
    );
    assert_eq!(tracks[1].artist.as_deref(), Some("Some Uploader"));
    assert_eq!(tracks[1].duration_ms, Some(3_723_000));
    let candidates = host
        .stream_candidates(
            &query("Song", Some(201_000)),
            5,
            raygun_resolver::QualityTarget::Any,
        )
        .await;
    let candidates = candidates[0].result.as_ref().unwrap();
    assert_eq!(candidates[0].id, "abc123def45");
    assert_eq!(candidates[0].quality.as_deref(), Some("YouTube"));

    // With a key: the official API.
    let host = load("test-key").await;
    let results = host.search_music("song", 10).await;
    assert_eq!(results[0].error, None);
    let tracks = &results[0].tracks;
    assert_eq!(tracks.len(), 2, "channels are skipped");
    assert_eq!(tracks[0].title, "Band & Friends - Song (Official Audio)");
    assert_eq!(tracks[0].artist.as_deref(), Some("Band"));
    assert_eq!(tracks[0].duration_ms, Some(201_000));
    assert_eq!(tracks[1].title, "Song 'Live'");
    assert_eq!(tracks[1].duration_ms, Some(3_723_000));

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
    let catalog = move || {
        vec![
            manga(
                "m1",
                json!({ "ja-ro": "Sousou no Frieren" }),
                json!([{ "en": "Frieren" }]),
            ),
            // An English main title wins over an English alternative.
            manga(
                "m2",
                json!({ "en": "Plain English" }),
                json!([{ "en": "PE" }]),
            ),
            manga("m3", json!({ "ja-ro": "Third" }), json!([])),
        ]
    };
    let app = Router::new()
        .route(
            "/manga",
            get(move |uri: Uri| async move {
                let params = query_params(&uri);
                // Latest updates ask for the series of new chapters by id.
                let ids: Vec<String> = uri
                    .query()
                    .unwrap_or_default()
                    .split('&')
                    .filter_map(|pair| pair.strip_prefix("ids%5B%5D="))
                    .map(str::to_owned)
                    .collect();
                if !ids.is_empty() {
                    let items: Vec<_> = catalog()
                        .into_iter()
                        .filter(|item| ids.iter().any(|id| item["id"] == id.as_str()))
                        .collect();
                    return json_response(StatusCode::OK, json!({ "result": "ok", "data": items, "total": items.len(), "offset": 0, "limit": 100 }));
                }
                let searched = params.get("title").cloned();
                assert_eq!(params.get("availableTranslatedLanguage%5B%5D").map(String::as_str), Some("en"));
                let items = match searched.as_deref() {
                    Some("nothing") => vec![],
                    _ => catalog().into_iter().take(2).collect(),
                };
                let offset: usize = params.get("offset").and_then(|offset| offset.parse().ok()).unwrap_or(0);
                json_response(StatusCode::OK, json!({ "result": "ok", "data": items, "total": if items.is_empty() { 0 } else { 30 }, "offset": offset, "limit": 24 }))
            }),
        )
        .route(
            "/chapter",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                assert_eq!(params.get("translatedLanguage%5B%5D").map(String::as_str), Some("en"));
                assert_eq!(params.get("order%5BreadableAt%5D").map(String::as_str), Some("desc"));
                let of = |manga: &str| json!({
                    "id": format!("c-{manga}"),
                    "attributes": { "volume": null, "chapter": "1", "title": null },
                    "relationships": [{ "id": manga, "type": "manga" }, { "id": "g", "type": "scanlation_group" }],
                });
                let offset = params.get("offset").map(String::as_str).unwrap_or("0");
                let data = if offset == "0" { vec![of("m2"), of("m1"), of("m2")] } else { vec![of("m1"), of("m3")] };
                json_response(StatusCode::OK, json!({ "result": "ok", "data": data, "total": 102, "offset": offset.parse::<u32>().unwrap(), "limit": 100 }))
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
    assert_eq!(
        popular.items[1].title, "Plain English",
        "an English main title wins over an English alternative"
    );
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

    // Latest updates: the series of the newest chapters in the language, in their order, each
    // once; a later page leaves out the series already shown.
    let ids = |listing: &crate::manga::MangaListing| -> Vec<String> {
        listing.items.iter().map(|item| item.id.clone()).collect()
    };
    let latest = host
        .manga_list(provider, crate::manga::MangaList::Latest, 1)
        .await
        .unwrap();
    assert_eq!(ids(&latest), ["m2", "m1"]);
    assert!(latest.has_next_page);
    let later = host
        .manga_list(provider, crate::manga::MangaList::Latest, 2)
        .await
        .unwrap();
    assert_eq!(ids(&later), ["m3"]);
    assert!(!later.has_next_page);

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

#[tokio::test]
async fn internet_archive_films_browse_details_episodes_and_videos() {
    let app = Router::new()
        .route(
            "/advancedsearch.php",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                assert_eq!(params["rows"], "30");
                let q = &params["q"];
                let docs = if q.contains("Film_Noir") {
                    json!([{ "identifier": "noir", "title": "Detour" }])
                } else {
                    json!([
                        { "identifier": "chaplin", "title": "Charlie Chaplin Festival" },
                        { "identifier": "serial", "title": ["Flash Gordon", "alt"] },
                    ])
                };
                json_response(StatusCode::OK, json!({ "response": { "numFound": 45, "docs": docs } }))
            }),
        )
        .route(
            "/metadata/chaplin",
            get(|| async {
                json_response(StatusCode::OK, json!({
                    "metadata": {
                        "title": "Charlie Chaplin Festival", "date": "1938", "creator": "Charles Chaplin",
                        "subject": "comedy; silent", "description": "Four shorts.<br>See <a href=\"x\">IMDb</a> &amp; more.",
                    },
                    "files": [
                        { "name": "chaplin.mpeg", "format": "MPEG2", "source": "original", "height": "480" },
                        { "name": "chaplin.mp4", "format": "h.264", "source": "derivative", "original": "chaplin.mpeg", "height": "480" },
                        { "name": "chaplin_512kb.mp4", "format": "512Kb MPEG4", "source": "derivative", "original": "chaplin.mpeg", "height": "240" },
                        { "name": "chaplin.ogv", "format": "Ogg Video", "source": "derivative", "original": "chaplin.mpeg", "height": "304" },
                        { "name": "chaplin.asr.srt", "format": "SubRip", "source": "original" },
                    ],
                }))
            }),
        )
        .route(
            "/metadata/serial",
            get(|| async {
                json_response(StatusCode::OK, json!({
                    "metadata": { "title": "Flash Gordon" },
                    "files": [
                        { "name": "ch 2.mp4", "format": "MPEG4", "source": "original", "title": "Chapter 2" },
                        { "name": "ch 1.mp4", "format": "MPEG4", "source": "original", "title": "Chapter 1" },
                    ],
                }))
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension(
        "internet-archive",
        "https://archive.org",
        address,
    )])
    .await;
    let provider = "dev.raygun.internet-archive/films";
    assert_eq!(host.providers(crate::video::VIDEO_SOURCE).await.len(), 1);

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert!(popular.has_next_page, "30 of 45 shown");
    assert_eq!(popular.items[1].title, "Flash Gordon");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some(format!("http://{address}/services/img/chaplin").as_str())
    );
    let mut filters = std::collections::BTreeMap::new();
    filters.insert(
        "collection".to_owned(),
        raygun_model::FilterValue::Select("Film_Noir".into()),
    );
    let noir = host
        .manga_search_filtered(provider, "", 2, &filters)
        .await
        .unwrap();
    assert_eq!(noir.items[0].title, "Detour");
    assert!(!noir.has_next_page, "page 2 of 45 at 30 a page is the last");
    assert_eq!(host.manga_filters(provider).await.unwrap().len(), 2);

    let details = host.manga_details(provider, "chaplin").await.unwrap();
    assert_eq!(
        (details.media_type.as_deref(), details.year),
        (Some("movie"), Some(1938))
    );
    assert_eq!(
        details.artists,
        ["Charles Chaplin"],
        "creators are directors"
    );
    assert_eq!(details.genres, ["comedy", "silent"]);
    assert_eq!(
        details.description.as_deref(),
        Some("Four shorts.\nSee IMDb & more."),
        "HTML becomes text"
    );

    let film = host.video_episodes(provider, "chaplin").await.unwrap();
    assert_eq!(film.len(), 1);
    assert_eq!(
        (film[0].id.as_str(), film[0].name.as_str()),
        ("chaplin/chaplin.mpeg", "Film")
    );
    let serial = host.video_episodes(provider, "serial").await.unwrap();
    let names: Vec<_> = serial.iter().map(|episode| episode.name.as_str()).collect();
    assert_eq!(names, ["Chapter 1", "Chapter 2"], "in file order");

    let servers = host.video_servers(provider, &film[0].id).await.unwrap();
    assert_eq!(servers.len(), 1);
    let videos = servers[0].videos.as_ref().expect("resolved at once");
    let qualities: Vec<_> = videos.iter().map(|video| video.quality).collect();
    assert_eq!(qualities, [Some(480), Some(240)], "MP4s only, best first");
    assert_eq!(
        videos[0].url,
        format!("http://{address}/download/chaplin/chaplin.mp4")
    );
    assert_eq!(videos[0].label.as_deref(), Some("480p · h.264"));
    assert_eq!(
        videos[0].subtitles[0].label.as_deref(),
        Some("Automatic (English)")
    );
    let again = host.video_links(provider, &film[0].id).await.unwrap();
    assert_eq!(again.len(), 2);
}

fn html_response(body: String) -> Response {
    (
        StatusCode::OK,
        [(header::CONTENT_TYPE, "text/html; charset=utf-8")],
        body,
    )
        .into_response()
}

/// A series card as WEBTOON's rankings, genres and search pages draw one.
fn webtoon_card(section: &str, title_no: u32, title: &str) -> String {
    format!(
        r#"<li><a href="https://www.webtoons.com/en/{section}/slug/list?title_no={title_no}" class="link">
        <div class="image_wrap"><img src="https://webtoon-phinf.pstatic.net/x/{title_no} Thumb_Poster.jpg?type=q90"></div>
        <div class="info_text"><strong class="title"> {title} </strong></div></a></li>"#
    )
}

fn webtoon_list(cards: &[String], more: &str) -> String {
    format!(
        r#"<html><body><ul class="webtoon_list">{}</ul>{more}</body></html>"#,
        cards.concat()
    )
}

#[tokio::test]
async fn webtoon_lists_series_episodes_and_pages() {
    let app = Router::new()
        .route(
            "/en/ranking/{name}",
            get(|axum::extract::Path(name): axum::extract::Path<String>| async move {
                match name.as_str() {
                    "popular" => html_response(webtoon_list(
                        &[webtoon_card("action", 1, "One"), webtoon_card("canvas", 9, "Nine")],
                        "",
                    )),
                    // One ranking failing leaves the others.
                    "trending" => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
                    _ => html_response(webtoon_list(&[webtoon_card("drama", 1, "One again")], "")),
                }
            }),
        )
        .route(
            "/en/search/{section}",
            get(|axum::extract::Path(section): axum::extract::Path<String>, uri: Uri| async move {
                let params = query_params(&uri);
                assert_eq!(params["keyword"], "tower");
                if section == "originals" {
                    html_response(webtoon_list(&[webtoon_card("fantasy", 95, "Tower of God")], ""))
                } else {
                    let next = format!(r#"<a class="pagination" href="/en/search/canvas?keyword=tower&page={}">next</a>"#, params["page"].parse::<u32>().unwrap() + 1);
                    html_response(webtoon_list(&[webtoon_card("canvas", 624546, "THE TOWER.")], &next))
                }
            }),
        )
        .route(
            "/en/canvas/list",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                assert_eq!((params["genreTab"].as_str(), params["sortOrder"].as_str()), ("ROMANCE", "UPDATE"));
                html_response(format!(
                    r#"<html><body><div class="challenge_lst"><ul><li>
                    <a href="https://www.webtoons.com/en/canvas/love/list?title_no=77" class="challenge_item">
                    <span class="img_area"><img src="https://webtoon-phinf.pstatic.net/c/77.png?type=f164_164"></span>
                    <p class="subj">Love</p></a></li></ul></div>
                    <ul class="lst_type1"><li><a href="https://www.webtoons.com/en/canvas/side/list?title_no=5"><p class="subj">Side ranking</p></a></li></ul>
                    <a href="/en/canvas/list?genreTab=ROMANCE&page={}">2</a></body></html>"#,
                    params["page"].parse::<u32>().unwrap() + 1
                ))
            }),
        )
        .route(
            "/en/series/series/list",
            get(|| async {
                html_response(r#"<html><head><meta property="og:image" content="https://swebtoon-phinf.pstatic.net/a/2480.jpg?type=crop540_540"></head><body>
                <div class="detail_header"><div class="info"><h2 class="genre g_action">Action</h2><h1 class="subj">Jungle Juice</h1>
                <div class="author_area"> Hyeong Eun , Ilhwa
                 ... <button type="button" class="ico_info2 _btnAuthorInfo">author info</button></div></div></div>
                <div class="aside detail" id="_asideDetail"><p class="day_info"><span class="txt_ico_up">UP</span>EVERY SUNDAY</p>
                <p class="summary">Wings  sprout.</p></div></body></html>"#.to_owned())
            }),
        )
        .route(
            "/api/v1/webtoon/2480/episodes",
            get(|| async {
                json_response(StatusCode::OK, json!({ "result": { "episodeList": [
                    { "episodeNo": 1, "episodeTitle": "Episode 1", "exposureDateMillis": 1614042038000_i64 },
                    { "episodeNo": 2, "episodeTitle": " Episode 2 ", "exposureDateMillis": 1614646838000_i64 },
                ] }, "success": true }))
            }),
        )
        .route(
            "/en/series/series/episode/viewer",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                assert_eq!((params["title_no"].as_str(), params["episode_no"].as_str()), ("2480", "2"));
                html_response(r#"<html><body><div id="_imageList">
                <img src="https://webtoons-static.pstatic.net/image/bg_transparency.png" class="_images" data-url="https://webtoon-phinf.pstatic.net/p/1.jpg?type=q90">
                <img src="https://webtoons-static.pstatic.net/image/bg_transparency.png" class="_images" data-url="https://webtoon-phinf.pstatic.net/p/2.jpg?type=q90">
                </div></body></html>"#.to_owned())
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension_bases(
        "webtoons",
        &["https://www.webtoons.com", "https://m.webtoons.com"],
        address,
    )])
    .await;
    let provider = "dev.raygun.webtoons/webtoons";
    let ids = |listing: &crate::manga::MangaListing| -> Vec<String> {
        listing.items.iter().map(|item| item.id.clone()).collect()
    };

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert_eq!(
        ids(&popular),
        ["en:webtoon:1", "en:canvas:9"],
        "each series once"
    );
    assert_eq!(popular.items[0].title, "One");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://webtoon-phinf.pstatic.net/x/1%20Thumb_Poster.jpg?type=q90")
    );
    // The site (here this server) is the Referer the picture host wants.
    let referer = format!("http://{address}/");
    assert_eq!(popular.items[0].cover_headers["Referer"], referer);

    let both = host.manga_search(provider, "tower", 1).await.unwrap();
    assert_eq!(
        ids(&both),
        ["en:webtoon:95", "en:canvas:624546"],
        "Originals first"
    );
    assert!(both.has_next_page, "Canvas has a second page");

    let chosen = BTreeMap::from([
        (
            "type".to_owned(),
            raygun_model::FilterValue::Select("canvas".into()),
        ),
        (
            "genre".to_owned(),
            raygun_model::FilterValue::Select("romance".into()),
        ),
        (
            "sort".to_owned(),
            raygun_model::FilterValue::Select("UPDATE".into()),
        ),
    ]);
    let romance = host
        .manga_search_filtered(provider, "", 1, &chosen)
        .await
        .unwrap();
    assert_eq!(
        ids(&romance),
        ["en:canvas:77"],
        "the side ranking isn't the list"
    );
    assert!(romance.has_next_page);

    let details = host
        .manga_details(provider, "en:webtoon:2480")
        .await
        .unwrap();
    assert_eq!(details.title, "Jungle Juice");
    assert_eq!(details.authors, ["Hyeong Eun", "Ilhwa"]);
    assert_eq!(details.genres, ["Action"]);
    assert_eq!(details.status, "ongoing");
    assert_eq!(details.description.as_deref(), Some("Wings sprout."));
    assert!(
        details
            .url
            .unwrap()
            .ends_with("/en/series/series/list?title_no=2480")
    );
    assert!(host.manga_details(provider, "xx:webtoon:1").await.is_err());

    let chapters = host
        .manga_chapters(provider, "en:webtoon:2480")
        .await
        .unwrap();
    let names: Vec<_> = chapters
        .iter()
        .map(|chapter| chapter.name.as_str())
        .collect();
    assert_eq!(names, ["Episode 2", "Episode 1"], "newest first");
    assert_eq!(chapters[0].id, "en:webtoon:2480:2");
    assert_eq!(chapters[0].date_upload, Some(1_614_646_838_000));

    let pages = host.manga_pages(provider, &chapters[0].id).await.unwrap();
    let urls: Vec<_> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://webtoon-phinf.pstatic.net/p/1.jpg?type=q90",
            "https://webtoon-phinf.pstatic.net/p/2.jpg?type=q90"
        ]
    );
    assert!(pages.iter().all(|page| page.headers["Referer"] == referer));
}

#[tokio::test]
async fn tapas_lists_series_and_only_free_episodes() {
    let app = Router::new()
        .route(
            "/ranking",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                assert_eq!((params["category_type"].as_str(), params["page"].as_str()), ("COMIC", "0"));
                json_response(StatusCode::OK, json!({
                    "data": { "items": [
                        { "seriesId": 42, "title": " Solo ", "mature": false,
                          "assetProperty": { "bookCoverImage": { "path": "https://story-a.tapas.io/bc/42" } } },
                        { "seriesId": 43, "title": "Grown-ups", "mature": true },
                        { "seriesId": "bad", "title": "Bad id" },
                    ] },
                    "meta": { "pagination": { "page": 0, "last": false } },
                }))
            }),
        )
        .route(
            "/search",
            get(|| async {
                html_response(r#"<html><body><ul>
                <li class="search-item-wrap"><div class="item-thumb-wrap"><a class="thumb-wrap" data-series-id="69274" href="/series/Solo-Q"><img src="https://us-a.tapas.io/sa/q.jpg"></a></div>
                <div class="title-section"><p class="title"><a class="link" data-series-id="69274" href="/series/Solo-Q">#_h_i_g_h_L_i_g_h_t_#Solo#/_h_i_g_h_L_i_g_h_t_#-Q</a></p></div></li>
                </ul><a class="paging__button paging__button--next" href="?pageNumber=2&q=solo">next</a></body></html>"#.to_owned())
            }),
        )
        .route(
            "/series/42/episodes",
            get(|uri: Uri| async move {
                let params = query_params(&uri);
                let page: u32 = params["page"].parse().unwrap();
                let episode = |id: u32, free: bool, scheduled: bool| json!({
                    "id": id, "title": format!("Episode {id}"), "scene": id,
                    "publish_date": "2025-08-24T18:00:00Z", "free": free, "scheduled": scheduled,
                });
                // 45 episodes, 20 to a page; only some free; one listed twice across pages.
                let episodes = match page {
                    1 => vec![episode(45, false, false), episode(44, true, true), episode(30, true, false)],
                    2 => vec![episode(30, true, false), episode(21, false, false)],
                    _ => vec![episode(2, true, false), episode(1, true, false)],
                };
                json_response(StatusCode::OK, json!({ "code": 200, "data": {
                    "pagination": { "page": page + 1, "has_next": page < 3, "total": 45 },
                    "episodes": episodes,
                } }))
            }),
        )
        .route(
            "/episode/{id}",
            get(|axum::extract::Path(id): axum::extract::Path<u32>| async move {
                let body = if id == 2 {
                    r#"<article class="viewer__body"><img class="content__img js-lazy" src="data:image/gif;base64,R0lGOD" data-src="https://us-a.tapas.io/pc/1.png?__token__=exp=1~hmac=a&amp;version=v4"></article>"#
                } else {
                    r#"<div class="locked">Unlock with coins</div>"#
                };
                html_response(format!("<html><body>{body}</body></html>"))
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension_bases(
        "tapas",
        &[
            "https://story-api.tapas.io/cosmos/api/v1/landing",
            "https://tapas.io",
        ],
        address,
    )])
    .await;
    let provider = "dev.raygun.tapas/tapas";

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    assert!(popular.has_next_page);
    assert_eq!(popular.items.len(), 2, "a series needs a number for an id");
    assert_eq!(popular.items[0].title, "Solo");
    assert_eq!(
        popular.items[0].cover_url.as_deref(),
        Some("https://story-a.tapas.io/bc/42.jpg")
    );
    assert!(popular.items[1].nsfw, "Tapas marks mature series");

    let found = host.manga_search(provider, "solo", 1).await.unwrap();
    assert_eq!(found.items[0].id, "69274");
    assert_eq!(found.items[0].title, "Solo-Q", "search highlights removed");
    assert!(found.has_next_page);

    let chapters = host.manga_chapters(provider, "42").await.unwrap();
    let ids: Vec<_> = chapters.iter().map(|chapter| chapter.id.as_str()).collect();
    assert_eq!(ids, ["30", "2", "1"], "free, published episodes, each once");
    assert_eq!(chapters[0].number, Some(30.0));
    assert_eq!(chapters[0].date_upload, Some(1_756_058_400_000));

    let pages = host.manga_pages(provider, "2").await.unwrap();
    assert_eq!(
        pages[0].url,
        "https://us-a.tapas.io/pc/1.png?__token__=exp=1~hmac=a&version=v4"
    );
    let locked = host.manga_pages(provider, "45").await.unwrap_err();
    assert!(locked.contains("coins"), "{locked}");
}

#[tokio::test]
async fn naver_webtoon_interleaves_days_and_leaves_out_paid_episodes() {
    let app = Router::new()
        .route(
            "/api/webtoon/titlelist/weekday",
            get(|| async {
                let title = |id: u32, adult: bool| json!({
                    "titleId": id, "titleName": format!("Title {id}"), "adult": adult,
                    "thumbnailUrl": format!("https://image-comic.pstatic.net/webtoon/{id}/t.jpg"),
                });
                json_response(StatusCode::OK, json!({
                    "dayOfWeek": "WEDNESDAY",
                    "titleListMap": {
                        "MONDAY": [title(1, false), title(2, false)],
                        "WEDNESDAY": [title(3, true), title(1, false)],
                        "THURSDAY": [title(4, false)],
                    },
                }))
            }),
        )
        .route(
            "/api/article/list",
            get(|uri: Uri| async move {
                let page: u32 = query_params(&uri)["page"].parse().unwrap();
                let article = |no: u32, charge: bool, date: &str| json!({
                    "no": no, "subtitle": format!("{no}화"), "charge": charge, "serviceDateDescription": date,
                });
                let articles = match page {
                    1 => vec![article(43, true, "26.11.04"), article(42, false, "26.10.28")],
                    2 => vec![article(22, false, "3일 전")],
                    _ => vec![article(1, false, "21.04.21")],
                };
                json_response(StatusCode::OK, json!({
                    "articleList": articles,
                    "pageInfo": { "page": page, "totalPages": 3 },
                }))
            }),
        )
        .route(
            "/webtoon/detail",
            get(|| async {
                html_response(r#"<html><body><img src="https://image-comic.pstatic.net/webtoon/7/1/thumbnail_202x120.jpg">
                <div class="wt_viewer" id="sectionContWide">
                <img src="https://image-comic.pstatic.net/webtoon/7/1/IMAG01_1.jpg" alt="comic content" id="content_image_0">
                <img src="https://image-comic.pstatic.net/webtoon/7/1/IMAG01_2.jpg" alt="comic content" id="content_image_1">
                <img src="https://ssl.pstatic.net/static/ad.png" id="banner">
                </div></body></html>"#.to_owned())
            }),
        );
    let address = serve(app);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![patched_extension(
        "naver-webtoon",
        "https://comic.naver.com",
        address,
    )])
    .await;
    let provider = "dev.raygun.naver-webtoon/naver";

    let popular = host
        .manga_list(provider, crate::manga::MangaList::Popular, 1)
        .await
        .unwrap();
    let ids: Vec<_> = popular.items.iter().map(|item| item.id.as_str()).collect();
    assert_eq!(
        ids,
        ["3", "4", "1", "2"],
        "each day's most popular in turn, today's first, each once"
    );
    assert!(popular.items[0].nsfw, "series for grown-ups are marked");
    let referer = format!("http://{address}/");
    assert_eq!(popular.items[0].cover_headers["Referer"], referer);

    let chapters = host.manga_chapters(provider, "7").await.unwrap();
    let ids: Vec<_> = chapters.iter().map(|chapter| chapter.id.as_str()).collect();
    assert_eq!(ids, ["7:42", "7:22", "7:1"], "paid early episodes left out");
    assert_eq!(chapters[0].date_upload, Some(1_793_145_600_000));
    assert_eq!(
        chapters[1].date_upload, None,
        "a relative date isn't guessed"
    );

    let pages = host.manga_pages(provider, "7:1").await.unwrap();
    let urls: Vec<_> = pages.iter().map(|page| page.url.as_str()).collect();
    assert_eq!(
        urls,
        [
            "https://image-comic.pstatic.net/webtoon/7/1/IMAG01_1.jpg",
            "https://image-comic.pstatic.net/webtoon/7/1/IMAG01_2.jpg"
        ]
    );
    assert!(pages.iter().all(|page| page.headers["Referer"] == referer));
}
