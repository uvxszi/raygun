//! The proxy on the local network while a video plays on a TV (Play on TV). A TV can't reach the
//! loopback proxy, so each cast opens its own listener on this device's LAN address, and every
//! address it serves starts with a random token: `http://<lan ip>:<port>/cast/<token>/…`. Behind
//! the token it serves what the loopback proxy does for the video player, with the stream's
//! headers added and HLS and DASH playlists rewritten to lead back to it:
//!
//! - `media/<id>/<scheme>/<host>/<path>`: a remote stream (as the `/media/` route),
//! - `stream/<token>`: a source registered with the proxy (a file on this device),
//! - `local/<path>`: a downloaded video under the proxy's local folder,
//! - `torrent/<token>/<name>`: a torrent's video as its pieces arrive,
//! - `subtitle/<n>.vtt` or `.srt`: a subtitle file converted for the TV.
//!
//! The listener closes when the [`LanRoute`] is dropped.

use std::collections::BTreeMap;
use std::net::{IpAddr, SocketAddr};
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use axum::Router;
use axum::extract::{ConnectInfo, Request, State};
use axum::http::{HeaderValue, StatusCode, header};
use axum::response::{IntoResponse, Response};

use super::{
    MAX_REGISTERED, MAX_SUBTITLE_BYTES, ProxyState, fetch_subtitle, forward_media, local_path,
    serve_registered, with_cors,
};
use crate::manifest;
use crate::subtitles;

/// What a DLNA TV is told it may do with a stream: seek by byte range, stream it.
const DLNA_FEATURES: &str =
    "DLNA.ORG_OP=01;DLNA.ORG_CI=0;DLNA.ORG_FLAGS=01700000000000000000000000000000";

/// A subtitle file a TV reads through the route.
#[derive(Debug, Clone)]
enum SubtitleFile {
    Remote {
        url: String,
        headers: BTreeMap<String, String>,
    },
    Local(PathBuf),
}

/// What the TV gets a subtitle file as.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SubtitleFormat {
    /// For Google Cast.
    WebVtt,
    /// For DLNA TVs.
    SubRip,
}

struct LanShared {
    token: String,
    address: SocketAddr,
    proxy: ProxyState,
    subtitles: Mutex<Vec<SubtitleFile>>,
    /// The subtitle file Samsung TVs are pointed to with `CaptionInfo.sec`.
    caption: Mutex<Option<String>>,
}

/// Opens [`LanRoute`]s. Cheap to clone.
#[derive(Clone)]
pub struct LanLinks {
    pub(super) state: ProxyState,
}

impl LanLinks {
    /// Starts a listener on `ip` (this device's address on the TV's network) for one cast.
    pub async fn open(&self, ip: IpAddr) -> std::io::Result<LanRoute> {
        let listener = tokio::net::TcpListener::bind((ip, 0)).await?;
        let address = listener.local_addr()?;
        let shared = Arc::new(LanShared {
            token: format!(
                "{}{}",
                uuid::Uuid::new_v4().simple(),
                uuid::Uuid::new_v4().simple()
            ),
            address,
            proxy: self.state.clone(),
            subtitles: Mutex::new(Vec::new()),
            caption: Mutex::new(None),
        });
        let router = Router::new().fallback(serve_lan).with_state(shared.clone());
        let (stop, stopped) = tokio::sync::oneshot::channel::<()>();
        tokio::spawn(async move {
            let served = axum::serve(
                listener,
                router.into_make_service_with_connect_info::<SocketAddr>(),
            )
            .with_graceful_shutdown(async {
                let _ = stopped.await;
            })
            .await;
            if let Err(error) = served {
                tracing::warn!(%error, "the cast route stopped");
            }
        });
        tracing::info!(%address, "cast route listening");
        Ok(LanRoute {
            shared,
            stop: Some(stop),
        })
    }
}

/// One cast's listener on the local network; it closes when dropped.
pub struct LanRoute {
    shared: Arc<LanShared>,
    stop: Option<tokio::sync::oneshot::Sender<()>>,
}

impl Drop for LanRoute {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl LanRoute {
    pub fn address(&self) -> SocketAddr {
        self.shared.address
    }

    fn base(&self) -> String {
        format!("http://{}/cast/{}/", self.shared.address, self.shared.token)
    }

    /// The loopback proxy's path for one of its own links, if `url` is one.
    fn loopback_path<'a>(&self, url: &'a str) -> Option<&'a str> {
        url.strip_prefix(&format!("http://{}/", self.shared.proxy.address))
    }

    /// The TV's address for a video: a link of the loopback proxy (a download, a file on this
    /// device) on the route, or a remote stream fetched with `headers`. `None` for anything else.
    pub fn link(&self, url: &str, headers: &BTreeMap<String, String>) -> Option<String> {
        if let Some(path) = self.loopback_path(url) {
            let known = ["stream/", "local/", "media/", "torrent/"]
                .iter()
                .any(|prefix| path.starts_with(prefix));
            return known.then(|| format!("{}{path}", self.base()));
        }
        let remote = reqwest::Url::parse(url).ok()?;
        if !matches!(remote.scheme(), "http" | "https") || remote.host_str().is_none() {
            return None;
        }
        let id = uuid::Uuid::new_v4().simple().to_string();
        {
            let mut media = self
                .shared
                .proxy
                .media
                .lock()
                .expect("media registry poisoned");
            media.headers.insert(id.clone(), headers.clone());
            media.order.push_back(id.clone());
            while media.order.len() > MAX_REGISTERED {
                if let Some(expired) = media.order.pop_front() {
                    media.headers.remove(&expired);
                }
            }
        }
        Some(format!(
            "{}media/{id}/{}",
            self.base(),
            manifest::media_path(&remote)
        ))
    }

    /// The TV's address for a subtitle file in `format`: a remote file fetched with `headers`, or
    /// a subtitle or local link of the loopback proxy. `None` for anything else.
    pub fn subtitle(
        &self,
        url: &str,
        headers: &BTreeMap<String, String>,
        format: SubtitleFormat,
    ) -> Option<String> {
        let file = match self.loopback_path(url) {
            Some(path) => {
                if let Some(token) = path.strip_prefix("subtitle/") {
                    let images = self
                        .shared
                        .proxy
                        .images
                        .lock()
                        .expect("image registry poisoned");
                    let (url, headers) = images.images.get(token)?.clone();
                    SubtitleFile::Remote { url, headers }
                } else {
                    let relative = path.strip_prefix("local/")?;
                    let root = self.shared.proxy.local.as_deref()?;
                    SubtitleFile::Local(local_path(root, relative)?)
                }
            }
            None if url.starts_with("https://") || url.starts_with("http://") => {
                SubtitleFile::Remote {
                    url: url.to_owned(),
                    headers: headers.clone(),
                }
            }
            None => return None,
        };
        let mut files = self.shared.subtitles.lock().expect("subtitles poisoned");
        files.push(file);
        let extension = match format {
            SubtitleFormat::WebVtt => "vtt",
            SubtitleFormat::SubRip => "srt",
        };
        Some(format!(
            "{}subtitle/{}.{extension}",
            self.base(),
            files.len() - 1
        ))
    }

    /// Names the subtitle file a DLNA TV is pointed to in its media responses (Samsung's
    /// `CaptionInfo.sec`).
    pub fn set_caption(&self, url: Option<String>) {
        *self.shared.caption.lock().expect("caption poisoned") = url;
    }
}

/// Whether `given` is the route's token, compared in constant time.
fn same_token(expected: &str, given: &str) -> bool {
    expected.len() == given.len()
        && expected
            .bytes()
            .zip(given.bytes())
            .fold(0u8, |difference, (a, b)| difference | (a ^ b))
            == 0
}

/// What follows `/cast/<token>/` in `path` when the token is the route's.
fn behind_token<'a>(expected: &str, path: &'a str) -> Option<&'a str> {
    let rest = path.strip_prefix("/cast/")?;
    let (token, rest) = rest.split_once('/')?;
    same_token(expected, token).then_some(rest)
}

async fn serve_lan(State(lan): State<Arc<LanShared>>, request: Request) -> Response {
    // Only this device and devices on its network: a forwarded port mustn't open it further.
    let peer = request
        .extensions()
        .get::<ConnectInfo<SocketAddr>>()
        .map(|ConnectInfo(peer)| peer.ip());
    if peer.is_some_and(|ip| !ip.is_loopback() && !crate::local_network::is_local_ip(ip)) {
        return StatusCode::FORBIDDEN.into_response();
    }
    let path = request.uri().path().to_owned();
    // Anything without the token is as if it weren't there.
    let Some(rest) = behind_token(&lan.token, &path) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let (kind, rest) = rest.split_once('/').unwrap_or((rest, ""));
    let response = match kind {
        "media" => with_dlna(&lan, lan_media(&lan, rest, request).await),
        "stream" => with_dlna(&lan, serve_registered(&lan.proxy, rest, request).await),
        "local" => match lan
            .proxy
            .local
            .as_deref()
            .and_then(|root| local_path(root, rest))
        {
            Some(file) => with_dlna(
                &lan,
                super::serve_file(&file.to_string_lossy(), request).await,
            ),
            None => StatusCode::NOT_FOUND.into_response(),
        },
        "subtitle" => lan_subtitle(&lan, rest).await,
        "torrent" => with_dlna(
            &lan,
            super::torrent::serve(&lan.proxy.torrents, rest, request).await,
        ),
        _ => StatusCode::NOT_FOUND.into_response(),
    };
    with_cors(response)
}

async fn lan_media(lan: &LanShared, rest: &str, request: Request) -> Response {
    let Some((id, rest)) = rest.split_once('/') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let headers = lan
        .proxy
        .media
        .lock()
        .expect("media registry poisoned")
        .headers
        .get(id)
        .cloned();
    let Some(headers) = headers else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(upstream) = manifest::upstream_url(rest, request.uri().query()) else {
        return StatusCode::BAD_REQUEST.into_response();
    };
    let link_base = format!("http://{}/cast/{}/media/{id}/", lan.address, lan.token);
    forward_media(
        &lan.proxy,
        &link_base,
        &upstream,
        &headers,
        request.headers(),
    )
    .await
}

async fn lan_subtitle(lan: &LanShared, name: &str) -> Response {
    let Some((index, extension)) = name.split_once('.') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let file = index.parse::<usize>().ok().and_then(|index| {
        lan.subtitles
            .lock()
            .expect("subtitles poisoned")
            .get(index)
            .cloned()
    });
    let (Some(file), Some(format)) = (
        file,
        match extension {
            "vtt" => Some(SubtitleFormat::WebVtt),
            "srt" => Some(SubtitleFormat::SubRip),
            _ => None,
        },
    ) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let bytes = match file {
        SubtitleFile::Remote { url, headers } => {
            fetch_subtitle(&lan.proxy.client, &url, &headers).await
        }
        SubtitleFile::Local(path) => match tokio::fs::read(&path).await {
            Ok(bytes) if bytes.len() <= MAX_SUBTITLE_BYTES => Ok(bytes),
            Ok(_) => Err("the file is too large".to_owned()),
            Err(error) => Err(error.to_string()),
        },
    };
    let text = match bytes {
        Ok(bytes) => String::from_utf8_lossy(&bytes).into_owned(),
        Err(error) => {
            // An empty track rather than a failure: a TV may give up on the video for it.
            tracing::warn!(%error, "subtitle file for the TV failed");
            String::new()
        }
    };
    let cues = subtitles::parse(&text);
    let (body, kind) = match format {
        SubtitleFormat::WebVtt => (subtitles::to_vtt(&cues), "text/vtt; charset=utf-8"),
        SubtitleFormat::SubRip => (
            subtitles::to_srt(&cues),
            "application/x-subrip; charset=utf-8",
        ),
    };
    (
        [
            (header::CONTENT_TYPE, kind),
            (header::CACHE_CONTROL, "no-store"),
        ],
        body,
    )
        .into_response()
}

/// The headers DLNA TVs look for on a stream: it streams, it seeks, and where its subtitles are.
fn with_dlna(lan: &LanShared, mut response: Response) -> Response {
    let headers = response.headers_mut();
    headers.insert(
        "transferMode.dlna.org",
        HeaderValue::from_static("Streaming"),
    );
    headers.insert(
        "contentFeatures.dlna.org",
        HeaderValue::from_static(DLNA_FEATURES),
    );
    if let Some(caption) = lan.caption.lock().expect("caption poisoned").as_deref()
        && let Ok(value) = HeaderValue::from_str(caption)
    {
        headers.insert("CaptionInfo.sec", value);
    }
    response
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_the_routes_token_opens_it() {
        let token = "a".repeat(64);
        assert_eq!(
            behind_token(&token, &format!("/cast/{token}/media/x/https/host/a.m3u8")),
            Some("media/x/https/host/a.m3u8")
        );
        assert_eq!(
            behind_token(&token, &format!("/cast/{}/local/a", "b".repeat(64))),
            None
        );
        assert_eq!(behind_token(&token, "/cast/aaaa/local/a"), None);
        assert_eq!(behind_token(&token, &format!("/cast/{token}")), None);
        assert_eq!(behind_token(&token, &format!("/media/{token}/x")), None);
        assert!(same_token("abc", "abc"));
        assert!(!same_token("abc", "abd"));
        assert!(!same_token("abc", "abcd"));
    }
}
