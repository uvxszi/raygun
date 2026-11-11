//! What runtimes may use (`heap`, `Limits`): memory by what runs (converted plugins get more), a
//! pool all of a host's runtimes share, garbage collected after calls, scripts that run too long
//! (regular expressions included) or too deep stopped, and failures that say what ran out.

use std::time::{Duration, Instant};

use raygun_model::ExtensionStatus;
use serde_json::json;

use crate::{
    CallError, Exhaustion, ExtensionHost, ExtensionSource, HeapPool, HostConfig, Limits,
    MANIFEST_FILE,
};

const MIB: usize = 1024 * 1024;

/// An extension `id` whose provider `p` runs `body` as `run(args)`; `nuclear` marks it as a
/// converted Nuclear plugin.
fn extension(id: &str, nuclear: bool, body: &str) -> ExtensionSource {
    let mut manifest = json!({
        "id": id, "name": id, "version": "1.0.0", "apiVersion": 1, "runtime": "js",
        "entry": "index.js", "provides": ["music.metadata"], "permissions": { "net": [] },
    });
    if nuclear {
        manifest["nuclear"] =
            json!({ "package": id, "converter": crate::nuclear::CONVERTER_VERSION, "report": {} });
    }
    let code = format!(
        "raygun.providers.register({{ kind: 'music.metadata', id: 'p', name: 'P', run: async (args) => {{ {body} }} }});"
    );
    ExtensionSource::built_in(&[(MANIFEST_FILE, &manifest.to_string()), ("index.js", &code)])
}

fn host(limits: Limits) -> ExtensionHost {
    ExtensionHost::start(HostConfig {
        limits,
        call_timeout: Duration::from_secs(20),
        ..HostConfig::default()
    })
    .unwrap()
}

async fn load(host: &ExtensionHost, sources: Vec<ExtensionSource>) {
    for info in host.load(sources).await {
        assert_eq!(info.status, ExtensionStatus::Loaded, "{:?}", info.error);
    }
}

fn small_limits() -> Limits {
    Limits {
        memory_bytes: 8 * MIB,
        converted_memory_bytes: 48 * MIB,
        pool: HeapPool::new(1024 * MIB),
        ..Limits::default()
    }
}

/// Builds a string of `args.mb` megabytes and answers its length.
const HOLD: &str = "const big = 'x'.repeat(args.mb * 1024 * 1024); return big.length;";

#[tokio::test]
async fn converted_plugins_get_the_larger_budget_and_running_out_says_so() {
    let host = host(small_limits());
    load(
        &host,
        vec![
            extension("dev.raygun.plain", false, HOLD),
            extension("nuclear.converted", true, HOLD),
        ],
    )
    .await;
    assert_eq!(
        host.heap_use("dev.raygun.plain", false).unwrap().limit,
        8 * MIB
    );
    assert_eq!(
        host.heap_use("nuclear.converted", false).unwrap().limit,
        48 * MIB
    );

    let error = host
        .call("dev.raygun.plain/p", "run", json!({ "mb": 12 }))
        .await
        .unwrap_err();
    let CallError::Exhausted { what, detail, .. } = &error else {
        panic!("{error:?}");
    };
    assert_eq!(*what, Exhaustion::Memory);
    assert!(
        error
            .to_string()
            .starts_with("P needed more memory than a source may use\n"),
        "{error}"
    );
    assert!(detail.contains("out of memory"), "{detail}");
    assert!(
        detail.contains("a source of its kind may hold 8 MB; it held up to"),
        "{detail}"
    );

    // The same work fits a converted plugin's budget, and the plain one still works after.
    assert_eq!(
        host.call("nuclear.converted/p", "run", json!({ "mb": 12 }))
            .await
            .unwrap(),
        json!(12 * MIB)
    );
    assert_eq!(
        host.call("dev.raygun.plain/p", "run", json!({ "mb": 1 }))
            .await
            .unwrap(),
        json!(MIB)
    );
    let used = host.heap_use("nuclear.converted", true).unwrap();
    assert!(used.peak >= 12 * MIB, "{used:?}");
    assert!(
        used.now < 4 * MIB,
        "the string went with the call: {used:?}"
    );
}

#[tokio::test]
async fn runtimes_share_a_pool_and_the_one_asking_past_it_fails() {
    let host = host(Limits {
        memory_bytes: 64 * MIB,
        converted_memory_bytes: 64 * MIB,
        pool: HeapPool::new(24 * MIB),
        ..Limits::default()
    });
    // One keeps 12 MB between calls; then the other can't take 16 MB more.
    let keeper = "globalThis.kept = 'k'.repeat(args.mb * 1024 * 1024); return kept.length;";
    load(
        &host,
        vec![
            extension("dev.raygun.keeper", false, keeper),
            extension("dev.raygun.taker", false, HOLD),
        ],
    )
    .await;
    host.call("dev.raygun.keeper/p", "run", json!({ "mb": 12 }))
        .await
        .unwrap();
    let error = host
        .call("dev.raygun.taker/p", "run", json!({ "mb": 16 }))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            CallError::Exhausted {
                what: Exhaustion::Memory,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(
        error
            .detail()
            .contains("all sources together may hold 24 MB, and the others held the rest"),
        "{error}"
    );
    // Once the keeper lets go, there's room again.
    host.call("dev.raygun.keeper/p", "run", json!({ "mb": 0 }))
        .await
        .unwrap();
    assert_eq!(
        host.call("dev.raygun.taker/p", "run", json!({ "mb": 16 }))
            .await
            .unwrap(),
        json!(16 * MIB)
    );
}

#[tokio::test]
async fn garbage_that_refers_to_itself_goes_after_the_call() {
    let host = host(Limits {
        memory_bytes: 128 * MIB,
        ..small_limits()
    });
    // A big page's worth of objects first, as a scrape builds: QuickJS then waits for the heap
    // to grow by half again before it collects on its own. Its objects go when the page does
    // (reference counting); pairs that point at each other don't, and would stay until then.
    let body = "let page = []; \
                for (let i = 0; i < 300; i += 1) { page.push({ s: 'x'.repeat(100000) + i }); } \
                page = null; \
                for (let i = 0; i < args.pairs; i += 1) { const a = { pad: 'p'.repeat(512) + i }; const b = { a }; a.b = b; } \
                return 'made';";
    load(&host, vec![extension("dev.raygun.cycles", false, body)]).await;
    let before = host.heap_use("dev.raygun.cycles", true).unwrap();
    host.call("dev.raygun.cycles/p", "run", json!({ "pairs": 12_000 }))
        .await
        .unwrap();
    let after = host.heap_use("dev.raygun.cycles", false).unwrap();
    assert!(
        after.peak > before.now + 24 * MIB,
        "the call held a page: {before:?} → {after:?}"
    );
    assert!(
        after.now < before.now + COLLECTED,
        "and what it left was collected: {before:?} → {after:?}"
    );
}

/// What a runtime may still hold after a call that made garbage, once it's collected.
const COLLECTED: usize = 2 * MIB;

/// Spins for `args.ms` milliseconds without waiting.
const SPIN: &str = "const end = Date.now() + args.ms; while (Date.now() < end) {} return 'done';";

#[tokio::test]
async fn converted_code_gets_twice_the_time_without_waiting() {
    let host = host(Limits {
        cpu_budget: Duration::from_millis(500),
        ..small_limits()
    });
    load(
        &host,
        vec![
            extension("dev.raygun.spin", false, SPIN),
            extension("nuclear.spin", true, SPIN),
        ],
    )
    .await;
    let error = host
        .call("dev.raygun.spin/p", "run", json!({ "ms": 750 }))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            CallError::Exhausted {
                what: Exhaustion::Time,
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(
        host.call("nuclear.spin/p", "run", json!({ "ms": 750 }))
            .await
            .unwrap(),
        json!("done")
    );
}

#[tokio::test]
async fn runaway_regular_expressions_and_deep_calls_stop_and_the_lane_goes_on() {
    let host = host(Limits {
        cpu_budget: Duration::from_millis(400),
        ..small_limits()
    });
    let body = "if (args.regex) { return /^(a+)+$/.test('a'.repeat(40) + 'b'); } \
                if (args.deep) { const down = (n) => down(n + 1) + 1; return down(0); } \
                return 'fine';";
    load(&host, vec![extension("dev.raygun.runaway", false, body)]).await;

    let started = Instant::now();
    let error = host
        .call("dev.raygun.runaway/p", "run", json!({ "regex": true }))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            CallError::Exhausted {
                what: Exhaustion::Time,
                ..
            }
        ),
        "{error:?}"
    );
    assert!(
        error
            .to_string()
            .starts_with("P kept the processor busy too long\n"),
        "{error}"
    );
    assert!(
        started.elapsed() < Duration::from_secs(5),
        "{:?}",
        started.elapsed()
    );

    let error = host
        .call("dev.raygun.runaway/p", "run", json!({ "deep": true }))
        .await
        .unwrap_err();
    assert!(
        matches!(
            error,
            CallError::Exhausted {
                what: Exhaustion::Stack,
                ..
            }
        ),
        "{error:?}"
    );
    assert_eq!(
        host.call("dev.raygun.runaway/p", "run", json!({}))
            .await
            .unwrap(),
        json!("fine")
    );
}

#[tokio::test]
async fn background_work_gives_sources_the_longer_limit() {
    use futures_util::StreamExt as _;

    assert_eq!(crate::waiting_limit(), crate::INTERACTIVE_TIMEOUT);
    assert!(!crate::is_background());
    let seen = crate::in_background(async {
        // Futures polled within the work see it too, as a fan-out's calls are.
        let nested: Vec<_> = futures_util::stream::iter(0..3)
            .map(|_| async { crate::waiting_limit() })
            .buffer_unordered(3)
            .collect()
            .await;
        // A task of its own isn't covered.
        let spawned = tokio::spawn(async { crate::is_background() })
            .await
            .unwrap();
        (crate::is_background(), nested, spawned)
    })
    .await;
    assert_eq!(seen, (true, vec![crate::BACKGROUND_TIMEOUT; 3], false));
    assert_eq!(crate::waiting_limit(), crate::INTERACTIVE_TIMEOUT);

    // A call made in the background still answers like any other.
    let host = host(small_limits());
    load(
        &host,
        vec![extension("dev.raygun.slow", false, "return 'late';")],
    )
    .await;
    let answer = crate::in_background(host.call_interactive("dev.raygun.slow/p", "run", json!({})))
        .await
        .unwrap();
    assert_eq!(answer, json!("late"));
}

#[tokio::test]
async fn a_source_still_working_goes_on_and_asking_again_gets_its_answer() {
    let host = host(small_limits());
    // Answers in 700 ms; the asks below wait 200 ms each.
    let slow = "await raygun.time.sleep(700); return args.q + ' found';";
    load(&host, vec![extension("dev.raygun.slow", false, slow)]).await;
    let ask = |q: &'static str| {
        host.call_or_go_on(
            "dev.raygun.slow/p",
            "run",
            json!({ "q": q }),
            Duration::from_millis(200),
            Duration::from_secs(10),
        )
    };
    let error = ask("song").await.unwrap_err();
    assert!(
        error
            .to_string()
            .starts_with("P is still working on it — try again in a moment\n"),
        "{error}"
    );
    assert_eq!(error.kind(), Some(raygun_net::IssueKind::Timeout));
    // Asked again while it goes on: it waits for the same call, as long again.
    assert!(matches!(
        ask("song").await,
        Err(CallError::StillWorking { .. })
    ));
    tokio::time::sleep(Duration::from_millis(500)).await;
    assert_eq!(ask("song").await.unwrap(), json!("song found"));
    // The answer is handed over once: the next ask asks anew.
    assert!(matches!(
        ask("song").await,
        Err(CallError::StillWorking { .. })
    ));
    // A source that's quick enough answers as always, and other asks aren't answered by it.
    let quick = host
        .call_or_go_on(
            "dev.raygun.slow/p",
            "run",
            json!({ "q": "other" }),
            Duration::from_secs(5),
            Duration::from_secs(10),
        )
        .await
        .unwrap();
    assert_eq!(quick, json!("other found"));
}

#[tokio::test]
async fn an_ask_left_before_its_time_stops_rather_than_goes_on() {
    let host = host(small_limits());
    let slow = "await raygun.time.sleep(400); return 'late';";
    load(&host, vec![extension("dev.raygun.left", false, slow)]).await;
    let ask = |wait: u64| {
        host.call_or_go_on(
            "dev.raygun.left/p",
            "run",
            json!({}),
            Duration::from_millis(wait),
            Duration::from_secs(10),
        )
    };
    // Left after 100 ms of its 300: nothing goes on for it.
    assert!(
        tokio::time::timeout(Duration::from_millis(100), ask(300))
            .await
            .is_err()
    );
    tokio::time::sleep(Duration::from_millis(600)).await;
    // So nothing was kept: this ask starts anew and waits its own time.
    let started = Instant::now();
    assert_eq!(ask(2_000).await.unwrap(), json!("late"));
    assert!(started.elapsed() >= Duration::from_millis(300));
}

#[tokio::test]
async fn converted_plugins_spread_over_the_lanes() {
    let host = ExtensionHost::start(HostConfig {
        lanes: 2,
        ..HostConfig::default()
    })
    .unwrap();
    // Placed in this order; by count alone, both converted ones would share lane 0.
    load(
        &host,
        vec![
            extension("dev.order.a", true, "return 1;"),
            extension("dev.order.b", false, "return 1;"),
            extension("dev.order.c", true, "return 1;"),
            extension("dev.order.d", false, "return 1;"),
        ],
    )
    .await;
    let lane = |id: &str| host.lane_of(id).unwrap();
    assert_ne!(lane("dev.order.a"), lane("dev.order.c"), "one per lane");
    assert_ne!(lane("dev.order.b"), lane("dev.order.d"));
}
