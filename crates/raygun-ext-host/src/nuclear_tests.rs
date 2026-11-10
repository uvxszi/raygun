//! Real Nuclear plugins from the registry, converted and run (M17). Fixtures and their licenses:
//! tests/fixtures/nuclear/README.md.

use std::collections::BTreeMap;
use std::path::Path;

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use serde_json::json;

use crate::nuclear::{self, Converted};
use crate::{ExtensionHost, ExtensionSource, HostConfig, MANIFEST_FILE, Manifest};

fn fixture(name: &str) -> BTreeMap<String, String> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("tests/fixtures/nuclear")
        .join(format!("nuclear-plugin-{name}.zip"));
    nuclear::unzip_sources(&std::fs::read(path).unwrap()).unwrap()
}

fn extension(converted: &Converted) -> ExtensionSource {
    ExtensionSource::built_in(&[
        (MANIFEST_FILE, &converted.manifest),
        ("dist/index.js", &converted.bundle),
    ])
}

#[test]
fn unzipping_keeps_only_sources() {
    let files = fixture("kugou");
    assert!(files.contains_key("package.json"));
    assert!(files.contains_key("src/index.ts"));
    assert!(
        !files
            .keys()
            .any(|path| path.ends_with(".krc") || path.ends_with(".md")),
        "binary fixtures and docs are skipped: {:?}",
        files.keys().collect::<Vec<_>>()
    );
}

#[tokio::test]
async fn real_registry_plugins_convert_and_load() {
    let cases = [
        (
            "musicbrainz",
            "nuclear.nuclear-plugin-musicbrainz",
            "music.metadata",
        ),
        ("lrclib", "nuclear.nuclear-plugin-lrclib", "lyrics"),
        ("kugou", "nuclear.nuclear-plugin-kugou", "lyrics"),
        (
            "discogs",
            "nuclear.nuclear-plugin-discogs",
            "music.metadata",
        ),
    ];
    for (name, id, kind) in cases {
        let converted =
            nuclear::convert(&fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
        let manifest =
            Manifest::parse(&converted.manifest).unwrap_or_else(|error| panic!("{name}: {error}"));
        assert_eq!(manifest.id, id);
        assert!(
            manifest.provides.iter().any(|provided| provided == kind),
            "{name}"
        );
        assert!(!converted.report.hosts.is_empty(), "{name}: hosts detected");
        println!("{name}: {:?}", converted.report);

        let host = ExtensionHost::start(HostConfig::default()).unwrap();
        let infos = host.load(vec![extension(&converted)]).await;
        let info = &infos[0];
        assert_eq!(
            info.status,
            raygun_model::ExtensionStatus::Loaded,
            "{name}: {:?}",
            info.error
        );
        assert!(
            info.providers.iter().any(|provider| provider.kind == kind),
            "{name} registered {:?}",
            info.providers
        );
        let compatibility = info.compatibility.as_ref().unwrap();
        assert_eq!(compatibility.package, format!("nuclear-plugin-{name}"));
        assert_eq!(
            compatibility.converter,
            nuclear::CONVERTER_VERSION,
            "{name}: the converter bundle and the Rust constant agree"
        );
    }
}

#[tokio::test]
async fn the_youtube_plugins_run_on_raygun_youtube() {
    let cases = [
        (
            "youtube",
            "nuclear.nuclear-plugin-youtube/youtube",
            "music.stream",
        ),
        (
            "youtube-playlists",
            "nuclear.nuclear-plugin-youtube-playlists/youtube-playlists",
            "music.metadata",
        ),
    ];
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    for (name, provider, kind) in cases {
        let converted =
            nuclear::convert(&fixture(name)).unwrap_or_else(|error| panic!("{name}: {error}"));
        // `Ytdlp` is real now: YouTube's hosts are the plugin's to reach.
        assert!(converted.report.stubbed_apis.is_empty(), "{name}");
        for host in ["www.youtube.com", "music.youtube.com", "*.googlevideo.com"] {
            assert!(
                converted.report.hosts.iter().any(|known| known == host),
                "{name}: {host}"
            );
        }
        assert_eq!(converted.report.supported, [kind]);
        let infos = host.load(vec![extension(&converted)]).await;
        assert_eq!(
            infos[0].status,
            raygun_model::ExtensionStatus::Loaded,
            "{name}: {:?}",
            infos[0].error
        );
        let registered: Vec<_> = infos[0]
            .providers
            .iter()
            .map(|registered| (registered.id.as_str(), registered.kind.as_str()))
            .collect();
        assert_eq!(registered, [(provider, kind)], "{name}");
    }
    // A search that isn't a playlist's address finds nothing in the playlists plugin, without
    // asking YouTube.
    let results = host.search_music("one more time", 5).await;
    let playlists = results
        .iter()
        .find(|result| result.provider.extension_id == "nuclear.nuclear-plugin-youtube-playlists")
        .unwrap();
    assert_eq!(playlists.error, None);
    assert!(playlists.tracks.is_empty());
}

/// The first bytes of a stream, as the player asks for them: with the stream's headers.
async fn stream_head(
    client: &reqwest::Client,
    source: &raygun_model::MediaSource,
) -> (u16, Vec<u8>, Option<String>) {
    let raygun_model::MediaSource::Url { url, headers } = source else {
        panic!("not a web stream: {source:?}");
    };
    let mut request = client.get(url).header("Range", "bytes=0-65535");
    for (name, value) in headers {
        request = request.header(name.as_str(), value.as_str());
    }
    let response = request.send().await.unwrap();
    let status = response.status().as_u16();
    let range = response
        .headers()
        .get("content-range")
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    (status, response.bytes().await.unwrap().to_vec(), range)
}

/// Whether bytes start an audio file YouTube serves: WebM (EBML) or MP4.
fn is_audio(bytes: &[u8]) -> bool {
    bytes.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) || bytes.get(4..8) == Some(b"ftyp".as_slice())
}

/// Nuclear's YouTube plugins from its registry, converted and run against YouTube itself: songs
/// found, their streams resolved and their first bytes fetched as the player does, and public
/// playlists read:
/// `cargo test -p raygun-ext-host --lib -- --ignored nuclear_youtube_live --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to jsDelivr, GitHub and YouTube"]
async fn nuclear_youtube_live() {
    use std::time::Instant;

    use crate::StreamUse;
    use raygun_resolver::QualityTarget;

    const REGISTRY: &str =
        "https://cdn.jsdelivr.net/gh/NuclearPlayer/plugin-registry@master/plugins.json";
    const STREAMS: &str = "nuclear.nuclear-plugin-youtube";
    const PLAYLISTS: &str = "nuclear.nuclear-plugin-youtube-playlists";
    let client = raygun_net::http_client().unwrap();
    let download = |url: reqwest::Url| {
        let client = client.clone();
        async move {
            let response = client.get(url.clone()).send().await.unwrap();
            assert!(
                response.status().is_success(),
                "{url}: {}",
                response.status()
            );
            response.bytes().await.unwrap().to_vec()
        }
    };
    let registry_url = reqwest::Url::parse(REGISTRY).unwrap();
    let index =
        crate::repo::parse_index(&String::from_utf8(download(registry_url.clone()).await).unwrap())
            .unwrap();
    let mut sources = Vec::new();
    for id in [STREAMS, PLAYLISTS] {
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == id)
            .unwrap_or_else(|| panic!("the registry lists {id}"));
        let bytes = download(crate::repo::package_url(&registry_url, entry).unwrap()).await;
        crate::package::check_nuclear_download(&bytes, entry).unwrap();
        let converted = nuclear::convert(&nuclear::unzip_sources(&bytes).unwrap()).unwrap();
        crate::package::from_converted(&converted.manifest, &converted.bundle).unwrap();
        println!("{id} {}: {:?}", entry.version, converted.report);
        sources.push(extension(&converted));
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    for info in host.load(sources).await {
        assert_eq!(
            info.status,
            raygun_model::ExtensionStatus::Loaded,
            "{}: {:?}",
            info.id,
            info.error
        );
    }

    let songs = [
        ("Daft Punk", "One More Time"),
        ("Kenshi Yonezu", "Lemon"),
        ("米津玄師", "Lemon"),
        ("Stromae", "Alors on danse"),
        ("Queen", "Bohemian Rhapsody"),
        ("BLACKPINK", "How You Like That"),
        ("Rosalía", "Despechá"),
        ("아이유", "좋은 날"),
    ];
    for (round, (artist, title)) in songs.into_iter().enumerate() {
        let query = raygun_model::TrackQuery {
            title: title.into(),
            artist: Some(artist.into()),
            album: None,
            duration_ms: None,
        };
        let started = Instant::now();
        let answers = host.stream_candidates(&query, 10, QualityTarget::Any).await;
        let searched = started.elapsed();
        let answer = answers
            .into_iter()
            .find(|answer| answer.provider.extension_id == STREAMS)
            .expect("the plugin answered");
        let candidates = answer
            .result
            .unwrap_or_else(|error| panic!("{artist} – {title}: {error}"));
        let ranked =
            raygun_resolver::rank(&query, candidates, &raygun_resolver::History::default());
        let best = &ranked
            .first()
            .unwrap_or_else(|| panic!("{artist} – {title}: no candidate matches"))
            .candidate;
        // Every other song is read as a download is: AAC.
        let using = StreamUse {
            quality: QualityTarget::Any,
            decodable: round % 2 == 1,
        };
        let started = Instant::now();
        let stream = host
            .resolve_stream(&best.provider_id, &best.id, using)
            .await
            .unwrap_or_else(|error| panic!("{artist} – {title}: {error}"));
        let resolved = started.elapsed();
        let started = Instant::now();
        let (status, bytes, range) = stream_head(&client, &stream.source).await;
        let fetched = started.elapsed();
        println!(
            "{artist} – {title}: {} candidates in {searched:.2?}, best {:?} by {:?} ({:?} ms, score {}), stream in {resolved:.2?}, 64 KB in {fetched:.2?} ({status}, {range:?}), expires {:?}",
            ranked.len(),
            best.title,
            best.artist,
            best.duration_ms,
            ranked[0].score,
            stream.expires_at,
        );
        assert!(
            matches!(status, 200 | 206),
            "{artist} – {title}: HTTP {status}"
        );
        assert_eq!(bytes.len(), 65_536, "{artist} – {title}");
        assert!(is_audio(&bytes), "{artist} – {title}: not audio");
        if using.decodable {
            assert_eq!(
                bytes.get(4..8),
                Some(b"ftyp".as_slice()),
                "{artist} – {title}: AAC"
            );
        }
        // The song itself: by its artist, as YouTube spells them in the query's script.
        assert!(
            raygun_resolver::artist_agrees(&query, best),
            "{artist} – {title}: {best:?}"
        );
    }

    let provider = format!("{PLAYLISTS}/youtube-playlists");
    for url in [
        "https://www.youtube.com/playlist?list=PLSbDLF8wQ3oIkmgwFI3HmEYV-qhLwLVHx",
        "https://music.youtube.com/playlist?list=RDCLAK5uy_kw2wIlEv9llILhO0qoMTLsBBhmjzuibAc",
    ] {
        let started = Instant::now();
        let found = host.search_playlists(url, false, 5).await;
        let searched = started.elapsed();
        assert_eq!(found.len(), 1, "{url}");
        let started = Instant::now();
        let playlist = host
            .music_playlist(&provider, &found[0].id)
            .await
            .unwrap_or_else(|error| panic!("{url}: {error}"));
        println!(
            "{url}: {:?} by {:?}, {} songs (card in {searched:.2?}, page in {:.2?}); first {:?} by {:?}",
            playlist.playlist.title,
            playlist.playlist.author,
            playlist.tracks.len(),
            started.elapsed(),
            playlist.tracks.first().map(|track| &track.title),
            playlist.tracks.first().map(|track| &track.artist),
        );
        assert!(playlist.tracks.len() >= 10, "{url}");
        assert!(playlist.tracks.iter().all(|track| track.artist.is_some()));
    }
}

#[tokio::test]
async fn the_lrclib_plugin_answers_lyrics_through_the_shim() {
    let app = Router::new().route(
        "/api/search",
        get(|| async {
            (
                StatusCode::OK,
                [(header::CONTENT_TYPE, "application/json")],
                json!([{
                    "id": 1, "trackName": "Hello, Dolly", "artistName": "Louis Armstrong",
                    "albumName": "Hello, Dolly", "duration": 148, "instrumental": false,
                    "plainLyrics": "Hello, Dolly\nWell, hello, Dolly",
                    "syncedLyrics": "[00:10.00] Hello, Dolly\n[00:14.50] Well, hello, Dolly"
                }])
                .to_string(),
            )
                .into_response()
        }),
    );
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap()
            .block_on(async move {
                let listener = tokio::net::TcpListener::from_std(listener).unwrap();
                axum::serve(listener, app).await.unwrap();
            });
    });

    // Point the plugin at the fixture server before converting, as if it were its real API.
    let mut files = fixture("lrclib");
    for source in files.values_mut() {
        *source = source.replace("https://lrclib.net", &format!("http://{address}"));
    }
    let converted = nuclear::convert(&files).unwrap();
    assert_eq!(converted.report.hosts, vec!["127.0.0.1".to_owned()]);
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(vec![extension(&converted)]).await;

    let query = raygun_model::TrackQuery {
        title: "Hello, Dolly".into(),
        artist: Some("Louis Armstrong".into()),
        album: None,
        duration_ms: Some(148_000),
    };
    let lyrics = host.lyrics(&query).await.unwrap().unwrap();
    assert_eq!(lyrics.provider_name, "LRCLIB");
    let lines: Vec<_> = lyrics
        .synced
        .iter()
        .map(|line| (line.time_ms, line.text.as_str()))
        .collect();
    assert_eq!(
        lines,
        vec![(10_000, "Hello, Dolly"), (14_500, "Well, hello, Dolly")]
    );
}

#[test]
fn ui_only_plugins_are_rejected_with_a_reason() {
    let mut files = BTreeMap::new();
    files.insert(
        "package.json".to_owned(),
        json!({ "name": "nuclear-plugin-queue", "version": "1.0.0", "main": "index.js",
                "nuclear": { "categories": ["queue"] } })
        .to_string(),
    );
    files.insert("index.js".to_owned(), "module.exports = {};".to_owned());
    let error = nuclear::convert(&files).unwrap_err();
    assert!(error.contains("it provides queue"), "{error}");
}

#[tokio::test]
async fn converted_plugins_reach_public_hosts_and_register_nuclear_kinds() {
    let manifest = json!({
        "id": "nuclear.sample", "name": "Sample", "version": "1.0.0", "apiVersion": 1,
        "runtime": "js", "entry": "dist/index.js", "provides": ["lyrics"],
        "permissions": { "net": ["lrclib.net"] }, "contentRating": "safe",
        "nuclear": { "package": "nuclear-plugin-sample", "report": {} },
    })
    .to_string();
    let parsed = Manifest::parse(&manifest).unwrap();
    assert_eq!(
        parsed.net.patterns(),
        [
            crate::permissions::HostPattern::Exact("lrclib.net".into()),
            crate::permissions::HostPattern::AnyPublic
        ]
    );
    let allows = |url: &str| parsed.net.allows(&url.parse().unwrap());
    assert!(allows("https://cf-media.sndcdn.com/a.mp3"));
    assert!(!allows("http://127.0.0.1:8810/"));
    assert!(!allows("http://192.168.1.10/"));

    // A kind the package didn't declare loads when Raygun runs that kind for Nuclear plugins.
    let code = "raygun.providers.register({ kind: 'lyrics', id: 'l', run: async () => null });\n\
                raygun.providers.register({ kind: 'music.stream', id: 's', run: async () => null });";
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host
        .load(vec![ExtensionSource::built_in(&[
            (MANIFEST_FILE, &manifest),
            ("dist/index.js", code),
        ])])
        .await;
    assert_eq!(
        infos[0].status,
        raygun_model::ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    let kinds: Vec<_> = infos[0]
        .providers
        .iter()
        .map(|provider| provider.kind.as_str())
        .collect();
    assert_eq!(kinds, ["lyrics", "music.stream"]);
    assert_eq!(infos[0].compatibility.as_ref().unwrap().converter, 1);

    // Kinds outside Nuclear's still need declaring.
    let other = manifest.replace("nuclear.sample", "nuclear.other");
    let infos = host
        .load(vec![ExtensionSource::built_in(&[
            (MANIFEST_FILE, &other),
            (
                "dist/index.js",
                "raygun.providers.register({ kind: 'manga.source', id: 'm', run: async () => null });",
            ),
        ])])
        .await;
    assert_eq!(infos[0].status, raygun_model::ExtensionStatus::Failed);
    // It still names its converter, so a newer conversion is offered.
    assert_eq!(infos[0].compatibility.as_ref().unwrap().converter, 1);
}

/// Nuclear's plugin registry added by its address, against the real registry: a plugin found in
/// it, downloaded, checked, converted and run the way the app installs one:
/// `cargo test -p raygun-ext-host -- --ignored nuclear_registry_live --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to jsDelivr, GitHub and MusicBrainz"]
async fn nuclear_registry_live_installs_and_runs_a_plugin() {
    const REGISTRY: &str =
        "https://cdn.jsdelivr.net/gh/NuclearPlayer/plugin-registry@master/plugins.json";
    const PLUGIN: &str = "nuclear.nuclear-plugin-musicbrainz";
    let client = raygun_net::http_client().unwrap();
    let download = |url: reqwest::Url| {
        let client = client.clone();
        async move {
            let response = client.get(url.clone()).send().await.unwrap();
            assert!(
                response.status().is_success(),
                "{url}: {}",
                response.status()
            );
            response.bytes().await.unwrap().to_vec()
        }
    };
    let registry_url = reqwest::Url::parse(REGISTRY).unwrap();
    let index =
        crate::repo::parse_index(&String::from_utf8(download(registry_url.clone()).await).unwrap())
            .unwrap();
    println!(
        "{}: {} plugins Raygun can run",
        index.name,
        index.extensions.len()
    );
    assert_eq!(index.name, "Nuclear plugins");
    assert!(
        index
            .extensions
            .iter()
            .all(|entry| entry.format == crate::repo::PackageFormat::Nuclear)
    );
    let entry = index
        .extensions
        .iter()
        .find(|entry| entry.id == PLUGIN)
        .expect("the registry lists the MusicBrainz plugin");

    let bytes = download(crate::repo::package_url(&registry_url, entry).unwrap()).await;
    crate::package::check_nuclear_download(&bytes, entry).unwrap();
    let converted = nuclear::convert(&nuclear::unzip_sources(&bytes).unwrap()).unwrap();
    let verified = crate::package::from_converted(&converted.manifest, &converted.bundle).unwrap();
    assert_eq!(verified.manifest.id, PLUGIN);
    println!("converted: {:?}", converted.report);

    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(vec![extension(&converted)]).await;
    assert_eq!(
        infos[0].status,
        raygun_model::ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    let results = host.search_music("the entertainer scott joplin", 5).await;
    let found = results
        .iter()
        .find(|result| result.provider.extension_id == PLUGIN)
        .expect("the plugin searched");
    for track in &found.tracks {
        println!("{} | {:?} | {:?}", track.title, track.artist, track.album);
    }
    assert_eq!(found.error, None);
    assert!(!found.tracks.is_empty());
}
