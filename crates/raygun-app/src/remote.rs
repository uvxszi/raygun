//! The LAN remote (M20): its switch, pairing and paired devices, and what a paired browser can
//! see and do. The server itself lives in `raygun-remote`.

use std::sync::{Arc, Mutex};

use raygun_model::{ConnectCommand, QueueSnapshot, RemoteDevice, RemoteStatus};
use raygun_remote::{Access, Backend, PairedDevice, Pairing, Running};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};

use crate::collections::{CollectionsService, portable_url};
use crate::connect::ConnectService;
use crate::extensions::ExtensionService;
use crate::library::LibraryService;
use crate::local_network::{LocalNetwork, REMOTE_DENIED};
use crate::mcp::{parse_control, track_arg};
use crate::playback::PlaybackService;
use crate::startup::AppDb;

const ENABLED_KEY: &str = "remote_enabled";
const DEVICES_KEY: &str = "remote_devices";
const SEARCH_LIBRARY_LIMIT: u32 = 8;
const SEARCH_TRACKS_PER_PROVIDER: usize = 8;

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
        let pairing = self.pairing.lock().expect("pairing poisoned");
        let pair_url = url
            .as_ref()
            .zip(pairing.current_code(now_ms()))
            .map(|(url, code)| format!("{url}/#pair={code}"));
        Ok(RemoteStatus {
            enabled,
            pair_qr_svg: pair_url.as_deref().and_then(qr_svg),
            pair_url,
            url,
            devices: pairing
                .devices()
                .iter()
                .map(|device| RemoteDevice {
                    id: device.id.clone(),
                    name: device.name.clone(),
                    paired_at: device.paired_at as f64,
                })
                .collect(),
            error,
        })
    }

    /// Turning the remote on opens a pairing code right away, so the QR code shows. On Android 17
    /// it needs local network access first, or no other device could reach it.
    pub async fn set_enabled(&self, enabled: bool) -> Result<RemoteStatus, String> {
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
            self.pairing
                .lock()
                .expect("pairing poisoned")
                .new_code(now_ms());
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

    pub async fn new_code(&self) -> Result<RemoteStatus, String> {
        self.pairing
            .lock()
            .expect("pairing poisoned")
            .new_code(now_ms());
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

    /// Removes a paired device; its browser loses access at once.
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
        let app = self.app.clone();
        let access = Access::new(
            self.pairing.clone(),
            move |devices| {
                let app = app.clone();
                tauri::async_runtime::spawn(async move {
                    let db = &app.state::<AppDb>().inner().0;
                    if let Err(message) = save_devices(db, &devices).await {
                        tracing::warn!(%message, "failed to save paired remote devices");
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
                tracing::info!(port = running.port, "remote listening on the local network");
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

/// The pairing address as a QR code.
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

/// What a paired browser sees and does.
struct RemoteBackend {
    app: AppHandle,
}

impl Backend for RemoteBackend {
    fn snapshot(&self) -> Value {
        let playback = self.app.state::<PlaybackService>();
        let state = playback.state();
        let queue = playback.queue();
        let current = state
            .current_item_id
            .as_ref()
            .and_then(|id| queue.items.iter().find(|item| &item.id == id));
        json!({
            "playback": {
                "status": state.status,
                "positionMs": state.position_ms.round(),
                "durationMs": state.duration_ms.map(f64::round),
                "volume": state.volume,
                "shuffle": state.shuffle,
                "repeat": state.repeat,
                "source": state.source_label,
                "error": state.error,
                "current": current.map(|item| json!({
                    "id": item.id,
                    "title": item.title,
                    "artist": item.artist,
                    "album": item.album,
                    "artworkUrl": remote_artwork(item.artwork_url.as_deref()),
                })),
            },
            "queue": queue.order.iter()
                .filter_map(|index| queue.items.get(*index as usize))
                .map(|item| json!({
                    "id": item.id,
                    "title": item.title,
                    "artist": item.artist,
                    "current": state.current_item_id.as_ref() == Some(&item.id),
                }))
                .collect::<Vec<_>>(),
        })
    }

    fn control(&self, command: &Value) -> Result<(), String> {
        let control = parse_control(command)?;
        self.app.state::<PlaybackService>().control(control);
        Ok(())
    }

    async fn search(&self, query: &str) -> Result<Value, String> {
        let library = self.app.state::<LibraryService>();
        let extensions = self.app.state::<Arc<ExtensionService>>();
        let (local, providers) = futures_util::future::join(
            library.search(query, SEARCH_LIBRARY_LIMIT),
            extensions.search(query),
        )
        .await;
        let top = crate::search::merge_top(&providers);
        let online = |track: &raygun_model::OnlineTrack| {
            json!({
                "title": track.title,
                "artist": track.artist,
                "album": track.album,
                "durationMs": track.duration_ms,
                "artworkUrl": remote_artwork(track.artwork_url.as_deref()),
            })
        };
        let mut groups = Vec::new();
        let local = local.unwrap_or_default();
        if !local.is_empty() {
            groups.push(json!({
                "name": "Your library",
                "tracks": local.iter().map(|track| json!({
                    "title": track.title,
                    "artist": track.artist,
                    "album": track.album,
                    "durationMs": track.duration_ms,
                    "artworkUrl": null,
                })).collect::<Vec<_>>(),
            }));
        }
        if !top.is_empty() {
            groups.push(json!({
                "name": "Top results",
                "tracks": top.iter().map(online).collect::<Vec<_>>(),
            }));
        }
        for result in &providers {
            groups.push(json!({
                "name": result.provider.name,
                "error": result.error,
                "tracks": result.tracks.iter().take(SEARCH_TRACKS_PER_PROVIDER)
                    .map(online).collect::<Vec<_>>(),
            }));
        }
        Ok(json!({ "groups": groups }))
    }

    async fn play(&self, track: &Value) -> Result<(), String> {
        let track = track_arg(track)?;
        let item = self
            .app
            .state::<CollectionsService>()
            .queue_item_for(track)
            .await?;
        self.app
            .state::<PlaybackService>()
            .enqueue(vec![item], true);
        Ok(())
    }

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

/// Artwork a browser elsewhere on the network can load: the local artwork server only listens on
/// this device.
fn remote_artwork(url: Option<&str>) -> Option<&str> {
    url.filter(|url| portable_url(url))
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
    fn the_qr_code_is_svg_and_artwork_must_be_reachable() {
        let svg = qr_svg("http://192.168.1.20:8820/#pair=abcd234567").unwrap();
        assert!(svg.contains("<svg"));
        assert!(svg.contains("#000000"));

        assert_eq!(
            remote_artwork(Some("https://coverartarchive.org/a.jpg")),
            Some("https://coverartarchive.org/a.jpg")
        );
        assert_eq!(
            remote_artwork(Some("http://127.0.0.1:40371/art/a.png")),
            None
        );
        assert_eq!(remote_artwork(None), None);
    }
}
