//! The built-in browser on desktop (M46.6): each site in a window of its own, with its own browser
//! profile and no access to Raygun (remote pages get no IPC). A menu bar holds back, forward,
//! reload and home, and a Page menu the rest (with their shortcuts). The blocker answers listed ad
//! and tracker requests with 403 (WebView2's `WebResourceRequested`, on Windows) and keeps popups
//! to the site. Where a site was left and its switch are reported the way Android's browser
//! reports them.

use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use serde::Deserialize;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::webview::{NewWindowResponse, PageLoadEvent};
use tauri::{AppHandle, Manager, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder, Wry};

use super::blocklist::{self, Blocklist};

/// Browser windows' labels start with this.
const LABEL_PREFIX: &str = "site-";
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
}

/// One open site.
struct Site {
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
    });
    let menu = menu(app, &label, &site).map_err(|error| error.to_string())?;
    let report = Arc::new(report);
    let report_load = report.clone();

    let on_navigation = site.clone();
    let on_popup = (site.clone(), app.clone(), label.clone());
    let on_load = site.clone();
    let on_title = site.clone();
    let window = WebviewWindowBuilder::new(app, &label, WebviewUrl::External(url.clone()))
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
            true
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
        })
        .build()
        .map_err(|error| error.to_string())?;

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
        tauri::WindowEvent::Destroyed => report(on_close.report()),
        _ => {}
    });
    #[cfg(windows)]
    install_blocker(&window, site);
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
    if cfg!(target_os = "macos") {
        // macOS menus can't hold plain items at the top.
        Menu::with_items(app, &[&page_menu])
    } else {
        Menu::with_items(app, &[&back, &forward, &reload, &home, &page_menu])
    }
}

fn menu_action(window: &WebviewWindow, site: &Site, action: &str) {
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
