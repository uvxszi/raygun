//! A lasting disk cache for the images people see again and again (M46.2): covers, posters,
//! thumbnails and extension icons, served by the proxy at `/cover/<token>`.
//!
//! It is kept apart from the reader's page cache, so reading a long chapter never pushes covers
//! out. Images open from disk at once; once a copy is older than the refresh age it is checked
//! again in the background (`If-None-Match` / `If-Modified-Since`, as Coil does for Mihon), so a
//! cover a source changed in place shows its new version next time. A cover whose address changes
//! gets a new token and is fetched fresh.
//!
//! Covers of titles in the library are "kept": they live in `keep/` and are never evicted. The
//! rest are trimmed least recently used first once the folder passes its size limit.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use reqwest::header;

use crate::page_cache::{MAX_IMAGE_BYTES, evict, sniff_content_type, touch};

const KEEP: &str = "keep";
const META: &str = "meta";

/// When an image was last checked and what the server said about it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Meta {
    checked_ms: u64,
    etag: Option<String>,
    last_modified: Option<String>,
}

impl Meta {
    fn encode(&self) -> String {
        format!(
            "{}\n{}\n{}\n",
            self.checked_ms,
            self.etag.as_deref().unwrap_or_default(),
            self.last_modified.as_deref().unwrap_or_default()
        )
    }

    fn decode(text: &str) -> Self {
        let mut lines = text.lines();
        let checked_ms = lines.next().and_then(|line| line.parse().ok()).unwrap_or(0);
        let field = |line: Option<&str>| line.filter(|line| !line.is_empty()).map(str::to_owned);
        Self {
            checked_ms,
            etag: field(lines.next()),
            last_modified: field(lines.next()),
        }
    }
}

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as u64)
        .unwrap_or(0)
}

/// What a fetch of an image brought back.
pub enum Fetched {
    Image {
        bytes: Vec<u8>,
        etag: Option<String>,
        last_modified: Option<String>,
    },
    /// 304: the copy is still current.
    Unchanged,
}

/// Fetches an image, conditionally when validators are known.
async fn fetch(
    client: &reqwest::Client,
    url: &str,
    headers: &BTreeMap<String, String>,
    known: Option<&Meta>,
) -> Result<Fetched, String> {
    // A connected server's covers sign in (`server_auth`).
    let mut request = crate::server_auth::get(client, url, headers);
    if let Some(meta) = known {
        if let Some(etag) = &meta.etag {
            request = request.header(header::IF_NONE_MATCH, etag);
        }
        if let Some(modified) = &meta.last_modified {
            request = request.header(header::IF_MODIFIED_SINCE, modified);
        }
    }
    let response = request.send().await.map_err(|error| error.to_string())?;
    let status = response.status();
    if status == reqwest::StatusCode::NOT_MODIFIED {
        return Ok(Fetched::Unchanged);
    }
    if !status.is_success() {
        return Err(format!("HTTP {}", status.as_u16()));
    }
    let header_text = |name: header::HeaderName| {
        response
            .headers()
            .get(name)
            .and_then(|value| value.to_str().ok())
            .map(str::to_owned)
    };
    let etag = header_text(header::ETAG);
    let last_modified = header_text(header::LAST_MODIFIED);
    let declared = header_text(header::CONTENT_TYPE).unwrap_or_default();
    if response
        .content_length()
        .is_some_and(|length| length > MAX_IMAGE_BYTES as u64)
    {
        return Err("the image is too large".into());
    }
    let bytes = response.bytes().await.map_err(|error| error.to_string())?;
    if bytes.len() > MAX_IMAGE_BYTES {
        return Err("the image is too large".into());
    }
    let sniffed = sniff_content_type(&bytes);
    if !sniffed.starts_with("image/") && !declared.starts_with("image/") {
        return Err("the server didn't send an image".into());
    }
    Ok(Fetched::Image {
        bytes: bytes.to_vec(),
        etag,
        last_modified,
    })
}

/// Sizes of a cache on disk.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Usage {
    pub bytes: u64,
    pub files: u64,
}

#[derive(Clone)]
pub struct CoverCache {
    dir: PathBuf,
    max_bytes: Arc<AtomicU64>,
    /// Copies older than this are checked again; 0 never checks.
    refresh_ms: Arc<AtomicU64>,
    written: Arc<AtomicU64>,
    /// Checks running, so one image is checked once at a time.
    checking: Arc<Mutex<HashSet<String>>>,
}

impl CoverCache {
    pub fn new(dir: PathBuf, max_bytes: u64, refresh: Duration) -> Self {
        Self {
            dir,
            max_bytes: Arc::new(AtomicU64::new(max_bytes)),
            refresh_ms: Arc::new(AtomicU64::new(refresh.as_millis() as u64)),
            written: Arc::new(AtomicU64::new(u64::MAX / 2)),
            checking: Arc::new(Mutex::new(HashSet::new())),
        }
    }

    /// Changes the limits (from settings) and trims to the new size.
    pub fn set_limits(&self, max_bytes: u64, refresh: Duration) {
        self.max_bytes.store(max_bytes, Ordering::Relaxed);
        self.refresh_ms
            .store(refresh.as_millis() as u64, Ordering::Relaxed);
        self.written.store(u64::MAX / 2, Ordering::Relaxed);
    }

    fn valid(token: &str) -> bool {
        !token.is_empty() && token.bytes().all(|byte| byte.is_ascii_hexdigit())
    }

    fn file(&self, token: &str, kept: bool) -> PathBuf {
        if kept {
            self.dir.join(KEEP).join(token)
        } else {
            self.dir.join(token)
        }
    }

    fn meta_file(&self, token: &str) -> PathBuf {
        self.dir.join(META).join(token)
    }

    /// Moves a copy to where it belongs now (a title joined or left the library).
    async fn settle(&self, token: &str, kept: bool) -> Option<PathBuf> {
        let wanted = self.file(token, kept);
        if tokio::fs::try_exists(&wanted).await.unwrap_or(false) {
            return Some(wanted);
        }
        let other = self.file(token, !kept);
        if tokio::fs::try_exists(&other).await.unwrap_or(false) {
            if let Some(parent) = wanted.parent() {
                let _ = tokio::fs::create_dir_all(parent).await;
            }
            if tokio::fs::rename(&other, &wanted).await.is_ok() {
                return Some(wanted);
            }
            return Some(other);
        }
        None
    }

    async fn read_meta(&self, token: &str) -> Meta {
        tokio::fs::read_to_string(self.meta_file(token))
            .await
            .map(|text| Meta::decode(&text))
            .unwrap_or_default()
    }

    async fn store(&self, token: &str, kept: bool, bytes: &[u8], meta: &Meta) {
        let path = self.file(token, kept);
        let result = async {
            let parent = path.parent().expect("cache files have a folder");
            tokio::fs::create_dir_all(parent).await?;
            tokio::fs::create_dir_all(self.dir.join(META)).await?;
            let temporary = parent.join(format!("{token}.part"));
            tokio::fs::write(&temporary, bytes).await?;
            tokio::fs::rename(&temporary, &path).await?;
            tokio::fs::write(self.meta_file(token), meta.encode()).await
        }
        .await;
        if let Err(error) = result {
            tracing::debug!(%error, "cover cache write failed");
            return;
        }
        // Covers that aren't kept are trimmed once enough was written.
        let written = self
            .written
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        let max = self.max_bytes.load(Ordering::Relaxed);
        if written + bytes.len() as u64 > max / 10 {
            self.written.store(0, Ordering::Relaxed);
            let dir = self.dir.clone();
            let _ = tokio::task::spawn_blocking(move || {
                // The size limit covers kept covers too; only the others go.
                let kept = usage_of(&dir.join(KEEP)).bytes;
                evict(&dir, max.saturating_sub(kept));
                clean_meta(&dir);
            })
            .await;
        }
    }

    /// The copy on disk for `token`, kept or not, without fetching or checking it: for a link
    /// whose address this run doesn't know (one the UI saved from an earlier start, M46.47).
    pub async fn stored(&self, token: &str) -> Option<Vec<u8>> {
        if !Self::valid(token) {
            return None;
        }
        for kept in [true, false] {
            let path = self.file(token, kept);
            if let Ok(bytes) = tokio::fs::read(&path).await {
                let _ = tokio::task::spawn_blocking(move || touch(&path)).await;
                return Some(bytes);
            }
        }
        None
    }

    /// The image for `token`: from disk (checked again in the background when old), or fetched.
    pub async fn get(
        &self,
        client: &reqwest::Client,
        token: &str,
        url: &str,
        headers: &BTreeMap<String, String>,
        kept: bool,
    ) -> Result<Vec<u8>, String> {
        if !Self::valid(token) {
            return Err("invalid token".into());
        }
        if let Some(path) = self.settle(token, kept).await
            && let Ok(bytes) = tokio::fs::read(&path).await
        {
            let _ = tokio::task::spawn_blocking(move || touch(&path)).await;
            let refresh = self.refresh_ms.load(Ordering::Relaxed);
            if refresh > 0 {
                let meta = self.read_meta(token).await;
                if now_ms().saturating_sub(meta.checked_ms) > refresh {
                    self.check_later(client, token, url, headers, kept, meta);
                }
            }
            return Ok(bytes);
        }
        match fetch(client, url, headers, None).await? {
            Fetched::Image {
                bytes,
                etag,
                last_modified,
            } => {
                let meta = Meta {
                    checked_ms: now_ms(),
                    etag,
                    last_modified,
                };
                self.store(token, kept, &bytes, &meta).await;
                Ok(bytes)
            }
            Fetched::Unchanged => Err("the server answered 304 to a plain request".into()),
        }
    }

    /// Checks an old copy in the background; the next view gets a changed image.
    fn check_later(
        &self,
        client: &reqwest::Client,
        token: &str,
        url: &str,
        headers: &BTreeMap<String, String>,
        kept: bool,
        meta: Meta,
    ) {
        if !self
            .checking
            .lock()
            .expect("cover checks poisoned")
            .insert(token.to_owned())
        {
            return;
        }
        let (cache, client) = (self.clone(), client.clone());
        let (token, url, headers) = (token.to_owned(), url.to_owned(), headers.clone());
        tokio::spawn(async move {
            match fetch(&client, &url, &headers, Some(&meta)).await {
                Ok(Fetched::Unchanged) => {
                    let checked = Meta {
                        checked_ms: now_ms(),
                        ..meta
                    };
                    let _ = tokio::fs::create_dir_all(cache.dir.join(META)).await;
                    let _ = tokio::fs::write(cache.meta_file(&token), checked.encode()).await;
                }
                Ok(Fetched::Image {
                    bytes,
                    etag,
                    last_modified,
                }) => {
                    let fresh = Meta {
                        checked_ms: now_ms(),
                        etag,
                        last_modified,
                    };
                    cache.store(&token, kept, &bytes, &fresh).await;
                }
                // Offline or the host is down: keep the copy and try again next time.
                Err(error) => tracing::debug!(%error, "cover check failed"),
            }
            cache
                .checking
                .lock()
                .expect("cover checks poisoned")
                .remove(&token);
        });
    }

    /// Drops one cached copy, so it's fetched again when next seen.
    pub async fn forget(&self, token: &str) {
        if !Self::valid(token) {
            return;
        }
        for kept in [false, true] {
            let _ = tokio::fs::remove_file(self.file(token, kept)).await;
        }
        let _ = tokio::fs::remove_file(self.meta_file(token)).await;
    }

    /// Puts every cached cover where it belongs: those in `kept` (the library's) into `keep/`,
    /// the rest back among those evicted normally. Returns how many moved in and out.
    pub async fn sync_kept(&self, kept: HashSet<String>) -> (u64, u64) {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || {
            let keep = dir.join(KEEP);
            let mut moved_out = 0;
            if let Ok(entries) = std::fs::read_dir(&keep) {
                for entry in entries.filter_map(Result::ok) {
                    let name = entry.file_name().to_string_lossy().into_owned();
                    if !kept.contains(&name)
                        && std::fs::rename(entry.path(), dir.join(&name)).is_ok()
                    {
                        moved_out += 1;
                    }
                }
            }
            let mut moved_in = 0;
            for token in &kept {
                let loose = dir.join(token);
                if CoverCache::valid(token) && loose.is_file() {
                    let _ = std::fs::create_dir_all(&keep);
                    if std::fs::rename(&loose, keep.join(token)).is_ok() {
                        moved_in += 1;
                    }
                }
            }
            (moved_in, moved_out)
        })
        .await
        .unwrap_or((0, 0))
    }

    pub async fn usage(&self) -> Usage {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || {
            let top = usage_of(&dir);
            let kept = usage_of(&dir.join(KEEP));
            Usage {
                bytes: top.bytes + kept.bytes,
                files: top.files + kept.files,
            }
        })
        .await
        .unwrap_or_default()
    }

    /// Deletes every cached image; they're fetched again when next seen.
    pub async fn clear(&self) {
        let _ = tokio::fs::remove_dir_all(&self.dir).await;
        self.written.store(u64::MAX / 2, Ordering::Relaxed);
    }
}

/// Bytes and files directly in `dir`.
pub fn usage_of(dir: &Path) -> Usage {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Usage::default();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.metadata().ok())
        .filter(|metadata| metadata.is_file())
        .fold(Usage::default(), |usage, metadata| Usage {
            bytes: usage.bytes + metadata.len(),
            files: usage.files + 1,
        })
}

/// Removes check records whose image is gone.
fn clean_meta(dir: &Path) {
    let Ok(entries) = std::fs::read_dir(dir.join(META)) else {
        return;
    };
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name();
        if !dir.join(&name).exists() && !dir.join(KEEP).join(&name).exists() {
            let _ = std::fs::remove_file(entry.path());
        }
    }
}

#[cfg(test)]
mod tests {
    use std::net::{Ipv4Addr, SocketAddr};
    use std::sync::atomic::AtomicUsize;

    use axum::Router;
    use axum::http::HeaderMap;
    use axum::response::{IntoResponse, Response};
    use axum::routing::get;

    use super::*;

    const PNG_A: &[u8] = b"\x89PNG\r\n\x1a\nfirst";
    const PNG_B: &[u8] = b"\x89PNG\r\n\x1a\nsecond";
    static VERSION: AtomicUsize = AtomicUsize::new(0);
    static REQUESTS: AtomicUsize = AtomicUsize::new(0);

    async fn origin() -> SocketAddr {
        // Version 0 has ETag "a"; after a bump it serves "b". A matching If-None-Match gets 304.
        async fn cover(headers: HeaderMap) -> Response {
            REQUESTS.fetch_add(1, Ordering::SeqCst);
            let (etag, body) = if VERSION.load(Ordering::SeqCst) == 0 {
                ("\"a\"", PNG_A)
            } else {
                ("\"b\"", PNG_B)
            };
            if headers
                .get(header::IF_NONE_MATCH)
                .is_some_and(|value| value == etag)
            {
                return axum::http::StatusCode::NOT_MODIFIED.into_response();
            }
            (
                [(header::CONTENT_TYPE, "image/png"), (header::ETAG, etag)],
                body,
            )
                .into_response()
        }
        let listener = tokio::net::TcpListener::bind((Ipv4Addr::LOCALHOST, 0))
            .await
            .unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move {
            axum::serve(listener, Router::new().route("/cover.png", get(cover)))
                .await
                .unwrap()
        });
        address
    }

    async fn settle_checks(cache: &CoverCache) {
        for _ in 0..100 {
            if cache.checking.lock().unwrap().is_empty() {
                return;
            }
            tokio::time::sleep(Duration::from_millis(10)).await;
        }
    }

    #[tokio::test]
    async fn covers_come_from_disk_and_are_checked_again_when_old() {
        let origin = origin().await;
        let temp = tempfile::TempDir::new().unwrap();
        let cache = CoverCache::new(temp.path().join("covers"), 1 << 20, Duration::ZERO);
        let client = crate::http_client().unwrap();
        let url = format!("http://{origin}/cover.png");
        let headers = BTreeMap::new();

        assert_eq!(
            cache
                .get(&client, "ab", &url, &headers, false)
                .await
                .unwrap(),
            PNG_A
        );
        assert_eq!(REQUESTS.load(Ordering::SeqCst), 1);
        // Refresh off: the copy is used as is.
        assert_eq!(
            cache
                .get(&client, "ab", &url, &headers, false)
                .await
                .unwrap(),
            PNG_A
        );
        assert_eq!(REQUESTS.load(Ordering::SeqCst), 1);

        // Refresh on (everything is old): unchanged answers 304 and keeps the copy.
        cache.set_limits(1 << 20, Duration::from_millis(1));
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(
            cache
                .get(&client, "ab", &url, &headers, false)
                .await
                .unwrap(),
            PNG_A
        );
        settle_checks(&cache).await;
        assert_eq!(REQUESTS.load(Ordering::SeqCst), 2);

        // The source changed the image in place: the old copy shows once, then the new one.
        VERSION.store(1, Ordering::SeqCst);
        tokio::time::sleep(Duration::from_millis(5)).await;
        assert_eq!(
            cache
                .get(&client, "ab", &url, &headers, false)
                .await
                .unwrap(),
            PNG_A
        );
        settle_checks(&cache).await;
        cache.set_limits(1 << 20, Duration::ZERO);
        assert_eq!(
            cache
                .get(&client, "ab", &url, &headers, false)
                .await
                .unwrap(),
            PNG_B
        );

        // Kept covers live apart and move back when no longer kept.
        assert_eq!(
            cache
                .get(&client, "ab", &url, &headers, true)
                .await
                .unwrap(),
            PNG_B
        );
        assert!(temp.path().join("covers/keep/ab").exists());
        assert!(!temp.path().join("covers/ab").exists());
        assert_eq!(cache.sync_kept(HashSet::new()).await, (0, 1));
        assert!(temp.path().join("covers/ab").exists());
        // The library's covers move into keep/ at the sweep, even before they're seen again.
        assert_eq!(
            cache.sync_kept(HashSet::from(["ab".to_owned()])).await,
            (1, 0)
        );
        assert!(temp.path().join("covers/keep/ab").exists());
        assert_eq!(cache.sync_kept(HashSet::new()).await, (0, 1));

        let usage = cache.usage().await;
        assert_eq!((usage.files, usage.bytes), (1, PNG_B.len() as u64));
        cache.clear().await;
        assert_eq!(cache.usage().await, Usage::default());
        assert!(
            cache
                .get(&client, "../x", &url, &headers, false)
                .await
                .is_err()
        );
    }

    #[test]
    fn check_records_round_trip() {
        let meta = Meta {
            checked_ms: 42,
            etag: Some("\"x\"".into()),
            last_modified: None,
        };
        assert_eq!(Meta::decode(&meta.encode()), meta);
        assert_eq!(Meta::decode(""), Meta::default());
    }
}
