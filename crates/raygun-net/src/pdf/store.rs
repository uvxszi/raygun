//! The PDF documents the stream proxy serves pages of, at `/pdf/<token>/<page>?w=<pixels>`.
//!
//! A token names a document: `l<hex>` one under the proxy's local folder (an imported book, a
//! downloaded chapter), by its path there, so its links outlive the app; `w<hash>` one on the web,
//! registered by the service that read its address (with the headers its source gave), fetched
//! once into the web copies' folder and kept there, newest first, up to a size. A few documents
//! stay open; pages are drawn two at a time (the reader asks for the pages around the one shown,
//! which draws them ahead) and kept in the page cache by document, page and width.

use std::collections::{BTreeMap, HashMap};
use std::fmt;
use std::net::SocketAddr;
use std::ops::Range;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use futures_util::StreamExt;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use super::{Document, MAX_PDF_BYTES, PageText, is_pdf};
use crate::page_cache::PageCache;

/// Documents kept open at once.
const OPEN_DOCUMENTS: usize = 3;
/// Pages drawn at once.
const DRAWING: usize = 2;
/// The widths pages are drawn at: what the reader asks for is rounded up to one, so the cache
/// holds few versions of a page.
const WIDTHS: [u32; 12] = [
    480, 720, 960, 1200, 1440, 1680, 1920, 2400, 2880, 3360, 3840, 4096,
];
/// Drawn when the reader names no width.
const DEFAULT_WIDTH: u32 = 1200;
/// The web copies' folder is trimmed to this.
const MAX_WEB_COPIES_BYTES: u64 = 1536 * 1024 * 1024;
/// How long a document may take to arrive.
const FETCH_TIMEOUT: Duration = Duration::from_secs(600);
/// Hits a search returns at most.
const MAX_HITS: usize = 300;
/// Text kept around a hit.
const BEFORE: usize = 40;
const AFTER: usize = 80;

/// Why a document or page couldn't be had.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PdfError {
    /// No such document (a link from an earlier run whose copy is gone, a removed book).
    Missing,
    /// It's on the web and Offline Mode is on.
    Offline,
    Failed(String),
}

impl fmt::Display for PdfError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Missing => f.write_str("The PDF isn't on this device anymore"),
            Self::Offline => f.write_str(crate::network_mode::OFFLINE_MESSAGE),
            Self::Failed(message) => f.write_str(message),
        }
    }
}

impl From<PdfError> for String {
    fn from(error: PdfError) -> Self {
        error.to_string()
    }
}

/// A place in a document's text that matched a search.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchHit {
    /// The page, from 0.
    pub page: usize,
    pub before: String,
    pub matched: String,
    pub after: String,
}

/// Web documents by token: their address and the headers they're fetched with.
type WebDocuments = HashMap<String, (String, BTreeMap<String, String>)>;

struct Inner {
    address: SocketAddr,
    client: reqwest::Client,
    local: Option<PathBuf>,
    dir: Option<PathBuf>,
    pages: Option<PageCache>,
    web: Mutex<WebDocuments>,
    /// Open documents, the most recently used last.
    open: Mutex<Vec<(String, Arc<Document>)>>,
    /// One fetch per document at a time.
    fetching: tokio::sync::Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
    drawing: tokio::sync::Semaphore,
    /// Whole documents' text, read for searching.
    texts: Mutex<Vec<(String, Arc<Vec<PageText>>)>>,
}

/// The documents the proxy serves pages of. Cheap to clone.
#[derive(Clone)]
pub struct PdfStore {
    inner: Arc<Inner>,
}

impl PdfStore {
    pub(crate) fn new(
        address: SocketAddr,
        client: reqwest::Client,
        local: Option<PathBuf>,
        dir: Option<PathBuf>,
        pages: Option<PageCache>,
    ) -> Self {
        Self {
            inner: Arc::new(Inner {
                address,
                client,
                local,
                dir,
                pages,
                web: Mutex::default(),
                open: Mutex::default(),
                fetching: tokio::sync::Mutex::default(),
                drawing: tokio::sync::Semaphore::new(DRAWING),
                texts: Mutex::default(),
            }),
        }
    }

    /// The token of a document under the proxy's local folder, by its path there
    /// (`books/<hash>/document.pdf`).
    pub fn local_token(&self, relative: &str) -> String {
        let hex: String = relative
            .as_bytes()
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect();
        format!("l{hex}")
    }

    /// The token of a document on the web, fetched with `headers`; its pages can be served from
    /// now on.
    pub fn web_token(&self, url: &str, headers: &BTreeMap<String, String>) -> String {
        let token = format!("w{}", crate::proxy::image_token(url, headers));
        self.inner
            .web
            .lock()
            .expect("pdf registry poisoned")
            .insert(token.clone(), (url.to_owned(), headers.clone()));
        token
    }

    /// The proxy address of page `index` (from 0); the reader adds `?w=<pixels>`.
    pub fn page_url(&self, token: &str, index: usize) -> String {
        format!("http://{}/pdf/{token}/{index}", self.inner.address)
    }

    /// The document behind `token`, opened (a web one fetched first).
    pub async fn document(&self, token: &str) -> Result<Arc<Document>, PdfError> {
        if let Some(document) = self.opened(token) {
            return Ok(document);
        }
        let path = self.file_of(token).await?;
        let document = tokio::task::spawn_blocking(move || Document::open(&path))
            .await
            .map_err(|error| PdfError::Failed(error.to_string()))?
            .map_err(PdfError::Failed)?;
        let document = Arc::new(document);
        let mut open = self.inner.open.lock().expect("open documents poisoned");
        open.retain(|(kept, _)| kept != token);
        open.push((token.to_owned(), document.clone()));
        if open.len() > OPEN_DOCUMENTS {
            open.remove(0);
        }
        Ok(document)
    }

    fn opened(&self, token: &str) -> Option<Arc<Document>> {
        let mut open = self.inner.open.lock().expect("open documents poisoned");
        let at = open.iter().position(|(kept, _)| kept == token)?;
        let entry = open.remove(at);
        let document = entry.1.clone();
        open.push(entry);
        Some(document)
    }

    /// Where the document's file is, fetching a web one that isn't here yet.
    async fn file_of(&self, token: &str) -> Result<PathBuf, PdfError> {
        if let Some(hex) = token.strip_prefix('l') {
            let relative = decode_hex(hex)
                .and_then(|bytes| String::from_utf8(bytes).ok())
                .ok_or(PdfError::Missing)?;
            let path = self
                .inner
                .local
                .as_deref()
                .and_then(|root| crate::proxy::local_path(root, &relative))
                .ok_or(PdfError::Missing)?;
            return if tokio::fs::try_exists(&path).await.unwrap_or(false) {
                Ok(path)
            } else {
                Err(PdfError::Missing)
            };
        }
        let hash = token
            .strip_prefix('w')
            .filter(|hash| !hash.is_empty() && hash.bytes().all(|byte| byte.is_ascii_hexdigit()))
            .ok_or(PdfError::Missing)?;
        let dir = self.inner.dir.as_deref().ok_or(PdfError::Missing)?;
        let path = dir.join(format!("{hash}.pdf"));
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            let touched = path.clone();
            let _ = tokio::task::spawn_blocking(move || crate::page_cache::touch(&touched)).await;
            return Ok(path);
        }
        let lock = {
            let mut fetching = self.inner.fetching.lock().await;
            fetching.entry(token.to_owned()).or_default().clone()
        };
        let _fetching = lock.lock().await;
        // Fetched while this waited.
        if tokio::fs::try_exists(&path).await.unwrap_or(false) {
            return Ok(path);
        }
        let (url, headers) = self
            .inner
            .web
            .lock()
            .expect("pdf registry poisoned")
            .get(token)
            .cloned()
            .ok_or(PdfError::Missing)?;
        self.fetch(&url, &headers, dir, &path).await?;
        Ok(path)
    }

    async fn fetch(
        &self,
        url: &str,
        headers: &BTreeMap<String, String>,
        dir: &Path,
        path: &Path,
    ) -> Result<(), PdfError> {
        crate::network_mode::check(url).map_err(|_| PdfError::Offline)?;
        let failed = |error: &dyn fmt::Display| {
            PdfError::Failed(format!("The PDF couldn't be fetched ({error})"))
        };
        let response = crate::server_auth::get(&self.inner.client, url, headers)
            .timeout(FETCH_TIMEOUT)
            .send()
            .await
            .map_err(|error| failed(&error))?;
        let status = response.status();
        if !status.is_success() {
            return Err(failed(&format!("the site answered {}", status.as_u16())));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_PDF_BYTES)
        {
            return Err(PdfError::Failed("This PDF is larger than 1 GB".into()));
        }
        tokio::fs::create_dir_all(dir)
            .await
            .map_err(|error| failed(&error))?;
        let partial = path.with_extension("part");
        let written = async {
            let mut file = tokio::fs::File::create(&partial)
                .await
                .map_err(|error| failed(&error))?;
            let mut head: Vec<u8> = Vec::new();
            let mut size: u64 = 0;
            let mut body = response.bytes_stream();
            while let Some(chunk) = body.next().await {
                let chunk = chunk.map_err(|error| failed(&error))?;
                size += chunk.len() as u64;
                if size > MAX_PDF_BYTES {
                    return Err(PdfError::Failed("This PDF is larger than 1 GB".into()));
                }
                if head.len() < 1024 {
                    head.extend_from_slice(&chunk[..chunk.len().min(1024 - head.len())]);
                    // A web page in place of the document (a sign-in or an error).
                    if head.len() >= 1024 && !is_pdf(&head) {
                        return Err(PdfError::Failed(
                            "The source sent a web page, not a PDF".into(),
                        ));
                    }
                }
                tokio::io::AsyncWriteExt::write_all(&mut file, &chunk)
                    .await
                    .map_err(|error| failed(&error))?;
            }
            if !is_pdf(&head) {
                return Err(PdfError::Failed(
                    "The source sent a web page, not a PDF".into(),
                ));
            }
            tokio::io::AsyncWriteExt::flush(&mut file)
                .await
                .map_err(|error| failed(&error))?;
            Ok(())
        }
        .await;
        if let Err(error) = written {
            let _ = tokio::fs::remove_file(&partial).await;
            return Err(error);
        }
        tokio::fs::rename(&partial, path)
            .await
            .map_err(|error| failed(&error))?;
        let dir = dir.to_path_buf();
        let _ = tokio::task::spawn_blocking(move || {
            crate::page_cache::evict(&dir, MAX_WEB_COPIES_BYTES)
        })
        .await;
        Ok(())
    }

    /// Copies a document to `target` (a download keeps the file itself): a web one is fetched
    /// first, unless its copy is here.
    pub async fn save_copy(&self, token: &str, target: &Path) -> Result<u64, PdfError> {
        let path = self.file_of(token).await?;
        tokio::fs::copy(&path, target)
            .await
            .map_err(|error| PdfError::Failed(format!("The PDF couldn't be saved ({error})")))
    }

    /// Page `index` drawn about `width` pixels wide (rounded up to one of a few widths), as PNG:
    /// from the page cache, else drawn and kept there.
    pub async fn page_png(
        &self,
        token: &str,
        index: usize,
        width: Option<u32>,
    ) -> Result<Vec<u8>, PdfError> {
        let width = width_for(width);
        let key = page_key(token, index, width);
        if let Some(cache) = &self.inner.pages
            && let Some(bytes) = cache.read(&key).await
        {
            return Ok(bytes);
        }
        let document = self.document(token).await?;
        if index >= document.page_count() {
            return Err(PdfError::Missing);
        }
        let _drawing = self
            .inner
            .drawing
            .acquire()
            .await
            .map_err(|error| PdfError::Failed(error.to_string()))?;
        let bytes = tokio::task::spawn_blocking(move || document.render_png(index, width))
            .await
            .map_err(|error| PdfError::Failed(error.to_string()))?
            .map_err(PdfError::Failed)?;
        if let Some(cache) = &self.inner.pages {
            cache.write(&key, &bytes).await;
        }
        Ok(bytes)
    }

    /// The text of pages `range` (from 0) of a document.
    pub async fn texts(&self, token: &str, range: Range<usize>) -> Result<Vec<PageText>, PdfError> {
        if let Some(all) = self.kept_texts(token) {
            return Ok(all
                .get(range.start.min(all.len())..range.end.min(all.len()))
                .unwrap_or_default()
                .to_vec());
        }
        let document = self.document(token).await?;
        tokio::task::spawn_blocking(move || {
            let end = range.end.min(document.page_count());
            (range.start.min(end)..end)
                .map(|index| document.page_text(index))
                .collect()
        })
        .await
        .map_err(|error| PdfError::Failed(error.to_string()))
    }

    fn kept_texts(&self, token: &str) -> Option<Arc<Vec<PageText>>> {
        self.inner
            .texts
            .lock()
            .expect("pdf texts poisoned")
            .iter()
            .find(|(kept, _)| kept == token)
            .map(|(_, texts)| texts.clone())
    }

    /// Every page's text: read once (kept beside the web copies, so later searches are quick).
    async fn all_texts(&self, token: &str) -> Result<Arc<Vec<PageText>>, PdfError> {
        if let Some(texts) = self.kept_texts(token) {
            return Ok(texts);
        }
        let document = self.document(token).await?;
        let count = document.page_count();
        let file = self
            .inner
            .dir
            .as_ref()
            .map(|dir| dir.join("text").join(format!("{}.json", text_key(token))));
        let stored = match &file {
            Some(file) => tokio::fs::read(file)
                .await
                .ok()
                .and_then(|bytes| serde_json::from_slice::<Vec<PageText>>(&bytes).ok())
                .filter(|texts| texts.len() == count),
            None => None,
        };
        let texts = match stored {
            Some(texts) => texts,
            None => {
                let read = tokio::task::spawn_blocking(move || {
                    (0..count)
                        .map(|index| document.page_text(index))
                        .collect::<Vec<_>>()
                })
                .await
                .map_err(|error| PdfError::Failed(error.to_string()))?;
                if let Some(file) = &file
                    && let Ok(json) = serde_json::to_vec(&read)
                {
                    if let Some(parent) = file.parent() {
                        let _ = tokio::fs::create_dir_all(parent).await;
                    }
                    let _ = tokio::fs::write(file, json).await;
                }
                read
            }
        };
        let texts = Arc::new(texts);
        let mut kept = self.inner.texts.lock().expect("pdf texts poisoned");
        kept.retain(|(other, _)| other != token);
        kept.push((token.to_owned(), texts.clone()));
        if kept.len() > OPEN_DOCUMENTS {
            kept.remove(0);
        }
        Ok(texts)
    }

    /// Where `query` appears in pages `range` of a document, in page order (case ignored).
    pub async fn search(
        &self,
        token: &str,
        query: &str,
        range: Range<usize>,
    ) -> Result<Vec<SearchHit>, PdfError> {
        let query: Vec<char> = fold(&query.split_whitespace().collect::<Vec<_>>().join(" "));
        if query.is_empty() {
            return Ok(Vec::new());
        }
        let texts = self.all_texts(token).await?;
        let mut hits = Vec::new();
        for page in range.start.min(texts.len())..range.end.min(texts.len()) {
            hits.extend(find(
                &texts[page].plain(),
                &query,
                page,
                MAX_HITS - hits.len(),
            ));
            if hits.len() >= MAX_HITS {
                break;
            }
        }
        Ok(hits)
    }

    /// What the web copies take on disk.
    pub async fn usage(&self) -> crate::cover_cache::Usage {
        match self.inner.dir.clone() {
            Some(dir) => tokio::task::spawn_blocking(move || crate::cover_cache::usage_of(&dir))
                .await
                .unwrap_or_default(),
            None => crate::cover_cache::Usage::default(),
        }
    }

    /// Deletes the web copies (and their texts); they're fetched again when read.
    pub async fn clear(&self) {
        self.inner
            .open
            .lock()
            .expect("open documents poisoned")
            .clear();
        self.inner.texts.lock().expect("pdf texts poisoned").clear();
        if let Some(dir) = &self.inner.dir {
            let _ = tokio::fs::remove_dir_all(dir).await;
        }
    }
}

/// The width a page is drawn at for a reader asking for `wanted` pixels.
fn width_for(wanted: Option<u32>) -> u32 {
    let wanted = wanted.unwrap_or(DEFAULT_WIDTH);
    WIDTHS
        .iter()
        .copied()
        .find(|width| *width >= wanted)
        .unwrap_or(WIDTHS[WIDTHS.len() - 1])
}

/// A drawn page's name in the page cache (hex, as its names are).
fn page_key(token: &str, index: usize, width: u32) -> String {
    hex(&Sha256::digest(format!("pdf\0{token}\0{index}\0{width}").as_bytes())[..16])
}

fn text_key(token: &str) -> String {
    hex(&Sha256::digest(token.as_bytes())[..16])
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn decode_hex(hex: &str) -> Option<Vec<u8>> {
    if hex.is_empty() || !hex.len().is_multiple_of(2) {
        return None;
    }
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(hex.get(at..at + 2)?, 16).ok())
        .collect()
}

/// Text for matching: one lower-case character per character, whitespace as spaces.
fn fold(text: &str) -> Vec<char> {
    text.chars()
        .map(|char| {
            if char.is_whitespace() {
                ' '
            } else {
                char.to_lowercase().next().unwrap_or(char)
            }
        })
        .collect()
}

/// Up to `limit` places `query` (folded) appears in a page's `text`.
fn find(text: &str, query: &[char], page: usize, limit: usize) -> Vec<SearchHit> {
    let chars: Vec<char> = text.chars().collect();
    let folded = fold(text);
    let mut hits = Vec::new();
    let mut at = 0;
    while hits.len() < limit && at + query.len() <= folded.len() {
        if folded[at..at + query.len()] != *query {
            at += 1;
            continue;
        }
        let end = at + query.len();
        let start = at.saturating_sub(BEFORE);
        let stop = (end + AFTER).min(chars.len());
        let mut before: String = chars[start..at].iter().collect();
        let mut after: String = chars[end..stop].iter().collect();
        // Cut at words, marking what's left out.
        if start > 0 {
            before = match before.split_once(' ') {
                Some((_, rest)) if !rest.is_empty() => format!("…{rest}"),
                _ => format!("…{before}"),
            };
        }
        if stop < chars.len() {
            after = match after.rsplit_once(' ') {
                Some((rest, _)) if !rest.is_empty() => format!("{rest}…"),
                _ => format!("{after}…"),
            };
        }
        hits.push(SearchHit {
            page,
            before: before.replace('\n', " "),
            matched: chars[at..end].iter().collect::<String>().replace('\n', " "),
            after: after.replace('\n', " "),
        });
        at = end;
    }
    hits
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn widths_round_up_to_a_few() {
        assert_eq!(width_for(None), DEFAULT_WIDTH);
        assert_eq!(width_for(Some(1)), 480);
        assert_eq!(width_for(Some(1080)), 1200);
        assert_eq!(width_for(Some(99_999)), 4096);
    }

    #[test]
    fn local_tokens_name_their_path() {
        let store = PdfStore::new(
            "127.0.0.1:9".parse().unwrap(),
            crate::http_client().unwrap(),
            None,
            None,
            None,
        );
        let token = store.local_token("books/ab/document.pdf");
        assert_eq!(
            decode_hex(&token[1..]).as_deref(),
            Some(b"books/ab/document.pdf".as_slice())
        );
        assert!(decode_hex("abc").is_none());
        assert_eq!(
            store.page_url(&token, 2),
            format!("http://127.0.0.1:9/pdf/{token}/2")
        );
    }

    #[test]
    fn search_finds_words_with_their_surroundings() {
        let text = "The keeper climbed the stairs of the old Lighthouse every night, and the lighthouse shone.";
        let hits = find(text, &fold("LIGHTHOUSE"), 4, 10);
        assert_eq!(hits.len(), 2);
        assert_eq!(hits[0].page, 4);
        assert_eq!(hits[0].matched, "Lighthouse");
        assert!(hits[0].before.ends_with("the old "));
        assert!(hits[0].after.starts_with(" every night"));
        assert_eq!(find(text, &fold("lighthouse"), 0, 1).len(), 1);
    }
}
