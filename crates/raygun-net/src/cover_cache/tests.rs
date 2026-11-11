use std::net::{Ipv4Addr, SocketAddr};
use std::sync::atomic::AtomicUsize;

use axum::Router;
use axum::extract::State;
use axum::http::HeaderMap;
use axum::response::{IntoResponse, Response};
use axum::routing::get;

use super::*;
use crate::cover_format::tests::{jpeg_of, photo};

const PNG_A: &[u8] = b"\x89PNG\r\n\x1a\nfirst";
const PNG_B: &[u8] = b"\x89PNG\r\n\x1a\nsecond";
/// What a webview's picture request accepts (Chromium's), and what `fetch()` does.
const IMAGE_ACCEPT: &str = "image/avif,image/webp,image/apng,image/svg+xml,image/*,*/*;q=0.8";
const FETCH_ACCEPT: &str = "*/*";
static VERSION: AtomicUsize = AtomicUsize::new(0);
static REQUESTS: AtomicUsize = AtomicUsize::new(0);

async fn origin() -> SocketAddr {
    // Version 0 has ETag "a"; after a bump it serves "b". A matching If-None-Match gets 304.
    async fn cover(headers: HeaderMap) -> Response {
        REQUESTS.fetch_add(1, Ordering::SeqCst);
        let (etag, body) = if VERSION.load(Ordering::SeqCst) == 0 {
            ("\"a\"", PNG_A)
        } else {
            ("\"b\"", PNG_B)
        };
        if headers
            .get(header::IF_NONE_MATCH)
            .is_some_and(|value| value == etag)
        {
            return axum::http::StatusCode::NOT_MODIFIED.into_response();
        }
        (
            [(header::CONTENT_TYPE, "image/png"), (header::ETAG, etag)],
            body,
        )
            .into_response()
    }
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move {
        axum::serve(listener, Router::new().route("/cover.png", get(cover)))
            .await
            .unwrap()
    });
    address
}

/// A source serving one JPEG at `/cover.jpg`, counting requests.
async fn jpeg_origin(jpeg: Vec<u8>) -> (SocketAddr, Arc<AtomicUsize>) {
    async fn cover(State((jpeg, requests)): State<(Arc<Vec<u8>>, Arc<AtomicUsize>)>) -> Response {
        requests.fetch_add(1, Ordering::SeqCst);
        (
            [
                (header::CONTENT_TYPE, "image/jpeg"),
                (header::ETAG, "\"j\""),
            ],
            jpeg.as_ref().clone(),
        )
            .into_response()
    }
    let requests = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/cover.jpg", get(cover))
        .with_state((Arc::new(jpeg), requests.clone()));
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (address, requests)
}

async fn settle_checks(cache: &CoverCache) {
    for _ in 0..100 {
        if cache.checking.lock().unwrap().is_empty() {
            return;
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
}

/// Waits until the conversion queue is empty and its worker idle.
async fn settle_conversions(cache: &CoverCache) {
    for _ in 0..500 {
        {
            let queue = cache.jxl.queue.lock().unwrap();
            if !queue.running && queue.order.is_empty() && queue.queued.is_empty() {
                return;
            }
        }
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    panic!("conversions didn't finish");
}

#[tokio::test]
async fn covers_come_from_disk_and_are_checked_again_when_old() {
    let origin = origin().await;
    let temp = tempfile::TempDir::new().unwrap();
    let cache = CoverCache::new(temp.path().join("covers"), 1 << 20, Duration::ZERO);
    let client = crate::http_client().unwrap();
    let url = format!("http://{origin}/cover.png");
    let headers = BTreeMap::new();

    assert_eq!(
        cache
            .get(&client, "ab", &url, &headers, false)
            .await
            .unwrap(),
        PNG_A
    );
    assert_eq!(REQUESTS.load(Ordering::SeqCst), 1);
    // Refresh off: the copy is used as is.
    assert_eq!(
        cache
            .get(&client, "ab", &url, &headers, false)
            .await
            .unwrap(),
        PNG_A
    );
    assert_eq!(REQUESTS.load(Ordering::SeqCst), 1);

    // Refresh on (everything is old): unchanged answers 304 and keeps the copy.
    cache.set_limits(1 << 20, Duration::from_millis(1));
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(
        cache
            .get(&client, "ab", &url, &headers, false)
            .await
            .unwrap(),
        PNG_A
    );
    settle_checks(&cache).await;
    assert_eq!(REQUESTS.load(Ordering::SeqCst), 2);

    // The source changed the image in place: the old copy shows once, then the new one.
    VERSION.store(1, Ordering::SeqCst);
    tokio::time::sleep(Duration::from_millis(5)).await;
    assert_eq!(
        cache
            .get(&client, "ab", &url, &headers, false)
            .await
            .unwrap(),
        PNG_A
    );
    settle_checks(&cache).await;
    cache.set_limits(1 << 20, Duration::ZERO);
    assert_eq!(
        cache
            .get(&client, "ab", &url, &headers, false)
            .await
            .unwrap(),
        PNG_B
    );

    // Kept covers live apart and move back when no longer kept.
    assert_eq!(
        cache
            .get(&client, "ab", &url, &headers, true)
            .await
            .unwrap(),
        PNG_B
    );
    assert!(temp.path().join("covers/keep/ab").exists());
    assert!(!temp.path().join("covers/ab").exists());
    assert_eq!(cache.sync_kept(HashSet::new()).await, (0, 1));
    assert!(temp.path().join("covers/ab").exists());
    // The library's covers move into keep/ at the sweep, even before they're seen again.
    assert_eq!(
        cache.sync_kept(HashSet::from(["ab".to_owned()])).await,
        (1, 0)
    );
    assert!(temp.path().join("covers/keep/ab").exists());
    assert_eq!(cache.sync_kept(HashSet::new()).await, (0, 1));

    // Not a picture worth converting: kept as it came, and recorded so.
    settle_conversions(&cache).await;
    let usage = cache.usage().await;
    assert_eq!((usage.files, usage.bytes), (1, PNG_B.len() as u64));
    assert_eq!(cache.read_meta("ab").await.stored, Stored::Kept);
    assert_eq!(cache.saving().await, Saving::default());
    cache.clear().await;
    assert_eq!(cache.usage().await, Usage::default());
    assert!(
        cache
            .get(&client, "../x", &url, &headers, false)
            .await
            .is_err()
    );
}

/// The first tier of what's kept about songs: pictures go least recently used first once the
/// cache is full, except the ones in use and the kept ones.
#[tokio::test]
async fn trimming_takes_the_oldest_pictures_but_not_those_in_use() {
    let temp = tempfile::TempDir::new().unwrap();
    let cache = CoverCache::new(temp.path().join("covers"), 1 << 20, Duration::ZERO);
    let meta = Meta::default();
    let ten = b"0123456789";
    for (token, kept) in [("aa", false), ("bb", false), ("cc", false), ("ee", true)] {
        cache.store(token, kept, ten, &meta).await;
    }
    // Seen long ago, oldest first: aa (playing), bb, cc.
    for (seconds, token) in [(1, "aa"), (2, "bb"), (3, "cc")] {
        std::fs::File::options()
            .write(true)
            .open(temp.path().join("covers").join(token))
            .unwrap()
            .set_modified(UNIX_EPOCH + Duration::from_secs(seconds))
            .unwrap();
    }
    cache.spare_with(|| HashSet::from(["aa".to_owned()]));
    // The kept one counts toward the limit: 30 bytes for the rest, trimmed to 90 %.
    cache.set_limits(40, Duration::ZERO);
    cache.store("dd", false, ten, &meta).await;
    let left = |token: &str| temp.path().join("covers").join(token).exists();
    assert!(left("aa"), "the picture in use stays");
    assert!(!left("bb") && !left("cc"), "the least recently seen go");
    assert!(left("dd"));
    assert!(temp.path().join("covers/keep/ee").exists());
}

#[test]
fn check_records_round_trip() {
    let meta = Meta {
        checked_ms: 42,
        etag: Some("\"x\"".into()),
        last_modified: None,
        stored: Stored::Jxl(1234),
    };
    assert_eq!(Meta::decode(&meta.encode()), meta);
    assert_eq!(Meta::decode(""), Meta::default());
    for stored in [Stored::Pending, Stored::Kept, Stored::Jxl(7)] {
        let meta = Meta {
            stored,
            ..Meta::default()
        };
        assert_eq!(Meta::decode(&meta.encode()), meta);
    }
    // Records from before JPEG XL have three lines: their copies wait to be converted.
    assert_eq!(
        Meta::decode("42\n\"x\"\nTue, 01 Jan 2030 00:00:00 GMT\n"),
        Meta {
            checked_ms: 42,
            etag: Some("\"x\"".into()),
            last_modified: Some("Tue, 01 Jan 2030 00:00:00 GMT".into()),
            stored: Stored::Pending,
        }
    );
}

/// A JPEG is served as it came, then stored as JPEG XL once, in the background; requests that
/// take JPEG XL get that file, the others the original JPEG rebuilt.
#[tokio::test]
async fn covers_are_stored_as_jpeg_xl_once_and_served_as_each_request_takes_them() {
    let jpeg = jpeg_of(&photo(320, 240));
    let (origin, requests) = jpeg_origin(jpeg.clone()).await;
    let temp = tempfile::TempDir::new().unwrap();
    let cache = CoverCache::new(temp.path().join("covers"), 1 << 24, Duration::ZERO);
    let client = crate::http_client().unwrap();
    let url = format!("http://{origin}/cover.jpg");
    let headers = BTreeMap::new();

    let first = cache
        .get_served(&client, "c0", &url, &headers, false, IMAGE_ACCEPT)
        .await
        .unwrap();
    assert_eq!(first.bytes, jpeg, "the first view gets the original");
    assert_eq!(first.content_type, "image/jpeg");
    settle_conversions(&cache).await;
    let stored = std::fs::read(temp.path().join("covers/c0")).unwrap();
    assert!(is_jxl(&stored), "the copy on disk is JPEG XL");
    assert!(stored.len() < jpeg.len());
    let meta = cache.read_meta("c0").await;
    assert_eq!(meta.stored, Stored::Jxl(jpeg.len() as u64));
    assert_eq!(
        meta.etag.as_deref(),
        Some("\"j\""),
        "the check record stays"
    );
    assert_eq!(cache.jxl.runs.load(Ordering::SeqCst), 1);

    // A webview without JPEG XL, and fetches from Rust or scripts: the JPEG, rebuilt.
    for accept in [IMAGE_ACCEPT, FETCH_ACCEPT, ""] {
        let served = cache
            .get_served(&client, "c0", &url, &headers, false, accept)
            .await
            .unwrap();
        assert_eq!(served.bytes, jpeg, "{accept}");
        assert_eq!(served.content_type, "image/jpeg");
    }
    // A request that lists JPEG XL gets it whatever the probe said.
    let listed = cache
        .get_served(&client, "c0", &url, &headers, false, "image/jxl,image/*")
        .await
        .unwrap();
    assert_eq!(
        (listed.bytes, listed.content_type),
        (stored.clone(), JXL_TYPE)
    );
    // Once the probe says the webview shows it, its picture requests get the file as it is,
    // while fetches still get the JPEG.
    cache.set_webview_jxl(true);
    let shown = cache
        .get_served(&client, "c0", &url, &headers, false, IMAGE_ACCEPT)
        .await
        .unwrap();
    assert_eq!((shown.bytes, shown.content_type), (stored, JXL_TYPE));
    let fetched = cache
        .get_served(&client, "c0", &url, &headers, false, FETCH_ACCEPT)
        .await
        .unwrap();
    assert_eq!(fetched.bytes, jpeg);

    // Seen again and again: never converted twice, never fetched again.
    for _ in 0..3 {
        cache
            .get(&client, "c0", &url, &headers, false)
            .await
            .unwrap();
        cache.convert_later("c0");
    }
    settle_conversions(&cache).await;
    assert_eq!(cache.jxl.runs.load(Ordering::SeqCst), 1);
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    let saving = cache.saving().await;
    assert_eq!(saving.files, 1);
    assert_eq!(saving.original_bytes, jpeg.len() as u64);
    assert!(saving.bytes < saving.original_bytes);
}

/// Copies from before JPEG XL (three-line records) convert when next seen, kept ones too, and a
/// link from an earlier start finds its JPEG XL copy.
#[tokio::test]
async fn old_copies_convert_when_next_seen() {
    let jpeg = jpeg_of(&photo(240, 240));
    let temp = tempfile::TempDir::new().unwrap();
    let dir = temp.path().join("covers");
    std::fs::create_dir_all(dir.join(KEEP)).unwrap();
    std::fs::create_dir_all(dir.join(META)).unwrap();
    std::fs::write(dir.join(KEEP).join("d1"), &jpeg).unwrap();
    std::fs::write(
        dir.join(META).join("d1"),
        format!("{}\n\"e\"\n\n", now_ms()),
    )
    .unwrap();
    let cache = CoverCache::new(dir.clone(), 1 << 24, Duration::from_secs(3600));
    let client = crate::http_client().unwrap();
    // Nothing answers there: the copy on disk is all there is.
    let url = "http://127.0.0.1:9/cover.jpg";

    let first = cache
        .get(&client, "d1", url, &BTreeMap::new(), true)
        .await
        .unwrap();
    assert_eq!(first, jpeg);
    settle_conversions(&cache).await;
    assert!(is_jxl(&std::fs::read(dir.join(KEEP).join("d1")).unwrap()));
    let meta = cache.read_meta("d1").await;
    assert_eq!(meta.etag.as_deref(), Some("\"e\""));
    assert_eq!(meta.stored, Stored::Jxl(jpeg.len() as u64));

    // An earlier start's link: served from disk, rebuilt for a webview without JPEG XL.
    let served = cache.stored_served("d1", IMAGE_ACCEPT).await.unwrap();
    assert_eq!((served.bytes, served.content_type), (jpeg, "image/jpeg"));
    assert!(cache.stored_served("00ff", IMAGE_ACCEPT).await.is_none());
}

/// A conversion of a copy that changed while it ran is dropped: the newer copy stays.
#[tokio::test]
async fn a_copy_that_changed_meanwhile_is_not_replaced() {
    let older = jpeg_of(&photo(200, 200));
    let newer = jpeg_of(&photo(220, 200));
    let temp = tempfile::TempDir::new().unwrap();
    let cache = CoverCache::new(temp.path().join("covers"), 1 << 24, Duration::ZERO);
    cache.store("e1", false, &newer, &Meta::default()).await;
    settle_conversions(&cache).await;
    let converted = std::fs::read(temp.path().join("covers/e1")).unwrap();
    // A conversion of the older picture, finishing late.
    let late = cover_format::convert(&older);
    let checker = cache.clone();
    tokio::task::spawn_blocking(move || checker.swap("e1", &older, Some(late)))
        .await
        .unwrap();
    assert_eq!(
        std::fs::read(temp.path().join("covers/e1")).unwrap(),
        converted
    );
    let rebuilt = cover_format::displayable(&converted).unwrap();
    assert_eq!(rebuilt.bytes, newer);
}

/// A stored JPEG XL that can't be read is dropped and fetched again from its source.
#[tokio::test]
async fn an_unreadable_copy_is_fetched_again() {
    let jpeg = jpeg_of(&photo(160, 160));
    let (origin, requests) = jpeg_origin(jpeg.clone()).await;
    let temp = tempfile::TempDir::new().unwrap();
    let dir = temp.path().join("covers");
    std::fs::create_dir_all(dir.join(META)).unwrap();
    std::fs::write(dir.join("f1"), b"\xFF\x0Abroken").unwrap();
    std::fs::write(dir.join(META).join("f1"), Meta::default().encode()).unwrap();
    std::fs::write(dir.join("f2"), b"\xFF\x0Abroken").unwrap();
    let cache = CoverCache::new(dir.clone(), 1 << 24, Duration::ZERO);
    let client = crate::http_client().unwrap();
    let url = format!("http://{origin}/cover.jpg");

    let served = cache
        .get_served(&client, "f1", &url, &BTreeMap::new(), false, IMAGE_ACCEPT)
        .await
        .unwrap();
    assert_eq!((served.bytes, served.content_type), (jpeg, "image/jpeg"));
    assert_eq!(requests.load(Ordering::SeqCst), 1);
    // Without its address, the broken copy is only dropped.
    assert!(cache.stored_served("f2", IMAGE_ACCEPT).await.is_none());
    assert!(!dir.join("f2").exists());
}

#[test]
fn versions_made_for_other_webviews_stay_within_their_memory() {
    let shown = |size: usize| {
        Arc::new(Shown {
            bytes: vec![0; size],
            content_type: "image/jpeg",
        })
    };
    let mut showing = Showing::default();
    let quarter = SHOWN_BYTES / 4;
    for index in 0..8 {
        showing.put(format!("t{index}:0"), shown(quarter - 1));
        assert!(showing.bytes <= SHOWN_BYTES);
    }
    assert_eq!(showing.kept.len(), 4);
    // The least recently used went first; one seen again stays.
    assert!(showing.get("t4:0").is_some());
    showing.put("t8:0".into(), shown(quarter - 1));
    assert!(showing.get("t4:0").is_some());
    assert!(showing.get("t5:0").is_none());
    // A picture too large to keep isn't.
    showing.put("big:0".into(), shown(quarter + 1));
    assert!(showing.get("big:0").is_none());
    showing.forget("t4");
    assert!(showing.get("t4:0").is_none());
    assert_eq!(
        showing.bytes,
        showing
            .kept
            .values()
            .map(|(shown, _)| shown.bytes.len())
            .sum::<usize>()
    );
}

/// Converting rests while the app's gate holds (battery saver, in the background with nothing
/// playing) and picks up again after; covers asked for meanwhile are served as they came.
#[tokio::test]
async fn conversions_rest_while_the_app_says_so_and_pick_up_after() {
    use futures_util::FutureExt;
    let (resting, rest) = tokio::sync::watch::channel(true);
    let temp = tempfile::TempDir::new().unwrap();
    let cache = CoverCache::new(temp.path().join("covers"), 1 << 24, Duration::ZERO);
    cache.convert_when(move || {
        let mut rest = rest.clone();
        async move {
            while *rest.borrow_and_update() {
                if rest.changed().await.is_err() {
                    return;
                }
            }
        }
        .boxed()
    });
    let jpeg = jpeg_of(&photo(240, 180));
    cache.store("b0", false, &jpeg, &Meta::default()).await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(cache.jxl.runs.load(Ordering::SeqCst), 0, "resting");
    let served = cache.stored_served("b0", IMAGE_ACCEPT).await.unwrap();
    assert_eq!(
        (served.bytes, served.content_type),
        (jpeg.clone(), "image/jpeg")
    );
    assert!(cache.jxl.queue.lock().unwrap().running, "the worker waits");

    resting.send(false).unwrap();
    settle_conversions(&cache).await;
    assert_eq!(cache.jxl.runs.load(Ordering::SeqCst), 1);
    let stored = std::fs::read(temp.path().join("covers/b0")).unwrap();
    assert!(is_jxl(&stored), "converted once the rest ended");

    // Resting again holds the next one until it ends.
    resting.send(true).unwrap();
    cache
        .store("b1", false, &jpeg_of(&photo(200, 200)), &Meta::default())
        .await;
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(cache.jxl.runs.load(Ordering::SeqCst), 1);
    resting.send(false).unwrap();
    settle_conversions(&cache).await;
    assert_eq!(cache.jxl.runs.load(Ordering::SeqCst), 2);
}

#[test]
fn requests_take_jpeg_xl_when_they_list_it_or_the_webview_shows_it() {
    assert!(takes_jxl("image/jxl,image/avif,image/*,*/*;q=0.8", false));
    assert!(!takes_jxl(IMAGE_ACCEPT, false));
    assert!(takes_jxl(IMAGE_ACCEPT, true));
    // Fetches and platform loaders get a widely shown format.
    assert!(!takes_jxl(FETCH_ACCEPT, true));
    assert!(!takes_jxl("", true));
    assert!(!takes_jxl(
        "text/html, image/gif, image/jpeg, *; q=.2",
        true
    ));
    // Refused outright.
    assert!(!takes_jxl("image/jxl;q=0, image/jpeg", false));
}

/// Covers stored before JPEG XL convert without being asked for again: the webview keeps covers
/// it showed and never asks for them, so the start's pass queues them.
#[tokio::test]
async fn covers_stored_before_jpeg_xl_convert_without_being_seen_again() {
    let temp = tempfile::TempDir::new().unwrap();
    let dir = temp.path().join("covers");
    std::fs::create_dir_all(dir.join(KEEP)).unwrap();
    std::fs::create_dir_all(dir.join(META)).unwrap();
    // An older cache: the pictures and their records, no conversion recorded.
    std::fs::write(dir.join("c0"), jpeg_of(&photo(240, 180))).unwrap();
    std::fs::write(dir.join(KEEP).join("c1"), jpeg_of(&photo(200, 200))).unwrap();
    for token in ["c0", "c1"] {
        std::fs::write(dir.join(META).join(token), Meta::default().encode()).unwrap();
    }
    let cache = CoverCache::new(dir.clone(), 1 << 24, Duration::ZERO);
    assert_eq!(cache.convert_pending().await, 2);
    settle_conversions(&cache).await;
    assert!(is_jxl(&std::fs::read(dir.join("c0")).unwrap()));
    assert!(is_jxl(&std::fs::read(dir.join(KEEP).join("c1")).unwrap()));
    // Looked at once: nothing is queued again.
    assert_eq!(cache.convert_pending().await, 0);
}

/// A source whose covers take `delay` each (`/a/<name>`) and one that never answers
/// (`/stuck.png`), counting requests.
async fn slow_origin(delay: Duration) -> (SocketAddr, Arc<AtomicUsize>) {
    async fn cover(State((delay, requests)): State<(Duration, Arc<AtomicUsize>)>) -> Response {
        requests.fetch_add(1, Ordering::SeqCst);
        tokio::time::sleep(delay).await;
        ([(header::CONTENT_TYPE, "image/png")], PNG_A).into_response()
    }
    async fn stuck() -> Response {
        tokio::time::sleep(Duration::from_secs(600)).await;
        ([(header::CONTENT_TYPE, "image/png")], PNG_A).into_response()
    }
    let requests = Arc::new(AtomicUsize::new(0));
    let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
        .await
        .unwrap();
    let address = listener.local_addr().unwrap();
    let router = Router::new()
        .route("/a/{name}", get(cover))
        .route("/stuck.png", get(stuck))
        .with_state((delay, requests.clone()));
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    (address, requests)
}

/// A playlist's rows as a webview asks for their covers: in row order, `connections` at a time
/// (a webview opens six to one host). Returns when each row's cover arrived, from the first ask.
async fn rows_loaded(
    cache: &CoverCache,
    rows: &[(String, String)],
    connections: usize,
) -> Vec<Duration> {
    let client = crate::http_client().unwrap();
    let slots = Arc::new(tokio::sync::Semaphore::new(connections));
    let start = std::time::Instant::now();
    let asks = rows.iter().map(|(token, url)| {
        let (cache, client, slots) = (cache.clone(), client.clone(), slots.clone());
        let (token, url) = (token.clone(), url.clone());
        async move {
            let _slot = slots.acquire_owned().await.unwrap();
            let _ = cache
                .get(&client, &token, &url, &BTreeMap::new(), false)
                .await;
            start.elapsed()
        }
    });
    futures_util::future::join_all(asks).await
}

/// Rows of the same album share a cover: it's fetched once however many rows ask at once (an
/// album-heavy playlist asked its sources for each row's copy before).
#[tokio::test]
async fn rows_sharing_a_cover_fetch_it_once() {
    let (origin, requests) = slow_origin(Duration::from_millis(150)).await;
    let temp = tempfile::TempDir::new().unwrap();
    let cache = CoverCache::new(temp.path().join("covers"), 1 << 24, Duration::ZERO);
    // 24 rows: three albums of eight songs.
    let rows: Vec<(String, String)> = (0..24)
        .map(|row| {
            let album = row / 8;
            (
                format!("a{album}"),
                format!("http://{origin}/a/{album}.png"),
            )
        })
        .collect();
    let times = rows_loaded(&cache, &rows, 6).await;
    let last = times.iter().max().unwrap();
    eprintln!(
        "shared covers: {} fetches, last row after {} ms",
        requests.load(Ordering::SeqCst),
        last.as_millis()
    );
    assert_eq!(requests.load(Ordering::SeqCst), 3);
}

/// A cover host that never answers gives up after the cache's timeout, so it doesn't keep one of
/// the webview's few connections to the proxy (and the rows after it) waiting.
#[tokio::test]
async fn a_cover_host_that_never_answers_gives_up() {
    let (origin, _) = slow_origin(Duration::from_millis(20)).await;
    let temp = tempfile::TempDir::new().unwrap();
    let cache = CoverCache::new(temp.path().join("covers"), 1 << 24, Duration::ZERO)
        .with_timeout(Duration::from_millis(300));
    // Two stuck covers in the first rows, with two connections: every row after waits on them.
    let mut rows: Vec<(String, String)> = (0..2)
        .map(|row| {
            (
                format!("5{row}"),
                format!("http://{origin}/stuck.png?{row}"),
            )
        })
        .collect();
    rows.extend((0..10).map(|row| (format!("b{row}"), format!("http://{origin}/a/{row}.png"))));
    let times = rows_loaded(&cache, &rows, 2).await;
    let rest = times[2..].iter().max().unwrap();
    eprintln!(
        "behind stuck covers: they gave up after {} ms, the other rows loaded by {} ms",
        times[0].as_millis(),
        rest.as_millis()
    );
    assert!(times[0] < Duration::from_secs(2), "{:?}", times[0]);
    assert!(*rest < Duration::from_secs(2), "{rest:?}");
}

/// Offline Mode: stored covers are served as they are (old ones aren't checked) and nothing is
/// fetched; a cover never stored fails with Offline Mode's message, for its placeholder.
#[tokio::test]
async fn offline_mode_serves_stored_covers_and_fetches_none() {
    let _switched = crate::network_mode::tests::switched();
    let jpeg = jpeg_of(&photo(64, 48));
    let (origin, requests) = jpeg_origin(jpeg.clone()).await;
    let client = crate::network_mode::tests::client_reaching("covers.example", origin);
    let temp = tempfile::TempDir::new().unwrap();
    // Everything counts as old: an online view would check it again.
    let cache = CoverCache::new(
        temp.path().join("covers"),
        1 << 24,
        Duration::from_millis(1),
    );
    let url = format!("http://covers.example:{}/cover.jpg", origin.port());
    let headers = BTreeMap::new();
    assert_eq!(
        cache
            .get(&client, "d0", &url, &headers, false)
            .await
            .unwrap(),
        jpeg
    );
    assert_eq!(requests.load(Ordering::SeqCst), 1);

    crate::network_mode::set_offline(true);
    tokio::time::sleep(Duration::from_millis(5)).await;
    let stored = cache
        .get(&client, "d0", &url, &headers, false)
        .await
        .unwrap();
    assert!(!stored.is_empty());
    settle_checks(&cache).await;
    let error = cache
        .get(&client, "d1", &format!("{url}?other"), &headers, false)
        .await
        .unwrap_err();
    assert!(crate::network_mode::mentions_offline(&error), "{error}");
    assert_eq!(requests.load(Ordering::SeqCst), 1, "nothing was sent");
}

#[test]
fn data_saver_asks_for_smaller_covers_where_addresses_name_a_size() {
    assert_eq!(
        smaller_version("https://is1-ssl.mzstatic.com/image/thumb/a/600x600bb.jpg").as_deref(),
        Some("https://is1-ssl.mzstatic.com/image/thumb/a/300x300bb.jpg")
    );
    assert_eq!(
        smaller_version(
            "https://e-cdns-images.dzcdn.net/images/cover/ab/1000x1000-000000-80-0-0.jpg"
        )
        .as_deref(),
        Some("https://e-cdns-images.dzcdn.net/images/cover/ab/300x300-000000-80-0-0.jpg")
    );
    assert_eq!(
        smaller_version("https://i.ytimg.com/vi/abc/maxresdefault.jpg").as_deref(),
        Some("https://i.ytimg.com/vi/abc/mqdefault.jpg")
    );
    assert_eq!(
        smaller_version("https://image.tmdb.org/t/p/original/x.jpg").as_deref(),
        Some("https://image.tmdb.org/t/p/w342/x.jpg")
    );
    assert_eq!(
        smaller_version("https://coverartarchive.org/release/1/front").as_deref(),
        Some("https://coverartarchive.org/release/1/front-250")
    );
    // Already small, or no size in the address.
    assert_eq!(
        smaller_version("https://is1-ssl.mzstatic.com/a/100x100bb.jpg"),
        None
    );
    assert_eq!(
        smaller_version("https://image.tmdb.org/t/p/w185/x.jpg"),
        None
    );
    assert_eq!(smaller_version("https://example.com/cover.jpg"), None);
}
