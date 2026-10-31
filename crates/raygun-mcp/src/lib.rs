//! Raygun's MCP server (ARCHITECTURE §9): the Model Context Protocol over streamable HTTP, on
//! loopback only, behind a bearer token.
//!
//! Each `POST /mcp` carries one JSON-RPC message and gets a plain JSON answer, which the
//! streamable-HTTP transport allows in place of an SSE stream. What the tools do lives behind
//! [`Backend`], implemented by the app; this crate is transport, auth and dispatch.

use std::collections::HashSet;
use std::future::Future;
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::post;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

/// The protocol revision this server speaks.
pub const PROTOCOL_VERSION: &str = "2025-06-18";
/// Ports tried in order; the first free one is used.
pub const PORTS: std::ops::RangeInclusive<u16> = 8810..=8819;
const SESSION_HEADER: &str = "mcp-session-id";

/// A tool the agent can call.
#[derive(Debug, Clone)]
pub struct ToolDef {
    pub name: &'static str,
    pub description: &'static str,
    /// JSON Schema of the arguments object.
    pub input_schema: Value,
}

/// A resource the agent can read.
#[derive(Debug, Clone)]
pub struct ResourceDef {
    pub uri: &'static str,
    pub name: &'static str,
    pub description: &'static str,
}

/// What the tools and resources do. Results and errors are short texts (usually JSON) for the
/// agent; an `Err` from a tool becomes an `isError` result, not a protocol error.
pub trait Backend: Send + Sync + 'static {
    fn tools(&self) -> Vec<ToolDef>;
    fn call_tool(
        &self,
        name: &str,
        args: Value,
    ) -> impl Future<Output = Result<String, String>> + Send;
    fn resources(&self) -> Vec<ResourceDef>;
    /// `None` when no resource has this URI.
    fn read_resource(
        &self,
        uri: &str,
    ) -> impl Future<Output = Option<Result<String, String>>> + Send;
}

/// The token clients must send, shared with the app so a new token takes effect at once.
#[derive(Clone)]
pub struct Token(Arc<RwLock<String>>);

impl Token {
    pub fn new(value: String) -> Self {
        Self(Arc::new(RwLock::new(value)))
    }

    pub fn set(&self, value: String) {
        *self.0.write().expect("token lock poisoned") = value;
    }

    fn matches(&self, presented: &str) -> bool {
        let expected = self.0.read().expect("token lock poisoned");
        constant_time_eq(expected.as_bytes(), presented.as_bytes())
    }
}

/// A fresh random token: 64 hex characters from two random UUIDs (244 random bits).
pub fn generate_token() -> String {
    format!(
        "{}{}",
        uuid::Uuid::new_v4().simple(),
        uuid::Uuid::new_v4().simple()
    )
}

struct Shared<B> {
    backend: B,
    token: Token,
    sessions: Mutex<HashSet<String>>,
    server_name: &'static str,
    server_version: &'static str,
}

/// The `/mcp` route for `backend`.
pub fn router<B: Backend>(backend: B, token: Token, server_version: &'static str) -> Router {
    let shared = Arc::new(Shared {
        backend,
        token,
        sessions: Mutex::new(HashSet::new()),
        server_name: "raygun",
        server_version,
    });
    Router::new()
        .route(
            "/mcp",
            post(handle_post::<B>)
                .get(|| async { StatusCode::METHOD_NOT_ALLOWED })
                .delete(handle_delete::<B>),
        )
        .with_state(shared)
}

/// A running server; dropping it (or calling [`Running::stop`]) shuts it down.
pub struct Running {
    pub address: SocketAddr,
    stop: Option<oneshot::Sender<()>>,
}

impl Running {
    pub fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

/// Binds the first free port in [`PORTS`] on 127.0.0.1 and serves `router` until stopped.
pub async fn serve(router: Router) -> std::io::Result<Running> {
    let mut last_error = None;
    for port in PORTS {
        match TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
            Ok(listener) => {
                let address = listener.local_addr()?;
                let (stop, stopped) = oneshot::channel::<()>();
                tokio::spawn(async move {
                    let shutdown = async {
                        let _ = stopped.await;
                    };
                    if let Err(error) = axum::serve(listener, router)
                        .with_graceful_shutdown(shutdown)
                        .await
                    {
                        tracing::warn!(%error, "MCP server stopped with an error");
                    }
                });
                return Ok(Running {
                    address,
                    stop: Some(stop),
                });
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("no port to try")))
}

async fn handle_delete<B: Backend>(
    State(shared): State<Arc<Shared<B>>>,
    headers: HeaderMap,
) -> Response {
    if let Err(refusal) = check_access(&shared.token, &headers) {
        return refusal;
    }
    if let Some(session) = header_text(&headers, SESSION_HEADER) {
        shared
            .sessions
            .lock()
            .expect("sessions lock poisoned")
            .remove(session);
    }
    StatusCode::OK.into_response()
}

async fn handle_post<B: Backend>(
    State(shared): State<Arc<Shared<B>>>,
    headers: HeaderMap,
    body: Bytes,
) -> Response {
    if let Err(refusal) = check_access(&shared.token, &headers) {
        return refusal;
    }
    let message: Value = match serde_json::from_slice(&body) {
        Ok(message) => message,
        Err(error) => {
            return json_response(rpc_error(
                Value::Null,
                -32700,
                &format!("Parse error: {error}"),
            ));
        }
    };
    if !message.is_object() {
        // Batches were dropped from the protocol in 2025-06-18.
        return json_response(rpc_error(
            Value::Null,
            -32600,
            "Expected one JSON-RPC object",
        ));
    }
    let method = message
        .get("method")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let id = message.get("id").cloned();

    let Some(id) = id else {
        // Notifications and responses from the client need no answer.
        return StatusCode::ACCEPTED.into_response();
    };

    if method == "initialize" {
        let session = uuid::Uuid::new_v4().simple().to_string();
        shared
            .sessions
            .lock()
            .expect("sessions lock poisoned")
            .insert(session.clone());
        let mut response = json_response(rpc_result(
            id,
            json!({
                "protocolVersion": PROTOCOL_VERSION,
                "capabilities": { "tools": {}, "resources": {} },
                "serverInfo": { "name": shared.server_name, "version": shared.server_version },
                "instructions": "Controls the Raygun media player on this device. \
                    Start with now_playing or search_music.",
            }),
        ));
        if let Ok(value) = HeaderValue::from_str(&session) {
            response.headers_mut().insert(SESSION_HEADER, value);
        }
        return response;
    }

    if let Some(session) = header_text(&headers, SESSION_HEADER)
        && !shared
            .sessions
            .lock()
            .expect("sessions lock poisoned")
            .contains(session)
    {
        return (StatusCode::NOT_FOUND, "Unknown or ended session").into_response();
    }

    let params = message.get("params").cloned().unwrap_or(Value::Null);
    json_response(dispatch(&shared.backend, id, method, params).await)
}

async fn dispatch<B: Backend>(backend: &B, id: Value, method: &str, params: Value) -> Value {
    match method {
        "ping" => rpc_result(id, json!({})),
        "tools/list" => {
            let tools: Vec<Value> = backend
                .tools()
                .into_iter()
                .map(|tool| {
                    json!({
                        "name": tool.name,
                        "description": tool.description,
                        "inputSchema": tool.input_schema,
                    })
                })
                .collect();
            rpc_result(id, json!({ "tools": tools }))
        }
        "tools/call" => {
            let Some(name) = params.get("name").and_then(Value::as_str) else {
                return rpc_error(id, -32602, "tools/call needs a tool name");
            };
            if !backend.tools().iter().any(|tool| tool.name == name) {
                return rpc_error(id, -32602, &format!("Unknown tool: {name}"));
            }
            let args = params
                .get("arguments")
                .cloned()
                .unwrap_or_else(|| json!({}));
            let (text, is_error) = match backend.call_tool(name, args).await {
                Ok(text) => (text, false),
                Err(message) => (message, true),
            };
            rpc_result(
                id,
                json!({ "content": [{ "type": "text", "text": text }], "isError": is_error }),
            )
        }
        "resources/list" => {
            let resources: Vec<Value> = backend
                .resources()
                .into_iter()
                .map(|resource| {
                    json!({
                        "uri": resource.uri,
                        "name": resource.name,
                        "description": resource.description,
                        "mimeType": "application/json",
                    })
                })
                .collect();
            rpc_result(id, json!({ "resources": resources }))
        }
        "resources/read" => {
            let Some(uri) = params.get("uri").and_then(Value::as_str) else {
                return rpc_error(id, -32602, "resources/read needs a uri");
            };
            match backend.read_resource(uri).await {
                Some(Ok(text)) => rpc_result(
                    id,
                    json!({ "contents": [{ "uri": uri, "mimeType": "application/json", "text": text }] }),
                ),
                Some(Err(message)) => rpc_error(id, -32603, &message),
                None => rpc_error(id, -32002, &format!("Resource not found: {uri}")),
            }
        }
        // Optional capabilities Raygun doesn't offer: answer empty rather than fail clients.
        "prompts/list" => rpc_result(id, json!({ "prompts": [] })),
        "resources/templates/list" => rpc_result(id, json!({ "resourceTemplates": [] })),
        _ => rpc_error(id, -32601, &format!("Method not found: {method}")),
    }
}

/// Refuses requests without the token, or from a page a rebinding attack could control.
#[allow(clippy::result_large_err)]
fn check_access(token: &Token, headers: &HeaderMap) -> Result<(), Response> {
    let host = header_text(headers, header::HOST.as_str()).unwrap_or_default();
    if !is_loopback_authority(host) {
        return Err((StatusCode::FORBIDDEN, "Host must be localhost").into_response());
    }
    if let Some(origin) = header_text(headers, header::ORIGIN.as_str()) {
        let authority = origin
            .strip_prefix("http://")
            .or_else(|| origin.strip_prefix("https://"));
        if !authority.is_some_and(is_loopback_authority) {
            return Err((StatusCode::FORBIDDEN, "Origin not allowed").into_response());
        }
    }
    let presented = header_text(headers, header::AUTHORIZATION.as_str())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default();
    if presented.is_empty() || !token.matches(presented) {
        let mut response =
            (StatusCode::UNAUTHORIZED, "A valid bearer token is required").into_response();
        response
            .headers_mut()
            .insert(header::WWW_AUTHENTICATE, HeaderValue::from_static("Bearer"));
        return Err(response);
    }
    Ok(())
}

/// `localhost`, `127.x.x.x` or `[::1]`, with or without a port.
fn is_loopback_authority(authority: &str) -> bool {
    let host = if let Some(rest) = authority.strip_prefix('[') {
        rest.split(']').next().unwrap_or_default()
    } else {
        authority.split(':').next().unwrap_or_default()
    };
    let host = host.to_ascii_lowercase();
    host == "localhost" || host == "::1" || host.starts_with("127.")
}

fn header_text<'a>(headers: &'a HeaderMap, name: &str) -> Option<&'a str> {
    headers.get(name).and_then(|value| value.to_str().ok())
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    a.len() == b.len() && a.iter().zip(b).fold(0u8, |acc, (x, y)| acc | (x ^ y)) == 0
}

fn rpc_result(id: Value, result: Value) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "result": result })
}

fn rpc_error(id: Value, code: i64, message: &str) -> Value {
    json!({ "jsonrpc": "2.0", "id": id, "error": { "code": code, "message": message } })
}

fn json_response(body: Value) -> Response {
    (
        [(
            header::CONTENT_TYPE,
            HeaderValue::from_static("application/json"),
        )],
        body.to_string(),
    )
        .into_response()
}

#[cfg(test)]
mod tests;
