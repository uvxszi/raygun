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
        json!({ "name": "nuclear-plugin-dash", "version": "1.0.0", "main": "index.js",
                "nuclear": { "categories": ["dashboard"] } })
        .to_string(),
    );
    files.insert("index.js".to_owned(), "module.exports = {};".to_owned());
    let error = nuclear::convert(&files).unwrap_err();
    assert!(error.contains("it provides dashboard"), "{error}");
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
