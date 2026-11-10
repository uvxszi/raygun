//! Tier 2 for Mihon extensions Raygun can't convert: their own code, run apart from Raygun behind
//! the same `manga.source` interface, behind the "Allow running source code" switch
//! (`Settings::run_source_code`). See `docs/extensions/formats/mihon.md`.
//!
//! - Desktop: a `java -jar mihon-host.jar` sidecar per extension ([`sidecar`]), with a pinned JRE
//!   and dex2jar fetched on first use ([`tools`]). The host loads the APK's classes (converted to
//!   a jar) and answers over stdio.
//! - Android: an isolated `:mihon` process in the `raygun-audio` plugin loads the dex directly;
//!   Rust reaches it through the plugin, not through this module.

pub mod tools;

#[cfg(not(target_os = "android"))]
mod sidecar;

use std::path::{Path, PathBuf};
use std::sync::Arc;

use serde_json::Value;

/// What a device can do for tier 2, for the source's details and Settings.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct ContainerState {
    /// This build and platform can run an extension's own code at all.
    pub supported: bool,
    /// The tools are in place (desktop); on Android this is the same as `supported`.
    pub ready: bool,
    /// A one-line reason it isn't ready, for the UI.
    pub note: String,
}

#[cfg(not(target_os = "android"))]
pub use desktop::MihonContainer;

#[cfg(not(target_os = "android"))]
mod desktop {
    use super::*;
    use std::collections::HashMap;
    use std::sync::atomic::{AtomicBool, Ordering};

    use tauri::AppHandle;
    use tauri_specta::Event as _;
    use tokio::sync::{Mutex, Notify};

    use super::sidecar::{IDLE, Load, Sidecar};
    use crate::events::MihonRuntimeEvent;

    /// Runs Mihon extensions' own code in per-extension JVM sidecars on desktop.
    pub struct MihonContainer {
        app: AppHandle,
        tools: Arc<tools::Tools>,
        data_root: PathBuf,
        user_agent: Arc<dyn Fn() -> String + Send + Sync>,
        sidecars: Mutex<HashMap<String, Arc<Sidecar>>>,
        setting_up: AtomicBool,
        cancel: Arc<Notify>,
    }

    impl MihonContainer {
        pub fn new(
            app: AppHandle,
            root: &Path,
            host_jar: PathBuf,
            client: reqwest::Client,
            user_agent: Arc<dyn Fn() -> String + Send + Sync>,
        ) -> Arc<Self> {
            let container = Arc::new(Self {
                app,
                tools: Arc::new(tools::Tools::new(root, host_jar, client)),
                data_root: root.join("mihon-run"),
                user_agent,
                sidecars: Mutex::new(HashMap::new()),
                setting_up: AtomicBool::new(false),
                cancel: Arc::new(Notify::new()),
            });
            container.clone().sweep_idle();
            container
        }

        pub fn state(&self) -> ContainerState {
            let tools = self.tools.state();
            let note = if !tools.supported {
                "Running source code needs Raygun on a supported computer".to_owned()
            } else if !tools.host {
                "This build is missing the Mihon host".to_owned()
            } else if self.setting_up.load(Ordering::SeqCst) {
                "Setting up the runtime…".to_owned()
            } else if !tools.ready() {
                "Set up the runtime to run this source's own code".to_owned()
            } else {
                String::new()
            };
            ContainerState {
                supported: tools.supported && tools.host,
                ready: tools.ready(),
                note,
            }
        }

        /// The runtime state the UI shows (`Settings`, a source's details).
        pub fn runtime_state(&self) -> raygun_model::MihonRuntimeState {
            let state = self.state();
            raygun_model::MihonRuntimeState {
                supported: state.supported,
                ready: state.ready,
                setting_up: self.setting_up.load(Ordering::SeqCst),
                note: state.note,
            }
        }

        /// Fetches the JRE and dex2jar if they're missing, announcing the runtime state as it
        /// goes. A second call while one is running does nothing; `cancel_setup` stops it.
        pub async fn setup(self: &Arc<Self>) -> Result<(), String> {
            if self.setting_up.swap(true, Ordering::SeqCst) {
                return Err("The runtime is already being set up".into());
            }
            MihonRuntimeEvent(self.runtime_state()).emit(&self.app).ok();
            let container = self.clone();
            let announce = {
                let container = container.clone();
                let last = std::sync::Mutex::new(std::time::Instant::now());
                move |_done: u64, _total: Option<u64>| {
                    let mut last = last.lock().expect("mihon setup progress poisoned");
                    if last.elapsed() >= std::time::Duration::from_millis(300) {
                        *last = std::time::Instant::now();
                        MihonRuntimeEvent(container.runtime_state())
                            .emit(&container.app)
                            .ok();
                    }
                }
            };
            let result = self.tools.ensure(announce, self.cancel.clone()).await;
            self.setting_up.store(false, Ordering::SeqCst);
            MihonRuntimeEvent(self.runtime_state()).emit(&self.app).ok();
            result
        }

        /// Stops a setup that's running.
        pub fn cancel_setup(&self) {
            self.cancel.notify_waiters();
        }

        /// The sources an installed APK extension registers; starts its sidecar.
        pub async fn sources(&self, extension: &ApkExtension) -> Result<Vec<Value>, String> {
            self.sidecar(extension).await?.sources().await
        }

        /// One `manga.source` call on an installed APK extension.
        pub async fn call(
            &self,
            extension: &ApkExtension,
            method: &str,
            params: Value,
        ) -> Result<Value, String> {
            self.sidecar(extension).await?.call(method, params).await
        }

        async fn sidecar(&self, extension: &ApkExtension) -> Result<Arc<Sidecar>, String> {
            if !self.tools.state().ready() {
                return Err(self.state().note);
            }
            let mut sidecars = self.sidecars.lock().await;
            if let Some(sidecar) = sidecars.get(&extension.package) {
                return Ok(sidecar.clone());
            }
            let load = Load {
                apk: extension.apk.clone(),
                work: self.data_root.join(&extension.package),
                package: extension.package.clone(),
                class: extension.class.clone(),
                user_agent: (self.user_agent)(),
                cookies: extension.cookies.clone(),
            };
            let sidecar = Arc::new(Sidecar::new(self.tools.clone(), load));
            sidecars.insert(extension.package.clone(), sidecar.clone());
            Ok(sidecar)
        }

        /// Stops sidecars that have sat idle, checked each `IDLE`.
        fn sweep_idle(self: Arc<Self>) {
            tauri::async_runtime::spawn(async move {
                loop {
                    tokio::time::sleep(IDLE).await;
                    let idle: Vec<Arc<Sidecar>> = {
                        let sidecars = self.sidecars.lock().await;
                        let mut idle = Vec::new();
                        for sidecar in sidecars.values() {
                            if !sidecar.touched_within(IDLE).await {
                                idle.push(sidecar.clone());
                            }
                        }
                        idle
                    };
                    for sidecar in idle {
                        sidecar.stop().await;
                    }
                }
            });
        }
    }
}

#[cfg(target_os = "android")]
pub use android::MihonContainer;

#[cfg(target_os = "android")]
mod android {
    use super::*;

    /// On Android the extension's dex loads in the `raygun-audio` plugin's `:mihon` process; Rust
    /// reaches it there, not here. This keeps the app's call sites the same on both platforms.
    pub struct MihonContainer;

    impl MihonContainer {
        pub fn new(
            _app: tauri::AppHandle,
            _root: &Path,
            _host_jar: PathBuf,
            _client: reqwest::Client,
            _user_agent: Arc<dyn Fn() -> String + Send + Sync>,
        ) -> Arc<Self> {
            Arc::new(Self)
        }

        pub fn state(&self) -> ContainerState {
            // The design is an isolated `:mihon` process in the raygun-audio plugin loading the
            // dex; it isn't wired in this build, so the UI stays honest.
            ContainerState {
                supported: false,
                ready: false,
                note: "Running a source's own code on Android isn't in this build yet".to_owned(),
            }
        }

        pub fn runtime_state(&self) -> raygun_model::MihonRuntimeState {
            let state = self.state();
            raygun_model::MihonRuntimeState {
                supported: state.supported,
                ready: state.ready,
                setting_up: false,
                note: state.note,
            }
        }
    }
}

/// An installed APK extension the container runs: where its checked APK is, the class Mihon
/// loads, and Raygun's cookies for its hosts.
#[derive(Debug, Clone)]
pub struct ApkExtension {
    pub package: String,
    pub apk: PathBuf,
    pub class: String,
    pub cookies: Vec<Value>,
}
