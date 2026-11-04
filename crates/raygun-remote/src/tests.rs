use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};

use axum::body::{Body, to_bytes};
use axum::http::Request;
use futures_util::StreamExt;
use tower::ServiceExt;

use super::*;

/// Counts controls; Connect's snapshot shows the count so changes are visible.
#[derive(Default)]
struct Fake {
    controls: AtomicU32,
    /// What this device's user answers when another device asks to play here.
    allow: AtomicBool,
    /// Who asked.
    asked: Mutex<Vec<String>>,
    /// Connect commands received.
    commands: Mutex<Vec<Value>>,
    /// Link cards received (M46.47), with the id of the token each device got.
    links: Mutex<Vec<(Value, String)>>,
}

impl Backend for Arc<Fake> {
    fn device(&self) -> Value {
        json!({ "id": "pc", "name": "Living room PC", "kind": "computer" })
    }

    fn connect_snapshot(&self) -> (Value, Value) {
        (
            json!({ "status": "playing", "controls": self.controls.load(Ordering::SeqCst) }),
            json!({ "items": [], "order": [] }),
        )
    }

    fn connect_command(&self, command: &Value) -> Result<(), String> {
        if command.get("type").is_none() {
            return Err("Unknown command".into());
        }
        self.commands.lock().unwrap().push(command.clone());
        Ok(())
    }

    async fn ask(&self, name: &str) -> bool {
        self.asked.lock().unwrap().push(name.to_owned());
        self.allow.load(Ordering::SeqCst)
    }

    async fn link(&self, card: Option<&Value>, granted: &str) -> Option<Value> {
        let card = card?.clone();
        self.links.lock().unwrap().push((card, granted.to_owned()));
        Some(json!({ "syncId": "sync-pc", "name": "Living room PC" }))
    }
}

struct Setup {
    app: Router,
    fake: Arc<Fake>,
    pairing: Arc<Mutex<Pairing>>,
    saved: Arc<Mutex<Vec<PairedDevice>>>,
}

fn setup() -> Setup {
    let fake = Arc::new(Fake::default());
    let pairing = Arc::new(Mutex::new(Pairing::default()));
    let saved = Arc::new(Mutex::new(Vec::new()));
    let sink = saved.clone();
    let access = Access::new(
        pairing.clone(),
        move |devices| *sink.lock().unwrap() = devices,
        || 1_000,
    );
    Setup {
        app: router(fake.clone(), access),
        fake,
        pairing,
        saved,
    }
}

async fn send(app: &Router, request: Request<Body>) -> (StatusCode, Value) {
    let response = app.clone().oneshot(request).await.unwrap();
    let status = response.status();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    (
        status,
        serde_json::from_slice(&bytes).unwrap_or(Value::Null),
    )
}

fn post_json(path: &str, token: Option<&str>, body: Value) -> Request<Body> {
    let mut builder = Request::post(path).header("content-type", "application/json");
    if let Some(token) = token {
        builder = builder.header("authorization", format!("Bearer {token}"));
    }
    builder.body(Body::from(body.to_string())).unwrap()
}

/// Admits a device and returns its token.
fn paired(setup: &Setup) -> String {
    setup.pairing.lock().unwrap().admit("Laptop", 0).token
}

#[tokio::test]
async fn serves_on_a_port_in_range() {
    let running = serve_on(setup().app, Ipv4Addr::LOCALHOST).await.unwrap();
    assert!(PORTS.contains(&running.port));
    running.stop();
}

#[test]
fn lan_addresses_exclude_loopback_and_unspecified() {
    assert!(usable_lan_ip("192.168.1.20".parse().unwrap()));
    assert!(usable_lan_ip("10.0.2.15".parse().unwrap()));
    assert!(!usable_lan_ip("127.0.0.1".parse().unwrap()));
    assert!(!usable_lan_ip("0.0.0.0".parse().unwrap()));
    assert!(!usable_lan_ip("::1".parse().unwrap()));
}

#[tokio::test]
async fn linking_swaps_cards_and_names_the_token_given() {
    let setup = setup();
    setup.fake.allow.store(true, Ordering::SeqCst);
    let card = json!({ "syncId": "sync-phone", "name": "Pixel 4" });
    let (status, answer) = send(
        &setup.app,
        post_json(
            "/api/connect/hello",
            None,
            json!({ "name": "Pixel 4", "link": card }),
        ),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(
        answer["link"]["syncId"], "sync-pc",
        "this device's card comes back"
    );
    let links = setup.fake.links.lock().unwrap().clone();
    assert_eq!(links.len(), 1);
    assert_eq!(links[0].0, card);
    assert_eq!(
        links[0].1, answer["deviceId"],
        "the token given is named, to take it back"
    );

    // An older Raygun sends no card: it plays here, nothing is linked.
    let (status, answer) = send(
        &setup.app,
        post_json("/api/connect/hello", None, json!({ "name": "Old phone" })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert!(answer["link"].is_null());
    assert_eq!(setup.fake.links.lock().unwrap().len(), 1);
}

#[tokio::test]
async fn a_scanned_code_must_be_the_one_shown_and_works_once() {
    let setup = setup();
    setup.fake.allow.store(true, Ordering::SeqCst);
    let hello = |code: &str| {
        post_json(
            "/api/connect/hello",
            None,
            json!({ "name": "Pixel 4", "code": code }),
        )
    };
    let (status, answer) = send(&setup.app, hello("abcd234567")).await;
    assert_eq!(status, StatusCode::FORBIDDEN, "no code is shown");
    assert!(answer["error"].as_str().unwrap().contains("expired"));
    assert!(
        setup.fake.asked.lock().unwrap().is_empty(),
        "nobody is asked"
    );

    let code = setup.pairing.lock().unwrap().new_code(0);
    let (status, answer) = send(&setup.app, hello("wrong")).await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert!(answer["error"].as_str().unwrap().contains("Scan it again"));

    let (status, answer) = send(&setup.app, hello(&code)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(answer["token"].is_string());
    assert_eq!(
        *setup.fake.asked.lock().unwrap(),
        ["Pixel 4"],
        "the usual question is still asked"
    );
    assert_eq!(
        send(&setup.app, hello(&code)).await.0,
        StatusCode::FORBIDDEN,
        "a code works once"
    );
}

#[tokio::test]
async fn another_raygun_plays_here_once_this_device_allows_it() {
    let setup = setup();
    let (status, device) = send(
        &setup.app,
        Request::get("/api/device").body(Body::empty()).unwrap(),
    )
    .await;
    assert_eq!(status, StatusCode::OK, "naming the device needs no token");
    assert_eq!(device["name"], "Living room PC");

    let (status, answer) = send(
        &setup.app,
        post_json("/api/connect/hello", None, json!({ "name": " Pixel 4 " })),
    )
    .await;
    assert_eq!(status, StatusCode::FORBIDDEN);
    assert_eq!(answer["error"], "That device didn't allow it.");
    assert!(setup.saved.lock().unwrap().is_empty());

    setup.fake.allow.store(true, Ordering::SeqCst);
    let (status, answer) = send(
        &setup.app,
        post_json("/api/connect/hello", None, json!({ "name": " Pixel 4 " })),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["device"]["id"], "pc");
    let token = answer["token"].as_str().unwrap().to_owned();
    assert_eq!(*setup.fake.asked.lock().unwrap(), ["Pixel 4", "Pixel 4"]);
    let saved = setup.saved.lock().unwrap().clone();
    assert_eq!(saved.len(), 1, "an allowed device gets a token");
    assert_eq!(saved[0].name, "Pixel 4");

    assert_eq!(
        send(
            &setup.app,
            post_json("/api/connect/command", None, json!({ "type": "control" }))
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let command = json!({ "type": "control", "control": { "type": "pause" } });
    let (status, _) = send(
        &setup.app,
        post_json("/api/connect/command", Some(&token), command.clone()),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(*setup.fake.commands.lock().unwrap(), [command]);
    let (status, answer) = send(
        &setup.app,
        post_json("/api/connect/command", Some(&token), json!({})),
    )
    .await;
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(answer["error"], "Unknown command");
}

#[tokio::test]
async fn connect_events_send_the_queue_and_the_state_as_they_change() {
    let setup = setup();
    let token = paired(&setup);
    assert_eq!(
        send(
            &setup.app,
            Request::get("/api/connect/events?token=made-up")
                .body(Body::empty())
                .unwrap()
        )
        .await
        .0,
        StatusCode::UNAUTHORIZED
    );
    let response = setup
        .app
        .clone()
        .oneshot(
            Request::get(format!("/api/connect/events?token={token}"))
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::OK);
    let mut body = response.into_body().into_data_stream();
    let mut next = async || {
        let chunk = tokio::time::timeout(Duration::from_secs(5), body.next())
            .await
            .expect("an event within a few seconds")
            .unwrap()
            .unwrap();
        String::from_utf8(chunk.to_vec()).unwrap()
    };

    let first = next().await;
    assert!(first.contains("event: queue"), "{first}");
    let second = next().await;
    assert!(second.contains("event: state"), "{second}");
    assert!(second.contains(r#""controls":0"#), "{second}");

    setup.fake.controls.fetch_add(1, Ordering::SeqCst);
    let changed = next().await;
    assert!(
        changed.contains("event: state"),
        "only the state changed: {changed}"
    );
    assert!(changed.contains(r#""controls":1"#), "{changed}");
}
