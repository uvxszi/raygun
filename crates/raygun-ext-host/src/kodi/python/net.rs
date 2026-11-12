//! A Kodi add-on's requests, sent by Raygun: public hosts only (the names resolved here, so a
//! public name can't lead to this device or the local network), Offline Mode honoured, bodies
//! capped, and every request within the run's time. The add-on's own headers go as they are
//! (its User-Agent too); cookies are its own business (Python keeps them, as Kodi's do).

use std::net::SocketAddr;
use std::sync::{Arc, OnceLock};
use std::time::Duration;

use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use crate::permissions::{HostPattern, NetPolicy, is_public_ip};

/// Largest body read.
const MAX_BODY_BYTES: usize = 32 * 1024 * 1024;
/// Most redirects followed for one request.
const MAX_REDIRECTS: usize = 10;

/// Resolves names, keeping only public addresses (all of them for tests' loopback hosts).
struct PublicResolver {
    loopback: bool,
}

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        let loopback = self.loopback;
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let found = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let allowed: Vec<SocketAddr> = found
                .filter(|address| {
                    is_public_ip(address.ip()) || (loopback && address.ip().is_loopback())
                })
                .collect();
            if allowed.is_empty() {
                return Err(format!("{host} doesn't resolve to a public address").into());
            }
            Ok(Box::new(allowed.into_iter()) as Addrs)
        })
    }
}

/// Why a request failed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) enum FetchError {
    /// No time left in the run, or the request took longer than allowed.
    Timeout(String),
    /// Anything else: an address Raygun won't reach, no connection, Offline Mode.
    Failed(String),
}

/// One answer.
pub(crate) struct Answer {
    pub status: u16,
    pub url: String,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
}

/// The add-on's client: two of them, one following redirects, made at the first request.
pub(crate) struct Fetcher {
    clients: OnceLock<Result<(reqwest::Client, reqwest::Client, tokio::runtime::Runtime), String>>,
    policy: NetPolicy,
    loopback: bool,
}

impl Fetcher {
    /// `loopback` also lets the add-on reach 127.0.0.1 (tests against a local server).
    pub fn new(loopback: bool) -> Self {
        let mut patterns = vec![HostPattern::AnyPublic];
        if loopback {
            patterns.push(HostPattern::Exact("127.0.0.1".into()));
            patterns.push(HostPattern::Exact("localhost".into()));
        }
        Self {
            clients: OnceLock::new(),
            policy: NetPolicy::new(patterns),
            loopback,
        }
    }

    fn clients(
        &self,
    ) -> Result<&(reqwest::Client, reqwest::Client, tokio::runtime::Runtime), FetchError> {
        self.clients
            .get_or_init(|| {
                let resolver = Arc::new(PublicResolver {
                    loopback: self.loopback,
                });
                let policy = self.policy.clone();
                let redirects = reqwest::redirect::Policy::custom(move |attempt| {
                    if attempt.previous().len() >= MAX_REDIRECTS {
                        attempt.error("too many redirects")
                    } else if policy.allows(attempt.url()) {
                        attempt.follow()
                    } else {
                        attempt.error("a redirect to an address Raygun doesn't reach")
                    }
                });
                let following = raygun_net::client_builder()
                    .dns_resolver(resolver.clone())
                    .redirect(redirects)
                    .build()
                    .map_err(|error| error.to_string())?;
                let staying = raygun_net::client_builder()
                    .dns_resolver(resolver)
                    .redirect(reqwest::redirect::Policy::none())
                    .build()
                    .map_err(|error| error.to_string())?;
                let runtime = tokio::runtime::Builder::new_current_thread()
                    .enable_all()
                    .build()
                    .map_err(|error| error.to_string())?;
                Ok((following, staying, runtime))
            })
            .as_ref()
            .map_err(|error| FetchError::Failed(error.clone()))
    }

    /// Sends one request, waiting at most `timeout`.
    pub fn fetch(
        &self,
        method: &str,
        url: &str,
        headers: &[(String, String)],
        body: Option<Vec<u8>>,
        timeout: Duration,
        follow: bool,
    ) -> Result<Answer, FetchError> {
        let parsed = reqwest::Url::parse(url)
            .map_err(|error| FetchError::Failed(format!("{url:?} isn't a web address: {error}")))?;
        if !matches!(parsed.scheme(), "http" | "https") {
            return Err(FetchError::Failed(format!(
                "{url} isn't a web address Raygun fetches"
            )));
        }
        if !self.policy.allows(&parsed) {
            return Err(FetchError::Failed(format!(
                "{url} is not a public web address"
            )));
        }
        let host = parsed.host_str().unwrap_or_default().to_owned();
        if !raygun_net::network_mode::allows(parsed.as_str()) {
            return Err(FetchError::Failed(
                raygun_net::network_mode::OFFLINE_MESSAGE.to_owned(),
            ));
        }
        if timeout.is_zero() {
            return Err(FetchError::Timeout(format!(
                "no time was left to ask {host}"
            )));
        }
        let method = reqwest::Method::from_bytes(method.to_ascii_uppercase().as_bytes())
            .map_err(|_| FetchError::Failed(format!("unknown method {method}")))?;
        let (following, staying, runtime) = self.clients()?;
        let client = if follow { following } else { staying };
        let mut builder = client.request(method, parsed).timeout(timeout);
        for (name, value) in headers {
            let lower = name.to_ascii_lowercase();
            if matches!(
                lower.as_str(),
                "host"
                    | "content-length"
                    | "connection"
                    | "transfer-encoding"
                    | "keep-alive"
                    | "proxy-connection"
                    | "upgrade"
                    | "te"
            ) {
                continue;
            }
            // Only compression the add-on's Python can undo itself.
            if lower == "accept-encoding" {
                let kept: Vec<&str> = value
                    .split(',')
                    .map(str::trim)
                    .filter(|coding| {
                        let coding = coding.split(';').next().unwrap_or_default().trim();
                        matches!(coding, "gzip" | "deflate" | "identity" | "*")
                    })
                    .collect();
                if !kept.is_empty() {
                    builder = builder.header(name.as_str(), kept.join(", "));
                }
                continue;
            }
            builder = builder.header(name.as_str(), value.as_str());
        }
        if let Some(body) = body {
            builder = builder.body(body);
        }
        runtime.block_on(async {
            let response = builder
                .send()
                .await
                .map_err(|error| describe(&host, &error))?;
            let status = response.status().as_u16();
            let final_url = response.url().to_string();
            let headers: Vec<(String, String)> = response
                .headers()
                .iter()
                .map(|(name, value)| {
                    (
                        name.to_string(),
                        String::from_utf8_lossy(value.as_bytes()).into_owned(),
                    )
                })
                .collect();
            if response
                .content_length()
                .is_some_and(|length| length > MAX_BODY_BYTES as u64)
            {
                return Err(FetchError::Failed(format!(
                    "{final_url} sent more than 32 MiB"
                )));
            }
            let mut response = response;
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| describe(&host, &error))?
            {
                if body.len() + chunk.len() > MAX_BODY_BYTES {
                    return Err(FetchError::Failed(format!(
                        "{final_url} sent more than 32 MiB"
                    )));
                }
                body.extend_from_slice(&chunk);
            }
            Ok(Answer {
                status,
                url: final_url,
                headers,
                body,
            })
        })
    }
}

/// A failed request in a few words: the innermost cause is what helps.
fn describe(host: &str, error: &reqwest::Error) -> FetchError {
    if error.is_timeout() {
        return FetchError::Timeout(format!("{host} didn't answer in time"));
    }
    let mut cause: &dyn std::error::Error = error;
    while let Some(inner) = cause.source() {
        cause = inner;
    }
    let detail = cause.to_string();
    if detail.to_ascii_lowercase().contains("timed out") {
        return FetchError::Timeout(format!("{host} didn't answer in time"));
    }
    FetchError::Failed(format!("could not reach {host}: {detail}"))
}
