//! Files that arrive while they're read: a torrent's video, served at
//! `/torrent/<token>/<name>` with byte ranges, so players open and seek in it as in any file.
//!
//! The proxy only answers HTTP; what holds the bytes ([`ArrivingFiles`], the app's torrents) is
//! set once it exists ([`TorrentLinks::serve`]). A read waits for the pieces it needs: the
//! reader fetches the ones around it first.

use std::net::SocketAddr;
use std::pin::Pin;
use std::sync::{Arc, RwLock};

use axum::body::{Body, Bytes};
use axum::extract::Request;
use axum::http::{StatusCode, header};
use axum::response::{IntoResponse, Response};
use futures_util::future::BoxFuture;
use tokio::io::{AsyncRead, AsyncReadExt};

/// How much one read of the body asks for.
const CHUNK: usize = 64 * 1024;

/// A reader of a file from some place in it.
pub type FileReader = Pin<Box<dyn AsyncRead + Send>>;

/// What `/torrent/` serves: files whose bytes come as they're read.
pub trait ArrivingFiles: Send + Sync + 'static {
    /// The length of the file behind `token`, while it's open.
    fn file_len(&self, token: &str) -> Option<u64>;
    /// A reader of that file from `start`.
    fn read_from(&self, token: &str, start: u64)
    -> BoxFuture<'static, std::io::Result<FileReader>>;
}

pub(crate) type Shared = Arc<RwLock<Option<Arc<dyn ArrivingFiles>>>>;

/// Hands out `/torrent/` addresses, and takes what serves them.
#[derive(Clone)]
pub struct TorrentLinks {
    address: SocketAddr,
    files: Shared,
}

impl TorrentLinks {
    pub(crate) fn new(address: SocketAddr, files: Shared) -> Self {
        Self { address, files }
    }

    /// What serves the route from now on.
    pub fn serve(&self, files: Arc<dyn ArrivingFiles>) {
        *self.files.write().unwrap_or_else(|p| p.into_inner()) = Some(files);
    }

    /// The address of the file behind `token`; `name` (its file name) only tells players its
    /// kind.
    pub fn link(&self, token: &str, name: &str) -> String {
        format!(
            "http://{}/torrent/{token}/{}",
            self.address,
            path_name(name)
        )
    }
}

/// A file name that stays one path segment: anything but letters, digits, `.`, `-` and `_`
/// becomes `_`.
fn path_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|character| {
            if character.is_ascii_alphanumeric() || matches!(character, '.' | '-' | '_') {
                character
            } else {
                '_'
            }
        })
        .collect();
    let name = name.trim_start_matches('.');
    if name.is_empty() {
        "video".to_owned()
    } else {
        name.to_owned()
    }
}

pub(crate) fn shared() -> Shared {
    Arc::new(RwLock::new(None))
}

/// What a `Range` header asks of a file of `len` bytes: `Ok(None)` for all of it, the first
/// range's first and last byte otherwise, `Err(())` when nothing of it is in the file.
pub(crate) fn byte_range(range: Option<&str>, len: u64) -> Result<Option<(u64, u64)>, ()> {
    let Some(range) = range else {
        return Ok(None);
    };
    let Some(ranges) = range.trim().strip_prefix("bytes=") else {
        // Another unit: the whole file, as if no range was asked.
        return Ok(None);
    };
    // Several ranges: the first one is answered.
    let first = ranges.split(',').next().unwrap_or_default().trim();
    let Some((start, end)) = first.split_once('-') else {
        return Ok(None);
    };
    let (start, end) = (start.trim(), end.trim());
    if start.is_empty() {
        // The last `end` bytes.
        let suffix: u64 = end.parse().map_err(|_| ())?;
        if suffix == 0 || len == 0 {
            return Err(());
        }
        return Ok(Some((len.saturating_sub(suffix), len - 1)));
    }
    let start: u64 = start.parse().map_err(|_| ())?;
    if start >= len {
        return Err(());
    }
    let end = if end.is_empty() {
        len - 1
    } else {
        let end: u64 = end.parse().map_err(|_| ())?;
        if end < start {
            return Err(());
        }
        end.min(len - 1)
    };
    Ok(Some((start, end)))
}

/// The type players are told, by the file name's extension.
pub(crate) fn content_type(name: &str) -> &'static str {
    let extension = name
        .rsplit_once('.')
        .map(|(_, extension)| extension.to_ascii_lowercase())
        .unwrap_or_default();
    match extension.as_str() {
        "mp4" | "m4v" => "video/mp4",
        "mkv" => "video/x-matroska",
        "webm" => "video/webm",
        "avi" => "video/x-msvideo",
        "mov" => "video/quicktime",
        "ts" | "m2ts" | "mts" => "video/mp2t",
        "mpg" | "mpeg" => "video/mpeg",
        "wmv" => "video/x-ms-wmv",
        "flv" => "video/x-flv",
        "ogv" => "video/ogg",
        "3gp" => "video/3gpp",
        "mp3" => "audio/mpeg",
        "m4a" => "audio/mp4",
        "flac" => "audio/flac",
        "ogg" | "opus" => "audio/ogg",
        _ => "application/octet-stream",
    }
}

/// The body: `remaining` bytes of `reader`, a chunk at a time.
fn body_of(reader: FileReader, remaining: u64) -> Body {
    let stream =
        futures_util::stream::unfold((reader, remaining), |(mut reader, remaining)| async move {
            if remaining == 0 {
                return None;
            }
            let want = usize::try_from(remaining).unwrap_or(usize::MAX).min(CHUNK);
            let mut buffer = vec![0u8; want];
            match reader.read(&mut buffer).await {
                // The file ended early: the body stops here.
                Ok(0) => None,
                Ok(read) => {
                    buffer.truncate(read);
                    Some((
                        Ok::<_, std::io::Error>(Bytes::from(buffer)),
                        (reader, remaining - read as u64),
                    ))
                }
                Err(error) => Some((Err(error), (reader, 0))),
            }
        });
    Body::from_stream(stream)
}

/// Answers a request for `<token>/<name>` from `files`.
pub(crate) async fn serve(files: &Shared, rest: &str, request: Request) -> Response {
    let Some((token, name)) = rest.split_once('/') else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let files = files.read().unwrap_or_else(|p| p.into_inner()).clone();
    let Some(files) = files else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let Some(len) = files.file_len(token) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let range = request
        .headers()
        .get(header::RANGE)
        .and_then(|value| value.to_str().ok());
    let (status, start, end) = match byte_range(range, len) {
        Ok(None) => (StatusCode::OK, 0, len.saturating_sub(1)),
        Ok(Some((start, end))) => (StatusCode::PARTIAL_CONTENT, start, end),
        Err(()) => {
            return (
                StatusCode::RANGE_NOT_SATISFIABLE,
                [(header::CONTENT_RANGE, format!("bytes */{len}"))],
            )
                .into_response();
        }
    };
    let length = if len == 0 { 0 } else { end - start + 1 };
    let mut response = Response::builder()
        .status(status)
        .header(header::CONTENT_TYPE, content_type(name))
        .header(header::ACCEPT_RANGES, "bytes")
        .header(header::CONTENT_LENGTH, length)
        .header(header::CACHE_CONTROL, "no-store");
    if status == StatusCode::PARTIAL_CONTENT {
        response = response.header(header::CONTENT_RANGE, format!("bytes {start}-{end}/{len}"));
    }
    if request.method() == axum::http::Method::HEAD || length == 0 {
        return response
            .body(Body::empty())
            .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response());
    }
    let reader = match files.read_from(token, start).await {
        Ok(reader) => reader,
        Err(error) => {
            tracing::debug!(%error, "a torrent file couldn't be read");
            return (StatusCode::BAD_GATEWAY, error.to_string()).into_response();
        }
    };
    response
        .body(body_of(reader, length))
        .unwrap_or_else(|_| StatusCode::INTERNAL_SERVER_ERROR.into_response())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ranges_are_read_as_players_send_them() {
        assert_eq!(byte_range(None, 100), Ok(None));
        assert_eq!(byte_range(Some("bytes=0-"), 100), Ok(Some((0, 99))));
        assert_eq!(byte_range(Some("bytes=10-19"), 100), Ok(Some((10, 19))));
        // Past the end is cut to the end; the last bytes by a suffix.
        assert_eq!(byte_range(Some("bytes=90-200"), 100), Ok(Some((90, 99))));
        assert_eq!(byte_range(Some("bytes=-10"), 100), Ok(Some((90, 99))));
        assert_eq!(byte_range(Some("bytes=-500"), 100), Ok(Some((0, 99))));
        // Several ranges: the first.
        assert_eq!(byte_range(Some("bytes=5-6, 20-30"), 100), Ok(Some((5, 6))));
        // Nothing of it in the file.
        assert_eq!(byte_range(Some("bytes=100-"), 100), Err(()));
        assert_eq!(byte_range(Some("bytes=20-10"), 100), Err(()));
        assert_eq!(byte_range(Some("bytes=-0"), 100), Err(()));
        assert_eq!(byte_range(Some("bytes=x-"), 100), Err(()));
        // Another unit: the whole file.
        assert_eq!(byte_range(Some("items=0-1"), 100), Ok(None));
    }

    #[test]
    fn names_stay_one_segment_and_say_the_kind() {
        assert_eq!(
            path_name("Big Buck Bunny (2008).mp4"),
            "Big_Buck_Bunny__2008_.mp4"
        );
        assert_eq!(path_name("../../etc"), "_.._etc");
        assert_eq!(path_name(""), "video");
        assert_eq!(content_type("Sintel.2010.1080p.MKV"), "video/x-matroska");
        assert_eq!(content_type("film.mp4"), "video/mp4");
        assert_eq!(content_type("notes"), "application/octet-stream");
    }
}
