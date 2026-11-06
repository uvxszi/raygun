//! Websites the user added and Raygun's built-in browser (M46.3).
//!
//! On Android the browser is `WebsiteActivity` in the `raygun-audio` plugin, in its own process;
//! on desktop it's a window of its own (`desktop`, M46.6). Rust keeps
//! the list, opens a site with its settings and the blocklist's path, keeps the blocklist
//! (downloaded and refreshed weekly, written in the form `Blocklist.kt` reads) and stores what
//! the browser reports: where a site was left and whether its blocker is on.

pub mod blocklist;
#[cfg(not(target_os = "android"))]
mod desktop;

use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use raygun_db::websites as store;
use raygun_model::{BlocklistStatus, Website, WebsitePrefs};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tokio::sync::Mutex;

use crate::catalog::{now_ms, to_message};
use crate::startup::AppDb;

/// Window ids of pages opened for sources (M46.11), as opposed to the user's sites.
pub const PAGE_PREFIX: &str = "source-";

const LIST_FILE: &str = "blocklist.bin";
const STATUS_KEY: &str = "web.blocklist";
/// The list is downloaded again when a site opens after this long.
const REFRESH: Duration = Duration::from_secs(7 * 24 * 60 * 60);
/// The biggest list read (HaGeZi PRO is about 4 MB).
const MAX_LIST_BYTES: usize = 32 * 1024 * 1024;

#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct StoredStatus {
    domains: u32,
    updated_at: Option<i64>,
    source: Option<String>,
}

pub struct WebsiteService {
    app: AppHandle,
    dir: PathBuf,
    client: reqwest::Client,
    /// One download at a time; the last one's error.
    updating: Mutex<Option<String>>,
}

/// An address as typed: `example.com/x` becomes `https://example.com/x`. Only web addresses.
pub fn normalize(input: &str) -> Result<url::Url, String> {
    let input = input.trim();
    let with_scheme = if input.contains("://") {
        input.to_owned()
    } else {
        format!("https://{input}")
    };
    let url = url::Url::parse(&with_scheme).map_err(|_| "That isn't a web address".to_owned())?;
    let host = url.host_str().unwrap_or_default();
    if !matches!(url.scheme(), "http" | "https") || host.is_empty() || !host.contains('.') {
        return Err("Enter a web address, like example.com".into());
    }
    Ok(url)
}

/// A site's name from its address: the host without `www.`.
/// A site's pin in quick access: Library → Websites, which opens it (`site`).
pub fn website_pin(id: &str, name: &str) -> crate::quick_access::QuickPin {
    crate::quick_access::QuickPin {
        kind: "website",
        to: "/library".to_owned(),
        search: [("media", "websites"), ("site", id)]
            .into_iter()
            .map(|(key, value)| (key.to_owned(), value.to_owned()))
            .collect(),
        name: name.to_owned(),
        artwork_url: None,
    }
}

pub fn default_name(url: &url::Url) -> String {
    let host = url.host_str().unwrap_or_default();
    host.strip_prefix("www.").unwrap_or(host).to_owned()
}

impl WebsiteService {
    pub fn new(app: AppHandle, dir: PathBuf) -> Result<Self, reqwest::Error> {
        // A desktop browser profile that was in use when its data was cleared goes now.
        #[cfg(not(target_os = "android"))]
        desktop::clear_marked(&dir);
        Ok(Self {
            app,
            dir,
            client: raygun_net::http_client()?,
            updating: Mutex::new(None),
        })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    async fn prefs(&self) -> WebsitePrefs {
        crate::settings::load(&self.db())
            .await
            .map(|settings| settings.websites)
            .unwrap_or_default()
    }

    fn view(&self, site: store::Website) -> Website {
        let host = url::Url::parse(&site.url)
            .ok()
            .and_then(|url| url.host_str().map(str::to_owned))
            .unwrap_or_default();
        let images = self.app.state::<raygun_net::ImageLinks>();
        let icon = (!host.is_empty()).then(|| {
            images.kept_cover(&format!("https://{host}/favicon.ico"), &Default::default())
        });
        Website {
            id: site.id,
            name: site.name,
            url: site.url,
            host,
            icon,
            last_url: site.last_url,
            block_ads: site.block_ads,
        }
    }

    pub async fn list(&self) -> Result<Vec<Website>, String> {
        Ok(store::list(&self.db())
            .await
            .map_err(to_message)?
            .into_iter()
            .map(|site| self.view(site))
            .collect())
    }

    /// Adds a site; one added before at the same address is given back instead of a second copy.
    /// The blocklist is fetched now if it's missing, so it's there when the site first opens.
    pub async fn add(self: &Arc<Self>, name: Option<String>, url: &str) -> Result<Website, String> {
        let url = normalize(url)?;
        let name = name
            .map(|name| name.trim().to_owned())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| default_name(&url));
        let db = self.db();
        if let Some(site) = store::list(&db)
            .await
            .map_err(to_message)?
            .into_iter()
            .find(|site| site.url == url.as_str())
        {
            return Ok(self.view(site));
        }
        let id = store::add(&db, &name, url.as_str())
            .await
            .map_err(to_message)?;
        let site = store::get(&db, &id)
            .await
            .map_err(to_message)?
            .ok_or("The site wasn't saved")?;
        self.refresh_list_later().await;
        crate::quick_access::add(&self.app, website_pin(&site.id, &site.name)).await;
        Ok(self.view(site))
    }

    pub async fn edit(&self, id: &str, name: &str, url: &str) -> Result<(), String> {
        let url = normalize(url)?;
        let name = match name.trim() {
            "" => default_name(&url),
            name => name.to_owned(),
        };
        store::edit(&self.db(), id, &name, url.as_str())
            .await
            .map_err(to_message)
    }

    pub async fn remove(&self, id: &str) -> Result<(), String> {
        store::remove(&self.db(), id).await.map_err(to_message)?;
        let pin = website_pin(id, "");
        crate::quick_access::remove(&self.app, &pin.to, &pin.search).await;
        Ok(())
    }

    /// Turns a site's own ad blocker on or off, as its menu in the browser does.
    pub async fn set_block_ads(&self, id: &str, on: bool) -> Result<(), String> {
        let db = self.db();
        let site = store::get(&db, id)
            .await
            .map_err(to_message)?
            .ok_or("The site was removed")?;
        store::set_state(&db, id, site.last_url.as_deref(), on)
            .await
            .map_err(to_message)
    }

    /// Forgets where a site was left: it opens at its address again.
    pub async fn reset(&self, id: &str) -> Result<(), String> {
        let db = self.db();
        let site = store::get(&db, id)
            .await
            .map_err(to_message)?
            .ok_or("The site was removed")?;
        store::set_state(&db, id, None, site.block_ads)
            .await
            .map_err(to_message)
    }

    /// Opens a site in the built-in browser, where it was left.
    pub async fn open(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let site = store::get(&self.db(), id)
            .await
            .map_err(to_message)?
            .ok_or("The site was removed")?;
        let url = site.last_url.as_deref().unwrap_or(&site.url).to_owned();
        self.show(&site.id, &url, &site.url, site.block_ads).await
    }

    /// Opens an address that isn't one of the user's sites, such as a source's website (M46.11).
    /// `key` (starting with [`PAGE_PREFIX`]) names its window; nothing about it is stored.
    pub async fn open_page(self: &Arc<Self>, key: &str, url: &str) -> Result<(), String> {
        let url = normalize(url)?;
        self.show(key, url.as_str(), url.as_str(), true).await
    }

    async fn show(
        self: &Arc<Self>,
        id: &str,
        url: &str,
        home: &str,
        site_blocks_ads: bool,
    ) -> Result<(), String> {
        let prefs = self.prefs().await;
        let list = self.dir.join(LIST_FILE);
        self.refresh_list_later().await;
        let request = serde_json::json!({
            "id": id,
            "url": url,
            "home": home,
            "blockAds": prefs.block_ads && site_blocks_ads,
            "blockPopups": prefs.block_popups,
            "desktop": prefs.desktop,
            "blocklist": list.to_string_lossy(),
            // The desktop browser's own profile (M46.6).
            "profile": self.dir.join("desktop").to_string_lossy(),
        });
        self.launch(&request)
    }

    #[cfg(target_os = "android")]
    fn launch(&self, request: &serde_json::Value) -> Result<(), String> {
        self.app
            .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .website_open(request)
    }

    #[cfg(not(target_os = "android"))]
    fn launch(&self, request: &serde_json::Value) -> Result<(), String> {
        let reports = self.app.state::<WebsiteReports>().0.clone();
        desktop::open(&self.app, request, move |json| {
            let _ = reports.send(json);
        })
    }

    /// Downloads the blocklist in the background when the blocker is on and the list is missing,
    /// a week old, or another list was chosen. A download already running isn't started twice.
    async fn refresh_list_later(self: &Arc<Self>) {
        if !self.prefs().await.block_ads || !self.list_is_old().await {
            return;
        }
        if self.updating.try_lock().is_err() {
            return;
        }
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            if let Err(error) = service.update_blocklist().await {
                tracing::info!(%error, "blocklist not updated");
            }
        });
    }

    /// Clears the browser's cookies, storage and cache.
    pub fn clear_data(&self) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            self.app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .website_clear()
        }
        #[cfg(not(target_os = "android"))]
        {
            desktop::close_all(&self.app);
            desktop::clear_profile(&self.dir);
            Ok(())
        }
    }

    async fn stored_status(&self) -> StoredStatus {
        raygun_db::local_value(&self.db(), STATUS_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    }

    async fn list_is_old(&self) -> bool {
        let status = self.stored_status().await;
        let source_changed = status.source != self.prefs().await.list_url();
        let old = status
            .updated_at
            .is_none_or(|at| now_ms().saturating_sub(at) > REFRESH.as_millis() as i64);
        old || source_changed || !self.dir.join(LIST_FILE).exists()
    }

    pub async fn status(&self) -> BlocklistStatus {
        let stored = self.stored_status().await;
        BlocklistStatus {
            domains: stored.domains,
            updated_at: stored.updated_at.map(|at| at as f64),
            source: stored.source,
            error: self.updating.lock().await.clone(),
        }
    }

    /// Downloads the chosen list and writes it for the browser.
    pub async fn update_blocklist(&self) -> Result<BlocklistStatus, String> {
        let mut last_error = self.updating.lock().await;
        let result = self.download_list().await;
        *last_error = result.as_ref().err().cloned();
        drop(last_error);
        result?;
        Ok(self.status().await)
    }

    async fn download_list(&self) -> Result<(), String> {
        let source = self
            .prefs()
            .await
            .list_url()
            .ok_or("Choose a blocklist first")?;
        let response = self
            .client
            .get(&source)
            .timeout(Duration::from_secs(60))
            .send()
            .await
            .map_err(|error| format!("The list couldn't be downloaded: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "The list couldn't be downloaded (HTTP {})",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > MAX_LIST_BYTES as u64)
        {
            return Err("The list is too large".into());
        }
        let bytes = response.bytes().await.map_err(to_message)?;
        if bytes.len() > MAX_LIST_BYTES {
            return Err("The list is too large".into());
        }
        let text = String::from_utf8_lossy(&bytes).into_owned();
        let dir = self.dir.clone();
        let domains = tauri::async_runtime::spawn_blocking(move || {
            let domains = blocklist::parse(&text);
            if domains.is_empty() {
                return Err("The list has no domains in it".to_owned());
            }
            std::fs::create_dir_all(&dir).map_err(to_message)?;
            let partial = dir.join(format!("{LIST_FILE}.part"));
            std::fs::write(&partial, blocklist::encode(&domains)).map_err(to_message)?;
            std::fs::rename(&partial, dir.join(LIST_FILE)).map_err(to_message)?;
            Ok(domains.len())
        })
        .await
        .map_err(to_message)??;
        let status = StoredStatus {
            domains: domains.min(u32::MAX as usize) as u32,
            updated_at: Some(now_ms()),
            source: Some(source),
        };
        raygun_db::set_local_value(
            &self.db(),
            STATUS_KEY,
            &serde_json::to_string(&status).map_err(to_message)?,
        )
        .await
        .map_err(to_message)?;
        tracing::info!(domains, "blocklist updated");
        Ok(())
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Report {
    id: String,
    url: Option<String>,
    block_ads: bool,
}

/// The site's own blocker after a report. With the blocker off in Settings the browser opens
/// every site unblocked and reports it so; that says nothing about the site's own switch, which
/// stays as it was unless the site's menu turned it on.
async fn site_blocks_ads(db: &raygun_db::Db, id: &str, reported: bool) -> bool {
    if reported {
        return true;
    }
    let everywhere = crate::settings::load(db)
        .await
        .map(|settings| settings.websites.block_ads)
        .unwrap_or(true);
    if everywhere {
        return false;
    }
    store::get(db, id)
        .await
        .ok()
        .flatten()
        .is_none_or(|site| site.block_ads)
}

/// The downloads site windows started, in each window's Downloads menu (desktop). Waits on the
/// main thread.
#[cfg(not(target_os = "android"))]
pub fn show_downloads(
    app: &AppHandle,
    downloads: &[crate::website_downloads::browser::BrowserDownload],
) {
    desktop::show_downloads(app, downloads);
}

/// Where the browser's reports go: Android's over JNI, the desktop windows' when they close.
/// Only the desktop windows hold the sender.
#[cfg_attr(target_os = "android", allow(dead_code))]
struct WebsiteReports(tokio::sync::mpsc::UnboundedSender<String>);

/// The browser's reports (where a site is, its blocker) become the site's stored state.
pub fn install(app: &AppHandle) {
    let (sender, mut reports) = tokio::sync::mpsc::unbounded_channel::<String>();
    app.manage(WebsiteReports(sender.clone()));
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        while let Some(json) = reports.recv().await {
            // A download the browser's sheet sent here (`{"download": …}`), or a question about
            // the ones it started (`{"downloadAsk": …}`).
            if let Ok(serde_json::Value::Object(mut fields)) =
                serde_json::from_str::<serde_json::Value>(&json)
            {
                if let Some(download) = fields.remove("download") {
                    crate::website_downloads::take_request(&app, download);
                    continue;
                }
                if let Some(ask) = fields.remove("downloadAsk") {
                    crate::website_downloads::take_ask(&app, ask);
                    continue;
                }
            }
            let Ok(report) = serde_json::from_str::<Report>(&json) else {
                tracing::warn!(%json, "unreadable browser report");
                continue;
            };
            // Pages opened for sources aren't the user's sites: nothing to remember.
            if report.id.starts_with(PAGE_PREFIX) {
                continue;
            }
            let url = report
                .url
                .filter(|url| url.starts_with("https://") || url.starts_with("http://"));
            let db = app.state::<AppDb>().inner().0.clone();
            // Incognito (M47): the site stays where it was left before.
            let url = crate::privacy::site_address(&app, &db, &report.id, url).await;
            let block_ads = site_blocks_ads(&db, &report.id, report.block_ads).await;
            if let Err(error) = store::set_state(&db, &report.id, url.as_deref(), block_ads).await {
                tracing::warn!(%error, "could not store a site's state");
            }
        }
    });
    tauri_plugin_raygun_audio::set_website_handler(move |json| {
        let _ = sender.send(json);
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_completed_and_checked() {
        assert_eq!(
            normalize("archive.org/details/x").unwrap().as_str(),
            "https://archive.org/details/x"
        );
        assert_eq!(
            normalize(" http://example.com ").unwrap().as_str(),
            "http://example.com/"
        );
        assert!(normalize("ftp://example.com").is_err());
        assert!(normalize("javascript:alert(1)").is_err());
        assert!(normalize("localhost").is_err());
        assert!(normalize("").is_err());
        assert_eq!(
            default_name(&normalize("www.wikipedia.org").unwrap()),
            "wikipedia.org"
        );
    }

    #[tokio::test]
    async fn a_report_with_the_blocker_off_everywhere_keeps_the_sites_own_switch() {
        let temp = tempfile::TempDir::new().unwrap();
        let db = raygun_db::Db::open(&temp.path().join("raygun.db"))
            .await
            .unwrap();
        let id = store::add(&db, "Archive", "https://archive.org/")
            .await
            .unwrap();

        // The blocker is on in Settings: the browser's word goes.
        assert!(!site_blocks_ads(&db, &id, false).await);
        assert!(site_blocks_ads(&db, &id, true).await);

        // Off in Settings, every site opens unblocked and is reported so: its switch stays.
        crate::settings::update(
            &db,
            raygun_model::SettingChange::Websites(WebsitePrefs {
                block_ads: false,
                ..WebsitePrefs::default()
            }),
        )
        .await
        .unwrap();
        assert!(site_blocks_ads(&db, &id, false).await);
        store::set_state(&db, &id, None, false).await.unwrap();
        assert!(!site_blocks_ads(&db, &id, false).await);
        // Turned on in the site's menu, it's on.
        assert!(site_blocks_ads(&db, &id, true).await);
    }
}
