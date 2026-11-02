//! Loopback stream proxy (ARCHITECTURE §6.2, adapted from the idea in Nuclear's
//! `stream_server.rs`, AGPL-3.0).
//!
//! The webview's `<audio>`/`<video>` element can't send custom headers and is subject to CORS, but
//! many hosts require a Referer, User-Agent or Cookie. The proxy serves each registered source at
//! `http://127.0.0.1:<port>/stream/<token>`: remote sources are fetched with their headers plus the
//! client's `Range`, local files are served with Range support. Only registered tokens resolve, so
//! the proxy can't be used to fetch arbitrary URLs.
//!
//! It also serves library artwork from one folder at `/art/<name>`, limited to names the caller's
//! validator accepts, the YouTube embed host page at `/embed/youtube` (M08), remote images
//! (manga covers and pages, M24) at `/image/<token>`, and subtitle files for the video player
//! (M41) at `/subtitle/<token>`.

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::hash::{Hash, Hasher};
use std::net::{Ipv4Addr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Router;
use axum::body::Body;
use axum::extract::{Path, Request, State};
use axum::http::{HeaderMap, HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use futures_util::StreamExt;
use raygun_model::MediaSource;

use crate::images::{ImageError, fetch_image};
use crate::page_cache::{PageCache, sniff_content_type};
use tower::ServiceExt;
use tower_http::services::ServeFile;

/// Older tokens are forgotten beyond this many registrations.
const MAX_REGISTERED: usize = 64;
/// Image links remembered (a few library pages of covers plus several chapters of pages).
const MAX_IMAGES: usize = 4096;
/// Longest a subtitle file may take to arrive, and its biggest size.
const SUBTITLE_TIMEOUT: Duration = Duration::from_secs(20);
const MAX_SUBTITLE_BYTES: usize = 5 * 1024 * 1024;
/// Served in place of a subtitle file that can't be fetched: an empty track. The video player
/// gives up on the whole video when any file it was handed fails.
const EMPTY_SUBTITLES: &str = "WEBVTT

";

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

/// A folder of files served at `/art/<name>`.
#[derive(Clone)]
pub struct StaticFiles {
    pub dir: PathBuf,
    /// Only names this returns true for are served (e.g. content-hash file names).
    pub is_allowed: fn(&str) -> bool,
}

/// Remote images by a stable token, so one image always has one URL and the webview caches it.
#[derive(Default)]
struct ImageRegistry {
    images: HashMap<String, (String, BTreeMap<String, String>)>,
    order: VecDeque<String>,
}

#[derive(Clone)]
struct ProxyState {
    registry: Arc<Mutex<Registry>>,
    images: Arc<Mutex<ImageRegistry>>,
    client: reqwest::Client,
    artwork: Option<StaticFiles>,
    pages: Option<PageCache>,
    local: Option<PathBuf>,
}

/// What the proxy serves from disk besides streams.
#[derive(Clone, Default)]
pub struct ProxyStorage {
    /// Artwork files at `/art/<name>`.
    pub artwork: Option<StaticFiles>,
    /// Disk cache for `/image/<token>` (M27).
    pub pages: Option<PageCache>,
    /// A folder served at `/local/<path>`, e.g. downloaded chapters (M28).
    pub local: Option<PathBuf>,
}

pub struct StreamProxy {
    address: SocketAddr,
    registry: Arc<Mutex<Registry>>,
    images: ImageLinks,
}

/// Hands out proxy URLs for remote images. Cheap to clone and share between services.
#[derive(Clone)]
pub struct ImageLinks {
    address: SocketAddr,
    registry: Arc<Mutex<ImageRegistry>>,
}

impl ImageLinks {
    /// The proxy URL for `url`, fetched with `headers` (e.g. a Referer the host requires).
    pub fn link(&self, url: &str, headers: &BTreeMap<String, String>) -> String {
        format!("http://{}/image/{}", self.address, self.token(url, headers))
    }

    /// The proxy URL of a subtitle file, which never fails: a file that can't be fetched comes
    /// back as an empty track.
    pub fn subtitle(&self, url: &str, headers: &BTreeMap<String, String>) -> String {
        format!(
            "http://{}/subtitle/{}",
            self.address,
            self.token(url, headers)
        )
    }

    fn token(&self, url: &str, headers: &BTreeMap<String, String>) -> String {
        let mut hasher = std::collections::hash_map::DefaultHasher::new();
        url.hash(&mut hasher);
        headers.hash(&mut hasher);
        let token = format!("{:016x}", hasher.finish());
        let mut registry = self.registry.lock().expect("image registry poisoned");
        if registry
            .images
            .insert(token.clone(), (url.to_owned(), headers.clone()))
            .is_none()
        {
            registry.order.push_back(token.clone());
            while registry.order.len() > MAX_IMAGES {
                if let Some(expired) = registry.order.pop_front() {
                    registry.images.remove(&expired);
                }
            }
        }
        token
    }

    /// The proxy URL of a file under the [`ProxyStorage::local`] folder, by its relative path.
    pub fn local(&self, relative: &str) -> String {
        format!("http://{}/local/{relative}", self.address)
    }

    /// Whether `url` is one of this proxy's image or local-file URLs. Pages are saved by fetching
    /// them back through the proxy, which knows their headers and cached copies (M46.1).
    pub fn is_page(&self, url: &str) -> bool {
        url.strip_prefix(&format!("http://{}/", self.address))
            .is_some_and(|rest| rest.starts_with("image/") || rest.starts_with("local/"))
    }
}

impl StreamProxy {
    /// Binds a free loopback port and serves in the background on the current Tokio runtime.
    pub async fn start(
        client: reqwest::Client,
        artwork: Option<StaticFiles>,
    ) -> std::io::Result<Self> {
        Self::start_with(
            client,
            ProxyStorage {
                artwork,
                ..ProxyStorage::default()
            },
        )
        .await
    }

    /// Like [`Self::start`], with everything it serves from disk.
    pub async fn start_with(
        client: reqwest::Client,
        storage: ProxyStorage,
    ) -> std::io::Result<Self> {
        let ProxyStorage {
            artwork,
            pages,
            local,
        } = storage;
        let registry = Arc::new(Mutex::new(Registry::default()));
        let images = Arc::new(Mutex::new(ImageRegistry::default()));
        let state = ProxyState {
            registry: registry.clone(),
            images: images.clone(),
            client,
            artwork,
            pages,
            local,
        };
        let router = Router::new()
            .route("/stream/{token}", get(stream))
            .route("/image/{token}", get(serve_image))
            .route("/subtitle/{token}", get(serve_subtitle))
            .route("/art/{name}", get(serve_artwork))
            .route("/local/{*path}", get(serve_local))
            .route("/embed/youtube", get(youtube_embed))
            .with_state(state);
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?;
        let address = listener.local_addr()?;
        tokio::spawn(async move {
            if let Err(error) = axum::serve(listener, router).await {
                tracing::error!(%error, "stream proxy stopped");
            }
        });
        tracing::info!(%address, "stream proxy listening");
        Ok(Self {
            address,
            registry,
            images: ImageLinks {
                address,
                registry: images,
            },
        })
    }

    /// The handle other services use to link remote images through this proxy.
    pub fn images(&self) -> ImageLinks {
        self.images.clone()
    }

    pub fn address(&self) -> SocketAddr {
        self.address
    }

    /// URL of the YouTube embed host page. Uses the hostname `localhost` because YouTube's embed
    /// check refuses literal IP addresses in the Referer.
    pub fn youtube_embed_url(&self) -> String {
        format!("http://localhost:{}/embed/youtube", self.address.port())
    }

    /// URL of an artwork file served from the [`StaticFiles`] folder.
    pub fn artwork_url(&self, name: &str) -> String {
        artwork_url(self.address, name)
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
        // Played by YouTube's own player, never as a byte stream.
        // Neither has bytes to proxy: YouTube plays in its embed, a lookup resolves first.
        Some(MediaSource::YouTube { .. } | MediaSource::Lookup { .. }) => {
            StatusCode::NOT_FOUND.into_response()
        }
    };
    with_cors(response)
}

/// Builds artwork URLs without holding the proxy (e.g. from another service).
pub fn artwork_url(address: SocketAddr, name: &str) -> String {
    format!("http://{address}/art/{name}")
}

async fn serve_artwork(
    State(state): State<ProxyState>,
    Path(name): Path<String>,
    request: Request,
) -> Response {
    let response = match &state.artwork {
        Some(files) if (files.is_allowed)(&name) => {
            let mut response = serve_file(&files.dir.join(&name).to_string_lossy(), request).await;
            response.headers_mut().insert(
                header::CACHE_CONTROL,
                HeaderValue::from_static("public, max-age=31536000, immutable"),
            );
            response
        }
        _ => StatusCode::NOT_FOUND.into_response(),
    };
    with_cors(response)
}

/// A path under the local folder: plain segments only, nothing hidden, nothing going up.
fn local_path(root: &std::path::Path, relative: &str) -> Option<PathBuf> {
    let mut path = root.to_path_buf();
    for segment in relative.split('/') {
        let plain = !segment.is_empty()
            && !segment.starts_with('.')
            && segment
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'.' | b'_' | b'-'));
        if !plain {
            return None;
        }
        path.push(segment);
    }
    Some(path)
}

async fn serve_local(
    State(state): State<ProxyState>,
    Path(relative): Path<String>,
    request: Request,
) -> Response {
    let path = state
        .local
        .as_deref()
        .and_then(|root| local_path(root, &relative));
    let response = match path {
        Some(path) => serve_file(&path.to_string_lossy(), request).await,
        None => StatusCode::NOT_FOUND.into_response(),
    };
    with_cors(response)
}

async fn serve_image(
    State(state): State<ProxyState>,
    Path(token): Path<String>,
    request: Request,
) -> Response {
    let image = state
        .images
        .lock()
        .expect("image registry poisoned")
        .images
        .get(&token)
        .cloned();
    let response = match image {
        None => StatusCode::NOT_FOUND.into_response(),
        Some((url, headers)) if state.pages.is_some() => {
            let cache = state.pages.as_ref().expect("checked above");
            cached_image(cache, &state.client, &token, &url, &headers).await
        }
        Some((url, headers)) => {
            let mut response = forward(&state.client, &url, &headers, request.headers()).await;
            if response.status().is_success() {
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=86400"),
                );
            }
            response
        }
    };
    with_cors(response)
}

async fn serve_subtitle(State(state): State<ProxyState>, Path(token): Path<String>) -> Response {
    let link = state
        .images
        .lock()
        .expect("image registry poisoned")
        .images
        .get(&token)
        .cloned();
    let Some((url, headers)) = link else {
        return with_cors(StatusCode::NOT_FOUND.into_response());
    };
    let body = match fetch_subtitle(&state.client, &url, &headers).await {
        Ok(bytes) => bytes,
        Err(error) => {
            tracing::warn!(%error, "subtitle request failed");
            EMPTY_SUBTITLES.as_bytes().to_vec()
        }
    };
    with_cors(
        (
            [
                (header::CONTENT_TYPE, "text/plain; charset=utf-8"),
                (header::CACHE_CONTROL, "no-store"),
            ],
            body,
        )
            .into_response(),
    )
}

async fn fetch_subtitle(
    client: &reqwest::Client,
    url: &str,
    headers: &BTreeMap<String, String>,
) -> Result<Vec<u8>, String> {
    let mut request = client.get(url).timeout(SUBTITLE_TIMEOUT);
    for (name, value) in headers {
        request = request.header(name, value);
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    if !response.status().is_success() {
        return Err(format!("the host answered {}", response.status()));
    }
    let html = response
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .is_some_and(|kind| kind.starts_with("text/html"));
    if html {
        return Err("the host sent a web page".into());
    }
    let mut bytes = Vec::new();
    let mut body = response.bytes_stream();
    while let Some(chunk) = body.next().await {
        let chunk = chunk.map_err(|error| error.to_string())?;
        if bytes.len() + chunk.len() > MAX_SUBTITLE_BYTES {
            return Err("the file is too large".into());
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok(bytes)
}

/// Serves an image from the page cache, or fetches it whole, stores it and serves it.
async fn cached_image(
    cache: &PageCache,
    client: &reqwest::Client,
    token: &str,
    url: &str,
    headers: &BTreeMap<String, String>,
) -> Response {
    if let Some(bytes) = cache.read(token).await {
        return image_response(bytes);
    }
    match fetch_image(client, url, headers).await {
        Ok(image) => {
            cache.write(token, &image.bytes).await;
            image_response(image.bytes)
        }
        Err(ImageError::Status(status) | ImageError::Busy(status, _)) => {
            StatusCode::from_u16(status)
                .unwrap_or(StatusCode::BAD_GATEWAY)
                .into_response()
        }
        Err(error) => {
            tracing::warn!(%error, "image request failed");
            (StatusCode::BAD_GATEWAY, error.to_string()).into_response()
        }
    }
}

fn image_response(bytes: Vec<u8>) -> Response {
    (
        [
            (header::CONTENT_TYPE, sniff_content_type(&bytes)),
            (header::CACHE_CONTROL, "public, max-age=86400"),
        ],
        bytes,
    )
        .into_response()
}

async fn youtube_embed() -> Response {
    (
        [
            (header::CONTENT_TYPE, "text/html; charset=utf-8"),
            (header::CACHE_CONTROL, "no-store"),
        ],
        include_str!("embed/youtube.html"),
    )
        .into_response()
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
