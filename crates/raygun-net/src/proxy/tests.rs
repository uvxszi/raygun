use std::collections::BTreeMap;

use axum::http::HeaderMap;

use super::*;

const SONG: &[u8] = b"0123456789abcdef";
const REQUIRED_REFERER: &str = "https://allowed.example/";

/// Origin that behaves like a picky host: it needs a Referer and supports byte ranges.
async fn start_origin() -> SocketAddr {
    async fn song(headers: HeaderMap) -> Response {
        if headers
            .get(header::REFERER)
            .and_then(|value| value.to_str().ok())
            != Some(REQUIRED_REFERER)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        match headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
        {
            Some("bytes=2-5") => Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, "audio/mpeg")
                .header(header::CONTENT_RANGE, format!("bytes 2-5/{}", SONG.len()))
                .header(header::ACCEPT_RANGES, "bytes")
                .body(Body::from(&SONG[2..6]))
                .unwrap(),
            _ => Response::builder()
                .header(header::CONTENT_TYPE, "audio/mpeg")
                .body(Body::from(SONG))
                .unwrap(),
        }
    }

    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/song.mp3", get(song)))
            .await
            .unwrap();
    });
    address
}

fn remote(origin: SocketAddr, with_referer: bool) -> MediaSource {
    let mut headers = BTreeMap::new();
    if with_referer {
        headers.insert("Referer".to_owned(), REQUIRED_REFERER.to_owned());
    }
    MediaSource::Url {
        url: format!("http://{origin}/song.mp3"),
        headers,
    }
}

async fn proxy() -> StreamProxy {
    StreamProxy::start(crate::http_client().unwrap(), None)
        .await
        .unwrap()
}

#[tokio::test]
async fn injects_source_headers_and_streams_the_body() {
    let origin = start_origin().await;
    let proxy = proxy().await;
    let url = proxy.register(&remote(origin, true));

    let response = reqwest::get(&url).await.unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/mpeg");
    assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    assert_eq!(response.bytes().await.unwrap().as_ref(), SONG);
}

#[tokio::test]
async fn linked_fonts_are_served_and_nothing_else() {
    let temp = tempfile::TempDir::new().unwrap();
    let file = temp.path().join("Face-Regular.ttf");
    std::fs::write(&file, b"\x00\x01\x00\x00font").unwrap();
    let proxy = proxy().await;
    let url = proxy.fonts().link(&file);
    assert_eq!(proxy.fonts().link(&file), url, "one file, one address");

    let response = reqwest::get(&url).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    assert_eq!(response.headers()[header::CONTENT_TYPE], "font/ttf");
    assert_eq!(
        response.bytes().await.unwrap().as_ref(),
        b"\x00\x01\x00\x00font"
    );

    let unknown = format!(
        "http://{}/font/00112233445566778899aabbccddeeff",
        proxy.address()
    );
    assert_eq!(reqwest::get(&unknown).await.unwrap().status(), 404);
}

#[tokio::test]
async fn without_the_header_the_host_refuses() {
    let origin = start_origin().await;
    let proxy = proxy().await;
    let url = proxy.register(&remote(origin, false));

    let response = reqwest::get(&url).await.unwrap();

    assert_eq!(response.status(), 403);
}

#[tokio::test]
async fn forwards_range_requests() {
    let origin = start_origin().await;
    let proxy = proxy().await;
    let url = proxy.register(&remote(origin, true));

    let response = reqwest::Client::new()
        .get(&url)
        .header(header::RANGE, "bytes=2-5")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()[header::CONTENT_RANGE], "bytes 2-5/16");
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"2345");
}

#[test]
fn youtube_streams_go_by_range_and_whole_files_come_back_whole() {
    const YOUTUBE: &str = "https://rr1---sn-x.googlevideo.com/videoplayback?itag=251";
    let none = HeaderMap::new();
    let mut asked = HeaderMap::new();
    asked.insert(header::RANGE, HeaderValue::from_static("bytes=100-"));

    // A player asking for the whole file: YouTube's server gets `bytes=0-`, the player a 200.
    let (range, added) = upstream_range(YOUTUBE, &none);
    assert_eq!(
        range.as_ref().map(|range| range.as_bytes()),
        Some(&b"bytes=0-"[..])
    );
    assert!(added);
    assert_eq!(
        client_status(reqwest::StatusCode::PARTIAL_CONTENT, added),
        200
    );
    assert_eq!(client_status(reqwest::StatusCode::FORBIDDEN, added), 403);
    // Its own range goes as it is, and its answer too.
    let (range, added) = upstream_range(YOUTUBE, &asked);
    assert_eq!(
        range.as_ref().map(|range| range.as_bytes()),
        Some(&b"bytes=100-"[..])
    );
    assert!(!added);
    assert_eq!(
        client_status(reqwest::StatusCode::PARTIAL_CONTENT, added),
        206
    );
    // Other hosts get no range they didn't ask for.
    assert_eq!(
        upstream_range("https://audius.co/stream", &none),
        (None, false)
    );
}

#[tokio::test]
async fn serves_local_files_with_ranges() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("track.mp3");
    std::fs::write(&path, SONG).unwrap();
    let proxy = proxy().await;
    let url = proxy.register(&MediaSource::LocalFile {
        path: path.display().to_string(),
    });

    let response = reqwest::Client::new()
        .get(&url)
        .header(header::RANGE, "bytes=10-")
        .send()
        .await
        .unwrap();

    assert_eq!(response.status(), 206);
    assert_eq!(response.headers()[header::CONTENT_TYPE], "audio/mpeg");
    assert_eq!(response.bytes().await.unwrap().as_ref(), b"abcdef");
}

#[tokio::test]
async fn unknown_tokens_are_not_found() {
    let proxy = proxy().await;

    let response = reqwest::get(format!("http://{}/stream/nope", proxy.address()))
        .await
        .unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn old_tokens_expire() {
    let temp = tempfile::TempDir::new().unwrap();
    let path = temp.path().join("track.mp3");
    std::fs::write(&path, SONG).unwrap();
    let proxy = proxy().await;
    let source = MediaSource::LocalFile {
        path: path.display().to_string(),
    };
    let first = proxy.register(&source);
    for _ in 0..MAX_REGISTERED {
        proxy.register(&source);
    }

    let response = reqwest::get(&first).await.unwrap();

    assert_eq!(response.status(), 404);
}

#[tokio::test]
async fn serves_only_allowed_artwork_names() {
    let temp = tempfile::TempDir::new().unwrap();
    std::fs::write(temp.path().join("good.png"), b"png-bytes").unwrap();
    std::fs::write(temp.path().join("secret.txt"), b"nope").unwrap();
    let proxy = StreamProxy::start(
        crate::http_client().unwrap(),
        Some(StaticFiles {
            dir: temp.path().to_path_buf(),
            is_allowed: |name| name.ends_with(".png"),
        }),
    )
    .await
    .unwrap();

    let good = reqwest::get(proxy.artwork_url("good.png")).await.unwrap();
    assert_eq!(good.status(), 200);
    assert_eq!(good.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(good.bytes().await.unwrap().as_ref(), b"png-bytes");

    let refused = reqwest::get(proxy.artwork_url("secret.txt")).await.unwrap();
    assert_eq!(refused.status(), 404);
}

#[tokio::test]
async fn serves_local_files_by_plain_relative_paths() {
    let temp = tempfile::TempDir::new().unwrap();
    let chapter = temp.path().join("manga/t1/c1");
    std::fs::create_dir_all(&chapter).unwrap();
    std::fs::write(chapter.join("001.png"), b"\x89PNG one").unwrap();
    std::fs::write(temp.path().join(".secret"), b"nope").unwrap();
    let proxy = StreamProxy::start_with(
        crate::http_client().unwrap(),
        ProxyStorage {
            local: Some(temp.path().to_path_buf()),
            ..ProxyStorage::default()
        },
    )
    .await
    .unwrap();
    let images = proxy.images();

    let page = reqwest::get(images.local("manga/t1/c1/001.png"))
        .await
        .unwrap();
    assert_eq!(page.status(), 200);
    assert_eq!(page.headers()[header::CONTENT_TYPE], "image/png");
    assert_eq!(page.bytes().await.unwrap().as_ref(), b"\x89PNG one");

    for refused in [
        ".secret",
        "manga/t1/c1/%2e%2e%2f%2e%2e%2fc1/001.png",
        "manga//t1",
        "manga/t1/c1/%2e%2e",
    ] {
        let response = reqwest::get(images.local(refused)).await.unwrap();
        assert_eq!(response.status(), 404, "{refused}");
    }
    assert!(local_path(temp.path(), "a/../b").is_none());
    assert!(local_path(temp.path(), "a/.b").is_none());
    assert!(local_path(temp.path(), r"a\b").is_none());
    assert!(local_path(temp.path(), "ok/001.png").is_some());
}

#[tokio::test]
async fn serves_the_youtube_embed_page_on_localhost() {
    let proxy = proxy().await;

    let url = proxy.youtube_embed_url();
    assert!(url.starts_with("http://localhost:"), "{url}");
    let response = reqwest::get(format!("http://{}/embed/youtube", proxy.address()))
        .await
        .unwrap();

    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/html; charset=utf-8"
    );
    assert!(response.headers().get("x-frame-options").is_none());
    assert!(
        response
            .text()
            .await
            .unwrap()
            .contains("youtube.com/iframe_api")
    );
}

#[tokio::test]
async fn youtube_sources_are_not_streamed() {
    let proxy = proxy().await;
    let url = proxy.register(&MediaSource::YouTube {
        video_id: "M7lc1UVf-VE".into(),
    });

    assert_eq!(reqwest::get(&url).await.unwrap().status(), 404);
}

#[tokio::test]
async fn images_get_stable_links_with_their_headers() {
    let origin = start_origin().await;
    let proxy = proxy().await;
    let images = proxy.images();
    let url = format!("http://{origin}/song.mp3");
    let referer = BTreeMap::from([("Referer".to_owned(), REQUIRED_REFERER.to_owned())]);

    let link = images.link(&url, &referer);
    assert_eq!(images.link(&url, &referer), link, "one image, one link");
    assert_ne!(
        images.link(&url, &BTreeMap::new()),
        link,
        "headers are part of the link"
    );

    let response = reqwest::get(&link).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()[header::CACHE_CONTROL],
        "public, max-age=86400"
    );
    assert_eq!(response.bytes().await.unwrap().as_ref(), SONG);

    let refused = reqwest::get(images.link(&url, &BTreeMap::new()))
        .await
        .unwrap();
    assert_eq!(refused.status(), 403);
    assert!(
        refused.headers().get(header::CACHE_CONTROL).is_none(),
        "failures aren't cached"
    );

    let unknown = format!("http://{}/image/0000000000000000", proxy.address());
    assert_eq!(reqwest::get(unknown).await.unwrap().status(), 404);
}

#[tokio::test]
async fn cached_images_are_served_from_disk_after_the_first_fetch() {
    use std::sync::atomic::{AtomicUsize, Ordering};

    static HITS: AtomicUsize = AtomicUsize::new(0);
    async fn page() -> Response {
        HITS.fetch_add(1, Ordering::SeqCst);
        Response::builder()
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(&b"\x89PNG page"[..]))
            .unwrap()
    }
    async fn html() -> Response {
        Response::builder()
            .header(header::CONTENT_TYPE, "text/html")
            .body(Body::from("<html>challenge</html>"))
            .unwrap()
    }
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let origin = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let app = Router::new()
            .route("/page.png", get(page))
            .route("/challenge", get(html));
        axum::serve(listener, app).await.unwrap();
    });

    let temp = tempfile::TempDir::new().unwrap();
    let proxy = StreamProxy::start_with(
        crate::http_client().unwrap(),
        ProxyStorage {
            pages: Some(PageCache::new(temp.path().join("pages"), 1 << 20)),
            ..ProxyStorage::default()
        },
    )
    .await
    .unwrap();
    let images = proxy.images();
    let link = images.link(&format!("http://{origin}/page.png"), &BTreeMap::new());

    for _ in 0..2 {
        let response = reqwest::get(&link).await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.headers()[header::CONTENT_TYPE], "image/png");
        assert_eq!(response.bytes().await.unwrap().as_ref(), b"\x89PNG page");
    }
    assert_eq!(
        HITS.load(Ordering::SeqCst),
        1,
        "the second answer came from disk"
    );

    let challenge = images.link(&format!("http://{origin}/challenge"), &BTreeMap::new());
    let response = reqwest::get(&challenge).await.unwrap();
    assert_eq!(
        response.status(),
        502,
        "a page that isn't an image is an error"
    );
    assert!(
        std::fs::read_dir(temp.path().join("pages"))
            .unwrap()
            .count()
            == 1,
        "only the image was stored"
    );
}

#[tokio::test]
async fn covers_from_an_earlier_start_come_from_disk_and_the_port_is_remembered() {
    async fn cover() -> Response {
        Response::builder()
            .header(header::CONTENT_TYPE, "image/png")
            .body(Body::from(&b"\x89PNG cover"[..]))
            .unwrap()
    }
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let origin = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/cover.png", get(cover)))
            .await
            .unwrap();
    });
    let temp = tempfile::TempDir::new().unwrap();
    let storage = || ProxyStorage {
        covers: Some(crate::CoverCache::new(
            temp.path().join("covers"),
            1 << 20,
            std::time::Duration::from_secs(3600),
        )),
        port_file: Some(temp.path().join("proxy-port")),
        ..ProxyStorage::default()
    };

    let first = StreamProxy::start_with(crate::http_client().unwrap(), storage())
        .await
        .unwrap();
    let remembered: u16 = std::fs::read_to_string(temp.path().join("proxy-port"))
        .unwrap()
        .parse()
        .unwrap();
    assert_eq!(remembered, first.address().port());
    let link = first
        .images()
        .cover(&format!("http://{origin}/cover.png"), &BTreeMap::new());
    assert_eq!(reqwest::get(&link).await.unwrap().status(), 200);

    // The first one still holds its port, so this one binds another and remembers it.
    let second = StreamProxy::start_with(crate::http_client().unwrap(), storage())
        .await
        .unwrap();
    let token = link.rsplit('/').next().unwrap();
    let again = reqwest::get(format!("http://{}/cover/{token}", second.address()))
        .await
        .unwrap();
    assert_eq!(
        again.status(),
        200,
        "served from disk without knowing the address"
    );
    assert_eq!(again.bytes().await.unwrap().as_ref(), b"\x89PNG cover");
    let unknown = reqwest::get(format!("http://{}/cover/00ff", second.address()))
        .await
        .unwrap();
    assert_eq!(unknown.status(), 404);
    assert_eq!(
        std::fs::read_to_string(temp.path().join("proxy-port")).unwrap(),
        second.address().port().to_string()
    );
}

/// Covers stored as JPEG XL reach every webview: as JPEG XL where the request takes it, the
/// original JPEG rebuilt elsewhere, with `Vary: Accept` so caches keep them apart.
#[tokio::test]
async fn covers_stored_as_jpeg_xl_go_as_each_request_takes_them() {
    let jpeg = crate::cover_format::tests::jpeg_of(&crate::cover_format::tests::photo(300, 200));
    let body = jpeg.clone();
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let origin = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let cover = move || {
            let body = body.clone();
            async move { ([(header::CONTENT_TYPE, "image/jpeg")], body).into_response() }
        };
        axum::serve(listener, Router::new().route("/cover.jpg", get(cover)))
            .await
            .unwrap();
    });
    let temp = tempfile::TempDir::new().unwrap();
    let covers = crate::CoverCache::new(
        temp.path().join("covers"),
        1 << 24,
        std::time::Duration::ZERO,
    );
    let proxy = StreamProxy::start_with(
        crate::http_client().unwrap(),
        ProxyStorage {
            covers: Some(covers.clone()),
            ..ProxyStorage::default()
        },
    )
    .await
    .unwrap();
    let link = proxy
        .images()
        .cover(&format!("http://{origin}/cover.jpg"), &BTreeMap::new());
    let client = reqwest::Client::new();
    let ask = |accept: &'static str| client.get(&link).header(header::ACCEPT, accept).send();
    let image_accept = "image/avif,image/webp,image/apng,image/*,*/*;q=0.8";

    let first = ask(image_accept).await.unwrap();
    assert_eq!(first.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(first.bytes().await.unwrap().as_ref(), jpeg.as_slice());
    let token = link.rsplit('/').next().unwrap();
    let stored = temp.path().join("covers").join(token);
    for _ in 0..500 {
        if std::fs::read(&stored).is_ok_and(|bytes| crate::images::is_jxl(&bytes)) {
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    let jxl = std::fs::read(&stored).unwrap();
    assert!(crate::images::is_jxl(&jxl), "stored as JPEG XL");

    let rebuilt = ask(image_accept).await.unwrap();
    assert_eq!(rebuilt.headers()[header::CONTENT_TYPE], "image/jpeg");
    assert_eq!(rebuilt.headers()[header::VARY], "Accept");
    assert_eq!(rebuilt.bytes().await.unwrap().as_ref(), jpeg.as_slice());
    let listed = ask("image/jxl,image/*").await.unwrap();
    assert_eq!(listed.headers()[header::CONTENT_TYPE], "image/jxl");
    assert_eq!(listed.bytes().await.unwrap().as_ref(), jxl.as_slice());
    covers.set_webview_jxl(true);
    let shown = ask(image_accept).await.unwrap();
    assert_eq!(shown.headers()[header::CONTENT_TYPE], "image/jxl");
    let fetched = ask("*/*").await.unwrap();
    assert_eq!(fetched.headers()[header::CONTENT_TYPE], "image/jpeg");
}

#[tokio::test]
async fn subtitles_never_fail() {
    let origin = start_origin().await;
    let proxy = proxy().await;
    let links = proxy.images();
    let url = format!("http://{origin}/song.mp3");
    let referer = BTreeMap::from([("Referer".to_owned(), REQUIRED_REFERER.to_owned())]);

    let served = reqwest::get(links.subtitle(&url, &referer)).await.unwrap();
    assert_eq!(served.status(), 200);
    assert_eq!(served.bytes().await.unwrap().as_ref(), SONG);

    // Refused by the host (no Referer) and missing: both an empty track.
    for link in [
        links.subtitle(&url, &BTreeMap::new()),
        links.subtitle(&format!("http://{origin}/missing.srt"), &BTreeMap::new()),
    ] {
        let response = reqwest::get(link).await.unwrap();
        assert_eq!(response.status(), 200);
        assert_eq!(response.text().await.unwrap(), EMPTY_SUBTITLES);
    }

    let unknown = format!("http://{}/subtitle/0000000000000000", proxy.address());
    assert_eq!(reqwest::get(unknown).await.unwrap().status(), 404);
}

/// A host serving an HLS stream that needs the Referer for everything: a playlist (disguised as
/// text) naming one relative and one absolute segment, and the segments with byte ranges.
async fn start_hls_origin() -> SocketAddr {
    async fn playlist(headers: HeaderMap) -> Response {
        if headers
            .get(header::REFERER)
            .and_then(|value| value.to_str().ok())
            != Some(REQUIRED_REFERER)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        let host = headers[header::HOST].to_str().unwrap().to_owned();
        let body = format!(
            "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXTINF:4,\nseg1.ts\n#EXTINF:4,\nhttp://{host}/other/seg2.ts\n#EXT-X-ENDLIST\n"
        );
        ([(header::CONTENT_TYPE, "text/plain")], body).into_response()
    }
    async fn segment(headers: HeaderMap) -> Response {
        if headers
            .get(header::REFERER)
            .and_then(|value| value.to_str().ok())
            != Some(REQUIRED_REFERER)
        {
            return StatusCode::FORBIDDEN.into_response();
        }
        match headers
            .get(header::RANGE)
            .and_then(|value| value.to_str().ok())
        {
            Some("bytes=2-5") => Response::builder()
                .status(StatusCode::PARTIAL_CONTENT)
                .header(header::CONTENT_TYPE, "video/mp2t")
                .header(header::CONTENT_RANGE, format!("bytes 2-5/{}", SONG.len()))
                .body(Body::from(&SONG[2..6]))
                .unwrap(),
            _ => ([(header::CONTENT_TYPE, "video/mp2t")], SONG).into_response(),
        }
    }

    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        let router = Router::new()
            .route("/show/index.txt", get(playlist))
            .route("/show/seg1.ts", get(segment))
            .route("/other/seg2.ts", get(segment));
        axum::serve(listener, router).await.unwrap();
    });
    address
}

#[tokio::test]
async fn media_playlists_lead_back_into_the_proxy_with_their_headers() {
    let origin = start_hls_origin().await;
    let proxy = proxy().await;
    let referer = BTreeMap::from([("Referer".to_owned(), REQUIRED_REFERER.to_owned())]);
    let media = proxy.media();
    let link = media
        .link(&format!("http://{origin}/show/index.txt"), &referer)
        .unwrap();
    assert!(media.is_local(&link));
    assert!(link.ends_with(&format!("/http/{origin}/show/index.txt")));

    let response = reqwest::get(&link).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "application/vnd.apple.mpegurl"
    );
    assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    let playlist = response.text().await.unwrap();
    let segments: Vec<&str> = playlist
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    let base = link.trim_end_matches("index.txt");
    assert_eq!(segments[0], format!("{base}seg1.ts"));
    assert_eq!(
        segments[1],
        link.replace("/show/index.txt", "/other/seg2.ts")
    );

    // Segments get the Referer too, and ranges pass through.
    for segment in segments {
        let whole = reqwest::get(segment).await.unwrap();
        assert_eq!(whole.status(), 200);
        assert_eq!(whole.bytes().await.unwrap().as_ref(), SONG);
    }
    let part = reqwest::Client::new()
        .get(format!("{base}seg1.ts"))
        .header(header::RANGE, "bytes=2-5")
        .send()
        .await
        .unwrap();
    assert_eq!(part.status(), 206);
    assert_eq!(part.headers()[header::CONTENT_RANGE], "bytes 2-5/16");
    assert_eq!(part.bytes().await.unwrap().as_ref(), b"2345");

    // A preflight is answered; unknown tokens and other schemes are not served.
    let preflight = reqwest::Client::new()
        .request(reqwest::Method::OPTIONS, &link)
        .send()
        .await
        .unwrap();
    assert_eq!(preflight.status(), 204);
    assert_eq!(
        preflight.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN],
        "*"
    );
    let unknown = format!(
        "http://{}/media/nope/http/{origin}/show/index.txt",
        proxy.address()
    );
    assert_eq!(reqwest::get(unknown).await.unwrap().status(), 404);
    assert!(media.link("file:///etc/passwd", &referer).is_none());
}

#[tokio::test]
async fn the_cast_route_serves_only_behind_its_token_with_the_streams_headers() {
    let origin = start_hls_origin().await;
    let proxy = proxy().await;
    let referer = BTreeMap::from([("Referer".to_owned(), REQUIRED_REFERER.to_owned())]);
    let route = proxy.lan().open(Ipv4Addr::LOCALHOST.into()).await.unwrap();
    let link = route
        .link(&format!("http://{origin}/show/index.txt"), &referer)
        .unwrap();
    let address = route.address();
    assert!(link.starts_with(&format!("http://{address}/cast/")));

    // The playlist leads back into the route, and its segments get the Referer.
    let response = reqwest::get(&link).await.unwrap();
    assert_eq!(response.status(), 200);
    assert_eq!(response.headers()["transferMode.dlna.org"], "Streaming");
    let playlist = response.text().await.unwrap();
    let segments: Vec<&str> = playlist
        .lines()
        .filter(|line| !line.starts_with('#'))
        .collect();
    assert_eq!(segments.len(), 2);
    let route_base = &link[..link.find("/media/").unwrap()];
    for segment in segments {
        assert!(segment.starts_with(route_base));
        let whole = reqwest::get(segment).await.unwrap();
        assert_eq!(whole.bytes().await.unwrap().as_ref(), SONG);
    }

    // Another token, a shorter one, no token: nothing.
    let token = link
        .split("/cast/")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap()
        .to_owned();
    let wrong = link.replace(&token, &"0".repeat(token.len()));
    assert_eq!(reqwest::get(&wrong).await.unwrap().status(), 404);
    let shorter = link.replace(&token, &token[..10]);
    assert_eq!(reqwest::get(&shorter).await.unwrap().status(), 404);
    let bare = link.replace(&format!("/cast/{token}"), "");
    assert_eq!(reqwest::get(&bare).await.unwrap().status(), 404);

    // The loopback proxy's links move onto the route; other addresses don't.
    let registered = proxy.register(&remote(start_origin().await, true));
    let moved = route.link(&registered, &BTreeMap::new()).unwrap();
    let body = reqwest::get(&moved).await.unwrap().bytes().await.unwrap();
    assert_eq!(body.as_ref(), SONG);
    assert!(route.link("file:///etc/passwd", &BTreeMap::new()).is_none());
    let embed = format!("http://{}/embed/youtube", proxy.address());
    assert!(route.link(&embed, &BTreeMap::new()).is_none());

    // The listener closes with the route.
    drop(route);
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    assert!(reqwest::get(&link).await.is_err());
}

#[tokio::test]
async fn the_cast_route_serves_subtitles_as_webvtt_or_subrip() {
    async fn srt() -> Response {
        (
            [(header::CONTENT_TYPE, "application/x-subrip")],
            "1\n00:00:01,000 --> 00:00:02,000\nHello\n",
        )
            .into_response()
    }
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let origin = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/en.srt", get(srt)))
            .await
            .unwrap();
    });
    let proxy = proxy().await;
    let route = proxy.lan().open(Ipv4Addr::LOCALHOST.into()).await.unwrap();
    let remote = format!("http://{origin}/en.srt");
    let vtt = route
        .subtitle(&remote, &BTreeMap::new(), lan::SubtitleFormat::WebVtt)
        .unwrap();
    assert!(vtt.ends_with(".vtt"));
    let response = reqwest::get(&vtt).await.unwrap();
    assert_eq!(
        response.headers()[header::CONTENT_TYPE],
        "text/vtt; charset=utf-8"
    );
    assert_eq!(response.headers()[header::ACCESS_CONTROL_ALLOW_ORIGIN], "*");
    assert_eq!(
        response.text().await.unwrap(),
        "WEBVTT\n\n00:00:01.000 --> 00:00:02.000\nHello\n"
    );
    // The player's own subtitle link works the same way.
    let proxied = proxy.images().subtitle(&remote, &BTreeMap::new());
    let srt = route
        .subtitle(&proxied, &BTreeMap::new(), lan::SubtitleFormat::SubRip)
        .unwrap();
    let text = reqwest::get(&srt).await.unwrap().text().await.unwrap();
    assert!(text.contains("00:00:01,000 --> 00:00:02,000\r\nHello"));
    let file = route.subtitle(
        "file:///x.srt",
        &BTreeMap::new(),
        lan::SubtitleFormat::SubRip,
    );
    assert!(file.is_none());
}

#[tokio::test]
async fn links_from_an_earlier_start_point_at_this_one() {
    let proxy = proxy().await;
    let links = proxy.images();
    let here = proxy.address();
    assert_eq!(
        links.relink("http://127.0.0.1:1/cover/abc"),
        format!("http://{here}/cover/abc")
    );
    assert_eq!(
        links.relink("http://localhost:40123/art/cover.jpg"),
        format!("http://{here}/art/cover.jpg")
    );
    // Sources' addresses and other local servers stay as they are.
    for url in [
        "https://example.com/cover/abc",
        "http://127.0.0.1:8080/api/thing",
        "http://127.0.0.1:x/cover/abc",
    ] {
        assert_eq!(links.relink(url), url);
    }
}
