//! HTTP for Shosetsu scripts (M32): the OkHttp-shaped API kotlin-lib (GPL-3.0) gives scripts
//! (`GET`, `POST`, `Request`, `RequestDocument`, `GETDocument`, builders for headers, form bodies,
//! requests and URLs, the cookie jar), run as blocking calls on the script's worker thread.
//! Scripts may reach any public host, never this machine or the local network: names and
//! redirects are checked, and the resolver drops private answers.
//!
//! Bodies are decoded as OkHttp's `body().string()` does: a byte-order mark, else the
//! Content-Type's charset, else UTF-8. A bot check (Cloudflare's challenge, a captcha page) that
//! answers in a site's place is noted, so the script's failure can say so.

use std::fmt;
use std::net::SocketAddr;
use std::sync::{Arc, Mutex};
use std::time::Duration;

use mlua::{AnyUserData, Lua, MetaMethod, Table, UserData, UserDataMethods, Value};
use reqwest::cookie::CookieStore;
use reqwest::dns::{Addrs, Name, Resolve, Resolving};

use super::dom::{LuaNode, parse_document};
use crate::cookies::CookieJar;
use crate::permissions::{HostPattern, NetPolicy, is_public_ip};

/// Response bodies larger than this fail the request.
const MAX_BODY_BYTES: usize = 16 * 1024 * 1024;
const TIMEOUT: Duration = Duration::from_secs(20);
const MAX_REDIRECTS: usize = 10;
/// How much of an error page is looked at for a bot check.
const SNIFF_BYTES: usize = 64 * 1024;

/// Resolves names, keeping only public addresses.
struct PublicResolver;

impl Resolve for PublicResolver {
    fn resolve(&self, name: Name) -> Resolving {
        Box::pin(async move {
            let host = name.as_str().to_owned();
            let found = tokio::net::lookup_host((host.as_str(), 0)).await?;
            let public: Vec<SocketAddr> =
                found.filter(|address| is_public_ip(address.ip())).collect();
            if public.is_empty() {
                return Err(format!("{host} doesn't resolve to a public address").into());
            }
            Ok(Box::new(public.into_iter()) as Addrs)
        })
    }
}

/// Why a request failed, kept typed so the script's failure can be explained in plain words.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum FetchError {
    /// A bot check (Cloudflare's challenge, a captcha) answered in the site's place.
    BrowserCheck { host: String, status: u16 },
    /// The site answered with an error status.
    Status { status: u16, url: String },
    Timeout { host: String },
    /// No answer at all: the name doesn't resolve, the connection or TLS failed.
    Unreachable { host: String, detail: String },
    TooLarge { url: String },
    /// This machine or the local network, which scripts may not reach.
    NotPublic { url: String },
    BadUrl { url: String, detail: String },
}

impl fmt::Display for FetchError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::BrowserCheck { host, status } => {
                write!(f, "HTTP {status} from {host}: a bot check page")
            }
            Self::Status { status, url } => write!(f, "HTTP {status} from {url}"),
            Self::Timeout { host } => write!(f, "{host} didn't answer in time"),
            Self::Unreachable { host, detail } => write!(f, "could not reach {host}: {detail}"),
            Self::TooLarge { url } => write!(f, "{url} sent more than 16 MiB"),
            Self::NotPublic { url } => write!(f, "{url} is not a public web address"),
            Self::BadUrl { url, detail } => write!(f, "invalid URL {url:?}: {detail}"),
        }
    }
}

impl std::error::Error for FetchError {}

/// Runs a script's requests: its own small runtime, since the script waits for each answer.
pub struct Fetcher {
    client: reqwest::Client,
    runtime: tokio::runtime::Runtime,
    policy: NetPolicy,
    cookies: Arc<CookieJar>,
    /// The host of the last bot check seen since [`Fetcher::start_call`].
    challenged: Mutex<Option<String>>,
}

impl Fetcher {
    /// `loopback` also allows 127.0.0.1, for tests against a local server. The script's cookies
    /// live in `cookies` (M46.11).
    pub fn new(loopback: bool, cookies: Arc<CookieJar>) -> Result<Self, String> {
        let mut patterns = vec![HostPattern::AnyPublic];
        if loopback {
            patterns.push(HostPattern::Exact("127.0.0.1".into()));
        }
        let policy = NetPolicy::new(patterns);
        let redirect_policy = policy.clone();
        let redirects = reqwest::redirect::Policy::custom(move |attempt| {
            if attempt.previous().len() >= MAX_REDIRECTS {
                attempt.error("too many redirects")
            } else if redirect_policy.allows(attempt.url()) {
                attempt.follow()
            } else {
                attempt.error("redirect to a local address")
            }
        });
        let client = raygun_net::client_builder()
            .dns_resolver(Arc::new(PublicResolver))
            .redirect(redirects)
            .cookie_provider(cookies.clone())
            .timeout(TIMEOUT)
            .build()
            .map_err(|error| error.to_string())?;
        let runtime = tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .map_err(|error| error.to_string())?;
        Ok(Self {
            client,
            runtime,
            policy,
            cookies,
            challenged: Mutex::new(None),
        })
    }

    /// Forgets the bot checks seen so far: a new call starts.
    pub fn start_call(&self) {
        if let Ok(mut challenged) = self.challenged.lock() {
            *challenged = None;
        }
    }

    /// The host of a bot check seen during the current call.
    pub fn challenged(&self) -> Option<String> {
        self.challenged.lock().ok()?.clone()
    }

    pub fn execute(&self, request: &LuaRequest) -> Result<LuaResponse, FetchError> {
        let url = reqwest::Url::parse(&request.url).map_err(|error| FetchError::BadUrl {
            url: request.url.clone(),
            detail: error.to_string(),
        })?;
        if !self.policy.allows(&url) {
            return Err(FetchError::NotPublic {
                url: url.to_string(),
            });
        }
        let host = url.host_str().unwrap_or_default().to_owned();
        let method = reqwest::Method::from_bytes(request.method.to_ascii_uppercase().as_bytes())
            .map_err(|_| FetchError::BadUrl {
                url: url.to_string(),
                detail: format!("unknown method {}", request.method),
            })?;
        let mut builder = self.client.request(method, url.clone());
        let mut has_agent = false;
        let mut has_type = false;
        for (name, value) in &request.headers {
            has_agent |= name.eq_ignore_ascii_case("user-agent");
            has_type |= name.eq_ignore_ascii_case("content-type");
            builder = builder.header(name, value);
        }
        if !has_agent {
            builder = builder.header("User-Agent", raygun_net::user_agent());
        }
        if let Some(body) = &request.body {
            if !has_type && !body.content_type.is_empty() {
                builder = builder.header("Content-Type", &body.content_type);
            }
            builder = builder.body(body.bytes.clone());
        }
        #[cfg(test)]
        if std::env::var_os("SHOSETSU_TRACE").is_some() {
            eprintln!("  > {} {url}", request.method);
        }
        let response = self.runtime.block_on(async {
            let response = builder.send().await.map_err(|error| describe(&host, &error))?;
            let status = response.status().as_u16();
            let final_url = response.url().to_string();
            let headers: Vec<(String, String)> = response
                .headers()
                .iter()
                .filter_map(|(name, value)| {
                    Some((name.to_string(), value.to_str().ok()?.to_owned()))
                })
                .collect();
            if response
                .content_length()
                .is_some_and(|length| length > MAX_BODY_BYTES as u64)
            {
                return Err(FetchError::TooLarge { url: final_url });
            }
            let mut body = Vec::new();
            let mut response = response;
            while let Some(chunk) = response
                .chunk()
                .await
                .map_err(|error| describe(&host, &error))?
            {
                if body.len() + chunk.len() > MAX_BODY_BYTES {
                    return Err(FetchError::TooLarge { url: final_url });
                }
                body.extend_from_slice(&chunk);
            }
            Ok(LuaResponse {
                status,
                request: LuaRequest {
                    url: final_url,
                    ..request.clone()
                },
                headers,
                body: Arc::new(body),
            })
        })?;
        #[cfg(test)]
        if std::env::var_os("SHOSETSU_TRACE").is_some() {
            eprintln!(
                "  < {} {} ({} bytes)",
                response.status,
                response.request.url,
                response.body.len()
            );
        }
        if bot_check(response.status, &response.headers, &response.body) {
            tracing::debug!(%host, status = response.status, "a bot check answered");
            if let Ok(mut challenged) = self.challenged.lock() {
                *challenged = Some(host);
            }
        }
        Ok(response)
    }

    /// The cookies this script's jar would send to `url`, as name and value.
    fn cookies_for(&self, url: &url::Url) -> Vec<(String, String)> {
        let Some(header) = self.cookies.cookies(url) else {
            return Vec::new();
        };
        header
            .to_str()
            .unwrap_or_default()
            .split(';')
            .filter_map(|pair| {
                let (name, value) = pair.trim().split_once('=')?;
                Some((name.trim().to_owned(), value.trim().to_owned()))
            })
            .collect()
    }

    /// Keeps cookies in this script's jar, as `Set-Cookie` lines for `url`.
    fn save_cookies(&self, url: &url::Url, lines: &[String]) {
        let values: Vec<reqwest::header::HeaderValue> = lines
            .iter()
            .filter_map(|line| reqwest::header::HeaderValue::from_str(line).ok())
            .collect();
        self.cookies.set_cookies(&mut values.iter(), url);
    }
}

/// A request that failed outright, in a few words: the innermost cause ("operation timed out",
/// a refused address) is what a user needs; reqwest's own message repeats the whole URL.
fn describe(host: &str, error: &reqwest::Error) -> FetchError {
    if error.is_timeout() {
        return FetchError::Timeout {
            host: host.to_owned(),
        };
    }
    let mut cause: &dyn std::error::Error = error;
    while let Some(inner) = cause.source() {
        cause = inner;
    }
    let detail = cause.to_string();
    if detail.to_ascii_lowercase().contains("timed out") {
        return FetchError::Timeout {
            host: host.to_owned(),
        };
    }
    FetchError::Unreachable {
        host: host.to_owned(),
        detail,
    }
}

fn header<'a>(headers: &'a [(String, String)], name: &str) -> Option<&'a str> {
    headers
        .iter()
        .find(|(key, _)| key.eq_ignore_ascii_case(name))
        .map(|(_, value)| value.as_str())
}

/// Whether a bot check (Cloudflare's challenge, DDoS-Guard, DataDome, a captcha page) answered
/// in the site's place. Raygun can't pass these, and doesn't try to.
pub fn bot_check(status: u16, headers: &[(String, String)], body: &[u8]) -> bool {
    if header(headers, "cf-mitigated").is_some_and(|value| value.eq_ignore_ascii_case("challenge"))
    {
        return true;
    }
    if !matches!(status, 401 | 403 | 429 | 503) {
        return false;
    }
    if header(headers, "x-datadome").is_some() {
        return true;
    }
    let server = header(headers, "server")
        .unwrap_or_default()
        .to_ascii_lowercase();
    let page = String::from_utf8_lossy(&body[..body.len().min(SNIFF_BYTES)]).to_ascii_lowercase();
    let cloudflare = server.contains("cloudflare")
        && [
            "challenge-platform",
            "cf_chl",
            "cf-chl",
            "just a moment",
            "cf-turnstile",
            "cdn-cgi/challenge",
        ]
        .iter()
        .any(|marker| page.contains(marker));
    let others = server.contains("ddos-guard")
        || [
            "captcha-delivery.com",
            "sucuri website firewall",
            "px-captcha",
            "g-recaptcha",
            "h-captcha",
            "cf-turnstile",
        ]
        .iter()
        .any(|marker| page.contains(marker));
    cloudflare || others
}

/// The charset a `Content-Type` names.
fn charset(content_type: &str) -> Option<&str> {
    content_type.split(';').skip(1).find_map(|part| {
        let (key, value) = part.split_once('=')?;
        key.trim()
            .eq_ignore_ascii_case("charset")
            .then(|| value.trim().trim_matches(['"', '\'']))
    })
}

/// The `<meta charset>` (or `http-equiv` Content-Type) of an HTML page, from its start.
fn meta_charset(body: &[u8]) -> Option<&'static encoding_rs::Encoding> {
    let head = String::from_utf8_lossy(&body[..body.len().min(4096)]).to_ascii_lowercase();
    let start = head.find("charset=")? + "charset=".len();
    let label: String = head[start..]
        .trim_start_matches(['"', '\''])
        .chars()
        .take_while(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | ':' | '.'))
        .collect();
    encoding_rs::Encoding::for_label(label.as_bytes())
}

/// A body as text, the way OkHttp's `body().string()` decodes it: a byte-order mark, else the
/// Content-Type's charset, else UTF-8. `sniff` also reads an HTML page's own `<meta charset>`
/// when the header names none and the bytes aren't UTF-8 (pages for documents only).
pub fn decode(body: &[u8], content_type: Option<&str>, sniff: bool) -> String {
    if let Some((encoding, bom)) = encoding_rs::Encoding::for_bom(body) {
        return encoding
            .decode_without_bom_handling(&body[bom..])
            .0
            .into_owned();
    }
    let named = content_type
        .and_then(charset)
        .and_then(|label| encoding_rs::Encoding::for_label(label.as_bytes()));
    let encoding = match named {
        Some(encoding) => encoding,
        None if sniff && std::str::from_utf8(body).is_err() => {
            meta_charset(body).unwrap_or(encoding_rs::UTF_8)
        }
        None => encoding_rs::UTF_8,
    };
    encoding.decode_without_bom_handling(body).0.into_owned()
}

#[derive(Clone, Default)]
pub struct LuaHeaders(pub Vec<(String, String)>);

#[derive(Default)]
struct HeadersBuilder(Vec<(String, String)>);

#[derive(Clone)]
pub struct LuaBody {
    content_type: String,
    bytes: Vec<u8>,
}

#[derive(Default)]
struct FormBuilder(Vec<(String, String)>);

#[derive(Clone)]
pub struct LuaRequest {
    method: String,
    url: String,
    headers: Vec<(String, String)>,
    body: Option<LuaBody>,
}

struct RequestBuilder(LuaRequest);

#[derive(Clone)]
pub struct LuaResponse {
    status: u16,
    /// The request as it was answered: its URL is the final one, after redirects.
    request: LuaRequest,
    headers: Vec<(String, String)>,
    body: Arc<Vec<u8>>,
}

struct ResponseBody {
    bytes: Arc<Vec<u8>>,
    content_type: Option<String>,
}

/// OkHttp's `HttpUrl`: what `request:url()` gives and `HttpUrl(text)` makes. It reads as its text
/// with `tostring` and `..`.
#[derive(Clone)]
pub struct LuaUrl(url::Url);

/// `HttpUrl.Builder`; `None` until a scheme and host are set (`HttpUrlBuilder()`).
struct UrlBuilder(Option<url::Url>);

/// A cookie as OkHttp's `Cookie` shows one.
#[derive(Clone)]
struct LuaCookie {
    name: String,
    value: String,
    domain: String,
    path: String,
}

/// OkHttp's `Cookie.Builder`.
#[derive(Default)]
struct CookieBuilder(LuaCookie);

impl Default for LuaCookie {
    fn default() -> Self {
        Self {
            name: String::new(),
            value: String::new(),
            domain: String::new(),
            path: "/".into(),
        }
    }
}

/// A list of cookies, read like Java's `List` (`size`, `get` from 0).
#[derive(Clone)]
struct CookieList(Vec<LuaCookie>);

/// The script's cookie jar (`CookieJar()`, `HttpClient():cookieJar()`).
struct LuaCookieJar(Arc<Fetcher>);

fn form_encode(text: &str) -> String {
    url::form_urlencoded::byte_serialize(text.as_bytes()).collect()
}

/// `%XX` escapes back to the bytes they stand for (a path segment as text).
fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        let escaped = (bytes[index] == b'%')
            .then(|| bytes.get(index + 1..index + 3))
            .flatten()
            .and_then(|hex| u8::from_str_radix(std::str::from_utf8(hex).ok()?, 16).ok());
        match escaped {
            Some(byte) => {
                out.push(byte);
                index += 3;
            }
            None => {
                out.push(bytes[index]);
                index += 1;
            }
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A URL argument: a string or an `HttpUrl`.
fn url_text(value: &Value) -> mlua::Result<String> {
    match value {
        Value::String(text) => Ok(text.to_str()?.to_owned()),
        Value::UserData(data) => Ok(data.borrow::<LuaUrl>()?.0.to_string()),
        other => Err(mlua::Error::runtime(format!(
            "expected a URL, got {}",
            other.type_name()
        ))),
    }
}

fn parse_url(text: &str) -> mlua::Result<url::Url> {
    url::Url::parse(text)
        .map_err(|error| mlua::Error::runtime(format!("invalid URL {text:?}: {error}")))
}

impl UserData for LuaHeaders {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("get", |_, this, name: String| {
            Ok(header(&this.0, &name).map(str::to_owned))
        });
        methods.add_method("size", |_, this, ()| Ok(this.0.len()));
        methods.add_method("name", |_, this, index: usize| {
            Ok(this.0.get(index).map(|(name, _)| name.clone()))
        });
        methods.add_method("value", |_, this, index: usize| {
            Ok(this.0.get(index).map(|(_, value)| value.clone()))
        });
        methods.add_method("values", |lua, this, name: String| {
            lua.create_sequence_from(
                this.0
                    .iter()
                    .filter(|(key, _)| key.eq_ignore_ascii_case(&name))
                    .map(|(_, value)| value.clone()),
            )
        });
        methods.add_method("names", |lua, this, ()| {
            lua.create_sequence_from(this.0.iter().map(|(name, _)| name.clone()))
        });
        methods.add_method("newBuilder", |_, this, ()| {
            Ok(HeadersBuilder(this.0.clone()))
        });
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(this
                .0
                .iter()
                .map(|(name, value)| format!("{name}: {value}\n"))
                .collect::<String>())
        });
    }
}

impl UserData for HeadersBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        // `add(name, value)`, or OkHttp's `add("Name: value")`.
        methods.add_function(
            "add",
            |_, (this, name, value): (AnyUserData, String, Option<String>)| {
                let (name, value) = match value {
                    Some(value) => (name, value),
                    None => {
                        let (name, value) = name.split_once(':').ok_or_else(|| {
                            mlua::Error::runtime(format!("unexpected header: {name}"))
                        })?;
                        (name.trim().to_owned(), value.trim().to_owned())
                    }
                };
                this.borrow_mut::<HeadersBuilder>()?.0.push((name, value));
                Ok(this)
            },
        );
        methods.add_function(
            "set",
            |_, (this, name, value): (AnyUserData, String, String)| {
                {
                    let mut builder = this.borrow_mut::<HeadersBuilder>()?;
                    builder
                        .0
                        .retain(|(key, _)| !key.eq_ignore_ascii_case(&name));
                    builder.0.push((name, value));
                }
                Ok(this)
            },
        );
        methods.add_function("removeAll", |_, (this, name): (AnyUserData, String)| {
            this.borrow_mut::<HeadersBuilder>()?
                .0
                .retain(|(key, _)| !key.eq_ignore_ascii_case(&name));
            Ok(this)
        });
        methods.add_method("get", |_, this, name: String| {
            Ok(header(&this.0, &name).map(str::to_owned))
        });
        methods.add_method("build", |_, this, ()| Ok(LuaHeaders(this.0.clone())));
    }
}

impl UserData for LuaBody {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("contentType", |_, this, ()| Ok(this.content_type.clone()));
        methods.add_method("contentLength", |_, this, ()| Ok(this.bytes.len()));
    }
}

impl UserData for FormBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function(
            "add",
            |_, (this, name, value): (AnyUserData, String, String)| {
                this.borrow_mut::<FormBuilder>()?
                    .0
                    .push((form_encode(&name), form_encode(&value)));
                Ok(this)
            },
        );
        methods.add_function(
            "addEncoded",
            |_, (this, name, value): (AnyUserData, String, String)| {
                this.borrow_mut::<FormBuilder>()?.0.push((name, value));
                Ok(this)
            },
        );
        methods.add_method("build", |_, this, ()| {
            let text = this
                .0
                .iter()
                .map(|(name, value)| format!("{name}={value}"))
                .collect::<Vec<_>>()
                .join("&");
            Ok(LuaBody {
                content_type: "application/x-www-form-urlencoded".into(),
                bytes: text.into_bytes(),
            })
        });
    }
}

impl UserData for LuaRequest {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("url", |_, this, ()| Ok(LuaUrl(parse_url(&this.url)?)));
        methods.add_method("method", |_, this, ()| Ok(this.method.clone()));
        methods.add_method("header", |_, this, name: String| {
            Ok(header(&this.headers, &name).map(str::to_owned))
        });
        methods.add_method("headers", |_, this, ()| {
            Ok(LuaHeaders(this.headers.clone()))
        });
        methods.add_method("body", |_, this, ()| Ok(this.body.clone()));
        methods.add_method("newBuilder", |_, this, ()| Ok(RequestBuilder(this.clone())));
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(format!("Request{{method={}, url={}}}", this.method, this.url))
        });
    }
}

impl UserData for RequestBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_function("url", |_, (this, url): (AnyUserData, Value)| {
            this.borrow_mut::<RequestBuilder>()?.0.url = url_text(&url)?;
            Ok(this)
        });
        for name in ["header", "addHeader"] {
            let replace = name == "header";
            methods.add_function(
                name,
                move |_, (this, key, value): (AnyUserData, String, String)| {
                    {
                        let mut builder = this.borrow_mut::<RequestBuilder>()?;
                        if replace {
                            builder
                                .0
                                .headers
                                .retain(|(existing, _)| !existing.eq_ignore_ascii_case(&key));
                        }
                        builder.0.headers.push((key, value));
                    }
                    Ok(this)
                },
            );
        }
        methods.add_function("removeHeader", |_, (this, key): (AnyUserData, String)| {
            this.borrow_mut::<RequestBuilder>()?
                .0
                .headers
                .retain(|(existing, _)| !existing.eq_ignore_ascii_case(&key));
            Ok(this)
        });
        methods.add_function(
            "headers",
            |_, (this, headers): (AnyUserData, AnyUserData)| {
                let headers = headers.borrow::<LuaHeaders>()?.0.clone();
                this.borrow_mut::<RequestBuilder>()?.0.headers = headers;
                Ok(this)
            },
        );
        for (name, method) in [("get", "GET"), ("head", "HEAD")] {
            methods.add_function(name, move |_, this: AnyUserData| {
                {
                    let mut builder = this.borrow_mut::<RequestBuilder>()?;
                    builder.0.method = method.into();
                    builder.0.body = None;
                }
                Ok(this)
            });
        }
        for (name, method) in [
            ("post", "POST"),
            ("put", "PUT"),
            ("patch", "PATCH"),
            ("delete", "DELETE"),
        ] {
            methods.add_function(
                name,
                move |_, (this, body): (AnyUserData, Option<AnyUserData>)| {
                    let body = body
                        .map(|body| body.borrow::<LuaBody>().map(|body| body.clone()))
                        .transpose()?;
                    {
                        let mut builder = this.borrow_mut::<RequestBuilder>()?;
                        builder.0.method = method.into();
                        builder.0.body = body;
                    }
                    Ok(this)
                },
            );
        }
        methods.add_function(
            "method",
            |_, (this, method, body): (AnyUserData, String, Option<AnyUserData>)| {
                let body = body
                    .map(|body| body.borrow::<LuaBody>().map(|body| body.clone()))
                    .transpose()?;
                {
                    let mut builder = this.borrow_mut::<RequestBuilder>()?;
                    builder.0.method = method;
                    builder.0.body = body;
                }
                Ok(this)
            },
        );
        for name in ["cacheControl", "tag"] {
            methods.add_function(name, |_, (this, _): (AnyUserData, Value)| Ok(this));
        }
        methods.add_method("build", |_, this, ()| Ok(this.0.clone()));
    }
}

impl UserData for LuaResponse {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("code", |_, this, ()| Ok(this.status));
        methods.add_method("isSuccessful", |_, this, ()| {
            Ok((200..300).contains(&this.status))
        });
        methods.add_method("isRedirect", |_, this, ()| {
            Ok(matches!(this.status, 300..=303 | 307 | 308))
        });
        methods.add_method("message", |_, this, ()| {
            Ok(reqwest::StatusCode::from_u16(this.status)
                .ok()
                .and_then(|status| status.canonical_reason())
                .unwrap_or_default()
                .to_owned())
        });
        methods.add_method("body", |_, this, ()| {
            Ok(ResponseBody {
                bytes: this.body.clone(),
                content_type: header(&this.headers, "content-type").map(str::to_owned),
            })
        });
        methods.add_method("headers", |_, this, ()| {
            Ok(LuaHeaders(this.headers.clone()))
        });
        methods.add_method("header", |_, this, (name, fallback): (String, Option<String>)| {
            Ok(header(&this.headers, &name).map(str::to_owned).or(fallback))
        });
        methods.add_method("request", |_, this, ()| Ok(this.request.clone()));
        methods.add_method("close", |_, _, ()| Ok(()));
    }
}

impl UserData for ResponseBody {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("string", |_, this, ()| {
            Ok(decode(&this.bytes, this.content_type.as_deref(), false))
        });
        methods.add_method("bytes", |lua, this, ()| lua.create_string(&**this.bytes));
        methods.add_method("contentLength", |_, this, ()| Ok(this.bytes.len()));
        methods.add_method("contentType", |_, this, ()| Ok(this.content_type.clone()));
        methods.add_method("close", |_, _, ()| Ok(()));
    }
}

impl UserData for LuaUrl {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("toString", |_, this, ()| Ok(this.0.to_string()));
        methods.add_method("scheme", |_, this, ()| Ok(this.0.scheme().to_owned()));
        methods.add_method("host", |_, this, ()| {
            Ok(this.0.host_str().unwrap_or_default().to_owned())
        });
        methods.add_method("port", |_, this, ()| {
            Ok(this.0.port_or_known_default().unwrap_or_default())
        });
        for name in ["encodedPath", "path"] {
            methods.add_method(name, |_, this, ()| Ok(this.0.path().to_owned()));
        }
        methods.add_method("pathSegments", |lua, this, ()| {
            lua.create_sequence_from(
                this.0
                    .path_segments()
                    .into_iter()
                    .flatten()
                    .map(percent_decode),
            )
        });
        methods.add_method("encodedPathSegments", |lua, this, ()| {
            lua.create_sequence_from(
                this.0
                    .path_segments()
                    .into_iter()
                    .flatten()
                    .map(str::to_owned),
            )
        });
        methods.add_method("encodedQuery", |_, this, ()| {
            Ok(this.0.query().map(str::to_owned))
        });
        methods.add_method("query", |_, this, ()| {
            Ok(this.0.query().map(|query| {
                url::form_urlencoded::parse(query.as_bytes())
                    .map(|(key, value)| format!("{key}={value}"))
                    .collect::<Vec<_>>()
                    .join("&")
            }))
        });
        methods.add_method("queryParameter", |_, this, name: String| {
            Ok(this
                .0
                .query_pairs()
                .find(|(key, _)| *key == name)
                .map(|(_, value)| value.into_owned()))
        });
        methods.add_method("queryParameterNames", |lua, this, ()| {
            let mut names: Vec<String> = Vec::new();
            for (key, _) in this.0.query_pairs() {
                if !names.iter().any(|name| *name == key) {
                    names.push(key.into_owned());
                }
            }
            lua.create_sequence_from(names)
        });
        methods.add_method("fragment", |_, this, ()| {
            Ok(this.0.fragment().map(str::to_owned))
        });
        methods.add_method("resolve", |_, this, link: String| {
            Ok(this.0.join(&link).ok().map(LuaUrl))
        });
        methods.add_method("newBuilder", |_, this, ()| {
            Ok(UrlBuilder(Some(this.0.clone())))
        });
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| Ok(this.0.to_string()));
        methods.add_meta_function(MetaMethod::Concat, |_, (left, right): (Value, Value)| {
            let text = |value: &Value| -> mlua::Result<String> {
                match value {
                    Value::UserData(data) => Ok(data.borrow::<LuaUrl>()?.0.to_string()),
                    Value::String(text) => Ok(text.to_str()?.to_owned()),
                    Value::Integer(number) => Ok(number.to_string()),
                    Value::Number(number) => Ok(number.to_string()),
                    other => Err(mlua::Error::runtime(format!(
                        "attempt to concatenate a {} value",
                        other.type_name()
                    ))),
                }
            };
            Ok(text(&left)? + &text(&right)?)
        });
        methods.add_meta_method(MetaMethod::Eq, |_, this, other: AnyUserData| {
            Ok(other
                .borrow::<LuaUrl>()
                .is_ok_and(|other| other.0 == this.0))
        });
    }
}

impl UserData for UrlBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        fn edit(
            this: &AnyUserData,
            change: impl FnOnce(&mut url::Url) -> mlua::Result<()>,
        ) -> mlua::Result<()> {
            let mut builder = this.borrow_mut::<UrlBuilder>()?;
            let url = builder
                .0
                .as_mut()
                .ok_or_else(|| mlua::Error::runtime("set the URL's scheme and host first"))?;
            change(url)
        }
        methods.add_function("scheme", |_, (this, scheme): (AnyUserData, String)| {
            {
                let mut builder = this.borrow_mut::<UrlBuilder>()?;
                match builder.0.as_mut() {
                    Some(url) => url
                        .set_scheme(&scheme)
                        .map_err(|()| mlua::Error::runtime(format!("bad scheme {scheme}")))?,
                    None => builder.0 = Some(parse_url(&format!("{scheme}://placeholder.invalid/"))?),
                }
            }
            Ok(this)
        });
        methods.add_function("host", |_, (this, host): (AnyUserData, String)| {
            {
                let mut builder = this.borrow_mut::<UrlBuilder>()?;
                let url = builder
                    .0
                    .get_or_insert_with(|| url::Url::parse("https://placeholder.invalid/").expect("a valid URL"));
                url.set_host(Some(&host))
                    .map_err(|error| mlua::Error::runtime(format!("bad host {host}: {error}")))?;
            }
            Ok(this)
        });
        methods.add_function("port", |_, (this, port): (AnyUserData, u16)| {
            edit(&this, |url| {
                url.set_port(Some(port))
                    .map_err(|()| mlua::Error::runtime("this URL can't have a port"))
            })?;
            Ok(this)
        });
        for name in ["addPathSegment", "addEncodedPathSegment"] {
            let encoded = name == "addEncodedPathSegment";
            methods.add_function(name, move |_, (this, segment): (AnyUserData, String)| {
                edit(&this, |url| {
                    let mut segments = url
                        .path_segments_mut()
                        .map_err(|()| mlua::Error::runtime("this URL has no path"))?;
                    segments.pop_if_empty();
                    if encoded {
                        // Already escaped: pushed as it is, through the path.
                        drop(segments);
                        let path = format!("{}/{segment}", url.path().trim_end_matches('/'));
                        url.set_path(&path);
                    } else {
                        segments.push(&segment);
                    }
                    Ok(())
                })?;
                Ok(this)
            });
        }
        methods.add_function("addPathSegments", |_, (this, segments): (AnyUserData, String)| {
            edit(&this, |url| {
                let mut path = url
                    .path_segments_mut()
                    .map_err(|()| mlua::Error::runtime("this URL has no path"))?;
                path.pop_if_empty();
                for segment in segments.split('/').filter(|segment| !segment.is_empty()) {
                    path.push(segment);
                }
                Ok(())
            })?;
            Ok(this)
        });
        methods.add_function("encodedPath", |_, (this, path): (AnyUserData, String)| {
            edit(&this, |url| {
                url.set_path(&path);
                Ok(())
            })?;
            Ok(this)
        });
        for name in ["addQueryParameter", "addEncodedQueryParameter"] {
            let encoded = name == "addEncodedQueryParameter";
            methods.add_function(
                name,
                move |_, (this, key, value): (AnyUserData, String, Option<String>)| {
                    edit(&this, |url| {
                        if encoded {
                            let pair = match &value {
                                Some(value) => format!("{key}={value}"),
                                None => key.clone(),
                            };
                            let query = match url.query() {
                                Some(query) if !query.is_empty() => format!("{query}&{pair}"),
                                _ => pair,
                            };
                            url.set_query(Some(&query));
                        } else {
                            url.query_pairs_mut()
                                .append_pair(&key, value.as_deref().unwrap_or_default());
                        }
                        Ok(())
                    })?;
                    Ok(this)
                },
            );
        }
        methods.add_function(
            "setQueryParameter",
            |_, (this, key, value): (AnyUserData, String, Option<String>)| {
                edit(&this, |url| {
                    let kept: Vec<(String, String)> = url
                        .query_pairs()
                        .filter(|(existing, _)| *existing != key)
                        .map(|(key, value)| (key.into_owned(), value.into_owned()))
                        .collect();
                    let mut pairs = url.query_pairs_mut();
                    pairs.clear();
                    for (key, value) in &kept {
                        pairs.append_pair(key, value);
                    }
                    if let Some(value) = &value {
                        pairs.append_pair(&key, value);
                    }
                    drop(pairs);
                    if url.query() == Some("") {
                        url.set_query(None);
                    }
                    Ok(())
                })?;
                Ok(this)
            },
        );
        methods.add_function(
            "removeAllQueryParameters",
            |_, (this, key): (AnyUserData, String)| {
                edit(&this, |url| {
                    let kept: Vec<(String, String)> = url
                        .query_pairs()
                        .filter(|(existing, _)| *existing != key)
                        .map(|(key, value)| (key.into_owned(), value.into_owned()))
                        .collect();
                    if kept.is_empty() {
                        url.set_query(None);
                    } else {
                        url.query_pairs_mut().clear().extend_pairs(kept);
                    }
                    Ok(())
                })?;
                Ok(this)
            },
        );
        for name in ["query", "encodedQuery"] {
            methods.add_function(name, |_, (this, query): (AnyUserData, Option<String>)| {
                edit(&this, |url| {
                    url.set_query(query.as_deref());
                    Ok(())
                })?;
                Ok(this)
            });
        }
        for name in ["fragment", "encodedFragment"] {
            methods.add_function(name, |_, (this, fragment): (AnyUserData, Option<String>)| {
                edit(&this, |url| {
                    url.set_fragment(fragment.as_deref());
                    Ok(())
                })?;
                Ok(this)
            });
        }
        methods.add_method("build", |_, this, ()| {
            this.0
                .clone()
                .map(LuaUrl)
                .ok_or_else(|| mlua::Error::runtime("a URL needs a scheme and a host"))
        });
    }
}

impl UserData for LuaCookie {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("name", |_, this, ()| Ok(this.name.clone()));
        methods.add_method("value", |_, this, ()| Ok(this.value.clone()));
        methods.add_method("domain", |_, this, ()| Ok(this.domain.clone()));
        methods.add_method("path", |_, this, ()| Ok(this.path.clone()));
        methods.add_meta_method(MetaMethod::ToString, |_, this, ()| {
            Ok(format!("{}={}", this.name, this.value))
        });
    }
}

impl UserData for CookieBuilder {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        for field in ["name", "value", "domain", "hostOnlyDomain", "path"] {
            methods.add_function(field, move |_, (this, text): (AnyUserData, String)| {
                {
                    let mut builder = this.borrow_mut::<CookieBuilder>()?;
                    match field {
                        "name" => builder.0.name = text,
                        "value" => builder.0.value = text,
                        "path" => builder.0.path = text,
                        _ => builder.0.domain = text,
                    }
                }
                Ok(this)
            });
        }
        for flag in ["secure", "httpOnly", "expiresAt"] {
            methods.add_function(flag, |_, (this, _): (AnyUserData, mlua::Variadic<Value>)| {
                Ok(this)
            });
        }
        methods.add_method("build", |_, this, ()| Ok(this.0.clone()));
    }
}

impl UserData for CookieList {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("size", |_, this, ()| Ok(this.0.len()));
        methods.add_method("isEmpty", |_, this, ()| Ok(this.0.is_empty()));
        methods.add_method("get", |_, this, index: usize| Ok(this.0.get(index).cloned()));
        methods.add_meta_method(MetaMethod::Len, |_, this, ()| Ok(this.0.len()));
    }
}

impl UserData for LuaCookieJar {
    fn add_methods<M: UserDataMethods<Self>>(methods: &mut M) {
        methods.add_method("loadForRequest", |_, this, url: Value| {
            let url = parse_url(&url_text(&url)?)?;
            let domain = url.host_str().unwrap_or_default().to_owned();
            Ok(CookieList(
                this.0
                    .cookies_for(&url)
                    .into_iter()
                    .map(|(name, value)| LuaCookie {
                        name,
                        value,
                        domain: domain.clone(),
                        path: "/".into(),
                    })
                    .collect(),
            ))
        });
        methods.add_method(
            "saveFromResponse",
            |_, this, (url, cookies): (Value, Value)| {
                let url = parse_url(&url_text(&url)?)?;
                let mut lines = Vec::new();
                let mut add = |cookie: &LuaCookie| {
                    let mut line = format!("{}={}; Path={}", cookie.name, cookie.value, cookie.path);
                    if !cookie.domain.is_empty() {
                        line.push_str(&format!("; Domain={}", cookie.domain));
                    }
                    lines.push(line);
                };
                match cookies {
                    Value::UserData(data) => {
                        if let Ok(list) = data.borrow::<CookieList>() {
                            list.0.iter().for_each(&mut add);
                        } else {
                            add(&*data.borrow::<LuaCookie>()?);
                        }
                    }
                    Value::Table(table) => {
                        for cookie in table.sequence_values::<AnyUserData>() {
                            add(&*cookie?.borrow::<LuaCookie>()?);
                        }
                    }
                    _ => {}
                }
                this.0.save_cookies(&url, &lines);
                Ok(())
            },
        );
    }
}

/// A `Set-Cookie` line as a cookie for `url`.
fn parse_cookie(url: &url::Url, line: &str) -> Option<LuaCookie> {
    let mut parts = line.split(';');
    let (name, value) = parts.next()?.trim().split_once('=')?;
    let mut cookie = LuaCookie {
        name: name.trim().to_owned(),
        value: value.trim().to_owned(),
        domain: url.host_str().unwrap_or_default().to_owned(),
        path: "/".into(),
    };
    for attribute in parts {
        if let Some((key, value)) = attribute.trim().split_once('=') {
            if key.trim().eq_ignore_ascii_case("domain") {
                cookie.domain = value.trim().trim_start_matches('.').to_owned();
            } else if key.trim().eq_ignore_ascii_case("path") {
                cookie.path = value.trim().to_owned();
            }
        }
    }
    (!cookie.name.is_empty()).then_some(cookie)
}

fn request(
    method: &str,
    url: String,
    headers: Option<AnyUserData>,
    body: Option<LuaBody>,
) -> mlua::Result<LuaRequest> {
    let headers = match headers {
        Some(headers) => headers.borrow::<LuaHeaders>()?.0.clone(),
        None => Vec::new(),
    };
    Ok(LuaRequest {
        method: method.into(),
        url,
        headers,
        body,
    })
}

/// A document from a successful response; a failed one is the error, kept typed.
fn document(fetcher: &Fetcher, request: &LuaRequest) -> mlua::Result<LuaNode> {
    let response = fetcher.execute(request).map_err(mlua::Error::external)?;
    if !(200..300).contains(&response.status) {
        let host = reqwest::Url::parse(&response.request.url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_default();
        let error = if bot_check(response.status, &response.headers, &response.body) {
            FetchError::BrowserCheck {
                host,
                status: response.status,
            }
        } else {
            FetchError::Status {
                status: response.status,
                url: response.request.url,
            }
        };
        return Err(mlua::Error::external(error));
    }
    let base = reqwest::Url::parse(&response.request.url).ok();
    let text = decode(&response.body, header(&response.headers, "content-type"), true);
    Ok(parse_document(&text, base))
}

pub fn install(lua: &Lua, globals: &Table, fetcher: Arc<Fetcher>) -> mlua::Result<()> {
    globals.set(
        "DEFAULT_HEADERS",
        lua.create_function(|_, ()| Ok(LuaHeaders::default()))?,
    )?;
    globals.set(
        "DEFAULT_CACHE_CONTROL",
        lua.create_function(|_, ()| Ok(true))?,
    )?;
    globals.set(
        "DEFAULT_BODY",
        lua.create_function(|_, ()| {
            Ok(LuaBody {
                content_type: "application/x-www-form-urlencoded".into(),
                bytes: Vec::new(),
            })
        })?,
    )?;
    globals.set(
        "_GET",
        lua.create_function(
            |_, (url, headers, _cache): (Value, Option<AnyUserData>, Value)| {
                request("GET", url_text(&url)?, headers, None)
            },
        )?,
    )?;
    globals.set(
        "_POST",
        lua.create_function(
            |_,
             (url, headers, body, _cache): (
                Value,
                Option<AnyUserData>,
                Option<AnyUserData>,
                Value,
            )| {
                let body = body
                    .map(|body| body.borrow::<LuaBody>().map(|body| body.clone()))
                    .transpose()?;
                request("POST", url_text(&url)?, headers, body)
            },
        )?,
    )?;
    let run = fetcher.clone();
    globals.set(
        "Request",
        lua.create_function(move |_, request: AnyUserData| {
            run.execute(&*request.borrow::<LuaRequest>()?)
                .map_err(mlua::Error::external)
        })?,
    )?;
    let run = fetcher.clone();
    globals.set(
        "RequestDocument",
        lua.create_function(move |_, request: AnyUserData| {
            document(&run, &*request.borrow::<LuaRequest>()?)
        })?,
    )?;
    let run = fetcher.clone();
    globals.set(
        "GETDocument",
        lua.create_function(move |_, url: Value| {
            document(&run, &request("GET", url_text(&url)?, None, None)?)
        })?,
    )?;
    globals.set(
        "HeadersBuilder",
        lua.create_function(|_, ()| Ok(HeadersBuilder::default()))?,
    )?;
    globals.set(
        "FormBodyBuilder",
        lua.create_function(|_, ()| Ok(FormBuilder::default()))?,
    )?;
    globals.set(
        "RequestBuilder",
        lua.create_function(|_, ()| {
            Ok(RequestBuilder(LuaRequest {
                method: "GET".into(),
                url: String::new(),
                headers: Vec::new(),
                body: None,
            }))
        })?,
    )?;
    globals.set(
        "MediaType",
        lua.create_function(|_, kind: String| Ok(kind))?,
    )?;
    globals.set(
        "RequestBody",
        lua.create_function(|_, (data, kind): (mlua::String, Option<String>)| {
            Ok(LuaBody {
                content_type: kind.unwrap_or_else(|| "text/plain; charset=utf-8".into()),
                bytes: data.as_bytes().to_vec(),
            })
        })?,
    )?;
    globals.set(
        "HttpUrl",
        lua.create_function(|_, url: Value| {
            Ok(url_text(&url)
                .ok()
                .and_then(|text| url::Url::parse(&text).ok())
                .filter(|url| matches!(url.scheme(), "http" | "https"))
                .map(LuaUrl))
        })?,
    )?;
    globals.set(
        "HttpUrlBuilder",
        lua.create_function(|_, ()| Ok(UrlBuilder(None)))?,
    )?;
    let jar = fetcher;
    globals.set(
        "_CookieJar",
        lua.create_function(move |_, ()| Ok(LuaCookieJar(jar.clone())))?,
    )?;
    globals.set(
        "CookieBuilder",
        lua.create_function(|_, ()| Ok(CookieBuilder::default()))?,
    )?;
    globals.set(
        "CookieParse",
        lua.create_function(|_, (url, line): (Value, String)| {
            Ok(parse_cookie(&parse_url(&url_text(&url)?)?, &line))
        })?,
    )?;
    globals.set(
        "CookieParseAll",
        lua.create_function(|_, (url, headers): (Value, AnyUserData)| {
            let url = parse_url(&url_text(&url)?)?;
            let headers = headers.borrow::<LuaHeaders>()?;
            Ok(CookieList(
                headers
                    .0
                    .iter()
                    .filter(|(name, _)| name.eq_ignore_ascii_case("set-cookie"))
                    .filter_map(|(_, line)| parse_cookie(&url, line))
                    .collect(),
            ))
        })?,
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn refuses_local_addresses() {
        let fetcher = Fetcher::new(false, CookieJar::in_memory()).unwrap();
        for url in [
            "http://127.0.0.1/",
            "http://localhost:8080/",
            "http://192.168.1.10/",
            "http://10.0.2.2/",
            "http://[::1]/",
            "http://router.lan/",
            "file:///etc/passwd",
        ] {
            let request = request("GET", url.into(), None, None).unwrap();
            let error = fetcher
                .execute(&request)
                .err()
                .map(|error| error.to_string())
                .unwrap_or_default();
            assert!(
                error.contains("not a public") || error.contains("invalid URL"),
                "{url}: {error}"
            );
        }
    }

    #[test]
    fn bodies_decode_as_okhttp_does() {
        // windows-1251 "Привет", as Russian sites send it.
        let cp1251 = [0xCF, 0xF0, 0xE8, 0xE2, 0xE5, 0xF2];
        assert_eq!(decode(&cp1251, Some("text/html; charset=windows-1251"), false), "Привет");
        // Shift_JIS "日本", named with quotes.
        let sjis = [0x93, 0xFA, 0x96, 0x7B];
        assert_eq!(decode(&sjis, Some("text/html;charset=\"Shift_JIS\""), false), "日本");
        // A byte-order mark wins over the header.
        let bom = [0xEF, 0xBB, 0xBF, b'h', b'i'];
        assert_eq!(decode(&bom, Some("text/html; charset=iso-8859-1"), false), "hi");
        assert_eq!(decode("ok ✓".as_bytes(), None, false), "ok ✓");
        // A page's own meta charset, for documents whose header names none.
        let mut page = b"<html><head><meta charset=\"windows-1251\"></head><body>".to_vec();
        page.extend_from_slice(&cp1251);
        assert!(decode(&page, Some("text/html"), true).ends_with("Привет"));
        assert!(!decode(&page, Some("text/html"), false).ends_with("Привет"));
    }

    #[test]
    fn bot_checks_are_told_from_errors() {
        let cloudflare = vec![("Server".to_owned(), "cloudflare".to_owned())];
        let challenge = b"<html><head><title>Just a moment...</title><script src=\"/cdn-cgi/challenge-platform/h/b\"></script>";
        assert!(bot_check(403, &cloudflare, challenge));
        assert!(bot_check(
            403,
            &[("cf-mitigated".into(), "challenge".into())],
            b""
        ));
        assert!(!bot_check(403, &cloudflare, b"<h1>Forbidden</h1>"));
        assert!(!bot_check(200, &cloudflare, challenge), "a page about challenges");
        assert!(!bot_check(404, &[], b"not found"));
        assert!(bot_check(
            403,
            &[("server".into(), "ddos-guard".into())],
            b""
        ));
    }

    #[test]
    fn cookies_parse_from_set_cookie_lines() {
        let url = url::Url::parse("https://www.example.org/a").unwrap();
        let cookie = parse_cookie(&url, "PHPSESSID=abc_123; Path=/; Domain=.example.org; HttpOnly")
            .unwrap();
        assert_eq!(
            (cookie.name.as_str(), cookie.value.as_str(), cookie.domain.as_str()),
            ("PHPSESSID", "abc_123", "example.org")
        );
        assert!(parse_cookie(&url, "garbage").is_none());
    }
}
