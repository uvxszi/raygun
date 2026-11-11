//! Sources that misbehave on purpose: whatever one does (recursing without end while it loads,
//! answering with something nested past any limit or referring to itself, throwing from timers and
//! callbacks, holding more than it may, leaving work behind when it's reloaded), its call or its
//! load fails and the host, its lanes and every other source go on. A process that ended here
//! would end Raygun.

use std::collections::HashMap;
use std::time::Duration;

use raygun_model::ExtensionStatus;
use serde_json::json;

use crate::{ExtensionHost, ExtensionSource, HeapPool, HostConfig, Limits, MANIFEST_FILE};

const MIB: usize = 1024 * 1024;

/// An extension `id` that runs `top` as it loads, and whose provider `p` runs `body` as
/// `run(args)`.
fn extension(id: &str, top: &str, body: &str) -> ExtensionSource {
    let manifest = json!({
        "id": id, "name": id, "version": "1.0.0", "apiVersion": 1, "runtime": "js",
        "entry": "index.js", "provides": ["music.metadata"], "permissions": { "net": [] },
    });
    let code = format!(
        "{top}\nraygun.providers.register({{ kind: 'music.metadata', id: 'p', name: 'P', run: async (args) => {{ {body} }} }});"
    );
    ExtensionSource::built_in(&[(MANIFEST_FILE, &manifest.to_string()), ("index.js", &code)])
}

fn host() -> ExtensionHost {
    ExtensionHost::start(HostConfig {
        limits: Limits {
            memory_bytes: 16 * MIB,
            converted_memory_bytes: 48 * MIB,
            pool: HeapPool::new(512 * MIB),
            cpu_budget: Duration::from_secs(2),
            ..Limits::default()
        },
        call_timeout: Duration::from_secs(20),
        lanes: 2,
        ..HostConfig::default()
    })
    .unwrap()
}

/// A source every test keeps asking, to show the host goes on.
fn steady() -> ExtensionSource {
    extension("dev.raygun.steady", "", "return 'still here';")
}

async fn steady_answers(host: &ExtensionHost) {
    assert_eq!(
        host.call("dev.raygun.steady/p", "run", json!({}))
            .await
            .unwrap(),
        json!("still here")
    );
}

#[tokio::test]
async fn a_source_that_recurses_without_end_as_it_loads_fails_to_load_alone() {
    let host = host();
    let infos = host
        .load(vec![
            extension(
                "dev.raygun.forever",
                "const down = (n) => down(n + 1) + 1; down(0);",
                "return 1;",
            ),
            steady(),
        ])
        .await;
    let forever = infos
        .iter()
        .find(|info| info.id == "dev.raygun.forever")
        .unwrap();
    assert_eq!(forever.status, ExtensionStatus::Failed);
    steady_answers(&host).await;
}

#[tokio::test]
async fn answers_nested_past_any_limit_or_referring_to_themselves_fail_the_call() {
    let host = host();
    let body = "if (args.cycle) { const o = {}; o.self = o; return o; } \
                let o = {}; for (let i = 0; i < 200000; i++) { o = { a: o }; } return o;";
    let infos = host
        .load(vec![extension("dev.raygun.nested", "", body), steady()])
        .await;
    assert!(
        infos
            .iter()
            .all(|info| info.status == ExtensionStatus::Loaded)
    );
    assert!(
        host.call("dev.raygun.nested/p", "run", json!({}))
            .await
            .is_err()
    );
    assert!(
        host.call("dev.raygun.nested/p", "run", json!({ "cycle": true }))
            .await
            .is_err()
    );
    steady_answers(&host).await;
}

#[tokio::test]
async fn throwing_from_callbacks_and_unhandled_rejections_stay_in_the_source() {
    let host = host();
    let body = "Promise.reject(new Error('nobody catches this')); \
                Promise.resolve().then(() => { throw new Error('thrown in a callback'); }); \
                if (args.throw) { throw new TypeError('thrown by the call'); } \
                return 'answered';";
    host.load(vec![extension("dev.raygun.thrower", "", body), steady()])
        .await;
    assert!(
        host.call("dev.raygun.thrower/p", "run", json!({ "throw": true }))
            .await
            .is_err()
    );
    assert_eq!(
        host.call("dev.raygun.thrower/p", "run", json!({}))
            .await
            .unwrap(),
        json!("answered")
    );
    steady_answers(&host).await;
}

#[tokio::test]
async fn a_source_asking_for_more_memory_than_it_may_fails_alone() {
    let host = host();
    let body =
        "const parts = []; for (;;) { parts.push('x'.repeat(4 * 1024 * 1024) + parts.length); }";
    host.load(vec![extension("dev.raygun.greedy", "", body), steady()])
        .await;
    assert!(
        host.call("dev.raygun.greedy/p", "run", json!({}))
            .await
            .is_err()
    );
    steady_answers(&host).await;
}

#[tokio::test]
async fn reloading_while_a_source_still_works_lets_its_runtime_go_safely() {
    let host = host();
    // Work that never ends, holding objects of the runtime: a call that never answers, and a
    // promise chain left behind.
    let top =
        "globalThis.kept = []; for (let i = 0; i < 1000; i++) { kept.push({ i, next: kept }); }";
    let body = "globalThis.pending = new Promise(() => {}); return await globalThis.pending;";
    let sources = vec![extension("dev.raygun.lingering", top, body), steady()];
    host.load(sources.clone()).await;
    let waiting = {
        let host = host.clone();
        tokio::spawn(async move { host.call("dev.raygun.lingering/p", "run", json!({})).await })
    };
    tokio::time::sleep(Duration::from_millis(200)).await;
    // Every reload drops the lane's runtimes; QuickJS frees them while the call still waits.
    for _ in 0..3 {
        host.load(sources.clone()).await;
    }
    waiting.abort();
    let _ = waiting.await;
    host.load(sources).await;
    steady_answers(&host).await;
}

#[tokio::test]
async fn held_back_sources_are_not_loaded_and_say_why() {
    let host = host();
    host.set_held_back(HashMap::from([(
        "dev.raygun.suspect".to_owned(),
        "Held back after Raygun stopped while it loaded".to_owned(),
    )]));
    let infos = host
        .load(vec![
            extension("dev.raygun.suspect", "", "return 1;"),
            steady(),
        ])
        .await;
    let suspect = infos
        .iter()
        .find(|info| info.id == "dev.raygun.suspect")
        .unwrap();
    assert_eq!(suspect.status, ExtensionStatus::Failed);
    assert!(
        suspect
            .error
            .as_deref()
            .is_some_and(|error| error.ends_with("Held back after Raygun stopped while it loaded")),
        "{:?}",
        suspect.error
    );
    assert!(
        host.call("dev.raygun.suspect/p", "run", json!({}))
            .await
            .is_err()
    );
    steady_answers(&host).await;
    // Let go again, it loads.
    host.set_held_back(HashMap::new());
    host.load(vec![
        extension("dev.raygun.suspect", "", "return 1;"),
        steady(),
    ])
    .await;
    assert_eq!(
        host.call("dev.raygun.suspect/p", "run", json!({}))
            .await
            .unwrap(),
        json!(1)
    );
}

#[test]
fn a_chapter_nested_thousands_deep_is_read_on_a_small_stack() {
    let deep = format!("{}text{}", "<div>".repeat(10_000), "</div>".repeat(10_000));
    let passage = json!({ "html": deep });
    let blocks = std::thread::Builder::new()
        .stack_size(2 * MIB)
        .spawn(move || crate::novel::parse_passage(&passage).len())
        .unwrap()
        .join()
        .expect("reading the chapter overflowed its stack");
    assert!(blocks > 0);
}
