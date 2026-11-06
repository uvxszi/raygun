//! Browser checks passed by hand (experimental, off unless `Settings::manual_browser_checks` is on).
//!
//! Some sites answer with a bot check (Cloudflare's "Just a moment...", DDoS-Guard, a captcha)
//! instead of the page. Raygun never solves, skips or automates such a check. With the setting on,
//! the person can open the check in a visible browser and pass it themselves, as in Mihon's
//! "Open in WebView":
//!
//! 1. A source's request that meets a check is kept as it was sent ([`CheckedRequest`]: address,
//!    method, headers, body, User-Agent) by its runtime (`js.rs`, Shosetsu's `http.rs`). When the
//!    call fails with [`IssueKind::BotCheck`], the extension's lane notes it ([`Challenges::note`]):
//!    the extension, the source's name, the request to open ([`pick_request`]: that request, else
//!    a visit of the site the failure's details name, else the extension's website) and the call,
//!    to ask it again.
//! 2. `SourceIssue` in the UI asks `browser_check_target` for the failure it shows
//!    ([`Challenges::find`]); with the setting on, it offers "Open the site to pass the check".
//! 3. `raygun-app/src/browser_check.rs` opens that request ([`CheckedRequest::window`]) with the
//!    User-Agent the source sent, so the browser meets the same check: a Tauri window on desktop
//!    (its own profile; Windows sends the method, headers and body too), `BrowserCheckActivity` in
//!    the `raygun-audio` plugin on Android (its own process and WebView data). Neither touches the
//!    page. What the browser sees of its pages ([`Seen`]) tells whether a check showed and was
//!    passed ([`CheckWindow`]): a check's page (its answer says `cf-mitigated: challenge`, or its
//!    title is a check's), then one that isn't. The browser closes then, or when the person
//!    closes it.
//! 4. `ExtensionHost::finish_browser_check`: only a passed check changes anything. The browser's
//!    cookies (such as `cf_clearance`) and its User-Agent, which the pass is tied to, go into the
//!    extension's cookie jar for that site ([`crate::cookies::CookieJar::import`], the jar
//!    JavaScript, Miru and Shosetsu sources share; their requests to the site then go with that
//!    User-Agent), the source's failure record is forgotten, and the failed call is asked again.
//!    A page that loads without any check is said so: the problem is something else.

use std::collections::{BTreeMap, HashMap};
use std::sync::{Mutex, PoisonError};

use raygun_net::issue::IssueKind;
use reqwest::Url;
use serde::{Deserialize, Serialize};

/// How long a cookie the browser gave without an expiry (Android's `CookieManager` gives only
/// names and values) is kept: long enough to outlast a restart; the site asks again when it no
/// longer takes it.
pub const UNDATED_LIFETIME_SECS: i64 = 7 * 24 * 60 * 60;

/// The body type a check window posts with when the source named none: a browser form's.
const FORM: &str = "application/x-www-form-urlencoded";

/// A source's request that met a browser check, as the source sent it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CheckedRequest {
    pub url: String,
    /// In capitals (`GET`, `POST`).
    pub method: String,
    /// The headers the source set, its User-Agent apart.
    pub headers: Vec<(String, String)>,
    pub body: Option<String>,
    /// The User-Agent it went with: the source's own, else the one a check on the site was passed
    /// with, else the one sources send (`raygun_net::user_agent`, Shosetsu's for Shosetsu scripts).
    pub user_agent: String,
}

impl CheckedRequest {
    pub fn new<'a>(
        url: &str,
        method: &str,
        headers: impl IntoIterator<Item = (&'a str, &'a str)>,
        body: Option<&str>,
        user_agent: &str,
    ) -> Self {
        Self {
            url: url.to_owned(),
            method: method.trim().to_ascii_uppercase(),
            headers: headers
                .into_iter()
                .filter(|(name, _)| !name.eq_ignore_ascii_case("user-agent"))
                .map(|(name, value)| (name.to_owned(), value.to_owned()))
                .collect(),
            body: body.map(str::to_owned),
            user_agent: user_agent.to_owned(),
        }
    }

    /// A plain visit of `url` with the User-Agent sources send, for a check whose request isn't
    /// known.
    pub fn visit(url: &str) -> Self {
        Self::new(url, "GET", [], None, &raygun_net::user_agent())
    }

    /// The request as the check window sends it, so the browser meets the check the source met;
    /// `None` unless it's a web address. A POST stays a POST with its body (the check may guard
    /// only those); anything else is a visit of the address. Headers the browser sets itself
    /// (cookies, lengths, encodings, `Accept`, `Origin`, `Sec-*`), conditional ones and ranges are
    /// left to it.
    pub fn window(&self) -> Option<WindowRequest> {
        let url = Url::parse(self.url.trim())
            .ok()
            .filter(|url| matches!(url.scheme(), "http" | "https") && url.host_str().is_some())?;
        let post = self.method == "POST";
        let mut headers = BTreeMap::<String, String>::new();
        for (name, value) in &self.headers {
            let lower = name.trim().to_ascii_lowercase();
            let unwanted = browser_sets(&lower) || (!post && lower == "content-type");
            let known = headers
                .keys()
                .any(|known| known.eq_ignore_ascii_case(name.trim()));
            if unwanted || known || lower.is_empty() || value.contains(['\r', '\n']) {
                continue;
            }
            headers.insert(name.trim().to_owned(), value.trim().to_owned());
        }
        if post
            && !headers
                .keys()
                .any(|name| name.eq_ignore_ascii_case("content-type"))
        {
            headers.insert("Content-Type".to_owned(), FORM.to_owned());
        }
        Some(WindowRequest {
            url: url.to_string(),
            method: if post { "POST" } else { "GET" }.to_owned(),
            headers,
            body: post.then(|| self.body.clone().unwrap_or_default()),
            user_agent: self.user_agent.clone(),
        })
    }
}

/// Headers (lower case) a browser sets itself, or that would make its page something else.
fn browser_sets(lower: &str) -> bool {
    matches!(
        lower,
        "user-agent"
            | "cookie"
            | "cookie2"
            | "host"
            | "origin"
            | "accept"
            | "accept-encoding"
            | "content-length"
            | "connection"
            | "keep-alive"
            | "transfer-encoding"
            | "te"
            | "trailer"
            | "upgrade"
            | "expect"
            | "range"
    ) || ["sec-", "proxy-", "if-"]
        .iter()
        .any(|prefix| lower.starts_with(prefix))
}

/// What a check window opens: [`CheckedRequest::window`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowRequest {
    pub url: String,
    /// `GET`, or `POST` with [`Self::body`].
    pub method: String,
    /// The source's own headers a page may send.
    pub headers: BTreeMap<String, String>,
    pub body: Option<String>,
    pub user_agent: String,
}

/// A provider call that failed on a browser check, to ask again once the check is passed.
#[derive(Debug, Clone, PartialEq)]
pub struct FailedCall {
    /// The provider's full `<extension>/<provider>` id.
    pub provider: String,
    pub method: String,
    pub args: serde_json::Value,
}

/// A source that met a browser check, and where.
#[derive(Debug, Clone, PartialEq)]
pub struct Challenge {
    pub extension_id: String,
    /// The source's name for people, as failures name it.
    pub source: String,
    /// What the check window opens.
    pub request: CheckedRequest,
    /// The call that failed on the check.
    pub call: Option<FailedCall>,
}

impl Challenge {
    /// The address the check window opens.
    pub fn url(&self) -> &str {
        &self.request.url
    }
}

/// A failed call that met a browser check, as its lane saw it.
#[derive(Debug, Clone)]
pub struct Met<'a> {
    pub extension_id: &'a str,
    pub source: &'a str,
    /// The failure's details, which name the challenged site when the request isn't known.
    pub detail: &'a str,
    /// The extension's own site, the last resort.
    pub website: Option<&'a str>,
    /// The request that met the check, when the runtime kept it.
    pub request: Option<CheckedRequest>,
    pub call: Option<FailedCall>,
}

/// The latest browser check of each source, by its name.
#[derive(Debug, Default)]
pub struct Challenges(Mutex<HashMap<String, Challenge>>);

impl Challenges {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Challenge>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes a failed call, when it met a browser check (`kind`).
    pub fn note(&self, kind: Option<IssueKind>, met: Met<'_>) {
        if kind != Some(IssueKind::BotCheck) {
            return;
        }
        let Some(request) = pick_request(met.request, met.detail, met.website) else {
            return;
        };
        self.map().insert(
            met.source.to_lowercase(),
            Challenge {
                extension_id: met.extension_id.to_owned(),
                source: met.source.to_owned(),
                request,
                call: met.call,
            },
        );
    }

    /// The check behind an error text: `source` when the page knows it, else the source the
    /// error's first line starts with (the longest name that fits).
    pub fn find(&self, error: &str, source: Option<&str>) -> Option<Challenge> {
        let map = self.map();
        if let Some(found) = source.and_then(|source| map.get(&source.trim().to_lowercase())) {
            return Some(found.clone());
        }
        let first = error.trim().lines().next()?.to_lowercase();
        map.iter()
            .filter(|(name, _)| first.starts_with(name.as_str()))
            .max_by_key(|(name, _)| name.len())
            .map(|(_, challenge)| challenge.clone())
    }

    /// The check last met by any source of `extension_id`.
    pub fn of_extension(&self, extension_id: &str) -> Option<Challenge> {
        self.map()
            .values()
            .find(|challenge| challenge.extension_id == extension_id)
            .cloned()
    }

    /// Forgets `extension_id`'s checks: they were passed.
    pub fn clear(&self, extension_id: &str) {
        self.map()
            .retain(|_, challenge| challenge.extension_id != extension_id);
    }
}

/// What a check window opens for a failure: the request that met the check when the runtime kept
/// it (and a browser can open it), else a visit of the site the failure's details name, else of
/// the extension's website.
pub fn pick_request(
    request: Option<CheckedRequest>,
    detail: &str,
    website: Option<&str>,
) -> Option<CheckedRequest> {
    request
        .filter(|request| request.window().is_some())
        .or_else(|| challenged_site(detail).map(|url| CheckedRequest::visit(&url)))
        .or_else(|| {
            let site = Url::parse(website?.trim()).ok()?;
            matches!(site.scheme(), "http" | "https").then(|| CheckedRequest::visit(site.as_str()))
        })
}

/// The site a failure's details name as answering with a browser check, as `https://<host>/`.
/// Reads the forms JavaScript and Shosetsu sources' failures take: "<host> answered HTTP 403
/// with a browser check", "HTTP 403 from <host>: a bot check page", "... Try again later
/// (<host>: ...)".
pub fn challenged_site(detail: &str) -> Option<String> {
    detail.lines().find_map(|line| {
        let host = if let Some(start) = line.find(" answered HTTP ")
            && line.contains("browser check")
        {
            line[..start].rsplit(' ').next()
        } else if let Some(end) = line.find(": a bot check page") {
            line[..end].rsplit("from ").next()
        } else if let Some(start) = line.find("Try again later (") {
            line[start + "Try again later (".len()..].split(':').next()
        } else {
            None
        }?;
        site_root(host.trim())
    })
}

fn site_root(host: &str) -> Option<String> {
    let valid = !host.is_empty()
        && host
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | ':'));
    if !valid {
        return None;
    }
    Url::parse(&format!("https://{host}/"))
        .ok()
        .filter(|url| url.host_str().is_some_and(|name| name.contains('.')))
        .map(|url| url.to_string())
}

/// Whether a page's title is a browser check's own ("Just a moment...", "Attention Required!",
/// DDoS-Guard's, a captcha page's). The Android browser keeps the same list in
/// `BrowserCheckActivity.kt`. Checks translate their titles; their answers' `cf-mitigated`
/// header tells them in any language ([`Seen::Answer`]).
pub fn looks_like_challenge(title: &str) -> bool {
    let title = title.trim().to_lowercase();
    CHALLENGE_TITLES.iter().any(|mark| title.contains(mark))
}

const CHALLENGE_TITLES: &[&str] = &[
    "just a moment",
    "attention required",
    "checking your browser",
    "checking if the site connection is secure",
    "verify you are human",
    "verifying you are human",
    "security check",
    "ddos-guard",
    "please wait",
    "one more step",
    "captcha",
    "making sure you're not a bot",
];

/// Something a check window saw of its pages (the top frame), in the order it saw it. Android's
/// browser reports these as JSON objects with a `kind`.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum Seen {
    /// A page started loading.
    Start { url: String },
    /// A page's answer, when the browser tells (Android for failed answers, Windows for checks'):
    /// its status, and whether it said it's a browser check (Cloudflare's `cf-mitigated:
    /// challenge`). It may come before or after its page's start.
    Answer {
        url: String,
        status: u16,
        #[serde(default)]
        mitigated: bool,
    },
    /// The page's title, once known or changed.
    Title { title: String },
    /// A page finished loading.
    Finish { url: String },
    /// A page couldn't load (no connection, a name that doesn't resolve).
    Failed { url: String },
}

/// How a check window ended, as far as what it saw tells ([`CheckWindow::outcome`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum WindowOutcome {
    /// A check showed, then a page that isn't one loaded: the check was passed.
    Passed,
    /// Pages loaded, and none of them was a check.
    NoCheck,
    /// A check showed, and no page that isn't one loaded after it.
    NotPassed,
    /// No page loaded.
    NotLoaded,
}

/// A check window's pages, from what it saw ([`Seen`]).
#[derive(Debug, Clone, Default)]
pub struct CheckWindow {
    pages: Vec<Page>,
    /// Answers that said they're a check and came before their page started, by address.
    early: Vec<String>,
}

#[derive(Debug, Clone, Default)]
struct Page {
    url: String,
    title: String,
    /// Its answer was seen.
    answered: bool,
    mitigated: bool,
    finished: bool,
    failed: bool,
}

impl Page {
    fn new(url: String, mitigated: bool) -> Self {
        Self {
            url,
            mitigated,
            ..Self::default()
        }
    }

    /// A web page (not the window's blank start or an error page of its own).
    fn web(&self) -> bool {
        self.url.starts_with("https://") || self.url.starts_with("http://")
    }

    fn is_check(&self) -> bool {
        self.mitigated || looks_like_challenge(&self.title)
    }

    fn loaded(&self) -> bool {
        self.finished && !self.failed
    }
}

/// Earlier answers kept for pages yet to start.
const EARLY_ANSWERS: usize = 16;

impl CheckWindow {
    pub fn see(&mut self, seen: Seen) {
        match seen {
            Seen::Start { url } => {
                let url = same_page(&url);
                let mitigated = self.take_early(&url);
                self.pages.push(Page::new(url, mitigated));
            }
            Seen::Answer {
                url,
                status,
                mitigated,
            } => {
                let url = same_page(&url);
                tracing::debug!(%url, status, mitigated, "a check window's page answered");
                match self.pages.last_mut() {
                    Some(page) if page.url == url && !page.finished && !page.answered => {
                        page.answered = true;
                        page.mitigated |= mitigated;
                    }
                    _ if mitigated => {
                        self.early.retain(|early| *early != url);
                        if self.early.len() >= EARLY_ANSWERS {
                            self.early.remove(0);
                        }
                        self.early.push(url);
                    }
                    _ => {}
                }
            }
            Seen::Title { title } => {
                if let Some(page) = self.pages.last_mut() {
                    page.title = title;
                }
            }
            Seen::Finish { url } => self.current(&url).finished = true,
            Seen::Failed { url } => {
                let page = self.current(&url);
                page.finished = true;
                page.failed = true;
            }
        }
    }

    /// The page at `url` that's showing, made when the browser didn't say it started.
    fn current(&mut self, url: &str) -> &mut Page {
        let url = same_page(url);
        if self.pages.last().is_none_or(|page| page.url != url) {
            let mitigated = self.take_early(&url);
            self.pages.push(Page::new(url, mitigated));
        }
        let last = self.pages.len() - 1;
        &mut self.pages[last]
    }

    fn take_early(&mut self, url: &str) -> bool {
        let before = self.early.len();
        self.early.retain(|early| early != url);
        self.early.len() != before
    }

    pub fn outcome(&self) -> WindowOutcome {
        let pages: Vec<&Page> = self.pages.iter().filter(|page| page.web()).collect();
        match pages.iter().rposition(|page| page.is_check()) {
            Some(last) if pages[last + 1..].iter().any(|page| page.loaded()) => {
                WindowOutcome::Passed
            }
            Some(_) => WindowOutcome::NotPassed,
            None if pages.iter().any(|page| page.loaded()) => WindowOutcome::NoCheck,
            None => WindowOutcome::NotLoaded,
        }
    }

    /// Whether the page showing now is a check.
    pub fn showing_check(&self) -> bool {
        self.pages
            .iter()
            .rfind(|page| page.web())
            .is_some_and(Page::is_check)
    }
}

/// An address as pages are matched by: without its fragment, as the URL parser writes it.
fn same_page(url: &str) -> String {
    match Url::parse(url.trim()) {
        Ok(mut url) => {
            url.set_fragment(None);
            url.to_string()
        }
        Err(_) => url.trim().to_owned(),
    }
}

/// A cookie as the browser gave it.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct BrowserCookie {
    pub name: String,
    pub value: String,
    /// The cookie's domain; `None` keeps it to the page's host.
    pub domain: Option<String>,
    pub path: Option<String>,
    /// Unix seconds; `None` when the browser didn't say.
    pub expires: Option<i64>,
    pub secure: bool,
    pub http_only: bool,
}

impl BrowserCookie {
    /// The cookie as a `Set-Cookie` header's value, lasting until it expires or for
    /// [`UNDATED_LIFETIME_SECS`]; `None` for an expired cookie or one that can't be written.
    pub fn set_cookie(&self, now_secs: i64) -> Option<String> {
        let plain = |text: &str| !text.chars().any(|char| matches!(char, ';' | '\r' | '\n'));
        if self.name.is_empty()
            || self.name.contains('=')
            || !plain(&self.name)
            || !plain(&self.value)
        {
            return None;
        }
        let max_age = self.lifetime(now_secs)?;
        let mut header = format!("{}={}; Max-Age={max_age}", self.name, self.value);
        if let Some(domain) = self.domain.as_deref().filter(|domain| plain(domain)) {
            header.push_str("; Domain=");
            header.push_str(domain.trim_start_matches('.'));
        }
        header.push_str("; Path=");
        header.push_str(
            self.path
                .as_deref()
                .filter(|path| plain(path))
                .unwrap_or("/"),
        );
        if self.secure {
            header.push_str("; Secure");
        }
        if self.http_only {
            header.push_str("; HttpOnly");
        }
        Some(header)
    }

    /// Seconds it's kept from `now_secs`: until it expires, or [`UNDATED_LIFETIME_SECS`];
    /// `None` once expired.
    pub fn lifetime(&self, now_secs: i64) -> Option<i64> {
        match self.expires {
            Some(expires) if expires <= now_secs => None,
            Some(expires) => Some(expires - now_secs),
            None => Some(UNDATED_LIFETIME_SECS),
        }
    }
}

/// A `Cookie` header's pairs (`a=1; b=2`, what Android's `CookieManager.getCookie` gives).
pub fn parse_cookie_header(header: &str) -> Vec<BrowserCookie> {
    header
        .split(';')
        .filter_map(|pair| {
            let (name, value) = pair.split_once('=')?;
            let name = name.trim();
            (!name.is_empty()).then(|| BrowserCookie {
                name: name.to_owned(),
                value: value.trim().to_owned(),
                ..BrowserCookie::default()
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn met<'a>(detail: &'a str, request: Option<CheckedRequest>) -> Met<'a> {
        Met {
            extension_id: "x.y",
            source: "Novels",
            detail,
            website: None,
            request,
            call: None,
        }
    }

    #[test]
    fn the_challenged_site_is_read_from_each_runtime_s_details() {
        assert_eq!(
            challenged_site("comics.example answered HTTP 403 with a browser check\nTypeError: x")
                .as_deref(),
            Some("https://comics.example/")
        );
        assert_eq!(
            challenged_site("Foxaholic wants a browser check (HTTP 403 from www.foxaholic.com: a bot check page)")
                .as_deref(),
            Some("https://www.foxaholic.com/")
        );
        assert_eq!(
            challenged_site(
                "Royal Road wants a browser check that Raygun can't do. Try again later (www.royalroad.com: attempt to index nil)"
            )
            .as_deref(),
            Some("https://www.royalroad.com/")
        );
        assert_eq!(challenged_site("site.example answered HTTP 404"), None);
        assert_eq!(challenged_site("no host here"), None);
    }

    #[test]
    fn the_window_opens_the_request_that_met_the_check() {
        let searched = CheckedRequest::new(
            "https://novels.example/search?q=sword",
            "get",
            [
                ("Referer", "https://novels.example/"),
                ("User-Agent", "Source/1.0"),
                ("Cookie", "a=1"),
                ("Accept", "application/json"),
                ("Content-Type", "text/plain"),
                ("If-None-Match", "\"x\""),
                ("X-Requested-With", "XMLHttpRequest"),
                ("referer", "https://other.example/"),
            ],
            None,
            "Source/1.0",
        );
        assert!(
            searched
                .headers
                .iter()
                .all(|(name, _)| name != "User-Agent")
        );
        // The source's own request: its address, its headers a page may send, its User-Agent.
        let picked = pick_request(
            Some(searched.clone()),
            "novels.example answered HTTP 403 with a browser check",
            Some("https://novels.example/"),
        )
        .unwrap();
        assert_eq!(picked, searched);
        let window = picked.window().unwrap();
        assert_eq!(window.url, "https://novels.example/search?q=sword");
        assert_eq!(window.method, "GET");
        assert_eq!(window.body, None);
        assert_eq!(window.user_agent, "Source/1.0");
        assert_eq!(
            window.headers,
            BTreeMap::from([
                ("Referer".to_owned(), "https://novels.example/".to_owned()),
                ("X-Requested-With".to_owned(), "XMLHttpRequest".to_owned()),
            ])
        );

        // A POST stays one, with its body; a form's type when the source named none.
        let posted = CheckedRequest::new(
            "https://novels.example/wp-admin/admin-ajax.php",
            "POST",
            [],
            Some("action=search&q=sword"),
            "Source/1.0",
        )
        .window()
        .unwrap();
        assert_eq!(posted.method, "POST");
        assert_eq!(posted.body.as_deref(), Some("action=search&q=sword"));
        assert_eq!(posted.headers["Content-Type"], FORM);
        let json = CheckedRequest::new(
            "https://api.novels.example/graphql",
            "POST",
            [("content-type", "application/json")],
            Some("{}"),
            "Source/1.0",
        )
        .window()
        .unwrap();
        assert_eq!(
            json.headers,
            BTreeMap::from([("content-type".to_owned(), "application/json".to_owned())])
        );
        // Other methods are a visit of the address.
        let deleted = CheckedRequest::new("https://novels.example/x", "DELETE", [], None, "S")
            .window()
            .unwrap();
        assert_eq!((deleted.method.as_str(), deleted.body), ("GET", None));

        // Without the request: the site the details name, else the extension's website.
        let visit = pick_request(
            None,
            "novels.example answered HTTP 403 with a browser check",
            None,
        )
        .unwrap();
        assert_eq!(visit.url, "https://novels.example/");
        assert_eq!(visit.method, "GET");
        assert_eq!(visit.user_agent, raygun_net::user_agent());
        let not_web = CheckedRequest::visit("file:///etc/hosts");
        assert_eq!(not_web.window(), None);
        assert_eq!(
            pick_request(Some(not_web), "", Some("https://site.example/home"))
                .unwrap()
                .url,
            "https://site.example/home"
        );
        assert_eq!(pick_request(None, "", Some("ftp://site.example/")), None);
        assert_eq!(pick_request(None, "", None), None);
    }

    #[test]
    fn checks_are_found_by_the_source_a_failure_names() {
        let checks = Challenges::default();
        let detail = "novels.example answered HTTP 403 with a browser check";
        checks.note(None, met(detail, None));
        checks.note(Some(IssueKind::Denied), met(detail, None));
        assert_eq!(checks.find("Novels wants a browser check", None), None);

        let request =
            CheckedRequest::new("http://novels.example/list?page=2", "GET", [], None, "A");
        checks.note(
            Some(IssueKind::BotCheck),
            Met {
                call: Some(FailedCall {
                    provider: "x.y/novels".into(),
                    method: "popular".into(),
                    args: serde_json::json!({ "page": 2 }),
                }),
                ..met(detail, Some(request.clone()))
            },
        );
        checks.note(
            Some(IssueKind::BotCheck),
            Met {
                extension_id: "x.z",
                source: "Novels Plus",
                detail: "",
                website: Some("https://plus.example/home"),
                request: None,
                call: None,
            },
        );
        let found = checks
            .find(
                "Novels Plus wants a browser check, which Raygun can't do\n...",
                None,
            )
            .unwrap();
        assert_eq!(found.extension_id, "x.z");
        assert_eq!(found.url(), "https://plus.example/home");
        let novels = checks.find("anything", Some("novels")).unwrap();
        assert_eq!(novels.request, request);
        assert_eq!(novels.call.unwrap().method, "popular");
        assert_eq!(checks.find("Deezer didn't answer", None), None);

        checks.clear("x.y");
        assert_eq!(
            checks.find("Novels wants a browser check", Some("Novels")),
            None
        );
        assert!(checks.of_extension("x.z").is_some());
    }

    #[test]
    fn challenge_pages_are_told_by_their_title() {
        assert!(looks_like_challenge("Just a moment..."));
        assert!(looks_like_challenge("Attention Required! | Cloudflare"));
        assert!(looks_like_challenge("DDoS-Guard"));
        assert!(!looks_like_challenge("Royal Road"));
        assert!(!looks_like_challenge(""));
    }

    fn window(seen: &[Seen]) -> CheckWindow {
        let mut window = CheckWindow::default();
        for event in seen {
            window.see(event.clone());
        }
        window
    }

    fn start(url: &str) -> Seen {
        Seen::Start { url: url.into() }
    }

    fn finish(url: &str) -> Seen {
        Seen::Finish { url: url.into() }
    }

    fn title(title: &str) -> Seen {
        Seen::Title {
            title: title.into(),
        }
    }

    fn mitigated(url: &str) -> Seen {
        Seen::Answer {
            url: url.into(),
            status: 403,
            mitigated: true,
        }
    }

    const PAGE: &str = "https://novels.example/search?q=sword";

    #[test]
    fn a_check_s_answer_is_told_in_any_language_and_either_order() {
        // Android: the answer comes before its page starts; the title is translated.
        let passed = window(&[
            mitigated(PAGE),
            start(PAGE),
            title("Un instant…"),
            finish(PAGE),
            start(PAGE),
            title("Sword – Search"),
            finish(PAGE),
        ]);
        assert_eq!(passed.outcome(), WindowOutcome::Passed);
        assert!(!passed.showing_check());
        // Windows: the page starts first, then its answer.
        let checking = window(&[
            start("about:blank"),
            finish("about:blank"),
            start(PAGE),
            mitigated(&format!("{PAGE}#top")),
            title("Einen Moment bitte …"),
            finish(PAGE),
        ]);
        assert_eq!(checking.outcome(), WindowOutcome::NotPassed);
        assert!(checking.showing_check());
        // Then the same address answers with the page (JSON without a title) once passed.
        let mut answered = checking.clone();
        answered.see(start(PAGE));
        answered.see(finish(PAGE));
        assert_eq!(answered.outcome(), WindowOutcome::Passed);
    }

    #[test]
    fn checks_are_told_by_title_where_no_answer_says() {
        let check = [
            start("https://site.example/"),
            title("DDoS-Guard"),
            finish("https://site.example/"),
        ];
        assert_eq!(window(&check).outcome(), WindowOutcome::NotPassed);
        let mut passed = window(&check);
        // A check that sends the browser on: the page after it starts before the title changes.
        passed.see(start("https://site.example/novels"));
        passed.see(title("Novels"));
        assert_eq!(passed.outcome(), WindowOutcome::NotPassed, "not loaded yet");
        passed.see(finish("https://site.example/novels"));
        assert_eq!(passed.outcome(), WindowOutcome::Passed);
        // A check that shows again after one try is still not passed.
        passed.see(start("https://site.example/novels"));
        passed.see(title("Just a moment..."));
        passed.see(finish("https://site.example/novels"));
        assert_eq!(passed.outcome(), WindowOutcome::NotPassed);
    }

    #[test]
    fn a_page_without_a_check_is_told_apart() {
        let plain = window(&[start(PAGE), title("Sword – Search"), finish(PAGE)]);
        assert_eq!(plain.outcome(), WindowOutcome::NoCheck);
        assert!(!plain.showing_check());
        // A refusal isn't a check: no check's mark in its answer or title.
        let refused = window(&[
            Seen::Answer {
                url: PAGE.into(),
                status: 403,
                mitigated: false,
            },
            start(PAGE),
            title("403 Forbidden"),
            finish(PAGE),
        ]);
        assert_eq!(refused.outcome(), WindowOutcome::NoCheck);
        // Nothing loaded: no connection, or closed at once.
        let offline = window(&[start(PAGE), Seen::Failed { url: PAGE.into() }]);
        assert_eq!(offline.outcome(), WindowOutcome::NotLoaded);
        assert_eq!(window(&[]).outcome(), WindowOutcome::NotLoaded);
        assert_eq!(
            window(&[start("about:blank"), finish("about:blank")]).outcome(),
            WindowOutcome::NotLoaded
        );
        // An early answer for another address doesn't make this page a check.
        let elsewhere = window(&[
            mitigated("https://cdn.example/x.js"),
            start(PAGE),
            finish(PAGE),
        ]);
        assert_eq!(elsewhere.outcome(), WindowOutcome::NoCheck);
    }

    #[test]
    fn android_s_reports_are_read() {
        let seen: Vec<Seen> = serde_json::from_value(serde_json::json!([
            { "kind": "answer", "url": PAGE, "status": 403, "mitigated": true },
            { "kind": "start", "url": PAGE },
            { "kind": "title", "title": "Just a moment..." },
            { "kind": "finish", "url": PAGE },
            { "kind": "failed", "url": "https://other.example/" },
        ]))
        .unwrap();
        assert_eq!(seen[0], mitigated(PAGE));
        assert_eq!(seen.len(), 5);
    }

    #[test]
    fn browser_cookies_become_lasting_set_cookie_headers() {
        let clearance = BrowserCookie {
            name: "cf_clearance".into(),
            value: "abc".into(),
            domain: Some(".site.example".into()),
            path: None,
            expires: Some(1_000 + 3_600),
            secure: true,
            http_only: true,
        };
        assert_eq!(
            clearance.set_cookie(1_000).unwrap(),
            "cf_clearance=abc; Max-Age=3600; Domain=site.example; Path=/; Secure; HttpOnly"
        );
        assert_eq!(clearance.set_cookie(10_000), None, "expired");

        let pairs = parse_cookie_header("cf_clearance=abc; theme=dark ;=x; broken");
        assert_eq!(pairs.len(), 2);
        assert_eq!(
            pairs[1].set_cookie(0).unwrap(),
            format!("theme=dark; Max-Age={UNDATED_LIFETIME_SECS}; Path=/")
        );
        let bad = BrowserCookie {
            name: "a".into(),
            value: "b; Domain=evil.example".into(),
            ..BrowserCookie::default()
        };
        assert_eq!(bad.set_cookie(0), None);
    }
}
