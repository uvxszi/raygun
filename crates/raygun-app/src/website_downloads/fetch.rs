//! One file's bytes into its partial file: with the page's cookies, User-Agent and Referer,
//! carrying on from what's already there when the server takes ranges (and the file hasn't
//! changed: `If-Range`), starting over when it doesn't. Cancellable between any two chunks.

use std::path::Path;
use std::time::Duration;

use futures_util::StreamExt;
use reqwest::StatusCode;
use reqwest::header::{self, HeaderMap};
use serde::{Deserialize, Serialize};
use tokio::io::AsyncWriteExt;
use tokio::sync::watch;

/// A download that hasn't moved for this long is retried (from where it is).
pub const STALL: Duration = Duration::from_secs(30);

/// What a request carries from the page that started it.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Origin {
    pub url: String,
    pub user_agent: Option<String>,
    pub referer: Option<String>,
    /// A `Cookie` header's value, from the browser's cookies for the address.
    pub cookies: Option<String>,
}

/// What's known about the file on the server, to carry on later.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Validator {
    pub etag: Option<String>,
    pub last_modified: Option<String>,
    pub total: Option<u64>,
    /// The server said it takes ranges.
    pub ranges: bool,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Failure {
    Cancelled,
    /// Worth trying again (from where it is).
    Transient(String),
    Permanent(String),
}

/// A finished file: its size and the type the server gave.
#[derive(Debug, Clone, PartialEq)]
pub struct Fetched {
    pub bytes: u64,
    pub mime: Option<String>,
}

/// Waits until `cancel` says so.
pub async fn cancelled(cancel: &mut watch::Receiver<bool>) {
    loop {
        if *cancel.borrow_and_update() {
            return;
        }
        if cancel.changed().await.is_err() {
            // The sender is gone: nothing can cancel any more.
            std::future::pending::<()>().await;
        }
    }
}

/// `bytes 100-199/1000` → (100, Some(1000)).
pub fn content_range(value: &str) -> Option<(u64, Option<u64>)> {
    let rest = value.trim().strip_prefix("bytes")?.trim();
    let (range, total) = rest.split_once('/')?;
    let (start, _end) = range.split_once('-')?;
    let start = start.trim().parse().ok()?;
    let total = total.trim().parse().ok();
    Some((start, total))
}

fn text(headers: &HeaderMap, name: header::HeaderName) -> Option<String> {
    headers
        .get(name)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned)
}

fn status_failure(status: StatusCode) -> Failure {
    let code = status.as_u16();
    match code {
        408 | 425 | 429 | 500..=599 => {
            Failure::Transient(format!("The site didn't answer (HTTP {code})"))
        }
        401 | 403 => Failure::Permanent(format!(
            "The site refused the download (HTTP {code}). Open the page again and start it there"
        )),
        404 | 410 => Failure::Permanent(format!("The file isn't there any more (HTTP {code})")),
        _ => Failure::Permanent(format!("The site answered HTTP {code}")),
    }
}

fn network_failure(error: reqwest::Error) -> Failure {
    if error.is_builder() {
        return Failure::Permanent("That address can't be downloaded".into());
    }
    let reason = if error.is_timeout() {
        "The site took too long to answer"
    } else if error.is_connect() {
        "The site couldn't be reached"
    } else {
        "The connection broke"
    };
    Failure::Transient(reason.to_owned())
}

fn disk_failure(error: std::io::Error) -> Failure {
    Failure::Permanent(format!("The file couldn't be written: {error}"))
}

/// Fetches `origin.url` into `partial`, carrying on from its length when it can. `progress`
/// gets the bytes so far and the whole size when known.
pub async fn fetch(
    client: &reqwest::Client,
    origin: &Origin,
    partial: &Path,
    validator: &mut Validator,
    cancel: &mut watch::Receiver<bool>,
    progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
) -> Result<Fetched, Failure> {
    if origin.url.starts_with("data:") {
        return write_data_url(&origin.url, partial, progress).await;
    }
    // At most one start over (the server ignored or refused the range).
    for _ in 0..2 {
        let mut have = tokio::fs::metadata(partial)
            .await
            .map(|meta| meta.len())
            .unwrap_or(0);
        if have > 0 && !validator.ranges {
            have = 0;
        }
        let mut request = client
            .get(&origin.url)
            .header(header::ACCEPT, "*/*")
            // Ranges count the bytes as sent: no compression.
            .header(header::ACCEPT_ENCODING, "identity");
        if let Some(agent) = origin
            .user_agent
            .as_deref()
            .filter(|agent| !agent.is_empty())
        {
            request = request.header(header::USER_AGENT, agent);
        }
        if let Some(referer) = origin
            .referer
            .as_deref()
            .filter(|referer| referer.starts_with("https://") || referer.starts_with("http://"))
        {
            request = request.header(header::REFERER, referer);
        }
        if let Some(cookies) = origin
            .cookies
            .as_deref()
            .filter(|cookies| !cookies.is_empty())
        {
            request = request.header(header::COOKIE, cookies);
        }
        if have > 0 {
            request = request.header(header::RANGE, format!("bytes={have}-"));
            let strong = validator
                .etag
                .as_deref()
                .filter(|tag| !tag.starts_with("W/"));
            if let Some(check) = strong.or(validator.last_modified.as_deref()) {
                request = request.header(header::IF_RANGE, check);
            }
        }
        let response = tokio::select! {
            response = request.send() => response.map_err(network_failure)?,
            () = cancelled(cancel) => return Err(Failure::Cancelled),
        };
        let status = response.status();
        let headers = response.headers().clone();
        let mime = text(&headers, header::CONTENT_TYPE);
        let append = match status {
            StatusCode::PARTIAL_CONTENT => {
                let range = text(&headers, header::CONTENT_RANGE)
                    .as_deref()
                    .and_then(content_range);
                match range {
                    Some((start, total)) if start == have => {
                        validator.total = total.or(validator.total);
                        true
                    }
                    // A range other than the one asked for: start over.
                    _ => {
                        validator.ranges = false;
                        truncate(partial).await?;
                        continue;
                    }
                }
            }
            StatusCode::RANGE_NOT_SATISFIABLE => {
                let total = text(&headers, header::CONTENT_RANGE)
                    .as_deref()
                    .and_then(|value| value.trim().strip_prefix("bytes */"))
                    .and_then(|total| total.trim().parse::<u64>().ok())
                    .or(validator.total);
                if have > 0 && total == Some(have) {
                    progress(have, Some(have));
                    return Ok(Fetched { bytes: have, mime });
                }
                validator.ranges = false;
                truncate(partial).await?;
                continue;
            }
            status if status.is_success() => {
                validator.total = response.content_length();
                false
            }
            status => return Err(status_failure(status)),
        };
        if !append {
            have = 0;
        }
        validator.etag = text(&headers, header::ETAG);
        validator.last_modified = text(&headers, header::LAST_MODIFIED);
        validator.ranges = status == StatusCode::PARTIAL_CONTENT
            || text(&headers, header::ACCEPT_RANGES)
                .is_some_and(|ranges| ranges.to_ascii_lowercase().contains("bytes"));

        if let Some(parent) = partial.parent() {
            tokio::fs::create_dir_all(parent)
                .await
                .map_err(disk_failure)?;
        }
        let mut file = tokio::fs::OpenOptions::new()
            .create(true)
            .write(true)
            .append(append)
            .truncate(!append)
            .open(partial)
            .await
            .map_err(disk_failure)?;
        let total = validator.total;
        progress(have, total);
        let mut stream = response.bytes_stream();
        loop {
            let chunk = tokio::select! {
                chunk = tokio::time::timeout(STALL, stream.next()) => chunk,
                () = cancelled(cancel) => {
                    let _ = file.flush().await;
                    return Err(Failure::Cancelled);
                }
                // Offline Mode stops it where it is.
                () = raygun_net::network_mode::went_offline() => {
                    let _ = file.flush().await;
                    return Err(Failure::Transient(
                        raygun_net::network_mode::OFFLINE_MESSAGE.to_owned(),
                    ));
                }
            };
            let chunk = match chunk {
                Err(_) => {
                    let _ = file.flush().await;
                    return Err(Failure::Transient("The download stalled".into()));
                }
                Ok(None) => break,
                Ok(Some(Err(error))) => {
                    let _ = file.flush().await;
                    return Err(network_failure(error));
                }
                Ok(Some(Ok(chunk))) => chunk,
            };
            file.write_all(&chunk).await.map_err(disk_failure)?;
            have += chunk.len() as u64;
            progress(have, total);
        }
        file.flush().await.map_err(disk_failure)?;
        file.sync_all().await.map_err(disk_failure)?;
        if total.is_some_and(|total| have < total) {
            return Err(Failure::Transient("The download was cut short".into()));
        }
        return Ok(Fetched { bytes: have, mime });
    }
    Err(Failure::Transient(
        "The site sent the file in pieces Raygun couldn't join".into(),
    ))
}

async fn truncate(partial: &Path) -> Result<(), Failure> {
    match tokio::fs::remove_file(partial).await {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(disk_failure(error)),
    }
}

/// A `data:` address's bytes (base64 or percent-encoded).
pub fn data_url_bytes(url: &str) -> Option<(Vec<u8>, Option<String>)> {
    use base64::Engine;
    let rest = url.strip_prefix("data:")?;
    let (meta, data) = rest.split_once(',')?;
    let base64 = meta.ends_with(";base64");
    let mime = meta
        .trim_end_matches(";base64")
        .split(';')
        .next()
        .filter(|mime| !mime.is_empty())
        .map(str::to_owned);
    let decoded = super::names::percent_decode(data.as_bytes());
    let bytes = if base64 {
        let cleaned: Vec<u8> = decoded
            .into_iter()
            .filter(|byte| !byte.is_ascii_whitespace())
            .collect();
        base64::engine::general_purpose::STANDARD
            .decode(&cleaned)
            .or_else(|_| base64::engine::general_purpose::STANDARD_NO_PAD.decode(&cleaned))
            .ok()?
    } else {
        decoded
    };
    Some((bytes, mime))
}

async fn write_data_url(
    url: &str,
    partial: &Path,
    progress: &mut (dyn FnMut(u64, Option<u64>) + Send),
) -> Result<Fetched, Failure> {
    let (bytes, mime) =
        data_url_bytes(url).ok_or_else(|| Failure::Permanent("The file is unreadable".into()))?;
    if let Some(parent) = partial.parent() {
        tokio::fs::create_dir_all(parent)
            .await
            .map_err(disk_failure)?;
    }
    tokio::fs::write(partial, &bytes)
        .await
        .map_err(disk_failure)?;
    let length = bytes.len() as u64;
    progress(length, Some(length));
    Ok(Fetched {
        bytes: length,
        mime,
    })
}
