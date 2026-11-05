//! Opening the check that blocks a source so the person can pass it by hand (experimental). The
//! whole flow is described in `raygun_ext_host::browser_check`; this module is the app's part:
//! the setting gate (`Settings::manual_browser_checks`, off by default and turned on only after
//! the UI's warning), and the visible browser, which Raygun never drives: a Tauri window on
//! desktop, `BrowserCheckActivity` in the `raygun-audio` plugin on Android, both with their own
//! browser data. The browser opens the request that met the check with the User-Agent the source
//! sent, and reports what it saw of its pages, its cookies for the address and its User-Agent;
//! the host then tells whether the check was passed, takes the pass and asks the failed call
//! again.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

#[cfg(not(target_os = "android"))]
use raygun_ext_host::browser_check::WindowOutcome;
use raygun_ext_host::browser_check::{BrowserCookie, CheckWindow, WindowRequest};
use raygun_model::{BrowserCheckResult, BrowserCheckTarget, Settings};
use tauri::AppHandle;

use crate::extensions::ExtensionService;

/// Whether the person turned on solving browser checks themselves.
pub fn allowed(settings: &Settings) -> Result<(), String> {
    if settings.manual_browser_checks {
        Ok(())
    } else {
        Err("Solving browser checks yourself is off (Settings → Browse)".to_owned())
    }
}

/// What the browser gave back once closed.
struct Browsed {
    /// Its cookies for the checked address.
    cookies: Vec<BrowserCookie>,
    /// The User-Agent it used, which a pass holds for.
    user_agent: Option<String>,
    /// What it saw of its pages.
    window: CheckWindow,
}

pub struct BrowserChecks {
    app: AppHandle,
    /// The desktop browser's own data (`web/checks`), apart from Raygun's and the websites'.
    #[cfg_attr(target_os = "android", allow(dead_code))]
    profile: PathBuf,
    /// One browser check open at a time.
    open: AtomicBool,
}

impl BrowserChecks {
    pub fn new(app: AppHandle, profile: PathBuf) -> Self {
        Self {
            app,
            profile,
            open: AtomicBool::new(false),
        }
    }

    /// The source behind a failure's text whose browser check can be passed by hand; `None`
    /// while the setting is off or no source met a check.
    pub async fn target(
        &self,
        settings: &Settings,
        extensions: &ExtensionService,
        error: &str,
        source: Option<&str>,
    ) -> Option<BrowserCheckTarget> {
        allowed(settings).ok()?;
        let (_ready, host) = extensions.ready().await;
        host.browser_check(error, source)
            .map(|challenge| BrowserCheckTarget {
                url: challenge.url().to_owned(),
                extension_id: challenge.extension_id,
                source: challenge.source,
            })
    }

    /// Opens the request `extension_id`'s source met a check on and waits until the browser
    /// closes. When the check was passed, the extension takes the browser's cookies and
    /// User-Agent and the failed call is asked again; the result says how it went.
    pub async fn open(
        &self,
        settings: &Settings,
        extensions: &ExtensionService,
        extension_id: &str,
    ) -> Result<BrowserCheckResult, String> {
        allowed(settings)?;
        let challenge = {
            let (_ready, host) = extensions.ready().await;
            host.browser_check_of(extension_id)
        }
        .ok_or("This source has no browser check to pass right now")?;
        let request = challenge
            .request
            .window()
            .ok_or("This check's address doesn't open in a browser")?;
        if self.open.swap(true, Ordering::AcqRel) {
            return Err("A browser check is open already".to_owned());
        }
        let browsed = self.browse(&request).await;
        self.open.store(false, Ordering::Release);
        let browsed = browsed?;
        let (_ready, host) = extensions.ready().await;
        let result = host
            .finish_browser_check(
                &challenge,
                &browsed.window,
                &browsed.cookies,
                browsed.user_agent.as_deref(),
            )
            .await?;
        tracing::info!(
            extension = %extension_id,
            outcome = ?result.outcome,
            cookies = result.cookies,
            "browser check opened by hand"
        );
        Ok(result)
    }

    #[cfg(target_os = "android")]
    async fn browse(&self, request: &WindowRequest) -> Result<Browsed, String> {
        use tauri::Manager;
        let app = self.app.clone();
        let request = serde_json::to_value(request).map_err(|error| error.to_string())?;
        let answer = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .browser_check(&request)
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(read_answer(&answer))
    }

    /// A window of its own that opens the request with the source's User-Agent: exactly on
    /// Windows (method, headers and body, see `exact`), as a visit of the address elsewhere. It
    /// closes when the person closes it, or once a page that isn't a check follows one. Cookies
    /// are read off the main thread (WebView2 deadlocks otherwise).
    #[cfg(not(target_os = "android"))]
    async fn browse(&self, request: &WindowRequest) -> Result<Browsed, String> {
        use raygun_ext_host::browser_check::Seen;
        use tauri::webview::PageLoadEvent;
        use tauri::{Url, WebviewUrl, WebviewWindowBuilder, WindowEvent};

        let url = Url::parse(&request.url).map_err(|error| error.to_string())?;
        let host = url.host_str().unwrap_or("the site").to_owned();
        let (watch, finished) = Watch::new(&host);
        // Windows sends the request itself once the window is up.
        let start = if cfg!(windows) {
            Url::parse("about:blank").map_err(|error| error.to_string())?
        } else {
            url.clone()
        };
        let on_load = watch.clone();
        let on_title = watch.clone();
        let window = WebviewWindowBuilder::new(
            &self.app,
            format!("browser-check-{}", host.replace(['.', ':'], "-")),
            WebviewUrl::External(start),
        )
        .title(watch.title(false))
        .inner_size(1000.0, 760.0)
        .data_directory(self.profile.clone())
        .user_agent(&request.user_agent)
        .on_page_load(move |window, payload| {
            let url = payload.url().to_string();
            let seen = match payload.event() {
                PageLoadEvent::Started => Seen::Start { url },
                PageLoadEvent::Finished => Seen::Finish { url },
            };
            on_load.see(&window, seen);
        })
        .on_document_title_changed(move |window, title| {
            on_title.see(&window, Seen::Title { title })
        })
        .build()
        .map_err(|error| error.to_string())?;
        #[cfg(windows)]
        exact::send(&window, request, watch.clone());
        let on_close = watch.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                // Kept open until its cookies are read.
                api.prevent_close();
                on_close.finish();
            }
            WindowEvent::Destroyed => on_close.finish(),
            _ => {}
        });
        let _ = finished.await;
        let reader = window.clone();
        let site = url.clone();
        let cookies =
            tauri::async_runtime::spawn_blocking(move || reader.cookies_for_url(site)).await;
        let _ = window.destroy();
        let cookies = cookies
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        Ok(Browsed {
            cookies: cookies
                .into_iter()
                .map(|cookie| BrowserCookie {
                    name: cookie.name().to_owned(),
                    value: cookie.value().to_owned(),
                    domain: cookie.domain().map(str::to_owned),
                    path: cookie.path().map(str::to_owned),
                    expires: cookie
                        .expires_datetime()
                        .map(|expires| expires.unix_timestamp()),
                    secure: cookie.secure().unwrap_or(false),
                    http_only: cookie.http_only().unwrap_or(false),
                })
                .collect(),
            // The one the window was opened with.
            user_agent: Some(request.user_agent.clone()),
            window: watch
                .window
                .lock()
                .map(|window| window.clone())
                .unwrap_or_default(),
        })
    }
}

/// The Android browser's answer (`BrowserCheckActivity`): `{cookies, userAgent, seen}`, the
/// cookies as a `Cookie` header's pairs and `seen` as [`Seen`] objects.
///
/// [`Seen`]: raygun_ext_host::browser_check::Seen
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn read_answer(answer: &serde_json::Value) -> Browsed {
    use raygun_ext_host::browser_check::{Seen, parse_cookie_header};

    let mut window = CheckWindow::default();
    for seen in answer["seen"].as_array().into_iter().flatten() {
        match serde_json::from_value::<Seen>(seen.clone()) {
            Ok(seen) => window.see(seen),
            Err(error) => tracing::debug!(%error, "a check window report Raygun can't read"),
        }
    }
    Browsed {
        cookies: parse_cookie_header(answer["cookies"].as_str().unwrap_or_default()),
        user_agent: answer["userAgent"]
            .as_str()
            .map(str::trim)
            .filter(|agent| !agent.is_empty())
            .map(str::to_owned),
        window,
    }
}

/// A desktop check window's state: what it saw, and the end of the wait for it.
#[cfg(not(target_os = "android"))]
struct Watch {
    window: std::sync::Mutex<CheckWindow>,
    done: std::sync::Mutex<Option<tokio::sync::oneshot::Sender<()>>>,
    host: String,
    /// The window's title says the site asked for no check.
    no_check: AtomicBool,
}

#[cfg(not(target_os = "android"))]
impl Watch {
    fn new(host: &str) -> (std::sync::Arc<Self>, tokio::sync::oneshot::Receiver<()>) {
        let (done, finished) = tokio::sync::oneshot::channel();
        let watch = Self {
            window: std::sync::Mutex::new(CheckWindow::default()),
            done: std::sync::Mutex::new(Some(done)),
            host: host.to_owned(),
            no_check: AtomicBool::new(false),
        };
        (std::sync::Arc::new(watch), finished)
    }

    fn title(&self, no_check: bool) -> String {
        if no_check {
            format!("{} didn't ask for a check — close this window", self.host)
        } else {
            format!("{} — pass the check, then close this window", self.host)
        }
    }

    /// Ends the wait for the window.
    fn finish(&self) {
        if let Some(done) = self.done.lock().ok().and_then(|mut done| done.take()) {
            let _ = done.send(());
        }
    }

    /// Notes what the window saw.
    fn saw(&self, seen: raygun_ext_host::browser_check::Seen) -> Option<WindowOutcome> {
        let mut window = self.window.lock().ok()?;
        window.see(seen);
        Some(window.outcome())
    }

    /// Notes what the window saw and answers it: the title says when the site asked for no
    /// check, and a passed check closes the window a moment later (once it's still passed: a
    /// check may come back).
    fn see(
        self: &std::sync::Arc<Self>,
        window: &tauri::WebviewWindow,
        seen: raygun_ext_host::browser_check::Seen,
    ) {
        let Some(outcome) = self.saw(seen) else {
            return;
        };
        let no_check = outcome == WindowOutcome::NoCheck;
        if self.no_check.swap(no_check, Ordering::AcqRel) != no_check {
            let _ = window.set_title(&self.title(no_check));
        }
        if outcome == WindowOutcome::Passed {
            let watch = self.clone();
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(std::time::Duration::from_millis(800)).await;
                let passed = watch
                    .window
                    .lock()
                    .is_ok_and(|window| window.outcome() == WindowOutcome::Passed);
                if passed {
                    watch.finish();
                }
            });
        }
    }
}

/// Windows: the window sends the checked request itself, as the source did (its method, headers
/// and body: WebView2's `NavigateWithWebResourceRequest`), and hears which answers say they're a
/// check in any language (`cf-mitigated: challenge`, `WebResourceResponseReceived`).
#[cfg(windows)]
mod exact {
    use std::sync::Arc;

    use raygun_ext_host::browser_check::{Seen, WindowRequest};
    use tauri::WebviewWindow;
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        ICoreWebView2, ICoreWebView2_2, ICoreWebView2Controller, ICoreWebView2Environment2,
    };
    use windows::core::{HSTRING, Interface, PWSTR};

    use super::Watch;

    pub(super) fn send(window: &WebviewWindow, request: &WindowRequest, watch: Arc<Watch>) {
        let sent = request.clone();
        let started = window.with_webview(move |platform| {
            // SAFETY: WebView2 calls are made on the thread `with_webview` runs on (the UI
            // thread), with the controller it hands over.
            if let Err(error) = unsafe { navigate(&platform.controller(), &sent, watch) } {
                tracing::warn!(%error, "the check window didn't open the request");
            }
        });
        if let Err(error) = started {
            tracing::warn!(%error, "the check window visits the address instead");
            if let Ok(url) = tauri::Url::parse(&request.url) {
                let _ = window.navigate(url);
            }
        }
    }

    unsafe fn navigate(
        controller: &ICoreWebView2Controller,
        request: &WindowRequest,
        watch: Arc<Watch>,
    ) -> windows::core::Result<()> {
        unsafe {
            let webview = controller.CoreWebView2()?;
            if let Err(error) = hear_checks(&webview, watch) {
                tracing::warn!(%error, "the check window can't read its answers' headers");
            }
            if let Err(error) = send_request(&webview, request) {
                tracing::warn!(%error, "the check window visits the address instead");
                webview.Navigate(&HSTRING::from(request.url.as_str()))?;
            }
        }
        Ok(())
    }

    unsafe fn hear_checks(webview: &ICoreWebView2, watch: Arc<Watch>) -> windows::core::Result<()> {
        use webview2_com::WebResourceResponseReceivedEventHandler;

        unsafe {
            let handler =
                WebResourceResponseReceivedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let response = args.Response()?;
                    let mut value = PWSTR::null();
                    // Fails for an answer without the header.
                    if response
                        .Headers()?
                        .GetHeader(&HSTRING::from("cf-mitigated"), &mut value)
                        .is_err()
                    {
                        return Ok(());
                    }
                    if !webview2_com::take_pwstr(value).eq_ignore_ascii_case("challenge") {
                        return Ok(());
                    }
                    let mut uri = PWSTR::null();
                    args.Request()?.Uri(&mut uri)?;
                    let mut status = 0i32;
                    response.StatusCode(&mut status)?;
                    watch.saw(Seen::Answer {
                        url: webview2_com::take_pwstr(uri),
                        status: u16::try_from(status).unwrap_or_default(),
                        mitigated: true,
                    });
                    Ok(())
                }));
            let mut token = 0i64;
            webview
                .cast::<ICoreWebView2_2>()?
                .add_WebResourceResponseReceived(&handler, &mut token)?;
        }
        Ok(())
    }

    unsafe fn send_request(
        webview: &ICoreWebView2,
        request: &WindowRequest,
    ) -> windows::core::Result<()> {
        use windows::Win32::UI::Shell::SHCreateMemStream;

        unsafe {
            let webview = webview.cast::<ICoreWebView2_2>()?;
            let headers: String = request
                .headers
                .iter()
                .map(|(name, value)| format!("{name}: {value}\r\n"))
                .collect();
            let body = request
                .body
                .as_deref()
                .and_then(|body| SHCreateMemStream(Some(body.as_bytes())));
            let sent = webview
                .Environment()?
                .cast::<ICoreWebView2Environment2>()?
                .CreateWebResourceRequest(
                    &HSTRING::from(request.url.as_str()),
                    &HSTRING::from(request.method.as_str()),
                    body.as_ref(),
                    &HSTRING::from(headers),
                )?;
            webview.NavigateWithWebResourceRequest(&sent)
        }
    }
}

#[cfg(test)]
mod tests {
    use raygun_ext_host::browser_check::WindowOutcome;

    use super::*;

    #[test]
    fn browser_checks_stay_off_until_turned_on() {
        let mut settings = Settings::default();
        assert!(!settings.manual_browser_checks);
        assert!(allowed(&settings).is_err());
        settings.manual_browser_checks = true;
        assert!(allowed(&settings).is_ok());
    }

    #[test]
    fn the_android_browser_s_answer_is_read() {
        let page = "https://novels.example/search?q=sword";
        let answer = serde_json::json!({
            "cookies": "cf_clearance=passed; theme=dark",
            "userAgent": " Mozilla/5.0 (Linux; Android 17) ",
            "seen": [
                { "kind": "answer", "url": page, "status": 403, "mitigated": true },
                { "kind": "start", "url": page },
                { "kind": "title", "title": "Un instant…" },
                { "kind": "finish", "url": page },
                { "kind": "somethingNew" },
                { "kind": "start", "url": page },
                { "kind": "title", "title": "Sword" },
                { "kind": "finish", "url": page },
            ],
        });
        let browsed = read_answer(&answer);
        assert_eq!(browsed.cookies.len(), 2);
        assert_eq!(browsed.cookies[0].name, "cf_clearance");
        assert_eq!(
            browsed.user_agent.as_deref(),
            Some("Mozilla/5.0 (Linux; Android 17)")
        );
        assert_eq!(browsed.window.outcome(), WindowOutcome::Passed);

        // Closed at once: nothing seen, nothing passed.
        let closed = read_answer(&serde_json::json!({ "cookies": "", "userAgent": "" }));
        assert!(closed.cookies.is_empty());
        assert_eq!(closed.user_agent, None);
        assert_eq!(closed.window.outcome(), WindowOutcome::NotLoaded);
    }
}
