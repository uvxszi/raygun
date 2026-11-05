//! Cookies kept per extension (M46.11): one jar that the extension's HTTP client shares, saved as
//! `extensions/.data/<id>/cookies.json` so a site's choices (consent, language, a login the
//! extension made) last across restarts. Session cookies stay in memory, as in a browser.
//! Clearing an extension's data deletes the file with its storage. Cookies from a browser check
//! passed by hand come in through [`CookieJar::import`] (see `crate::browser_check`), with the
//! User-Agent the browser passed it with: a pass (Cloudflare's `cf_clearance`) holds only for
//! that User-Agent, so the extension's requests to the site go with it while the pass's cookies
//! last ([`CookieJar::user_agent_for`], `user-agents.json` beside the cookies).

use std::collections::HashMap;
use std::io::BufReader;
use std::path::{Path, PathBuf};
use std::sync::{Arc, LazyLock, Mutex, PoisonError, Weak};

use cookie_store::{CookieStore, RawCookie};
use reqwest::Url;
use reqwest::header::HeaderValue;
use serde::{Deserialize, Serialize};

use crate::browser_check::BrowserCookie;

/// Where the User-Agents of passed checks are kept, beside the cookies.
const USER_AGENTS_FILE: &str = "user-agents.json";

/// The User-Agent a site's browser check was passed with, and until when it counts (Unix
/// seconds: as long as the pass's cookies last).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct PassedAgent {
    agent: String,
    until: i64,
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_secs() as i64)
}

/// Whether `host` is `domain` or under it.
fn under(host: &str, domain: &str) -> bool {
    host == domain
        || host
            .strip_suffix(domain)
            .is_some_and(|start| start.ends_with('.'))
}

/// The jars in use, by file: cookies imported for an extension go into the jar its loaded copy
/// sends from. Each load opens a fresh jar from the file (so cleared data stays cleared); the
/// latest one opened is the one in use.
static OPEN: LazyLock<Mutex<HashMap<PathBuf, Weak<CookieJar>>>> = LazyLock::new(Default::default);

#[derive(Debug)]
pub struct CookieJar {
    store: Mutex<CookieStore>,
    /// The User-Agents of passed browser checks, by the domain their cookies are for.
    agents: Mutex<HashMap<String, PassedAgent>>,
    path: Option<PathBuf>,
}

impl CookieJar {
    /// Opens the jar saved at `path`, or an empty one kept in memory when `None`.
    pub fn open(path: Option<PathBuf>) -> Arc<Self> {
        let store = path
            .as_ref()
            .and_then(|path| std::fs::File::open(path).ok())
            .and_then(
                |file| match cookie_store::serde::json::load(BufReader::new(file)) {
                    Ok(store) => Some(store),
                    Err(error) => {
                        tracing::warn!(%error, "extension cookies unreadable, starting empty");
                        None
                    }
                },
            )
            .unwrap_or_default();
        let agents = path
            .as_ref()
            .and_then(|path| std::fs::read(path.with_file_name(USER_AGENTS_FILE)).ok())
            .and_then(|text| serde_json::from_slice(&text).ok())
            .unwrap_or_default();
        let jar = Arc::new(Self {
            store: Mutex::new(store),
            agents: Mutex::new(agents),
            path,
        });
        if let Some(path) = &jar.path {
            let mut open = OPEN.lock().unwrap_or_else(PoisonError::into_inner);
            open.retain(|_, jar| jar.strong_count() > 0);
            open.insert(path.clone(), Arc::downgrade(&jar));
        }
        jar
    }

    /// The jar in use for the file at `path`, else one opened from it.
    pub fn at(path: &Path) -> Arc<Self> {
        let open = OPEN
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .get(path)
            .and_then(Weak::upgrade);
        open.unwrap_or_else(|| Self::open(Some(path.to_path_buf())))
    }

    /// Adds cookies a browser gave for `url` (a browser check passed by hand) and saves them,
    /// with the `user_agent` the browser passed the check with: the extension's requests to the
    /// site (the address's host, and the domains its cookies name) go with it while those cookies
    /// last. Returns how many cookies were taken; cookies for another site or already expired are
    /// left out.
    pub fn import(&self, url: &Url, cookies: &[BrowserCookie], user_agent: Option<&str>) -> usize {
        let now = now_secs();
        let Ok(mut store) = self.store.lock() else {
            return 0;
        };
        let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
        let mut domains = vec![host.clone()];
        let mut until = None::<i64>;
        let mut taken = 0;
        for cookie in cookies {
            let Some(raw) = cookie
                .set_cookie(now)
                .and_then(|header| RawCookie::parse(header).ok())
            else {
                continue;
            };
            if store.insert_raw(&raw, url).is_ok() {
                taken += 1;
                let lasts = now + cookie.lifetime(now).unwrap_or_default();
                until = Some(until.map_or(lasts, |until| until.max(lasts)));
                let domain = cookie
                    .domain
                    .as_deref()
                    .map(|domain| domain.trim().trim_start_matches('.').to_ascii_lowercase())
                    .filter(|domain| domain.contains('.') && under(&host, domain));
                if let Some(domain) = domain
                    && !domains.contains(&domain)
                {
                    domains.push(domain);
                }
            }
        }
        if taken > 0 {
            self.save(&store);
        }
        drop(store);
        let agent = user_agent.map(str::trim).filter(|agent| !agent.is_empty());
        if let Some(agent) = agent
            && let Some(until) = until
            && !host.is_empty()
            && let Ok(mut agents) = self.agents.lock()
        {
            agents.retain(|_, passed| passed.until > now);
            for domain in domains {
                agents.insert(
                    domain,
                    PassedAgent {
                        agent: agent.to_owned(),
                        until,
                    },
                );
            }
            self.save_agents(&agents);
        }
        taken
    }

    /// The User-Agent a browser check for `url`'s site was passed with, while the pass's cookies
    /// last; the closest domain wins.
    pub fn user_agent_for(&self, url: &Url) -> Option<String> {
        let host = url.host_str()?.to_ascii_lowercase();
        let now = now_secs();
        let agents = self.agents.lock().ok()?;
        agents
            .iter()
            .filter(|(domain, passed)| passed.until > now && under(&host, domain))
            .max_by_key(|(domain, _)| domain.len())
            .map(|(_, passed)| passed.agent.clone())
    }

    pub fn in_memory() -> Arc<Self> {
        Self::open(None)
    }

    fn save_agents(&self, agents: &HashMap<String, PassedAgent>) {
        let Some(path) = &self.path else {
            return;
        };
        let path = path.with_file_name(USER_AGENTS_FILE);
        let write = || -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let temp = path.with_extension("json.tmp");
            std::fs::write(&temp, serde_json::to_vec(agents)?)?;
            std::fs::rename(&temp, &path)?;
            Ok(())
        };
        if let Err(error) = write() {
            tracing::warn!(%error, "could not save the User-Agents of passed checks");
        }
    }

    /// Writes the cookies that outlive the session (expired and session ones are left out).
    fn save(&self, store: &CookieStore) {
        let Some(path) = &self.path else {
            return;
        };
        let write = || -> Result<(), Box<dyn std::error::Error>> {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let temp = path.with_extension("json.tmp");
            let mut file = std::io::BufWriter::new(std::fs::File::create(&temp)?);
            cookie_store::serde::json::save(store, &mut file).map_err(|error| error.to_string())?;
            drop(file);
            std::fs::rename(&temp, path)?;
            Ok(())
        };
        if let Err(error) = write() {
            tracing::warn!(%error, "could not save an extension's cookies");
        }
    }
}

impl reqwest::cookie::CookieStore for CookieJar {
    fn set_cookies(&self, cookie_headers: &mut dyn Iterator<Item = &HeaderValue>, url: &Url) {
        let cookies: Vec<RawCookie<'static>> = cookie_headers
            .filter_map(|value| value.to_str().ok())
            .filter_map(|text| RawCookie::parse(text.to_owned()).ok())
            .collect();
        if cookies.is_empty() {
            return;
        }
        let Ok(mut store) = self.store.lock() else {
            return;
        };
        store.store_response_cookies(cookies.into_iter(), url);
        self.save(&store);
    }

    fn cookies(&self, url: &Url) -> Option<HeaderValue> {
        let store = self.store.lock().ok()?;
        let header = store
            .get_request_values(url)
            .map(|(name, value)| format!("{name}={value}"))
            .collect::<Vec<_>>()
            .join("; ");
        if header.is_empty() {
            None
        } else {
            HeaderValue::from_str(&header).ok()
        }
    }
}

#[cfg(test)]
mod tests {
    use reqwest::cookie::CookieStore as _;

    use super::*;

    #[test]
    fn lasting_cookies_are_saved_and_session_ones_are_not() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.y").join("cookies.json");
        let url = Url::parse("https://site.example/novels").unwrap();
        let jar = CookieJar::open(Some(path.clone()));
        let lasting = HeaderValue::from_static("consent=yes; Max-Age=3600; Path=/");
        let session = HeaderValue::from_static("visit=1; Path=/");
        jar.set_cookies(&mut [&lasting, &session].into_iter(), &url);
        let sent = jar.cookies(&url).unwrap();
        assert!(sent.to_str().unwrap().contains("consent=yes"));
        assert!(sent.to_str().unwrap().contains("visit=1"));
        assert!(
            jar.cookies(&Url::parse("https://other.example/").unwrap())
                .is_none()
        );

        drop(jar);
        let reopened = CookieJar::open(Some(path));
        assert_eq!(reopened.cookies(&url).unwrap(), "consent=yes");
    }

    #[test]
    fn a_browser_s_cookies_reach_the_jar_in_use_and_its_file() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.y").join("cookies.json");
        let url = Url::parse("https://www.site.example/").unwrap();
        let jar = CookieJar::open(Some(path.clone()));
        let clearance = BrowserCookie {
            name: "cf_clearance".into(),
            value: "solved".into(),
            domain: Some(".site.example".into()),
            ..BrowserCookie::default()
        };
        let elsewhere = BrowserCookie {
            name: "other".into(),
            value: "1".into(),
            domain: Some("other.example".into()),
            ..BrowserCookie::default()
        };
        assert_eq!(
            CookieJar::at(&path).import(&url, &[clearance, elsewhere], None),
            1
        );
        let sent = jar
            .cookies(&Url::parse("https://site.example/novel/1").unwrap())
            .unwrap();
        assert_eq!(sent, "cf_clearance=solved");
        assert_eq!(jar.user_agent_for(&url), None, "no User-Agent was given");

        drop(jar);
        let reopened = CookieJar::at(&path);
        assert_eq!(reopened.cookies(&url).unwrap(), "cf_clearance=solved");
    }

    #[test]
    fn a_passed_check_s_user_agent_goes_to_its_site_while_its_cookies_last() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("x.y").join("cookies.json");
        let url = Url::parse("https://www.site.example/search?q=a").unwrap();
        let jar = CookieJar::open(Some(path.clone()));
        let clearance = BrowserCookie {
            name: "cf_clearance".into(),
            value: "solved".into(),
            domain: Some(".site.example".into()),
            expires: Some(now_secs() + 1_800),
            ..BrowserCookie::default()
        };
        assert_eq!(jar.import(&url, &[clearance], Some(" Browser/2.0 ")), 1);
        let agent = |address: &str| jar.user_agent_for(&Url::parse(address).unwrap());
        assert_eq!(
            agent("https://www.site.example/").as_deref(),
            Some("Browser/2.0")
        );
        // The cookie's domain: every host under it.
        assert_eq!(
            agent("https://api.site.example/v1").as_deref(),
            Some("Browser/2.0")
        );
        assert_eq!(agent("https://othersite.example/"), None);
        assert_eq!(agent("https://example/"), None);

        // Kept across a restart, beside the cookies.
        assert!(dir.path().join("x.y").join(USER_AGENTS_FILE).exists());
        drop(jar);
        let reopened = CookieJar::open(Some(path));
        assert_eq!(
            reopened
                .user_agent_for(&Url::parse("https://www.site.example/x").unwrap())
                .as_deref(),
            Some("Browser/2.0")
        );

        // Nothing taken, nothing kept; a cookie that expires takes the User-Agent with it.
        let memory = CookieJar::in_memory();
        assert_eq!(memory.import(&url, &[], Some("Browser/2.0")), 0);
        assert_eq!(memory.user_agent_for(&url), None);
        let short = BrowserCookie {
            name: "__cf_bm".into(),
            value: "x".into(),
            expires: Some(now_secs() + 1),
            ..BrowserCookie::default()
        };
        assert_eq!(memory.import(&url, &[short], Some("Browser/3.0")), 1);
        assert_eq!(memory.user_agent_for(&url).as_deref(), Some("Browser/3.0"));
        memory
            .agents
            .lock()
            .unwrap()
            .values_mut()
            .for_each(|passed| passed.until = 0);
        assert_eq!(memory.user_agent_for(&url), None);
    }
}
