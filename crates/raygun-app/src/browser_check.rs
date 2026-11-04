//! Opening a source's site so its browser check can be passed by hand (experimental). The whole
//! flow is described in `raygun_ext_host::browser_check`; this module is the app's part: the
//! setting gate (`Settings::manual_browser_checks`, off by default and turned on only after the
//! UI's warning), and the visible browser, which Raygun never drives: a Tauri window on desktop,
//! `BrowserCheckActivity` in the `raygun-audio` plugin on Android, both with the User-Agent
//! sources send and their own browser data. The site's cookies then go to the extension's jar.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};

use raygun_ext_host::browser_check::BrowserCookie;
use raygun_model::{BrowserCheckTarget, Settings};
use tauri::{AppHandle, Url};

use crate::extensions::ExtensionService;

/// Whether the person turned on solving browser checks themselves.
pub fn allowed(settings: &Settings) -> Result<(), String> {
    if settings.manual_browser_checks {
        Ok(())
    } else {
        Err("Solving browser checks yourself is off (Settings → Browse)".to_owned())
    }
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
                extension_id: challenge.extension_id,
                source: challenge.source,
                url: challenge.url,
            })
    }

    /// Opens the site `extension_id`'s sources met a check on, waits until the browser closes,
    /// and hands the site's cookies to the extension. Returns how many cookies it took.
    pub async fn open(
        &self,
        settings: &Settings,
        extensions: &ExtensionService,
        extension_id: &str,
    ) -> Result<u32, String> {
        allowed(settings)?;
        let challenge = {
            let (_ready, host) = extensions.ready().await;
            host.browser_check_of(extension_id)
        }
        .ok_or("This source has no browser check to pass right now")?;
        let url = Url::parse(&challenge.url).map_err(|error| error.to_string())?;
        if self.open.swap(true, Ordering::AcqRel) {
            return Err("A browser check is open already".to_owned());
        }
        let cookies = self.browse(&url).await;
        self.open.store(false, Ordering::Release);
        let cookies = cookies?;
        let (_ready, host) = extensions.ready().await;
        let taken = host.import_browser_cookies(extension_id, url.as_str(), &cookies)?;
        tracing::info!(extension = %extension_id, cookies = taken, "browser check passed by hand");
        Ok(u32::try_from(taken).unwrap_or(u32::MAX))
    }

    #[cfg(target_os = "android")]
    async fn browse(&self, url: &Url) -> Result<Vec<BrowserCookie>, String> {
        use tauri::Manager;
        let app = self.app.clone();
        let request = serde_json::json!({
            "url": url.as_str(),
            "userAgent": raygun_net::user_agent(),
        });
        let header = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .browser_check(&request)
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(raygun_ext_host::browser_check::parse_cookie_header(&header))
    }

    /// A window of its own that closes when the person closes it, or once a page that isn't a
    /// check follows one. Cookies are read off the main thread (WebView2 deadlocks otherwise).
    #[cfg(not(target_os = "android"))]
    async fn browse(&self, url: &Url) -> Result<Vec<BrowserCookie>, String> {
        use std::sync::{Arc, Mutex};

        use raygun_ext_host::browser_check::looks_like_challenge;
        use tauri::{WebviewUrl, WebviewWindowBuilder, WindowEvent};
        use tokio::sync::oneshot;

        let (done, finished) = oneshot::channel::<()>();
        let done = Arc::new(Mutex::new(Some(done)));
        let finish = |done: &Arc<Mutex<Option<oneshot::Sender<()>>>>| {
            if let Some(done) = done.lock().ok().and_then(|mut done| done.take()) {
                let _ = done.send(());
            }
        };
        let saw_check = Arc::new(AtomicBool::new(false));
        let on_title = done.clone();
        let host = url.host_str().unwrap_or("the site").to_owned();
        let window = WebviewWindowBuilder::new(
            &self.app,
            format!("browser-check-{}", host.replace(['.', ':'], "-")),
            WebviewUrl::External(url.clone()),
        )
        .title(format!("{host} — pass the check, then close this window"))
        .inner_size(1000.0, 760.0)
        .data_directory(self.profile.clone())
        .user_agent(&raygun_net::user_agent())
        .on_document_title_changed(move |_window, title| {
            if looks_like_challenge(&title) {
                saw_check.store(true, Ordering::Release);
            } else if saw_check.load(Ordering::Acquire) {
                finish(&on_title);
            }
        })
        .build()
        .map_err(|error| error.to_string())?;
        let on_close = done.clone();
        window.on_window_event(move |event| match event {
            WindowEvent::CloseRequested { api, .. } => {
                // Kept open until its cookies are read.
                api.prevent_close();
                finish(&on_close);
            }
            WindowEvent::Destroyed => finish(&on_close),
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
        Ok(cookies
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
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_checks_stay_off_until_turned_on() {
        let mut settings = Settings::default();
        assert!(!settings.manual_browser_checks);
        assert!(allowed(&settings).is_err());
        settings.manual_browser_checks = true;
        assert!(allowed(&settings).is_ok());
    }
}
