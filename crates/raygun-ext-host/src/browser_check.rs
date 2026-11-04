//! Browser checks passed by hand (experimental, off unless `Settings::manual_browser_checks` is on).
//!
//! Some sites answer with a bot check (Cloudflare's "Just a moment...", DDoS-Guard, a captcha)
//! instead of the page. Raygun never solves, skips or automates such a check. With the setting on,
//! the person can open the site in a visible browser and pass the check themselves, as in Mihon's
//! "Open in WebView":
//!
//! 1. A provider call that fails with [`IssueKind::BotCheck`] is noted here by
//!    `ExtensionHost::call` ([`Challenges::note`]): the extension, the source's name, and the site
//!    that answered with the check (read from the failure's details, else the extension's
//!    website).
//! 2. `SourceIssue` in the UI asks `browser_check_target` for the failure it shows
//!    ([`Challenges::find`]); with the setting on, it offers "Open the site to pass the check".
//! 3. `raygun-app/src/browser_check.rs` opens the site with the User-Agent sources send: a Tauri
//!    window on desktop (its own profile), `BrowserCheckActivity` in the `raygun-audio` plugin on
//!    Android (its own process and WebView data). The browser closes when the person closes it, or
//!    once a page that isn't a check follows one ([`looks_like_challenge`] on the page's title).
//! 4. The site's cookies (such as `cf_clearance`) go into the extension's cookie jar
//!    ([`crate::cookies::CookieJar::import`], the jar JavaScript, Miru and Shosetsu sources share),
//!    the source's failure record is forgotten, and the UI asks again.

use std::collections::HashMap;
use std::sync::{Mutex, PoisonError};

use raygun_net::issue::IssueKind;
use reqwest::Url;

/// How long a cookie the browser gave without an expiry (Android's `CookieManager` gives only
/// names and values) is kept: long enough to outlast a restart; the site asks again when it no
/// longer takes it.
pub const UNDATED_LIFETIME_SECS: i64 = 7 * 24 * 60 * 60;

/// A source that met a browser check, and where.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Challenge {
    pub extension_id: String,
    /// The source's name for people, as failures name it.
    pub source: String,
    /// The page to open: the challenged site's root.
    pub url: String,
}

/// The latest browser check of each source, by its name.
#[derive(Debug, Default)]
pub struct Challenges(Mutex<HashMap<String, Challenge>>);

impl Challenges {
    fn map(&self) -> std::sync::MutexGuard<'_, HashMap<String, Challenge>> {
        self.0.lock().unwrap_or_else(PoisonError::into_inner)
    }

    /// Notes a failed call of `extension_id`'s source `source`, when it met a browser check.
    /// `detail` is the failure's details; `website` the extension's own site, used when the
    /// details don't name the challenged one.
    pub fn note(
        &self,
        kind: Option<IssueKind>,
        extension_id: &str,
        source: &str,
        detail: &str,
        website: Option<&str>,
    ) {
        if kind != Some(IssueKind::BotCheck) {
            return;
        }
        let url = challenged_site(detail).or_else(|| {
            let site = Url::parse(website?.trim()).ok()?;
            matches!(site.scheme(), "http" | "https").then(|| site.to_string())
        });
        let Some(url) = url else {
            return;
        };
        self.map().insert(
            source.to_lowercase(),
            Challenge {
                extension_id: extension_id.to_owned(),
                source: source.to_owned(),
                url,
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
/// `BrowserCheckActivity.kt`.
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
        let max_age = match self.expires {
            Some(expires) if expires <= now_secs => return None,
            Some(expires) => expires - now_secs,
            None => UNDATED_LIFETIME_SECS,
        };
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

    #[test]
    fn the_challenged_site_is_read_from_each_runtime_s_details() {
        assert_eq!(
            challenged_site("mangadex.org answered HTTP 403 with a browser check\nTypeError: x")
                .as_deref(),
            Some("https://mangadex.org/")
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
    fn checks_are_found_by_the_source_a_failure_names() {
        let checks = Challenges::default();
        let detail = "novels.example answered HTTP 403 with a browser check";
        checks.note(None, "x.y", "Novels", detail, None);
        checks.note(Some(IssueKind::Denied), "x.y", "Novels", detail, None);
        assert_eq!(checks.find("Novels wants a browser check", None), None);

        checks.note(Some(IssueKind::BotCheck), "x.y", "Novels", detail, None);
        checks.note(
            Some(IssueKind::BotCheck),
            "x.z",
            "Novels Plus",
            "",
            Some("https://plus.example/home"),
        );
        let found = checks
            .find(
                "Novels Plus wants a browser check, which Raygun can't do\n...",
                None,
            )
            .unwrap();
        assert_eq!(found.extension_id, "x.z");
        assert_eq!(found.url, "https://plus.example/home");
        assert_eq!(
            checks.find("anything", Some("novels")).unwrap().url,
            "https://novels.example/"
        );
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
