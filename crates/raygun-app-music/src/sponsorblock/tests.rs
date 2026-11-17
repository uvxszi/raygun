use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::{Arc, Mutex};

use axum::extract::{Path as UrlPath, RawQuery, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use raygun_model::{SegmentAction, SegmentCategory, SponsorBlockPrefs};
use tempfile::TempDir;

use super::*;

const VIDEO: &str = "dQw4w9WgXcQ";

/// What the fake SponsorBlock was asked, and what it answers.
#[derive(Default)]
struct Fake {
    asked: Mutex<Vec<(String, String)>>,
    count: AtomicUsize,
    /// Answers with this status and no parts instead.
    status: Mutex<Option<u16>>,
}

fn answer(video: &str) -> String {
    serde_json::json!([
        {
            "videoID": "someOtherId",
            "segments": [{ "category": "sponsor", "actionType": "skip", "segment": [1.0, 9.0],
                "UUID": "other", "videoDuration": 100.0 }]
        },
        {
            "videoID": video,
            "segments": [
                { "category": "music_offtopic", "actionType": "skip", "segment": [180.5, 212.0],
                  "UUID": "outro", "videoDuration": 212.3 },
                { "category": "sponsor", "actionType": "skip", "segment": [0.0, 12.25],
                  "UUID": "intro", "videoDuration": 212.3 },
                { "category": "sponsor", "actionType": "mute", "segment": [50.0, 60.0],
                  "UUID": "muted", "videoDuration": 212.3 },
                { "category": "poi_highlight", "actionType": "poi", "segment": [70.0, 70.0],
                  "UUID": "highlight", "videoDuration": 212.3 }
            ]
        }
    ])
    .to_string()
}

async fn serve(fake: Arc<Fake>) -> String {
    async fn segments(
        State(fake): State<Arc<Fake>>,
        UrlPath(prefix): UrlPath<String>,
        RawQuery(query): RawQuery,
    ) -> axum::response::Response {
        fake.count.fetch_add(1, Ordering::SeqCst);
        fake.asked
            .lock()
            .unwrap()
            .push((prefix, query.unwrap_or_default()));
        if let Some(status) = *fake.status.lock().unwrap() {
            return StatusCode::from_u16(status).unwrap().into_response();
        }
        answer(VIDEO).into_response()
    }
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    let router = axum::Router::new()
        .route("/api/skipSegments/{prefix}", axum::routing::get(segments))
        .with_state(fake);
    tokio::spawn(async move { axum::serve(listener, router).await.unwrap() });
    format!("http://{address}")
}

fn service(base: &str, dir: Option<PathBuf>, conditions: Arc<Mutex<Conditions>>) -> SponsorBlock {
    SponsorBlock::with(
        base,
        raygun_net::http_client().unwrap(),
        dir,
        Box::new(move || *conditions.lock().unwrap()),
    )
}

fn ids(segments: &[Segment]) -> Vec<&str> {
    segments.iter().map(|segment| segment.id.as_str()).collect()
}

#[tokio::test]
async fn lookups_send_only_a_prefix_of_the_hash_and_keep_this_videos_skippable_parts() {
    let fake = Arc::new(Fake::default());
    let base = serve(fake.clone()).await;
    let sponsorblock = service(&base, None, Arc::default());
    let segments = sponsorblock.segments(VIDEO, Want::Now).await.unwrap();
    // In order; muted parts, highlights and other videos' parts left out; times in ms.
    assert_eq!(ids(&segments), ["intro", "outro"]);
    assert_eq!(segments[0].end_ms, 12_250.0);
    assert_eq!(segments[1].category, SegmentCategory::MusicOfftopic);
    assert_eq!(segments[1].video_ms, 212_300.0);
    let asked = fake.asked.lock().unwrap().clone();
    let (prefix, query) = &asked[0];
    assert_eq!(prefix, &hash_hex(VIDEO)[..4]);
    assert!(!query.contains(VIDEO), "the video id itself is never sent");
    let query = url::form_urlencoded::parse(query.as_bytes())
        .into_owned()
        .collect::<HashMap<String, String>>();
    assert!(query["categories"].contains("\"music_offtopic\""));
    assert_eq!(query["actionTypes"], r#"["skip"]"#);
}

#[tokio::test]
async fn parts_are_kept_a_day_on_disk_and_asked_for_again_after() {
    let fake = Arc::new(Fake::default());
    let base = serve(fake.clone()).await;
    let temp = TempDir::new().unwrap();
    let first = service(&base, Some(temp.path().to_owned()), Arc::default());
    first.segments(VIDEO, Want::Now).await.unwrap();
    first.segments(VIDEO, Want::Now).await.unwrap();
    assert_eq!(fake.count.load(Ordering::SeqCst), 1);
    assert_eq!(first.kept(VIDEO).map(|kept| kept.len()), Some(2));

    // Raygun started again: what's on disk answers.
    let again = service(&base, Some(temp.path().to_owned()), Arc::default());
    assert_eq!(
        ids(&again.segments(VIDEO, Want::Now).await.unwrap()),
        ["intro", "outro"]
    );
    assert_eq!(fake.count.load(Ordering::SeqCst), 1);

    // A day later it's asked for again.
    again.memory().get_mut(VIDEO).unwrap().fetched_ms -= FRESH_MS + 1;
    again.segments(VIDEO, Want::Now).await.unwrap();
    assert_eq!(fake.count.load(Ordering::SeqCst), 2);
}

#[tokio::test]
async fn offline_mode_asks_nothing_and_data_saver_keeps_to_what_it_has() {
    let fake = Arc::new(Fake::default());
    let base = serve(fake.clone()).await;
    let conditions = Arc::new(Mutex::new(Conditions {
        offline: true,
        ..Conditions::default()
    }));
    let sponsorblock = service(&base, None, conditions.clone());
    assert!(
        sponsorblock
            .segments(VIDEO, Want::Now)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(fake.count.load(Ordering::SeqCst), 0);

    // Data Saver asks for what plays, but not for songs ahead of their turn.
    *conditions.lock().unwrap() = Conditions {
        saving: true,
        no_ahead: true,
        ..Conditions::default()
    };
    assert!(
        sponsorblock
            .segments(VIDEO, Want::Ahead)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(fake.count.load(Ordering::SeqCst), 0);
    assert_eq!(
        sponsorblock.segments(VIDEO, Want::Now).await.unwrap().len(),
        2
    );
    assert_eq!(fake.count.load(Ordering::SeqCst), 1);

    // Stale parts stand in, offline and while saving data.
    sponsorblock.memory().get_mut(VIDEO).unwrap().fetched_ms -= FRESH_MS + 1;
    assert_eq!(
        sponsorblock.segments(VIDEO, Want::Now).await.unwrap().len(),
        2
    );
    conditions.lock().unwrap().offline = true;
    assert_eq!(
        sponsorblock.segments(VIDEO, Want::Now).await.unwrap().len(),
        2
    );
    assert_eq!(fake.count.load(Ordering::SeqCst), 1);
}

#[tokio::test]
async fn nothing_goes_to_disk_while_incognito() {
    let fake = Arc::new(Fake::default());
    let base = serve(fake.clone()).await;
    let temp = TempDir::new().unwrap();
    let conditions = Arc::new(Mutex::new(Conditions {
        incognito: true,
        ..Conditions::default()
    }));
    let sponsorblock = service(&base, Some(temp.path().to_owned()), conditions);
    sponsorblock.segments(VIDEO, Want::Now).await.unwrap();
    assert_eq!(std::fs::read_dir(temp.path()).map_or(0, Iterator::count), 0);
}

#[tokio::test]
async fn failures_fall_back_to_kept_parts_and_a_prefix_without_any_is_none() {
    let fake = Arc::new(Fake::default());
    let base = serve(fake.clone()).await;
    let sponsorblock = service(&base, None, Arc::default());
    *fake.status.lock().unwrap() = Some(500);
    let error = sponsorblock.segments(VIDEO, Want::Now).await.unwrap_err();
    assert!(error.contains("HTTP 500"), "{error}");

    *fake.status.lock().unwrap() = Some(404);
    assert!(
        sponsorblock
            .segments(VIDEO, Want::Now)
            .await
            .unwrap()
            .is_empty()
    );

    *fake.status.lock().unwrap() = None;
    sponsorblock.memory().clear();
    sponsorblock.segments(VIDEO, Want::Now).await.unwrap();
    sponsorblock.memory().get_mut(VIDEO).unwrap().fetched_ms -= FRESH_MS + 1;
    *fake.status.lock().unwrap() = Some(429);
    assert_eq!(
        sponsorblock.segments(VIDEO, Want::Now).await.unwrap().len(),
        2
    );
}

#[tokio::test]
async fn nothing_is_asked_while_sponsorblock_is_off_or_for_what_isnt_a_video() {
    let fake = Arc::new(Fake::default());
    let base = serve(fake.clone()).await;
    let sponsorblock = service(&base, None, Arc::default());
    assert!(
        sponsorblock
            .segments("not a video", Want::Now)
            .await
            .unwrap()
            .is_empty()
    );
    sponsorblock.set_prefs(SponsorBlockPrefs {
        enabled: false,
        ..SponsorBlockPrefs::default()
    });
    assert!(!sponsorblock.enabled());
    assert!(
        sponsorblock
            .segments(VIDEO, Want::Now)
            .await
            .unwrap()
            .is_empty()
    );
    assert_eq!(fake.count.load(Ordering::SeqCst), 0);
}

#[test]
fn parts_are_shown_as_the_settings_say_and_those_of_another_cut_left_out() {
    let parsed = parse(&answer(VIDEO), VIDEO).unwrap();
    let prefs = SponsorBlockPrefs {
        sponsor: SegmentAction::Show,
        ..SponsorBlockPrefs::default()
    };
    let parts = shown(&prefs, &parsed, Some(212_000.0));
    assert_eq!(parts.len(), 2);
    assert_eq!(parts[0].action, SegmentAction::Show);
    assert_eq!(parts[1].action, SegmentAction::Skip);
    // The video playing is 20 s longer than the one the parts were marked on.
    assert!(shown(&prefs, &parsed, Some(232_300.0)).is_empty());
    let off = SponsorBlockPrefs {
        music_offtopic: SegmentAction::Off,
        ..prefs
    };
    assert_eq!(shown(&off, &parsed, None).len(), 1);
}

#[test]
fn kept_parts_past_their_time_or_count_are_swept() {
    let temp = TempDir::new().unwrap();
    for name in ["a", "b", "c"] {
        std::fs::write(temp.path().join(format!("{name}.json")), "{}").unwrap();
    }
    sweep(temp.path(), 2, KEPT_MS);
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 2);
    sweep(temp.path(), 10, 0);
    std::thread::sleep(Duration::from_millis(5));
    sweep(temp.path(), 10, 0);
    assert_eq!(std::fs::read_dir(temp.path()).unwrap().count(), 0);
}

/// Asks SponsorBlock itself (`SPONSORBLOCK_VIDEO` names the video; by default a music video
/// whose intro and ending are marked non-music):
/// `cargo test -p raygun-app -- --ignored sponsorblock_live --nocapture`.
#[tokio::test]
#[ignore = "network"]
async fn sponsorblock_live() {
    let video = std::env::var("SPONSORBLOCK_VIDEO").unwrap_or_else(|_| "kJQP7kiw5Fk".to_owned());
    let sponsorblock = service(API, None, Arc::default());
    let segments = sponsorblock.segments(&video, Want::Now).await.unwrap();
    for segment in &segments {
        println!(
            "{:?} {:.1}–{:.1} s (marked on {:.1} s)",
            segment.category,
            segment.start_ms / 1000.0,
            segment.end_ms / 1000.0,
            segment.video_ms / 1000.0
        );
        assert!(segment.start_ms < segment.end_ms);
    }
    println!("{} parts of {video}", segments.len());
}
