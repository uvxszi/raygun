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
//! (manga pages, M24) at `/image/<token>`, covers, posters and icons from a lasting cache
//! (M46.2) at `/cover/<token>`, subtitle files for the video player (M41) at
//! `/subtitle/<token>`, and streams for the desktop video player (M46.4) at
//! `/media/<token>/<scheme>/<host>/<path>`, with their playlists rewritten to lead back to it.
//! Font files the app links (the device's own fonts on Android) are served at `/font/<token>`.
//! Pages of PDF books and chapters are drawn at `/pdf/<token>/<page>?w=<pixels>` ([`crate::pdf`]).
//! While a video plays on a TV, a second listener serves it on the local network ([`lan`]).

use std::collections::{BTreeMap, HashMap, VecDeque};
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

use crate::cover_cache::{CoverCache, Served};
use crate::images::{ImageError, fetch_image};
use crate::manifest::{self, Manifest};
use crate::page_cache::{PageCache, sniff_content_type};
use crate::pdf::PdfStore;
use tower::ServiceExt;
use tower_http::services::ServeFile;

/// Older tokens are forgotten beyond this many registrations.
const MAX_REGISTERED: usize = 64;
/// The biggest playlist or manifest the `/media/` route rewrites.
const MAX_MANIFEST_BYTES: usize = 16 * 1024 * 1024;
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
    /// Covers that stay cached whatever the size limit: the library's (M46.2).
    kept: std::collections::HashSet<String>,
}

/// The headers of each stream played through `/media/` (M46.4), by token.
#[derive(Default)]
struct MediaRegistry {
    headers: HashMap<String, BTreeMap<String, String>>,
    order: VecDeque<String>,
}

#[derive(Clone)]
struct ProxyState {
    address: SocketAddr,
    registry: Arc<Mutex<Registry>>,
    media: Arc<Mutex<MediaRegistry>>,
    images: Arc<Mutex<ImageRegistry>>,
    fonts: Arc<Mutex<HashMap<String, PathBuf>>>,
    client: reqwest::Client,
    artwork: Option<StaticFiles>,
    pages: Option<PageCache>,
    covers: Option<CoverCache>,
    local: Option<PathBuf>,
    pdfs: PdfStore,
}

/// What the proxy serves from disk besides streams.
#[derive(Clone, Default)]
pub struct ProxyStorage {
    /// Artwork files at `/art/<name>`.
    pub artwork: Option<StaticFiles>,
    /// Disk cache for `/image/<token>` (M27).
    pub pages: Option<PageCache>,
    /// Lasting cache for `/cover/<token>` (M46.2).
    pub covers: Option<CoverCache>,
    /// A folder served at `/local/<path>`, e.g. downloaded chapters (M28).
    pub local: Option<PathBuf>,
    /// Where the port is remembered (M46.47): the next start binds the same one when it's free,
    /// so links the UI kept from last time (its saved query cache) still work.
    pub port_file: Option<PathBuf>,
    /// Where PDF documents from the web are kept once fetched (`/pdf/`).
    pub pdf_dir: Option<PathBuf>,
}

/// Binds the remembered port when it's free, else any, and remembers the one bound.
async fn bind_remembered(
    port_file: Option<&std::path::Path>,
) -> std::io::Result<tokio::net::TcpListener> {
    let remembered = match port_file {
        Some(file) => tokio::fs::read_to_string(file)
            .await
            .ok()
            .and_then(|text| text.trim().parse::<u16>().ok())
            .filter(|port| *port >= 1024),
        None => None,
    };
    let listener = match remembered {
        Some(port) => match tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, port)).await {
            Ok(listener) => listener,
            Err(_) => tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?,
        },
        None => tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).await?,
    };
    if let Some(file) = port_file {
        let port = listener.local_addr()?.port();
        if remembered != Some(port) {
            if let Some(parent) = file.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            let _ = tokio::fs::write(file, port.to_string()).await;
        }
    }
    Ok(listener)
}

pub struct StreamProxy {
    address: SocketAddr,
    registry: Arc<Mutex<Registry>>,
    images: ImageLinks,
    media: MediaLinks,
    fonts: FontLinks,
    pdfs: PdfStore,
    /// What the routes share, for the local network's listener (Play on TV).
    state: ProxyState,
}

/// Hands out `/media/` addresses for streams the webview plays (M46.4): every request under one
/// is fetched with the stream's headers, and playlists are rewritten to stay in the route.
#[derive(Clone)]
pub struct MediaLinks {
    address: SocketAddr,
    registry: Arc<Mutex<MediaRegistry>>,
}

impl MediaLinks {
    /// The proxy address for a stream at `url` that needs `headers`; `None` for an address that
    /// isn't a web one.
    pub fn link(&self, url: &str, headers: &BTreeMap<String, String>) -> Option<String> {
        let url = reqwest::Url::parse(url).ok()?;
        if !matches!(url.scheme(), "http" | "https") || url.host_str().is_none() {
            return None;
        }
        let token = uuid::Uuid::new_v4().simple().to_string();
        let mut registry = self.registry.lock().expect("media registry poisoned");
        registry.headers.insert(token.clone(), headers.clone());
        registry.order.push_back(token.clone());
        while registry.order.len() > MAX_REGISTERED {
            if let Some(expired) = registry.order.pop_front() {
                registry.headers.remove(&expired);
            }
        }
        Some(format!(
            "http://{}/media/{token}/{}",
            self.address,
            manifest::media_path(&url)
        ))
    }

    /// Whether `url` already points at this proxy (a download, a subtitle…).
    pub fn is_local(&self, url: &str) -> bool {
        url.starts_with(&format!("http://{}/", self.address))
    }

    /// The YouTube embed host page with YouTube's own controls, for trailers.
    pub fn youtube_embed_url(&self) -> String {
        format!(
            "http://localhost:{}/embed/youtube?controls=1",
            self.address.port()
        )
    }
}

/// Hands out `/font/` addresses for font files on disk, which the webview loads with
/// `@font-face` where it can't use installed families by name (Android). Only linked files are
/// served, and one file always has one address.
#[derive(Clone)]
pub struct FontLinks {
    address: SocketAddr,
    files: Arc<Mutex<HashMap<String, PathBuf>>>,
}

impl FontLinks {
    pub fn link(&self, path: &std::path::Path) -> String {
        use sha2::{Digest, Sha256};
        let digest = Sha256::digest(path.to_string_lossy().as_bytes());
        let token: String = digest
            .iter()
            .take(16)
            .map(|byte| format!("{byte:02x}"))
            .collect();
        self.files
            .lock()
            .expect("font registry poisoned")
            .insert(token.clone(), path.to_path_buf());
        format!("http://{}/font/{token}", self.address)
    }
}

/// Hands out proxy URLs for remote images. Cheap to clone and share between services.
#[derive(Clone)]
pub struct ImageLinks {
    address: SocketAddr,
    registry: Arc<Mutex<ImageRegistry>>,
    covers: Option<CoverCache>,
}

/// The cache name of an image: a hash of its address and headers, the same in every version.
pub fn image_token(url: &str, headers: &BTreeMap<String, String>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    hasher.update(url.as_bytes());
    for (name, value) in headers {
        hasher.update([0]);
        hasher.update(name.as_bytes());
        hasher.update([1]);
        hasher.update(value.as_bytes());
    }
    hasher
        .finalize()
        .iter()
        .take(16)
        .map(|byte| format!("{byte:02x}"))
        .collect()
}

impl ImageLinks {
    /// The proxy URL of a cover, poster, thumbnail or icon: cached on disk and checked again when
    /// old (M46.2). Reader pages use [`Self::link`].
    pub fn cover(&self, url: &str, headers: &BTreeMap<String, String>) -> String {
        format!("http://{}/cover/{}", self.address, self.token(url, headers))
    }

    /// Like [`Self::cover`], for an image that stays cached whatever the size limit: library
    /// covers, followed podcasts, installed extensions.
    pub fn kept_cover(&self, url: &str, headers: &BTreeMap<String, String>) -> String {
        let token = self.token(url, headers);
        self.registry
            .lock()
            .expect("image registry poisoned")
            .kept
            .insert(token.clone());
        format!("http://{}/cover/{token}", self.address)
    }

    /// Drops the cached copy of a cover, so it's fetched again when next seen (a manual refresh).
    pub async fn forget_cover(&self, url: &str, headers: &BTreeMap<String, String>) {
        if let Some(covers) = &self.covers {
            covers.forget(&image_token(url, headers)).await;
        }
    }

    /// The lasting cover cache, for its usage and settings.
    pub fn cover_cache(&self) -> Option<&CoverCache> {
        self.covers.as_ref()
    }

    /// The source address behind one of this proxy's image or cover links; other URLs as they
    /// are. Stored posters must outlive the app, and a proxy link doesn't.
    pub fn original(&self, url: &str) -> String {
        self.source(url).0
    }

    /// Like [`Self::original`], with the headers the image is fetched with (a source's Referer).
    pub fn source(&self, url: &str) -> (String, BTreeMap<String, String>) {
        let token = url
            .strip_prefix(&format!("http://{}/", self.address))
            .and_then(|rest| {
                rest.strip_prefix("cover/")
                    .or_else(|| rest.strip_prefix("image/"))
            });
        token
            .and_then(|token| {
                self.registry
                    .lock()
                    .expect("image registry poisoned")
                    .images
                    .get(token)
                    .cloned()
            })
            .unwrap_or_else(|| (url.to_owned(), BTreeMap::new()))
    }

    /// One of this proxy's links from an earlier start (another port) at this run's address, so
    /// a link saved then still finds its copy on disk; other URLs as they are.
    pub fn relink(&self, url: &str) -> String {
        let Some(rest) = ["http://127.0.0.1:", "http://localhost:"]
            .iter()
            .find_map(|origin| url.strip_prefix(origin))
        else {
            return url.to_owned();
        };
        let Some((port, path)) = rest.split_once('/') else {
            return url.to_owned();
        };
        let ours = ["cover/", "image/", "art/", "local/", "pdf/"]
            .iter()
            .any(|route| path.starts_with(route));
        if !ours || port.parse::<u16>().is_err() {
            return url.to_owned();
        }
        format!("http://{}/{path}", self.address)
    }

    /// Whether `url` is a remote address this proxy can cache (not already one of its own).
    pub fn is_remote(&self, url: &str) -> bool {
        (url.starts_with("https://") || url.starts_with("http://"))
            && !url.starts_with(&format!("http://{}/", self.address))
    }

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
        let token = image_token(url, headers);
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
            .is_some_and(|rest| {
                rest.starts_with("image/")
                    || rest.starts_with("cover/")
                    || rest.starts_with("local/")
                    || rest.starts_with("pdf/")
            })
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
            covers,
            local,
            port_file,
            pdf_dir,
        } = storage;
        // Bound first: the `/media/` route links back to its own address.
        let listener = bind_remembered(port_file.as_deref()).await?;
        let address = listener.local_addr()?;
        let registry = Arc::new(Mutex::new(Registry::default()));
        let images = Arc::new(Mutex::new(ImageRegistry::default()));
        let media = Arc::new(Mutex::new(MediaRegistry::default()));
        let fonts = Arc::new(Mutex::new(HashMap::new()));
        let pdfs = PdfStore::new(
            address,
            client.clone(),
            local.clone(),
            pdf_dir,
            pages.clone(),
        );
        let state = ProxyState {
            address,
            registry: registry.clone(),
            media: media.clone(),
            images: images.clone(),
            fonts: fonts.clone(),
            client,
            artwork,
            pages,
            covers: covers.clone(),
            local,
            pdfs: pdfs.clone(),
        };
        let shared = state.clone();
        let router = Router::new()
            .route("/stream/{token}", get(stream))
            .route("/image/{token}", get(serve_image))
            .route("/cover/{token}", get(serve_cover))
            .route("/subtitle/{token}", get(serve_subtitle))
            .route("/art/{name}", get(serve_artwork))
            .route("/local/{*path}", get(serve_local))
            .route("/font/{token}", get(serve_font))
            .route("/pdf/{token}/{page}", get(serve_pdf_page))
            .route(
                "/media/{token}/{*rest}",
                get(serve_media).options(preflight),
            )
            .route("/embed/youtube", get(youtube_embed))
            .with_state(state);
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
                covers,
            },
            media: MediaLinks {
                address,
                registry: media,
            },
            fonts: FontLinks {
                address,
                files: fonts,
            },
            pdfs,
            state: shared,
        })
    }

    /// The handle that serves videos on the local network while they play on a TV.
    pub fn lan(&self) -> lan::LanLinks {
        lan::LanLinks {
            state: self.state.clone(),
        }
    }

    /// The handle that links streams through `/media/` (M46.4).
    pub fn media(&self) -> MediaLinks {
        self.media.clone()
    }

    /// The handle other services use to link remote images through this proxy.
    pub fn images(&self) -> ImageLinks {
        self.images.clone()
    }

    /// The handle that links font files at `/font/`.
    pub fn fonts(&self) -> FontLinks {
        self.fonts.clone()
    }

    /// The PDF documents whose pages `/pdf/` draws.
    pub fn pdfs(&self) -> PdfStore {
        self.pdfs.clone()
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
    with_cors(serve_registered(&state, &token, request).await)
}

/// A source registered with [`StreamProxy::register`], by its token.
async fn serve_registered(state: &ProxyState, token: &str, request: Request) -> Response {
    let source = state
        .registry
        .lock()
        .expect("proxy registry poisoned")
        .sources
        .get(token)
        .cloned();
    match source {
        None => StatusCode::NOT_FOUND.into_response(),
        Some(MediaSource::LocalFile { path }) => serve_file(&path, request).await,
        Some(MediaSource::Url { url, headers }) => {
            forward(&state.client, &url, &headers, request.headers()).await
        }
        // Neither has bytes to proxy: YouTube plays in its embed, a lookup resolves first.
        Some(MediaSource::YouTube { .. } | MediaSource::Lookup { .. }) => {
            StatusCode::NOT_FOUND.into_response()
        }
    }
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
pub(crate) fn local_path(root: &std::path::Path, relative: &str) -> Option<PathBuf> {
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

/// A PDF page drawn as a picture, as wide as `?w=` asks (rounded up to one of a few widths).
async fn serve_pdf_page(
    State(state): State<ProxyState>,
    Path((token, page)): Path<(String, String)>,
    request: Request,
) -> Response {
    let Ok(index) = page.parse::<usize>() else {
        return with_cors(StatusCode::NOT_FOUND.into_response());
    };
    let width = request.uri().query().and_then(|query| {
        query
            .split('&')
            .find_map(|pair| pair.strip_prefix("w=")?.parse::<u32>().ok())
    });
    let response = match state.pdfs.page_png(&token, index, width).await {
        Ok(bytes) => image_response(bytes),
        Err(crate::pdf::PdfError::Offline) => offline_response(),
        Err(crate::pdf::PdfError::Missing) => StatusCode::NOT_FOUND.into_response(),
        Err(error) => {
            tracing::debug!(%error, "pdf page not drawn");
            (StatusCode::BAD_GATEWAY, error.to_string()).into_response()
        }
    };
    with_cors(response)
}

/// A font file linked with [`FontLinks::link`], by its token.
async fn serve_font(
    State(state): State<ProxyState>,
    Path(token): Path<String>,
    request: Request,
) -> Response {
    let path = state
        .fonts
        .lock()
        .expect("font registry poisoned")
        .get(&token)
        .cloned();
    let response = match path {
        Some(path) => {
            let mut response = serve_file(&path.to_string_lossy(), request).await;
            if response.status().is_success() {
                response.headers_mut().insert(
                    header::CACHE_CONTROL,
                    HeaderValue::from_static("public, max-age=86400"),
                );
            }
            response
        }
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
        // A picture its source fetches can't be forwarded as it is.
        Some((url, headers)) if crate::images::source_marked(&url).is_some() => {
            match fetch_image(&state.client, &url, &headers).await {
                Ok(image) => image_response(image.bytes),
                Err(error) if crate::network_mode::offline() => {
                    tracing::debug!(%error, "picture not fetched while offline");
                    offline_response()
                }
                Err(error) => (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
            }
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

/// A cover from the lasting cache (M46.2), or the page cache when there is none. Copies stored
/// as JPEG XL go as the request takes them (its `Accept`, the webview's probe).
async fn serve_cover(
    State(state): State<ProxyState>,
    Path(token): Path<String>,
    request: Request,
) -> Response {
    let accept = request
        .headers()
        .get(header::ACCEPT)
        .and_then(|value| value.to_str().ok())
        .unwrap_or_default()
        .to_owned();
    let entry = {
        let registry = state.images.lock().expect("image registry poisoned");
        registry
            .images
            .get(&token)
            .cloned()
            .map(|(url, headers)| (url, headers, registry.kept.contains(&token)))
    };
    let Some((url, headers, kept)) = entry else {
        // A link from an earlier start (the UI's saved cache, M46.47): its copy on disk, if any.
        let stored = match &state.covers {
            Some(covers) => covers.stored_served(&token, &accept).await,
            None => None,
        };
        return with_cors(match stored {
            Some(served) => cover_response(served),
            None => StatusCode::NOT_FOUND.into_response(),
        });
    };
    let Some(covers) = &state.covers else {
        return serve_image(State(state), Path(token), request).await;
    };
    let response = match covers
        .get_served(&state.client, &token, &url, &headers, kept, &accept)
        .await
    {
        Ok(served) => cover_response(served),
        // Not cached, and Offline Mode is on: the page shows its placeholder.
        Err(error) if crate::network_mode::mentions_offline(&error) => offline_response(),
        Err(error) => {
            tracing::debug!(%error, "cover request failed");
            (StatusCode::BAD_GATEWAY, error).into_response()
        }
    };
    with_cors(response)
}

/// A cover with its type; what's sent depends on the request's `Accept`, so caches keep one
/// version per kind of request.
fn cover_response(served: Served) -> Response {
    (
        [
            (header::CONTENT_TYPE, served.content_type),
            (header::CACHE_CONTROL, "public, max-age=86400"),
            (header::VARY, "Accept"),
        ],
        served.bytes,
    )
        .into_response()
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
    crate::network_mode::check(url)?;
    let request = crate::server_auth::get(client, url, headers).timeout(SUBTITLE_TIMEOUT);
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
    if !crate::network_mode::allows(url) {
        return offline_response();
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

/// The client's `Range`, or `bytes=0-` for a stream that must be asked for by range (YouTube's,
/// [`crate::wants_range`]); `true` when it was added, so the whole file goes back as asked.
fn upstream_range(url: &str, request_headers: &HeaderMap) -> (Option<HeaderValue>, bool) {
    match request_headers.get(header::RANGE) {
        Some(range) => (Some(range.clone()), false),
        None if crate::wants_range(url) => (Some(HeaderValue::from_static("bytes=0-")), true),
        None => (None, false),
    }
}

/// The status the player gets: the whole file it asked for is `200`, though asked for by range.
fn client_status(status: reqwest::StatusCode, added_range: bool) -> u16 {
    if added_range && status == reqwest::StatusCode::PARTIAL_CONTENT {
        200
    } else {
        status.as_u16()
    }
}

/// What the proxy answers for something it would fetch while Offline Mode is on: only what's on
/// this device (downloads, cached covers and pages) is served then.
fn offline_response() -> Response {
    (
        StatusCode::SERVICE_UNAVAILABLE,
        [(header::CACHE_CONTROL, "no-store")],
        crate::network_mode::OFFLINE_MESSAGE,
    )
        .into_response()
}

async fn forward(
    client: &reqwest::Client,
    url: &str,
    source_headers: &std::collections::BTreeMap<String, String>,
    request_headers: &HeaderMap,
) -> Response {
    if !crate::network_mode::allows(url) {
        return offline_response();
    }
    // A connected server's address signs in (`server_auth`): stored addresses carry no secrets.
    let mut upstream = crate::server_auth::get(client, url, source_headers);
    let (range, added_range) = upstream_range(url, request_headers);
    if let Some(range) = range {
        upstream = upstream.header(header::RANGE, range);
    }

    let upstream = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "stream proxy upstream request failed");
            return (StatusCode::BAD_GATEWAY, error.to_string()).into_response();
        }
    };

    let status = client_status(upstream.status(), added_range);
    let mut response = Response::builder().status(status);
    for name in FORWARDED_RESPONSE_HEADERS {
        if status == 200 && name == header::CONTENT_RANGE {
            continue;
        }
        if let Some(value) = upstream.headers().get(&name) {
            response = response.header(name, value.as_bytes());
        }
    }
    // A stream being played stops when Offline Mode turns on.
    response
        .body(Body::from_stream(crate::network_mode::until_offline(
            url,
            upstream.bytes_stream(),
        )))
        .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response())
}

/// A stream for the desktop video player: fetched with its token's headers; a playlist or
/// manifest comes back rewritten so what it names is fetched through here too.
async fn serve_media(State(state): State<ProxyState>, request: Request) -> Response {
    let path = request.uri().path();
    let Some((token, rest)) = path
        .strip_prefix("/media/")
        .and_then(|path| path.split_once('/'))
    else {
        return with_cors(StatusCode::NOT_FOUND.into_response());
    };
    let headers = state
        .media
        .lock()
        .expect("media registry poisoned")
        .headers
        .get(token)
        .cloned();
    let Some(headers) = headers else {
        return with_cors(StatusCode::NOT_FOUND.into_response());
    };
    let Some(upstream) = manifest::upstream_url(rest, request.uri().query()) else {
        return with_cors(StatusCode::BAD_REQUEST.into_response());
    };
    let link_base = format!("http://{}/media/{token}/", state.address);
    with_cors(forward_media(&state, &link_base, &upstream, &headers, request.headers()).await)
}

async fn forward_media(
    state: &ProxyState,
    link_base: &str,
    url: &reqwest::Url,
    source_headers: &BTreeMap<String, String>,
    request_headers: &HeaderMap,
) -> Response {
    if !crate::network_mode::allows(url.as_str()) {
        return offline_response();
    }
    let mut upstream = crate::server_auth::get(&state.client, url.as_str(), source_headers);
    let (range, added_range) = upstream_range(url.as_str(), request_headers);
    if let Some(range) = range {
        upstream = upstream.header(header::RANGE, range);
    }
    let upstream = match upstream.send().await {
        Ok(response) => response,
        Err(error) => {
            tracing::warn!(%error, "media request failed");
            return (StatusCode::BAD_GATEWAY, error.to_string()).into_response();
        }
    };
    let status = upstream.status();
    let shown_status = client_status(status, added_range);
    let final_url = upstream.url().clone();
    let mut forwarded = Vec::new();
    for name in FORWARDED_RESPONSE_HEADERS {
        if shown_status == 200 && name == header::CONTENT_RANGE {
            continue;
        }
        if let Some(value) = upstream.headers().get(&name) {
            forwarded.push((name, value.clone()));
        }
    }

    // The first bytes say whether this is a playlist to rewrite or media to pass on.
    let mut body = upstream.bytes_stream();
    let mut head = Vec::new();
    while head.len() < manifest::SNIFF_BYTES {
        match body.next().await {
            Some(Ok(chunk)) => head.extend_from_slice(&chunk),
            Some(Err(error)) => {
                return (StatusCode::BAD_GATEWAY, error.to_string()).into_response();
            }
            None => break,
        }
    }
    let kind = if status.is_success() {
        manifest::sniff(&head)
    } else {
        None
    };
    let Some(kind) = kind else {
        let mut response = Response::builder().status(shown_status);
        for (name, value) in forwarded {
            response = response.header(name, value);
        }
        let first = futures_util::stream::once(async move {
            Ok::<_, reqwest::Error>(axum::body::Bytes::from(head))
        });
        return response
            .body(Body::from_stream(crate::network_mode::until_offline(
                url.as_str(),
                first.chain(body),
            )))
            .unwrap_or_else(|_| StatusCode::BAD_GATEWAY.into_response());
    };

    let mut bytes = head;
    while let Some(chunk) = body.next().await {
        match chunk {
            Ok(chunk) if bytes.len() + chunk.len() <= MAX_MANIFEST_BYTES => {
                bytes.extend_from_slice(&chunk);
            }
            Ok(_) => return (StatusCode::BAD_GATEWAY, "playlist too large").into_response(),
            Err(error) => return (StatusCode::BAD_GATEWAY, error.to_string()).into_response(),
        }
    }
    let text = String::from_utf8_lossy(&bytes);
    let link = |target: &reqwest::Url| format!("{link_base}{}", manifest::media_path(target));
    let (rewritten, content_type) = match kind {
        Manifest::Hls => (
            manifest::rewrite_hls(&text, &final_url, &link),
            "application/vnd.apple.mpegurl",
        ),
        Manifest::Dash => (
            manifest::rewrite_dash(&text, &final_url, final_url != *url, &link),
            "application/dash+xml",
        ),
    };
    (
        [
            (header::CONTENT_TYPE, content_type),
            (header::CACHE_CONTROL, "no-store"),
        ],
        rewritten,
    )
        .into_response()
}

/// Answers CORS preflights for `/media/` (hls.js and dash.js may ask with `Range`).
async fn preflight() -> Response {
    let mut response = StatusCode::NO_CONTENT.into_response();
    let headers = response.headers_mut();
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_METHODS,
        HeaderValue::from_static("GET, HEAD, OPTIONS"),
    );
    headers.insert(
        header::ACCESS_CONTROL_ALLOW_HEADERS,
        HeaderValue::from_static("range, content-type"),
    );
    headers.insert(
        header::ACCESS_CONTROL_MAX_AGE,
        HeaderValue::from_static("86400"),
    );
    with_cors(response)
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

pub mod lan;

#[cfg(test)]
mod tests;
