//! Streams an openly licensed film over BitTorrent through the stream proxy, as the video player
//! reads it: the first megabyte, then a seek to the middle. Network; ignored by default:
//!
//! `cargo test -p raygun-torrent --test streaming_live -- --ignored --nocapture`
//!
//! `TORRENT_LINK` picks another torrent (a magnet link, or a `.torrent` file's address).

use std::sync::Arc;
use std::time::{Duration, Instant};

use raygun_torrent::{Engine, EngineConfig, Source, TorrentLink};

/// Sintel (Blender Foundation, CC BY 3.0), as WebTorrent's examples share it; found through
/// the DHT and these trackers.
const SINTEL: &str = "magnet:?xt=urn:btih:08ada5a7a6183aae1e09d831df6748d566095a10&dn=Sintel\
    &tr=udp%3A%2F%2Ftracker.opentrackr.org%3A1337%2Fannounce\
    &tr=udp%3A%2F%2Fexplodie.org%3A6969\
    &tr=udp%3A%2F%2Ftracker.empire-js.us%3A1337\
    &tr=udp%3A%2F%2Fopen.demonii.com%3A1337%2Fannounce";

const MB: u64 = 1024 * 1024;

struct Files(Arc<Engine>);

impl raygun_net::ArrivingFiles for Files {
    fn file_len(&self, token: &str) -> Option<u64> {
        self.0.file_len(token)
    }

    fn read_from(
        &self,
        token: &str,
        start: u64,
    ) -> futures_util::future::BoxFuture<'static, std::io::Result<raygun_net::FileReader>> {
        let engine = self.0.clone();
        let token = token.to_owned();
        Box::pin(async move { engine.read_from(&token, start).await })
    }
}

#[tokio::test(flavor = "multi_thread")]
#[ignore = "network: streams a film over BitTorrent"]
async fn streams_a_film_and_seeks_live() {
    let link = std::env::var("TORRENT_LINK").unwrap_or_else(|_| SINTEL.to_owned());
    let cache = tempfile::tempdir().unwrap();
    let engine = Arc::new(Engine::new(EngineConfig {
        dir: cache.path().join("torrents"),
        port: 0,
        dht: true,
        upload_limit: Some(256 * 1024),
        https_trackers: true,
    }));
    let parsed = raygun_torrent::parse(&link).unwrap();
    let hint = parsed.hint().clone();
    let source = match parsed {
        TorrentLink::Magnet(magnet) => Source::Magnet(magnet),
        TorrentLink::File { url, .. } => Source::File(
            reqwest::get(&url)
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .to_vec(),
        ),
    };
    let started = Instant::now();
    let opened = tokio::time::timeout(
        Duration::from_secs(180),
        engine.open(source, &hint, u64::MAX),
    )
    .await
    .expect("opened in time")
    .expect("the torrent opens");
    println!(
        "opened {} ({} MB) in {:?}",
        opened.name,
        opened.len / MB,
        started.elapsed()
    );

    let proxy = raygun_net::StreamProxy::start(raygun_net::http_client().unwrap(), None)
        .await
        .unwrap();
    let links = proxy.torrents();
    links.serve(Arc::new(Files(engine.clone())));
    let url = links.link(&opened.token, &opened.name);
    let client = reqwest::Client::new();

    // The start, as a player opens a file.
    let first = tokio::time::timeout(
        Duration::from_secs(300),
        client
            .get(&url)
            .header("Range", format!("bytes=0-{}", MB - 1))
            .send(),
    )
    .await
    .expect("answered in time")
    .unwrap();
    assert_eq!(first.status(), 206);
    let head = tokio::time::timeout(Duration::from_secs(300), first.bytes())
        .await
        .expect("the first megabyte in time")
        .unwrap();
    assert_eq!(head.len() as u64, MB);
    if opened.name.to_ascii_lowercase().ends_with(".mp4") {
        assert_eq!(&head[4..8], b"ftyp", "an MP4 starts with its file type box");
    }
    let progress = engine.progress(&opened.token).unwrap();
    println!(
        "first MB after {:?}: {} peers ({} seeds), {} KB/s",
        started.elapsed(),
        progress.peers,
        progress.seeds,
        progress.download / 1024
    );

    // The subtitle files that come with it (Sintel has nine), read whole as players read them:
    // through the proxy's `/subtitle/` route (`video::proxied_subtitles`), which reads the
    // file's own `/torrent/` address.
    if std::env::var("TORRENT_LINK").is_err() {
        let languages: Vec<&str> = opened
            .subtitles
            .iter()
            .filter_map(|subtitle| subtitle.language.as_deref())
            .collect();
        assert!(languages.contains(&"en"), "{languages:?}");
    }
    for subtitle in &opened.subtitles {
        let address = proxy.images().subtitle(
            &links.link(&subtitle.token, &subtitle.name),
            &Default::default(),
        );
        let text = tokio::time::timeout(Duration::from_secs(180), async {
            client.get(&address).send().await?.text().await
        })
        .await
        .expect("the subtitles in time")
        .unwrap();
        assert!(
            text.contains("-->"),
            "{}: {}",
            subtitle.name,
            text.chars().take(200).collect::<String>()
        );
        println!(
            "{} ({:?}, {}): {} bytes after {:?}",
            subtitle.name,
            subtitle.language,
            subtitle.format,
            text.len(),
            started.elapsed()
        );
    }

    // A seek to the middle: its pieces are fetched first.
    let middle = opened.len / 2;
    let seek = Instant::now();
    let part = client
        .get(&url)
        .header(
            "Range",
            format!("bytes={middle}-{}", middle + 256 * 1024 - 1),
        )
        .send()
        .await
        .unwrap();
    assert_eq!(part.status(), 206);
    assert_eq!(
        part.headers()["content-range"],
        format!("bytes {middle}-{}/{}", middle + 256 * 1024 - 1, opened.len).as_str()
    );
    let bytes = tokio::time::timeout(Duration::from_secs(300), part.bytes())
        .await
        .expect("the middle in time")
        .unwrap();
    assert_eq!(bytes.len(), 256 * 1024);
    println!("seek answered in {:?}", seek.elapsed());

    // Closing removes the torrent and its pieces, its subtitle files with it.
    engine.close(&opened.token, false).await;
    assert!(engine.file_len(&opened.token).is_none());
    assert!(
        opened
            .subtitles
            .iter()
            .all(|subtitle| engine.file_len(&subtitle.token).is_none())
    );
    assert!(
        !cache
            .path()
            .join("torrents")
            .join(&opened.info_hash)
            .exists()
    );
}
