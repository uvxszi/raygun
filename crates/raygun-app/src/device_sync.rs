//! Device sync (M21): the switch, this device's identity, pairing, peers and the log, plus
//! discovery. The protocol lives in `raygun-sync`; merging in `raygun-db::sync`.

use std::net::{IpAddr, Ipv4Addr};
use std::sync::{Arc, Mutex};

use raygun_db::sync::{self as store, SyncKind as StoreKind};
use raygun_model::{SyncKind, SyncPeerInfo, SyncSessionInfo, SyncStatus};
use raygun_sync::discovery::Discovery;
use raygun_sync::{Engine, Identity, Running, fingerprint, to_hex};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::SyncChangedEvent;
use crate::startup::AppDb;

const ENABLED_KEY: &str = "sync_enabled";
const PRIVATE_KEY: &str = "sync_private_key";
const PUBLIC_KEY: &str = "sync_public_key";
const KINDS_KEY: &str = "sync_kinds";
const NAME_KEY: &str = "sync_device_name";
/// Sessions shown in the log.
const LOG_LENGTH: i64 = 20;

pub struct SyncService {
    app: AppHandle,
    engine: Arc<Engine>,
    state: Mutex<ServerState>,
}

#[derive(Default)]
struct ServerState {
    running: Option<Running>,
    discovery: Option<Discovery>,
    error: Option<String>,
}

impl SyncService {
    /// Loads this device's identity (creating it once) and starts serving when sync was left on.
    pub async fn start(app: AppHandle) -> Result<Self, String> {
        let db = app.state::<AppDb>().inner().0.clone();
        let identity = load_identity(&db).await?;
        let kinds = raygun_db::local_value(&db, KINDS_KEY)
            .await
            .map_err(to_message)?
            .and_then(|json| serde_json::from_str::<Vec<StoreKind>>(&json).ok())
            .unwrap_or_else(|| StoreKind::ALL.to_vec());
        let name = raygun_db::local_value(&db, NAME_KEY)
            .await
            .map_err(to_message)?
            .unwrap_or_else(default_device_name);
        let engine = Arc::new(Engine::new(db.clone(), identity, name, kinds));
        let events = app.clone();
        engine.on_synced(Arc::new(move |_peer, summary| {
            if summary.changed > 0
                && let Err(error) = SyncChangedEvent(summary.changed as u32).emit(&events)
            {
                tracing::warn!(%error, "failed to emit sync event");
            }
        }));
        let service = Self {
            app,
            engine,
            state: Mutex::new(ServerState::default()),
        };
        if service.enabled().await? {
            service.run().await;
            service.sync_all_in_background();
        }
        Ok(service)
    }

    pub async fn status(&self) -> Result<SyncStatus, String> {
        let db = self.db();
        let enabled = self.enabled().await?;
        let pairing_text = self.engine.open_offer(lan_ip());
        let peers = store::peers(&db).await.map_err(to_message)?;
        let log = store::sessions(&db, LOG_LENGTH).await.map_err(to_message)?;
        Ok(SyncStatus {
            enabled,
            device_name: self.engine.name(),
            fingerprint: fingerprint(&self.engine.identity().public),
            pairing_qr_svg: pairing_text.as_deref().and_then(crate::remote::qr_svg),
            pairing_text,
            kinds: self.engine.kinds().into_iter().map(model_kind).collect(),
            peers: peers
                .into_iter()
                .map(|peer| SyncPeerInfo {
                    fingerprint: raygun_sync::channel::from_hex(&peer.public_key)
                        .map(|key| fingerprint(&key))
                        .unwrap_or_default(),
                    device_id: peer.device_id,
                    name: peer.name,
                    address: peer.address,
                    paired_at: peer.paired_at as f64,
                    last_sync_at: peer.last_sync_at.map(|at| at as f64),
                })
                .collect(),
            log: log
                .into_iter()
                .map(|session| SyncSessionInfo {
                    id: session.id as u32,
                    peer_name: session.peer_name,
                    started_at: session.started_at as f64,
                    finished: session.finished_at.is_some(),
                    sent: session.sent as u32,
                    received: session.received as u32,
                    changed: session.changed as u32,
                    error: session.error,
                    undone: session.undone,
                })
                .collect(),
            error: self.lock().error.clone(),
        })
    }

    pub async fn set_enabled(&self, enabled: bool) -> Result<SyncStatus, String> {
        raygun_db::set_local_value(
            &self.db(),
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
            state.discovery = None;
            if let Some(running) = state.running.take() {
                running.stop();
                tracing::info!("device sync stopped");
            }
            state.error = None;
        }
        self.status().await
    }

    /// Opens a pairing code; the status then carries the text and QR code to show.
    pub async fn open_pairing(&self) -> Result<SyncStatus, String> {
        self.engine
            .open_pairing(lan_ip())
            .map_err(|error| error.to_string())?;
        self.status().await
    }

    /// Pairs with the device that showed `text` and syncs with it right away.
    pub async fn join(&self, text: &str) -> Result<SyncStatus, String> {
        let peer = self
            .engine
            .join(text)
            .await
            .map_err(|error| error.to_string())?;
        if let Err(error) = self.engine.sync_with(&peer.device_id).await {
            tracing::warn!(%error, "first sync after pairing failed");
        }
        self.status().await
    }

    /// Syncs with one device, or every paired device.
    pub async fn sync_now(&self, device_id: Option<&str>) -> Result<SyncStatus, String> {
        match device_id {
            Some(device_id) => {
                self.engine
                    .sync_with(device_id)
                    .await
                    .map_err(|error| error.to_string())?;
            }
            None => self.sync_all().await?,
        }
        self.status().await
    }

    pub async fn remove_peer(&self, device_id: &str) -> Result<SyncStatus, String> {
        store::remove_peer(&self.db(), device_id)
            .await
            .map_err(to_message)?;
        self.status().await
    }

    pub async fn set_kinds(&self, kinds: Vec<SyncKind>) -> Result<SyncStatus, String> {
        let kinds: Vec<StoreKind> = kinds.into_iter().map(store_kind).collect();
        let json = serde_json::to_string(&kinds).map_err(to_message)?;
        raygun_db::set_local_value(&self.db(), KINDS_KEY, &json)
            .await
            .map_err(to_message)?;
        self.engine.set_kinds(kinds);
        self.status().await
    }

    pub async fn set_device_name(&self, name: &str) -> Result<SyncStatus, String> {
        let name: String = name.trim().chars().take(40).collect();
        if name.is_empty() {
            return Err("A device needs a name".into());
        }
        raygun_db::set_local_value(&self.db(), NAME_KEY, &name)
            .await
            .map_err(to_message)?;
        self.engine.set_name(name);
        self.status().await
    }

    /// Undoes the newest session that changed something here.
    pub async fn undo_last(&self) -> Result<SyncStatus, String> {
        let db = self.db();
        let last = store::sessions(&db, LOG_LENGTH)
            .await
            .map_err(to_message)?
            .into_iter()
            .find(|session| !session.undone && session.changed > 0)
            .ok_or_else(|| "No sync to undo".to_owned())?;
        let restored = store::undo(&db, last.id).await.map_err(to_message)?;
        tracing::info!(session = last.id, restored, "sync undone");
        if let Err(error) = SyncChangedEvent(restored as u32).emit(&self.app) {
            tracing::warn!(%error, "failed to emit sync event");
        }
        self.status().await
    }

    /// This device for a link (M46.47): its sync id, public key and address, while sync runs.
    pub fn link_info(&self) -> Option<(String, String, Option<String>)> {
        let port = self.engine.port()?;
        Some((
            self.engine.device_id().to_owned(),
            to_hex(&self.engine.identity().public),
            Some(std::net::SocketAddr::new(lan_ip(), port).to_string()),
        ))
    }

    /// Keeps a linked device as a sync peer (M46.47): trusted by its key, no code needed. A
    /// device linked again with the same key keeps what it already received.
    pub async fn link_peer(&self, card: &crate::device_link::LinkCard) -> Result<(), String> {
        if card.sync_id == self.engine.device_id() {
            return Err("That's this device".into());
        }
        if raygun_sync::channel::from_hex(&card.public_key).is_none() {
            return Err("The other device's key can't be read".into());
        }
        let db = self.db();
        let known = store::peer(&db, &card.sync_id).await.map_err(to_message)?;
        match known {
            Some(peer) if peer.public_key == card.public_key => {
                store::peer_seen(&db, &card.sync_id, card.sync_address.as_deref(), None)
                    .await
                    .map_err(to_message)?;
            }
            _ => {
                store::save_peer(
                    &db,
                    &store::Peer {
                        device_id: card.sync_id.clone(),
                        name: card.name.clone(),
                        public_key: card.public_key.clone(),
                        address: card.sync_address.clone(),
                        paired_at: crate::catalog::now_ms(),
                        last_sync_at: None,
                        sent_seq: 0,
                        kinds: Vec::new(),
                    },
                )
                .await
                .map_err(to_message)?;
                tracing::info!(device = %card.name, "linked for sync");
            }
        }
        Ok(())
    }

    /// Sync is on and some device is linked.
    pub async fn has_linked(&self) -> bool {
        self.enabled().await.unwrap_or(false)
            && store::peers(&self.db())
                .await
                .is_ok_and(|peers| !peers.is_empty())
    }

    /// Syncs with every linked device, one after another; failures only go to the log.
    pub async fn sync_quietly(&self) {
        if let Err(message) = self.sync_all().await {
            tracing::info!(%message, "sync with linked devices failed");
        }
    }

    /// When each linked device last synced, by its sync id.
    pub async fn last_synced(&self) -> std::collections::HashMap<String, i64> {
        store::peers(&self.db())
            .await
            .unwrap_or_default()
            .into_iter()
            .filter_map(|peer| peer.last_sync_at.map(|at| (peer.device_id, at)))
            .collect()
    }

    async fn sync_all(&self) -> Result<(), String> {
        for peer in store::peers(&self.db()).await.map_err(to_message)? {
            if let Err(error) = self.engine.sync_with(&peer.device_id).await {
                tracing::info!(peer = %peer.name, %error, "sync failed");
            }
        }
        Ok(())
    }

    fn sync_all_in_background(&self) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let service = app.state::<SyncService>();
            if let Err(message) = service.sync_all().await {
                tracing::warn!(%message, "sync on start failed");
            }
        });
    }

    async fn run(&self) {
        let result = self.engine.serve().await;
        let mut state = self.lock();
        match result {
            Ok(running) => {
                tracing::info!(port = running.port, "device sync listening");
                state.discovery = self.discover(running.port);
                state.running = Some(running);
                state.error = None;
            }
            Err(error) => {
                tracing::warn!(%error, "device sync could not start");
                state.error = Some(format!("Couldn't start: {error}"));
            }
        }
    }

    /// Keeps paired devices' addresses current from mDNS. Failing only loses discovery: the last
    /// known addresses still work.
    fn discover(&self, port: u16) -> Option<Discovery> {
        let db = self.db();
        let started = Discovery::start(
            self.engine.device_id(),
            lan_ip(),
            port,
            move |id, address| {
                let db = db.clone();
                tauri::async_runtime::spawn(async move {
                    if let Ok(Some(peer)) = store::peer(&db, &id).await
                        && peer.address.as_deref() != Some(address.as_str())
                    {
                        tracing::info!(peer = %peer.name, %address, "found a paired device");
                        let _ = store::peer_seen(&db, &id, Some(&address), None).await;
                    }
                });
            },
        );
        match started {
            Ok(discovery) => Some(discovery),
            Err(error) => {
                tracing::warn!(%error, "device discovery unavailable");
                None
            }
        }
    }

    async fn enabled(&self) -> Result<bool, String> {
        Ok(raygun_db::local_value(&self.db(), ENABLED_KEY)
            .await
            .map_err(to_message)?
            .as_deref()
            == Some("true"))
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, ServerState> {
        self.state.lock().expect("sync state poisoned")
    }
}

async fn load_identity(db: &raygun_db::Db) -> Result<Identity, String> {
    let private = raygun_db::local_value(db, PRIVATE_KEY)
        .await
        .map_err(to_message)?;
    let public = raygun_db::local_value(db, PUBLIC_KEY)
        .await
        .map_err(to_message)?;
    if let Some(identity) = private
        .zip(public)
        .and_then(|(private, public)| Identity::from_hex(&private, &public))
    {
        return Ok(identity);
    }
    let identity = Identity::generate().map_err(|error| error.to_string())?;
    raygun_db::set_local_value(db, PRIVATE_KEY, &to_hex(&identity.private))
        .await
        .map_err(to_message)?;
    raygun_db::set_local_value(db, PUBLIC_KEY, &to_hex(&identity.public))
        .await
        .map_err(to_message)?;
    Ok(identity)
}

fn lan_ip() -> IpAddr {
    raygun_remote::lan_ip().unwrap_or(IpAddr::V4(Ipv4Addr::LOCALHOST))
}

/// This device's name as other devices see it (sync, the remote, Connect).
pub async fn device_name(db: &raygun_db::Db) -> String {
    raygun_db::local_value(db, NAME_KEY)
        .await
        .ok()
        .flatten()
        .filter(|name| !name.trim().is_empty())
        .unwrap_or_else(default_device_name)
}

fn default_device_name() -> String {
    match std::env::consts::OS {
        "android" => "Android phone".into(),
        "ios" => "iPhone".into(),
        "windows" => "Windows PC".into(),
        "macos" => "Mac".into(),
        _ => "Computer".into(),
    }
}

fn model_kind(kind: StoreKind) -> SyncKind {
    match kind {
        StoreKind::Settings => SyncKind::Settings,
        StoreKind::Library => SyncKind::Library,
        StoreKind::History => SyncKind::History,
    }
}

fn store_kind(kind: SyncKind) -> StoreKind {
    match kind {
        SyncKind::Settings => StoreKind::Settings,
        SyncKind::Library => StoreKind::Library,
        SyncKind::History => StoreKind::History,
    }
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kinds_convert_both_ways() {
        for kind in StoreKind::ALL {
            assert_eq!(store_kind(model_kind(kind)), kind);
        }
        assert!(!default_device_name().is_empty());
    }
}
