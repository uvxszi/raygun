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
