//! Many sources at once (M46.48): lanes keep a busy extension from holding up the rest, calls
//! nobody waits for stop with their requests, failures say what happened, and a search over
//! dozens of fast, slow, failing and hanging sources answers without waiting on the slowest.
//! `cargo test -p raygun-ext-host many_ -- --nocapture` prints the timings.

use std::net::SocketAddr;
use std::sync::Arc;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::http::{StatusCode, header};
use axum::response::IntoResponse;
use axum::routing::get;
use raygun_model::ExtensionStatus;
use raygun_net::IssueKind;
use serde_json::json;

use crate::{CallError, ExtensionHost, ExtensionSource, HostConfig, Limits};

/// Requests the counting route is answering at once, and the most it ever did.
#[derive(Default)]
struct Load {
    now: AtomicUsize,
    most: AtomicUsize,
}

fn start_server(load: Arc<Load>) -> SocketAddr {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let address = listener.local_addr().unwrap();
    std::thread::spawn(move || {
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .unwrap();
        runtime.block_on(async move {
            let app = Router::new()
                .route(
                    "/slow",
                    get(|| async {
                        tokio::time::sleep(Duration::from_millis(400)).await;
                        r#"{"tracks":[{"title":"Slow song"}]}"#
                    }),
                )
                .route(
                    "/hang",
                    get(|| async {
                        tokio::time::sleep(Duration::from_secs(60)).await;
                        "late"
                    }),
                )
                .route(
                    "/check",
                    get(|| async {
                        (
                            StatusCode::FORBIDDEN,
                            [
                                (header::CONTENT_TYPE, "text/html"),
                                (header::HeaderName::from_static("cf-mitigated"), "challenge"),
                            ],
                            "<!DOCTYPE html><html><head><title>Just a moment...</title></head></html>",
                        )
                            .into_response()
                    }),
                )
                .route(
                    "/limit",
                    get(|| async { (StatusCode::TOO_MANY_REQUESTS, "slow down").into_response() }),
                )
                .route(
                    "/big",
                    get(|| async {
                        let row = r#"<li class="item"><a href="/title/12345">A "quoted" title</a></li>"#;
                        row.repeat(4 * 1024 * 1024 / row.len())
                    }),
                )
                .route(
                    "/count",
                    get(move || {
                        let load = load.clone();
                        async move {
                            let now = load.now.fetch_add(1, Ordering::SeqCst) + 1;
                            load.most.fetch_max(now, Ordering::SeqCst);
                            tokio::time::sleep(Duration::from_millis(150)).await;
                            load.now.fetch_sub(1, Ordering::SeqCst);
                            "counted"
                        }
                    }),
                );
            let listener = tokio::net::TcpListener::from_std(listener).unwrap();
            axum::serve(listener, app).await.unwrap();
        });
    });
    address
}

fn manifest(id: &str, name: &str) -> String {
    json!({
        "id": id,
        "name": name,
        "version": "1.0.0",
        "apiVersion": 1,
        "runtime": "js",
        "entry": "index.js",
        "provides": ["music.metadata"],
        "permissions": { "net": ["127.0.0.1"] },
    })
    .to_string()
}

/// An extension `id` whose provider `p` (named `name`) has `methods`, an object literal's body.
fn extension(id: &str, name: &str, methods: &str) -> ExtensionSource {
    let code = format!(
        "raygun.providers.register({{ kind: 'music.metadata', id: 'p', name: '{name}', {methods} }});"
    );
    ExtensionSource::built_in(&[
        ("raygun-extension.json", &manifest(id, name)),
        ("index.js", &code),
    ])
}

fn host(lanes: usize) -> ExtensionHost {
    ExtensionHost::start(HostConfig {
        limits: Limits {
            cpu_budget: Duration::from_secs(5),
            ..Limits::default()
        },
        call_timeout: Duration::from_secs(20),
        lanes,
        ..HostConfig::default()
    })
    .unwrap()
}

async fn load(host: &ExtensionHost, sources: Vec<ExtensionSource>) {
    for info in host.load(sources).await {
        assert_eq!(info.status, ExtensionStatus::Loaded, "{:?}", info.error);
    }
}

/// How long a quick call takes while another extension runs script for a second and a half.
async fn quick_call_while_another_is_busy(lanes: usize) -> Duration {
    let host = host(lanes);
    load(
        &host,
        vec![
            extension(
                "dev.test.busy",
                "Busy",
                "run: async () => { const end = Date.now() + 1500; while (Date.now() < end) {} return 'done'; }",
            ),
            extension("dev.test.quick", "Quick", "run: async () => 'quick'"),
        ],
    )
    .await;
    let busy = {
        let host = host.clone();
        tokio::spawn(async move { host.call("dev.test.busy/p", "run", json!({})).await })
    };
    tokio::time::sleep(Duration::from_millis(100)).await;
    let started = Instant::now();
    assert_eq!(
        host.call("dev.test.quick/p", "run", json!({})).await,
        Ok(json!("quick"))
    );
    let quick = started.elapsed();
    assert_eq!(busy.await.unwrap(), Ok(json!("done")));
    quick
}

#[tokio::test(flavor = "multi_thread")]
async fn many_a_busy_extension_holds_up_only_its_own_lane() {
    let shared = quick_call_while_another_is_busy(1).await;
    let apart = quick_call_while_another_is_busy(2).await;
    println!(
        "quick call while another extension is busy: one lane {shared:?}, two lanes {apart:?}"
    );
    assert!(shared >= Duration::from_millis(1000), "{shared:?}");
    assert!(apart < Duration::from_millis(500), "{apart:?}");
}

#[tokio::test(flavor = "multi_thread")]
async fn many_a_call_nobody_waits_for_stops_its_requests() {
    let address = start_server(Arc::default());
    let host = host(2);
    load(
        &host,
        vec![extension(
            "dev.test.stop",
            "Stop",
            &format!(
                "run: async () => {{
                    try {{ await raygun.http.fetch('http://{address}/hang'); globalThis.after = 'fetched'; }}
                    catch (error) {{ globalThis.after = error.message; }}
                    return 'finished';
                }},
                peek: async () => globalThis.after ?? 'waiting'"
            ),
        )],
    )
    .await;
    let started = Instant::now();
    let error = host
        .call_within(
            "dev.test.stop/p",
            "run",
            json!({}),
            Duration::from_millis(300),
        )
        .await
        .unwrap_err();
    assert!(matches!(error, CallError::Timeout { .. }), "{error:?}");
    assert_eq!(
        error.to_string().lines().next(),
        Some("Stop didn't answer in time")
    );
    // The request was stopped with the call, not left to hang for a minute.
    tokio::time::sleep(Duration::from_millis(200)).await;
    let after = host
        .call("dev.test.stop/p", "peek", json!({}))
        .await
        .unwrap();
    assert!(
        after.as_str().unwrap().starts_with("stopped"),
        "{after} after {:?}",
        started.elapsed()
    );
}

#[tokio::test(flavor = "multi_thread")]
async fn many_requests_to_one_site_take_turns() {
    let load_seen = Arc::new(Load::default());
    let address = start_server(load_seen.clone());
    let host = host(2);
    load(
        &host,
        vec![extension(
            "dev.test.many",
            "Many",
            &format!(
                "run: async () => (await Promise.all(Array.from({{ length: 15 }}, () =>
                    raygun.http.fetch('http://{address}/count').then((response) => response.text())))).length"
            ),
        )],
    )
    .await;
    let started = Instant::now();
    assert_eq!(
        host.call("dev.test.many/p", "run", json!({})).await,
        Ok(json!(15))
    );
    let most = load_seen.most.load(Ordering::SeqCst);
    println!(
        "15 requests to one site: at most {most} at once, {:?} in all",
        started.elapsed()
    );
    assert!(most <= 6, "{most}");
    assert!(most >= 2, "{most}");
}

#[tokio::test(flavor = "multi_thread")]
async fn many_failures_say_what_the_site_did() {
    let address = start_server(Arc::default());
    // A port nothing listens on.
    let closed = std::net::TcpListener::bind("127.0.0.1:0")
        .unwrap()
        .local_addr()
        .unwrap()
        .port();
    let host = host(2);
    load(
        &host,
        vec![
            extension(
                "dev.test.check",
                "Checked",
                &format!(
                    "run: async () => {{ const response = await raygun.http.fetch('http://{address}/check'); return (await response.json()).items; }}"
                ),
            ),
            extension(
                "dev.test.limit",
                "Limited",
                &format!(
                    "run: async () => {{ const response = await raygun.http.fetch('http://{address}/limit');
                       if (!response.ok) throw new Error(`HTTP ${{response.status}}`); return 1; }}"
                ),
            ),
            extension(
                "dev.test.gone",
                "Gone",
                &format!("run: async () => (await raygun.http.fetch('http://127.0.0.1:{closed}/')).text()"),
            ),
        ],
    )
    .await;
    let summary = |error: CallError| error.to_string().lines().next().unwrap().to_owned();

    let checked = host
        .call("dev.test.check/p", "run", json!({}))
        .await
        .unwrap_err();
    assert_eq!(checked.kind(), Some(IssueKind::BotCheck));
    assert_eq!(
        summary(checked.clone()),
        "Checked wants a browser check, which Raygun can't do"
    );
    // The raw details stay for More info.
    assert!(checked.detail().contains("HTTP 403"), "{checked}");

    let limited = host
        .call("dev.test.limit/p", "run", json!({}))
        .await
        .unwrap_err();
    assert_eq!(limited.kind(), Some(IssueKind::RateLimited));
    assert_eq!(summary(limited), "Limited is limiting requests — try later");

    let gone = host
        .call("dev.test.gone/p", "run", json!({}))
        .await
        .unwrap_err();
    assert_eq!(gone.kind(), Some(IssueKind::Refused), "{gone}");
    assert_eq!(summary(gone), "Gone refused the connection — try later");
}

/// What a big page costs a script: handed over as a string of its own, against the JSON
/// envelope it used to come in (escaped by Rust, then parsed back in the script; only the
/// script's half is measured here). `cargo test -p raygun-ext-host -- --ignored many_big_pages
/// --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement"]
async fn many_big_pages_reach_scripts_without_a_json_round_trip() {
    let address = start_server(Arc::default());
    let host = host(2);
    load(
        &host,
        vec![extension(
            "dev.test.big",
            "Big",
            &format!(
                "run: async ({{ envelope }}) => {{
                    const started = Date.now();
                    const response = await raygun.http.fetch('http://{address}/big');
                    let body = await response.text();
                    if (envelope) {{
                        const text = JSON.stringify({{ value: {{ status: response.status, url: response.url, headers: response.headers, body }} }});
                        body = JSON.parse(text).value.body;
                    }}
                    return {{ length: body.length, ms: Date.now() - started }};
                }}"
            ),
        )],
    )
    .await;
    for envelope in [false, true, false, true] {
        let started = Instant::now();
        let answer = host
            .call("dev.test.big/p", "run", json!({ "envelope": envelope }))
            .await
            .unwrap();
        println!(
            "4 MB page {}: {:?} in all, {} ms in the script",
            if envelope {
                "with the old envelope"
            } else {
                "as a string"
            },
            started.elapsed(),
            answer["ms"]
        );
    }
}

/// How long loading takes with many extensions, on one lane and on four. `cargo test -p
/// raygun-ext-host -- --ignored many_extensions_load --nocapture`.
#[tokio::test(flavor = "multi_thread")]
#[ignore = "a measurement"]
async fn many_extensions_load_on_several_lanes_at_once() {
    let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../extensions");
    let mut sources = Vec::new();
    for copy in 0..4 {
        for folder in crate::discover_installed(&dir) {
            let crate::ExtensionFiles::Dir(path) = &folder.files else {
                continue;
            };
            let read = |name: &str| std::fs::read_to_string(path.join(name)).ok();
            let (Some(manifest), Some(code)) =
                (read("raygun-extension.json"), read("dist/index.js"))
            else {
                continue;
            };
            let mut manifest: serde_json::Value = serde_json::from_str(&manifest).unwrap();
            let id = format!("{}{copy}", manifest["id"].as_str().unwrap());
            manifest["id"] = json!(id);
            let entry = manifest["entry"]
                .as_str()
                .unwrap_or("dist/index.js")
                .to_owned();
            sources.push(ExtensionSource::built_in(&[
                ("raygun-extension.json", &manifest.to_string()),
                (&entry, &code),
            ]));
        }
    }
    let count = sources.len();
    for lanes in [1, 4] {
        let host = host(lanes);
        let started = Instant::now();
        let infos = host.load(sources.clone()).await;
        let loaded = infos
            .iter()
            .filter(|info| info.status == ExtensionStatus::Loaded)
            .count();
        println!(
            "{count} extensions ({loaded} loaded) on {lanes} lane(s): {:?}",
            started.elapsed()
        );
    }
}

#[tokio::test(flavor = "multi_thread")]
async fn many_sources_search_without_waiting_on_the_slowest() {
    let address = start_server(Arc::default());
    let host = host(4);
    let mut sources = Vec::new();
    for index in 0..32 {
        let id = format!("dev.test.s{index}");
        let name = format!("S{index}");
        let search = match index % 4 {
            // Fast.
            0 => "search: async ({ query }) => ({ tracks: [{ title: query }] })".to_owned(),
            // Slow, behind a slow site.
            1 => format!(
                "search: async () => (await raygun.http.fetch('http://{address}/slow')).json()"
            ),
            // Failing.
            2 => "search: async () => { throw new Error('this source is broken'); }".to_owned(),
            // Hanging.
            _ => "search: async () => new Promise(() => {})".to_owned(),
        };
        sources.push(extension(&id, &name, &search));
    }
    // One that answers late, counting how often it was asked.
    sources.push(extension(
        "dev.test.late",
        "Late",
        "search: async () => { globalThis.asked = (globalThis.asked ?? 0) + 1;
            await raygun.time.sleep(3500); return { tracks: [{ title: `asked ${globalThis.asked}` }] }; }",
    ));
    let loading = Instant::now();
    load(&host, sources).await;
    let loaded = loading.elapsed();

    let started = Instant::now();
    let (results, pending) = host.search_music_now("song", 10).await;
    let first = started.elapsed();
    println!(
        "33 sources: loaded in {loaded:?}; first answer in {first:?} with {} sources in, {} still searching",
        results.len(),
        pending.len()
    );
    // The fast, slow and failing ones are in; the search didn't wait on the hanging or late ones.
    assert!(first < Duration::from_secs(4), "{first:?}");
    assert_eq!(results.len(), 24);
    assert_eq!(pending.len(), 9);
    assert!(pending.iter().any(|provider| provider.name == "Late"));
    let failed = results
        .iter()
        .filter(|result| result.error.is_some())
        .count();
    assert_eq!(failed, 8);
    assert!(
        results
            .iter()
            .filter_map(|result| result.error.as_deref())
            .all(|error| error.ends_with(": this source is broken"))
    );
    // Good answers come before failures.
    let first_failure = results
        .iter()
        .position(|result| result.error.is_some())
        .unwrap();
    assert!(
        results[..first_failure]
            .iter()
            .all(|result| result.error.is_none())
    );

    // The late source kept going: asking again picks up its answer without asking it twice.
    tokio::time::sleep(Duration::from_millis(1500)).await;
    let again = Instant::now();
    let (results, pending) = host.search_music_now("song", 10).await;
    println!(
        "asked again: {:?}, {} sources in, {} still searching",
        again.elapsed(),
        results.len(),
        pending.len()
    );
    let late = results
        .iter()
        .find(|result| result.provider.name == "Late")
        .expect("the late source answered by now");
    assert_eq!(late.tracks[0].title, "asked 1");
    assert_eq!(pending.len(), 8);
}
