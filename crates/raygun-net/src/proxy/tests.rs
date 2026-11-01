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
    let proxy = StreamProxy::start_with_cache(
        crate::http_client().unwrap(),
        None,
        Some(PageCache::new(temp.path().join("pages"), 1 << 20)),
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
    assert_eq!(response.status(), 200);
    assert!(
        std::fs::read_dir(temp.path().join("pages"))
            .unwrap()
            .count()
            == 1,
        "only the image was stored"
    );
}
