//! The built-in browser on desktop (M46.6): each site in a window of its own, with its own browser
//! profile and no access to Raygun (remote pages get no IPC). A menu bar holds back, forward,
//! reload and home, and a Page menu the rest (with their shortcuts). The blocker answers listed ad
//! and tracker requests with 403 (WebView2's `WebResourceRequested`, on Windows) and keeps popups
//! to the site. Where a site was left and its switch are reported the way Android's browser
//! reports them. Files the site downloads show in a Downloads menu that appears with the first:
//! how far each is, then Open (or Retry).
//!
//! Site scripts (`super::scripts`) are initialization scripts of the window, after Raygun's page
//! bridge; switching one on or off makes the window again. Offers from pages (WebView2's web
//! messages, Windows) show in an "In Raygun" menu while the page has some.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use raygun_model::WebsiteDownloadState;
use serde::Deserialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};

use super::blocklist::{self, Blocklist};
use crate::website_downloads::browser::BrowserDownload;

/// Browser windows' labels start with this.
const LABEL_PREFIX: &str = "site-";
/// Between a window's label and the rest of a download's ticket.
const TICKET_MARK: char = '|';
/// Downloads a window's menu lists at most (the newest).
const MENU_DOWNLOADS: usize = 8;
/// The browser's profile, under the websites folder.
pub const PROFILE: &str = "desktop";
/// Left when the profile couldn't be deleted; it's deleted at the next start.
pub const CLEAR_MARK: &str = "clear-desktop-profile";

/// What `WebsiteService::open` sends the browser (the same as Android's).
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Request {
    id: String,
    url: String,
    home: String,
    block_ads: bool,
    block_popups: bool,
    blocklist: PathBuf,
    profile: PathBuf,
    #[serde(default)]
    background_play: bool,
    /// Raygun's page bridge (`super::scripts::BRIDGE`).
    #[serde(default)]
    bridge_code: String,
    #[serde(default)]
    site_scripts: Vec<SiteScripts>,
    #[serde(default)]
    suggestions: Vec<Suggestion>,
}

/// An extension's site scripts as the window gets them (`super::scripts::BrowserScripts`).
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct SiteScripts {
    id: String,
    name: String,
    #[serde(default)]
    code: String,
    on: bool,
}

/// Site scripts a repository has for the site (`super::scripts::Suggestion`).
#[derive(Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Suggestion {
    id: String,
    name: String,
    repository: String,
}

/// Offers the page made (Windows), newest first, and the menu that shows them.
#[derive(Default)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Offers {
    menu: Option<Submenu<Wry>>,
    list: Vec<Offer>,
    /// Names offers in menu ids, so a click finds the one it was made for.
    next: u64,
}

/// One offer, as checked lightly here; Raygun checks it again before it acts.
#[derive(Clone)]
#[cfg_attr(not(windows), allow(dead_code))]
struct Offer {
    number: u64,
    extension: String,
    page: String,
    kind: String,
    url: String,
    title: Option<String>,
    offer: serde_json::Value,
}

/// Offers kept per page, at most.
#[cfg_attr(not(windows), allow(dead_code))]
const MAX_OFFERS: usize = 20;
/// Offers the menu lists, the newest.
#[cfg_attr(not(windows), allow(dead_code))]
const MENU_OFFERS: usize = 5;

/// The open site windows, by label, for their Downloads menus.
#[derive(Default)]
struct OpenSites(Mutex<HashMap<String, Arc<Site>>>);

/// A window's Downloads menu, once something downloads, and what it lists (newest first).
#[derive(Default)]
struct DownloadsMenu {
    menu: Option<Submenu<Wry>>,
    items: Vec<(BrowserDownload, MenuItem<Wry>)>,
}

/// One open site.
struct Site {
    /// Its window's label.
    label: String,
    downloads: Mutex<DownloadsMenu>,
    id: String,
    home: Url,
    /// Where the page is (from the navigation that started it), for first-party requests.
    url: Mutex<Url>,
    title: Mutex<String>,
    block_ads: AtomicBool,
    block_popups: bool,
    blocked: AtomicU32,
    /// The last popup blocked, for "Open the blocked popup".
    popup: Mutex<Option<Url>>,
    popup_item: Mutex<Option<MenuItem<Wry>>>,
    list: Blocklist,
    /// The webview's User-Agent once read (Windows), for downloads.
    user_agent: Mutex<Option<String>>,
    scripts: Vec<SiteScripts>,
    suggestions: Vec<Suggestion>,
    offers: Mutex<Offers>,
    /// Pages are told when the window is minimized, so their own pauses can be told apart.
    background_play: bool,
    minimized: AtomicBool,
    /// Where reports go (`super::WebsiteReports`).
    send: Arc<dyn Fn(String) + Send + Sync>,
}

impl Site {
    fn same_site(&self, url: &Url) -> bool {
        let page = self.url.lock().expect("site poisoned");
        blocklist::site(url.host_str().unwrap_or_default())
            == blocklist::site(page.host_str().unwrap_or_default())
    }

    /// Whether a request is a listed ad or tracker. The page itself and its own site always load.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn blocks(&self, uri: &str, document: bool) -> bool {
        let Ok(url) = Url::parse(uri) else {
            return false;
        };
        if !matches!(url.scheme(), "http" | "https" | "ws" | "wss") {
            return false;
        }
        let Some(host) = url.host_str() else {
            return false;
        };
        if document && *self.url.lock().expect("site poisoned") == url {
            return false;
        }
        !self.same_site(&url) && self.list.blocks(host)
    }

    fn show_title(&self, window: &WebviewWindow) {
        let title = self.title.lock().expect("site poisoned").clone();
        let title = if title.trim().is_empty() {
            self.url
                .lock()
                .expect("site poisoned")
                .host_str()
                .unwrap_or_default()
                .to_owned()
        } else {
            title
        };
        let blocked = self.blocked.load(Ordering::Relaxed);
        let shown = if blocked > 0 {
            format!("{title} — {blocked} blocked")
        } else {
            title
        };
        let _ = window.set_title(&shown);
    }

    fn report(&self) -> String {
        serde_json::json!({
            "id": self.id,
            "url": self.url.lock().expect("site poisoned").as_str(),
            "blockAds": self.block_ads.load(Ordering::Relaxed),
        })
        .to_string()
    }
}

fn label(id: &str) -> String {
    let safe: String = id
        .chars()
        .filter(|char| char.is_ascii_alphanumeric() || *char == '-')
        .collect();
    format!("{LABEL_PREFIX}{safe}")
}

/// Opens a site, or brings its window forward when it's open already.
pub fn open(
    app: &AppHandle,
    request: &serde_json::Value,
    report: impl Fn(String) + Send + Sync + 'static,
) -> Result<(), String> {
    let request: Request =
        serde_json::from_value(request.clone()).map_err(|error| error.to_string())?;
    let label = label(&request.id);
    if let Some(window) = app.get_webview_window(&label) {
        let _ = window.unminimize();
        let _ = window.set_focus();
        return Ok(());
    }
    let url = Url::parse(&request.url).map_err(|error| error.to_string())?;
    let home = Url::parse(&request.home).map_err(|error| error.to_string())?;
    let site = Arc::new(Site {
        label: label.clone(),
        downloads: Mutex::new(DownloadsMenu::default()),
        id: request.id.clone(),
        home,
        url: Mutex::new(url.clone()),
        title: Mutex::new(String::new()),
        block_ads: AtomicBool::new(request.block_ads),
        block_popups: request.block_popups,
        blocked: AtomicU32::new(0),
        popup: Mutex::new(None),
        popup_item: Mutex::new(None),
        list: Blocklist::load(&request.blocklist),
        user_agent: Mutex::new(None),
        scripts: request
            .site_scripts
            .iter()
            .map(|script| SiteScripts {
                code: String::new(),
                ..script.clone()
            })
            .collect(),
        suggestions: request.suggestions.clone(),
        offers: Mutex::new(Offers::default()),
        background_play: request.background_play,
        minimized: AtomicBool::new(false),
        send: Arc::new(report),
    });
    let menu = menu(app, &label, &site).map_err(|error| error.to_string())?;
    let report = site.send.clone();
    let report_load = report.clone();

    let on_navigation = site.clone();
    let on_popup = (site.clone(), app.clone(), label.clone());
    let on_load = site.clone();
    let on_title = site.clone();
    #[cfg(not(windows))]
    let on_download = (site.clone(), app.clone(), label.clone());
    let builder = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url.clone()))
        .title(url.host_str().unwrap_or("Raygun"))
        .inner_size(1280.0, 860.0)
        .min_inner_size(480.0, 360.0)
        .data_directory(request.profile.clone())
        .menu(menu)
        .on_navigation(move |url| {
            // A top-level navigation: the page (and its site) that requests now belong to.
            if matches!(url.scheme(), "http" | "https") {
                *on_navigation.url.lock().expect("site poisoned") = url.clone();
            }
            // Links to other apps (mailto:, intent:, app schemes) never leave the window: only
            // "Open in your browser" hands anything to another program.
            matches!(url.scheme(), "http" | "https" | "about" | "blob" | "data")
        })
        .on_new_window(move |url, _features| {
            let (site, app, label) = &on_popup;
            let window = app.get_webview_window(label);
            if !site.block_popups || site.same_site(&url) {
                // Links meant for a new tab open here.
                if let Some(window) = &window {
                    let _ = window.navigate(url);
                }
            } else {
                site.blocked.fetch_add(1, Ordering::Relaxed);
                *site.popup.lock().expect("site poisoned") = Some(url);
                if let Some(item) = site.popup_item.lock().expect("site poisoned").as_ref() {
                    let _ = item.set_enabled(true);
                }
                if let Some(window) = &window {
                    site.show_title(window);
                }
            }
            NewWindowResponse::Deny
        })
        .on_page_load(move |window, payload| {
            if payload.event() == PageLoadEvent::Started {
                // A new page: the old one's offers go with it.
                on_load.clear_offers(&window);
            }
            if payload.event() == PageLoadEvent::Finished {
                *on_load.url.lock().expect("site poisoned") = payload.url().clone();
                on_load.show_title(&window);
                // Kept as it goes (M46.16), so the site reopens here even if Raygun quits first.
                report_load(on_load.report());
            }
        })
        .on_document_title_changed(move |window, title| {
            *on_title.title.lock().expect("site poisoned") = title;
            on_title.show_title(&window);
        });
    // Raygun's page bridge first, then each extension's scripts that are on here, in every frame
    // (each checks its own patterns and frames).
    let builder = std::iter::once(request.bridge_code.as_str())
        .chain(
            request
                .site_scripts
                .iter()
                .filter(|script| script.on)
                .map(|script| script.code.as_str()),
        )
        .filter(|code| !code.is_empty())
        .fold(builder, |builder, code| {
            builder.initialization_script_for_all_frames(code)
        });
    // Downloads are Raygun's (Windows: `DownloadStarting`, in `install_downloads`).
    #[cfg(not(windows))]
    let builder = builder.on_download(move |_webview, event| {
        let tauri::webview::DownloadEvent::Requested { url, destination } = event else {
            return true;
        };
        let (site, app, label) = &on_download;
        if url.scheme() == "blob" {
            // Only the page can read it: the webview saves it, into Downloads/Raygun.
            if let Some(path) = blob_destination(app, destination) {
                *destination = path;
            }
            return true;
        }
        let hint = destination
            .file_name()
            .map(|name| name.to_string_lossy().into_owned());
        crate::website_downloads::desktop::offer(
            app.clone(),
            label.clone(),
            site.download_request(url.to_string(), hint, None, None, None),
        );
        false
    });
    let window = builder.build().map_err(|error| error.to_string())?;

    let on_menu = site.clone();
    let prefix = format!("{label}:");
    window.on_menu_event(move |window, event| {
        let Some(action) = event.id().as_ref().strip_prefix(&prefix) else {
            return;
        };
        let Some(webview) = window.app_handle().get_webview_window(window.label()) else {
            return;
        };
        menu_action(&webview, &on_menu, action);
    });
    // Managed with the first window (later calls keep the one there).
    app.manage(OpenSites::default());
    if let Some(sites) = app.try_state::<OpenSites>() {
        sites
            .0
            .lock()
            .expect("sites poisoned")
            .insert(label.clone(), site.clone());
    }
    let on_close = site.clone();
    let watched = window.clone();
    window.on_window_event(move |event| match event {
        // Pages that change their address without loading (single-page sites) are caught when
        // the window is left or closed: the webview's own address is the one shown.
        tauri::WindowEvent::Focused(false) | tauri::WindowEvent::CloseRequested { .. } => {
            if let Ok(url) = watched.url()
                && matches!(url.scheme(), "http" | "https")
            {
                *on_close.url.lock().expect("site poisoned") = url;
            }
            report(on_close.report());
        }
        // Pages hear when the window is minimized and back (`raygunPage.state.background`).
        tauri::WindowEvent::Resized(_) if on_close.background_play => {
            let minimized = watched.is_minimized().unwrap_or(false);
            if on_close.minimized.swap(minimized, Ordering::Relaxed) != minimized {
                let _ = watched.eval(format!(
                    "window.__raygunPageReceive && window.__raygunPageReceive({{type:'state',background:{minimized},call:false,audioActive:false}})"
                ));
            }
        }
        tauri::WindowEvent::Destroyed => {
            report(on_close.report());
            if let Some(sites) = watched.app_handle().try_state::<OpenSites>() {
                let mut open = sites.0.lock().expect("sites poisoned");
                // A window opened again under the label since is left alone.
                if open
                    .get(&on_close.label)
                    .is_some_and(|site| Arc::ptr_eq(site, &on_close))
                {
                    open.remove(&on_close.label);
                }
            }
        }
        _ => {}
    });
    #[cfg(windows)]
    install_downloads(&window, site.clone());
    #[cfg(windows)]
    install_messages(&window, site.clone());
    #[cfg(windows)]
    install_blocker(&window, site);
    Ok(())
}

/// Closes a site's window and waits until it's gone, so it can be made again under its label.
pub async fn close(app: &AppHandle, id: &str) {
    let label = label(id);
    let Some(window) = app.get_webview_window(&label) else {
        return;
    };
    let _ = window.close();
    for _ in 0..60 {
        if app.get_webview_window(&label).is_none() {
            return;
        }
        tokio::time::sleep(std::time::Duration::from_millis(50)).await;
    }
}

impl Site {
    /// Forgets the page's offers (a new page started) and takes the menu down.
    fn clear_offers(&self, window: &WebviewWindow) {
        let mut offers = self.offers.lock().expect("site poisoned");
        if offers.list.is_empty() {
            return;
        }
        offers.list.clear();
        self.show_offers(window, &mut offers);
    }

    /// A message from the page's bridge (Windows' web messages): offers come and go.
    #[cfg_attr(not(windows), allow(dead_code))]
    fn take_message(&self, window: &WebviewWindow, source: &str, json: &str) {
        let Ok(serde_json::Value::Object(mut message)) = serde_json::from_str(json) else {
            return;
        };
        let Some(serde_json::Value::Object(message)) = message.remove("raygunBridge") else {
            return;
        };
        let text = |value: Option<&serde_json::Value>| {
            value.and_then(serde_json::Value::as_str).map(str::to_owned)
        };
        let Some(extension) = text(message.get("extension")) else {
            return;
        };
        // Only scripts this window runs offer anything.
        if !self
            .scripts
            .iter()
            .any(|script| script.on && script.id == extension)
        {
            return;
        }
        let mut offers = self.offers.lock().expect("site poisoned");
        match message.get("type").and_then(serde_json::Value::as_str) {
            Some("offer") => {
                let Some(offer) = message.get("offer").cloned() else {
                    return;
                };
                let kind = text(offer.get("kind")).unwrap_or_default();
                let url = text(offer.get("url")).unwrap_or_default();
                let web = (url.starts_with("https://") || url.starts_with("http://"))
                    && url.len() <= 2048;
                if !matches!(kind.as_str(), "video" | "song" | "playlist") || !web {
                    return;
                }
                let title = text(offer.get("title"))
                    .map(|title| title.chars().take(300).collect::<String>())
                    .filter(|title| !title.trim().is_empty());
                offers.list.retain(|known| known.url != url);
                offers.next += 1;
                let number = offers.next;
                offers.list.insert(
                    0,
                    Offer {
                        number,
                        extension,
                        page: source.to_owned(),
                        kind,
                        url,
                        title,
                        offer,
                    },
                );
                offers.list.truncate(MAX_OFFERS);
            }
            Some("withdraw") => match text(message.get("url")) {
                Some(url) => offers
                    .list
                    .retain(|known| known.extension != extension || known.url != url),
                None => offers.list.retain(|known| known.extension != extension),
            },
            _ => return,
        }
        self.show_offers(window, &mut offers);
    }

    /// The "In Raygun" menu, made again from the offers: each offer's own menu holds what
    /// Raygun can do with it. It's on the menu bar only while there are offers.
    fn show_offers(&self, window: &WebviewWindow, offers: &mut Offers) {
        if let (Some(menu), Some(bar)) = (offers.menu.take(), window.menu()) {
            let _ = bar.remove(&menu);
        }
        if offers.list.is_empty() {
            return;
        }
        match self.offers_menu(window.app_handle(), &offers.list) {
            Ok(menu) => {
                if let Some(bar) = window.menu()
                    && bar.append(&menu).is_ok()
                {
                    offers.menu = Some(menu);
                }
            }
            Err(error) => tracing::debug!(%error, "the browser's offers menu wasn't made"),
        }
    }

    fn offers_menu(&self, app: &AppHandle, list: &[Offer]) -> tauri::Result<Submenu<Wry>> {
        let menu = Submenu::with_id(app, format!("{}:offers", self.label), "▶ In Raygun", true)?;
        for offer in list.iter().take(MENU_OFFERS) {
            let id = |action: &str| format!("{}:offer:{}:{action}", self.label, offer.number);
            let mut actions = vec![("play", "Play in Raygun")];
            if offer.kind != "playlist" {
                actions.push(("queue", "Add to queue"));
            }
            if offer.kind == "song" {
                actions.push(("save", "Save to your library"));
            }
            if offer.kind != "playlist" {
                actions.push(("download", "Download"));
            }
            let title = offer.title.clone().unwrap_or_else(|| offer.url.clone());
            let kind = match offer.kind.as_str() {
                "song" => "Song",
                "playlist" => "Playlist",
                _ => "Video",
            };
            let items = actions
                .into_iter()
                .map(|(action, text)| MenuItem::with_id(app, id(action), text, true, None::<&str>))
                .collect::<tauri::Result<Vec<_>>>()?;
            let item_refs: Vec<&dyn tauri::menu::IsMenuItem<Wry>> = items
                .iter()
                .map(|item| item as &dyn tauri::menu::IsMenuItem<Wry>)
                .collect();
            let one = Submenu::with_items(
                app,
                menu_safe(&format!("{kind}: {}", shorten(&title, 60))),
                true,
                &item_refs,
            )?;
            menu.append(&one)?;
        }
        Ok(menu)
    }

    /// Hands an offer and what was picked for it to Raygun, which checks it and acts.
    fn offer_action(&self, window: &WebviewWindow, number: &str, action: &str) {
        let offer = {
            let offers = self.offers.lock().expect("site poisoned");
            offers
                .list
                .iter()
                .find(|offer| offer.number.to_string() == number)
                .cloned()
        };
        let Some(offer) = offer else {
            return;
        };
        if action == "play" {
            // Raygun plays it now; the page stops.
            let _ = window.eval(
                "document.querySelectorAll('video,audio').forEach(function(media){media.pause()})",
            );
        }
        (self.send)(
            serde_json::json!({
                "pageAction": {
                    "site": self.id,
                    "page": offer.page,
                    "extension": offer.extension,
                    "action": action,
                    "offer": offer.offer,
                }
            })
            .to_string(),
        );
    }

    /// The menu's switch for an extension's scripts on this site: Raygun keeps it and makes the
    /// window again, where it was.
    fn switch_scripts(&self, extension: &str) {
        let Some(script) = self.scripts.iter().find(|script| script.id == extension) else {
            return;
        };
        (self.send)(
            serde_json::json!({
                "siteScripts": {
                    "site": self.id,
                    "extension": extension,
                    "on": !script.on,
                    "page": self.url.lock().expect("site poisoned").as_str(),
                }
            })
            .to_string(),
        );
    }

    /// Installs site scripts a repository has for the site; the window is made again with them.
    fn install_scripts(&self, extension: &str) {
        let Some(suggestion) = self
            .suggestions
            .iter()
            .find(|suggestion| suggestion.id == extension)
        else {
            return;
        };
        (self.send)(
            serde_json::json!({
                "installSiteScripts": {
                    "site": self.id,
                    "extension": suggestion.id,
                    "repository": suggestion.repository,
                    "page": self.url.lock().expect("site poisoned").as_str(),
                }
            })
            .to_string(),
        );
    }

    /// A download from this window, before its place is picked.
    fn download_request(
        &self,
        url: String,
        name: Option<String>,
        mime: Option<String>,
        size: Option<f64>,
        disposition: Option<String>,
    ) -> crate::website_downloads::FileRequest {
        crate::website_downloads::FileRequest {
            url,
            user_agent: Some(
                self.user_agent
                    .lock()
                    .expect("site poisoned")
                    .clone()
                    .unwrap_or_else(raygun_net::user_agent),
            ),
            referer: Some(self.url.lock().expect("site poisoned").to_string()),
            cookies: None,
            content_disposition: disposition,
            name,
            mime,
            size,
            destination: crate::website_downloads::Destination::Downloads,
            ticket: Some(new_ticket(&self.label)),
        }
    }

    /// Shows changes to the window's downloads in its Downloads menu, made with the first.
    fn show_downloads(&self, window: &WebviewWindow, changes: &[&BrowserDownload]) {
        let app = window.app_handle();
        let mut shown = self.downloads.lock().expect("site poisoned");
        if shown.menu.is_none() {
            if changes
                .iter()
                .all(|change| change.state == WebsiteDownloadState::Cancelled)
            {
                return;
            }
            match self.downloads_menu(app, window) {
                Ok(menu) => shown.menu = Some(menu),
                Err(error) => {
                    tracing::warn!(%error, "the browser's Downloads menu wasn't made");
                    return;
                }
            }
        }
        let Some(menu) = shown.menu.clone() else {
            return;
        };
        for change in changes {
            let at = shown
                .items
                .iter()
                .position(|(known, _)| known.ticket == change.ticket);
            let result = match at {
                Some(index) if change.state == WebsiteDownloadState::Cancelled => {
                    let (_, item) = shown.items.remove(index);
                    menu.remove(&item)
                }
                Some(index) => {
                    let item = &shown.items[index].1;
                    let result = item
                        .set_text(menu_safe(&download_text(change)))
                        .and_then(|()| item.set_enabled(download_acts(change)));
                    shown.items[index].0 = (*change).clone();
                    result
                }
                None if change.state == WebsiteDownloadState::Cancelled => Ok(()),
                None => MenuItem::with_id(
                    app,
                    format!("{}:download:{}", self.label, change.ticket),
                    menu_safe(&download_text(change)),
                    download_acts(change),
                    None::<&str>,
                )
                .and_then(|item| {
                    menu.insert(&item, 0)?;
                    shown.items.insert(0, ((*change).clone(), item));
                    Ok(())
                }),
            };
            if let Err(error) = result {
                tracing::debug!(%error, "the browser's Downloads menu didn't change");
            }
        }
        while shown.items.len() > MENU_DOWNLOADS {
            if let Some((_, item)) = shown.items.pop() {
                let _ = menu.remove(&item);
            }
        }
        let listed: Vec<&BrowserDownload> = shown.items.iter().map(|(known, _)| known).collect();
        let _ = menu.set_text(downloads_title(&listed));
    }

    /// The Downloads menu, added to the window's menu bar: the downloads, then a way to Raygun's
    /// list of them.
    fn downloads_menu(
        &self,
        app: &AppHandle,
        window: &WebviewWindow,
    ) -> tauri::Result<Submenu<Wry>> {
        let all = MenuItem::with_id(
            app,
            format!("{}:downloads-all", self.label),
            "All downloads in Raygun",
            true,
            None::<&str>,
        )?;
        let menu = Submenu::with_id_and_items(
            app,
            format!("{}:downloads", self.label),
            "Downloads",
            true,
            &[&PredefinedMenuItem::separator(app)?, &all],
        )?;
        if let Some(bar) = window.menu() {
            bar.append(&menu)?;
        }
        Ok(menu)
    }
}

/// Shows the downloads site windows started in their Downloads menus. Waits on the main thread.
pub fn show_downloads(app: &AppHandle, downloads: &[BrowserDownload]) {
    let Some(sites) = app.try_state::<OpenSites>() else {
        return;
    };
    let mut by_window: HashMap<&str, Vec<&BrowserDownload>> = HashMap::new();
    for download in downloads {
        if let Some(label) = ticket_label(&download.ticket) {
            by_window.entry(label).or_default().push(download);
        }
    }
    for (label, changes) in by_window {
        let site = sites.0.lock().expect("sites poisoned").get(label).cloned();
        if let (Some(site), Some(window)) = (site, app.get_webview_window(label)) {
            site.show_downloads(&window, &changes);
        }
    }
}

/// A new download ticket for a window: its label, then a name of its own.
fn new_ticket(label: &str) -> String {
    format!("{label}{TICKET_MARK}{}", uuid::Uuid::now_v7())
}

/// The window a ticket was made for.
fn ticket_label(ticket: &str) -> Option<&str> {
    ticket
        .split_once(TICKET_MARK)
        .map(|(label, _)| label)
        .filter(|label| label.starts_with(LABEL_PREFIX))
}

/// A download's line in the menu: its name and how far it is, or what clicking it does.
fn download_text(download: &BrowserDownload) -> String {
    let name = &download.name;
    match download.state {
        WebsiteDownloadState::Queued => format!("{name} — waiting"),
        WebsiteDownloadState::Downloading => match download.total_bytes {
            Some(total) if total > 0.0 => format!(
                "{name} — {}% of {}",
                percent(download.done_bytes, total),
                crate::website_downloads::desktop::size_text(total)
            ),
            _ => format!(
                "{name} — {}",
                crate::website_downloads::desktop::size_text(download.done_bytes)
            ),
        },
        WebsiteDownloadState::Done => format!("Open {name}"),
        // One that never started (refused) has nothing to try again.
        WebsiteDownloadState::Failed if download.id.is_empty() => format!("{name} didn't download"),
        WebsiteDownloadState::Failed => format!("{name} didn't download — Retry"),
        WebsiteDownloadState::Cancelled => format!("{name} — cancelled"),
    }
}

/// Whether clicking a download's line does something: opens it, or tries it again.
fn download_acts(download: &BrowserDownload) -> bool {
    match download.state {
        WebsiteDownloadState::Done => true,
        WebsiteDownloadState::Failed => !download.id.is_empty(),
        _ => false,
    }
}

/// The Downloads menu's own name: how far the running ones are together, while any runs.
fn downloads_title(downloads: &[&BrowserDownload]) -> String {
    let running: Vec<&&BrowserDownload> = downloads
        .iter()
        .filter(|download| {
            matches!(
                download.state,
                WebsiteDownloadState::Queued | WebsiteDownloadState::Downloading
            )
        })
        .collect();
    if running.is_empty() {
        return "Downloads".into();
    }
    let sized = running
        .iter()
        .all(|download| download.total_bytes.is_some_and(|total| total > 0.0));
    if !sized {
        return "↓ Downloading".into();
    }
    let done: f64 = running.iter().map(|download| download.done_bytes).sum();
    let total: f64 = running
        .iter()
        .filter_map(|download| download.total_bytes)
        .sum();
    format!("↓ {}%", percent(done, total))
}

fn percent(done: f64, total: f64) -> u32 {
    ((done / total).clamp(0.0, 1.0) * 100.0).floor() as u32
}

/// `text` cut to `most` characters, with an ellipsis when it was longer.
fn shorten(text: &str, most: usize) -> String {
    if text.chars().count() <= most {
        return text.to_owned();
    }
    let mut cut: String = text.chars().take(most.saturating_sub(1)).collect();
    cut.push('…');
    cut
}

/// Menu text as written: Windows reads `&` as a shortcut mark.
fn menu_safe(text: &str) -> String {
    if cfg!(windows) {
        text.replace('&', "&&")
    } else {
        text.to_owned()
    }
}

/// Where a `blob:` download the webview saves itself goes: Downloads/Raygun, under a free name.
fn blob_destination(app: &AppHandle, suggested: &Path) -> Option<PathBuf> {
    let name = suggested.file_name()?.to_string_lossy().into_owned();
    let dir = app
        .try_state::<Arc<crate::export::ExportService>>()?
        .outputs()
        .folder(&crate::export::Place::Downloads);
    std::fs::create_dir_all(&dir).ok()?;
    let name = crate::website_downloads::names::unique_name(
        &crate::export::names::safe_name(&name),
        |candidate| dir.join(candidate).exists(),
    );
    Some(dir.join(name))
}

/// Takes the window's downloads (WebView2's `DownloadStarting`): each is cancelled in the webview
/// and offered to Raygun's downloads, except `blob:` ones, which only the page can read.
#[cfg(windows)]
fn install_downloads(window: &WebviewWindow, site: Arc<Site>) {
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    let started = window.with_webview(move |platform| {
        // SAFETY: as in `install_blocker`: on the UI thread, with its controller.
        if let Err(error) = unsafe { add_downloads(&platform.controller(), site, app, label) } {
            tracing::warn!(%error, "the browser's downloads didn't start");
        }
    });
    if let Err(error) = started {
        tracing::warn!(%error, "the browser's downloads didn't start");
    }
}

#[cfg(windows)]
unsafe fn add_downloads(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    site: Arc<Site>,
    app: AppHandle,
    label: String,
) -> windows::core::Result<()> {
    use webview2_com::DownloadStartingEventHandler;
    use webview2_com::Microsoft::Web::WebView2::Win32::{ICoreWebView2_4, ICoreWebView2Settings2};
    use windows::core::{HSTRING, Interface, PWSTR};

    unsafe {
        let webview = controller.CoreWebView2()?;
        // The page's User-Agent, for the download's requests.
        if let Ok(settings) = webview.Settings()?.cast::<ICoreWebView2Settings2>() {
            let mut agent = PWSTR::null();
            if settings.UserAgent(&mut agent).is_ok() {
                let agent = webview2_com::take_pwstr(agent);
                if !agent.is_empty() {
                    *site.user_agent.lock().expect("site poisoned") = Some(agent);
                }
            }
        }
        let handler = DownloadStartingEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let operation = args.DownloadOperation()?;
            let read = |get: &dyn Fn(*mut PWSTR) -> windows::core::Result<()>| {
                let mut value = PWSTR::null();
                get(&mut value)
                    .ok()
                    .map(|()| webview2_com::take_pwstr(value))
                    .filter(|value| !value.is_empty())
            };
            let uri = read(&|value| operation.Uri(value)).unwrap_or_default();
            let mut suggested = PWSTR::null();
            args.ResultFilePath(&mut suggested)?;
            let suggested = PathBuf::from(webview2_com::take_pwstr(suggested));
            if uri.starts_with("blob:") {
                if let Some(path) = blob_destination(&app, &suggested) {
                    args.SetResultFilePath(&HSTRING::from(path.as_os_str()))?;
                }
                return Ok(());
            }
            args.SetCancel(true)?;
            let disposition = read(&|value| operation.ContentDisposition(value));
            let mime = read(&|value| operation.MimeType(value));
            let mut total = 0i64;
            let size = operation
                .TotalBytesToReceive(&mut total)
                .ok()
                .filter(|()| total > 0)
                .map(|()| total as f64);
            let hint = suggested
                .file_name()
                .map(|name| name.to_string_lossy().into_owned());
            let request = site.download_request(uri, hint, mime, size, disposition);
            crate::website_downloads::desktop::offer(app.clone(), label.clone(), request);
            Ok(())
        }));
        let mut token = 0i64;
        webview
            .cast::<ICoreWebView2_4>()?
            .add_DownloadStarting(&handler, &mut token)?;
    }
    Ok(())
}

fn menu(app: &AppHandle, label: &str, site: &Arc<Site>) -> tauri::Result<Menu<Wry>> {
    let id = |name: &str| format!("{label}:{name}");
    // Plain buttons in the bar (Windows prints a top-level item's shortcut into its label); the
    // same actions with their shortcuts in the Page menu.
    let item = |name: &str, text: &str, keys: Option<&str>| {
        MenuItem::with_id(app, id(name), text, true, keys)
    };
    let back = item("back", "← Back", None)?;
    let forward = item("forward", "→ Forward", None)?;
    let reload = item("reload", "⟳ Reload", None)?;
    let home = item("home", "⌂ Home", None)?;
    let copy = item("copy", "Copy link", Some("CmdOrCtrl+Shift+C"))?;
    let browser = item("browser", "Open in your browser", None)?;
    let block = CheckMenuItem::with_id(
        app,
        id("block"),
        "Block ads on this site",
        true,
        site.block_ads.load(Ordering::Relaxed),
        None::<&str>,
    )?;
    let popup = MenuItem::with_id(
        app,
        id("popup"),
        "Open the blocked popup",
        false,
        None::<&str>,
    )?;
    *site.popup_item.lock().expect("site poisoned") = Some(popup.clone());
    let page_menu = Submenu::with_items(
        app,
        "Page",
        true,
        &[
            &item("back-key", "Back", Some("Alt+Left"))?,
            &item("forward-key", "Forward", Some("Alt+Right"))?,
            &item("reload-key", "Reload", Some("F5"))?,
            &item("home-key", "Home", Some("Alt+Home"))?,
            &PredefinedMenuItem::separator(app)?,
            &copy,
            &browser,
            &PredefinedMenuItem::separator(app)?,
            &block,
            &popup,
        ],
    )?;
    // Site scripts: a switch for each that runs here, and the repositories' ones to install.
    if !site.scripts.is_empty() || !site.suggestions.is_empty() {
        page_menu.append(&PredefinedMenuItem::separator(app)?)?;
        for script in &site.scripts {
            page_menu.append(&CheckMenuItem::with_id(
                app,
                id(&format!("script:{}", script.id)),
                menu_safe(&format!("Site scripts: {}", script.name)),
                true,
                script.on,
                None::<&str>,
            )?)?;
        }
        for suggestion in &site.suggestions {
            page_menu.append(&MenuItem::with_id(
                app,
                id(&format!("install:{}", suggestion.id)),
                menu_safe(&format!("Install {}…", suggestion.name)),
                true,
                None::<&str>,
            )?)?;
        }
    }
    if cfg!(target_os = "macos") {
        // macOS menus can't hold plain items at the top.
        Menu::with_items(app, &[&page_menu])
    } else {
        Menu::with_items(app, &[&back, &forward, &reload, &home, &page_menu])
    }
}

fn menu_action(window: &WebviewWindow, site: &Site, action: &str) {
    if let Some(ticket) = action.strip_prefix("download:") {
        download_action(window.app_handle(), ticket);
        return;
    }
    if action == "downloads-all" {
        show_raygun_downloads(window.app_handle());
        return;
    }
    if let Some(rest) = action.strip_prefix("offer:") {
        if let Some((number, what)) = rest.split_once(':') {
            site.offer_action(window, number, what);
        }
        return;
    }
    if let Some(extension) = action.strip_prefix("script:") {
        site.switch_scripts(extension);
        return;
    }
    if let Some(extension) = action.strip_prefix("install:") {
        site.install_scripts(extension);
        return;
    }
    let current = || site.url.lock().expect("site poisoned").clone();
    let done = match action.trim_end_matches("-key") {
        "back" => window.eval("history.back()"),
        "forward" => window.eval("history.forward()"),
        "reload" => window.eval("location.reload()"),
        "home" => window.navigate(site.home.clone()),
        "copy" => {
            if let Ok(mut clipboard) = arboard::Clipboard::new() {
                let _ = clipboard.set_text(current().to_string());
            }
            Ok(())
        }
        "browser" => {
            if let Err(error) = open_elsewhere(&current()) {
                tracing::warn!(%error, "the system browser didn't open");
            }
            Ok(())
        }
        "block" => {
            site.block_ads.fetch_xor(true, Ordering::Relaxed);
            window.eval("location.reload()")
        }
        "popup" => match site.popup.lock().expect("site poisoned").take() {
            Some(url) => window.navigate(url),
            None => Ok(()),
        },
        _ => Ok(()),
    };
    if let Err(error) = done {
        tracing::warn!(%error, action, "browser menu action failed");
    }
}

/// A download's line clicked: a finished one opens, a failed one is tried again.
fn download_action(app: &AppHandle, ticket: &str) {
    let Some(service) = app.try_state::<Arc<crate::website_downloads::WebsiteDownloadService>>()
    else {
        return;
    };
    let Some((id, state)) = service.by_ticket(ticket) else {
        return;
    };
    let done = match state {
        WebsiteDownloadState::Done => service.open(&id),
        WebsiteDownloadState::Failed => service.inner().retry(&id),
        _ => Ok(()),
    };
    if let Err(error) = done {
        tracing::info!(%error, "a browser download's menu item didn't work");
    }
}

/// Raygun's window, at Downloads → Files.
fn show_raygun_downloads(app: &AppHandle) {
    if let Some(window) = app.get_webview_window("main") {
        let _ = window.unminimize();
        let _ = window.set_focus();
    }
    if let Some(notices) = app.try_state::<Arc<crate::notifications::DownloadNotices>>() {
        notices.open("/downloads?tab=files".to_owned());
    }
}

/// Opens a web address in the system's browser. No shell: the address is one argument.
fn open_elsewhere(url: &Url) -> Result<(), String> {
    if !matches!(url.scheme(), "http" | "https") {
        return Err("Only web addresses open in a browser".into());
    }
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(windows, target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(url.as_str())
        .spawn()
        .map(|_| ())
        .map_err(|error| error.to_string())
}

/// Closes every browser window (before their profile is cleared).
pub fn close_all(app: &AppHandle) {
    for (label, window) in app.webview_windows() {
        if label.starts_with(LABEL_PREFIX) {
            let _ = window.close();
        }
    }
}

/// Deletes the browser profile, retrying while WebView2 lets go of it; if it never does, the
/// profile is deleted at the next start.
pub fn clear_profile(web: &Path) {
    let profile = web.join(PROFILE);
    let mark = web.join(CLEAR_MARK);
    std::thread::spawn(move || {
        for _ in 0..10 {
            if !profile.exists() || std::fs::remove_dir_all(&profile).is_ok() {
                let _ = std::fs::remove_file(&mark);
                return;
            }
            std::thread::sleep(std::time::Duration::from_millis(500));
        }
        let _ = std::fs::write(&mark, b"");
        tracing::info!("browser profile in use; it's cleared at the next start");
    });
}

/// Deletes a profile left to clear at the last run.
pub fn clear_marked(web: &Path) {
    let mark = web.join(CLEAR_MARK);
    if mark.exists() && std::fs::remove_dir_all(web.join(PROFILE)).is_ok() {
        let _ = std::fs::remove_file(mark);
    }
}

/// Hears the page bridge's messages (WebView2's `WebMessageReceived`): objects, which wry's
/// own text messages never are.
#[cfg(windows)]
fn install_messages(window: &WebviewWindow, site: Arc<Site>) {
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    let started = window.with_webview(move |platform| {
        // SAFETY: as in `install_blocker`: on the UI thread, with its controller.
        if let Err(error) = unsafe { add_messages(&platform.controller(), site, app, label) } {
            tracing::warn!(%error, "the browser's page messages didn't start");
        }
    });
    if let Err(error) = started {
        tracing::warn!(%error, "the browser's page messages didn't start");
    }
}

#[cfg(windows)]
unsafe fn add_messages(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    site: Arc<Site>,
    app: AppHandle,
    label: String,
) -> windows::core::Result<()> {
    use webview2_com::WebMessageReceivedEventHandler;
    use windows::core::PWSTR;

    unsafe {
        let webview = controller.CoreWebView2()?;
        let handler = WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            let mut source = PWSTR::null();
            args.Source(&mut source)?;
            let source = webview2_com::take_pwstr(source);
            let mut json = PWSTR::null();
            args.WebMessageAsJson(&mut json)?;
            let json = webview2_com::take_pwstr(json);
            if let Some(window) = app.get_webview_window(&label) {
                site.take_message(&window, &source, &json);
            }
            Ok(())
        }));
        let mut token = 0i64;
        webview.add_WebMessageReceived(&handler, &mut token)?;
    }
    Ok(())
}

/// Answers listed requests with 403 before they leave (WebView2's `WebResourceRequested`).
#[cfg(windows)]
fn install_blocker(window: &WebviewWindow, site: Arc<Site>) {
    let app = window.app_handle().clone();
    let label = window.label().to_owned();
    let started = window.with_webview(move |platform| {
        // SAFETY: WebView2 calls are made on the thread `with_webview` runs on (the UI thread),
        // with the controller it hands over.
        if let Err(error) = unsafe { add_blocker(&platform.controller(), site, app, label) } {
            tracing::warn!(%error, "the browser's blocker didn't start");
        }
    });
    if let Err(error) = started {
        tracing::warn!(%error, "the browser's blocker didn't start");
    }
}

#[cfg(windows)]
unsafe fn add_blocker(
    controller: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2Controller,
    site: Arc<Site>,
    app: AppHandle,
    label: String,
) -> windows::core::Result<()> {
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT, ICoreWebView2_2,
    };
    use webview2_com::WebResourceRequestedEventHandler;
    use windows::core::{HSTRING, Interface, PWSTR};

    unsafe {
        let webview = controller.CoreWebView2()?;
        webview.AddWebResourceRequestedFilter(
            &HSTRING::from("*"),
            COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        )?;
        let environment = webview.cast::<ICoreWebView2_2>()?.Environment()?;
        let reason = HSTRING::from("Blocked by Raygun");
        let no_headers = HSTRING::new();
        let handler = WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
            let Some(args) = args else {
                return Ok(());
            };
            if !site.block_ads.load(Ordering::Relaxed) {
                return Ok(());
            }
            let mut uri = PWSTR::null();
            args.Request()?.Uri(&mut uri)?;
            let uri = webview2_com::take_pwstr(uri);
            let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
            args.ResourceContext(&mut context)?;
            if !site.blocks(&uri, context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT) {
                return Ok(());
            }
            let response = environment.CreateWebResourceResponse(
                None::<&windows::Win32::System::Com::IStream>,
                403,
                &reason,
                &no_headers,
            )?;
            args.SetResponse(&response)?;
            site.blocked.fetch_add(1, Ordering::Relaxed);
            if let Some(window) = app.get_webview_window(&label) {
                site.show_title(&window);
            }
            Ok(())
        }));
        let mut token = 0i64;
        webview.add_WebResourceRequested(&handler, &mut token)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn download(state: WebsiteDownloadState, done: f64, total: Option<f64>) -> BrowserDownload {
        BrowserDownload {
            ticket: new_ticket("site-abc"),
            id: "1".into(),
            name: "Q&A.pdf".into(),
            state,
            done_bytes: done,
            total_bytes: total,
            folder: "Downloads/Raygun".into(),
            error: None,
            saved: None,
            mime: None,
        }
    }

    #[test]
    fn tickets_name_their_window() {
        let ticket = new_ticket("site-abc");
        assert_eq!(ticket_label(&ticket), Some("site-abc"));
        assert_ne!(new_ticket("site-abc"), ticket);
        // Android's (no window) and stray ones name none.
        assert_eq!(ticket_label("0190d9a0-8c3e-7"), None);
        assert_eq!(ticket_label("main|x"), None);
    }

    #[test]
    fn downloads_read_in_the_menu() {
        use WebsiteDownloadState::*;
        assert_eq!(
            download_text(&download(Downloading, 250_000.0, Some(1_000_000.0))),
            "Q&A.pdf — 25% of 1.0 MB"
        );
        assert_eq!(
            download_text(&download(Downloading, 2_500_000.0, None)),
            "Q&A.pdf — 2.5 MB"
        );
        assert_eq!(
            download_text(&download(Queued, 0.0, None)),
            "Q&A.pdf — waiting"
        );
        assert_eq!(
            download_text(&download(Done, 1.0, Some(1.0))),
            "Open Q&A.pdf"
        );
        assert_eq!(
            download_text(&download(Failed, 0.0, None)),
            "Q&A.pdf didn't download — Retry"
        );
        assert!(download_acts(&download(Done, 1.0, Some(1.0))));
        assert!(download_acts(&download(Failed, 0.0, None)));
        assert!(!download_acts(&download(Downloading, 0.0, None)));
        // A request that never started can't be tried again.
        let refused = BrowserDownload::refused(&new_ticket("site-abc"), "x.zip", "No");
        assert_eq!(download_text(&refused), "x.zip didn't download");
        assert!(!download_acts(&refused));
        if cfg!(windows) {
            assert_eq!(menu_safe("Q&A.pdf"), "Q&&A.pdf");
        }
    }

    #[test]
    fn the_menus_name_says_how_far_the_running_ones_are() {
        use WebsiteDownloadState::*;
        let half = download(Downloading, 500.0, Some(1000.0));
        let waiting = download(Queued, 0.0, Some(1000.0));
        let done = download(Done, 1000.0, Some(1000.0));
        assert_eq!(downloads_title(&[&half, &waiting, &done]), "↓ 25%");
        assert_eq!(downloads_title(&[&half]), "↓ 50%");
        let no_size = download(Downloading, 500.0, None);
        assert_eq!(downloads_title(&[&half, &no_size]), "↓ Downloading");
        assert_eq!(downloads_title(&[&done]), "Downloads");
        assert_eq!(downloads_title(&[]), "Downloads");
    }
}
