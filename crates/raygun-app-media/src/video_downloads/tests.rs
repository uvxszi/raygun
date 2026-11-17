use std::collections::BTreeMap;
use std::sync::Mutex as StdMutex;

use axum::Router;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
use axum::response::{IntoResponse, Response};
use tempfile::TempDir;

use super::*;

/// A film of `FILM_LEN` patterned bytes, served with Range support.
const FILM_LEN: usize = 100_000;

fn film() -> Vec<u8> {
    (0..FILM_LEN).map(|index| (index % 251) as u8).collect()
}

/// A request the test server saw: its path and Range header.
type Request = (String, Option<String>);

#[derive(Clone, Default)]
struct Seen(Arc<StdMutex<Vec<Request>>>);

/// Serves `/film.mp4` (with ranges) and a small HLS stream under `/hls/`.
async fn serve(State(seen): State<Seen>, uri: Uri, headers: HeaderMap) -> Response {
    let range = headers
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    seen.0
        .lock()
        .unwrap()
        .push((uri.path().to_owned(), range.clone()));
    let text = |body: &str| {
        (
            [(header::CONTENT_TYPE, "application/vnd.apple.mpegurl")],
            body.to_owned(),
        )
            .into_response()
    };
    match uri.path() {
        "/film.mp4" => {
            let body = film();
            match range.and_then(|range| {
                range
                    .strip_prefix("bytes=")?
                    .strip_suffix('-')?
                    .parse::<usize>()
                    .ok()
            }) {
                Some(start) if start >= body.len() => {
                    StatusCode::RANGE_NOT_SATISFIABLE.into_response()
                }
                Some(start) => (
                    StatusCode::PARTIAL_CONTENT,
                    [(
                        header::CONTENT_RANGE,
                        format!("bytes {start}-{}/{}", body.len() - 1, body.len()),
                    )],
                    body[start..].to_vec(),
                )
                    .into_response(),
                None => body.into_response(),
            }
        }
        "/hls/master.m3u8" => text(
            "#EXTM3U\n\
             #EXT-X-STREAM-INF:BANDWIDTH=500000,RESOLUTION=640x360\n\
             low.m3u8\n\
             #EXT-X-STREAM-INF:BANDWIDTH=1500000,RESOLUTION=1280x720\n\
             mid.m3u8\n",
        ),
        "/hls/mid.m3u8" => text(
            "#EXTM3U\n\
             #EXT-X-TARGETDURATION:4\n\
             #EXT-X-KEY:METHOD=AES-128,URI=\"key.bin\"\n\
             #EXTINF:4,\n\
             mid/1.ts\n\
             #EXTINF:4,\n\
             mid/2.ts\n\
             #EXT-X-ENDLIST\n",
        ),
        "/hls/live.m3u8" => text("#EXTM3U\n#EXTINF:4,\nmid/1.ts\n"),
        "/hls/key.bin" => b"0123456789abcdef".to_vec().into_response(),
        "/hls/mid/1.ts" => b"segment one".to_vec().into_response(),
        "/hls/mid/2.ts" => b"segment two".to_vec().into_response(),
        _ => StatusCode::NOT_FOUND.into_response(),
    }
}

async fn server() -> (String, Seen) {
    let seen = Seen::default();
    let app = Router::new().fallback(serve).with_state(seen.clone());
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
    let address = listener.local_addr().unwrap();
    tokio::spawn(async move { axum::serve(listener, app).await.unwrap() });
    (format!("http://{address}"), seen)
}

fn job<'a>(
    client: &'a reqwest::Client,
    dir: &'a Path,
    headers: &'a BTreeMap<String, String>,
    stopped: &'a (dyn Fn() -> bool + Sync),
    progress: &'a Progress,
) -> Job<'a> {
    Job {
        client,
        dir,
        headers,
        preferred: raygun_model::VideoResolution::P720,
        stopped,
        progress,
        torrents: None,
    }
}

#[tokio::test]
async fn a_plain_file_continues_where_it_stopped() {
    let (base, seen) = server().await;
    let temp = TempDir::new().unwrap();
    // An earlier run saved the first 40 000 bytes.
    std::fs::write(temp.path().join("video.part"), &film()[..40_000]).unwrap();
    let (client, headers, progress) = (
        raygun_net::http_client().unwrap(),
        BTreeMap::new(),
        Progress::default(),
    );
    let running = || false;
    let job = job(&client, temp.path(), &headers, &running, &progress);

    let name = job
        .video(&format!("{base}/film.mp4"), None)
        .await
        .ok()
        .unwrap();
    assert_eq!(name, "video.mp4");
    assert_eq!(
        std::fs::read(temp.path().join("video.mp4")).unwrap(),
        film()
    );
    assert!(!temp.path().join("video.part").exists());
    assert_eq!(
        seen.0.lock().unwrap()[0],
        ("/film.mp4".into(), Some("bytes=40000-".into()))
    );
    assert_eq!(progress.bytes.load(Ordering::SeqCst), FILM_LEN as u64);
    assert_eq!(progress.total_bytes.load(Ordering::SeqCst), FILM_LEN as u64);
}

#[tokio::test]
async fn a_finished_partial_file_is_kept() {
    let (base, _) = server().await;
    let temp = TempDir::new().unwrap();
    std::fs::write(temp.path().join("video.part"), film()).unwrap();
    let (client, headers, progress) = (
        raygun_net::http_client().unwrap(),
        BTreeMap::new(),
        Progress::default(),
    );
    let running = || false;
    let job = job(&client, temp.path(), &headers, &running, &progress);
    assert_eq!(
        job.video(&format!("{base}/film.mp4"), None)
            .await
            .ok()
            .as_deref(),
        Some("video.mp4")
    );
    assert_eq!(
        std::fs::read(temp.path().join("video.mp4")).unwrap(),
        film()
    );
}

#[tokio::test]
async fn an_hls_stream_saves_its_variant_key_and_segments() {
    let (base, seen) = server().await;
    let temp = TempDir::new().unwrap();
    let (client, headers, progress) = (
        raygun_net::http_client().unwrap(),
        BTreeMap::new(),
        Progress::default(),
    );
    let running = || false;
    let job = job(&client, temp.path(), &headers, &running, &progress);

    let name = job
        .video(&format!("{base}/hls/master.m3u8"), Some("hls"))
        .await
        .ok()
        .unwrap();
    assert_eq!(name, HLS_INDEX);
    let read = |name: &str| std::fs::read_to_string(temp.path().join(name)).unwrap();
    assert_eq!(
        read(HLS_INDEX),
        "#EXTM3U\n#EXT-X-TARGETDURATION:4\n#EXT-X-KEY:METHOD=AES-128,URI=\"vkey1.key\"\n\
         #EXTINF:4,\nv00001.ts\n#EXTINF:4,\nv00002.ts\n#EXT-X-ENDLIST\n"
    );
    assert_eq!(read("vkey1.key"), "0123456789abcdef");
    assert_eq!(read("v00002.ts"), "segment two");
    assert_eq!(progress.parts.load(Ordering::SeqCst), 3);
    assert_eq!(progress.total_parts.load(Ordering::SeqCst), 3);
    let paths: Vec<String> = seen
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|(path, _)| path.clone())
        .collect();
    assert!(
        paths.contains(&"/hls/mid.m3u8".to_owned()),
        "the 720p variant"
    );
    assert!(!paths.contains(&"/hls/low.m3u8".to_owned()));

    // A second run (after a pause) fetches only the playlists again.
    seen.0.lock().unwrap().clear();
    job.video(&format!("{base}/hls/master.m3u8"), Some("hls"))
        .await
        .ok()
        .unwrap();
    let again: Vec<String> = seen
        .0
        .lock()
        .unwrap()
        .iter()
        .map(|(path, _)| path.clone())
        .collect();
    assert_eq!(again, ["/hls/master.m3u8", "/hls/mid.m3u8"]);
}

#[tokio::test]
async fn live_streams_fail_and_a_pause_stops_the_download() {
    let (base, _) = server().await;
    let temp = TempDir::new().unwrap();
    let (client, headers, progress) = (
        raygun_net::http_client().unwrap(),
        BTreeMap::new(),
        Progress::default(),
    );
    let running = || false;
    let live = job(&client, temp.path(), &headers, &running, &progress)
        .video(&format!("{base}/hls/live.m3u8"), Some("hls"))
        .await;
    assert!(matches!(live, Err(Failure::Error(message)) if message.contains("Live")));

    let paused = || true;
    let stopped = job(&client, temp.path(), &headers, &paused, &progress)
        .video(&format!("{base}/hls/master.m3u8"), Some("hls"))
        .await;
    assert!(matches!(stopped, Err(Failure::Stopped)));
    let dash = job(&client, temp.path(), &headers, &running, &progress)
        .video("https://cdn.example/a.mpd", None)
        .await;
    assert!(matches!(dash, Err(Failure::Error(_))));
    // A torrent goes to the torrent engine, never fetched as a web file.
    let torrent = job(&client, temp.path(), &headers, &running, &progress)
        .video(&format!("{base}/film.torrent"), None)
        .await;
    assert!(
        matches!(&torrent, Err(Failure::Error(message)) if message == crate::torrents::TORRENTS_OFF)
    );
}

#[test]
fn file_names_follow_the_links() {
    assert_eq!(
        video_extension("https://cdn.example/a/film.MKV?t=1"),
        ".mkv"
    );
    assert_eq!(video_extension("https://cdn.example/download/123"), ".mp4");
    assert_eq!(
        subtitle_format("https://subs.example/en.srt", "1\n00:00"),
        Some("srt".into())
    );
    assert_eq!(
        subtitle_format("https://subs.example/file/9", "WEBVTT\n\n"),
        Some("vtt".into())
    );
    assert_eq!(
        subtitle_format("https://subs.example/file/9", "1\n00:00"),
        None
    );
}

fn new_download(key: &str) -> NewDownload {
    NewDownload {
        key: key.into(),
        title_id: None,
        title: "Sintel".into(),
        subtitle: None,
        poster: None,
        url: "https://cdn.example/sintel.mp4".into(),
        headers: "{}".into(),
        format: None,
        subtitles: "[]".into(),
    }
}

#[test]
fn only_a_downloads_own_folder_is_ever_deleted() {
    assert!(is_download_id("0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b"));
    for id in ["", "..", "../music", "a/b", "a\\b", ".", "x.y"] {
        assert!(!is_download_id(id), "{id}");
    }
}

#[tokio::test]
async fn deleted_videos_leave_nothing_behind() {
    let temp = TempDir::new().unwrap();
    let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
        .await
        .unwrap();
    let root = temp.path().join("downloads");
    let folder = root.join(FOLDER);

    // A finished download with its files, one whose files went missing, and one queued.
    let kept = store::enqueue(&db, &new_download("source:a:1"), 1)
        .await
        .unwrap();
    store::set_done(&db, &kept, "video.mp4", "[]", 2)
        .await
        .unwrap();
    std::fs::create_dir_all(folder.join(&kept)).unwrap();
    std::fs::write(folder.join(&kept).join("video.mp4"), b"film").unwrap();
    let missing = store::enqueue(&db, &new_download("source:a:2"), 1)
        .await
        .unwrap();
    store::set_done(&db, &missing, "video.mp4", "[]", 2)
        .await
        .unwrap();
    let queued = store::enqueue(&db, &new_download("source:a:3"), 1)
        .await
        .unwrap();
    std::fs::create_dir_all(folder.join(&queued)).unwrap();
    // Folders of videos deleted earlier: a whole download with its subtitles and a torrent's
    // pieces, and an empty one; a stray file and a folder that isn't a download's stay.
    let gone = "0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b";
    std::fs::create_dir_all(folder.join(gone).join("torrent")).unwrap();
    std::fs::write(folder.join(gone).join("video.mkv"), vec![0u8; 1_000]).unwrap();
    std::fs::write(folder.join(gone).join("sub1.srt"), vec![0u8; 24]).unwrap();
    std::fs::write(
        folder.join(gone).join("torrent").join("piece"),
        vec![0u8; 76],
    )
    .unwrap();
    std::fs::create_dir_all(folder.join("0190a1b2-0000-7000-8000-000000000000")).unwrap();
    std::fs::write(folder.join("notes.txt"), b"x").unwrap();
    std::fs::create_dir_all(folder.join(".cache")).unwrap();

    let swept = sweep_in(&db, &root).await.unwrap();
    assert_eq!(swept.folders, 2);
    assert_eq!(swept.bytes, 1_100.0);
    assert_eq!(swept.entries, 1, "the finished download whose file is gone");
    assert!(!folder.join(gone).exists());
    assert!(folder.join(&kept).join("video.mp4").exists());
    assert!(
        folder.join(&queued).exists(),
        "a queued download keeps its folder"
    );
    assert!(folder.join("notes.txt").exists());
    assert!(folder.join(".cache").exists());
    assert!(store::get(&db, &missing).await.unwrap().is_none());
    assert!(store::get(&db, &kept).await.unwrap().is_some());

    // Nothing left at all: the video folder goes too.
    let empty = TempDir::new().unwrap();
    let only = empty.path().join(FOLDER).join(gone);
    std::fs::create_dir_all(&only).unwrap();
    let swept = sweep_in(&db, empty.path()).await.unwrap();
    assert_eq!(swept.folders, 1);
    assert!(!empty.path().join(FOLDER).exists());
}

#[tokio::test]
async fn a_folder_is_removed_whole_and_its_empty_parent_with_it() {
    let temp = TempDir::new().unwrap();
    let folder = temp.path().join(FOLDER);
    let dir = folder.join("0190a1b2-c3d4-7e5f-8a9b-0c1d2e3f4a5b");
    std::fs::create_dir_all(dir.join("torrent")).unwrap();
    std::fs::write(dir.join("index.m3u8"), b"#EXTM3U").unwrap();
    assert!(remove_tree(&dir).await);
    // Already gone counts as removed.
    assert!(remove_tree(&dir).await);
    remove_if_empty(&folder).await;
    assert!(!folder.exists());
    // A parent with something left in it stays.
    std::fs::create_dir_all(folder.join("other")).unwrap();
    remove_if_empty(&folder).await;
    assert!(folder.exists());
}
