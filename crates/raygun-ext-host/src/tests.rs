use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use raygun_model::{ExtensionOrigin, ExtensionStatus, MediaSource};
use serde_json::json;

use crate::manifest::{Manifest, ManifestError};
use crate::permissions::{HostPattern, NetPolicy};
use crate::{CallError, ExtensionHost, ExtensionSource, HostConfig, Limits, discover_installed};

// ---- Manifest ----

fn manifest_json(overrides: serde_json::Value) -> String {
    let mut manifest = json!({
        "id": "dev.raygun.test",
        "name": "Test",
        "version": "1.0.0",
        "apiVersion": 1,
        "runtime": "js",
        "entry": "index.js",
        "provides": ["music.metadata"],
        "permissions": { "net": ["127.0.0.1"] },
    });
    for (key, value) in overrides.as_object().unwrap() {
        manifest[key] = value.clone();
    }
    manifest.to_string()
}

#[test]
fn manifest_accepts_a_valid_extension() {
    let manifest = Manifest::parse(&manifest_json(json!({
        "settings": [{ "key": "lang", "type": "select", "default": "en", "options": ["en", "ja"] }],
    })))
    .unwrap();
    assert_eq!(manifest.id, "dev.raygun.test");
    assert_eq!(manifest.version, semver::Version::new(1, 0, 0));
    assert_eq!(manifest.setting_default("lang"), Some(&json!("en")));
    assert_eq!(manifest.setting_default("missing"), None);
}

#[test]
fn manifest_rejects_invalid_fields() {
    let cases = [
        (
            json!({ "id": "noDots" }),
            ManifestError::Id("noDots".into()),
        ),
        (
            json!({ "id": "dev..x" }),
            ManifestError::Id("dev..x".into()),
        ),
        (json!({ "name": "  " }), ManifestError::Name),
        (
            json!({ "version": "1.0" }),
            ManifestError::Version("1.0".into()),
        ),
        (json!({ "apiVersion": 2 }), ManifestError::ApiVersion(2)),
        (
            json!({ "runtime": "lua" }),
            ManifestError::Runtime("lua".into()),
        ),
        (
            json!({ "entry": "../x.js" }),
            ManifestError::Entry("../x.js".into()),
        ),
        (
            json!({ "entry": "/abs.js" }),
            ManifestError::Entry("/abs.js".into()),
        ),
        (
            json!({ "entry": "C:/x.js" }),
            ManifestError::Entry("C:/x.js".into()),
        ),
        (
            json!({ "entry": "index.ts" }),
            ManifestError::Entry("index.ts".into()),
        ),
        (
            json!({ "provides": ["Music"] }),
            ManifestError::Kind("Music".into()),
        ),
        (
            json!({ "permissions": { "net": ["https://x.org"] } }),
            ManifestError::Net("https://x.org".into()),
        ),
        (
            json!({ "permissions": { "net": ["*.org"] } }),
            ManifestError::Net("*.org".into()),
        ),
        (
            json!({ "settings": [{ "key": "a", "type": "text" }, { "key": "a", "type": "text" }] }),
            ManifestError::Setting("a".into()),
        ),
    ];
    for (overrides, expected) in cases {
        assert_eq!(
            Manifest::parse(&manifest_json(overrides.clone())).unwrap_err(),
            expected,
            "{overrides}"
        );
    }
    assert!(matches!(
        Manifest::parse(&manifest_json(json!({ "unknownField": 1 }))),
        Err(ManifestError::Json(_))
    ));
}

#[test]
fn host_patterns_match_exact_hosts_and_subdomains() {
    let policy = NetPolicy::new(vec![
        HostPattern::parse("archive.org").unwrap(),
        HostPattern::parse("*.archive.org").unwrap(),
        HostPattern::parse("API.Example.com").unwrap(),
    ]);
    let allows = |url: &str| policy.allows(&url.parse().unwrap());
    assert!(allows("https://archive.org/advancedsearch.php"));
    assert!(allows("https://ia800.us.archive.org/x.mp3"));
    assert!(allows("http://api.example.com/"));
    assert!(allows("https://ARCHIVE.org./"));
    assert!(!allows("https://evilarchive.org/"));
    assert!(!allows("https://archive.org.evil.com/"));
    assert!(!allows("https://example.com/"));
    assert!(!allows("ftp://archive.org/"));
    assert!(!allows("file:///etc/passwd"));
    assert_eq!(
        HostPattern::parse("*.archive.org").unwrap().to_string(),
        "*.archive.org"
    );
}

// ---- Runtime ----

/// A local HTTP server for fetch tests, running on its own thread.
fn start_server() -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    let port = address.port();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let app = Router::new()
                .route("/json", get(|| async { ([(header::CONTENT_TYPE, "application/json")], r#"{"hello":"world"}"#) }))
                .route("/page", get(|| async {
                    Html(r#"<html><body><ul><li class="a" data-id="1">One <b>1</b></li><li class="a" data-id="2">Two</li></ul></body></html>"#)
                }))
                .route("/redirect-in", get(|| async { Redirect::temporary("/json") }))
                .route(
                    "/redirect-out",
                    get(move || async move {
                        Redirect::temporary(&format!("http://localhost:{port}/json"))
                    }),
                )
                .route(
                    "/echo",
                    post(|headers: HeaderMap, body: String| async move {
                        let token = headers
                            .get("x-token")
                            .and_then(|value| value.to_str().ok())
                            .unwrap_or("")
                            .to_owned();
                        (StatusCode::CREATED, [(header::CONTENT_TYPE, "text/plain")], format!("{token}:{body}"))
                            .into_response()
                    }),
                );
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    address
}

fn extension(id: &str, code: &str) -> ExtensionSource {
    ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            &manifest_json(json!({ "id": id, "name": id })),
        ),
        ("index.js", code),
    ])
}

fn test_host() -> ExtensionHost {
    ExtensionHost::start(HostConfig {
        limits: Limits {
            cpu_budget: Duration::from_millis(300),
            ..Limits::default()
        },
        call_timeout: Duration::from_secs(10),
        ..HostConfig::default()
    })
    .unwrap()
}

/// Loads one extension whose provider `p` runs `body` as `async (args) => { body }`.
async fn provider_host(body: &str) -> ExtensionHost {
    let host = test_host();
    let code = format!(
        "raygun.providers.register({{ kind: 'music.metadata', id: 'p', name: 'P', run: async (args) => {{ {body} }} }});"
    );
    let infos = host.load(vec![extension("dev.raygun.test", &code)]).await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    host
}

async fn run(
    host: &ExtensionHost,
    args: serde_json::Value,
) -> Result<serde_json::Value, CallError> {
    host.call("dev.raygun.test/p", "run", args).await
}

#[tokio::test]
async fn registers_providers_and_calls_them_with_json() {
    let host =
        provider_host("return { echo: args.value * 2, apiVersion: raygun.apiVersion };").await;
    let list = host.list().await;
    assert_eq!(list[0].providers.len(), 1);
    assert_eq!(list[0].providers[0].id, "dev.raygun.test/p");
    assert_eq!(list[0].providers[0].kind, "music.metadata");
    assert_eq!(list[0].net, vec!["127.0.0.1"]);
    assert_eq!(
        run(&host, json!({ "value": 21 })).await.unwrap(),
        json!({ "echo": 42, "apiVersion": 1 })
    );
    assert_eq!(
        host.call("dev.raygun.test/missing", "run", json!({})).await,
        Err(CallError::UnknownProvider("dev.raygun.test/missing".into()))
    );
    assert!(matches!(
        host.call("dev.raygun.test/p", "nope", json!({})).await,
        Err(CallError::Script(message)) if message.contains("has no method nope")
    ));
}

#[tokio::test]
async fn thrown_errors_become_script_errors() {
    let host = provider_host("throw new Error('boom from js');").await;
    assert!(matches!(
        run(&host, json!({})).await,
        Err(CallError::Script(message)) if message.contains("boom from js")
    ));
}

#[tokio::test]
async fn fetches_allowed_hosts_concurrently() {
    let address = start_server();
    let host = provider_host(&format!(
        r#"
        const base = 'http://{address}';
        const [a, b] = await Promise.all([raygun.http.fetch(base + '/json'), raygun.http.fetch(base + '/redirect-in')]);
        const posted = await raygun.http.fetch(base + '/echo', {{ method: 'POST', headers: {{ 'X-Token': 't0k' }}, body: 'hi' }});
        return {{ a: await a.json(), b: (await b.json()).hello, bUrl: b.url, status: posted.status, ok: posted.ok,
                 echo: await posted.text(), type: posted.headers['content-type'] }};
        "#
    ))
    .await;
    assert_eq!(
        run(&host, json!({})).await.unwrap(),
        json!({
            "a": { "hello": "world" },
            "b": "world",
            "bUrl": format!("http://{address}/json"),
            "status": 201,
            "ok": true,
            "echo": "t0k:hi",
            "type": "text/plain",
        })
    );
}

#[tokio::test]
async fn denies_hosts_and_redirects_outside_the_manifest() {
    let address = start_server();
    let port = address.port();
    let host = provider_host(&format!(
        r#"
        const attempt = async (url) => {{ try {{ await raygun.http.fetch(url); return 'allowed'; }} catch (e) {{ return e.message; }} }};
        return {{
          direct: await attempt('http://localhost:{port}/json'),
          redirect: await attempt('http://{address}/redirect-out'),
          scheme: await attempt('file:///etc/passwd'),
        }};
        "#
    ))
    .await;
    let result = run(&host, json!({})).await.unwrap();
    for case in ["direct", "redirect", "scheme"] {
        let message = result[case].as_str().unwrap();
        assert!(message.contains("not permitted"), "{case}: {message}");
    }
}

#[tokio::test]
async fn selects_html() {
    let address = start_server();
    let host = provider_host(&format!(
        r#"
        const html = await (await raygun.http.fetch('http://{address}/page')).text();
        const items = raygun.html.select(html, 'li.a');
        let bad;
        try {{ raygun.html.select(html, '!!'); }} catch (e) {{ bad = e.message; }}
        return {{
          texts: items.map((li) => li.text.trim()),
          ids: items.map((li) => li.attr('data-id')),
          missing: items[0].attr('nope'),
          nested: items[0].select('b').map((b) => b.text),
          bad,
        }};
        "#
    ))
    .await;
    let result = run(&host, json!({})).await.unwrap();
    assert_eq!(result["texts"], json!(["One 1", "Two"]));
    assert_eq!(result["ids"], json!(["1", "2"]));
    assert_eq!(result["missing"], json!(null));
    assert_eq!(result["nested"], json!(["1"]));
    assert!(result["bad"].as_str().unwrap().contains("invalid selector"));
}

#[tokio::test]
async fn storage_persists_across_loads() {
    let dir = tempfile::tempdir().unwrap();
    let code = r#"raygun.providers.register({ kind: 'music.metadata', id: 'p', name: 'P',
        set: async ({ key, value }) => { raygun.storage.set(key, value); return raygun.storage.keys(); },
        get: async ({ key }) => raygun.storage.get(key),
        remove: async ({ key }) => { raygun.storage.remove(key); return raygun.storage.keys(); },
    });"#;
    let config = HostConfig {
        storage_dir: Some(dir.path().to_owned()),
        ..HostConfig::default()
    };
    let host = ExtensionHost::start(config.clone()).unwrap();
    host.load(vec![extension("dev.raygun.test", code)]).await;
    let set = |key: &str, value: serde_json::Value| {
        host.call(
            "dev.raygun.test/p",
            "set",
            json!({ "key": key, "value": value }),
        )
    };
    assert_eq!(set("a", json!({ "n": 1 })).await.unwrap(), json!(["a"]));
    assert_eq!(set("b", json!([1, 2])).await.unwrap(), json!(["a", "b"]));
    assert!(dir.path().join("dev.raygun.test/storage.json").is_file());

    // A fresh host (an app restart) sees the stored values.
    let host = ExtensionHost::start(config).unwrap();
    host.load(vec![extension("dev.raygun.test", code)]).await;
    let get = |key: &str| host.call("dev.raygun.test/p", "get", json!({ "key": key }));
    assert_eq!(get("a").await.unwrap(), json!({ "n": 1 }));
    assert_eq!(get("missing").await.unwrap(), json!(null));
    assert_eq!(
        host.call("dev.raygun.test/p", "remove", json!({ "key": "a" }))
            .await
            .unwrap(),
        json!(["b"])
    );

    // Oversized values are refused and leave the store unchanged.
    let huge = "x".repeat(crate::storage::STORAGE_LIMIT_BYTES + 1);
    assert!(matches!(
        host.call("dev.raygun.test/p", "set", json!({ "key": "big", "value": huge })).await,
        Err(CallError::Script(message)) if message.contains("limited")
    ));
    assert_eq!(get("big").await.unwrap(), json!(null));
}

#[tokio::test]
async fn settings_fall_back_to_manifest_defaults() {
    let host = test_host();
    let manifest = manifest_json(json!({
        "settings": [{ "key": "lang", "type": "select", "default": "ja", "options": ["en", "ja"] }],
    }));
    let code = "raygun.providers.register({ kind: 'music.metadata', id: 'p', run: async () => [raygun.settings.get('lang'), raygun.settings.get('none')] });";
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest),
        ("index.js", code),
    ])])
    .await;
    assert_eq!(run(&host, json!({})).await.unwrap(), json!(["ja", null]));
    // A provider without a name takes the extension's.
    assert_eq!(host.list().await[0].providers[0].name, "Test");
}

#[tokio::test]
async fn interrupts_runaway_scripts() {
    let host = provider_host("if (args.spin) { for (;;) {} } return 'ok';").await;
    assert!(matches!(
        run(&host, json!({ "spin": true })).await,
        Err(CallError::Script(message)) if message.contains("interrupted")
    ));
    // The extension keeps working afterwards.
    assert_eq!(run(&host, json!({})).await.unwrap(), json!("ok"));
}

#[tokio::test]
async fn sandbox_has_no_ambient_capabilities() {
    let host = provider_host(
        "return { native: typeof __rg, require: typeof require, process: typeof process, std: typeof std, os: typeof os, fetch: typeof fetch };",
    )
    .await;
    assert_eq!(
        run(&host, json!({})).await.unwrap(),
        json!({ "native": "undefined", "require": "undefined", "process": "undefined",
                "std": "undefined", "os": "undefined", "fetch": "undefined" })
    );
}

#[tokio::test]
async fn reports_load_failures_without_stopping_others() {
    let host = test_host();
    let bad_kind =
        "raygun.providers.register({ kind: 'manga.source', id: 'p', run: async () => 1 });";
    let infos = host
        .load(vec![
            extension("dev.raygun.syntax", "this is not javascript"),
            extension("dev.raygun.kind", bad_kind),
            extension("dev.raygun.good", "raygun.providers.register({ kind: 'music.metadata', id: 'p', run: async () => 1 });"),
            ExtensionSource::built_in(&[("raygun-extension.json", "{")]),
        ])
        .await;
    let status = |id: &str| infos.iter().find(|info| info.id == id).unwrap();
    assert_eq!(status("dev.raygun.good").status, ExtensionStatus::Loaded);
    assert_eq!(status("dev.raygun.syntax").status, ExtensionStatus::Failed);
    let syntax = status("dev.raygun.syntax").error.clone().unwrap();
    assert!(syntax.contains("dev.raygun.syntax/index.js:1"), "{syntax}");
    assert!(
        status("dev.raygun.kind")
            .error
            .as_ref()
            .unwrap()
            .contains("provides")
    );
    assert!(
        status("built-in extension")
            .error
            .as_ref()
            .unwrap()
            .contains("manifest")
    );
    assert_eq!(
        host.call("dev.raygun.good/p", "run", json!({})).await,
        Ok(json!(1))
    );
}

#[tokio::test]
async fn installed_extensions_override_built_in_ones() {
    let dir = tempfile::tempdir().unwrap();
    let installed = dir.path().join("override");
    std::fs::create_dir_all(&installed).unwrap();
    std::fs::write(
        installed.join("raygun-extension.json"),
        manifest_json(json!({ "version": "2.0.0" })),
    )
    .unwrap();
    std::fs::write(
        installed.join("index.js"),
        "raygun.providers.register({ kind: 'music.metadata', id: 'p', run: async () => 'installed' });",
    )
    .unwrap();
    std::fs::create_dir_all(dir.path().join(".data/dev.raygun.test")).unwrap();
    std::fs::create_dir_all(dir.path().join("empty")).unwrap();

    let discovered = discover_installed(dir.path());
    assert_eq!(discovered.len(), 1, "only folders with a manifest count");

    let host = test_host();
    let built_in = extension(
        "dev.raygun.test",
        "raygun.providers.register({ kind: 'music.metadata', id: 'p', run: async () => 'built-in' });",
    );
    let mut sources = discovered;
    sources.insert(0, built_in);
    let infos = host.load(sources).await;
    assert_eq!(infos.len(), 1);
    assert_eq!(infos[0].origin, ExtensionOrigin::Installed);
    assert_eq!(infos[0].version, "2.0.0");
    assert_eq!(run(&host, json!({})).await.unwrap(), json!("installed"));
}

#[tokio::test]
async fn search_music_collects_tracks_per_provider() {
    let host = test_host();
    let good = r#"raygun.providers.register({ kind: 'music.metadata', id: 'search', name: 'Good',
      search: async ({ query, limit }) => ({ tracks: [
        { id: 7, title: ` ${query} `, artist: 'A', durationMs: 61000.6, artworkUrl: 'https://x.org/a.jpg',
          source: { kind: 'url', url: 'https://x.org/a.mp3', headers: {} } },
        { title: 'local', source: { kind: 'localFile', path: '/etc/passwd' }, artworkUrl: 'file:///x' },
        { title: 'yt', source: { kind: 'youTube', videoId: 'dQw4w9WgXcQ' } },
        { title: '' },
        { nope: true },
      ].slice(0, limit) }) });"#;
    let failing = "raygun.providers.register({ kind: 'music.metadata', id: 'search', name: 'Bad', search: async () => { throw new Error('offline'); } });";
    host.load(vec![
        extension("dev.raygun.good", good),
        extension("dev.raygun.bad", failing),
    ])
    .await;

    let mut results = host.search_music("song", 10).await;
    results.sort_by(|a, b| a.provider.name.cmp(&b.provider.name));
    assert_eq!(results.len(), 2);
    assert_eq!(results[0].provider.name, "Bad");
    assert!(results[0].error.as_ref().unwrap().contains("offline"));

    let tracks = &results[1].tracks;
    assert_eq!(results[1].error, None);
    assert_eq!(tracks.len(), 3);
    assert_eq!(tracks[0].external_id.as_deref(), Some("7"));
    assert_eq!(tracks[0].title, "song");
    assert_eq!(tracks[0].duration_ms, Some(61000));
    assert!(
        matches!(&tracks[0].source, Some(MediaSource::Url { url, .. }) if url == "https://x.org/a.mp3")
    );
    assert_eq!(
        tracks[1].source, None,
        "local files are never accepted from extensions"
    );
    assert_eq!(tracks[1].artwork_url, None);
    assert!(matches!(
        &tracks[2].source,
        Some(MediaSource::YouTube { .. })
    ));
}

/// Runs the bundled Internet Archive extension against the real service:
/// `cargo test -p raygun-ext-host -- --ignored internet_archive`.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn internet_archive_extension_finds_playable_tracks() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(discover_installed(&dir)).await;
    let archive = infos
        .iter()
        .find(|info| info.id == "dev.raygun.internet-archive")
        .unwrap();
    assert_eq!(
        archive.status,
        ExtensionStatus::Loaded,
        "{:?}",
        archive.error
    );

    let results = host.search_music("louis armstrong", 20).await;
    let result = &results[0];
    assert_eq!(result.error, None);
    assert!(!result.tracks.is_empty());
    for track in &result.tracks {
        println!(
            "{} — {:?} ({:?} ms)",
            track.title, track.artist, track.duration_ms
        );
        assert!(
            matches!(&track.source, Some(MediaSource::Url { url, .. }) if url.starts_with("https://archive.org/download/"))
        );
    }
}
