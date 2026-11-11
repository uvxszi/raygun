use std::net::SocketAddr;
use std::time::Duration;

use axum::Router;
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::{Html, IntoResponse, Redirect};
use axum::routing::{get, post};
use raygun_model::{
    ExtensionOrigin, ExtensionStatus, LyricsTranslationKind, LyricsTranslationOrigin, MediaSource,
};
use raygun_net::IssueKind;
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
                    "/challenge",
                    get(|| async {
                        (
                            StatusCode::FORBIDDEN,
                            [("cf-mitigated", "challenge")],
                            Html("<title>Just a moment...</title>"),
                        )
                    }),
                )
                .route("/guarded", get(guarded).post(guarded))
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

/// The User-Agent `/guarded`'s pass holds for, as Cloudflare's `cf_clearance` holds for the
/// browser that earned it.
const CHECKED_BROWSER: &str = "Checked Browser/1.0";

/// A page behind a browser check: Cloudflare's challenge answer, unless the request carries the
/// pass and the User-Agent it was passed with.
async fn guarded(headers: HeaderMap) -> axum::response::Response {
    let value = |name: header::HeaderName| {
        headers
            .get(name)
            .and_then(|value| value.to_str().ok())
            .unwrap_or("")
            .to_owned()
    };
    if value(header::COOKIE).contains("cf_clearance=passed")
        && value(header::USER_AGENT) == CHECKED_BROWSER
    {
        Html("<title>Novels</title>sword").into_response()
    } else {
        (
            StatusCode::FORBIDDEN,
            [("cf-mitigated", "challenge")],
            Html("<title>Just a moment...</title>"),
        )
            .into_response()
    }
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
    assert_eq!(
        host.call("dev.raygun.test/p", "nope", json!({})).await,
        Err(CallError::MissingMethod {
            name: "P".into(),
            provider: "dev.raygun.test/p".into(),
            method: "nope".into(),
        })
    );
}

#[tokio::test]
async fn youtube_files_are_asked_of_raygun_s_module_through_an_installed_source() {
    let host = provider_host("return null;").await;
    let using = crate::StreamUse::default();
    // The module answers through the extension's own fetch; what it says comes back as YouTube's.
    let answer = host
        .youtube_audio(
            "dev.raygun.test",
            "not a video",
            using,
            Duration::from_secs(5),
        )
        .await;
    match answer {
        Err(CallError::Said { name, message }) => {
            assert_eq!(name, "YouTube");
            assert!(message.contains("isn't a YouTube video"), "{message}");
        }
        other => panic!("YouTube's own words: {other:?}"),
    }
    // A source that isn't installed can't ask.
    assert_eq!(
        host.youtube_audio(
            "dev.raygun.missing",
            "dQw4w9WgXcQ",
            using,
            Duration::from_secs(5)
        )
        .await,
        Err(CallError::UnknownProvider("dev.raygun.missing".into()))
    );
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

/// What a check window saw: `pages` in order, each an address and a title; a check's page is
/// one whose answer said so (`cf-mitigated`), translated titles and all.
fn check_window(pages: &[(&str, &str, bool)]) -> crate::browser_check::CheckWindow {
    use crate::browser_check::Seen;
    let mut window = crate::browser_check::CheckWindow::default();
    for (url, title, check) in pages {
        if *check {
            window.see(Seen::Answer {
                url: (*url).into(),
                status: 403,
                mitigated: true,
            });
        }
        window.see(Seen::Start { url: (*url).into() });
        window.see(Seen::Title {
            title: (*title).into(),
        });
        window.see(Seen::Finish { url: (*url).into() });
    }
    window
}

#[tokio::test]
async fn a_browser_check_opens_the_failed_request_and_its_pass_comes_back() {
    use raygun_model::BrowserCheckOutcome;

    let address = start_server();
    let dir = tempfile::tempdir().unwrap();
    let host = ExtensionHost::start(HostConfig {
        storage_dir: Some(dir.path().to_owned()),
        call_timeout: Duration::from_secs(10),
        ..HostConfig::default()
    })
    .unwrap();
    let code = "raygun.providers.register({ kind: 'music.metadata', id: 'p', name: 'P', run: async (args) => { const answer = await raygun.http.fetch(args.url, args.options); if (answer.status !== 200) throw new Error('no data'); return answer.text(); } });";
    host.load(vec![extension("dev.raygun.test", code)]).await;
    let page = format!("http://{address}/guarded?q=sword");
    let search = json!({
        "url": page,
        "options": {
            "method": "POST",
            "headers": { "Referer": format!("http://{address}/"), "Content-Type": "application/x-www-form-urlencoded" },
            "body": "q=sword",
        },
    });

    // The request that met the check is what the window opens: its address, method, headers,
    // body and User-Agent, not the site's front page.
    let error = run(&host, search.clone()).await.unwrap_err();
    assert_eq!(error.kind(), Some(IssueKind::BotCheck));
    let check = host.browser_check(&error.to_string(), None).unwrap();
    assert_eq!(check.extension_id, "dev.raygun.test");
    assert_eq!(check.url(), page);
    let window = check.request.window().unwrap();
    assert_eq!(window.method, "POST");
    assert_eq!(window.body.as_deref(), Some("q=sword"));
    assert_eq!(window.user_agent, raygun_net::user_agent());
    assert_eq!(
        window.headers["Referer"],
        format!("http://{address}/"),
        "{window:?}"
    );
    let call = check.call.clone().unwrap();
    assert_eq!(
        (call.provider.as_str(), call.method.as_str()),
        ("dev.raygun.test/p", "run")
    );

    let cookies = crate::browser_check::parse_cookie_header("cf_clearance=passed");
    // A page that loads without a check changes nothing, and says so.
    let plain = check_window(&[(&page, "Novels", false)]);
    let result = host
        .finish_browser_check(&check, &plain, &cookies, Some(CHECKED_BROWSER))
        .await
        .unwrap();
    assert_eq!(result.outcome, BrowserCheckOutcome::NoCheck);
    assert_eq!(result.cookies, 0);
    // Closed while the check still showed.
    let closed = check_window(&[(&page, "Un instant…", true)]);
    let result = host
        .finish_browser_check(&check, &closed, &cookies, Some(CHECKED_BROWSER))
        .await
        .unwrap();
    assert_eq!(result.outcome, BrowserCheckOutcome::NotPassed);
    assert!(
        run(&host, search.clone()).await.is_err(),
        "nothing was taken"
    );

    // Passed with a browser the site's pass doesn't hold for: the call meets the check again.
    let passed = check_window(&[(&page, "Un instant…", true), (&page, "Novels", false)]);
    let result = host
        .finish_browser_check(&check, &passed, &cookies, Some("Other Browser/2.0"))
        .await
        .unwrap();
    assert_eq!(result.outcome, BrowserCheckOutcome::StillBlocked);
    assert_eq!(result.cookies, 1);
    assert!(result.error.unwrap().contains("browser check"));

    // Passed: the browser's cookies and User-Agent go to the extension, and the failed call is
    // asked again and works.
    let check = host.browser_check(&error.to_string(), None).unwrap();
    let result = host
        .finish_browser_check(&check, &passed, &cookies, Some(CHECKED_BROWSER))
        .await
        .unwrap();
    assert_eq!(result.outcome, BrowserCheckOutcome::Passed, "{result:?}");
    assert_eq!((result.cookies, result.error), (1, None));
    assert_eq!(
        run(&host, search).await.unwrap(),
        json!("<title>Novels</title>sword")
    );
    let seen = run(&host, json!({ "url": format!("http://{address}/whoami") }))
        .await
        .unwrap();
    assert_eq!(
        seen.as_str().unwrap(),
        format!("cf_clearance=passed|{CHECKED_BROWSER}")
    );
    assert_eq!(host.browser_check(&error.to_string(), None), None);
    assert!(
        host.import_browser_cookies("../x", &page, &cookies, None)
            .is_err()
    );
}

#[tokio::test]
async fn thrown_errors_become_script_errors() {
    let host = provider_host("throw new Error('boom from js');").await;
    // Users see the message alone; the stack trace only goes to the log.
    let error = run(&host, json!({})).await.unwrap_err();
    assert_eq!(
        error,
        CallError::Said {
            name: "P".into(),
            message: "boom from js".into()
        }
    );
    assert_eq!(error.to_string(), "P: boom from js");
    // JavaScript's own errors say the extension failed, the details below.
    let host = provider_host("return args.missing.field;").await;
    let error = run(&host, json!({})).await.unwrap_err();
    assert_eq!(error.kind(), Some(raygun_net::IssueKind::ExtensionFailed));
    let text = error.to_string();
    let (summary, detail) = text.split_once('\n').unwrap();
    assert_eq!(summary, "P's extension failed");
    assert!(detail.contains("undefined"), "{detail}");
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
    assert!(
        host.call(
            "dev.raygun.test/p",
            "set",
            json!({ "key": "big", "value": huge })
        )
        .await
        .unwrap_err()
        .detail()
        .contains("limited")
    );
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
    let error = run(&host, json!({ "spin": true })).await.unwrap_err();
    assert_eq!(error.kind(), Some(raygun_net::IssueKind::ExtensionFailed));
    assert!(
        matches!(
            error,
            CallError::Exhausted {
                what: crate::Exhaustion::Time,
                ..
            }
        ),
        "{error}"
    );
    assert!(error.detail().contains("without waiting"), "{error}");
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
        { title: 'yt', artist: ' "Rick Astley', source: { kind: 'youTube', videoId: 'dQw4w9WgXcQ' } },
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
    // A quote a source left before the name is gone.
    assert_eq!(tracks[2].artist.as_deref(), Some("Rick Astley"));
}

/// Songs a source marks explicit leave every list while Explicit music is off (M46.36).
#[tokio::test]
async fn explicit_songs_leave_while_explicit_music_is_off() {
    let host = test_host();
    let code = r#"raygun.providers.register({ kind: 'music.metadata', id: 'search', name: 'Marks',
      search: async () => ({ tracks: [
        { title: 'Clean', artist: 'A' },
        { title: 'Explicit', artist: 'A', explicit: true },
      ] }) });"#;
    host.load(vec![extension("dev.raygun.marks", code)]).await;
    let titles = |results: Vec<raygun_model::ProviderSearchResult>| -> Vec<String> {
        results
            .into_iter()
            .flat_map(|result| result.tracks)
            .map(|track| track.title)
            .collect()
    };
    let shown = host.search_music("song", 10).await;
    assert!(shown[0].tracks[1].explicit);
    assert_eq!(titles(shown), ["Clean", "Explicit"]);
    host.content().set_show_explicit_music(false);
    assert_eq!(titles(host.search_music("song", 10).await), ["Clean"]);
}

/// The Internet Archive's own record of an item (its `metadata`).
async fn archive_record(client: &reqwest::Client, identifier: &str) -> serde_json::Value {
    let body = client
        .get(format!(
            "https://archive.org/metadata/{identifier}/metadata"
        ))
        .send()
        .await
        .unwrap()
        .text()
        .await
        .unwrap();
    let record: serde_json::Value = serde_json::from_str(&body).unwrap();
    record["result"].clone()
}

/// The licence address the Internet Archive's own record of an item gives (empty without one).
async fn archive_licence(client: &reqwest::Client, identifier: &str) -> String {
    archive_record(client, identifier).await["licenseurl"]
        .as_str()
        .unwrap_or_default()
        .to_owned()
}

/// A public-domain mark or dedication, or a Creative Commons licence.
fn open_licence(licence: &str) -> bool {
    licence.contains("creativecommons.org") || licence.contains("publicdomain")
}

/// Whether a film's record holds up as public domain by the film source's rule: published more
/// than 95 years ago, or marked public domain and from before 1978; or a Prelinger Archives film
/// with Prelinger's dedication.
fn public_domain_film(record: &serde_json::Value) -> bool {
    let licence = record["licenseurl"].as_str().unwrap_or_default();
    let prelinger = match &record["collection"] {
        serde_json::Value::String(one) => one == "prelinger",
        serde_json::Value::Array(many) => many.iter().any(|name| name == "prelinger"),
        _ => false,
    };
    if prelinger && licence.contains("publicdomain") {
        return true;
    }
    let year = record["year"]
        .as_str()
        .or_else(|| record["date"].as_str())
        .and_then(|text| text.get(..4))
        .and_then(|year| year.parse::<i32>().ok());
    let Some(year) = year else {
        return false;
    };
    let marked = licence.contains("publicdomain/mark") || licence.contains("licenses/publicdomain");
    year < chrono::Datelike::year(&chrono::Utc::now()) - 95 || (marked && year <= 1977)
}

/// Runs the bundled Internet Archive extension against the real service, checking that every
/// track comes from an item marked public domain or openly licensed:
/// `cargo test -p raygun-ext-host -- --ignored internet_archive --nocapture`.
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

    let client = raygun_net::http_client().unwrap();
    // A netlabel artist and two broader searches: netlabel releases and Musopen's recordings.
    for query in ["kai engel", "ambient", "beethoven"] {
        let results = host.search_music(query, 20).await;
        let result = results
            .iter()
            .find(|result| result.provider.extension_id == "dev.raygun.internet-archive")
            .unwrap();
        assert_eq!(result.error, None);
        assert!(!result.tracks.is_empty(), "{query}");
        let mut items = std::collections::BTreeSet::new();
        for track in &result.tracks {
            println!(
                "{query}: {} — {:?} ({:?} ms)",
                track.title, track.artist, track.duration_ms
            );
            assert!(
                matches!(&track.source, Some(MediaSource::Url { url, .. }) if url.starts_with("https://archive.org/download/"))
            );
            let id = track.external_id.as_deref().unwrap();
            items.insert(id.split('/').next().unwrap().to_owned());
        }
        for item in items {
            let licence = archive_licence(&client, &item).await;
            println!("  {item}: {licence}");
            assert!(open_licence(&licence), "{item} carries {licence:?}");
        }
    }
}

/// The film source lists, in every collection it offers, only items whose own record holds up as
/// public domain (by age, or marked so and from before 1978), and plays one:
/// `cargo test -p raygun-ext-host -- --ignored internet_archive_films --nocapture`.
#[tokio::test]
#[ignore = "needs network access to archive.org"]
async fn internet_archive_films_live_are_public_domain() {
    use crate::manga::MangaList;
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let provider = "dev.raygun.internet-archive/films";
    let client = raygun_net::http_client().unwrap();

    // (what was listed, the titles)
    let mut lists = vec![
        (
            "popular".to_owned(),
            host.manga_list(provider, MangaList::Popular, 1)
                .await
                .unwrap(),
        ),
        (
            "latest".to_owned(),
            host.manga_list(provider, MangaList::Latest, 1)
                .await
                .unwrap(),
        ),
        (
            "search nosferatu".to_owned(),
            host.manga_search(provider, "nosferatu", 1).await.unwrap(),
        ),
    ];
    let filters = host.manga_filters(provider).await.unwrap();
    let collections = filters
        .iter()
        .find_map(|filter| match filter {
            raygun_model::SourceFilter::Select { key, options, .. } if key == "collection" => {
                Some(options.clone())
            }
            _ => None,
        })
        .expect("a collection filter");
    for option in &collections {
        let chosen = std::collections::BTreeMap::from([(
            "collection".to_owned(),
            raygun_model::FilterValue::Select(option.value.clone()),
        )]);
        let listing = host
            .manga_search_filtered(provider, "", 1, &chosen)
            .await
            .unwrap();
        lists.push((option.label.clone(), listing));
    }
    for (label, listing) in &lists {
        println!("{label}: {} titles", listing.items.len());
        assert!(!listing.items.is_empty(), "{label}");
        for item in listing.items.iter().take(8) {
            let record = archive_record(&client, &item.id).await;
            println!(
                "  {} · {} · {}",
                item.title, record["year"], record["licenseurl"]
            );
            assert!(
                public_domain_film(&record),
                "{label}: {} ({}) doesn't hold up as public domain",
                item.title,
                item.id
            );
        }
    }

    // A film plays from the Archive's own files (an item without a playable file says so).
    let mut played = false;
    for film in lists[0].1.items.iter().take(5) {
        let Ok(episodes) = host.video_episodes(provider, &film.id).await else {
            continue;
        };
        let servers = host.video_servers(provider, &episodes[0].id).await.unwrap();
        let videos = servers[0].videos.as_ref().unwrap();
        println!(
            "{}: {:?}",
            film.title,
            videos.first().map(|video| &video.url)
        );
        assert!(
            videos
                .iter()
                .all(|video| video.url.starts_with("https://archive.org/download/"))
        );
        played = true;
        break;
    }
    assert!(played, "one of the most viewed films plays");
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
      resolve: async ({ id, targetKbps, decodable }) => id === 'lazy'
        ? { source: { kind: 'youTube', videoId: 'dQw4w9WgXcQ' }, expiresAt: 1700000000000 }
        : id === 'hints'
          ? { source: { kind: 'url', url: `https://cdn.example.org/${targetKbps}-${decodable}.m4a`, headers: {} } }
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

    let playing = crate::host::StreamUse::default();
    let resolved = host
        .resolve_stream("dev.raygun.test/streams", "lazy", playing)
        .await
        .unwrap();
    assert!(matches!(resolved.source, MediaSource::YouTube { .. }));
    assert_eq!(resolved.expires_at, Some(1_700_000_000_000));
    // What the stream is for reaches the provider (M46.51).
    let download = crate::host::StreamUse {
        quality: raygun_resolver::QualityTarget::Kbps(128),
        decodable: true,
    };
    let resolved = host
        .resolve_stream("dev.raygun.test/streams", "hints", download)
        .await
        .unwrap();
    assert!(
        matches!(&resolved.source, MediaSource::Url { url, .. } if url.ends_with("/128-true.m4a")),
        "{:?}",
        resolved.source
    );
    let error = host
        .resolve_stream("dev.raygun.test/streams", "other", playing)
        .await
        .unwrap_err();
    assert_eq!(error.kind(), Some(raygun_net::IssueKind::NotFound));
    assert!(error.detail().contains("no playable stream"), "{error}");
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

    // A Creative Commons netlabel release, the kind of recording the extension streams.
    let results = host.search_music("calls and echoes kai engel", 10).await;
    let musicbrainz = results
        .iter()
        .find(|result| result.provider.extension_id == "dev.raygun.musicbrainz")
        .unwrap();
    assert_eq!(musicbrainz.error, None);
    for track in &musicbrainz.tracks {
        println!("found: {} | {:?}", track.title, track.artist);
    }
    let song = musicbrainz
        .tracks
        .iter()
        .find(|track| track.artist.as_deref() == Some("Kai Engel"))
        .expect("Kai Engel's recording ranks among the results");
    assert_eq!(song.source, None, "MusicBrainz has metadata only");

    // A second search right away stays within MusicBrainz's one-request-per-second limit.
    let again = host.search_music("rain catcher kai engel", 5).await;
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
    // The Internet Archive's own candidates: other stream sources may know the song too.
    let candidates: Vec<_> = host
        .stream_candidates(&query, 10, raygun_resolver::QualityTarget::Any)
        .await
        .into_iter()
        .filter(|answer| answer.provider.extension_id == "dev.raygun.internet-archive")
        .flat_map(|answer| {
            answer
                .result
                .unwrap_or_else(|error| panic!("{}: {error}", answer.provider.name))
        })
        .collect();
    assert!(!candidates.is_empty(), "{query:?}");
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
        title: "Calls and Echoes (Original Mix)".into(),
        artist: Some("Kai Engel & Friends".into()),
        album: None,
        duration_ms: Some(248_000),
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
    // The source's own words, with its name.
    assert_eq!(
        host.lyrics(&query).await,
        Err("dev.raygun.down: lyrics site is down".into())
    );
}

#[tokio::test]
async fn lyrics_translations_follow_their_lines() {
    let host = test_host();
    let provider = |id: &str, body: &str| {
        let manifest = manifest_json(json!({ "id": id, "name": id, "provides": ["lyrics"] }));
        let code = format!(
            "raygun.providers.register({{ kind: 'lyrics', id: 'l', name: 'Words', lyrics: async () => ({body}) }});"
        );
        ExtensionSource::built_in(&[("raygun-extension.json", &manifest), ("index.js", &code)])
    };
    // Out of order and with a bad line: each translation stays with its line.
    host.load(vec![provider(
        "dev.raygun.translated",
        r#"{
            synced: [
                { timeMs: 5000, text: 'second' },
                { timeMs: -1, text: 'bad' },
                { timeMs: 1000, text: 'first' },
            ],
            translations: [
                { language: 'en', lines: ['zweite', 'schlecht', 'erste'] },
                { language: 'ja-Latn', kind: 'romanization', lines: ['ni', null, 'ichi'] },
                { language: 'not a tag', lines: ['x', 'y', 'z'] },
                { language: 'fr', kind: 'notes', lines: ['x', 'y', 'z'] },
                { language: 'de', lines: ['', '', ''] },
            ],
        }"#,
    )])
    .await;
    let query = raygun_model::TrackQuery {
        title: "Song".into(),
        artist: Some("Band".into()),
        album: None,
        duration_ms: None,
    };
    let lyrics = host.lyrics(&query).await.unwrap().unwrap();
    let texts: Vec<&str> = lyrics
        .synced
        .iter()
        .map(|line| line.text.as_str())
        .collect();
    assert_eq!(texts, ["first", "second"]);
    let kept: Vec<(&str, LyricsTranslationKind, Vec<&str>)> = lyrics
        .translations
        .iter()
        .map(|translation| {
            (
                translation.language.as_str(),
                translation.kind,
                translation.lines.iter().map(String::as_str).collect(),
            )
        })
        .collect();
    assert_eq!(
        kept,
        [
            (
                "en",
                LyricsTranslationKind::Translation,
                vec!["erste", "zweite"]
            ),
            (
                "ja-Latn",
                LyricsTranslationKind::Romanization,
                vec!["ichi", "ni"]
            ),
        ]
    );
    assert!(lyrics.translations.iter().all(|translation| {
        translation.origin == LyricsTranslationOrigin::Source && translation.by == "Words"
    }));

    // Plain lyrics: the lines after the ones trimmed away.
    let host = test_host();
    host.load(vec![provider(
        "dev.raygun.plain",
        r#"{ plain: '\nOne\nTwo\n', translations: [{ language: 'de', lines: ['', 'Eins', 'Zwei'] }] }"#,
    )])
    .await;
    let lyrics = host.lyrics(&query).await.unwrap().unwrap();
    assert_eq!(lyrics.plain.as_deref(), Some("One\nTwo"));
    assert_eq!(lyrics.translations[0].lines, ["Eins", "Zwei"]);
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

#[tokio::test]
async fn watched_searches_report_each_source_as_it_answers() {
    let host = test_host();
    let full = r#"
        raygun.providers.register({
          kind: 'music.metadata', id: 'full', name: 'Full',
          search: async ({ query }) => ({ tracks: [{ title: query, artist: 'A' }] }),
          searchArtists: async ({ query }) => ({ artists: [{ id: 1, name: query }] }),
          searchPlaylists: async ({ featured }) => ({ playlists: [
            { id: 'p1', title: 'Mine' }, { id: 'p2', title: 'Theirs', featured: true },
          ].filter((playlist) => !featured || playlist.featured) }),
        });
    "#;
    let broken = "raygun.providers.register({ kind: 'music.metadata', id: 'songs', name: 'Broken', search: async () => { throw new Error('offline'); } });";
    host.load(vec![
        extension("dev.raygun.full", full),
        extension("dev.raygun.broken", broken),
    ])
    .await;

    let reported = std::cell::RefCell::new(Vec::new());
    let listings = raygun_model::SearchListingsPrefs::default();
    let songs = host
        .search_music_as_answered("song", 10, listings, |result, total| {
            reported.borrow_mut().push((
                result.provider.name.clone(),
                result.error.is_some(),
                total,
            ));
        })
        .await;
    let mut reported = reported.into_inner();
    reported.sort();
    assert_eq!(
        reported,
        [
            ("Broken".to_owned(), true, 2),
            ("Full".to_owned(), false, 2)
        ]
    );
    assert_eq!(songs.len(), 2, "every answer comes back, failures too");

    // Sources without the call answer with nothing, so the count still reaches the total.
    let answered = std::cell::RefCell::new(Vec::new());
    let artists = host
        .search_artists_as_answered("Daft Punk", 10, |provider, artists, total| {
            answered
                .borrow_mut()
                .push((provider.name.clone(), artists.len(), total));
        })
        .await;
    let mut answered = answered.into_inner();
    answered.sort();
    assert_eq!(
        answered,
        [("Broken".to_owned(), 0, 2), ("Full".to_owned(), 1, 2)]
    );
    assert_eq!(artists.len(), 1);
    assert_eq!(artists[0].name, "Daft Punk");

    let featured = host
        .search_playlists_as_answered("mix", true, 10, |_, _, _| {})
        .await;
    assert_eq!(featured.len(), 1);
    assert_eq!(featured[0].title, "Theirs");
    assert!(
        host.search_artists_as_answered("  ", 10, |_, _, _| panic!("nothing to ask"))
            .await
            .is_empty()
    );
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

/// `cargo test -p raygun-ext-host -- --ignored discography_live --nocapture`.
#[tokio::test]
#[ignore = "needs network access to MusicBrainz"]
async fn discography_live_releases() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let releases = host.artist_releases("Daft Punk", 100).await;
    for release in releases.iter().take(12) {
        println!(
            "{} · {:?} · {:?} · {:?}",
            release.provider.name, release.title, release.year, release.kind
        );
    }
    assert!(releases.iter().any(|release| release.title == "Discovery"));
    assert!(
        releases
            .iter()
            .any(|release| release.kind == Some(raygun_model::ReleaseKind::Single))
    );
    // Covers come from the Cover Art Archive.
    assert!(releases.iter().all(|release| {
        release
            .artwork_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://coverartarchive.org/"))
    }));
}

/// MusicBrainz's song, album and artist lookups and its genres, as tagging a library uses them,
/// with Cover Art Archive covers: `cargo test -p raygun-ext-host -- --ignored musicbrainz_live
/// --nocapture`.
#[tokio::test]
#[ignore = "needs network access to MusicBrainz"]
async fn musicbrainz_live_lookups_and_genres() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let musicbrainz = |extension: &str| extension == "dev.raygun.musicbrainz";

    let songs = host.search_music("daft punk get lucky", 10).await;
    let found = songs
        .iter()
        .find(|result| musicbrainz(&result.provider.extension_id))
        .expect("MusicBrainz searched");
    assert_eq!(found.error, None);
    for track in found.tracks.iter().take(3) {
        println!(
            "song: {} · {:?} · {:?}",
            track.title, track.artist, track.album
        );
    }
    assert!(
        found
            .tracks
            .iter()
            .any(|track| track.title.contains("Get Lucky"))
    );
    assert!(found.tracks.iter().any(|track| {
        track
            .artwork_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://coverartarchive.org/"))
    }));

    let albums = host.search_albums("random access memories", 5).await;
    let album = albums
        .iter()
        .find(|album| musicbrainz(&album.provider.extension_id))
        .expect("MusicBrainz finds the album");
    assert!(
        album
            .artwork_url
            .as_deref()
            .is_some_and(|url| url.starts_with("https://coverartarchive.org/"))
    );
    let detail = host
        .music_album(&album.provider.id, &album.id)
        .await
        .unwrap();
    println!(
        "album: {} with {} songs",
        detail.album.title,
        detail.tracks.len()
    );
    assert!(detail.tracks.len() > 10, "{}", detail.tracks.len());
    assert!(
        detail
            .tracks
            .iter()
            .all(|track| track.artwork_url.is_some())
    );
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

/// `cargo test -p raygun-ext-host -- --ignored follow_live --nocapture`.
#[tokio::test]
#[ignore = "needs network access to MusicBrainz"]
async fn follow_live_artist_events() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let mut found = 0;
    for artist in ["Barry Manilow", "Ray Wilson"] {
        let events = host.artist_events(artist, 10).await;
        for event in events.iter().take(3) {
            println!(
                "{artist}: {} · {} · {:?}",
                event.date, event.name, event.place
            );
        }
        found += events.len();
    }
    assert!(
        found > 0,
        "MusicBrainz lists coming events for touring artists"
    );
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
        // M46.48: authors, titles LibriVox files without their article, and genres.
        ("search", "jane austen"),
        ("search", "the count of monte cristo"),
        ("subject", "Mystery"),
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

/// Loyal Books (M46.47) against the site: `cargo test -p raygun-ext-host -- --ignored
/// loyalbooks_live --nocapture`.
#[tokio::test]
#[ignore = "needs network access to loyalbooks.com"]
async fn loyalbooks_live_lists_and_finds_books_with_feeds() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    host.load(discover_installed(&dir)).await;
    let provider = "dev.raygun.loyalbooks/books";
    for (list, query) in [
        ("popular", ""),
        ("search", "sherlock holmes"),
        ("subject", "Mystery"),
        ("subject", "Science fiction"),
    ] {
        let page = host.audiobooks(provider, list, query, 1).await.unwrap();
        println!(
            "{list}: {} books, more {}; first {:?}",
            page.books.len(),
            page.has_more,
            page.books.first().map(|book| (
                &book.title,
                &book.author,
                &book.feed_url,
                &book.artwork_url
            ))
        );
        assert!(!page.books.is_empty(), "{list}");
        assert!(page.books.iter().all(|book| {
            book.feed_url
                .starts_with("https://www.loyalbooks.com/book/")
                && book.feed_url.ends_with("/feed")
        }));
    }
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
        std::env::var("SHOSETSU_SCRIPTS").unwrap_or_else(|_| "shosetsu.ja.syosetsu".into());
    for id in wanted.split(',') {
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == id)
            .unwrap();
        let bytes = download(crate::repo::package_url(&index_url, entry).unwrap()).await;
        let sources = [shosetsu_package::LibrarySource {
            index_url: &index_url,
            index: &index,
        }];
        let package = shosetsu_package::assemble(entry, &bytes, &sources, |url| {
            let download = download(url);
            async move { Ok(download.await) }
        })
        .await
        .unwrap();
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

/// `SHOSETSU_DIR=<checkout> SHOSETSU_SCRIPTS=<id> PROBE=popular,search:elf,chapters:<id>,…
/// SHOSETSU_TRACE=1 cargo test -p raygun-ext-host -- --ignored shosetsu_probe --nocapture`: one
/// script's calls, step by step, with the requests it makes (to see why a script fails).
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to the script's site"]
async fn shosetsu_probe() {
    use crate::shosetsu::package::{self as shosetsu_package, LibrarySource};
    let dir = std::env::var("SHOSETSU_DIR").expect("SHOSETSU_DIR");
    let id = std::env::var("SHOSETSU_SCRIPTS").expect("SHOSETSU_SCRIPTS");
    let index_url = reqwest::Url::parse("https://example.invalid/repo/index.json").unwrap();
    let base = index_url.join(".").unwrap();
    let index = crate::repo::parse_index(
        &std::fs::read_to_string(std::path::Path::new(&dir).join("index.json")).unwrap(),
    )
    .unwrap();
    let libs_dir = std::env::var("SHOSETSU_LIBS_DIR").unwrap_or_else(|_| dir.clone());
    let libs_index = crate::repo::parse_index(
        &std::fs::read_to_string(std::path::Path::new(&libs_dir).join("index.json")).unwrap(),
    )
    .unwrap();
    let libs_url = reqwest::Url::parse("https://example.invalid/libs/index.json").unwrap();
    let entry = index
        .extensions
        .iter()
        .find(|entry| entry.id == id)
        .expect("the script");
    let client = raygun_net::http_client().unwrap();
    let bytes = survey_download(
        &client,
        Some(&dir),
        &base,
        crate::repo::package_url(&index_url, entry).unwrap(),
    )
    .await
    .unwrap();
    let sources = [
        LibrarySource {
            index_url: &index_url,
            index: &index,
        },
        LibrarySource {
            index_url: &libs_url,
            index: &libs_index,
        },
    ];
    let package = shosetsu_package::assemble(entry, &bytes, &sources, |url| {
        let local = if url.path().starts_with("/libs/") {
            (libs_dir.clone(), libs_url.join(".").unwrap())
        } else {
            (dir.clone(), base.clone())
        };
        let client = client.clone();
        async move { survey_download(&client, Some(&local.0), &local.1, url).await }
    })
    .await
    .unwrap();
    let temp = tempfile::TempDir::new().unwrap();
    crate::package::install(temp.path(), &package).unwrap();
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(discover_installed(temp.path())).await;
    eprintln!("loaded: {:?} {:?}", infos[0].status, infos[0].error);
    let provider = format!("{}/novel", entry.id);
    for step in std::env::var("PROBE")
        .unwrap_or_else(|_| "popular".into())
        .split(',')
    {
        let (method, argument) = step.split_once(':').unwrap_or((step, ""));
        let args = match method {
            "popular" | "latest" => json!({ "page": argument.parse::<u32>().unwrap_or(1) }),
            "search" => json!({ "query": argument, "page": 1 }),
            "filters" => json!({}),
            "passage" => json!({ "chapterId": argument }),
            _ => json!({ "id": argument }),
        };
        eprintln!("== {method} {argument}");
        let answer = host.call(&provider, method, args).await;
        let text = match answer {
            Ok(value) => serde_json::to_string_pretty(&value).unwrap(),
            Err(error) => format!("ERROR {error}"),
        };
        eprintln!("{}", text.chars().take(3000).collect::<String>());
    }
}

/// `SHOSETSU_DIR=<checkout of Shosetsu's repository> cargo test -p raygun-ext-host -- --ignored
/// shosetsu_load_costs --nocapture`: installs every script of a checkout into one folder and
/// loads them all as the app does at start, reporting the time it takes and each state's memory.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "reads a checkout of a Shosetsu repository"]
async fn shosetsu_load_costs() {
    use crate::shosetsu::package::{self as shosetsu_package, LibrarySource};
    let dir = std::env::var("SHOSETSU_DIR").expect("SHOSETSU_DIR");
    let index_url = reqwest::Url::parse("https://example.invalid/repo/index.json").unwrap();
    let base = index_url.join(".").unwrap();
    let index = crate::repo::parse_index(
        &std::fs::read_to_string(std::path::Path::new(&dir).join("index.json")).unwrap(),
    )
    .unwrap();
    let client = raygun_net::http_client().unwrap();
    let temp = tempfile::TempDir::new().unwrap();
    for entry in &index.extensions {
        let Ok(bytes) = survey_download(
            &client,
            Some(&dir),
            &base,
            crate::repo::package_url(&index_url, entry).unwrap(),
        )
        .await
        else {
            continue;
        };
        let sources = [LibrarySource {
            index_url: &index_url,
            index: &index,
        }];
        let package = shosetsu_package::assemble(entry, &bytes, &sources, |url| {
            let (client, dir, base) = (client.clone(), dir.clone(), base.clone());
            async move { survey_download(&client, Some(&dir), &base, url).await }
        })
        .await;
        if let Ok(package) = package {
            crate::package::install(temp.path(), &package).unwrap();
        }
    }
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let started = std::time::Instant::now();
    let infos = host.load(discover_installed(temp.path())).await;
    let took = started.elapsed();
    let loaded: Vec<_> = infos
        .iter()
        .filter(|info| info.status == ExtensionStatus::Loaded)
        .collect();
    let mut total = 0u64;
    let mut largest = (0u64, String::new());
    for info in &loaded {
        let used = host
            .call(&format!("{}/novel", info.id), "memory", json!({}))
            .await
            .ok()
            .and_then(|value| value.as_u64())
            .unwrap_or_default();
        total += used;
        if used > largest.0 {
            largest = (used, info.id.clone());
        }
    }
    eprintln!(
        "{} of {} scripts loaded in {took:?}; {} KiB of Lua state in all ({} KiB each on average, largest {} KiB: {})",
        loaded.len(),
        infos.len(),
        total / 1024,
        total / 1024 / loaded.len().max(1) as u64,
        largest.0 / 1024,
        largest.1
    );
    for info in infos
        .iter()
        .filter(|info| info.status != ExtensionStatus::Loaded)
    {
        eprintln!("  not loaded: {} {:?}", info.id, info.error);
    }
}

/// One script's run through the survey.
#[derive(Default)]
struct SurveyRow {
    id: String,
    /// The stage it stopped at (`install`, `load`, `listing`, `novel`, `chapters`, `passage`), or
    /// `ok` when a chapter's text came back.
    stage: String,
    error: String,
    /// `ok`, `none` (no search), `empty`, or the error.
    search: String,
    seconds: f64,
}

/// Runs one installed script through listing → search → novel → chapters → passage.
async fn survey_script(dir: std::path::PathBuf, id: String) -> SurveyRow {
    let started = std::time::Instant::now();
    let mut row = SurveyRow {
        id: id.clone(),
        search: "-".into(),
        ..SurveyRow::default()
    };
    let host = ExtensionHost::start(HostConfig::default()).unwrap();
    let infos = host.load(discover_installed(&dir)).await;
    let Some(info) = infos.first() else {
        row.stage = "load".into();
        row.error = "nothing loaded".into();
        return row;
    };
    if info.status != ExtensionStatus::Loaded {
        row.stage = "load".into();
        row.error = info.error.clone().unwrap_or_default();
        return row;
    }
    let provider = format!("{}/novel", info.id);
    let fail = |stage: &str, error: String| (stage.to_owned(), error);
    let result: Result<(), (String, String)> = async {
        let popular = host
            .manga_list(&provider, crate::manga::MangaList::Popular, 1)
            .await
            .map_err(|error| fail("listing", error))?;
        let novel = popular
            .items
            .first()
            .ok_or_else(|| fail("listing", "no novels".into()))?
            .clone();
        // A word of the first title, as someone would type it.
        let query = novel
            .title
            .split(|c: char| !c.is_alphanumeric())
            .find(|word| word.chars().count() >= 4)
            .unwrap_or(&novel.title)
            .to_owned();
        row.search = match host.manga_search(&provider, &query, 1).await {
            Ok(found) if found.items.is_empty() => format!("empty ({query})"),
            Ok(_) => "ok".into(),
            Err(error) if error.contains("can't search") => "none".into(),
            Err(error) => error.chars().take(100).collect(),
        };
        host.manga_details(&provider, &novel.id)
            .await
            .map_err(|error| fail("novel", error))?;
        let chapters = host
            .manga_chapters(&provider, &novel.id)
            .await
            .map_err(|error| fail("chapters", error))?;
        let first = chapters
            .first()
            .ok_or_else(|| fail("chapters", "no chapters".into()))?;
        let passage = host
            .novel_passage(&provider, &first.id)
            .await
            .map_err(|error| fail("passage", error))?;
        if passage.len() < 2 {
            return Err(fail("passage", format!("{} blocks", passage.len())));
        }
        Ok(())
    }
    .await;
    match result {
        Ok(()) => row.stage = "ok".into(),
        Err((stage, error)) => {
            row.stage = stage;
            row.error = error;
        }
    }
    row.seconds = started.elapsed().as_secs_f64();
    row
}

/// Reads a Shosetsu repository's file: from a checkout when `local` names one (`base` is the
/// address the checkout stands for), else from the web, gently and again after a 429.
async fn survey_download(
    client: &reqwest::Client,
    local: Option<&str>,
    base: &reqwest::Url,
    url: reqwest::Url,
) -> Result<Vec<u8>, String> {
    if let Some(dir) = local {
        let relative = base
            .make_relative(&url)
            .ok_or("not in the repository")?
            .replace("%20", " ");
        return std::fs::read(std::path::Path::new(dir).join(relative))
            .map_err(|error| error.to_string());
    }
    for attempt in 0..4u64 {
        // GitLab answers 429 to a burst of raw downloads.
        tokio::time::sleep(Duration::from_millis(500 + attempt * 4000)).await;
        let response = client
            .get(url.clone())
            .send()
            .await
            .map_err(|error| error.to_string())?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 3 {
            continue;
        }
        let response = response
            .error_for_status()
            .map_err(|error| error.to_string())?;
        return response
            .bytes()
            .await
            .map(|bytes| bytes.to_vec())
            .map_err(|error| error.to_string());
    }
    Err("rate limited".into())
}

/// `cargo test -p raygun-ext-host -- --ignored shosetsu_survey --nocapture`: every script in a
/// Shosetsu repository, each reporting how far it got: install, load, listing, search, novel,
/// chapters, passage. Prints a table instead of failing, to find runtime gaps.
/// - `SHOSETSU_INDEX`: the repository's `index.json` (Shosetsu's official one by default);
/// - `SHOSETSU_DIR`: read the index, scripts and libraries from a checkout of it instead;
/// - `SHOSETSU_LIBS_DIR`: a checkout of Shosetsu's official repository, for libraries a
///   community repository relies on without listing them (as when both are added);
/// - `SHOSETSU_SCRIPTS`: only these ids (comma-separated);
/// - `SURVEY_JOBS`: scripts checked at once (8); `SURVEY_OUT`: also write the table there.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "needs network access to the repository and many novel sites"]
async fn shosetsu_survey() {
    use crate::shosetsu::package::{self as shosetsu_package, LibrarySource};
    use futures_util::StreamExt;
    let index_url = reqwest::Url::parse(&std::env::var("SHOSETSU_INDEX").unwrap_or_else(|_| {
        "https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json".into()
    }))
    .unwrap();
    let client = raygun_net::http_client().unwrap();
    let local = std::env::var("SHOSETSU_DIR").ok();
    let base = index_url.join(".").unwrap();
    let text = String::from_utf8(
        survey_download(&client, local.as_deref(), &base, index_url.clone())
            .await
            .unwrap(),
    )
    .unwrap();
    let index = match crate::repo::parse_index(&text) {
        Ok(index) => index,
        Err(error) => {
            eprintln!("the index doesn't parse: {error}");
            return;
        }
    };
    let libs_dir = std::env::var("SHOSETSU_LIBS_DIR").ok();
    let libs_url =
        reqwest::Url::parse("https://gitlab.com/shosetsuorg/extensions/-/raw/dev/index.json")
            .unwrap();
    let libs_base = libs_url.join(".").unwrap();
    let libs_index = match &libs_dir {
        Some(dir) => crate::repo::parse_index(
            &std::fs::read_to_string(std::path::Path::new(dir).join("index.json")).unwrap(),
        )
        .ok(),
        None => None,
    };
    eprintln!("{} usable scripts in {index_url}", index.extensions.len());
    let wanted = std::env::var("SHOSETSU_SCRIPTS").ok();
    let root = tempfile::TempDir::new().unwrap();
    let mut rows = Vec::new();
    let mut installed = Vec::new();
    // Installed one by one (the repository host rate-limits), checked several at a time.
    for entry in &index.extensions {
        if wanted
            .as_deref()
            .is_some_and(|list| !list.split(',').any(|id| id == entry.id))
        {
            continue;
        }
        let dir = root.path().join(&entry.id);
        let result: Result<(), String> = async {
            let bytes = survey_download(
                &client,
                local.as_deref(),
                &base,
                crate::repo::package_url(&index_url, entry).unwrap(),
            )
            .await?;
            let mut sources = vec![LibrarySource {
                index_url: &index_url,
                index: &index,
            }];
            if let Some(libs_index) = &libs_index {
                sources.push(LibrarySource {
                    index_url: &libs_url,
                    index: libs_index,
                });
            }
            let package = shosetsu_package::assemble(entry, &bytes, &sources, |url| {
                let from_libs = libs_base
                    .make_relative(&url)
                    .is_some_and(|path| !path.starts_with(".."))
                    && base
                        .make_relative(&url)
                        .is_none_or(|path| path.starts_with(".."));
                let (dir, root) = if from_libs {
                    (libs_dir.clone(), libs_base.clone())
                } else {
                    (local.clone(), base.clone())
                };
                let client = client.clone();
                async move { survey_download(&client, dir.as_deref(), &root, url).await }
            })
            .await?;
            crate::package::install(&dir, &package)
                .map(|_| ())
                .map_err(|error| error.to_string())
        }
        .await;
        match result {
            Ok(()) => installed.push((dir, entry.id.clone())),
            Err(error) => rows.push(SurveyRow {
                id: entry.id.clone(),
                stage: "install".into(),
                error,
                search: "-".into(),
                ..SurveyRow::default()
            }),
        }
    }
    let jobs = std::env::var("SURVEY_JOBS")
        .ok()
        .and_then(|jobs| jobs.parse().ok())
        .unwrap_or(8);
    let checked: Vec<SurveyRow> = futures_util::stream::iter(installed)
        .map(|(dir, id)| survey_script(dir, id))
        .buffer_unordered(jobs)
        .collect()
        .await;
    rows.extend(checked);
    rows.sort_by(|a, b| a.id.cmp(&b.id));
    let working = rows.iter().filter(|row| row.stage == "ok").count();
    let searching = rows.iter().filter(|row| row.search == "ok").count();
    let mut table = format!(
        "{working} of {} scripts read a chapter; {searching} searched\n\n| script | result | search | time | error |\n|---|---|---|---|---|\n",
        rows.len()
    );
    for row in &rows {
        let error: String = row
            .error
            .replace('|', "/")
            .replace('\n', " ")
            .chars()
            .take(220)
            .collect();
        let search = row.search.replace('|', "/").replace('\n', " ");
        table.push_str(&format!(
            "| {} | {} | {search} | {:.0}s | {error} |\n",
            row.id, row.stage, row.seconds
        ));
    }
    eprintln!("{table}");
    if let Ok(path) = std::env::var("SURVEY_OUT") {
        std::fs::write(path, &table).unwrap();
    }
}

#[tokio::test]
async fn nsfw_switch_hides_nsfw_providers_and_reaches_extensions() {
    let host = test_host();
    // A lyrics provider (not music.*, which follows Explicit music since M46.36) and a music one.
    let code = "raygun.providers.register({ kind: 'lyrics', id: 'p', name: 'P', run: async () => raygun.content.showNsfw() }); raygun.providers.register({ kind: 'music.metadata', id: 'm', name: 'M', run: async () => true });";
    let adult = ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            &manifest_json(json!({
                "id": "dev.raygun.adult",
                "name": "Adult",
                "contentRating": "nsfw",
                "provides": ["lyrics", "music.metadata"],
            })),
        ),
        ("index.js", code),
    ]);
    let safe = ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            &manifest_json(json!({
                "id": "dev.raygun.safe",
                "name": "Safe",
                "provides": ["lyrics", "music.metadata"],
            })),
        ),
        ("index.js", code),
    ]);
    let infos = host.load(vec![safe, adult]).await;
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
        ids(host.providers("lyrics").await),
        ["dev.raygun.safe/p"],
        "off by default"
    );
    // Music follows Explicit music, on by default, whatever the NSFW switch says.
    assert_eq!(host.providers("music.metadata").await.len(), 2);
    host.content().set_show_explicit_music(false);
    assert_eq!(
        ids(host.providers("music.metadata").await),
        ["dev.raygun.safe/m"]
    );
    let call = |id: &'static str| host.call(id, "run", json!({}));
    assert_eq!(call("dev.raygun.safe/p").await.unwrap(), json!(false));

    host.content().set_show_nsfw(true);
    assert_eq!(host.providers("lyrics").await.len(), 2);
    assert_eq!(host.providers("music.metadata").await.len(), 1);
    assert_eq!(call("dev.raygun.safe/p").await.unwrap(), json!(true));
}

#[tokio::test]
async fn user_marks_decide_which_providers_are_adult_without_a_reload() {
    use raygun_model::{ContentOverride, ContentOverrides};
    let host = test_host();
    let code =
        "raygun.providers.register({ kind: 'lyrics', id: 'p', name: 'P', run: async () => null });";
    let source = |id: &str, rating: &str| {
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                &manifest_json(json!({
                    "id": id,
                    "name": id,
                    "contentRating": rating,
                    "provides": ["lyrics"],
                })),
            ),
            ("index.js", code),
        ])
    };
    host.load(vec![
        source("dev.raygun.rated", "nsfw"),
        source("dev.raygun.unrated", "safe"),
        source("dev.raygun.other", "safe"),
    ])
    .await;
    let ids = |providers: Vec<raygun_model::ProviderInfo>| {
        providers.into_iter().map(|p| p.id).collect::<Vec<_>>()
    };
    assert_eq!(
        ids(host.providers("lyrics").await),
        ["dev.raygun.other/p", "dev.raygun.unrated/p"]
    );

    // The rated one isn't 18+; the unrated one came from a repository marked 18+.
    host.content()
        .set_extension_repositories([("dev.raygun.unrated".to_owned(), "repo".to_owned())].into());
    host.content().set_overrides(ContentOverrides {
        extensions: [("dev.raygun.rated".to_owned(), ContentOverride::NotAdult)].into(),
        repositories: [("repo".to_owned(), ContentOverride::Adult)].into(),
        addons: Default::default(),
    });
    assert_eq!(
        ids(host.providers("lyrics").await),
        ["dev.raygun.other/p", "dev.raygun.rated/p"]
    );
    let listed = host.list().await;
    let adult = |id: &str| listed.iter().find(|info| info.id == id).unwrap().adult;
    assert!(!adult("dev.raygun.rated"));
    assert!(adult("dev.raygun.unrated"));

    host.content().set_show_nsfw(true);
    let shown = host.providers("lyrics").await;
    assert_eq!(shown.len(), 3);
    assert!(
        shown
            .iter()
            .any(|p| p.id == "dev.raygun.unrated/p" && p.adult)
    );
    assert!(
        shown
            .iter()
            .any(|p| p.id == "dev.raygun.rated/p" && !p.adult)
    );
    // Search's "No 18+" follows the marks too.
    let without = crate::without_adult_sources(true, host.providers("lyrics")).await;
    assert_eq!(ids(without).len(), 2);
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

/// Two calls into one extension at once, the quick one finishing first (M46.45): the slow one's
/// timer still fires, as a title's details did not while its episodes came back at once.
#[tokio::test]
async fn a_call_still_finishes_when_another_into_the_same_extension_ends_first() {
    let host = test_host();
    let code = "raygun.providers.register({ kind: 'music.metadata', id: 'p', name: 'P', \
        slow: async () => { await raygun.time.sleep(200); return 'slow'; }, \
        quick: async () => 'quick' });";
    host.load(vec![extension("dev.raygun.test", code)]).await;
    let both = tokio::time::timeout(
        Duration::from_secs(5),
        futures_util::future::join(
            host.call("dev.raygun.test/p", "slow", json!({})),
            host.call("dev.raygun.test/p", "quick", json!({})),
        ),
    )
    .await
    .expect("both calls answer");
    assert_eq!(both.0.unwrap(), json!("slow"));
    assert_eq!(both.1.unwrap(), json!("quick"));
}
