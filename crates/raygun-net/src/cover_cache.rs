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
//! rest are trimmed least recently used first once the folder passes its size limit, except the
//! covers in use (the playing and queued songs', [`CoverCache::spare_with`]).
//!
//! Copies are stored as JPEG XL ([`crate::cover_format`]): a cover is kept as it came and served
//! at once, then converted in the background, one at a time (resting while the app says so,
//! [`CoverCache::convert_when`]), and swapped in place; that file is the cache. Copies from before are converted when next seen, and each copy's check record says
//! how it's stored, so none is converted twice. A webview that shows JPEG XL (its probe said so,
//! [`CoverCache::set_webview_jxl`], or its request lists `image/jxl`) gets the file as it is;
//! other requests get a version made once and kept in memory ([`SHOWN_BYTES`]).

use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use futures_util::future::BoxFuture;
use reqwest::header;

use crate::cover_format::{self, Conversion, JXL_TYPE, Shown};
use crate::images::is_jxl;
use crate::page_cache::{MAX_IMAGE_BYTES, evict_sparing, sniff_content_type, touch};

/// Names the covers in use now, which trimming leaves.
type InUse = Arc<dyn Fn() -> HashSet<String> + Send + Sync>;
/// Returns once background work may run (the app's power and background state).
type Gate = Arc<dyn Fn() -> BoxFuture<'static, ()> + Send + Sync>;

const KEEP: &str = "keep";
const META: &str = "meta";
/// How long a cover's source has to send it: a host that never answers would otherwise hold one
/// of the webview's few connections to the proxy (six per host), and every row after it waits.
const FETCH_TIMEOUT: Duration = Duration::from_secs(15);
/// Versions made for requests that can't take JPEG XL are kept in memory up to this size.
const SHOWN_BYTES: usize = 6 * 1024 * 1024;

/// How many such versions are made at once: half the cores, 2 to 4, so a screen of covers
/// shows quickly without taking every core from the webview.
fn shown_at_once() -> usize {
    std::thread::available_parallelism().map_or(2, |cores| (cores.get() / 2).clamp(2, 4))
}

/// How a copy is stored.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
enum Stored {
    /// Not looked at yet: converted when next seen.
    #[default]
    Pending,
    /// Kept as it came: not a picture worth converting.
    Kept,
    /// JPEG XL; the original took this many bytes.
    Jxl(u64),
}

impl Stored {
    fn encode(self) -> String {
        match self {
            Self::Pending => String::new(),
            Self::Kept => "kept".into(),
            Self::Jxl(original) => format!("jxl {original}"),
        }
    }

    fn decode(line: Option<&str>) -> Self {
        match line.map(str::trim) {
            Some("kept") => Self::Kept,
            Some(line) => line
                .strip_prefix("jxl ")
                .and_then(|bytes| bytes.parse().ok())
                .map_or(Self::Pending, Self::Jxl),
            None => Self::Pending,
        }
    }
}

/// When an image was last checked and what the server said about it, and how it's stored.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
struct Meta {
    checked_ms: u64,
    etag: Option<String>,
    last_modified: Option<String>,
    stored: Stored,
}

impl Meta {
    fn encode(&self) -> String {
        format!(
            "{}\n{}\n{}\n{}\n",
            self.checked_ms,
            self.etag.as_deref().unwrap_or_default(),
            self.last_modified.as_deref().unwrap_or_default(),
            self.stored.encode()
        )
    }

    /// Records from before JPEG XL have three lines: their copies are converted when next seen.
    fn decode(text: &str) -> Self {
        let mut lines = text.lines();
        let checked_ms = lines.next().and_then(|line| line.parse().ok()).unwrap_or(0);
        let field = |line: Option<&str>| line.filter(|line| !line.is_empty()).map(str::to_owned);
        Self {
            checked_ms,
            etag: field(lines.next()),
            last_modified: field(lines.next()),
            stored: Stored::decode(lines.next()),
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
    timeout: Duration,
) -> Result<Fetched, String> {
    // A connected server's covers sign in (`server_auth`).
    let mut request = crate::server_auth::get(client, url, headers).timeout(timeout);
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
    if !sniffed.starts_with("image/") && !declared.starts_with("image/") && !is_jxl(&bytes) {
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

/// What storing covers as JPEG XL saves: the copies stored so, what they took as they came and
/// what they take now.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Saving {
    pub files: u64,
    pub original_bytes: u64,
    pub bytes: u64,
}

/// A cover as it goes to the webview.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Served {
    pub bytes: Vec<u8>,
    pub content_type: &'static str,
}

/// Copies waiting to be converted, in turn.
#[derive(Default)]
struct Queue {
    order: VecDeque<String>,
    queued: HashSet<String>,
    running: bool,
}

type Making = Arc<tokio::sync::OnceCell<Option<Arc<Shown>>>>;
/// A fetch of one cover that every request for it waits on.
type Fetching = Arc<tokio::sync::OnceCell<Result<Vec<u8>, String>>>;

/// Versions made for requests that can't take JPEG XL, least recently used out first, and those
/// being made, so each is made once.
#[derive(Default)]
struct Showing {
    kept: HashMap<String, (Arc<Shown>, u64)>,
    bytes: usize,
    tick: u64,
    making: HashMap<String, Making>,
}

impl Showing {
    fn get(&mut self, key: &str) -> Option<Arc<Shown>> {
        self.tick += 1;
        let tick = self.tick;
        self.kept.get_mut(key).map(|(shown, used)| {
            *used = tick;
            shown.clone()
        })
    }

    fn put(&mut self, key: String, shown: Arc<Shown>) {
        let size = shown.bytes.len();
        // One huge picture would push out everything else; it's made again when asked.
        if size > SHOWN_BYTES / 4 {
            return;
        }
        self.tick += 1;
        if let Some((old, _)) = self.kept.insert(key, (shown, self.tick)) {
            self.bytes -= old.bytes.len();
        }
        self.bytes += size;
        while self.bytes > SHOWN_BYTES {
            let oldest = self
                .kept
                .iter()
                .min_by_key(|(_, (_, used))| *used)
                .map(|(key, _)| key.clone());
            let Some((gone, _)) = oldest.and_then(|key| self.kept.remove(&key)) else {
                break;
            };
            self.bytes -= gone.bytes.len();
        }
    }

    fn forget(&mut self, token: &str) {
        let prefix = format!("{token}:");
        let gone: Vec<String> = self
            .kept
            .keys()
            .filter(|key| key.starts_with(&prefix))
            .cloned()
            .collect();
        for key in gone {
            if let Some((shown, _)) = self.kept.remove(&key) {
                self.bytes -= shown.bytes.len();
            }
        }
    }

    fn clear(&mut self) {
        self.kept.clear();
        self.bytes = 0;
    }
}

/// The JPEG XL side of the cache.
struct Jxl {
    /// Whether the webview shows JPEG XL, from its probe; until it says, it doesn't.
    webview: AtomicBool,
    queue: Mutex<Queue>,
    /// Holds writes, moves and swaps of copies apart, so a conversion never replaces a copy
    /// that changed while it ran.
    files: tokio::sync::Mutex<()>,
    showing: Mutex<Showing>,
    slots: tokio::sync::Semaphore,
    /// What the worker waits for before each conversion ([`CoverCache::convert_when`]).
    gate: Mutex<Option<Gate>>,
    /// Conversions run, for tests.
    #[cfg(test)]
    runs: std::sync::atomic::AtomicUsize,
}

impl Default for Jxl {
    fn default() -> Self {
        Self {
            webview: AtomicBool::new(false),
            queue: Mutex::default(),
            files: tokio::sync::Mutex::new(()),
            showing: Mutex::default(),
            slots: tokio::sync::Semaphore::new(shown_at_once()),
            gate: Mutex::new(None),
            #[cfg(test)]
            runs: std::sync::atomic::AtomicUsize::new(0),
        }
    }
}

/// Which version of a stored JPEG XL a made version belongs to.
fn shown_key(token: &str, jxl: &[u8]) -> String {
    use std::hash::{Hash, Hasher};
    let mut hasher = std::collections::hash_map::DefaultHasher::new();
    jxl.hash(&mut hasher);
    format!("{token}:{:016x}", hasher.finish())
}

/// Whether an `Accept` header takes JPEG XL: it lists `image/jxl`, or it's an image request
/// (`image/*`, as webviews' pictures ask) from a webview whose probe said it shows JPEG XL.
/// Requests from Rust and the platform (no `Accept`, or `*/*`) get a widely shown format.
fn takes_jxl(accept: &str, webview_shows: bool) -> bool {
    let mut images = false;
    for item in accept.split(',') {
        let mut parts = item.split(';');
        let kind = parts.next().unwrap_or_default().trim();
        let refused = parts.any(|parameter| {
            parameter
                .trim()
                .strip_prefix("q=")
                .and_then(|weight| weight.trim().parse::<f32>().ok())
                .is_some_and(|weight| weight <= 0.0)
        });
        if refused {
            continue;
        }
        if kind.eq_ignore_ascii_case(JXL_TYPE) {
            return true;
        }
        images |= kind.eq_ignore_ascii_case("image/*");
    }
    images && webview_shows
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
    in_use: Arc<Mutex<Option<InUse>>>,
    jxl: Arc<Jxl>,
    /// Fetches running, by token: rows of one album ask for one cover at once, and it's fetched
    /// once for all of them.
    fetching: Arc<Mutex<HashMap<String, Fetching>>>,
    timeout: Duration,
}

impl CoverCache {
    pub fn new(dir: PathBuf, max_bytes: u64, refresh: Duration) -> Self {
        Self {
            dir,
            max_bytes: Arc::new(AtomicU64::new(max_bytes)),
            refresh_ms: Arc::new(AtomicU64::new(refresh.as_millis() as u64)),
            written: Arc::new(AtomicU64::new(u64::MAX / 2)),
            checking: Arc::new(Mutex::new(HashSet::new())),
            in_use: Arc::new(Mutex::new(None)),
            jxl: Arc::new(Jxl::default()),
            fetching: Arc::new(Mutex::new(HashMap::new())),
            timeout: FETCH_TIMEOUT,
        }
    }

    /// The same cache giving sources `timeout` to send a cover.
    pub fn with_timeout(mut self, timeout: Duration) -> Self {
        self.timeout = timeout;
        self
    }

    /// What trimming leaves however long ago it was seen: the tokens `in_use` names when the
    /// folder is trimmed (the playing and queued songs' covers).
    pub fn spare_with(&self, in_use: impl Fn() -> HashSet<String> + Send + Sync + 'static) {
        *self.in_use.lock().expect("cover cache poisoned") = Some(Arc::new(in_use));
    }

    /// Changes the limits (from settings) and trims to the new size.
    pub fn set_limits(&self, max_bytes: u64, refresh: Duration) {
        self.max_bytes.store(max_bytes, Ordering::Relaxed);
        self.refresh_ms
            .store(refresh.as_millis() as u64, Ordering::Relaxed);
        self.written.store(u64::MAX / 2, Ordering::Relaxed);
    }

    /// What the webview's probe found: whether it shows JPEG XL. Its picture requests then get
    /// the stored files as they are.
    pub fn set_webview_jxl(&self, shows: bool) {
        self.jxl.webview.store(shows, Ordering::Relaxed);
    }

    /// What converting waits for before each cover: the app rests it in battery saver and while
    /// it's in the background with nothing playing, and it picks up again after. Covers being
    /// shown never wait.
    pub fn convert_when(&self, gate: impl Fn() -> BoxFuture<'static, ()> + Send + Sync + 'static) {
        *self.jxl.gate.lock().expect("cover gate poisoned") = Some(Arc::new(gate));
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
            let _files = self.jxl.files.lock().await;
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

    /// Queues every stored copy not yet looked at for converting (covers cached before JPEG XL,
    /// or whose conversion never ran): the webview keeps covers it has shown and doesn't ask for
    /// them again, so waiting for a cover to be seen would leave them as they are. Returns how
    /// many were queued.
    pub async fn convert_pending(&self) -> usize {
        let mut queued = 0;
        for dir in [self.dir.clone(), self.dir.join(KEEP)] {
            let Ok(mut entries) = tokio::fs::read_dir(&dir).await else {
                continue;
            };
            while let Ok(Some(entry)) = entries.next_entry().await {
                let name = entry.file_name();
                let Some(token) = name.to_str() else {
                    continue;
                };
                if !Self::valid(token) || !entry.file_type().await.is_ok_and(|kind| kind.is_file())
                {
                    continue;
                }
                if self.read_meta(token).await.stored == Stored::Pending {
                    self.convert_later(token);
                    queued += 1;
                }
            }
        }
        queued
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
            let _files = self.jxl.files.lock().await;
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
        self.convert_later(token);
        // Covers that aren't kept are trimmed once enough was written.
        let written = self
            .written
            .fetch_add(bytes.len() as u64, Ordering::Relaxed);
        let max = self.max_bytes.load(Ordering::Relaxed);
        if written + bytes.len() as u64 > max / 10 {
            self.written.store(0, Ordering::Relaxed);
            let dir = self.dir.clone();
            let in_use = self.in_use.lock().expect("cover cache poisoned").clone();
            let _ = tokio::task::spawn_blocking(move || {
                // The size limit covers kept covers too; only the others go, and of those not
                // the ones in use.
                let kept = usage_of(&dir.join(KEEP)).bytes;
                let spared = in_use.map(|in_use| in_use()).unwrap_or_default();
                evict_sparing(&dir, max.saturating_sub(kept), &spared);
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
                if self.read_meta(token).await.stored == Stored::Pending {
                    self.convert_later(token);
                }
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
            let meta = self.read_meta(token).await;
            // Copies from before JPEG XL, or whose conversion didn't run, are converted now.
            if meta.stored == Stored::Pending {
                self.convert_later(token);
            }
            let refresh = self.refresh_ms.load(Ordering::Relaxed);
            if refresh > 0 && now_ms().saturating_sub(meta.checked_ms) > refresh {
                self.check_later(client, token, url, headers, kept, meta);
            }
            return Ok(bytes);
        }
        // One fetch per cover, whoever else asks meanwhile; a request that goes away leaves the
        // fetch to the next one waiting.
        let fetching = self
            .fetching
            .lock()
            .expect("cover fetches poisoned")
            .entry(token.to_owned())
            .or_default()
            .clone();
        let result = fetching
            .get_or_init(|| self.fetch_and_store(client, token, url, headers, kept))
            .await
            .clone();
        let mut running = self.fetching.lock().expect("cover fetches poisoned");
        if running
            .get(token)
            .is_some_and(|cell| Arc::ptr_eq(cell, &fetching))
        {
            running.remove(token);
        }
        result
    }

    async fn fetch_and_store(
        &self,
        client: &reqwest::Client,
        token: &str,
        url: &str,
        headers: &BTreeMap<String, String>,
        kept: bool,
    ) -> Result<Vec<u8>, String> {
        match fetch(client, url, headers, None, self.timeout).await? {
            Fetched::Image {
                bytes,
                etag,
                last_modified,
            } => {
                let meta = Meta {
                    checked_ms: now_ms(),
                    etag,
                    last_modified,
                    stored: Stored::Pending,
                };
                self.store(token, kept, &bytes, &meta).await;
                Ok(bytes)
            }
            Fetched::Unchanged => Err("the server answered 304 to a plain request".into()),
        }
    }

    /// The cover for `token` as the request that asked takes it ([`Self::served`]). A stored
    /// JPEG XL that can't be read is dropped and the cover fetched again from its source.
    pub async fn get_served(
        &self,
        client: &reqwest::Client,
        token: &str,
        url: &str,
        headers: &BTreeMap<String, String>,
        kept: bool,
        accept: &str,
    ) -> Result<Served, String> {
        let bytes = self.get(client, token, url, headers, kept).await?;
        if let Some(served) = self.served(token, bytes, accept).await {
            return Ok(served);
        }
        tracing::warn!(token, "a stored cover couldn't be read; fetching it again");
        self.forget(token).await;
        let bytes = self.get(client, token, url, headers, kept).await?;
        self.served(token, bytes, accept)
            .await
            .ok_or_else(|| "the cover can't be read".into())
    }

    /// Like [`Self::stored`], as the request takes it; a copy that can't be read is dropped.
    pub async fn stored_served(&self, token: &str, accept: &str) -> Option<Served> {
        let bytes = self.stored(token).await?;
        let served = self.served(token, bytes, accept).await;
        if served.is_none() {
            self.forget(token).await;
        }
        served
    }

    /// `bytes` (a copy of `token`) as a request with this `Accept` takes them: anything but JPEG
    /// XL as it is, JPEG XL as it is where it's taken ([`takes_jxl`]), else a version made once
    /// and kept a while ([`cover_format::displayable`]). `None` when the JPEG XL can't be read.
    pub async fn served(&self, token: &str, bytes: Vec<u8>, accept: &str) -> Option<Served> {
        if !is_jxl(&bytes) {
            let content_type = sniff_content_type(&bytes);
            return Some(Served {
                bytes,
                content_type,
            });
        }
        if takes_jxl(accept, self.jxl.webview.load(Ordering::Relaxed)) {
            return Some(Served {
                bytes,
                content_type: JXL_TYPE,
            });
        }
        let shown = self.shown(token, bytes).await?;
        Some(Served {
            bytes: shown.bytes.clone(),
            content_type: shown.content_type,
        })
    }

    /// The version of a stored JPEG XL for requests that can't take it, made once.
    async fn shown(&self, token: &str, jxl: Vec<u8>) -> Option<Arc<Shown>> {
        let key = shown_key(token, &jxl);
        let making = {
            let mut showing = self.jxl.showing.lock().expect("shown covers poisoned");
            if let Some(shown) = showing.get(&key) {
                return Some(shown);
            }
            showing.making.entry(key.clone()).or_default().clone()
        };
        let shown = making
            .get_or_init(move || async move {
                let _slot = self.jxl.slots.acquire().await.ok()?;
                match tokio::task::spawn_blocking(move || cover_format::displayable(&jxl)).await {
                    Ok(Ok(shown)) => Some(Arc::new(shown)),
                    Ok(Err(error)) => {
                        tracing::warn!(%error, "a stored cover couldn't be read");
                        None
                    }
                    Err(error) => {
                        tracing::warn!(%error, "showing a cover stopped");
                        None
                    }
                }
            })
            .await
            .clone();
        let mut showing = self.jxl.showing.lock().expect("shown covers poisoned");
        if let Some(shown) = &shown {
            showing.put(key.clone(), shown.clone());
        }
        showing.making.remove(&key);
        shown
    }

    /// Queues a copy for converting; one worker converts the queue in turn, in the background.
    fn convert_later(&self, token: &str) {
        let start = {
            let mut queue = self.jxl.queue.lock().expect("cover queue poisoned");
            if !queue.queued.insert(token.to_owned()) {
                return;
            }
            queue.order.push_back(token.to_owned());
            !std::mem::replace(&mut queue.running, true)
        };
        if start {
            let cache = self.clone();
            tokio::spawn(cache.convert_queued());
        }
    }

    /// The worker: converts queued copies one at a time until none is left, waiting on the app's
    /// gate ([`Self::convert_when`]) before each. A copy queued again while it's converted gets a
    /// second look, which its record ends at once unless the copy changed meanwhile.
    async fn convert_queued(self) {
        loop {
            {
                let mut queue = self.jxl.queue.lock().expect("cover queue poisoned");
                if queue.order.is_empty() {
                    queue.running = false;
                    return;
                }
            }
            let gate = self.jxl.gate.lock().expect("cover gate poisoned").clone();
            if let Some(gate) = gate {
                gate().await;
            }
            let next = {
                let mut queue = self.jxl.queue.lock().expect("cover queue poisoned");
                let next = queue.order.pop_front();
                if let Some(token) = &next {
                    queue.queued.remove(token);
                }
                queue.running = next.is_some();
                next
            };
            let Some(token) = next else {
                return;
            };
            let cache = self.clone();
            let _ = tokio::task::spawn_blocking(move || cache.convert_now(&token)).await;
        }
    }

    /// The copy of `token` on disk now, kept or not.
    fn current(&self, token: &str) -> Option<(PathBuf, Vec<u8>)> {
        [true, false].into_iter().find_map(|kept| {
            let path = self.file(token, kept);
            std::fs::read(&path).ok().map(|bytes| (path, bytes))
        })
    }

    fn read_meta_now(&self, token: &str) -> Meta {
        std::fs::read_to_string(self.meta_file(token))
            .map(|text| Meta::decode(&text))
            .unwrap_or_default()
    }

    /// Converts one copy unless its record says it was; blocks.
    fn convert_now(&self, token: &str) {
        let Some((_, original)) = self.current(token) else {
            return;
        };
        if self.read_meta_now(token).stored != Stored::Pending {
            return;
        }
        // A source that sent JPEG XL already: nothing to convert.
        let conversion = if is_jxl(&original) {
            None
        } else {
            #[cfg(test)]
            self.jxl.runs.fetch_add(1, Ordering::SeqCst);
            // A converter that stops on a picture leaves it as it came, for good.
            let converted = std::panic::catch_unwind(|| cover_format::convert(&original));
            Some(converted.unwrap_or_else(|_| Conversion::Original("the converter stopped".into())))
        };
        self.swap(token, &original, conversion);
    }

    /// Puts a conversion of `original` in place, unless the copy changed or went meanwhile
    /// (checked again, trimmed, forgotten): that one is converted when next seen.
    fn swap(&self, token: &str, original: &[u8], conversion: Option<Conversion>) {
        let _files = self.jxl.files.blocking_lock();
        let Some((path, current)) = self.current(token) else {
            return;
        };
        if current != original {
            return;
        }
        let stored = match conversion {
            None => Stored::Jxl(original.len() as u64),
            Some(Conversion::Jxl { bytes, method }) => {
                let temporary = path.with_file_name(format!("{token}.jxl"));
                let written = std::fs::write(&temporary, &bytes)
                    .and_then(|()| std::fs::rename(&temporary, &path));
                if let Err(error) = written {
                    tracing::debug!(%error, "couldn't store a cover as JPEG XL");
                    let _ = std::fs::remove_file(&temporary);
                    return;
                }
                tracing::debug!(
                    token,
                    ?method,
                    before = original.len(),
                    after = bytes.len(),
                    "cover stored as JPEG XL"
                );
                Stored::Jxl(original.len() as u64)
            }
            Some(Conversion::Original(reason)) => {
                tracing::debug!(token, %reason, "cover kept as it came");
                Stored::Kept
            }
        };
        let meta = Meta {
            stored,
            ..self.read_meta_now(token)
        };
        let _ = std::fs::create_dir_all(self.dir.join(META));
        if let Err(error) = std::fs::write(self.meta_file(token), meta.encode()) {
            tracing::debug!(%error, "couldn't record how a cover is stored");
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
            match fetch(&client, &url, &headers, Some(&meta), cache.timeout).await {
                Ok(Fetched::Unchanged) => {
                    // Read again: a conversion may have recorded how the copy is stored.
                    let _files = cache.jxl.files.lock().await;
                    let checked = Meta {
                        checked_ms: now_ms(),
                        ..cache.read_meta(&token).await
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
                        stored: Stored::Pending,
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
        let _files = self.jxl.files.lock().await;
        for kept in [false, true] {
            let _ = tokio::fs::remove_file(self.file(token, kept)).await;
        }
        let _ = tokio::fs::remove_file(self.meta_file(token)).await;
        self.jxl
            .showing
            .lock()
            .expect("shown covers poisoned")
            .forget(token);
    }

    /// Puts every cached cover where it belongs: those in `kept` (the library's) into `keep/`,
    /// the rest back among those evicted normally. Returns how many moved in and out.
    pub async fn sync_kept(&self, kept: HashSet<String>) -> (u64, u64) {
        let dir = self.dir.clone();
        let jxl = self.jxl.clone();
        tokio::task::spawn_blocking(move || {
            let _files = jxl.files.blocking_lock();
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

    /// What storing covers as JPEG XL saves, from the copies on disk and their records.
    pub async fn saving(&self) -> Saving {
        let dir = self.dir.clone();
        tokio::task::spawn_blocking(move || saving_of(&dir))
            .await
            .unwrap_or_default()
    }

    /// Deletes every cached image; they're fetched again when next seen.
    pub async fn clear(&self) {
        let _files = self.jxl.files.lock().await;
        let _ = tokio::fs::remove_dir_all(&self.dir).await;
        self.written.store(u64::MAX / 2, Ordering::Relaxed);
        self.jxl
            .showing
            .lock()
            .expect("shown covers poisoned")
            .clear();
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

/// The copies stored as JPEG XL, by their records, against the files still there.
fn saving_of(dir: &Path) -> Saving {
    let mut sizes = HashMap::new();
    for folder in [dir.to_path_buf(), dir.join(KEEP)] {
        let Ok(entries) = std::fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if let Ok(metadata) = entry.metadata()
                && metadata.is_file()
            {
                sizes.insert(entry.file_name(), metadata.len());
            }
        }
    }
    let Ok(records) = std::fs::read_dir(dir.join(META)) else {
        return Saving::default();
    };
    let mut saving = Saving::default();
    for record in records.filter_map(Result::ok) {
        let Some(&size) = sizes.get(&record.file_name()) else {
            continue;
        };
        let Ok(text) = std::fs::read_to_string(record.path()) else {
            continue;
        };
        if let Stored::Jxl(original) = Meta::decode(&text).stored {
            saving.files += 1;
            saving.original_bytes += original.max(size);
            saving.bytes += size;
        }
    }
    saving
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
mod tests;
