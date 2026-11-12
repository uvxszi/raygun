//! Requests to the dictionaries' public APIs, made as Wikimedia's policies ask of a client: a
//! User-Agent naming Raygun and where to reach its makers, compressed answers, requests to one
//! host spaced out and at most two at a time, and a `429`/`503`'s `Retry-After` kept: the host
//! isn't asked again until then. Nothing is sent while Offline Mode is on (the shared client's
//! gate).

use std::collections::HashMap;
use std::io::Read;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use tokio::sync::Semaphore;

/// Sent with every request: Wikimedia's User-Agent policy wants the client named, with a way to
/// reach its makers.
pub fn user_agent() -> String {
    user_agent_for("dictionary lookups")
}

/// [`user_agent`] for another use of the same APIs (`answers in Search`).
pub fn user_agent_for(purpose: &str) -> String {
    format!(
        "Raygun/{} (https://github.com/uvxszi/raygun; {purpose}) reqwest",
        env!("CARGO_PKG_VERSION")
    )
}

/// Requests to one host start at least this far apart.
const SPACING: Duration = Duration::from_millis(120);
/// Requests to one host running at once, at most.
const PER_HOST: usize = 2;
/// How long one request may take.
const TIMEOUT: Duration = Duration::from_secs(10);
/// A host that asked to wait without saying how long is left alone this long.
const DEFAULT_HOLD: Duration = Duration::from_secs(30);
/// The longest wait a host's `Retry-After` is kept to.
const MAX_HOLD: Duration = Duration::from_secs(600);
/// The largest answer read, as sent and unpacked.
const MAX_SENT: usize = 4 * 1024 * 1024;
const MAX_UNPACKED: usize = 12 * 1024 * 1024;

/// What a request brought.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fetched {
    Body(String),
    /// The host has no such page (404).
    NotFound,
    /// It failed: one sentence naming the source.
    Failed(String),
}

#[derive(Default)]
struct Host {
    /// Asked to wait until then (`429`, `503`).
    held_until: Option<Instant>,
    /// When the last request started, or is planned to.
    last_start: Option<Instant>,
    turns: Option<Arc<Semaphore>>,
}

pub struct Polite {
    client: reqwest::Client,
    hosts: Mutex<HashMap<String, Host>>,
    agent: String,
}

impl Polite {
    pub fn new() -> Result<Self, String> {
        Self::with_agent(user_agent())
    }

    /// Requests sent with `agent` as their User-Agent.
    pub fn with_agent(agent: String) -> Result<Self, String> {
        let client = raygun_net::client_builder()
            .timeout(TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            client,
            hosts: Mutex::new(HashMap::new()),
            agent,
        })
    }

    fn hosts(&self) -> std::sync::MutexGuard<'_, HashMap<String, Host>> {
        self.hosts
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    /// How long `host` asked to be left alone, from now; none when it may be asked.
    pub fn held(&self, host: &str) -> Option<Duration> {
        let until = self.hosts().get(host)?.held_until?;
        until.checked_duration_since(Instant::now())
    }

    fn hold(&self, host: &str, wait: Duration) {
        self.hosts().entry(host.to_owned()).or_default().held_until =
            Some(Instant::now() + wait.min(MAX_HOLD));
    }

    /// Waits for `host`'s turn: a free slot, and the spacing since the last request.
    async fn turn(&self, host: &str) -> Option<tokio::sync::OwnedSemaphorePermit> {
        let (turns, wait) = {
            let mut hosts = self.hosts();
            let state = hosts.entry(host.to_owned()).or_default();
            let turns = state
                .turns
                .get_or_insert_with(|| Arc::new(Semaphore::new(PER_HOST)))
                .clone();
            let now = Instant::now();
            let start = state
                .last_start
                .map_or(now, |last| (last + SPACING).max(now));
            state.last_start = Some(start);
            (turns, start - now)
        };
        let permit = turns.acquire_owned().await.ok()?;
        if !wait.is_zero() {
            tokio::time::sleep(wait).await;
        }
        Some(permit)
    }

    /// GETs `url` (`source` names who answers, for messages).
    pub async fn get(&self, url: &str, accept: &str, source: &str) -> Fetched {
        let Some(host) = reqwest::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(str::to_owned))
        else {
            return Fetched::Failed(format!("{source} has no such address"));
        };
        if raygun_net::network_mode::offline() {
            return Fetched::Failed(raygun_net::network_mode::OFFLINE_MESSAGE.to_owned());
        }
        if let Some(wait) = self.held(&host) {
            return Fetched::Failed(format!(
                "{source} asked Raygun to wait; it's asked again in {} s",
                wait.as_secs().max(1)
            ));
        }
        let Some(_permit) = self.turn(&host).await else {
            return Fetched::Failed(format!("{source} couldn't be asked"));
        };
        let sent = self
            .client
            .get(url)
            .header(reqwest::header::USER_AGENT, &self.agent)
            .header(reqwest::header::ACCEPT, accept)
            .header(reqwest::header::ACCEPT_ENCODING, "gzip")
            .send()
            .await;
        let response = match sent {
            Ok(response) => response,
            Err(error) if raygun_net::network_mode::is_offline_error(&error) => {
                return Fetched::Failed(raygun_net::network_mode::OFFLINE_MESSAGE.to_owned());
            }
            Err(error) if error.is_timeout() => {
                return Fetched::Failed(format!("{source} didn't answer in time"));
            }
            Err(_) => return Fetched::Failed(format!("{source} couldn't be reached")),
        };
        let status = response.status();
        if status == reqwest::StatusCode::NOT_FOUND {
            return Fetched::NotFound;
        }
        if status == reqwest::StatusCode::TOO_MANY_REQUESTS
            || status == reqwest::StatusCode::SERVICE_UNAVAILABLE
        {
            let wait = retry_after(response.headers()).unwrap_or(DEFAULT_HOLD);
            self.hold(&host, wait);
            tracing::info!(%host, seconds = wait.as_secs(), "a dictionary asked to wait");
            return Fetched::Failed(format!(
                "{source} is busy; it's asked again in {} s",
                wait.as_secs().max(1)
            ));
        }
        if !status.is_success() {
            return Fetched::Failed(format!("{source} answered with an error ({status})"));
        }
        let gzipped = response
            .headers()
            .get(reqwest::header::CONTENT_ENCODING)
            .and_then(|value| value.to_str().ok())
            .is_some_and(|value| value.eq_ignore_ascii_case("gzip"));
        let bytes = match read_capped(response).await {
            Some(bytes) => bytes,
            None => return Fetched::Failed(format!("{source}'s answer couldn't be read")),
        };
        let bytes = if gzipped {
            match gunzip(&bytes) {
                Some(bytes) => bytes,
                None => return Fetched::Failed(format!("{source}'s answer couldn't be read")),
            }
        } else {
            bytes
        };
        Fetched::Body(String::from_utf8_lossy(&bytes).into_owned())
    }
}

/// The body, unless it's larger than [`MAX_SENT`].
async fn read_capped(mut response: reqwest::Response) -> Option<Vec<u8>> {
    let mut bytes = Vec::new();
    while let Some(chunk) = response.chunk().await.ok()? {
        bytes.extend_from_slice(&chunk);
        if bytes.len() > MAX_SENT {
            return None;
        }
    }
    Some(bytes)
}

fn gunzip(bytes: &[u8]) -> Option<Vec<u8>> {
    let mut out = Vec::new();
    flate2::read::GzDecoder::new(bytes)
        .take(MAX_UNPACKED as u64 + 1)
        .read_to_end(&mut out)
        .ok()?;
    (out.len() <= MAX_UNPACKED).then_some(out)
}

/// A `Retry-After` header: seconds, or an HTTP date.
pub fn retry_after(headers: &reqwest::header::HeaderMap) -> Option<Duration> {
    let value = headers
        .get(reqwest::header::RETRY_AFTER)?
        .to_str()
        .ok()?
        .trim();
    if let Ok(seconds) = value.parse::<u64>() {
        return Some(Duration::from_secs(seconds));
    }
    let at = chrono::DateTime::parse_from_rfc2822(value).ok()?;
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .ok()?
        .as_secs() as i64;
    Some(Duration::from_secs((at.timestamp() - now).max(1) as u64))
}
