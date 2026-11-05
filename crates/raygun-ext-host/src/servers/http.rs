//! Requests to a connected server: its own client (redirects only within the server's host),
//! bounded answers, and failures in the words of `raygun_net::issue`, named after the server.

use std::time::Duration;

use raygun_net::issue::{self, IssueKind};
use reqwest::Url;
use serde::de::DeserializeOwned;

use crate::failure::{Failure, Trouble};

/// Longest a request to a server may take.
const REQUEST_TIMEOUT: Duration = Duration::from_secs(20);
/// Largest answer read whole (feeds, JSON lists).
pub(crate) const MAX_ANSWER_BYTES: usize = 8 * 1024 * 1024;
const MAX_REDIRECTS: usize = 10;

/// A server's answer, read whole.
#[derive(Debug, Clone)]
pub(crate) struct Answer {
    /// Where the answer came from, after redirects.
    pub url: Url,
    pub content_type: String,
    pub body: Vec<u8>,
}

impl Answer {
    pub fn text(&self) -> String {
        String::from_utf8_lossy(&self.body).into_owned()
    }
}

/// The client for one server.
#[derive(Clone)]
pub(crate) struct Http {
    client: reqwest::Client,
    host: String,
}

impl Http {
    /// A client that follows redirects only within `base`'s host.
    pub fn new(base: &Url) -> Result<Self, String> {
        Self::build(base, false)
    }

    /// A client that also follows redirects to public sites: catalogues link to their mirrors
    /// (Project Gutenberg's search answers from `m.gutenberg.org`). Redirects elsewhere drop the
    /// sign-in.
    pub fn open(base: &Url) -> Result<Self, String> {
        Self::build(base, true)
    }

    fn build(base: &Url, open: bool) -> Result<Self, String> {
        let host = base.host_str().unwrap_or_default().to_ascii_lowercase();
        let allowed = host.clone();
        let redirects = reqwest::redirect::Policy::custom(move |attempt| {
            let target = attempt
                .url()
                .host_str()
                .unwrap_or_default()
                .to_ascii_lowercase();
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else if target == allowed || (open && crate::permissions::is_public_host(&target)) {
                attempt.follow()
            } else {
                let elsewhere = attempt.url().host_str().unwrap_or_default().to_owned();
                attempt.error(format!(
                    "the server sent Raygun to another site ({elsewhere})"
                ))
            }
        });
        let client = raygun_net::client_builder()
            .redirect(redirects)
            .timeout(REQUEST_TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self { client, host })
    }

    pub fn client(&self) -> &reqwest::Client {
        &self.client
    }

    /// Sends a request and reads at most `limit` bytes of its answer. A failed status is a
    /// failure, with what kind of trouble it was.
    pub async fn send(
        &self,
        request: reqwest::RequestBuilder,
        limit: usize,
    ) -> Result<Answer, Failure> {
        let host = self.host.clone();
        let exchange = async move {
            let mut response = request
                .send()
                .await
                .map_err(|error| request_failure(&host, &error))?;
            let status = response.status().as_u16();
            let url = response.url().clone();
            let content_type = response
                .headers()
                .get(reqwest::header::CONTENT_TYPE)
                .and_then(|value| value.to_str().ok())
                .unwrap_or_default()
                .to_ascii_lowercase();
            let too_large = || Failure {
                message: format!("{host} sent more than {} MB", limit / (1024 * 1024)),
                trouble: None,
                answered: true,
                checked: None,
            };
            if response
                .content_length()
                .is_some_and(|length| length > limit as u64)
            {
                return Err(too_large());
            }
            let mut body = Vec::new();
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| request_failure(&host, &error))?
            {
                if body.len() + chunk.len() > limit {
                    return Err(too_large());
                }
                body.extend_from_slice(&chunk);
            }
            if let Some(kind) = issue::status_kind(status) {
                return Err(Failure {
                    message: issue::answer_detail(&host, status, kind),
                    trouble: Some(Trouble {
                        kind,
                        detail: issue::answer_detail(&host, status, kind),
                    }),
                    answered: false,
                    checked: None,
                });
            }
            Ok(Answer {
                url,
                content_type,
                body,
            })
        };
        // Requests run on the extensions' network runtime, so a lane busy with script elsewhere
        // doesn't hold them up.
        match crate::js::network() {
            Some(network) => network
                .spawn(exchange)
                .await
                .unwrap_or_else(|_| Err(Failure::said("the request stopped before it finished"))),
            None => exchange.await,
        }
    }

    /// A JSON answer of type `T`; an answer of another shape says so.
    pub async fn json<T: DeserializeOwned>(
        &self,
        request: reqwest::RequestBuilder,
    ) -> Result<T, Failure> {
        let answer = self.send(request, MAX_ANSWER_BYTES).await?;
        serde_json::from_slice(&answer.body).map_err(|error| unreadable(&self.host, &error))
    }
}

/// A failure for an answer Raygun couldn't read.
pub(crate) fn unreadable(host: &str, error: &dyn std::fmt::Display) -> Failure {
    Failure {
        message: format!("{host} sent an answer Raygun can't read: {error}"),
        trouble: Some(Trouble {
            kind: IssueKind::SiteChanged,
            detail: format!("{host} sent an answer Raygun can't read: {error}"),
        }),
        answered: true,
        checked: None,
    }
}

fn request_failure(host: &str, error: &reqwest::Error) -> Failure {
    let detail = issue::request_detail(host, error);
    Failure {
        message: detail.clone(),
        trouble: Some(Trouble {
            kind: issue::request_kind(error),
            detail,
        }),
        answered: false,
        checked: None,
    }
}

/// Whether a failure is the server refusing the sign-in (401 or 403).
pub(crate) fn is_denied(failure: &Failure) -> bool {
    failure
        .trouble
        .as_ref()
        .is_some_and(|trouble| trouble.kind == IssueKind::Denied)
}

/// The HTTP status a failed answer had (as [`Http::send`] words it).
pub(crate) fn status_of(failure: &Failure) -> Option<u16> {
    let detail = &failure.trouble.as_ref()?.detail;
    let start = detail.find("answered HTTP ")? + "answered HTTP ".len();
    detail[start..].split_whitespace().next()?.parse().ok()
}

/// A call's argument as text: strings as they are, numbers as written.
pub(crate) fn text_arg(args: &serde_json::Value, key: &str) -> Option<String> {
    match args.get(key)? {
        serde_json::Value::String(text) => {
            let text = text.trim();
            (!text.is_empty()).then(|| text.to_owned())
        }
        serde_json::Value::Number(number) => Some(number.to_string()),
        _ => None,
    }
}

/// A call's numeric argument, within `range`, else `default`.
pub(crate) fn number_arg(
    args: &serde_json::Value,
    key: &str,
    default: u32,
    range: std::ops::RangeInclusive<u32>,
) -> u32 {
    args.get(key)
        .and_then(serde_json::Value::as_f64)
        .filter(|value| value.is_finite())
        .map_or(default, |value| {
            (value.max(0.0) as u32).clamp(*range.start(), *range.end())
        })
}

/// The id a call names, or a failure saying it's missing.
pub(crate) fn id_arg(args: &serde_json::Value, key: &str) -> Result<String, Failure> {
    text_arg(args, key).ok_or_else(|| Failure::said(format!("the call names no {key}")))
}

/// Unix milliseconds of an RFC 3339 date (`2010-06-16T00:00:00.0000000Z`); none for the
/// placeholder dates servers write for "unknown" (year 1).
pub(crate) fn date_ms(text: &str) -> Option<i64> {
    let parsed = chrono::DateTime::parse_from_rfc3339(text.trim()).ok()?;
    let ms = parsed.timestamp_millis();
    (parsed.format("%Y").to_string().parse::<i32>().ok()? > 1800).then_some(ms)
}

/// Text as one path segment: unreserved characters as they are, everything else
/// percent-encoded, so nothing a server sends (an id) can change the path.
pub(crate) fn segment(text: &str) -> String {
    let mut encoded = String::with_capacity(text.len());
    for byte in text.bytes() {
        if byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'.' | b'_' | b'~') {
            encoded.push(char::from(byte));
        } else {
            encoded.push_str(&format!("%{byte:02X}"));
        }
    }
    encoded
}

/// `base` joined with a path under it: `https://host/jellyfin` + `Items` is
/// `https://host/jellyfin/Items`.
pub(crate) fn under(base: &Url, path: &str) -> Url {
    let mut url = base.clone();
    let joined = format!(
        "{}/{}",
        base.path().trim_end_matches('/'),
        path.trim_start_matches('/')
    );
    url.set_path(&joined);
    url.set_query(None);
    url
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn paths_join_under_the_base() {
        let base: Url = "https://media.example.org/jellyfin/".parse().unwrap();
        assert_eq!(
            under(&base, "/Items").as_str(),
            "https://media.example.org/jellyfin/Items"
        );
        let root: Url = "http://10.0.0.5:4533".parse().unwrap();
        assert_eq!(
            under(&root, "rest/ping.view").as_str(),
            "http://10.0.0.5:4533/rest/ping.view"
        );
    }

    #[test]
    fn dates_skip_placeholders() {
        assert_eq!(date_ms("1970-01-01T00:00:01Z"), Some(1000));
        assert_eq!(date_ms("0001-01-01T00:00:00.0000000Z"), None);
        assert!(date_ms("2010-06-16T00:00:00.0000000Z").is_some());
        assert_eq!(date_ms("soon"), None);
    }

    #[test]
    fn arguments_are_read_leniently() {
        let args = serde_json::json!({ "id": 42, "query": "  ", "page": 3.7, "limit": 1e9 });
        assert_eq!(text_arg(&args, "id").as_deref(), Some("42"));
        assert_eq!(text_arg(&args, "query"), None);
        assert_eq!(number_arg(&args, "page", 1, 1..=1000), 3);
        assert_eq!(number_arg(&args, "limit", 20, 1..=100), 100);
        assert_eq!(number_arg(&args, "missing", 20, 1..=100), 20);
        assert!(id_arg(&args, "chapterId").is_err());
    }
}
