//! The platform's hidden web views for converted sources (`raygun_ext_host::webview`, Keiyoushi's
//! `runWebView`): on Android a WebView in the `raygun-audio` plugin's `:views` process
//! (`HiddenViews.kt`, `HiddenViewService.kt`); on Windows hidden windows of their own (WebView2,
//! one profile per extension under `web/views`, no capabilities). Other desktops have none:
//! sources that need one say so.

use std::path::PathBuf;

use tauri::AppHandle;

/// Installs the platform's views as the extension host's (`profile`: the desktop views' data).
pub fn install(app: &AppHandle, profile: PathBuf) {
    #[cfg(target_os = "android")]
    {
        drop(profile);
        android::install(app);
    }
    #[cfg(windows)]
    desktop::install(app, profile);
    #[cfg(not(any(windows, target_os = "android")))]
    let _ = (app, profile);
}

#[cfg(target_os = "android")]
mod android {
    use std::sync::Arc;

    use raygun_ext_host::webview::{self, Command, ViewHost};
    use tauri::{AppHandle, Manager};

    struct AndroidViews(AppHandle);

    impl ViewHost for AndroidViews {
        fn send(&self, command: &Command) -> Result<(), String> {
            let command = serde_json::to_value(command).map_err(|error| error.to_string())?;
            self.0
                .try_state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .ok_or("Raygun's player isn't ready")?
                .web_view(&command)
        }
    }

    pub fn install(app: &AppHandle) {
        tauri_plugin_raygun_audio::set_web_view_handler(|json| webview::deliver(&json));
        webview::set_host(Arc::new(AndroidViews(app.clone())));
    }
}

/// Windows: each view is a hidden window with its extension's own WebView2 profile and no
/// capabilities. Page events come from Tauri (loads, titles) and WebView2 (failed navigations,
/// checks' answers, a renderer's end); bridges and start scripts are initialization scripts that
/// check their origin; messages reach Raygun as objects marked with the view's token
/// (`WebMessageReceived`, which wry's own text messages never are); requests are answered through
/// `WebResourceRequested`, deferred on the UI thread until the source answers.
#[cfg(windows)]
mod desktop {
    use std::cell::RefCell;
    use std::collections::{BTreeMap, HashMap};
    use std::path::PathBuf;
    use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
    use std::sync::{Arc, Mutex, PoisonError};
    use std::time::Duration;

    use raygun_ext_host::permissions::{HostPattern, NetPolicy};
    use raygun_ext_host::webview::{
        self, Answer, Bridge, Command, Event, OpenView, ViewCookies, ViewHost,
    };
    use tauri::webview::PageLoadEvent;
    use tauri::{AppHandle, Url, WebviewUrl, WebviewWindow, WebviewWindowBuilder};
    use webview2_com::Microsoft::Web::WebView2::Win32::{
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
        COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT, COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE,
        ICoreWebView2, ICoreWebView2_2, ICoreWebView2Controller, ICoreWebView2Deferral,
        ICoreWebView2Environment, ICoreWebView2Environment2,
        ICoreWebView2WebResourceRequestedEventArgs,
    };
    use windows::core::{BOOL, HSTRING, Interface, PWSTR};

    /// What a view's handlers share.
    struct State {
        id: String,
        label: String,
        policy: NetPolicy,
        intercept: bool,
        block_images: bool,
        javascript: bool,
        /// Marks the page's bridge messages as this view's.
        token: String,
        bridges: Vec<Bridge>,
        pages: Mutex<Vec<String>>,
        /// A page given as text (`loadData`): its address answers with it, once.
        data: Mutex<Option<(String, String, String)>>,
        next_request: AtomicU64,
        closed: AtomicBool,
    }

    impl State {
        fn may_load(&self, url: &str) -> bool {
            let Ok(parsed) = Url::parse(url) else {
                return false;
            };
            match parsed.scheme() {
                "data" | "blob" | "about" => return true,
                "http" | "https" => {}
                _ => return false,
            }
            !webview::is_challenge(url) && self.policy.allows(&parsed)
        }

        /// The page given as text for `url`, when that's its address.
        fn take_data(&self, url: &str) -> Option<(String, String)> {
            let mut data = self.data.lock().unwrap_or_else(PoisonError::into_inner);
            let same = data
                .as_ref()
                .is_some_and(|(address, _, _)| same_address(address, url));
            if !same {
                return None;
            }
            data.take().map(|(_, text, mime)| (text, mime))
        }
    }

    fn same_address(left: &str, right: &str) -> bool {
        let normal =
            |text: &str| Url::parse(text).map_or_else(|_| text.to_owned(), |url| url.to_string());
        normal(left) == normal(right)
    }

    /// The views open now, by id.
    struct DesktopViews(Arc<Inner>);

    struct Inner {
        app: AppHandle,
        profile: PathBuf,
        views: Mutex<HashMap<String, (Arc<State>, WebviewWindow)>>,
    }

    thread_local! {
        /// Requests waiting for the source's answer, on the UI thread: by view and number.
        static DEFERRED: RefCell<HashMap<(String, u64), Deferred>> = RefCell::default();
    }

    struct Deferred {
        args: ICoreWebView2WebResourceRequestedEventArgs,
        deferral: ICoreWebView2Deferral,
        environment: ICoreWebView2Environment,
    }

    pub fn install(app: &AppHandle, profile: PathBuf) {
        webview::set_host(Arc::new(DesktopViews(Arc::new(Inner {
            app: app.clone(),
            profile,
            views: Mutex::new(HashMap::new()),
        }))));
    }

    impl ViewHost for DesktopViews {
        fn send(&self, command: &Command) -> Result<(), String> {
            self.0.send(command)
        }
    }

    impl Inner {
        fn send(self: &Arc<Self>, command: &Command) -> Result<(), String> {
            match command {
                Command::Open(spec) => self.open(spec),
                Command::Load { id, url, headers } => {
                    let url = url.clone();
                    let headers = headers.clone();
                    self.with_view(id, move |webview, _state| {
                        // SAFETY: on the UI thread, with the view's own WebView2.
                        unsafe { navigate(&webview, &url, &headers) }
                    })
                }
                Command::LoadData {
                    id,
                    base_url,
                    data,
                    mime,
                } => {
                    let state = self.state(id)?;
                    *state.data.lock().unwrap_or_else(PoisonError::into_inner) =
                        Some((base_url.clone(), data.clone(), mime.clone()));
                    let url = base_url.clone();
                    self.with_view(id, move |webview, _state| {
                        // SAFETY: as above.
                        unsafe { navigate(&webview, &url, &BTreeMap::new()) }
                    })
                }
                Command::Evaluate { id, call, script } => {
                    let (done, call, script) = (id.clone(), *call, script.clone());
                    let sent = self.with_view(id, move |webview, _state| {
                        let handler = webview2_com::ExecuteScriptCompletedHandler::create(
                            Box::new(move |result, json| {
                                webview::deliver_event(
                                    &done,
                                    Event::Evaluated {
                                        call,
                                        result: result.ok().map(|()| json),
                                    },
                                );
                                Ok(())
                            }),
                        );
                        // SAFETY: on the UI thread.
                        unsafe { webview.ExecuteScript(&HSTRING::from(script.as_str()), &handler) }
                    });
                    if sent.is_err() {
                        webview::deliver_event(id, Event::Evaluated { call, result: None });
                    }
                    Ok(())
                }
                Command::Answer {
                    id,
                    request,
                    response,
                } => {
                    let (key, response) = ((id.clone(), *request), response.clone());
                    self.app
                        .run_on_main_thread(move || complete(&key, response))
                        .map_err(|error| error.to_string())
                }
                Command::Reply { .. } => Ok(()),
                Command::Stop { id } => self.with_view(id, |webview, _state| {
                    // SAFETY: on the UI thread.
                    unsafe { webview.Stop() }
                }),
                Command::Close { id } => {
                    self.close(id);
                    Ok(())
                }
            }
        }

        fn state(&self, id: &str) -> Result<Arc<State>, String> {
            self.views
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(id)
                .map(|(state, _)| state.clone())
                .ok_or_else(|| "the web view was closed".to_owned())
        }

        /// Runs `work` with the view's WebView2 on the UI thread.
        fn with_view(
            &self,
            id: &str,
            work: impl FnOnce(ICoreWebView2, Arc<State>) -> windows::core::Result<()> + Send + 'static,
        ) -> Result<(), String> {
            let (state, window) = self
                .views
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .get(id)
                .cloned()
                .ok_or("the web view was closed")?;
            window
                .with_webview(move |platform| {
                    // SAFETY: the controller `with_webview` hands over, on its thread.
                    let webview = unsafe { platform.controller().CoreWebView2() };
                    if let Err(error) = webview.and_then(|webview| work(webview, state)) {
                        tracing::debug!(%error, "a web view command failed");
                    }
                })
                .map_err(|error| error.to_string())
        }

        fn open(self: &Arc<Self>, spec: &OpenView) -> Result<(), String> {
            let state = Arc::new(State {
                id: spec.id.clone(),
                label: format!(
                    "view-{}",
                    spec.id.replace(|c: char| !c.is_ascii_alphanumeric(), "-")
                ),
                policy: NetPolicy::new(
                    spec.hosts
                        .iter()
                        .filter_map(|pattern| HostPattern::parse(pattern))
                        .collect(),
                ),
                intercept: spec.intercept,
                block_images: spec.block_images,
                javascript: spec.javascript,
                token: uuid::Uuid::new_v4().simple().to_string(),
                bridges: spec.bridges.clone(),
                pages: Mutex::new(Vec::new()),
                data: Mutex::new(None),
                next_request: AtomicU64::new(0),
                closed: AtomicBool::new(false),
            });
            let folder: String = spec
                .extension
                .chars()
                .map(|c| {
                    if c.is_ascii_alphanumeric() || c == '.' {
                        c
                    } else {
                        '_'
                    }
                })
                .take(80)
                .collect();
            let blank = Url::parse("about:blank").map_err(|error| error.to_string())?;
            let on_load = state.clone();
            let on_title = state.clone();
            let mut builder =
                WebviewWindowBuilder::new(&self.app, &state.label, WebviewUrl::External(blank))
                    .title("Raygun")
                    .visible(false)
                    .focused(false)
                    .skip_taskbar(true)
                    .inner_size(412.0, 915.0)
                    .data_directory(self.profile.join(folder))
                    .user_agent(&spec.user_agent)
                    .on_navigation({
                        let state = state.clone();
                        move |url| state.may_load(url.as_str())
                    })
                    .on_page_load(move |_window, payload| {
                        let url = payload.url().to_string();
                        // The window's own blank start isn't one of the run's pages.
                        if url.starts_with("about:") {
                            return;
                        }
                        let event = match payload.event() {
                            PageLoadEvent::Started => {
                                let mut pages =
                                    on_load.pages.lock().unwrap_or_else(PoisonError::into_inner);
                                if url.starts_with("http")
                                    && !pages.contains(&url)
                                    && pages.len() < 32
                                {
                                    pages.push(url.clone());
                                }
                                Event::Started { url }
                            }
                            PageLoadEvent::Finished => Event::Finished { url },
                        };
                        webview::deliver_event(&on_load.id, event);
                    })
                    .on_document_title_changed(move |_window, title| {
                        webview::deliver_event(&on_title.id, Event::Title { title });
                    });
            for bridge in &spec.bridges {
                builder = builder.initialization_script(bridge_script(bridge, &state.token));
            }
            for script in &spec.scripts {
                builder = builder.initialization_script(guarded(&script.script, &script.origins));
            }
            let window = builder.build().map_err(|error| error.to_string())?;
            for cookies in &spec.cookies {
                set_cookies(&window, cookies);
            }
            self.views
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .insert(spec.id.clone(), (state.clone(), window.clone()));
            let started = window.with_webview(move |platform| {
                // SAFETY: the controller `with_webview` hands over, on its thread.
                if let Err(error) = unsafe { prepare(&platform.controller(), state) } {
                    tracing::warn!(%error, "a web view's handlers didn't start");
                }
            });
            if let Err(error) = started {
                self.close(&spec.id);
                return Err(error.to_string());
            }
            // Closed by its time whatever happens.
            let id = spec.id.clone();
            let host = Arc::downgrade(self);
            let timeout = Duration::from_millis(spec.timeout_ms) + Duration::from_secs(5);
            tauri::async_runtime::spawn(async move {
                tokio::time::sleep(timeout).await;
                if let Some(host) = host.upgrade() {
                    host.close(&id);
                }
            });
            Ok(())
        }

        fn close(&self, id: &str) {
            let Some((state, window)) = self
                .views
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .remove(id)
            else {
                return;
            };
            state.closed.store(true, Ordering::Release);
            let release = state.id.clone();
            let _ = self.app.run_on_main_thread(move || release_all(&release));
            let pages = state
                .pages
                .lock()
                .map(|pages| pages.clone())
                .unwrap_or_default();
            tauri::async_runtime::spawn(async move {
                let reader = window.clone();
                let cookies = tauri::async_runtime::spawn_blocking(move || {
                    pages
                        .iter()
                        .take(16)
                        .filter_map(|page| {
                            let url = Url::parse(page).ok()?;
                            let found = reader.cookies_for_url(url).ok()?;
                            let pairs: Vec<String> = found
                                .iter()
                                .map(|cookie| format!("{}={}", cookie.name(), cookie.value()))
                                .collect();
                            (!pairs.is_empty()).then(|| ViewCookies {
                                url: page.clone(),
                                cookies: pairs.join("; "),
                            })
                        })
                        .collect::<Vec<_>>()
                })
                .await
                .unwrap_or_default();
                let _ = window.destroy();
                webview::deliver_event(&state.id, Event::Closed { cookies });
            });
        }
    }

    /// The extension's cookies for a page, put in the view before it loads.
    fn set_cookies(window: &WebviewWindow, cookies: &ViewCookies) {
        let Ok(url) = Url::parse(&cookies.url) else {
            return;
        };
        let Some(host) = url.host_str().map(str::to_owned) else {
            return;
        };
        for pair in cookies.cookies.split("; ") {
            let Some((name, value)) = pair.split_once('=') else {
                continue;
            };
            let cookie = tauri::webview::cookie::Cookie::build((name.to_owned(), value.to_owned()))
                .domain(host.clone())
                .path("/")
                .build();
            if let Err(error) = window.set_cookie(cookie) {
                tracing::debug!(%error, "a cookie didn't reach a web view");
            }
        }
    }

    /// A bridge (`window.<name>.postMessage(text)`) for the origins the source named: its messages
    /// reach Raygun as objects marked with the view's token.
    fn bridge_script(bridge: &Bridge, token: &str) -> String {
        let name = serde_json::to_string(&bridge.name).unwrap_or_default();
        let token = serde_json::to_string(token).unwrap_or_default();
        guarded(
            &format!(
                "(() => {{ const post = (message) => {{ if (window.chrome && window.chrome.webview) window.chrome.webview.postMessage({{ raygunView: {token}, bridge: {name}, message: String(message) }}); }}; try {{ Object.defineProperty(window, {name}, {{ value: Object.freeze({{ postMessage: post }}), configurable: false }}); }} catch (e) {{}} }})();"
            ),
            &bridge.origins,
        )
    }

    /// `script`, run only in the origins given (`*` every one).
    fn guarded(script: &str, origins: &[String]) -> String {
        let origins = serde_json::to_string(origins).unwrap_or_else(|_| "[]".to_owned());
        format!(
            "(() => {{ const allowed = {origins}; if (!allowed.includes('*') && !allowed.some((rule) => rule.replace(/\\/$/, '') === location.origin)) return;\n{script}\n}})();"
        )
    }

    /// Opens `url`, with `headers` when the source gave some.
    unsafe fn navigate(
        webview: &ICoreWebView2,
        url: &str,
        headers: &BTreeMap<String, String>,
    ) -> windows::core::Result<()> {
        unsafe {
            if headers.is_empty() {
                return webview.Navigate(&HSTRING::from(url));
            }
            let lines: String = headers
                .iter()
                .filter(|(name, value)| {
                    !name.contains(['\r', '\n']) && !value.contains(['\r', '\n'])
                })
                .map(|(name, value)| format!("{name}: {value}\r\n"))
                .collect();
            let webview2 = webview.cast::<ICoreWebView2_2>()?;
            let request = webview2
                .Environment()?
                .cast::<ICoreWebView2Environment2>()?
                .CreateWebResourceRequest(
                    &HSTRING::from(url),
                    &HSTRING::from("GET"),
                    None::<&windows::Win32::System::Com::IStream>,
                    &HSTRING::from(lines),
                )?;
            webview2.NavigateWithWebResourceRequest(&request)
        }
    }

    /// The view's settings and handlers, before its first load.
    unsafe fn prepare(
        controller: &ICoreWebView2Controller,
        state: Arc<State>,
    ) -> windows::core::Result<()> {
        use webview2_com::{
            NavigationCompletedEventHandler, NewWindowRequestedEventHandler,
            ProcessFailedEventHandler, WebMessageReceivedEventHandler,
            WebResourceRequestedEventHandler, WebResourceResponseReceivedEventHandler,
        };

        unsafe {
            // Laid out and "visible" to its pages, in a window that never shows.
            controller.SetIsVisible(true)?;
            let webview = controller.CoreWebView2()?;
            webview.Settings()?.SetIsScriptEnabled(state.javascript)?;
            let environment = webview.cast::<ICoreWebView2_2>()?.Environment()?;
            let mut token = 0i64;

            let failed = state.clone();
            webview.add_NavigationCompleted(
                &NavigationCompletedEventHandler::create(Box::new(move |sender, args| {
                    let (Some(sender), Some(args)) = (sender, args) else {
                        return Ok(());
                    };
                    let mut success = BOOL::default();
                    args.IsSuccess(&mut success)?;
                    if !success.as_bool() {
                        let mut status = Default::default();
                        args.WebErrorStatus(&mut status)?;
                        let mut source = PWSTR::null();
                        sender.Source(&mut source)?;
                        webview::deliver_event(
                            &failed.id,
                            Event::Failed {
                                url: webview2_com::take_pwstr(source),
                                code: status.0,
                                description: format!("WebView2 error {}", status.0),
                                main_frame: true,
                            },
                        );
                    }
                    Ok(())
                })),
                &mut token,
            )?;

            let crashed = state.clone();
            webview.add_ProcessFailed(
                &ProcessFailedEventHandler::create(Box::new(move |_sender, _args| {
                    webview::deliver_event(&crashed.id, Event::Gone { crashed: true });
                    Ok(())
                })),
                &mut token,
            )?;

            webview.add_NewWindowRequested(
                &NewWindowRequestedEventHandler::create(Box::new(move |_sender, args| {
                    if let Some(args) = args {
                        args.SetHandled(true)?;
                    }
                    Ok(())
                })),
                &mut token,
            )?;

            let messages = state.clone();
            webview.add_WebMessageReceived(
                &WebMessageReceivedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    let mut json = PWSTR::null();
                    args.WebMessageAsJson(&mut json)?;
                    let json = webview2_com::take_pwstr(json);
                    let mut source = PWSTR::null();
                    args.Source(&mut source)?;
                    let source = webview2_com::take_pwstr(source);
                    take_message(&messages, &json, &source);
                    Ok(())
                })),
                &mut token,
            )?;

            let checks = state.clone();
            webview
                .cast::<ICoreWebView2_2>()?
                .add_WebResourceResponseReceived(
                    &WebResourceResponseReceivedEventHandler::create(Box::new(
                        move |_sender, args| {
                            let Some(args) = args else {
                                return Ok(());
                            };
                            let response = args.Response()?;
                            let mut value = PWSTR::null();
                            if response
                                .Headers()?
                                .GetHeader(&HSTRING::from("cf-mitigated"), &mut value)
                                .is_err()
                                || !webview2_com::take_pwstr(value)
                                    .eq_ignore_ascii_case("challenge")
                            {
                                return Ok(());
                            }
                            let mut uri = PWSTR::null();
                            args.Request()?.Uri(&mut uri)?;
                            let mut status = 0i32;
                            response.StatusCode(&mut status)?;
                            webview::deliver_event(
                                &checks.id,
                                Event::Answer {
                                    url: webview2_com::take_pwstr(uri),
                                    status: u16::try_from(status).unwrap_or_default(),
                                    mitigated: true,
                                },
                            );
                            Ok(())
                        },
                    )),
                    &mut token,
                )?;

            webview.AddWebResourceRequestedFilter(
                &HSTRING::from("*"),
                COREWEBVIEW2_WEB_RESOURCE_CONTEXT_ALL,
            )?;
            let requests = state;
            webview.add_WebResourceRequested(
                &WebResourceRequestedEventHandler::create(Box::new(move |_sender, args| {
                    let Some(args) = args else {
                        return Ok(());
                    };
                    requested(&requests, &args, &environment)
                })),
                &mut token,
            )?;
        }
        Ok(())
    }

    /// A message the page posted to a bridge: one of this view's, from an origin it allows.
    fn take_message(state: &State, json: &str, source: &str) {
        let Ok(message) = serde_json::from_str::<serde_json::Value>(json) else {
            return;
        };
        if message["raygunView"].as_str() != Some(state.token.as_str()) {
            return;
        }
        let Some(name) = message["bridge"].as_str() else {
            return;
        };
        let origin = Url::parse(source)
            .map(|url| url.origin().ascii_serialization())
            .unwrap_or_default();
        let allowed = state.bridges.iter().any(|bridge| {
            bridge.name == name
                && bridge
                    .origins
                    .iter()
                    .any(|rule| rule == "*" || rule.trim_end_matches('/') == origin)
        });
        if !allowed {
            return;
        }
        webview::deliver_event(
            &state.id,
            Event::Message {
                bridge: name.to_owned(),
                message: message["message"].as_str().unwrap_or_default().to_owned(),
                origin: Some(origin),
                main_frame: Some(true),
                reply: None,
            },
        );
    }

    /// A request the page makes: a page given as text answers its own address; what the source
    /// may not reach (or a check's provider) is refused; the rest waits for the source when it
    /// answers requests.
    unsafe fn requested(
        state: &Arc<State>,
        args: &ICoreWebView2WebResourceRequestedEventArgs,
        environment: &ICoreWebView2Environment,
    ) -> windows::core::Result<()> {
        unsafe {
            let request = args.Request()?;
            let mut uri = PWSTR::null();
            request.Uri(&mut uri)?;
            let uri = webview2_com::take_pwstr(uri);
            let mut context = COREWEBVIEW2_WEB_RESOURCE_CONTEXT::default();
            args.ResourceContext(&mut context)?;
            let document = context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_DOCUMENT;
            if document && let Some((text, mime)) = state.take_data(&uri) {
                let answer = Answer {
                    status: 200,
                    reason: "OK".into(),
                    mime: Some(mime),
                    encoding: Some("utf-8".into()),
                    body: base64_of(text.as_bytes()),
                    ..Answer::default()
                };
                return respond(args, environment, &answer);
            }
            let images = state.block_images && context == COREWEBVIEW2_WEB_RESOURCE_CONTEXT_IMAGE;
            if state.closed.load(Ordering::Acquire) || images || !state.may_load(&uri) {
                return respond(args, environment, &blocked());
            }

            if !state.intercept {
                return Ok(());
            }
            let number = state.next_request.fetch_add(1, Ordering::AcqRel) + 1;
            let mut method = PWSTR::null();
            request.Method(&mut method)?;
            let method = webview2_com::take_pwstr(method);
            let headers = request_headers(&request).unwrap_or_default();
            let deferral = args.GetDeferral()?;
            DEFERRED.with(|deferred| {
                deferred.borrow_mut().insert(
                    (state.id.clone(), number),
                    Deferred {
                        args: args.clone(),
                        deferral,
                        environment: environment.clone(),
                    },
                );
            });
            webview::deliver_event(
                &state.id,
                Event::Request {
                    request: number,
                    url: uri,
                    method,
                    headers,
                    main_frame: document,
                },
            );
        }
        Ok(())
    }

    unsafe fn request_headers(
        request: &webview2_com::Microsoft::Web::WebView2::Win32::ICoreWebView2WebResourceRequest,
    ) -> windows::core::Result<BTreeMap<String, String>> {
        let mut headers = BTreeMap::new();
        unsafe {
            let iterator = request.Headers()?.GetIterator()?;
            let mut has = BOOL::default();
            iterator.HasCurrentHeader(&mut has)?;
            while has.as_bool() && headers.len() < 64 {
                let (mut name, mut value) = (PWSTR::null(), PWSTR::null());
                iterator.GetCurrentHeader(&mut name, &mut value)?;
                headers.insert(
                    webview2_com::take_pwstr(name),
                    webview2_com::take_pwstr(value),
                );
                iterator.MoveNext(&mut has)?;
            }
        }
        Ok(headers)
    }

    fn blocked() -> Answer {
        Answer {
            status: 403,
            reason: "Blocked".into(),
            mime: Some("text/plain".into()),
            ..Answer::default()
        }
    }

    fn base64_of(bytes: &[u8]) -> String {
        use base64::Engine as _;
        base64::engine::general_purpose::STANDARD.encode(bytes)
    }

    /// Answers a request with what the source gave.
    unsafe fn respond(
        args: &ICoreWebView2WebResourceRequestedEventArgs,
        environment: &ICoreWebView2Environment,
        answer: &Answer,
    ) -> windows::core::Result<()> {
        use base64::Engine as _;
        use windows::Win32::UI::Shell::SHCreateMemStream;

        let (status, reason) = match answer.status {
            _ if answer.no_body => (403, "Blocked".to_owned()),
            0 => (200, "OK".to_owned()),
            status => (
                i32::from(status),
                if answer.reason.trim().is_empty() {
                    "OK".to_owned()
                } else {
                    answer.reason.clone()
                },
            ),
        };
        let mut lines: String = answer
            .headers
            .iter()
            .filter(|(name, value)| !name.contains(['\r', '\n']) && !value.contains(['\r', '\n']))
            .map(|(name, value)| format!("{name}: {value}\r\n"))
            .collect();
        let typed = answer
            .headers
            .keys()
            .any(|name| name.eq_ignore_ascii_case("content-type"));
        if !typed && let Some(mime) = &answer.mime {
            let charset = answer
                .encoding
                .as_deref()
                .map(|encoding| format!("; charset={encoding}"))
                .unwrap_or_default();
            lines.push_str(&format!("Content-Type: {mime}{charset}\r\n"));
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(&answer.body)
            .unwrap_or_default();
        unsafe {
            let body = SHCreateMemStream(Some(&bytes));
            let response = environment.CreateWebResourceResponse(
                body.as_ref(),
                status,
                &HSTRING::from(reason),
                &HSTRING::from(lines),
            )?;
            args.SetResponse(&response)
        }
    }

    /// The source's answer to a deferred request, on the UI thread: `None` lets it load.
    fn complete(key: &(String, u64), answer: Option<Answer>) {
        let Some(deferred) = DEFERRED.with(|deferred| deferred.borrow_mut().remove(key)) else {
            return;
        };
        // SAFETY: on the UI thread, which deferred it.
        unsafe {
            if let Some(answer) = answer
                && let Err(error) = respond(&deferred.args, &deferred.environment, &answer)
            {
                tracing::debug!(%error, "a web view's request wasn't answered");
            }
            let _ = deferred.deferral.Complete();
        }
    }

    /// A closing view's requests, let go as refused.
    fn release_all(id: &str) {
        let keys: Vec<(String, u64)> = DEFERRED.with(|deferred| {
            deferred
                .borrow()
                .keys()
                .filter(|(view, _)| view == id)
                .cloned()
                .collect()
        });
        for key in keys {
            complete(&key, Some(blocked()));
        }
    }
}
