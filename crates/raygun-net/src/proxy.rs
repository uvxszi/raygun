//! Loopback stream proxy (ARCHITECTURE §6.2, adapted from the idea in Nuclear's
//! `stream_server.rs`, AGPL-3.0).
//!
//! The webview's `<audio>`/`<video>` element can't send custom headers and is subject to CORS, but
//! many hosts require a Referer, User-Agent or Cookie. The proxy serves each registered source at
//! `http://127.0.0.1:<port>/stream/<token>`: remote sources are fetched with their headers plus the
//! client's `Range`, local files are served with Range support. Only registered tokens resolve, so
//! the proxy can't be used to fetch arbitrary URLs.

use std::collections::{HashMap, VecDeque};
use std::net::{Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use raygun_model::MediaSource;
use tower::ServiceExt;
use tower_http::services::ServeFile;

/// Older tokens are forgotten beyond this many registrations.
const MAX_REGISTERED: usize = 64;

const FORWARDED_RESPONSE_HEADERS: [header::HeaderName; 5] = [
    header::CONTENT_TYPE,
    header::CONTENT_LENGTH,
    header::CONTENT_RANGE,
    header::ACCEPT_RANGES,
    header::LAST_MODIFIED,
];

#[derive(Default)]
struct Registry {
    sources: HashMap<String, MediaSource>,
    order: VecDeque<String>,
}

#[derive(Clone)]
struct ProxyState {
    registry: Arc<Mutex<Registry>>,
    client: reqwest::Client,
}

pub struct StreamProxy {
    address: SocketAddr,
    registry: Arc<Mutex<Registry>>,
}

impl StreamProxy {
    /// Binds a free loopback port and serves in the background on the current Tokio runtime.
    pub async fn start(client: reqwest::Client) -> std::io::Result<Self> {
        let registry = Arc::new(Mutex::new(Registry::default()));
        let state = ProxyState {
            registry: registry.clone(),
            client,
        };
        let router = Router::new()
            .route("/stream/{token}", get(stream))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router).await {
                tracing::error!(%error, "stream proxy stopped");
            }
        });
        tracing::info!(%address, "stream proxy listening");
        Ok(Self { address, registry })
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// Makes `source` playable and returns the URL to hand to the engine.
    pub fn register(&self, source: &MediaSource) -> String {
        let token = uuid::Uuid::new_v4().simple().to_string();
        let mut registry = self.registry.lock().expect("proxy registry poisoned");
        registry.sources.insert(token.clone(), source.clone());
        registry.order.push_back(token.clone());
        while registry.order.len() > MAX_REGISTERED {
            if let Some(expired) = registry.order.pop_front() {
                registry.sources.remove(&expired);
            }
        }
        format!("http://{}/stream/{token}", self.address)
    }
}

async fn stream(
    State(state): State<ProxyState>,
    Path(token): Path<String>,
    request: Request,
) -> Response {
    let source = state
        .registry
        .lock()
        .expect("proxy registry poisoned")
        .sources
        .get(&token)
        .cloned();
    let response = match source {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(MediaSource::LocalFile { path }) => serve_file(&path, request).await,
        Some(MediaSource::Url { url, headers }) => {
            forward(&state.client, &url, &headers, request.headers()).await
        }
    };
    with_cors(response)
}

async fn serve_file(path: &str, request: Request) -> Response {
    match ServeFile::new(path).oneshot(request).await {
        Ok(response) => response.map(Body::new),
        Err(infallible) => match infallible {},
    }
}

async fn forward(
    client: &reqwest::Client,
    url: &str,
    source_headers: &std::collections::BTreeMap<String, String>,
    request_headers: &HeaderMap,
) -> Response {
    let mut upstream = client.get(url);
    for (name, value) in source_headers {
        upstream = upstream.header(name, value);
    }
    if let Some(range) = request_headers.get(header::RANGE) {
        upstream = upstream.header(header::RANGE, range);
    }

    let upstream = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "stream proxy upstream request failed");
            return (StatusCode::BAD_GATEWAY, error.to_string()).into_response();
        }
    };

    let mut response = Response::builder().status(upstream.status().as_u16());
    for name in FORWARDED_RESPONSE_HEADERS {
        if let Some(value) = upstream.headers().get(&name) {
            response = response.header(name, value.as_bytes());
        }
    }
    response
        .body(Body::from_stream(upstream.bytes_stream()))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

fn with_cors(mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_ORIGIN,
        HeaderValue::from_static("*"),
    );
    headers.insert(
        header::ACCESS_CONTROL_EXPOSE_HEADERS,
        HeaderValue::from_static("content-length, content-range, accept-ranges"),
    );
    response
}

#[cfg(test)]
mod tests;
