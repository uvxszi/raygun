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
                    "/cookie",
                    get(|| async { [(header::SET_COOKIE, "site=chosen; Max-Age=3600; Path=/")] }),
                )
                .route(
                    "/whoami",
                    get(|headers: HeaderMap| async move {
                        let value = |name: header::HeaderName| {
                            headers
                                .get(name)
                                .and_then(|value| value.to_str().ok())
                                .unwrap_or("")
                                .to_owned()
                        };
                        format!("{}|{}", value(header::COOKIE), value(header::USER_AGENT))
                    }),
                )
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
async fn cookies_last_per_extension_until_its_data_is_cleared() {
    let address = start_server();
    let dir = tempfile::tempdir().unwrap();
    let host = ExtensionHost::start(HostConfig {
        storage_dir: Some(dir.path().to_owned()),
        call_timeout: Duration::from_secs(10),
        ..HostConfig::default()
    })
    .unwrap();
    let code = "raygun.providers.register({ kind: 'music.metadata', id: 'p', name: 'P',         run: async (args) => (await raygun.http.fetch(args.url)).text() });";
    let source = || {
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                &manifest_json(json!({ "website": "https://site.example/" })),
            ),
            ("index.js", code),
        ])
    };
    let infos = host.load(vec![source()]).await;
    assert_eq!(infos[0].website.as_deref(), Some("https://site.example/"));
    let fetch = |path: &str| {
        let host = host.clone();
        let url = format!("http://{address}{path}");
        async move { run(&host, json!({ "url": url })).await.unwrap() }
    };
    fetch("/cookie").await;
    let seen = fetch("/whoami").await;
    let (cookie, agent) = seen.as_str().unwrap().split_once('|').unwrap();
    assert_eq!(cookie, "site=chosen");
    assert_eq!(agent, raygun_net::user_agent());
    assert!(dir.path().join("dev.raygun.test/cookies.json").exists());

    // A restart keeps them; clearing the extension's data forgets them.
    host.load(vec![source()]).await;
    assert_eq!(
        fetch("/whoami").await.as_str().unwrap().split('|').next(),
        Some("site=chosen")
    );
    crate::package::clear_data(dir.path(), "dev.raygun.test").unwrap();
    host.load(vec![source()]).await;
    assert_eq!(
        fetch("/whoami").await.as_str().unwrap().split('|').next(),
        Some("")
    );
}

#[tokio::test]
async fn thrown_errors_become_script_errors() {
    let host = provider_host("throw new Error('boom from js');").await;
    // Users see the message alone; the stack trace only goes to the log.
    assert_eq!(
        run(&host, json!({})).await,
        Err(CallError::Script("boom from js".into()))
    );
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

/// Runs the Radio Browser extension against the real directory:
/// `cargo test -p raygun-ext-host -- --ignored radio_browser`.
#[tokio::test]
#[ignore = "needs network access to radio-browser.info"]
async fn radio_browser_extension_finds_stations() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(discover_installed(&dir)).await;
    let radio = infos
        .iter()
        .find(|info| info.id == "dev.raygun.radio-browser")
        .unwrap();
    assert_eq!(radio.status, ExtensionStatus::Loaded, "{:?}", radio.error);

    let results = host.search_music("jazz", 10).await;
    let result = results
        .iter()
        .find(|result| result.provider.extension_id == "dev.raygun.radio-browser")
        .unwrap();
    assert_eq!(result.error, None);
    assert!(!result.tracks.is_empty());
    for track in &result.tracks {
        println!("{} | {:?} | {:?}", track.title, track.artist, track.album);
        assert!(matches!(&track.source, Some(MediaSource::Url { .. })));
    }
}

#[tokio::test]
async fn stream_providers_offer_and_resolve_candidates() {
    let host = test_host();
    let manifest = manifest_json(json!({ "provides": ["music.stream"] }));
    let code = r#"raygun.providers.register({ kind: 'music.stream', id: 'streams', name: 'Streams',
      candidates: async ({ title, artist, durationMs, limit }) => ({ candidates: [
        { id: 1, title: `${artist} - ${title}`, durationMs, quality: '320k',
          source: { kind: 'url', url: 'https://cdn.example.org/1.mp3', headers: {} } },
        { id: 'lazy', title, artist },
        { id: 'local', title, source: { kind: 'localFile', path: '/etc/passwd' } },
        { id: 'lookup', title, source: { kind: 'lookup', title, artist: null, album: null, durationMs: null } },
        { id: 'untitled', title: ' ' },
      ].slice(0, limit) }),
      resolve: async ({ id }) => id === 'lazy'
        ? { source: { kind: 'youTube', videoId: 'dQw4w9WgXcQ' }, expiresAt: 1700000000000 }
        : { source: { kind: 'localFile', path: '/etc/passwd' } },
    });"#;
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest),
        ("index.js", code),
    ])])
    .await;

    let query = raygun_model::TrackQuery {
        title: "Song".into(),
        artist: Some("Band".into()),
        album: None,
        duration_ms: Some(200_000),
    };
    let answers = host
        .stream_candidates(&query, 10, raygun_resolver::QualityTarget::Any)
        .await;
    assert_eq!(answers.len(), 1);
    let candidates = answers[0].result.as_ref().unwrap();
    let ids: Vec<_> = candidates
        .iter()
        .map(|candidate| candidate.id.as_str())
        .collect();
    assert_eq!(
        ids,
        vec!["1", "lazy"],
        "local files, lookups and untitled entries are dropped"
    );
    assert_eq!(candidates[0].title, "Band - Song");
    assert_eq!(candidates[0].duration_ms, Some(200_000));
    assert_eq!(candidates[0].provider_name, "Streams");
    assert_eq!(candidates[1].source, None);

    let resolved = host
        .resolve_stream("dev.raygun.test/streams", "lazy")
        .await
        .unwrap();
    assert!(matches!(resolved.source, MediaSource::YouTube { .. }));
    assert_eq!(resolved.expires_at, Some(1_700_000_000_000));
    assert!(matches!(
        host.resolve_stream("dev.raygun.test/streams", "other").await,
        Err(CallError::Script(message)) if message.contains("no playable source")
    ));
}

/// MusicBrainz finds a song with no stream; Internet Archive's `music.stream` provider offers
/// candidates for it; the resolver ranks them:
/// `cargo test -p raygun-ext-host -- --ignored musicbrainz_song_resolves`.
#[tokio::test]
#[ignore = "needs network access to musicbrainz.org and archive.org"]
async fn musicbrainz_song_resolves_through_internet_archive() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;

    let results = host
        .search_music("what a wonderful world armstrong", 10)
        .await;
    let musicbrainz = results
        .iter()
        .find(|result| result.provider.extension_id == "dev.raygun.musicbrainz")
        .unwrap();
    assert_eq!(musicbrainz.error, None);
    let song = musicbrainz
        .tracks
        .iter()
        .find(|track| track.artist.as_deref() == Some("Louis Armstrong"))
        .expect("Louis Armstrong's recording ranks among the results");
    assert_eq!(song.source, None, "MusicBrainz has metadata only");

    // A second search right away stays within MusicBrainz's one-request-per-second limit.
    let again = host.search_music("hello dolly armstrong", 5).await;
    let again = again
        .iter()
        .find(|result| result.provider.extension_id == "dev.raygun.musicbrainz")
        .unwrap();
    assert_eq!(
        again.error, None,
        "the extension paces and retries its requests"
    );
    println!(
        "song: {} | {:?} | {:?} ms",
        song.title, song.artist, song.duration_ms
    );

    let query = raygun_model::TrackQuery {
        title: song.title.clone(),
        artist: song.artist.clone(),
        album: song.album.clone(),
        duration_ms: song.duration_ms,
    };
    let candidates: Vec<_> = host
        .stream_candidates(&query, 10, raygun_resolver::QualityTarget::Any)
        .await
        .into_iter()
        .flat_map(|answer| answer.result.unwrap_or_default())
        .collect();
    assert!(!candidates.is_empty());
    let ranked = raygun_resolver::rank(&query, candidates, &raygun_resolver::History::default());
    for scored in &ranked {
        println!(
            "{:>4}  {} | {:?} | {:?} ms",
            scored.score,
            scored.candidate.title,
            scored.candidate.artist,
            scored.candidate.duration_ms
        );
    }
    let best = ranked.first().expect("a candidate passes the threshold");
    assert!(
        matches!(&best.candidate.source, Some(MediaSource::Url { url, .. }) if url.contains("archive.org/download/"))
    );
}

#[tokio::test]
async fn sleep_waits_without_spending_the_cpu_budget() {
    // The test host's CPU budget is 300 ms; sleeping longer must not trip it.
    let host = provider_host(
        "const started = Date.now(); await raygun.time.sleep(400); await raygun.time.sleep(-5); return Date.now() - started;",
    )
    .await;
    let waited = run(&host, json!({})).await.unwrap().as_f64().unwrap();
    assert!((390.0..2_000.0).contains(&waited), "{waited}");
}

/// Multi-artist credits and decorated titles still find streams:
/// `cargo test -p raygun-ext-host -- --ignored internet_archive_streams`.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn internet_archive_streams_match_credited_duets() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let query = raygun_model::TrackQuery {
        title: "Hello Dolly (Hello Dolly)".into(),
        artist: Some("Ella Fitzgerald & Louis Armstrong".into()),
        album: None,
        duration_ms: Some(148_000),
    };
    let candidates: Vec<_> = host
        .stream_candidates(&query, 10, raygun_resolver::QualityTarget::Any)
        .await
        .into_iter()
        .flat_map(|answer| answer.result.unwrap())
        .collect();
    for candidate in &candidates {
        println!("{} | {:?}", candidate.title, candidate.artist);
    }
    assert!(!candidates.is_empty());
}

#[tokio::test]
async fn lyrics_prefer_synced_answers_and_validate_lines() {
    let host = test_host();
    let provider = |id: &str, body: &str| {
        let manifest = manifest_json(json!({ "id": id, "name": id, "provides": ["lyrics"] }));
        let code = format!(
            "raygun.providers.register({{ kind: 'lyrics', id: 'l', name: '{id}', lyrics: async (q) => {{ {body} }} }});"
        );
        ExtensionSource::built_in(&[("raygun-extension.json", &manifest), ("index.js", &code)])
    };
    host.load(vec![
        provider("dev.raygun.plain", "return { plain: '  Just words  ' };"),
        provider("dev.raygun.empty", "return null;"),
        provider(
            "dev.raygun.synced",
            "return { synced: [ { timeMs: 5000, text: ' second ' }, { timeMs: -1, text: 'bad' }, { timeMs: 1000, text: 'first' }, { text: 'no time' } ] };",
        ),
    ])
    .await;
    let query = raygun_model::TrackQuery {
        title: "Song".into(),
        artist: Some("Band".into()),
        album: None,
        duration_ms: Some(200_000),
    };
    let lyrics = host.lyrics(&query).await.unwrap().unwrap();
    assert_eq!(lyrics.provider_name, "dev.raygun.synced");
    let lines: Vec<_> = lyrics
        .synced
        .iter()
        .map(|line| (line.time_ms, line.text.as_str()))
        .collect();
    assert_eq!(lines, vec![(1000, "first"), (5000, "second")]);

    // Only plain text known: that's the answer. Nothing known: None. All failing: the error.
    let host = test_host();
    host.load(vec![provider(
        "dev.raygun.plain",
        "return { plain: 'Words' };",
    )])
    .await;
    assert_eq!(
        host.lyrics(&query).await.unwrap().unwrap().plain.as_deref(),
        Some("Words")
    );
    let host = test_host();
    host.load(vec![provider(
        "dev.raygun.empty",
        "return { synced: [], plain: ' ' };",
    )])
    .await;
    assert_eq!(host.lyrics(&query).await, Ok(None));
    let host = test_host();
    host.load(vec![provider(
        "dev.raygun.down",
        "throw new Error('lyrics site is down');",
    )])
    .await;
    assert_eq!(host.lyrics(&query).await, Err("lyrics site is down".into()));
}

#[tokio::test]
async fn charts_and_tags_come_from_their_providers() {
    let host = test_host();
    let manifest = manifest_json(json!({ "provides": ["music.charts", "music.tags"] }));
    let code = r#"
        raygun.providers.register({
          kind: 'music.charts', id: 'charts', name: 'Charts',
          charts: async ({ region }) => ({ charts: [
            { id: 'top', name: ' Top songs ', description: region },
            { id: 7, name: 'Rock', tag: 'Rock', artworkUrl: 'javascript:alert(1)' },
            { id: '', name: 'No id' },
            { id: 'x', name: '   ' },
          ] }),
          chart: async ({ id, region, limit }) => ({ tracks: [
            { id: 1, title: `${id} ${region} ${limit}`, artist: 'A' },
            { title: '' },
          ] }),
        });
        raygun.providers.register({
          kind: 'music.tags', id: 'tags', name: 'Tags',
          tags: async ({ title }) => ({ tags: [
            { name: ' Rock ' }, { name: 'Pop', weight: 0.4 }, { name: 'Loud', weight: 7 },
            { name: '' }, { name: 'x'.repeat(61) }, { weight: 1 },
          ] }),
        });
    "#;
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest),
        ("index.js", code),
    ])])
    .await;

    let charts = host.music_charts(Some("gb")).await;
    let (provider, answer) = &charts[0];
    assert_eq!(provider.id, "dev.raygun.test/charts");
    let charts = answer.as_ref().unwrap();
    assert_eq!(
        charts.len(),
        2,
        "charts without an id or a name are dropped"
    );
    assert_eq!(charts[0].name, "Top songs");
    assert_eq!(charts[0].description.as_deref(), Some("gb"));
    assert_eq!(charts[1].id, "7");
    assert_eq!(charts[1].tag.as_deref(), Some("Rock"));
    assert_eq!(charts[1].artwork_url, None, "only web artwork");

    let songs = host
        .music_chart("dev.raygun.test/charts", "top", Some("gb"), 20)
        .await
        .unwrap();
    assert_eq!(songs.len(), 1);
    assert_eq!(songs[0].title, "top gb 20");

    let query = raygun_model::TrackQuery {
        title: "Song".into(),
        artist: Some("A".into()),
        album: None,
        duration_ms: None,
    };
    let tags = host.music_tags(&query).await;
    assert_eq!(
        tags[0].1.as_ref().unwrap(),
        &vec![
            ("Rock".to_owned(), 1.0),
            ("Pop".to_owned(), 0.4),
            ("Loud".to_owned(), 1.0)
        ]
    );
}

#[tokio::test]
async fn charts_have_groups_and_any_tag_has_songs() {
    let host = test_host();
    let manifest = manifest_json(json!({ "provides": ["music.charts"] }));
    let code = r#"
        raygun.providers.register({
          kind: 'music.charts', id: 'full', name: 'Full',
          charts: async () => ({ charts: [
            { id: 'a', name: 'Rising', group: 'Trending' },
            { id: 'b', name: 'Rock', tag: 'Rock' },
            { id: 'c', name: 'Main' },
            { id: 'd', name: 'Odd', group: 'sideways' },
          ] }),
          chart: async () => ({ tracks: [] }),
          tagSongs: async ({ tag, aliases, kind, region, limit }) => ({
            name: `${tag} ${kind}`,
            tracks: [{ title: `${aliases.join('+')} ${region} ${limit}`, artist: 'A' }],
          }),
        });
        raygun.providers.register({
          kind: 'music.charts', id: 'plain', name: 'Plain',
          charts: async () => ({ charts: [] }),
          chart: async () => ({ tracks: [] }),
        });
        raygun.providers.register({
          kind: 'music.charts', id: 'broken', name: 'Broken',
          charts: async () => ({ charts: [] }),
          chart: async () => ({ tracks: [] }),
          tagSongs: async () => { throw new Error('genre server is down'); },
        });
    "#;
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest),
        ("index.js", code),
    ])])
    .await;

    let charts = host.music_charts(None).await;
    let full = charts[0].1.as_ref().unwrap();
    let groups: Vec<_> = full.iter().map(|chart| chart.group).collect();
    assert_eq!(
        groups,
        vec![
            raygun_model::ChartGroup::Trending,
            raygun_model::ChartGroup::Genre,
            raygun_model::ChartGroup::Top,
            raygun_model::ChartGroup::Top,
        ],
        "a tag without a group is a genre chart; no or an unknown group is a main one"
    );

    let answers = host
        .music_tag_songs(
            "Hip-Hop",
            &["hip hop".into(), "rap".into()],
            "genre",
            Some("gb"),
            30,
        )
        .await;
    assert_eq!(answers.len(), 2, "sources without tagSongs are left out");
    let (provider, answer) = &answers[0];
    assert_eq!(provider.id, "dev.raygun.test/full");
    let (name, tracks) = answer.as_ref().unwrap();
    assert_eq!(name.as_deref(), Some("Hip-Hop genre"));
    assert_eq!(tracks[0].title, "hip hop+rap gb 30");
    assert!(
        answers[1]
            .1
            .as_ref()
            .unwrap_err()
            .contains("genre server is down")
    );
}

#[tokio::test]
async fn recommendations_come_from_the_first_source_that_has_some() {
    let host = test_host();
    let manifest = manifest_json(json!({ "provides": ["music.recommend"] }));
    let code = r#"
        raygun.providers.register({
          kind: 'music.recommend', id: 'empty', name: 'Empty',
          similarArtists: async () => ({ artists: [] }),
          radio: async () => { throw new Error('radio is off the air'); },
        });
        raygun.providers.register({
          kind: 'music.recommend', id: 'full', name: 'Full',
          similarArtists: async ({ artist, limit }) => ({ artists: [
            { id: 1, name: ` ${artist} II ` }, { id: 2, name: '' }, { id: 3, name: `${limit}` },
          ] }),
          topTracks: async ({ artist }) => ({ tracks: [{ title: `${artist} hit`, artist }] }),
          radio: async ({ artist }) => ({ tracks: [{ title: `${artist} radio`, artist }] }),
        });
    "#;
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest),
        ("index.js", code),
    ])])
    .await;

    assert_eq!(host.similar_artists("Nina", 4).await, vec!["Nina II", "4"]);
    let top = host
        .recommended_tracks("topTracks", json!({ "artist": "Nina", "limit": 5 }))
        .await;
    assert_eq!(top[0].title, "Nina hit");
    let radio = host
        .recommended_tracks(
            "radio",
            json!({ "artist": "Nina", "title": null, "limit": 5 }),
        )
        .await;
    assert_eq!(
        radio[0].title, "Nina radio",
        "a failing source gives way to the next"
    );
    assert!(
        host.recommended_tracks(
            "newReleases",
            json!({ "artist": "Nina", "days": 28, "limit": 5 })
        )
        .await
        .is_empty()
    );
}

#[tokio::test]
async fn artists_and_albums_come_from_sources_that_search_them() {
    let host = test_host();
    let code = r#"
        raygun.providers.register({
          kind: 'music.metadata', id: 'full', name: 'Full',
          search: async () => ({ tracks: [] }),
          searchArtists: async ({ query }) => ({ artists: [
            { id: 27, name: query, imageUrl: 'https://e.x/a.jpg' }, { id: '', name: 'No id' },
          ] }),
          searchAlbums: async () => ({ albums: [
            { id: 'a1', title: 'Discovery', artist: 'Daft Punk', year: 2001, trackCount: 14 },
            { id: 'a2', title: '  ' },
          ] }),
          album: async ({ id }) => ({
            album: { id, title: 'Discovery', artist: 'Daft Punk' },
            tracks: [{ title: 'One More Time', artist: 'Daft Punk' }],
          }),
        });
    "#;
    let other = "raygun.providers.register({ kind: 'music.metadata', id: 'songs', search: async () => ({ tracks: [] }) });";
    host.load(vec![
        extension("dev.raygun.full", code),
        extension("dev.raygun.songs", other),
    ])
    .await;
    let artists = host.search_artists("Daft Punk", 10).await;
    assert_eq!(
        artists.len(),
        1,
        "sources without the call and entries without ids are left out"
    );
    assert_eq!(artists[0].id, "27");
    assert_eq!(artists[0].name, "Daft Punk");
    let albums = host.search_albums("discovery", 10).await;
    assert_eq!(albums.len(), 1);
    assert_eq!(albums[0].year, Some(2001));
    let album = host
        .music_album("dev.raygun.full/full", "a1")
        .await
        .unwrap();
    assert_eq!(album.album.id, "a1");
    assert_eq!(album.tracks[0].title, "One More Time");
    assert!(
        host.music_album("dev.raygun.songs/songs", "a1")
            .await
            .is_err()
    );
    assert!(host.search_albums("  ", 10).await.is_empty());
}

/// The chart and tag extensions against their real services:
#[tokio::test]
async fn artist_info_and_credits_come_from_music_info_providers() {
    let host = test_host();
    let code = "raygun.providers.register({ kind: 'music.info', id: 'p', name: 'Facts',         artist: async ({ name }) => name === 'Nobody' ? { artist: null } : ({ artist: { name,           kind: 'Group', bio: ' A band. ', imageUrl: 'https://img.example/a.jpg', url: 'ftp://no',           members: ['One', '', 'Two'] } }),         credits: async ({ title }) => ({ credits: [{ role: 'Written by', names: ['A', ' '] },           { role: 'Empty', names: [] }], url: 'https://mb.example/r' }) });";
    let manifest = manifest_json(json!({ "provides": ["music.info"] }));
    host.load(vec![ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest),
        ("index.js", code),
    ])])
    .await;

    let info = host.artist_info("Daft Punk").await.unwrap();
    assert_eq!(info.name, "Daft Punk");
    assert_eq!(info.bio.as_deref(), Some("A band."));
    assert_eq!(info.image_url.as_deref(), Some("https://img.example/a.jpg"));
    assert_eq!(info.url, None);
    assert_eq!(info.members, ["One", "Two"]);
    assert_eq!(info.provider_name, "Facts");
    assert!(host.artist_info("Nobody").await.is_none());

    let credits = host
        .song_credits(&raygun_model::TrackQuery {
            title: "Song".into(),
            artist: None,
            album: None,
            duration_ms: None,
        })
        .await
        .unwrap();
    assert_eq!(credits.credits.len(), 1);
    assert_eq!(credits.credits[0].names, ["A"]);
    assert_eq!(credits.url.as_deref(), Some("https://mb.example/r"));
}

/// `cargo test -p raygun-ext-host -- --ignored music_info_live --nocapture`.
#[tokio::test]
#[ignore = "needs network access to MusicBrainz, Wikidata and Wikipedia"]
async fn music_info_live_artist_and_credits() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let info = host
        .artist_info("Ray Parker Jr.")
        .await
        .expect("artist info");
    println!("{info:#?}");
    assert!(info.bio.is_some_and(|bio| bio.len() > 40));
    let credits = host
        .song_credits(&raygun_model::TrackQuery {
            title: "Ghostbusters".into(),
            artist: Some("Ray Parker Jr.".into()),
            album: None,
            duration_ms: None,
        })
        .await
        .expect("credits");
    println!("{credits:#?}");
    assert!(
        credits
            .credits
            .iter()
            .any(|group| group.role == "Performed by")
    );
}

/// `cargo test -p raygun-ext-host -- --ignored discovery_live --nocapture`.
#[tokio::test]
#[ignore = "needs network access to Deezer, Apple, ListenBrainz and MusicBrainz"]
async fn discovery_live_charts_and_tags() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;

    for (provider, answer) in host.music_charts(Some("gb")).await {
        let charts = answer.unwrap_or_else(|error| panic!("{}: {error}", provider.name));
        assert!(!charts.is_empty(), "{}", provider.name);
        let first = &charts[0];
        let songs = host
            .music_chart(&provider.id, &first.id, Some("gb"), 10)
            .await
            .unwrap_or_else(|error| panic!("{} {}: {error}", provider.name, first.name));
        println!(
            "{}: {} charts; {}: {:?}",
            provider.name,
            charts.len(),
            first.name,
            songs
                .iter()
                .take(3)
                .map(|song| (&song.title, &song.artist))
                .collect::<Vec<_>>()
        );
        assert!(!songs.is_empty(), "{} {}", provider.name, first.name);
        // Every kind of chart a source lists plays (M46.19).
        let mut seen = std::collections::HashSet::new();
        for chart in charts.iter().filter(|chart| seen.insert(chart.group)) {
            let songs = host
                .music_chart(&provider.id, &chart.id, Some("gb"), 5)
                .await
                .unwrap_or_else(|error| panic!("{} {}: {error}", provider.name, chart.name));
            println!("  {:?} {}: {} songs", chart.group, chart.name, songs.len());
            assert!(!songs.is_empty(), "{} {}", provider.name, chart.name);
        }
    }

    // Songs for genres and moods from every source that has them (M46.19).
    for (tag, aliases, kind) in [
        ("Hip-Hop", vec!["hip hop", "rap"], "genre"),
        ("Breakcore", vec![], "genre"),
        ("Chill", vec!["relaxing", "mellow"], "mood"),
        ("Phonk", vec!["drift phonk"], "genre"),
    ] {
        let aliases: Vec<String> = aliases.into_iter().map(str::to_owned).collect();
        let answers = host
            .music_tag_songs(tag, &aliases, kind, Some("gb"), 10)
            .await;
        let mut filled = 0;
        for (provider, answer) in answers {
            match answer {
                Ok((name, songs)) => {
                    println!(
                        "{tag} / {}: {:?} {} songs {:?}",
                        provider.name,
                        name,
                        songs.len(),
                        songs.first().map(|song| (&song.title, &song.artist))
                    );
                    if !songs.is_empty() {
                        filled += 1;
                    }
                }
                Err(error) => panic!("{tag} / {}: {error}", provider.name),
            }
        }
        assert!(filled >= 2, "{tag}: only {filled} sources had songs");
    }

    // Recommendations for Made for You (M46.20).
    let similar = host.similar_artists("Daft Punk", 8).await;
    println!("similar to Daft Punk: {similar:?}");
    assert!(!similar.is_empty());
    // Deezer lists a little-known namesake first; the famous one is meant.
    let similar = host.similar_artists("Michael Jackson", 8).await;
    println!("similar to Michael Jackson: {similar:?}");
    assert!(
        similar.iter().any(|name| name.contains("Jackson")),
        "{similar:?}"
    );
    let top = host
        .recommended_tracks("topTracks", json!({ "artist": "Daft Punk", "limit": 5 }))
        .await;
    assert!(!top.is_empty());
    let radio = host
        .recommended_tracks(
            "radio",
            json!({ "artist": "Daft Punk", "title": null, "limit": 10 }),
        )
        .await;
    assert!(!radio.is_empty());
    let releases = host
        .recommended_tracks(
            "newReleases",
            json!({ "artist": "Taylor Swift", "days": 365, "limit": 5 }),
        )
        .await;
    println!(
        "top {:?}, radio {:?}, releases {:?}",
        top.first().map(|song| &song.title),
        radio.first().map(|song| &song.title),
        releases.first().map(|song| &song.title)
    );

    let albums = host.search_albums("random access memories", 5).await;
    println!(
        "albums: {:?}",
        albums
            .iter()
            .map(|album| (&album.provider.name, &album.title))
            .collect::<Vec<_>>()
    );
    let deezer = albums
        .iter()
        .find(|album| album.provider.extension_id == "dev.raygun.deezer")
        .expect("Deezer finds the album");
    let detail = host
        .music_album(&deezer.provider.id, &deezer.id)
        .await
        .unwrap();
    assert!(detail.tracks.len() > 10, "{}", detail.tracks.len());
    assert!(!host.search_artists("daft punk", 5).await.is_empty());

    let query = raygun_model::TrackQuery {
        title: "Get Lucky".into(),
        artist: Some("Daft Punk".into()),
        album: None,
        duration_ms: Some(248_000),
    };
    for (provider, answer) in host.music_tags(&query).await {
        let tags = answer.unwrap_or_else(|error| panic!("{}: {error}", provider.name));
        println!("{}: {tags:?}", provider.name);
        assert!(!tags.is_empty(), "{}", provider.name);
    }
}

/// LibriVox's books through the Internet Archive (M46.22):
/// `cargo test -p raygun-ext-host -- --ignored librivox_live --nocapture`.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn librivox_live_finds_books_with_feeds() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let provider = "dev.raygun.librivox/books";
    for (list, query) in [
        ("search", "pride and prejudice"),
        ("popular", ""),
        ("latest", ""),
    ] {
        let page = host.audiobooks(provider, list, query, 1).await.unwrap();
        println!(
            "{list}: {} books, more {}; first {:?}",
            page.books.len(),
            page.has_more,
            page.books
                .first()
                .map(|book| (&book.title, &book.author, &book.feed_url))
        );
        assert!(!page.books.is_empty(), "{list}");
        assert!(
            page.books
                .iter()
                .all(|book| book.feed_url.starts_with("https://librivox.org/rss/"))
        );
    }
}

/// LRCLIB's real service: `cargo test -p raygun-ext-host -- --ignored lrclib_live`.
#[tokio::test]
#[ignore = "needs network access to lrclib.net"]
async fn lrclib_live_returns_synced_lyrics() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let query = raygun_model::TrackQuery {
        title: "What a Wonderful World".into(),
        artist: Some("Louis Armstrong".into()),
        album: None,
        duration_ms: Some(139_000),
    };
    let lyrics = host
        .lyrics(&query)
        .await
        .unwrap()
        .expect("LRCLIB has this song");
    println!(
        "{} synced lines, first: {:?}",
        lyrics.synced.len(),
        lyrics.synced.first()
    );
    assert!(!lyrics.synced.is_empty());
}

/// YouTube Music's real search, without a key: `cargo test -p raygun-ext-host -- --ignored youtube_live`.
#[tokio::test]
#[ignore = "needs network access to music.youtube.com"]
async fn youtube_live_searches_without_a_key() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let results = host.search_music("daft punk get lucky", 10).await;
    let youtube = results
        .iter()
        .find(|result| result.provider.extension_id == "dev.raygun.youtube")
        .expect("the YouTube extension searched");
    assert_eq!(youtube.error, None);
    for track in youtube.tracks.iter().take(5) {
        println!(
            "{} · {:?} · {:?} · {:?}",
            track.title, track.artist, track.album, track.duration_ms
        );
    }
    let first = &youtube.tracks[0];
    assert!(first.title.contains("Get Lucky"));
    assert!(
        first
            .artist
            .as_deref()
            .unwrap_or_default()
            .contains("Daft Punk")
    );
    assert!(first.duration_ms.is_some());
    assert!(matches!(
        first.source,
        Some(raygun_model::MediaSource::YouTube { .. })
    ));
}

/// `cargo test -p raygun-ext-host -- --ignored gutenberg`.
#[tokio::test]
#[ignore = "needs network access to gutenberg.org"]
async fn gutenberg_extension_reads_a_real_book() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(discover_installed(&dir)).await;
    let gutenberg = infos
        .iter()
        .find(|info| info.id == "dev.raygun.gutenberg")
        .unwrap();
    assert_eq!(
        gutenberg.status,
        ExtensionStatus::Loaded,
        "{:?}",
        gutenberg.error
    );
    let provider = "dev.raygun.gutenberg/gutenberg";

    let found = host
        .manga_search(provider, "frankenstein", 1)
        .await
        .unwrap();
    assert!(found.items.iter().any(|item| item.id == "84"), "{found:?}");
    let chapters = host.manga_chapters(provider, "84").await.unwrap();
    assert!(chapters.len() >= 20, "{chapters:?}");
    assert!(
        chapters.iter().any(|chapter| chapter.name == "Letter 1"),
        "{chapters:?}"
    );
    let passage = host.novel_passage(provider, &chapters[0].id).await.unwrap();
    assert!(passage.len() > 5, "{passage:?}");
}

/// `cargo test -p raygun-ext-host -- --ignored shosetsu_official`: installs scripts from
/// Shosetsu's official repository as the app does, then reads a listing, a novel and a chapter.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to gitlab.com and the novel sites"]
async fn shosetsu_official_scripts_work() {
    use crate::shosetsu::package as shosetsu_package;
    const INDEX: &str = "https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json";
    let client = raygun_net::http_client().unwrap();
    let download = |url: reqwest::Url| {
        let client = client.clone();
        async move {
            client
                .get(url)
                .send()
                .await
                .unwrap()
                .bytes()
                .await
                .unwrap()
                .to_vec()
        }
    };
    let index_url = reqwest::Url::parse(INDEX).unwrap();
    let index =
        crate::repo::parse_index(&String::from_utf8(download(index_url.clone()).await).unwrap())
            .unwrap();
    assert!(index.extensions.len() > 20, "{}", index.extensions.len());

    let temp = tempfile::TempDir::new().unwrap();
    let wanted =
        std::env::var("SHOSETSU_SCRIPTS").unwrap_or_else(|_| "shosetsu.en.novelfull".into());
    for id in wanted.split(',') {
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == id)
            .unwrap();
        let bytes = download(crate::repo::package_url(&index_url, entry).unwrap()).await;
        let script = shosetsu_package::verify_script(&bytes, entry).unwrap();
        let mut queue = shosetsu_package::dependencies(&script);
        let mut libraries: Vec<(String, String)> = Vec::new();
        while let Some(name) = queue.pop() {
            if libraries.iter().any(|(known, _)| *known == name) {
                continue;
            }
            let bytes = download(index_url.join(&format!("lib/{name}.lua")).unwrap()).await;
            let code = shosetsu_package::verify_library(&name, &bytes, &index).unwrap();
            queue.extend(shosetsu_package::dependencies(&code));
            libraries.push((name, code));
        }
        let package = shosetsu_package::package(entry, script, libraries).unwrap();
        crate::package::install(temp.path(), &package).unwrap();
    }

    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(discover_installed(temp.path())).await;
    for info in &infos {
        assert_eq!(
            info.status,
            ExtensionStatus::Loaded,
            "{}: {:?}",
            info.id,
            info.error
        );
        let provider = format!("{}/novel", info.id);
        let popular = host
            .manga_list(&provider, crate::manga::MangaList::Popular, 1)
            .await
            .unwrap_or_else(|error| panic!("{}: {error}", info.id));
        eprintln!(
            "{}: {} novels, first {:?}",
            info.id,
            popular.items.len(),
            popular.items.first().map(|item| &item.title)
        );
        assert!(!popular.items.is_empty(), "{}", info.id);
        let novel = &popular.items[0];
        let details = host.manga_details(&provider, &novel.id).await.unwrap();
        let chapters = host.manga_chapters(&provider, &novel.id).await.unwrap();
        eprintln!(
            "  {}: {} chapters, status {}, genres {:?}",
            details.title,
            chapters.len(),
            details.status,
            details.genres
        );
        assert!(!chapters.is_empty());
        let passage = host
            .novel_passage(&provider, &chapters[0].id)
            .await
            .unwrap();
        eprintln!(
            "  first chapter: {} blocks, starts {:?}",
            passage.len(),
            passage.first()
        );
        assert!(passage.len() > 3);
    }
}

/// `cargo test -p raygun-ext-host -- --ignored shosetsu_survey --nocapture`: every script in
/// Shosetsu's official repository (or `SHOSETSU_SCRIPTS`), each reporting how far it got: load,
/// listing, novel, chapters, passage. Prints a table instead of failing, to find runtime gaps.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to gitlab.com and many novel sites"]
async fn shosetsu_survey() {
    use crate::shosetsu::package as shosetsu_package;
    const INDEX: &str = "https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json";
    let client = raygun_net::http_client().unwrap();
    // `SHOSETSU_DIR` reads the index and scripts from a checkout instead of GitLab.
    let local = std::env::var("SHOSETSU_DIR").ok();
    let download = |url: reqwest::Url| {
        let client = client.clone();
        let local = local.clone();
        async move {
            if let Some(dir) = local {
                let path = url
                    .path()
                    .split_once("/raw/dev/")
                    .map(|(_, path)| path.to_owned())
                    .ok_or("not in the repository")?;
                return std::fs::read(std::path::Path::new(&dir).join(path))
                    .map_err(|error| error.to_string());
            }
            // Gently: GitLab answers 429 to a burst of raw downloads.
            tokio::time::sleep(Duration::from_millis(400)).await;
            let response = client
                .get(url)
                .send()
                .await
                .and_then(reqwest::Response::error_for_status)
                .map_err(|error| error.to_string())?;
            response
                .bytes()
                .await
                .map(|bytes| bytes.to_vec())
                .map_err(|error| error.to_string())
        }
    };
    let index_url = reqwest::Url::parse(INDEX).unwrap();
    let index = crate::repo::parse_index(
        &String::from_utf8(download(index_url.clone()).await.unwrap()).unwrap(),
    )
    .unwrap();
    let wanted = std::env::var("SHOSETSU_SCRIPTS").ok();
    let mut report = Vec::new();
    for entry in &index.extensions {
        if wanted
            .as_deref()
            .is_some_and(|list| !list.split(',').any(|id| id == entry.id))
        {
            continue;
        }
        let temp = tempfile::TempDir::new().unwrap();
        let installed: Result<(), String> = async {
            let bytes = download(crate::repo::package_url(&index_url, entry).unwrap()).await?;
            let script = shosetsu_package::verify_script(&bytes, entry)
                .map_err(|error| error.to_string())?;
            let mut queue = shosetsu_package::dependencies(&script);
            let mut libraries: Vec<(String, String)> = Vec::new();
            while let Some(name) = queue.pop() {
                if libraries.iter().any(|(known, _)| *known == name) {
                    continue;
                }
                let bytes = download(index_url.join(&format!("lib/{name}.lua")).unwrap()).await?;
                let code = shosetsu_package::verify_library(&name, &bytes, &index)
                    .map_err(|error| error.to_string())?;
                queue.extend(shosetsu_package::dependencies(&code));
                libraries.push((name, code));
            }
            let package = shosetsu_package::package(entry, script, libraries)
                .map_err(|error| error.to_string())?;
            crate::package::install(temp.path(), &package)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        .await;
        if let Err(error) = installed {
            report.push((entry.id.clone(), "install", error));
            continue;
        }
        let host = ExtensionHost::start(HostConfig::default()).unwrap();
        let infos = host.load(discover_installed(temp.path())).await;
        let Some(info) = infos.first() else {
            report.push((entry.id.clone(), "load", "nothing loaded".into()));
            continue;
        };
        if info.status != ExtensionStatus::Loaded {
            report.push((entry.id.clone(), "load", format!("{:?}", info.error)));
            continue;
        }
        let provider = format!("{}/novel", info.id);
        let step = async {
            let popular = host
                .manga_list(&provider, crate::manga::MangaList::Popular, 1)
                .await
                .map_err(|error| ("listing", error.to_string()))?;
            let novel = popular
                .items
                .first()
                .ok_or(("listing", "no novels".to_owned()))?
                .clone();
            host.manga_details(&provider, &novel.id)
                .await
                .map_err(|error| ("novel", error.to_string()))?;
            let chapters = host
                .manga_chapters(&provider, &novel.id)
                .await
                .map_err(|error| ("chapters", error.to_string()))?;
            let first = chapters
                .first()
                .ok_or(("chapters", "no chapters".to_owned()))?;
            let passage = host
                .novel_passage(&provider, &first.id)
                .await
                .map_err(|error| ("passage", error.to_string()))?;
            if passage.len() < 2 {
                return Err(("passage", format!("{} blocks", passage.len())));
            }
            Ok(())
        }
        .await;
        match step {
            Ok(()) => report.push((entry.id.clone(), "ok", String::new())),
            Err((stage, error)) => report.push((entry.id.clone(), stage, error)),
        }
    }
    let working = report.iter().filter(|(_, stage, _)| *stage == "ok").count();
    eprintln!("{working} of {} scripts work", report.len());
    for (id, stage, error) in &report {
        let line: String = error.chars().take(160).collect();
        eprintln!("{stage:>8}  {id}  {line}");
    }
}

#[tokio::test]
async fn nsfw_switch_hides_nsfw_providers_and_reaches_extensions() {
    let host = test_host();
    let code = "raygun.providers.register({ kind: 'music.metadata', id: 'p', name: 'P', run: async () => raygun.content.showNsfw() });";
    let adult = ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            &manifest_json(
                json!({ "id": "dev.raygun.adult", "name": "Adult", "contentRating": "nsfw" }),
            ),
        ),
        ("index.js", code),
    ]);
    let infos = host
        .load(vec![extension("dev.raygun.safe", code), adult])
        .await;
    let rating = |id: &str| {
        infos
            .iter()
            .find(|info| info.id == id)
            .map(|info| info.content_rating.clone())
    };
    assert_eq!(rating("dev.raygun.adult").as_deref(), Some("nsfw"));
    assert_eq!(rating("dev.raygun.safe").as_deref(), Some("safe"));

    let ids = |providers: Vec<raygun_model::ProviderInfo>| {
        providers.into_iter().map(|p| p.id).collect::<Vec<_>>()
    };
    assert_eq!(
        ids(host.providers("music.metadata").await),
        ["dev.raygun.safe/p"],
        "off by default"
    );
    let call = |id: &'static str| host.call(id, "run", json!({}));
    assert_eq!(call("dev.raygun.safe/p").await.unwrap(), json!(false));

    host.content().set_show_nsfw(true);
    assert_eq!(host.providers("music.metadata").await.len(), 2);
    assert_eq!(call("dev.raygun.safe/p").await.unwrap(), json!(true));
}

/// `cargo test -p raygun-ext-host -- --ignored mangadex_filters`.
#[tokio::test]
#[ignore = "needs network access to api.mangadex.org"]
async fn mangadex_filters_live() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let provider = "dev.raygun.mangadex/mangadex";
    let raw = host.call(provider, "filters", json!({})).await;
    println!(
        "raw: {:?}",
        raw.as_ref()
            .map(|value| value.to_string().chars().take(300).collect::<String>())
    );
    let filters = host.manga_filters(provider).await.unwrap();
    println!("{} filters", filters.len());
    assert!(filters.len() >= 4, "sort, status, demographic and tags");
    let mut chosen = std::collections::BTreeMap::new();
    chosen.insert(
        "status".to_owned(),
        raygun_model::FilterValue::Select("completed".into()),
    );
    let listing = host
        .manga_search_filtered(provider, "", 1, &chosen)
        .await
        .unwrap();
    println!("{} completed titles", listing.items.len());
    assert!(!listing.items.is_empty());
}

#[tokio::test]
async fn video_sources_list_episodes_and_resolve_servers_one_by_one() {
    let host = test_host();
    let code = r#"
        raygun.providers.register({
          kind: 'video.source', id: 'v', name: 'Films',
          popular: async () => ({ items: [{ id: 'show', title: 'Show' }], hasNextPage: false }),
          search: async () => ({ items: [], hasNextPage: false }),
          details: async () => ({ title: 'Show', type: 'series', year: 1999, cast: ['A'], directors: ['B'] }),
          episodes: async () => ({ episodes: [
            { id: 'e1', season: 1, number: 1 }, { id: 'e2', season: 1, number: 2, name: 'Two' },
          ] }),
          servers: async ({ episodeId }) => ({ servers: [
            { id: `${episodeId}:fast`, name: 'Fast', videos: [{ url: 'https://cdn.example/a.mp4', quality: 720 }] },
            { id: `${episodeId}:slow`, name: 'Slow' },
            { id: `${episodeId}:broken`, name: 'Broken' },
          ] }),
          videos: async ({ serverId }) => {
            if (serverId.endsWith(':broken')) throw new Error('host is down');
            return { videos: [{ url: 'https://cdn.example/b.m3u8', format: 'hls', quality: 1080 }] };
          },
        });
    "#;
    let source = ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            &manifest_json(json!({ "id": "dev.raygun.films", "provides": ["video.source"] })),
        ),
        ("index.js", code),
    ]);
    let infos = host.load(vec![source]).await;
    assert_eq!(
        infos[0].status,
        ExtensionStatus::Loaded,
        "{:?}",
        infos[0].error
    );
    let provider = "dev.raygun.films/v";
    assert_eq!(host.providers(crate::video::VIDEO_SOURCE).await.len(), 1);

    let details = host.manga_details(provider, "show").await.unwrap();
    assert_eq!(
        (details.media_type.as_deref(), details.year),
        (Some("series"), Some(1999))
    );
    assert_eq!(
        (details.authors, details.artists),
        (vec!["A".to_owned()], vec!["B".to_owned()])
    );
    let episodes = host.video_episodes(provider, "show").await.unwrap();
    assert_eq!(episodes[0].name, "Episode 1");
    assert_eq!(episodes[1].volume.as_deref(), Some("1"));

    let servers = host.video_servers(provider, "e1").await.unwrap();
    assert_eq!(servers.len(), 3);
    assert_eq!(servers[0].videos.as_ref().unwrap()[0].quality, Some(720));
    let (slow, broken) = futures_util::future::join(
        host.video_links(provider, &servers[1].id),
        host.video_links(provider, &servers[2].id),
    )
    .await;
    assert_eq!(slow.unwrap()[0].format.as_deref(), Some("hls"));
    assert!(broken.unwrap_err().contains("host is down"));
}

#[test]
fn settings_without_labels_read_as_words() {
    assert_eq!(crate::host::key_label("itemsPerSearch"), "Items per search");
    assert_eq!(
        crate::host::key_label("items_per_search"),
        "Items per search"
    );
    assert_eq!(crate::host::key_label("apiKey"), "Api key");
    assert_eq!(crate::host::key_label("quality"), "Quality");
}
