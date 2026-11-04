//! What went wrong when a source failed, in words a listener understands.
//!
//! Sources fail all the time: sites go down, change, block, rate limit or time out. Every failure
//! is sorted into an [`IssueKind`], and an [`Issue`] reads as one short sentence naming the source
//! and what happened, then the raw details for "More info". Its text form is that sentence, a line
//! break, then the details: the UI shows the first line and keeps the rest under More info (the
//! same split works for any error text, so one-line errors stay as they are).

use std::fmt;

/// Why a request to a source failed.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum IssueKind {
    /// No route to the internet at all.
    Offline,
    /// The site's name didn't resolve (no connection, or the site is gone).
    Dns,
    /// The site didn't answer in time.
    Timeout,
    /// The secure connection failed: a bad certificate or TLS setup.
    Tls,
    /// The site refused the connection.
    Refused,
    /// The connection broke, or couldn't be made for another reason.
    Unreachable,
    /// A browser check (Cloudflare and the like) answered instead of the site.
    BotCheck,
    /// The site refused the request (401/403 without a check page).
    Denied,
    /// 404 or 410.
    NotFound,
    /// 429: too many requests.
    RateLimited,
    /// 5xx: the site is having problems.
    ServerError,
    /// The site answered, but not in the shape the extension reads: the site changed.
    SiteChanged,
    /// The extension's script failed on its own.
    ExtensionFailed,
    /// A file on this device couldn't be read or written.
    Storage,
    /// This device didn't allow it (a permission).
    Permission,
}

impl IssueKind {
    /// A stable name for the kind (`bot-check`, `rate-limited`...), for logs and the UI.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Offline => "offline",
            Self::Dns => "dns",
            Self::Timeout => "timeout",
            Self::Tls => "tls",
            Self::Refused => "refused",
            Self::Unreachable => "unreachable",
            Self::BotCheck => "bot-check",
            Self::Denied => "denied",
            Self::NotFound => "not-found",
            Self::RateLimited => "rate-limited",
            Self::ServerError => "server-error",
            Self::SiteChanged => "site-changed",
            Self::ExtensionFailed => "extension-failed",
            Self::Storage => "storage",
            Self::Permission => "permission",
        }
    }

    /// The short sentence for this kind of failure of `source` (its name for people).
    pub fn summary(self, source: &str) -> String {
        match self {
            Self::Offline => "No internet connection".to_owned(),
            Self::Dns => format!("Couldn't find {source}'s site — check your connection"),
            Self::Timeout => format!("{source} didn't answer in time"),
            Self::Tls => format!("Couldn't connect securely to {source}"),
            Self::Refused => format!("{source} refused the connection — try later"),
            Self::Unreachable => format!("Couldn't reach {source} — check your connection"),
            Self::BotCheck => format!("{source} wants a browser check, which Raygun can't do"),
            Self::Denied => format!("{source} refused access"),
            Self::NotFound => format!("Not found on {source}"),
            Self::RateLimited => format!("{source} is limiting requests — try later"),
            Self::ServerError => format!("{source} is having problems — try later"),
            Self::SiteChanged => {
                format!("{source} changed its site — its extension needs an update")
            }
            Self::ExtensionFailed => format!("{source}'s extension failed"),
            Self::Storage => "Couldn't read or write a file on this device".to_owned(),
            Self::Permission => "Raygun isn't allowed to do that on this device".to_owned(),
        }
    }

    /// Whether the failure is about the network or the site rather than the extension or this
    /// device: worth trying again later.
    pub fn is_transient(self) -> bool {
        matches!(
            self,
            Self::Offline
                | Self::Dns
                | Self::Timeout
                | Self::Refused
                | Self::Unreachable
                | Self::RateLimited
                | Self::ServerError
        )
    }

    /// How bad this is for picking the cause among several failures of one call: a browser check
    /// explains everything after it, a 404 the least.
    pub fn weight(self) -> u8 {
        match self {
            Self::BotCheck => 9,
            Self::RateLimited => 8,
            Self::Offline | Self::Dns => 7,
            Self::Tls | Self::Refused => 6,
            Self::Timeout | Self::Unreachable => 5,
            Self::Denied => 4,
            Self::ServerError => 3,
            Self::NotFound => 2,
            _ => 1,
        }
    }
}

/// A failure of one source: what kind, the source's name, and the raw details.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Issue {
    pub kind: IssueKind,
    /// The source's name for people ("MangaDex").
    pub source: String,
    /// What exactly failed, for More info; may be empty.
    pub detail: String,
}

impl Issue {
    pub fn new(kind: IssueKind, source: impl Into<String>, detail: impl Into<String>) -> Self {
        Self {
            kind,
            source: source.into(),
            detail: detail.into(),
        }
    }

    pub fn summary(&self) -> String {
        self.kind.summary(&self.source)
    }
}

impl fmt::Display for Issue {
    /// The summary, then the details on the next line.
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter.write_str(&self.summary())?;
        let detail = self.detail.trim();
        if !detail.is_empty() {
            write!(formatter, "\n{detail}")?;
        }
        Ok(())
    }
}

impl std::error::Error for Issue {}

/// A summary and its details as one error text (the form [`Issue`] displays as).
pub fn message(summary: &str, detail: &str) -> String {
    let detail = detail.trim();
    if detail.is_empty() || detail == summary {
        summary.to_owned()
    } else {
        format!("{summary}\n{detail}")
    }
}

/// `source`'s error with its name on the first line: as it is when the error names it already
/// (sources' failures do), else after the name ("Deezer: offline").
pub fn named(source: &str, error: &str) -> String {
    let first = error.lines().next().unwrap_or_default();
    if first.to_lowercase().contains(&source.to_lowercase()) {
        error.to_owned()
    } else {
        format!("{source}: {error}")
    }
}

/// Several sources' errors as one: the first one's sentence leads, everything else follows as
/// details.
pub fn joined(errors: &[String]) -> String {
    errors
        .iter()
        .map(|error| error.trim())
        .filter(|error| !error.is_empty())
        .collect::<Vec<_>>()
        .join("\n")
}

/// The kind of a failed HTTP status (400 and up); `None` for statuses that aren't failures.
pub fn status_kind(status: u16) -> Option<IssueKind> {
    match status {
        401 | 403 | 451 => Some(IssueKind::Denied),
        404 | 410 => Some(IssueKind::NotFound),
        408 | 504 | 522 | 524 => Some(IssueKind::Timeout),
        429 => Some(IssueKind::RateLimited),
        500..=599 => Some(IssueKind::ServerError),
        400..=499 => Some(IssueKind::Denied),
        _ => None,
    }
}

/// How much of a body is searched for a browser check: their pages are small and say so early.
const CHECK_SCAN_BYTES: usize = 16 * 1024;

/// Marks of browser checks and bot walls in a page (lower case).
const CHECK_MARKS: [&str; 16] = [
    "<title>just a moment",
    "challenge-platform",
    "cf-chl-",
    "cf_chl_",
    "attention required! | cloudflare",
    "checking your browser",
    "checking if the site connection is secure",
    "enable javascript and cookies to continue",
    "ddos-guard",
    "captcha-delivery.com",
    "px-captcha",
    "/_incapsula_resource",
    "incapsula incident",
    "sucuri website firewall",
    "verify you are human",
    "g-recaptcha",
];

/// Whether an answer is a browser check rather than the site: Cloudflare's challenge header, or a
/// check page from Cloudflare, DDoS-Guard, DataDome, PerimeterX, Imperva or Sucuri. Only failed
/// answers (403, 429, 503 and the like) are looked at: a site that answers is no check.
pub fn is_bot_check<'a>(
    status: u16,
    mut headers: impl Iterator<Item = (&'a str, &'a str)>,
    body: &str,
) -> bool {
    if status < 400 {
        return false;
    }
    if headers.any(|(name, value)| {
        name.eq_ignore_ascii_case("cf-mitigated") && value.eq_ignore_ascii_case("challenge")
    }) {
        return true;
    }
    if !matches!(status, 403 | 429 | 503) {
        return false;
    }
    let mut end = body.len().min(CHECK_SCAN_BYTES);
    while !body.is_char_boundary(end) {
        end -= 1;
    }
    let start = body[..end].to_ascii_lowercase();
    CHECK_MARKS.iter().any(|mark| start.contains(mark))
}

/// The kind of a failed HTTP answer, with browser checks told apart.
pub fn answer_kind<'a>(
    status: u16,
    headers: impl Iterator<Item = (&'a str, &'a str)>,
    body: &str,
) -> Option<IssueKind> {
    if is_bot_check(status, headers, body) {
        return Some(IssueKind::BotCheck);
    }
    status_kind(status)
}

/// The kind of an I/O failure.
pub fn io_kind(error: &std::io::Error) -> IssueKind {
    use std::io::ErrorKind;
    match error.kind() {
        ErrorKind::NetworkUnreachable | ErrorKind::NetworkDown => IssueKind::Offline,
        ErrorKind::HostUnreachable => IssueKind::Unreachable,
        ErrorKind::ConnectionRefused => IssueKind::Refused,
        ErrorKind::TimedOut => IssueKind::Timeout,
        ErrorKind::ConnectionReset
        | ErrorKind::ConnectionAborted
        | ErrorKind::BrokenPipe
        | ErrorKind::UnexpectedEof
        | ErrorKind::NotConnected
        | ErrorKind::AddrNotAvailable => IssueKind::Unreachable,
        ErrorKind::PermissionDenied => IssueKind::Permission,
        ErrorKind::NotFound
        | ErrorKind::AlreadyExists
        | ErrorKind::StorageFull
        | ErrorKind::QuotaExceeded
        | ErrorKind::ReadOnlyFilesystem
        | ErrorKind::IsADirectory
        | ErrorKind::NotADirectory
        | ErrorKind::DirectoryNotEmpty
        | ErrorKind::FileTooLarge
        | ErrorKind::InvalidFilename => IssueKind::Storage,
        _ => match error.raw_os_error() {
            // WSAENETUNREACH, WSAENETDOWN, ENETUNREACH, ENETDOWN.
            Some(10051 | 10050 | 101 | 100) => IssueKind::Offline,
            // WSAHOST_NOT_FOUND, WSANO_DATA, WSATRY_AGAIN.
            Some(11001 | 11004 | 11002) => IssueKind::Dns,
            _ => IssueKind::Unreachable,
        },
    }
}

/// Words lookups fail with, on every platform (lower case).
const DNS_MARKS: [&str; 8] = [
    "dns error",
    "failed to lookup address",
    "no such host is known",
    "name or service not known",
    "nodename nor servname",
    "no address associated with hostname",
    "temporary failure in name resolution",
    "name resolution",
];

/// The kind of a failed request: the network's (offline, lookup, timeout, TLS, refused) as far
/// as the error's causes tell.
pub fn request_kind(error: &reqwest::Error) -> IssueKind {
    if error.is_timeout() {
        return IssueKind::Timeout;
    }
    if let Some(status) = error.status()
        && let Some(kind) = status_kind(status.as_u16())
    {
        return kind;
    }
    let mut found = None;
    let mut cause: Option<&(dyn std::error::Error + 'static)> = Some(error);
    while let Some(current) = cause {
        let text = current.to_string().to_ascii_lowercase();
        if DNS_MARKS.iter().any(|mark| text.contains(mark)) {
            return IssueKind::Dns;
        }
        if current.downcast_ref::<rustls::Error>().is_some()
            || text.contains("certificate")
            || text.contains("tls handshake")
            || text.contains("handshake failure")
        {
            return IssueKind::Tls;
        }
        if let Some(io) = current.downcast_ref::<std::io::Error>() {
            // An I/O error wrapping a TLS error says so in its inner cause; keep looking.
            found = Some(io_kind(io));
        }
        cause = current.source();
    }
    if let Some(kind) = found {
        return kind;
    }
    if error.is_decode() || error.is_body() {
        return IssueKind::Unreachable;
    }
    IssueKind::Unreachable
}

/// The innermost cause of a failed request, which says what happened ("operation timed out", "No
/// such host is known"); reqwest's own message repeats the whole address.
pub fn request_detail(host: &str, error: &reqwest::Error) -> String {
    if error.is_timeout() {
        return format!("{host} didn't answer in time");
    }
    let mut cause: &dyn std::error::Error = error;
    while let Some(inner) = cause.source() {
        cause = inner;
    }
    format!("could not reach {host}: {cause}")
}

/// A failed request to `source` (its name) at `host`, as an issue.
pub fn request_issue(source: &str, host: &str, error: &reqwest::Error) -> Issue {
    Issue::new(request_kind(error), source, request_detail(host, error))
}

/// A failed answer from `source`, as an issue; `None` when the status isn't a failure.
pub fn answer_issue(source: &str, host: &str, status: u16, body: &str) -> Option<Issue> {
    let kind = answer_kind(status, std::iter::empty(), body)?;
    Some(Issue::new(kind, source, answer_detail(host, status, kind)))
}

/// The details of a failed answer: "mangadex.org answered HTTP 403 (a browser check)".
pub fn answer_detail(host: &str, status: u16, kind: IssueKind) -> String {
    if kind == IssueKind::BotCheck {
        format!("{host} answered HTTP {status} with a browser check")
    } else {
        format!("{host} answered HTTP {status}")
    }
}

/// The kind an error's text points at, for errors that only come as text (an extension's thrown
/// message, another service's error): a status it names, a timeout, a lookup, a refused
/// connection, a script error. `None` when the text is a plain sentence that says it all.
pub fn text_kind(text: &str) -> Option<IssueKind> {
    let lower = text.to_ascii_lowercase();
    if CHECK_MARKS.iter().any(|mark| lower.contains(mark))
        || lower.contains("cloudflare")
        || lower.contains("captcha")
        || lower.contains("browser check")
    {
        return Some(IssueKind::BotCheck);
    }
    if let Some(status) = named_status(&lower) {
        return status_kind(status);
    }
    if lower.contains("rate limit")
        || lower.contains("too many requests")
        || lower.contains("rate-limit")
    {
        return Some(IssueKind::RateLimited);
    }
    if DNS_MARKS.iter().any(|mark| lower.contains(mark)) {
        return Some(IssueKind::Dns);
    }
    if lower.contains("timed out")
        || lower.contains("timeout")
        || lower.contains("didn't answer in time")
        || lower.contains("did not answer in time")
    {
        return Some(IssueKind::Timeout);
    }
    if lower.contains("connection refused") || lower.contains("actively refused") {
        return Some(IssueKind::Refused);
    }
    if lower.contains("certificate") || lower.contains("tls ") || lower.contains("ssl ") {
        return Some(IssueKind::Tls);
    }
    if lower.contains("network is unreachable") || lower.contains("network unreachable") {
        return Some(IssueKind::Offline);
    }
    if lower.contains("error sending request")
        || lower.contains("could not reach")
        || lower.contains("connection reset")
        || lower.contains("connection closed")
        || lower.contains("error trying to connect")
    {
        return Some(IssueKind::Unreachable);
    }
    if is_script_error(text) {
        return Some(IssueKind::ExtensionFailed);
    }
    None
}

/// An HTTP status an error text names: "HTTP 404", "answered 503", "status 429", "(500)".
fn named_status(lower: &str) -> Option<u16> {
    const BEFORE: [&str; 7] = [
        "http ",
        "http/",
        "answered ",
        "status ",
        "status: ",
        "status code ",
        "code ",
    ];
    for marker in BEFORE {
        let mut rest = lower;
        while let Some(at) = rest.find(marker) {
            let after = &rest[at + marker.len()..];
            // "HTTP/1.1 404": skip the version.
            let after = after
                .strip_prefix("1.1 ")
                .or_else(|| after.strip_prefix("2 "))
                .unwrap_or(after);
            if let Some(status) = leading_status(after) {
                return Some(status);
            }
            rest = &rest[at + marker.len()..];
        }
    }
    None
}

/// Three digits at the start of `text` that make an error status (400–599), not followed by
/// another digit.
fn leading_status(text: &str) -> Option<u16> {
    let digits: String = text.chars().take_while(char::is_ascii_digit).collect();
    if digits.len() != 3 {
        return None;
    }
    let status: u16 = digits.parse().ok()?;
    (400..600).contains(&status).then_some(status)
}

/// JavaScript's and Lua's own errors (a script reading something that isn't there), as opposed
/// to a message the extension wrote for people.
pub fn is_script_error(text: &str) -> bool {
    const MARKS: [&str; 18] = [
        "TypeError",
        "ReferenceError",
        "SyntaxError",
        "RangeError",
        "InternalError",
        "is not a function",
        "is not defined",
        "is not an object",
        "is not iterable",
        "cannot read property",
        "Cannot read property",
        "cannot read properties",
        "Cannot read properties",
        "of undefined",
        "of null",
        "unexpected token",
        "Unexpected token",
        "attempt to index",
    ];
    MARKS.iter().any(|mark| text.contains(mark))
        || text.contains("attempt to call")
        || text.contains("stack overflow")
        || text.contains("out of memory")
        || text.contains("interrupted")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn issues_read_as_a_sentence_then_the_details() {
        let issue = Issue::new(
            IssueKind::BotCheck,
            "MangaDex",
            "mangadex.org answered HTTP 403 with a browser check",
        );
        assert_eq!(
            issue.to_string(),
            "MangaDex wants a browser check, which Raygun can't do\nmangadex.org answered HTTP 403 with a browser check"
        );
        let plain = Issue::new(IssueKind::Timeout, "Deezer", "  ");
        assert_eq!(plain.to_string(), "Deezer didn't answer in time");
        assert_eq!(message("Same", "Same"), "Same");
        assert_eq!(message("Short", "long"), "Short\nlong");
        // Errors name their source once.
        assert_eq!(
            named("Deezer", &plain.to_string()),
            "Deezer didn't answer in time"
        );
        assert_eq!(named("Deezer", "offline"), "Deezer: offline");
        assert_eq!(
            joined(&["A failed\nwhy".into(), " ".into(), "B: offline".into()]),
            "A failed\nwhy\nB: offline"
        );
    }

    #[test]
    fn statuses_map_to_kinds() {
        assert_eq!(status_kind(200), None);
        assert_eq!(status_kind(304), None);
        assert_eq!(status_kind(404), Some(IssueKind::NotFound));
        assert_eq!(status_kind(410), Some(IssueKind::NotFound));
        assert_eq!(status_kind(429), Some(IssueKind::RateLimited));
        assert_eq!(status_kind(403), Some(IssueKind::Denied));
        assert_eq!(status_kind(502), Some(IssueKind::ServerError));
        assert_eq!(status_kind(504), Some(IssueKind::Timeout));
        assert_eq!(status_kind(418), Some(IssueKind::Denied));
    }

    #[test]
    fn browser_checks_are_told_from_plain_refusals() {
        let cloudflare = r#"<!DOCTYPE html><html lang="en-US"><head><title>Just a moment...</title>
            <script src="/cdn-cgi/challenge-platform/h/b/orchestrate/chl_page/v1"></script>"#;
        assert!(is_bot_check(403, std::iter::empty(), cloudflare));
        assert!(is_bot_check(503, std::iter::empty(), cloudflare));
        assert!(is_bot_check(
            403,
            [("CF-Mitigated", "challenge")].into_iter(),
            "",
        ));
        assert!(is_bot_check(
            403,
            std::iter::empty(),
            "<html><title>DDoS-Guard</title></html>"
        ));
        // A site that answers is no check, and a plain 403 is a refusal.
        assert!(!is_bot_check(200, std::iter::empty(), cloudflare));
        assert!(!is_bot_check(403, std::iter::empty(), "Forbidden"));
        assert_eq!(
            answer_kind(403, std::iter::empty(), "Forbidden"),
            Some(IssueKind::Denied)
        );
        assert_eq!(
            answer_kind(403, std::iter::empty(), cloudflare),
            Some(IssueKind::BotCheck)
        );
        // A page cut mid-character still reads.
        let long = format!("{}é", "a".repeat(CHECK_SCAN_BYTES - 1));
        assert!(!is_bot_check(403, std::iter::empty(), &long));
    }

    #[test]
    fn io_errors_map_to_kinds() {
        use std::io::{Error, ErrorKind};
        assert_eq!(
            io_kind(&Error::from(ErrorKind::ConnectionRefused)),
            IssueKind::Refused
        );
        assert_eq!(
            io_kind(&Error::from(ErrorKind::NetworkUnreachable)),
            IssueKind::Offline
        );
        assert_eq!(
            io_kind(&Error::from(ErrorKind::TimedOut)),
            IssueKind::Timeout
        );
        assert_eq!(
            io_kind(&Error::from(ErrorKind::PermissionDenied)),
            IssueKind::Permission
        );
        assert_eq!(
            io_kind(&Error::from(ErrorKind::StorageFull)),
            IssueKind::Storage
        );
        assert_eq!(io_kind(&Error::from_raw_os_error(11001)), IssueKind::Dns);
    }

    #[test]
    fn error_texts_point_at_kinds() {
        let kind = |text: &str| text_kind(text);
        assert_eq!(
            kind("Deezer answered HTTP 503"),
            Some(IssueKind::ServerError)
        );
        assert_eq!(
            kind("HTTP 404 from https://x.example/a"),
            Some(IssueKind::NotFound)
        );
        assert_eq!(
            kind("The server answered 429 Too Many Requests"),
            Some(IssueKind::RateLimited)
        );
        assert_eq!(
            kind("MangaDex is rate limiting requests; try again shortly"),
            Some(IssueKind::RateLimited)
        );
        assert_eq!(
            kind("could not reach api.example: dns error: failed to lookup address information"),
            Some(IssueKind::Dns)
        );
        assert_eq!(
            kind("api.example didn't answer in time"),
            Some(IssueKind::Timeout)
        );
        assert_eq!(
            kind("TypeError: cannot read property 'data' of undefined"),
            Some(IssueKind::ExtensionFailed)
        );
        assert_eq!(
            kind("Just a moment... Cloudflare"),
            Some(IssueKind::BotCheck)
        );
        // Plain sentences, and numbers that aren't statuses, say it all.
        assert_eq!(kind("This chapter has no pages"), None);
        assert_eq!(kind("Unknown chart 4040"), None);
        assert_eq!(kind("answered 4041 times"), None);
        assert_eq!(
            kind("HTTP/1.1 502 Bad Gateway"),
            Some(IssueKind::ServerError)
        );
    }

    #[tokio::test]
    async fn refused_and_unknown_hosts_are_told_apart() {
        let client = crate::http_client().unwrap();
        // A port nothing listens on.
        let listener = std::net::TcpListener::bind("127.0.0.1:0").unwrap();
        let port = listener.local_addr().unwrap().port();
        drop(listener);
        let refused = client
            .get(format!("http://127.0.0.1:{port}/"))
            .send()
            .await
            .unwrap_err();
        assert_eq!(request_kind(&refused), IssueKind::Refused, "{refused:?}");
        let detail = request_detail("127.0.0.1", &refused);
        assert!(
            detail.starts_with("could not reach 127.0.0.1: "),
            "{detail}"
        );

        let unknown = client
            .get("http://raygun-no-such-host.invalid/")
            .send()
            .await
            .unwrap_err();
        assert_eq!(request_kind(&unknown), IssueKind::Dns, "{unknown:?}");
    }
}
