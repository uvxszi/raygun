//! Android 17's local network permission (`ACCESS_LOCAL_NETWORK`). Raygun asks for it before it
//! reaches a device on the user's network (a Stremio addon or a stream on the LAN) or lets devices
//! there reach it (the LAN remote); without it, those connections time out. The UI explains why
//! and asks (`local_network_request`); requests to the local network without it fail at once with
//! a message saying what to allow. Which hosts are local is `raygun_net::local_network`.

use std::sync::atomic::{AtomicBool, Ordering};

use raygun_model::LocalNetworkAccess;
use raygun_net::local_network::reaches_local_network;
use tauri::AppHandle;
use tauri_plugin_raygun_audio::LocalNetworkState;

/// An addon on the local network can't be reached without the permission.
pub const ADDONS_DENIED: &str = "Allow local network access to reach addons on your network";
/// A stream on the local network can't be played without the permission.
pub const VIDEOS_DENIED: &str = "Allow local network access to play videos from your network";
/// Devices on the network can't reach the LAN remote without the permission.
pub const REMOTE_DENIED: &str =
    "Allow local network access so devices on your network can reach the remote";

pub struct LocalNetwork {
    app: AppHandle,
    /// Set once the local network is open to the app. Android restarts an app whose permission is
    /// taken back, so a yes holds for the process; a no is checked again each time.
    allowed: AtomicBool,
}

impl LocalNetwork {
    pub fn new(app: AppHandle) -> Self {
        Self {
            app,
            allowed: AtomicBool::new(false),
        }
    }

    /// The permission for a connection to `address` (a URL, or an addon address without a
    /// scheme), or for devices reaching the LAN remote when there is none.
    pub async fn access(&self, address: Option<&str>) -> Result<LocalNetworkAccess, String> {
        let state = self.state().await?;
        let local = match address {
            Some(address) => reaches_local_network(&as_url(address)).await,
            None => true,
        };
        Ok(view(state, local))
    }

    /// Shows the system dialog unless the permission is granted or not needed.
    pub async fn request(&self) -> Result<LocalNetworkAccess, String> {
        let state = platform::request(&self.app).await?;
        self.remember(state);
        Ok(view(state, true))
    }

    pub async fn open_settings(&self) -> Result<(), String> {
        platform::open_settings(&self.app).await
    }

    /// Whether the app may use the local network now.
    pub async fn allowed(&self) -> bool {
        self.allowed.load(Ordering::Relaxed) || self.state().await.is_ok_and(|state| state.granted)
    }

    /// Fails at once with `message` when `url` is on the local network and the app may not reach
    /// it, rather than letting the request time out.
    pub async fn check(&self, url: &str, message: &str) -> Result<(), String> {
        if self.allowed.load(Ordering::Relaxed)
            || !reaches_local_network(url).await
            || self.allowed().await
        {
            Ok(())
        } else {
            Err(message.to_owned())
        }
    }

    async fn state(&self) -> Result<LocalNetworkState, String> {
        let state = platform::state(&self.app).await?;
        self.remember(state);
        Ok(state)
    }

    fn remember(&self, state: LocalNetworkState) {
        if state.granted {
            self.allowed.store(true, Ordering::Relaxed);
        }
    }
}

fn view(state: LocalNetworkState, local: bool) -> LocalNetworkAccess {
    LocalNetworkAccess {
        needed: state.enforced && local,
        granted: state.granted,
        blocked: state.blocked,
    }
}

/// An address as typed (`192.168.1.5:7000/manifest.json`, `stremio://…`) as a URL whose host can
/// be read.
fn as_url(address: &str) -> String {
    let address = address.trim();
    if let Some(rest) = address.strip_prefix("stremio://") {
        format!("http://{rest}")
    } else if address.contains("://") {
        address.to_owned()
    } else {
        format!("http://{address}")
    }
}

#[cfg(target_os = "android")]
mod platform {
    use tauri::{AppHandle, Manager};
    use tauri_plugin_raygun_audio::{LocalNetworkState, NativeAudio};

    /// Runs a plugin call on a blocking thread: it waits for Android's main thread, and for the
    /// user when a dialog shows.
    async fn call<T: Send + 'static>(
        app: &AppHandle,
        run: impl FnOnce(&NativeAudio<tauri::Wry>) -> Result<T, String> + Send + 'static,
    ) -> Result<T, String> {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            let plugin = app
                .try_state::<NativeAudio<tauri::Wry>>()
                .ok_or_else(|| "Android's permissions aren't available".to_owned())?;
            run(&plugin)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub async fn state(app: &AppHandle) -> Result<LocalNetworkState, String> {
        call(app, |plugin| plugin.local_network_state()).await
    }

    pub async fn request(app: &AppHandle) -> Result<LocalNetworkState, String> {
        call(app, |plugin| plugin.request_local_network()).await
    }

    pub async fn open_settings(app: &AppHandle) -> Result<(), String> {
        call(app, |plugin| plugin.open_app_settings()).await
    }
}

/// Other platforms don't restrict the local network.
#[cfg(not(target_os = "android"))]
mod platform {
    use tauri::AppHandle;
    use tauri_plugin_raygun_audio::LocalNetworkState;

    const OPEN: LocalNetworkState = LocalNetworkState {
        enforced: false,
        granted: true,
        blocked: false,
    };

    pub async fn state(_app: &AppHandle) -> Result<LocalNetworkState, String> {
        Ok(OPEN)
    }

    pub async fn request(_app: &AppHandle) -> Result<LocalNetworkState, String> {
        Ok(OPEN)
    }

    pub async fn open_settings(_app: &AppHandle) -> Result<(), String> {
        Err("Opening app settings is only needed on Android".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn typed_addresses_become_urls() {
        assert_eq!(
            as_url(" 192.168.1.5:7000/manifest.json "),
            "http://192.168.1.5:7000/manifest.json"
        );
        assert_eq!(
            as_url("stremio://nas.local/manifest.json"),
            "http://nas.local/manifest.json"
        );
        assert_eq!(
            as_url("https://v3-cinemeta.strem.io"),
            "https://v3-cinemeta.strem.io"
        );
    }

    #[test]
    fn access_is_needed_only_where_enforced_and_local() {
        let denied = LocalNetworkState {
            enforced: true,
            granted: false,
            blocked: true,
        };
        assert_eq!(
            view(denied, true),
            LocalNetworkAccess {
                needed: true,
                granted: false,
                blocked: true
            }
        );
        assert!(!view(denied, false).needed);
        let open = LocalNetworkState {
            enforced: false,
            granted: true,
            blocked: false,
        };
        assert!(!view(open, true).needed);
    }
}
