//! The server other Raygun devices link with and play through (M20, M46.15, M46.47): its switch,
//! the one-time code a link QR code carries, the tokens linked devices hold, and what Connect
//! does here. The server itself lives in `raygun-remote`.

use std::sync::{Arc, Mutex};

use raygun_model::{ConnectCommand, QueueSnapshot, RemoteStatus};
use raygun_remote::{Access, Backend, PairedDevice, Pairing, Running};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::connect::ConnectService;
use crate::local_network::{LocalNetwork, REMOTE_DENIED};
use crate::playback::PlaybackService;
use crate::startup::AppDb;

const ENABLED_KEY: &str = "remote_enabled";
const DEVICES_KEY: &str = "remote_devices";

pub struct RemoteService {
    app: AppHandle,
    pairing: Arc<Mutex<Pairing>>,
    state: Mutex<ServerState>,
}

#[derive(Default)]
struct ServerState {
    running: Option<Running>,
    /// This device on the network for Connect (M46.15), while the server runs.
    advertised: Option<raygun_remote::discovery::Advertised>,
    error: Option<String>,
}

impl RemoteService {
    /// Loads paired devices and starts the server when it was left on.
    pub async fn start(app: AppHandle) -> Result<Self, String> {
        let db = &app.state::<AppDb>().inner().0;
        let devices: Vec<PairedDevice> = raygun_db::local_value(db, DEVICES_KEY)
            .await
            .map_err(to_message)?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let service = Self {
            app: app.clone(),
            pairing: Arc::new(Mutex::new(Pairing::new(devices))),
            state: Mutex::new(ServerState::default()),
        };
        if service.enabled().await? {
            service.run().await;
        }
        Ok(service)
    }

    pub async fn status(&self) -> Result<RemoteStatus, String> {
        let enabled = self.enabled().await?;
        let (url, mut error) = {
            let state = self.lock();
            let url = state
                .running
                .as_ref()
                .map(|running| match raygun_remote::lan_ip() {
                    Some(ip) => format!("http://{ip}:{}", running.port),
                    None => format!("http://localhost:{}", running.port),
                });
            (url, state.error.clone())
        };
        // Left on while Android 17's local network permission was taken back: only this device
        // can reach it.
        if url.is_some() && error.is_none() && !self.local_network().allowed().await {
            error = Some(REMOTE_DENIED.to_owned());
        }
        let now = now_ms();
        let open = {
            let pairing = self.pairing.lock().expect("pairing poisoned");
            pairing
                .current_code(now)
                .map(str::to_owned)
                .zip(pairing.code_expires_at(now))
        };
        // The QR code names the address other devices reach: only with a LAN address.
        let address = url.as_ref().and(self.address());
        let link = address.zip(open).map(|(address, (code, expires))| {
            let invite = crate::device_link::LinkInvite {
                address,
                code,
                name: self.app.state::<Arc<ConnectService>>().me().name,
                // Raygun on the web links through sync's browser listener (docs/WEB.md).
                web: self
                    .app
                    .try_state::<crate::device_sync::SyncService>()
                    .map(|sync| sync.web_link_params())
                    .unwrap_or_default(),
            };
            (invite.to_text(), expires)
        });
        Ok(RemoteStatus {
            enabled,
            link_qr_svg: link.as_ref().and_then(|(text, _)| qr_svg(text)),
            link_expires_at: link.as_ref().map(|(_, expires)| *expires as f64),
            link_text: link.map(|(text, _)| text),
            url,
            error,
        })
    }

    /// Turning the server on. On Android 17 it needs local network access first, or no other
    /// device could reach it.
    pub async fn set_enabled(&self, enabled: bool) -> Result<RemoteStatus, String> {
        if enabled {
            raygun_net::network_mode::check_online()?;
        }
        if enabled && !self.local_network().allowed().await {
            return Err(REMOTE_DENIED.into());
        }
        raygun_db::set_local_value(
            self.db(),
            ENABLED_KEY,
            if enabled { "true" } else { "false" },
        )
        .await
        .map_err(to_message)?;
        if enabled {
            if self.lock().running.is_none() {
                self.run().await;
            }
        } else {
            let mut state = self.lock();
            if let Some(running) = state.running.take() {
                running.stop();
                tracing::info!("remote stopped");
            }
            state.advertised = None;
            state.error = None;
        }
        self.status().await
    }

    /// Offline Mode turned on or off: the server stops while it's on, so nothing reaches it or is
    /// announced; once it's off, it runs again if it was on.
    pub async fn follow_network(&self, offline: bool) {
        if offline {
            let mut state = self.lock();
            if let Some(running) = state.running.take() {
                running.stop();
                tracing::info!("remote stopped while offline");
            }
            state.advertised = None;
        } else if self.enabled().await.unwrap_or(false) && self.lock().running.is_none() {
            self.run().await;
        }
    }

    /// Opens a new one-time code for this device's link QR code (the server starts first),
    /// replacing the one before.
    pub async fn new_code(&self) -> Result<RemoteStatus, String> {
        self.ensure_running().await?;
        if let Some(error) = self.lock().error.clone() {
            return Err(error);
        }
        self.pairing
            .lock()
            .expect("pairing poisoned")
            .new_code(now_ms());
        // A browser that scans the code pairs through sync, with a code of its own.
        if let Some(sync) = self.app.try_state::<crate::device_sync::SyncService>() {
            sync.open_web_pairing();
        }
        self.status().await
    }

    /// Starts the server for a link (M46.47) when it isn't running.
    pub async fn ensure_running(&self) -> Result<(), String> {
        if self.lock().running.is_some() {
            return Ok(());
        }
        self.set_enabled(true).await.map(|_| ())
    }

    /// Tells the network this device's name again (after it was renamed), while the server runs.
    pub fn readvertise(&self) {
        let me = self.app.state::<Arc<ConnectService>>().me();
        let mut state = self.lock();
        let Some(port) = state.running.as_ref().map(|running| running.port) else {
            return;
        };
        // The old name goes first: the new one replaces it under the same instance.
        state.advertised = None;
        state.advertised = raygun_remote::lan_ip().and_then(|ip| {
            raygun_remote::discovery::advertise(&me.id, &me.name, me.kind, ip, port)
                .map_err(|error| tracing::warn!(%error, "couldn't advertise for Connect"))
                .ok()
        });
    }

    /// Where Connect reaches this device: `<lan ip>:<port>`, while the server runs.
    pub fn address(&self) -> Option<String> {
        let port = self.lock().running.as_ref()?.port;
        Some(format!("{}:{port}", raygun_remote::lan_ip()?))
    }

    /// A token for a linked device (M46.47), so it plays here without asking; `(id, token)`.
    pub async fn grant(&self, name: &str) -> Option<(String, String)> {
        // Only while the server runs: the token is for reaching it.
        self.lock().running.as_ref()?;
        let (device, devices) = {
            let mut pairing = self.pairing.lock().expect("pairing poisoned");
            let device = pairing.admit(name, now_ms());
            (device, pairing.devices().to_vec())
        };
        if let Err(message) = save_devices(self.db(), &devices).await {
            tracing::warn!(%message, "failed to save a linked device's token");
        }
        Some((device.id, device.token))
    }

    /// Takes back a linked device's token; it loses access at once.
    pub async fn forget(&self, id: &str) -> Result<RemoteStatus, String> {
        let devices = {
            let mut pairing = self.pairing.lock().expect("pairing poisoned");
            pairing.forget(id);
            pairing.devices().to_vec()
        };
        save_devices(self.db(), &devices).await?;
        self.status().await
    }

    async fn enabled(&self) -> Result<bool, String> {
        Ok(raygun_db::local_value(self.db(), ENABLED_KEY)
            .await
            .map_err(to_message)?
            .as_deref()
            == Some("true"))
    }

    async fn run(&self) {
        // Nothing listens on the local network while Offline Mode is on (`follow_network`).
        if raygun_net::network_mode::offline() {
            return;
        }
        let app = self.app.clone();
        let access = Access::new(
            self.pairing.clone(),
            move |devices| {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let db = &app.state::<AppDb>().inner().0;
                    if let Err(message) = save_devices(db, &devices).await {
                        tracing::warn!(%message, "failed to save linked devices' tokens");
                    }
                });
            },
            now_ms,
        );
        let router = raygun_remote::router(
            RemoteBackend {
                app: self.app.clone(),
            },
            access,
        );
        let result = raygun_remote::serve(router).await;
        let me = self.app.state::<Arc<ConnectService>>().me();
        let mut state = self.lock();
        match result {
            Ok(running) => {
                tracing::info!(port = running.port, "listening for linked devices");
                // Other Raygun devices hear of this one, to play on it (M46.15).
                state.advertised = raygun_remote::lan_ip().and_then(|ip| {
                    raygun_remote::discovery::advertise(&me.id, &me.name, me.kind, ip, running.port)
                        .map_err(|error| tracing::warn!(%error, "couldn't advertise for Connect"))
                        .ok()
                });
                state.running = Some(running);
                state.error = None;
            }
            Err(error) => {
                tracing::warn!(%error, "remote could not start");
                state.error = Some(format!("Couldn't start: {error}"));
            }
        }
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    fn local_network(&self) -> tauri::State<'_, LocalNetwork> {
        self.app.state::<LocalNetwork>()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ServerState> {
        self.state.lock().expect("remote state poisoned")
    }
}

async fn save_devices(db: &raygun_db::Db, devices: &[PairedDevice]) -> Result<(), String> {
    let json = serde_json::to_string(devices).map_err(to_message)?;
    raygun_db::set_local_value(db, DEVICES_KEY, &json)
        .await
        .map_err(to_message)
}

/// A link code as a QR code.
pub(crate) fn qr_svg(text: &str) -> Option<String> {
    let code = qrcode::QrCode::new(text.as_bytes()).ok()?;
    Some(
        code.render::<qrcode::render::svg::Color<'_>>()
            .min_dimensions(240, 240)
            .quiet_zone(true)
            .dark_color(qrcode::render::svg::Color("#000000"))
            .light_color(qrcode::render::svg::Color("#ffffff"))
            .build(),
    )
}

/// What linked devices see and do here.
struct RemoteBackend {
    app: AppHandle,
}

impl Backend for RemoteBackend {
    fn device(&self) -> Value {
        let me = self.app.state::<Arc<ConnectService>>().me();
        json!({ "id": me.id, "name": me.name, "kind": me.kind })
    }

    /// The player as another Raygun shows it: sources it can play and artwork at its source.
    fn connect_snapshot(&self) -> (Value, Value) {
        let playback = self.app.state::<PlaybackService>();
        let images = self.app.state::<raygun_net::ImageLinks>();
        let original = |url: &str| images.original(url);
        let queue = playback.queue();
        let queue = QueueSnapshot {
            items: queue
                .items
                .into_iter()
                .map(|item| crate::connect::portable_queue_item(item, &original))
                .collect(),
            order: queue.order,
            context: queue.context,
        };
        (
            serde_json::to_value(playback.state()).unwrap_or_default(),
            serde_json::to_value(queue).unwrap_or_default(),
        )
    }

    fn connect_command(&self, command: &Value) -> Result<(), String> {
        let command: ConnectCommand =
            serde_json::from_value(command.clone()).map_err(|_| "Unknown command".to_owned())?;
        // Playing here from another device makes this the active one: if this device was
        // playing elsewhere, that ends.
        if crate::connect::starts_playback(&command) {
            self.app
                .state::<Arc<ConnectService>>()
                .inner()
                .playing_here();
        }
        let images = self.app.state::<raygun_net::ImageLinks>();
        self.app
            .state::<PlaybackService>()
            .apply(crate::connect::arrived(command, &images));
        Ok(())
    }

    async fn ask(&self, name: &str) -> bool {
        let connect = self.app.state::<Arc<ConnectService>>().inner().clone();
        connect.ask(name).await
    }

    /// A device just allowed here sent its link card (M46.47): keep it for sync and Connect,
    /// and answer with this device's.
    async fn link(&self, card: Option<&Value>, granted: &str) -> Option<Value> {
        let card: crate::device_link::LinkCard = serde_json::from_value(card?.clone()).ok()?;
        crate::device_link::accept(&self.app, &card, Some(granted)).await;
        let mine = crate::device_link::card_for(&self.app, &card.name, false).await?;
        serde_json::to_value(mine.0).ok()
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_qr_code_is_svg() {
        let svg = qr_svg("raygun:link?at=192.168.1.20%3A8820&code=abcd234567&name=PC").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("#000000"));
    }
}
