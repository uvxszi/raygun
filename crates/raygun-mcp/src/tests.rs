use axum::body::{Body, to_bytes};
use axum::http::Request;
use tower::ServiceExt;

use super::*;

const TOKEN: &str = "secret-token";

struct Fake;

impl Backend for Fake {
    fn tools(&self) -> Vec<ToolDef> {
        vec![ToolDef {
            name: "echo",
            description: "Echoes its text, or fails on 'boom'",
            input_schema: json!({ "type": "object", "properties": { "text": { "type": "string" } } }),
        }]
    }

    async fn call_tool(&self, _name: &str, args: Value) -> Result<String, String> {
        match args.get("text").and_then(Value::as_str) {
            Some("boom") => Err("it broke".into()),
            Some(text) => Ok(text.to_owned()),
            None => Err("text is required".into()),
        }
    }

    fn resources(&self) -> Vec<ResourceDef> {
        vec![ResourceDef {
            uri: "raygun://now-playing",
            name: "Now playing",
            description: "The current track",
        }]
    }

    async fn read_resource(&self, uri: &str) -> Option<Result<String, String>> {
        (uri == "raygun://now-playing").then(|| Ok(r#"{"status":"idle"}"#.into()))
    }
}

fn app() -> Router {
    router(Fake, Token::new(TOKEN.into()), "0.1.0")
}

fn request() -> axum::http::request::Builder {
    Request::post("/mcp")
        .header("host", "127.0.0.1:8810")
        .header("authorization", format!("Bearer {TOKEN}"))
        .header("content-type", "application/json")
}

async fn send(
    app: &Router,
    builder: axum::http::request::Builder,
    body: Value,
) -> (StatusCode, HeaderMap, Value) {
    let response = app
        .clone()
        .oneshot(builder.body(Body::from(body.to_string())).unwrap())
        .await
        .unwrap();
    let status = response.status();
    let headers = response.headers().clone();
    let bytes = to_bytes(response.into_body(), 1 << 20).await.unwrap();
    let value = serde_json::from_slice(&bytes).unwrap_or(Value::Null);
    (status, headers, value)
}

async fn call(app: &Router, method: &str, params: Value) -> Value {
    let body = json!({ "jsonrpc": "2.0", "id": 7, "method": method, "params": params });
    send(app, request(), body).await.2
}

#[tokio::test]
async fn refuses_requests_without_the_token_or_from_other_hosts() {
    let app = app();
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    let cases = [
        (
            Request::post("/mcp").header("host", "127.0.0.1:8810"),
            StatusCode::UNAUTHORIZED,
        ),
        (
            Request::post("/mcp")
                .header("host", "localhost:8810")
                .header("authorization", "Bearer wrong"),
            StatusCode::UNAUTHORIZED,
        ),
        (
            Request::post("/mcp")
                .header("host", "evil.example:8810")
                .header("authorization", format!("Bearer {TOKEN}")),
            StatusCode::FORBIDDEN,
        ),
        (
            request().header("origin", "https://evil.example"),
            StatusCode::FORBIDDEN,
        ),
    ];
    for (builder, expected) in cases {
        assert_eq!(send(&app, builder, body.clone()).await.0, expected);
    }
    let (status, _, answer) = send(
        &app,
        request().header("origin", "http://localhost:5173"),
        body,
    )
    .await;
    assert_eq!(status, StatusCode::OK, "a loopback origin is fine");
    assert_eq!(answer["result"], json!({}));
}

#[tokio::test]
async fn initialize_opens_a_session_that_later_requests_must_match() {
    let app = app();
    let body = json!({
        "jsonrpc": "2.0", "id": 1, "method": "initialize",
        "params": { "protocolVersion": PROTOCOL_VERSION, "capabilities": {},
                    "clientInfo": { "name": "test", "version": "1" } }
    });
    let (status, headers, answer) = send(&app, request(), body).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(answer["result"]["protocolVersion"], PROTOCOL_VERSION);
    assert_eq!(answer["result"]["serverInfo"]["name"], "raygun");
    let session = headers[SESSION_HEADER].to_str().unwrap().to_owned();

    let ping = json!({ "jsonrpc": "2.0", "id": 2, "method": "ping" });
    let (status, _, _) = send(
        &app,
        request().header(SESSION_HEADER, &session),
        ping.clone(),
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    let (status, _, _) = send(&app, request().header(SESSION_HEADER, "nope"), ping.clone()).await;
    assert_eq!(status, StatusCode::NOT_FOUND);

    let ended = app
        .clone()
        .oneshot(
            Request::delete("/mcp")
                .header("host", "localhost")
                .header("authorization", format!("Bearer {TOKEN}"))
                .header(SESSION_HEADER, &session)
                .body(Body::empty())
                .unwrap(),
        )
        .await
        .unwrap();
    assert_eq!(ended.status(), StatusCode::OK);
    let (status, _, _) = send(&app, request().header(SESSION_HEADER, &session), ping).await;
    assert_eq!(status, StatusCode::NOT_FOUND, "an ended session is gone");
}

#[tokio::test]
async fn lists_and_calls_tools() {
    let app = app();
    let listed = call(&app, "tools/list", json!({})).await;
    assert_eq!(listed["result"]["tools"][0]["name"], "echo");
    assert!(listed["result"]["tools"][0]["inputSchema"].is_object());

    let ok = call(
        &app,
        "tools/call",
        json!({ "name": "echo", "arguments": { "text": "hi" } }),
    )
    .await;
    assert_eq!(ok["result"]["content"][0]["text"], "hi");
    assert_eq!(ok["result"]["isError"], false);

    let failed = call(
        &app,
        "tools/call",
        json!({ "name": "echo", "arguments": { "text": "boom" } }),
    )
    .await;
    assert_eq!(failed["result"]["content"][0]["text"], "it broke");
    assert_eq!(
        failed["result"]["isError"], true,
        "tool failures are results, not errors"
    );

    let unknown = call(&app, "tools/call", json!({ "name": "nope" })).await;
    assert_eq!(unknown["error"]["code"], -32602);
}

#[tokio::test]
async fn lists_and_reads_resources() {
    let app = app();
    let listed = call(&app, "resources/list", json!({})).await;
    assert_eq!(
        listed["result"]["resources"][0]["uri"],
        "raygun://now-playing"
    );
    let read = call(
        &app,
        "resources/read",
        json!({ "uri": "raygun://now-playing" }),
    )
    .await;
    assert_eq!(
        read["result"]["contents"][0]["text"],
        r#"{"status":"idle"}"#
    );
    let missing = call(&app, "resources/read", json!({ "uri": "raygun://nothing" })).await;
    assert_eq!(missing["error"]["code"], -32002);
}

#[tokio::test]
async fn notifications_bad_json_and_unknown_methods() {
    let app = app();
    let note = json!({ "jsonrpc": "2.0", "method": "notifications/initialized" });
    let response = app
        .clone()
        .oneshot(request().body(Body::from(note.to_string())).unwrap())
        .await
        .unwrap();
    assert_eq!(response.status(), StatusCode::ACCEPTED);

    let garbled = app
        .clone()
        .oneshot(request().body(Body::from("{not json")).unwrap())
        .await
        .unwrap();
    let bytes = to_bytes(garbled.into_body(), 1 << 16).await.unwrap();
    let answer: Value = serde_json::from_slice(&bytes).unwrap();
    assert_eq!(answer["error"]["code"], -32700);

    let batch = json!([{ "jsonrpc": "2.0", "id": 1, "method": "ping" }]);
    assert_eq!(
        send(&app, request(), batch).await.2["error"]["code"],
        -32600
    );

    assert_eq!(
        call(&app, "sampling/createMessage", json!({})).await["error"]["code"],
        -32601
    );
    assert_eq!(
        call(&app, "prompts/list", json!({})).await["result"]["prompts"],
        json!([])
    );
}

#[tokio::test]
async fn a_new_token_replaces_the_old_one_at_once() {
    let token = Token::new(TOKEN.into());
    let app = router(Fake, token.clone(), "0.1.0");
    token.set("fresh".into());
    let body = json!({ "jsonrpc": "2.0", "id": 1, "method": "ping" });
    assert_eq!(
        send(&app, request(), body.clone()).await.0,
        StatusCode::UNAUTHORIZED
    );
    let fresh = Request::post("/mcp")
        .header("host", "localhost")
        .header("authorization", "Bearer fresh");
    assert_eq!(send(&app, fresh, body).await.0, StatusCode::OK);
}

#[test]
fn tokens_are_long_and_random_and_hosts_are_checked() {
    let token = generate_token();
    assert_eq!(token.len(), 64);
    assert_ne!(token, generate_token());
    for host in [
        "localhost",
        "localhost:8810",
        "127.0.0.1:8811",
        "[::1]:8810",
        "LOCALHOST",
    ] {
        assert!(is_loopback_authority(host), "{host}");
    }
    for host in [
        "evil.example",
        "localhost.evil.example",
        "192.168.1.5:8810",
        "",
        "[::2]",
    ] {
        assert!(!is_loopback_authority(host), "{host}");
    }
}

#[tokio::test]
async fn serves_on_a_loopback_port() {
    let running = serve(app()).await.unwrap();
    assert!(running.address.ip().is_loopback());
    assert!(PORTS.contains(&running.address.port()));
    let second = serve(app()).await.unwrap();
    assert_ne!(
        second.address.port(),
        running.address.port(),
        "takes the next free port"
    );
    running.stop();
}
