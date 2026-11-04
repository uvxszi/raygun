//! Connect (M46.15): playing on another Raygun device, as Spotify's Connect does.
//!
//! This device finds others (mDNS, or an address typed in), asks once to play on one, moves the
//! queue there, and then hands that device every change of its own player
//! ([`PlaybackService::set_redirect`]) while it follows the other player by events. "Play here"
//! takes the queue back.
//!
//! The other side, what the LAN remote server does for a paired Raygun, lives in `remote.rs`;
//! the question to this device's user is [`ConnectService::ask`].

use std::collections::HashMap;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex, Weak};
use std::time::{Duration, Instant};

use futures_util::StreamExt;
use raygun_model::{
    ConnectCommand, ConnectDevice, ConnectRequest, ConnectSession, ConnectStatus, MediaSource,
    PlayContext, PlaybackControl, PlaybackState, PlaybackStatus, QueueItem, QueueItemInput,
    QueueSnapshot, RepeatMode,
};
use raygun_net::ImageLinks;
use serde::{Deserialize, Serialize};
use serde_json::json;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::{mpsc, oneshot};

use crate::collections::portable_url;
use crate::events::{ConnectEvent, ConnectRequestEvent};
use crate::local_network::{CONNECT_DENIED, LocalNetwork};
use crate::playback::PlaybackService;
use crate::startup::AppDb;

const DEVICES_KEY: &str = "connect_devices";
const ID_KEY: &str = "connect_device_id";
/// How long this device's user has to answer another device.
const ASK_TIMEOUT: Duration = Duration::from_secs(60);
/// How long a device that asks waits for the answer: a little longer than the other side asks.
const HELLO_TIMEOUT: Duration = Duration::from_secs(75);
const REQUEST_TIMEOUT: Duration = Duration::from_secs(8);
/// Tries to follow a device again after its events stop, before giving up on it.
const RECONNECTS: u32 = 3;

/// This device as others see it.
#[derive(Debug, Clone)]
pub struct Identity {
    pub id: String,
    pub name: String,
    /// `phone` or `computer`.
    pub kind: &'static str,
}

impl Identity {
    async fn load(db: &raygun_db::Db) -> Self {
        let id = match raygun_db::local_value(db, ID_KEY).await.ok().flatten() {
            Some(id) => id,
            None => {
                let id = uuid::Uuid::now_v7().simple().to_string();
                if let Err(error) = raygun_db::set_local_value(db, ID_KEY, &id).await {
                    tracing::warn!(%error, "failed to keep this device's Connect id");
                }
                id
            }
        };
        Self {
            id,
            name: crate::device_sync::device_name(db).await,
            kind: if cfg!(any(target_os = "android", target_os = "ios")) {
                "phone"
            } else {
                "computer"
            },
        }
    }
}

/// A device this one knows: heard, typed in, or allowed to play on (then with a token).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Known {
    id: String,
    name: String,
    kind: String,
    address: String,
    #[serde(default)]
    token: Option<String>,
    /// Its sync id once linked (M46.47).
    #[serde(default)]
    sync_id: Option<String>,
    /// The token this device minted for it, to take back when they unlink.
    #[serde(default)]
    granted: Option<String>,
}

#[derive(Deserialize)]
struct DeviceInfo {
    id: String,
    name: String,
    #[serde(default)]
    kind: String,
}

#[derive(Deserialize)]
struct Welcome {
    token: String,
    device: DeviceInfo,
    /// The other device's link card (M46.47); older versions send none.
    #[serde(default)]
    link: Option<crate::device_link::LinkCard>,
}

/// Why a request to another device failed.
enum Failure {
    /// It forgot this device: ask again.
    Unpaired,
    Other(String),
}

impl Failure {
    fn message(self, name: &str) -> String {
        match self {
            Self::Unpaired => format!("{name} doesn't know this device anymore"),
            Self::Other(message) => message,
        }
    }
}

struct Session {
    device: Known,
    generation: u64,
    state: Option<PlaybackState>,
    /// When `state` arrived: where a playing song is now follows from it.
    state_at: Instant,
    /// The queue as the device sent it: portable sources, artwork at its source address.
    queue: Option<QueueSnapshot>,
    /// The same with artwork through this device's cover cache, for the UI.
    shown_queue: Option<QueueSnapshot>,
    sender: tauri::async_runtime::JoinHandle<()>,
    follower: tauri::async_runtime::JoinHandle<()>,
}

#[derive(Default)]
struct Inner {
    /// Devices this one was allowed on, or added by address; kept device-local.
    known: Vec<Known>,
    /// Heard on the network or reached this run, by id.
    found: HashMap<String, Known>,
    session: Option<Session>,
    asking: Option<String>,
    error: Option<String>,
    /// Bumped by each session, so a finished one's tasks can't touch the next.
    generation: u64,
}

pub struct ConnectService {
    app: AppHandle,
    http: reqwest::Client,
    /// Its name changes when the device is renamed in Devices.
    me: Mutex<Identity>,
    inner: Mutex<Inner>,
    browsing: AtomicBool,
    /// Questions to this device's user, by id.
    questions: Mutex<HashMap<String, oneshot::Sender<bool>>>,
}

impl ConnectService {
    pub async fn start(app: AppHandle) -> Arc<Self> {
        let db = app.state::<AppDb>().inner().0.clone();
        let known = raygun_db::local_value(&db, DEVICES_KEY)
            .await
            .ok()
            .flatten()
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default();
        let me = Identity::load(&db).await;
        let http = raygun_net::client_builder()
            .connect_timeout(Duration::from_secs(5))
            .build()
            .unwrap_or_default();
        Arc::new(Self {
            app,
            http,
            me: Mutex::new(me),
            inner: Mutex::new(Inner {
                known,
                ..Inner::default()
            }),
            browsing: AtomicBool::new(false),
            questions: Mutex::new(HashMap::new()),
        })
    }

    pub fn me(&self) -> Identity {
        self.me.lock().unwrap_or_else(|p| p.into_inner()).clone()
    }

    /// This device's new name (renamed in Devices): the status, questions to other devices and
    /// link cards use it from now on.
    pub fn rename(&self, name: &str) {
        self.me.lock().unwrap_or_else(|p| p.into_inner()).name = name.to_owned();
        self.emit();
    }

    pub fn status(&self) -> ConnectStatus {
        let inner = self.lock();
        ConnectStatus {
            this_device: self.me().name,
            session: inner.session.as_ref().map(|session| ConnectSession {
                device_id: session.device.id.clone(),
                device_name: session.device.name.clone(),
                device_kind: session.device.kind.clone(),
                state: session.state.clone(),
                queue: session.shown_queue.clone(),
            }),
            asking: inner.asking.clone(),
            error: inner.error.clone(),
        }
    }

    /// Other devices: heard ones first, then those known from before.
    pub fn devices(self: &Arc<Self>) -> Vec<ConnectDevice> {
        self.browse();
        let inner = self.lock();
        let paired = |id: &str| {
            inner
                .known
                .iter()
                .any(|known| known.id == id && known.token.is_some())
        };
        let mut devices: Vec<ConnectDevice> = inner
            .found
            .values()
            .map(|device| view(device, paired(&device.id), true))
            .collect();
        devices.extend(
            inner
                .known
                .iter()
                .filter(|known| !inner.found.contains_key(&known.id))
                .map(|known| view(known, known.token.is_some(), false)),
        );
        devices.sort_by(|a, b| {
            b.found
                .cmp(&a.found)
                .then_with(|| a.name.to_lowercase().cmp(&b.name.to_lowercase()))
        });
        devices
    }

    /// Adds a device by its address (`192.168.1.20:8820`), for networks where mDNS doesn't reach.
    pub async fn add(&self, address: &str) -> Result<ConnectDevice, String> {
        let address = normalize_address(address)?;
        self.check_network(&address).await?;
        let response = self
            .http
            .get(format!("http://{address}/api/device"))
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|_| format!("Nothing answered at {address}. Is Raygun open there, with \"Let other devices play here\" on?"))?;
        let info: DeviceInfo = response
            .json()
            .await
            .map_err(|_| format!("{address} isn't a Raygun device"))?;
        if info.id == self.me().id {
            return Err("That's this device".into());
        }
        let device = Known {
            id: info.id,
            name: info.name,
            kind: kind_or_default(info.kind),
            address,
            token: None,
            sync_id: None,
            granted: None,
        };
        let (paired, known) = {
            let mut inner = self.lock();
            inner.found.insert(device.id.clone(), device.clone());
            let paired = match inner.known.iter_mut().find(|known| known.id == device.id) {
                Some(known) => {
                    known.address = device.address.clone();
                    known.name = device.name.clone();
                    known.token.is_some()
                }
                None => {
                    inner.known.push(device.clone());
                    false
                }
            };
            (paired, inner.known.clone())
        };
        self.save(&known).await;
        Ok(view(&device, paired, true))
    }

    /// Forgets a device: its token, and the address it was added by. A linked device is
    /// unlinked too (M46.47): no more syncing with it, and its token here stops working.
    pub async fn forget(self: &Arc<Self>, id: &str) -> Vec<ConnectDevice> {
        let (gone, known) = {
            let mut inner = self.lock();
            let gone = inner.known.iter().find(|known| known.id == id).cloned();
            inner.known.retain(|known| known.id != id);
            // Still on the network, but not linked anymore.
            if let Some(found) = inner.found.get_mut(id) {
                found.sync_id = None;
                found.granted = None;
            }
            (gone, inner.known.clone())
        };
        self.save(&known).await;
        if let Some(gone) = gone {
            if let Some(sync_id) = &gone.sync_id {
                let _ = self
                    .app
                    .state::<crate::device_sync::SyncService>()
                    .remove_peer(sync_id)
                    .await;
            }
            if let Some(granted) = &gone.granted {
                let _ = self
                    .app
                    .state::<crate::remote::RemoteService>()
                    .forget(granted)
                    .await;
            }
        }
        self.devices()
    }

    /// Links with a device (M46.47): asks it once, then both sync and either plays on the other.
    pub async fn link(self: &Arc<Self>, id: &str) -> Result<Vec<ConnectDevice>, String> {
        let device = {
            let inner = self.lock();
            inner
                .found
                .get(id)
                .or_else(|| inner.known.iter().find(|known| known.id == id))
                .cloned()
        }
        .ok_or("That device isn't around anymore")?;
        self.check_network(&device.address).await?;
        let result = self.hello(device).await;
        let mut inner = self.lock();
        inner.asking = None;
        inner.error = result.as_ref().err().cloned();
        drop(inner);
        self.emit();
        result.map(|_| self.devices())
    }

    /// A device that linked with this one (M46.47, this side of its question): it plays here by
    /// the token `granted` names, and this one plays there with the token it sent.
    pub async fn remember_link(&self, card: &crate::device_link::LinkCard, granted: Option<&str>) {
        let Some(address) = card.connect_address.clone() else {
            return;
        };
        let known = {
            let mut inner = self.lock();
            let granted = granted.map(str::to_owned).or_else(|| {
                inner
                    .known
                    .iter()
                    .find(|known| known.id == card.connect_id)
                    .and_then(|known| known.granted.clone())
            });
            inner.known.retain(|known| known.id != card.connect_id);
            let device = Known {
                id: card.connect_id.clone(),
                name: card.name.clone(),
                kind: kind_or_default(card.kind.clone()),
                address,
                token: card.token.clone(),
                sync_id: Some(card.sync_id.clone()),
                granted,
            };
            inner.found.insert(
                device.id.clone(),
                Known {
                    token: None,
                    ..device.clone()
                },
            );
            inner.known.push(device);
            inner.known.clone()
        };
        self.save(&known).await;
        self.emit();
    }

    /// Plays on another device: asks it first if it never allowed this one, moves the queue and
    /// the position there, pauses here, and from then on sends it everything this player would do.
    pub async fn connect(self: &Arc<Self>, id: &str) -> Result<ConnectStatus, String> {
        let device = {
            let inner = self.lock();
            if inner
                .session
                .as_ref()
                .is_some_and(|session| session.device.id == id)
            {
                drop(inner);
                return Ok(self.status());
            }
            let token = inner
                .known
                .iter()
                .find(|known| known.id == id)
                .and_then(|known| known.token.clone());
            inner
                .found
                .get(id)
                .or_else(|| inner.known.iter().find(|known| known.id == id))
                .cloned()
                .map(|device| Known { token, ..device })
        }
        .ok_or("That device isn't around anymore")?;
        self.check_network(&device.address).await?;

        let result = self.take_over(device.clone()).await;
        let mut inner = self.lock();
        inner.asking = None;
        inner.error = result.as_ref().err().cloned();
        drop(inner);
        self.emit();
        result.map(|()| self.status())
    }

    async fn take_over(self: &Arc<Self>, device: Known) -> Result<(), String> {
        let mut device = match device.token {
            Some(_) => device,
            None => self.hello(device).await?,
        };
        // What plays now, here or on the device played on so far, goes over.
        let (queue, state, since) = {
            let inner = self.lock();
            match &inner.session {
                Some(session) => (
                    session.queue.clone().unwrap_or(QueueSnapshot {
                        items: Vec::new(),
                        order: Vec::new(),
                        context: None,
                    }),
                    session.state.clone(),
                    session.state_at.elapsed(),
                ),
                None => {
                    let playback = self.app.state::<PlaybackService>();
                    (playback.queue(), Some(playback.state()), Duration::ZERO)
                }
            }
        };
        let images = self.app.state::<ImageLinks>();
        let transfer = state
            .as_ref()
            .and_then(|state| transfer(&queue, state, since, &|url| images.original(url)));
        if let Some(command) = &transfer {
            match self.send(&device, command).await {
                Err(Failure::Unpaired) => {
                    // It forgot this device: ask again, once.
                    device = self
                        .hello(Known {
                            token: None,
                            ..device
                        })
                        .await?;
                    self.send(&device, command)
                        .await
                        .map_err(|failure| failure.message(&device.name))?;
                }
                result => result.map_err(|failure| failure.message(&device.name))?,
            }
            if let Some(state) = &state
                && state.repeat != RepeatMode::Off
            {
                let repeat = ConnectCommand::Control {
                    control: PlaybackControl::SetRepeat { mode: state.repeat },
                };
                let _ = self.send(&device, &repeat).await;
            }
        }
        // The player that was playing stops: the other device, or this one.
        if let Some(previous) = self.end_session() {
            let pause = ConnectCommand::Control {
                control: PlaybackControl::Pause,
            };
            let _ = self.send(&previous.device, &pause).await;
        } else if transfer.is_some() {
            self.app
                .state::<PlaybackService>()
                .control_here(PlaybackControl::Pause);
        }
        self.begin(device);
        Ok(())
    }

    /// Asks a device whether this one may play there; its user answers on it.
    async fn hello(&self, device: Known) -> Result<Known, String> {
        self.lock().asking = Some(device.id.clone());
        self.emit();
        // The link card (M46.47): the other device syncs with this one and may play here too.
        let card = crate::device_link::card_for(&self.app, &device.name, true).await;
        let (card, granted) = card.unzip();
        let response = self
            .http
            .post(format!("http://{}/api/connect/hello", device.address))
            .json(&json!({ "name": self.me().name, "link": card }))
            .timeout(HELLO_TIMEOUT)
            .send()
            .await
            .map_err(|_| format!("Couldn't reach {}", device.name))?;
        if !response.status().is_success() {
            return Err(error_message(response, &device.name).await);
        }
        let welcome: Welcome = response.json().await.map_err(|_| {
            format!(
                "{} answered something Raygun doesn't understand",
                device.name
            )
        })?;
        let sync_id = match &welcome.link {
            Some(link) => {
                let sync = self.app.state::<crate::device_sync::SyncService>();
                match sync.link_peer(link).await {
                    Ok(()) => {
                        let app = self.app.clone();
                        let peer = link.sync_id.clone();
                        tauri::async_runtime::spawn(async move {
                            let sync = app.state::<crate::device_sync::SyncService>();
                            let _ = sync.sync_now(Some(&peer)).await;
                        });
                        Some(link.sync_id.clone())
                    }
                    Err(error) => {
                        tracing::warn!(%error, "couldn't keep the linked device for sync");
                        None
                    }
                }
            }
            None => None,
        };
        let device = Known {
            id: welcome.device.id,
            name: welcome.device.name,
            kind: kind_or_default(welcome.device.kind),
            address: device.address,
            token: Some(welcome.token),
            sync_id,
            granted: granted.flatten(),
        };
        let known = {
            let mut inner = self.lock();
            inner.known.retain(|known| known.id != device.id);
            inner.known.push(device.clone());
            inner.found.insert(
                device.id.clone(),
                Known {
                    token: None,
                    ..device.clone()
                },
            );
            inner.known.clone()
        };
        self.save(&known).await;
        Ok(device)
    }

    /// Starts sending this player's changes to `device` and following its player.
    fn begin(self: &Arc<Self>, device: Known) {
        let (commands, receiver) = mpsc::unbounded_channel();
        let generation = {
            let mut inner = self.lock();
            inner.generation += 1;
            inner.generation
        };
        let sender = tauri::async_runtime::spawn(Self::send_all(
            Arc::downgrade(self),
            device.clone(),
            generation,
            receiver,
        ));
        let follower = tauri::async_runtime::spawn(Self::follow(
            Arc::downgrade(self),
            device.clone(),
            generation,
        ));
        tracing::info!(device = %device.name, "playing on another device");
        self.lock().session = Some(Session {
            device,
            generation,
            state: None,
            state_at: Instant::now(),
            queue: None,
            shown_queue: None,
            sender,
            follower,
        });
        self.app
            .state::<PlaybackService>()
            .set_redirect(Some(Arc::new(move |command| {
                let _ = commands.send(command);
            })));
    }

    /// Ends the session, if any, and gives this device's player back its controls.
    fn end_session(&self) -> Option<Session> {
        let session = self.lock().session.take()?;
        session.sender.abort();
        session.follower.abort();
        self.app.state::<PlaybackService>().set_redirect(None);
        Some(session)
    }

    /// Stops playing on the other device. With `play_here`, its queue and position come back
    /// and it pauses; without, it keeps playing on its own.
    pub async fn disconnect(&self, play_here: bool) -> ConnectStatus {
        if let Some(session) = self.end_session()
            && play_here
        {
            let images = self.app.state::<ImageLinks>();
            let empty = QueueSnapshot {
                items: Vec::new(),
                order: Vec::new(),
                context: None,
            };
            let command = session.state.as_ref().and_then(|state| {
                transfer(
                    session.queue.as_ref().unwrap_or(&empty),
                    state,
                    session.state_at.elapsed(),
                    &|url| images.original(url),
                )
            });
            let pause = ConnectCommand::Control {
                control: PlaybackControl::Pause,
            };
            let _ = self.send(&session.device, &pause).await;
            if let Some(command) = command {
                let playback = self.app.state::<PlaybackService>();
                playback.apply(arrived(command, &images));
                if let Some(state) = &session.state {
                    playback.control_here(PlaybackControl::SetRepeat { mode: state.repeat });
                }
            }
        }
        self.lock().error = None;
        self.emit();
        self.status()
    }

    /// Sends this player's changes to the device, one after another.
    async fn send_all(
        service: Weak<Self>,
        device: Known,
        generation: u64,
        mut commands: mpsc::UnboundedReceiver<ConnectCommand>,
    ) {
        while let Some(command) = commands.recv().await {
            let Some(service) = service.upgrade() else {
                return;
            };
            let command = {
                let images = service.app.state::<ImageLinks>();
                portable_command(command, &|url| images.original(url))
            };
            if let Err(failure) = service.send(&device, &command).await {
                let message = failure.message(&device.name);
                tracing::warn!(%message, "a change didn't reach the other device");
                let mut inner = service.lock();
                if inner.generation == generation {
                    inner.error = Some(message);
                    drop(inner);
                    service.emit();
                }
            }
        }
    }

    /// Follows the other device's player; gives up after a few tries once its events stop.
    async fn follow(service: Weak<Self>, device: Known, generation: u64) {
        let mut tries = 0;
        loop {
            let Some(strong) = service.upgrade() else {
                return;
            };
            let ended = strong.events(&device, generation).await;
            if matches!(ended, Err(Failure::Unpaired)) {
                break;
            }
            tries += 1;
            if tries > RECONNECTS {
                break;
            }
            drop(strong);
            tokio::time::sleep(Duration::from_secs(1 << tries)).await;
        }
        let Some(service) = service.upgrade() else {
            return;
        };
        let current = service
            .lock()
            .session
            .as_ref()
            .is_some_and(|session| session.generation == generation);
        if current {
            tracing::warn!(device = %device.name, "lost the device played on");
            if let Some(session) = service.lock().session.take() {
                session.sender.abort();
            }
            service.app.state::<PlaybackService>().set_redirect(None);
            service.lock().error = Some(format!("Lost {}", device.name));
            service.emit();
        }
    }

    /// One connection to the device's events, until it ends.
    async fn events(&self, device: &Known, generation: u64) -> Result<(), Failure> {
        let token = device.token.as_deref().unwrap_or_default();
        let response = self
            .http
            .get(format!(
                "http://{}/api/connect/events?token={token}",
                device.address
            ))
            .send()
            .await
            .map_err(|error| Failure::Other(error.to_string()))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            return Err(Failure::Unpaired);
        }
        if !response.status().is_success() {
            return Err(Failure::Other(response.status().to_string()));
        }
        let mut body = response.bytes_stream();
        let mut buffer = String::new();
        while let Some(chunk) = body.next().await {
            let chunk = chunk.map_err(|error| Failure::Other(error.to_string()))?;
            buffer.push_str(&String::from_utf8_lossy(&chunk));
            while let Some(end) = buffer.find("\n\n") {
                let block: String = buffer.drain(..end + 2).collect();
                if let Some((event, data)) = parse_event(&block) {
                    self.received(generation, &event, &data);
                }
            }
        }
        Ok(())
    }

    fn received(&self, generation: u64, event: &str, data: &str) {
        let mut inner = self.lock();
        let Some(session) = inner
            .session
            .as_mut()
            .filter(|session| session.generation == generation)
        else {
            return;
        };
        match event {
            "state" => {
                let Ok(state) = serde_json::from_str::<PlaybackState>(data) else {
                    return;
                };
                session.state = Some(state);
                session.state_at = Instant::now();
            }
            "queue" => {
                let Ok(queue) = serde_json::from_str::<QueueSnapshot>(data) else {
                    return;
                };
                let images = self.app.state::<ImageLinks>();
                session.shown_queue = Some(QueueSnapshot {
                    items: queue
                        .items
                        .iter()
                        .cloned()
                        .map(|item| QueueItem {
                            artwork_url: item.artwork_url.map(|url| shown_artwork(&url, &images)),
                            ..item
                        })
                        .collect(),
                    order: queue.order.clone(),
                    context: queue.context.clone(),
                });
                session.queue = Some(queue);
            }
            _ => return,
        }
        drop(inner);
        self.emit();
    }

    async fn send(&self, device: &Known, command: &ConnectCommand) -> Result<(), Failure> {
        let response = self
            .http
            .post(format!("http://{}/api/connect/command", device.address))
            .bearer_auth(device.token.as_deref().unwrap_or_default())
            .json(command)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|_| Failure::Other(format!("Couldn't reach {}", device.name)))?;
        if response.status() == reqwest::StatusCode::UNAUTHORIZED {
            let known = {
                let mut inner = self.lock();
                for known in inner.known.iter_mut().filter(|known| known.id == device.id) {
                    known.token = None;
                }
                inner.known.clone()
            };
            self.save(&known).await;
            return Err(Failure::Unpaired);
        }
        if !response.status().is_success() {
            return Err(Failure::Other(error_message(response, &device.name).await));
        }
        Ok(())
    }

    // ---- This device, asked by others ----

    /// Asks this device's user whether `name` may play here; no answer in time is a no. On
    /// desktop the window comes forward, since the question is the reason someone is looking.
    pub async fn ask(&self, name: &str) -> bool {
        let id = uuid::Uuid::new_v4().simple().to_string();
        let (answer, answered) = oneshot::channel();
        self.questions
            .lock()
            .expect("questions poisoned")
            .insert(id.clone(), answer);
        let question = |open| {
            ConnectRequestEvent(ConnectRequest {
                id: id.clone(),
                name: name.to_owned(),
                open,
            })
        };
        if let Err(error) = question(true).emit(&self.app) {
            tracing::warn!(%error, "failed to show a Connect request");
        }
        #[cfg(not(any(target_os = "android", target_os = "ios")))]
        if let Some(window) = self.app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
        let allowed = tokio::time::timeout(ASK_TIMEOUT, answered)
            .await
            .ok()
            .and_then(Result::ok)
            .unwrap_or(false);
        self.questions
            .lock()
            .expect("questions poisoned")
            .remove(&id);
        let _ = question(false).emit(&self.app);
        allowed
    }

    pub fn answer(&self, id: &str, allow: bool) {
        if let Some(answer) = self
            .questions
            .lock()
            .expect("questions poisoned")
            .remove(id)
        {
            let _ = answer.send(allow);
        }
    }

    // ---- Helpers ----

    /// Starts listening for devices the first time they're asked for. Android only lets an app
    /// hear multicast while it holds a lock.
    fn browse(self: &Arc<Self>) {
        if self.browsing.swap(true, Ordering::SeqCst) {
            return;
        }
        #[cfg(target_os = "android")]
        if let Err(error) = self
            .app
            .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
            .multicast(true)
        {
            tracing::warn!(%error, "no multicast lock; devices may not be heard");
        }
        let heard = Arc::downgrade(self);
        let gone = Arc::downgrade(self);
        let result = raygun_remote::discovery::browse(
            self.me().id,
            move |found| {
                if let Some(service) = heard.upgrade() {
                    let device = Known {
                        id: found.id,
                        name: found.name,
                        kind: kind_or_default(found.kind),
                        address: found.address,
                        token: None,
                        sync_id: None,
                        granted: None,
                    };
                    service.lock().found.insert(device.id.clone(), device);
                }
            },
            move |id| {
                if let Some(service) = gone.upgrade() {
                    service.lock().found.remove(&id);
                }
            },
        );
        if let Err(error) = result {
            tracing::warn!(%error, "couldn't listen for devices");
        }
    }

    async fn check_network(&self, address: &str) -> Result<(), String> {
        self.app
            .state::<LocalNetwork>()
            .check(&format!("http://{address}/"), CONNECT_DENIED)
            .await
    }

    async fn save(&self, known: &[Known]) {
        let db = &self.app.state::<AppDb>().inner().0;
        let json = serde_json::to_string(known).unwrap_or_default();
        if let Err(error) = raygun_db::set_local_value(db, DEVICES_KEY, &json).await {
            tracing::warn!(%error, "failed to keep Connect devices");
        }
    }

    fn emit(&self) {
        if let Err(error) = ConnectEvent(self.status()).emit(&self.app) {
            tracing::warn!(%error, "failed to emit Connect status");
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("connect poisoned")
    }
}

fn view(device: &Known, paired: bool, found: bool) -> ConnectDevice {
    ConnectDevice {
        id: device.id.clone(),
        name: device.name.clone(),
        address: device.address.clone(),
        kind: device.kind.clone(),
        paired,
        found,
        sync_id: device.sync_id.clone(),
        last_sync_at: None,
    }
}

fn kind_or_default(kind: String) -> String {
    if kind == "phone" {
        kind
    } else {
        "computer".into()
    }
}

/// `host:port` from what someone typed: a scheme, a path or a missing port are fine.
pub(crate) fn normalize_address(text: &str) -> Result<String, String> {
    let text = text.trim();
    let text = text
        .strip_prefix("http://")
        .or_else(|| text.strip_prefix("https://"))
        .unwrap_or(text);
    let host_port = text.split(['/', '?', '#']).next().unwrap_or_default();
    if host_port.is_empty()
        || !host_port
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '.' | '-' | ':'))
    {
        return Err("Type the address another device shows, like 192.168.1.20:8820".into());
    }
    match host_port.rsplit_once(':') {
        Some((host, port)) if !host.is_empty() && port.parse::<u16>().is_ok() => {
            Ok(host_port.to_owned())
        }
        Some(_) => Err("That port isn't a number".into()),
        None => Ok(format!("{host_port}:{}", raygun_remote::PORTS.start())),
    }
}

/// The `(event, data)` of one server-sent event block.
fn parse_event(block: &str) -> Option<(String, String)> {
    let mut event = String::from("message");
    let mut data = Vec::new();
    for line in block.lines() {
        if let Some(name) = line.strip_prefix("event:") {
            event = name.trim().to_owned();
        } else if let Some(text) = line.strip_prefix("data:") {
            data.push(text.strip_prefix(' ').unwrap_or(text));
        }
    }
    (!data.is_empty()).then(|| (event, data.join("\n")))
}

async fn error_message(response: reqwest::Response, name: &str) -> String {
    let status = response.status();
    response
        .json::<serde_json::Value>()
        .await
        .ok()
        .and_then(|body| body.get("error")?.as_str().map(str::to_owned))
        .unwrap_or_else(|| format!("{name} answered {status}"))
}

/// The queue and position a device moves to another: the queue in its own order, the song
/// playing, where it is now, and whether it plays.
fn transfer(
    queue: &QueueSnapshot,
    state: &PlaybackState,
    since: Duration,
    original: &dyn Fn(&str) -> String,
) -> Option<ConnectCommand> {
    if queue.items.is_empty() {
        return None;
    }
    let start = state
        .current_item_id
        .as_ref()
        .and_then(|id| queue.items.iter().position(|item| &item.id == id));
    let playing = matches!(
        state.status,
        PlaybackStatus::Playing | PlaybackStatus::Loading | PlaybackStatus::Buffering
    );
    let moved = if state.status == PlaybackStatus::Playing {
        since.as_millis() as f64
    } else {
        0.0
    };
    let position = match state.status {
        PlaybackStatus::Ended => 0.0,
        _ => state.position_ms + moved,
    };
    Some(ConnectCommand::Replace {
        items: queue
            .items
            .iter()
            .cloned()
            .map(|item| portable(input(item), original))
            .collect(),
        start_index: start.map(|index| index as u32),
        shuffle: state.shuffle,
        start_ms: position,
        play: playing,
        context: queue.context.clone().map(|context| PlayContext {
            artwork_url: context.artwork_url.as_deref().map(original),
            ..context
        }),
    })
}

fn input(item: QueueItem) -> QueueItemInput {
    QueueItemInput {
        explicit: item.explicit,
        title: item.title,
        artist: item.artist,
        album: item.album,
        duration_ms: item.duration_ms,
        artwork_url: item.artwork_url,
        media_kind: item.media_kind,
        source: item.source,
    }
}

/// A queue item another device can play: a file or a link only this device reaches goes as a
/// lookup by its title and artist, and artwork as its source address.
pub(crate) fn portable(item: QueueItemInput, original: &dyn Fn(&str) -> String) -> QueueItemInput {
    let source = match &item.source {
        MediaSource::LocalFile { .. } => lookup(&item),
        MediaSource::Url { url, .. } if !portable_url(url) => lookup(&item),
        source => source.clone(),
    };
    let artwork_url = item
        .artwork_url
        .as_deref()
        .map(original)
        .filter(|url| portable_url(url));
    QueueItemInput {
        source,
        artwork_url,
        ..item
    }
}

/// A queue item as another device gets it, with its id kept (Connect's events).
pub(crate) fn portable_queue_item(item: QueueItem, original: &dyn Fn(&str) -> String) -> QueueItem {
    let id = item.id.clone();
    let input = portable(input(item), original);
    QueueItem {
        explicit: input.explicit,
        id,
        title: input.title,
        artist: input.artist,
        album: input.album,
        duration_ms: input.duration_ms,
        artwork_url: input.artwork_url,
        media_kind: input.media_kind,
        source: input.source,
    }
}

fn lookup(item: &QueueItemInput) -> MediaSource {
    MediaSource::Lookup {
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms,
    }
}

pub(crate) fn portable_command(
    command: ConnectCommand,
    original: &dyn Fn(&str) -> String,
) -> ConnectCommand {
    map_items(command, |item| portable(item, original))
}

/// A command from another device, with its artwork through this device's cover cache.
pub(crate) fn arrived(command: ConnectCommand, images: &ImageLinks) -> ConnectCommand {
    map_items(command, |item| QueueItemInput {
        artwork_url: item.artwork_url.map(|url| shown_artwork(&url, images)),
        ..item
    })
}

fn shown_artwork(url: &str, images: &ImageLinks) -> String {
    if images.is_remote(url) {
        images.cover(url, &Default::default())
    } else {
        url.to_owned()
    }
}

fn map_items(
    command: ConnectCommand,
    map: impl Fn(QueueItemInput) -> QueueItemInput,
) -> ConnectCommand {
    let all = |items: Vec<QueueItemInput>| items.into_iter().map(&map).collect();
    match command {
        ConnectCommand::Replace {
            items,
            start_index,
            shuffle,
            start_ms,
            play,
            context,
        } => ConnectCommand::Replace {
            items: all(items),
            start_index,
            shuffle,
            start_ms,
            play,
            context,
        },
        ConnectCommand::Enqueue { items, play_now } => ConnectCommand::Enqueue {
            items: all(items),
            play_now,
        },
        ConnectCommand::EnqueueNext { items } => ConnectCommand::EnqueueNext { items: all(items) },
        control @ ConnectCommand::Control { .. } => control,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn addresses_are_host_and_port() {
        assert_eq!(
            normalize_address(" http://192.168.1.20:8821/ ").unwrap(),
            "192.168.1.20:8821"
        );
        assert_eq!(normalize_address("10.0.2.2").unwrap(), "10.0.2.2:8820");
        assert_eq!(
            normalize_address("living-room.local:8822").unwrap(),
            "living-room.local:8822"
        );
        assert!(normalize_address("").is_err());
        assert!(normalize_address("pc:port").is_err());
        assert!(normalize_address("a b").is_err());
    }

    #[test]
    fn events_are_read_from_blocks() {
        assert_eq!(
            parse_event("event: state\ndata: {\"a\":1}\n\n"),
            Some(("state".into(), "{\"a\":1}".into()))
        );
        assert_eq!(parse_event(": keep-alive\n\n"), None);
    }

    fn item(id: &str, source: MediaSource) -> QueueItem {
        QueueItem {
            explicit: false,
            id: id.into(),
            title: format!("Song {id}"),
            artist: Some("Artist".into()),
            album: None,
            duration_ms: Some(200_000),
            artwork_url: Some("https://covers.example/a.jpg".into()),
            media_kind: raygun_model::MediaKind::Audio,
            source,
        }
    }

    #[test]
    fn the_queue_moves_with_portable_sources_and_the_position() {
        let same = |url: &str| url.to_owned();
        let queue = QueueSnapshot {
            items: vec![
                item(
                    "a",
                    MediaSource::LocalFile {
                        path: "C:/Music/a.flac".into(),
                    },
                ),
                item(
                    "b",
                    MediaSource::Url {
                        url: "http://127.0.0.1:4000/local/b.mp3".into(),
                        headers: Default::default(),
                    },
                ),
                item(
                    "c",
                    MediaSource::YouTube {
                        video_id: "abc".into(),
                    },
                ),
            ],
            order: vec![0, 1, 2],
            context: Some(PlayContext {
                kind: "album".into(),
                name: "Album".into(),
                subtitle: None,
                artwork_url: Some("http://127.0.0.1:4000/cover/a".into()),
                to: "/music/album".into(),
                search: Default::default(),
                played_at: 0.0,
            }),
        };
        let state = PlaybackState {
            status: PlaybackStatus::Playing,
            current_item_id: Some("b".into()),
            position_ms: 10_000.0,
            duration_ms: Some(200_000.0),
            volume: 1.0,
            repeat: RepeatMode::Off,
            shuffle: true,
            error: None,
            source_label: None,
            rate: 1.0,
            sleep_at: None,
            stop_after_current: false,
        };
        let Some(ConnectCommand::Replace {
            items,
            start_index,
            shuffle,
            start_ms,
            play,
            context,
        }) = transfer(&queue, &state, Duration::from_millis(1_500), &same)
        else {
            panic!("a queue moves");
        };
        assert_eq!(
            context.map(|context| context.name).as_deref(),
            Some("Album"),
            "where it was played from goes along"
        );
        assert_eq!(start_index, Some(1));
        assert!(shuffle && play);
        assert_eq!(start_ms, 11_500.0, "where the song is now");
        assert!(
            matches!(items[0].source, MediaSource::Lookup { .. }),
            "a file"
        );
        assert!(
            matches!(items[1].source, MediaSource::Lookup { .. }),
            "a local link"
        );
        assert!(matches!(items[2].source, MediaSource::YouTube { .. }));
        assert_eq!(
            items[0].artwork_url.as_deref(),
            Some("https://covers.example/a.jpg")
        );

        let paused = PlaybackState {
            status: PlaybackStatus::Paused,
            ..state
        };
        let Some(ConnectCommand::Replace { start_ms, play, .. }) =
            transfer(&queue, &paused, Duration::from_secs(9), &same)
        else {
            panic!("a queue moves");
        };
        assert_eq!(start_ms, 10_000.0, "a paused song hasn't moved");
        assert!(!play);
        let empty = QueueSnapshot {
            items: Vec::new(),
            order: Vec::new(),
            context: None,
        };
        assert!(
            transfer(&empty, &paused, Duration::ZERO, &same).is_none(),
            "nothing to move"
        );
    }
}
