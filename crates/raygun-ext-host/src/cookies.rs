//! Cookies kept per extension (M46.11): one jar that the extension's HTTP client shares, saved as
//! `extensions/.data/<id>/cookies.json` so a site's choices (consent, language, a login the
//! extension made) last across restarts. Session cookies stay in memory, as in a browser.
//! Clearing an extension's data deletes the file with its storage.

use std::io::BufReader;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use cookie_store::{CookieStore, RawCookie};
use reqwest::Url;
use reqwest::header::HeaderValue;

#[derive(Debug)]
pub struct CookieJar {
    store: Mutex<CookieStore>,
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
        Arc::new(Self {
            store: Mutex::new(store),
            path,
        })
    }

    pub fn in_memory() -> Arc<Self> {
        Self::open(None)
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

        let reopened = CookieJar::open(Some(path));
        assert_eq!(reopened.cookies(&url).unwrap(), "consent=yes");
    }
}
