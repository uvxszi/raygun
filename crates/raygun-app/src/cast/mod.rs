//! Play on TV: the player's video sent to a TV or another device on the network.
//!
//! - Finding devices, in one list: Google Cast devices advertise `_googlecast._tcp` over mDNS
//!   (the daemon of `raygun_remote::discovery`); DLNA renderers and DIAL devices answer SSDP
//!   searches ([`ssdp`]). A TV can also be added by its address.
//! - Playing: Google Cast through the Cast v2 protocol ([`googlecast`]), DLNA through
//!   AVTransport and RenderingControl ([`dlna`]). [`CastKind::AirPlay`] is in the model for the
//!   iOS app; nothing here finds or plays on AirPlay devices.
//! - The stream: a TV can't reach the loopback proxy, so each cast opens the proxy's route on the
//!   local network ([`raygun_net::LanRoute`]) at this device's address on the TV's network. It adds
//!   the stream's headers, rewrites playlists, and serves subtitles as WebVTT (Cast) or SubRip
//!   (DLNA).
//! - While connected, what the player opens or loads goes to the TV ([`CastService::follow`]).
//!   Progress goes into the player's report queue as if the player had reported it, so Continue
//!   watching, the library's episodes and sessions follow the TV; the player's own reports are
//!   left out meanwhile. Disconnect resumes the player from where the TV was.

mod dlna;
mod googlecast;
mod ssdp;

use std::collections::{BTreeMap, HashSet};
use std::net::{IpAddr, SocketAddr};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::{Duration, Instant};

use raygun_model::{CastControl, CastDevice, CastKind, CastState, CastStatus};
use raygun_net::{LanLinks, LanRoute, SubtitleFormat};
use serde_json::{Value, json};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::mpsc;

use crate::catalog::now_ms;
use crate::local_network::LocalNetwork;

/// The device couldn't play the video.
pub(crate) const TV_FAILED: &str = "The TV couldn't play this video";
/// The device stopped answering.
pub(crate) const LOST: &str = "Lost the connection to the TV";
const CAST_DENIED: &str = "Allow local network access to play on a TV";
/// Description addresses of TVs added by address (`local_value`).
const ADDED_KEY: &str = "cast.added";
/// While devices are asked for, the network is searched again this often.
const SEARCH_EVERY: Duration = Duration::from_secs(12);
const SEARCH_WAIT: Duration = Duration::from_secs(3);
/// A device found by SSDP that missed more searches than this is dropped.
const MISSES_LIMIT: u8 = 2;
/// Progress is saved this often while the TV plays.
const REPORT_EVERY_MS: f64 = 5_000.0;
/// The player hears of the moving position at least this often; changes go at once.
const EMIT_EVERY: Duration = Duration::from_secs(10);

/// What a device is asked to play.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct CastMedia {
    /// The video's progress key.
    pub key: String,
    /// Its address on the cast route.
    pub url: String,
    pub content_type: String,
    pub title: String,
    pub subtitle: Option<String>,
    /// A web address the TV can fetch.
    pub poster: Option<String>,
    pub tracks: Vec<Track>,
    /// The track shown, by index into `tracks`.
    pub active_track: Option<usize>,
    pub start_ms: f64,
}

/// A subtitle file on the cast route.
#[derive(Debug, Clone, PartialEq)]
pub(crate) struct Track {
    pub url: String,
    pub language: Option<String>,
    pub label: String,
}

pub(crate) enum Command {
    Control(CastControl),
    Load(Box<CastMedia>),
    Disconnect,
}

/// What a device said; fields it didn't mention are `None`.
#[derive(Debug, Clone, Default, PartialEq)]
pub(crate) struct Report {
    pub state: Option<CastState>,
    pub position_ms: Option<f64>,
    pub duration_ms: Option<f64>,
    pub volume: Option<f64>,
    pub muted: Option<bool>,
    pub error: Option<String>,
    /// The video it's about, when the device says: reports about the one before are dropped.
    pub key: Option<String>,
}

pub(crate) type Reporter = Arc<dyn Fn(Report) + Send + Sync>;

/// How a device is reached.
#[derive(Debug, Clone, PartialEq)]
enum Target {
    GoogleCast(SocketAddr),
    Dlna(dlna::Renderer),
}

#[derive(Debug, Clone)]
struct Known {
    device: CastDevice,
    target: Target,
    ip: IpAddr,
    /// Its mDNS name, when heard that way.
    mdns: Option<String>,
    /// Its description, for DLNA TVs.
    location: Option<String>,
    /// Searches missed in a row, for devices found by SSDP.
    misses: Option<u8>,
}

struct Session {
    generation: u64,
    /// The device, kept to reconnect even once it drops out of the list.
    known: Known,
    /// The player's request for the video on the TV.
    payload: Value,
    subtitle: Option<usize>,
    /// Open while the device's runner is.
    route: Option<Arc<LanRoute>>,
    commands: mpsc::UnboundedSender<Command>,
    status: CastStatus,
    reported_at: f64,
    emitted_at: Instant,
}

#[derive(Default)]
struct Inner {
    devices: BTreeMap<String, Known>,
    added: Vec<String>,
    added_loaded: bool,
    searched_at: Option<Instant>,
    searching: bool,
    /// The player's video (its request), for Play on TV.
    playing: Option<Value>,
    session: Option<Session>,
}

pub struct CastService {
    app: AppHandle,
    http: reqwest::Client,
    inner: Mutex<Inner>,
    browsing: AtomicBool,
    generations: AtomicU64,
    /// What the Android notification that keeps Raygun running says (device, title).
    kept_alive: Mutex<Option<(String, String)>>,
}

impl CastService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        let http = raygun_net::client_builder()
            .connect_timeout(Duration::from_secs(4))
            .build()
            .unwrap_or_default();
        Arc::new(Self {
            app,
            http,
            inner: Mutex::new(Inner::default()),
            browsing: AtomicBool::new(false),
            generations: AtomicU64::new(0),
            kept_alive: Mutex::new(None),
        })
    }

    fn lock(&self) -> MutexGuard<'_, Inner> {
        self.inner.lock().expect("cast state poisoned")
    }

    // ---- Devices ----

    /// The devices found so far; asking starts the search, and repeats it now and then.
    pub fn devices(self: &Arc<Self>) -> Vec<CastDevice> {
        self.search();
        self.listed()
    }

    /// Whether a search of the network is under way.
    pub fn searching(&self) -> bool {
        self.lock().searching
    }

    fn listed(&self) -> Vec<CastDevice> {
        let mut devices: Vec<CastDevice> = self
            .lock()
            .devices
            .values()
            .map(|known| known.device.clone())
            .collect();
        devices.sort_by_cached_key(|device| (device.name.to_lowercase(), device.id.clone()));
        devices
    }

    fn search(self: &Arc<Self>) {
        self.browse();
        let due = {
            let mut inner = self.lock();
            let due = !inner.searching
                && inner
                    .searched_at
                    .is_none_or(|at| at.elapsed() >= SEARCH_EVERY);
            if due {
                inner.searching = true;
                inner.searched_at = Some(Instant::now());
            }
            due
        };
        if !due {
            return;
        }
        self.devices_changed();
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            service.load_added().await;
            let answers = ssdp::search(&[ssdp::MEDIA_RENDERER, ssdp::DIAL], SEARCH_WAIT).await;
            let found = service.describe_all(answers, false).await;
            service.found_round(found);
        });
    }

    /// Starts listening for Cast devices the first time devices are asked for. Android only lets
    /// an app hear multicast while it holds a lock (Connect holds it too).
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
            tracing::warn!(%error, "no multicast lock; Cast devices may not be heard");
        }
        let heard = Arc::downgrade(self);
        let gone = Arc::downgrade(self);
        let result = raygun_remote::discovery::browse_service(
            googlecast::SERVICE,
            move |found| {
                if let Some(service) = heard.upgrade() {
                    service.heard(&found);
                }
            },
            move |fullname| {
                if let Some(service) = gone.upgrade() {
                    service.lost(&fullname);
                }
            },
        );
        if let Err(error) = result {
            tracing::warn!(%error, "couldn't listen for Cast devices");
            self.browsing.store(false, Ordering::SeqCst);
        }
    }

    fn heard(&self, heard: &raygun_remote::discovery::Heard) {
        let Some(known) = google_cast_device(heard) else {
            return;
        };
        let changed = {
            let mut inner = self.lock();
            // The same device answered DIAL first: the mDNS entry stands for it.
            inner.devices.retain(|id, other| {
                *id == known.device.id
                    || !(other.device.kind == CastKind::GoogleCast
                        && other.mdns.is_none()
                        && other.ip == known.ip)
            });
            let same = inner
                .devices
                .get(&known.device.id)
                .is_some_and(|old| old.device == known.device && old.target == known.target);
            if !same {
                inner.devices.insert(known.device.id.clone(), known);
            }
            !same
        };
        if changed {
            self.devices_changed();
        }
    }

    fn lost(&self, fullname: &str) {
        let before = self.lock().devices.len();
        self.lock()
            .devices
            .retain(|_, known| known.mdns.as_deref() != Some(fullname));
        if self.lock().devices.len() != before {
            self.devices_changed();
        }
    }

    /// Reads the descriptions of SSDP answers, at once; what can play a video is kept.
    async fn describe_all(&self, answers: Vec<(ssdp::Answer, IpAddr)>, added: bool) -> Vec<Known> {
        let looks = answers.into_iter().map(|(answer, from)| async move {
            self.classify(&answer.location, from, added).await
        });
        futures_util::future::join_all(looks)
            .await
            .into_iter()
            .flatten()
            .collect()
    }

    /// A device from its description: a DLNA renderer, or a Google device answering DIAL.
    async fn classify(&self, location: &str, from: IpAddr, added: bool) -> Option<Known> {
        let (description, dial) = match ssdp::describe(&self.http, location).await {
            Ok(found) => found,
            Err(error) => {
                tracing::debug!(%error, %location, "no description");
                return None;
            }
        };
        let name = |fallback: &str| {
            Some(description.name.clone())
                .filter(|name| !name.is_empty())
                .or_else(|| description.model.clone())
                .unwrap_or_else(|| fallback.to_owned())
        };
        if let Some(av_transport) = description.av_transport.clone() {
            let address = url::Url::parse(location)
                .ok()
                .and_then(|url| {
                    Some(format!(
                        "{}:{}",
                        url.host_str()?,
                        url.port_or_known_default()?
                    ))
                })
                .unwrap_or_else(|| from.to_string());
            let udn = if description.udn.is_empty() {
                location.to_owned()
            } else {
                description.udn.clone()
            };
            return Some(Known {
                device: CastDevice {
                    id: format!("dlna:{udn}"),
                    name: name("TV"),
                    kind: CastKind::Dlna,
                    model: description.model.clone(),
                    address,
                    added,
                },
                target: Target::Dlna(dlna::Renderer {
                    av_transport,
                    rendering_control: description.rendering_control.clone(),
                }),
                ip: from,
                mdns: None,
                location: Some(location.to_owned()),
                misses: (!added).then_some(0),
            });
        }
        let cast = SocketAddr::new(from, googlecast::PORT);
        if dial && (description.is_google() || port_open(cast).await) {
            return Some(Known {
                device: CastDevice {
                    id: format!("googlecast:{from}"),
                    name: name("Chromecast"),
                    kind: CastKind::GoogleCast,
                    model: description.model.clone(),
                    address: cast.to_string(),
                    added,
                },
                target: Target::GoogleCast(cast),
                ip: from,
                mdns: None,
                location: None,
                misses: (!added).then_some(0),
            });
        }
        None
    }

    fn found_round(&self, found: Vec<Known>) {
        {
            let mut inner = self.lock();
            let ids: HashSet<String> = found.iter().map(|known| known.device.id.clone()).collect();
            for known in found {
                // A Cast device heard by mDNS is already there under its own id.
                let heard = known.device.kind == CastKind::GoogleCast
                    && inner.devices.values().any(|other| {
                        other.device.kind == CastKind::GoogleCast
                            && other.mdns.is_some()
                            && other.ip == known.ip
                    });
                if heard {
                    continue;
                }
                match inner.devices.get_mut(&known.device.id) {
                    // Added by address: it stays, whatever the searches find.
                    Some(old) if old.device.added => {}
                    _ => {
                        inner.devices.insert(known.device.id.clone(), known);
                    }
                }
            }
            inner
                .devices
                .retain(|id, known| match known.misses.as_mut() {
                    Some(misses) if !ids.contains(id) => {
                        *misses += 1;
                        *misses <= MISSES_LIMIT
                    }
                    Some(misses) => {
                        *misses = 0;
                        true
                    }
                    None => true,
                });
            inner.searching = false;
        }
        self.devices_changed();
    }

    /// TVs added by address in earlier runs, described again once.
    async fn load_added(&self) {
        {
            let mut inner = self.lock();
            if inner.added_loaded {
                return;
            }
            inner.added_loaded = true;
        }
        let db = self.app.state::<crate::startup::AppDb>().0.clone();
        let locations: Vec<String> = match raygun_db::local_value(&db, ADDED_KEY).await {
            Ok(Some(saved)) => serde_json::from_str(&saved).unwrap_or_default(),
            _ => Vec::new(),
        };
        self.lock().added = locations.clone();
        for location in locations {
            let Some(ip) = host_ip(&location).await else {
                continue;
            };
            if let Some(known) = self.classify(&location, ip, true).await {
                self.lock().devices.insert(known.device.id.clone(), known);
            }
        }
    }

    /// Adds a TV by its address: an IP address or name with an optional port, or the address of
    /// its description.
    pub async fn add(self: &Arc<Self>, address: &str) -> Result<CastDevice, String> {
        let address = address.trim();
        if address.is_empty() {
            return Err("Type the TV's address".into());
        }
        let not_found =
            || format!("No TV answered at {address}. Check the address and that the TV is on");
        let probe_url = if address.contains("://") {
            address.to_owned()
        } else {
            format!("http://{address}/")
        };
        self.app
            .state::<LocalNetwork>()
            .check(&probe_url, CAST_DENIED)
            .await?;
        let known = if address.starts_with("http://") || address.starts_with("https://") {
            let ip = host_ip(address).await.ok_or_else(not_found)?;
            self.classify(address, ip, true)
                .await
                .ok_or_else(not_found)?
        } else {
            let (host, port) = split_host_port(address).ok_or_else(not_found)?;
            let ip = lookup(&host).await.ok_or_else(not_found)?;
            let answers = ssdp::search_at(
                ip,
                &[ssdp::MEDIA_RENDERER, ssdp::DIAL],
                Duration::from_secs(2),
            )
            .await;
            let mut found = self.describe_all(answers, true).await;
            found.sort_by_key(|known| known.device.kind != CastKind::Dlna);
            match found.into_iter().next() {
                Some(known) => known,
                None => {
                    // No answer to SSDP: a Cast device is still reached on its own port.
                    let cast = SocketAddr::new(ip, port.unwrap_or(googlecast::PORT));
                    if port.is_some_and(|port| port != googlecast::PORT) || !port_open(cast).await {
                        return Err(not_found());
                    }
                    Known {
                        device: CastDevice {
                            id: format!("googlecast:{ip}"),
                            name: host.clone(),
                            kind: CastKind::GoogleCast,
                            model: None,
                            address: cast.to_string(),
                            added: true,
                        },
                        target: Target::GoogleCast(cast),
                        ip,
                        mdns: None,
                        location: None,
                        misses: None,
                    }
                }
            }
        };
        let device = known.device.clone();
        let saved = {
            let mut inner = self.lock();
            let location = known.location.clone();
            inner.devices.insert(device.id.clone(), known);
            match location {
                Some(location) if !inner.added.contains(&location) => {
                    inner.added.push(location);
                    Some(inner.added.clone())
                }
                _ => None,
            }
        };
        if let Some(added) = saved {
            let db = self.app.state::<crate::startup::AppDb>().0.clone();
            let text = serde_json::to_string(&added).unwrap_or_default();
            if let Err(error) = raygun_db::set_local_value(&db, ADDED_KEY, &text).await {
                tracing::warn!(%error, "couldn't remember the TV");
            }
        }
        self.devices_changed();
        Ok(device)
    }

    /// The device list for the Android player while its sheet is open.
    pub fn devices_changed(&self) {
        #[cfg(target_os = "android")]
        {
            let update = json!({
                "type": "castDevices",
                "devices": self.listed(),
                "searching": self.searching(),
            });
            let _ = crate::video::update_player(&self.app, &update);
        }
    }

    // ---- Playing ----

    /// Remembers what the player plays (its request), for Play on TV. While connected, the video
    /// goes to the TV instead: the status is returned, and the player shows it.
    pub fn follow(self: &Arc<Self>, payload: &Value) -> Option<CastStatus> {
        let url = payload
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if url.is_empty() {
            return self.status();
        }
        let (commands, route, kind, start_ms, status) = {
            let mut inner = self.lock();
            inner.playing = Some(payload.clone());
            let session = inner.session.as_mut()?;
            let same = session.payload.get("id") == payload.get("id")
                && session.payload.get("url") == payload.get("url");
            if same {
                return Some(session.status.clone());
            }
            session.payload = payload.clone();
            session.subtitle = None;
            let start_ms = payload
                .get("startMs")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let status = &mut session.status;
            status.key = payload["id"].as_str().unwrap_or_default().to_owned();
            status.title = payload["title"].as_str().unwrap_or_default().to_owned();
            status.subtitle = payload["subtitle"].as_str().map(str::to_owned);
            // With the device's runner gone (stopped on the TV), Play starts this video there.
            status.state = if session.route.is_some() {
                CastState::Loading
            } else {
                CastState::Stopped
            };
            status.position_ms = start_ms;
            status.duration_ms = None;
            status.error = None;
            status.at_ms = now_ms() as f64;
            session.reported_at = now_ms() as f64;
            (
                session.commands.clone(),
                session.route.clone(),
                status.device.kind,
                start_ms,
                status.clone(),
            )
        };
        self.publish(Some(status.clone()));
        let Some(route) = route else {
            return Some(status);
        };
        let service = Arc::downgrade(self);
        let http = self.http.clone();
        let payload = payload.clone();
        tauri::async_runtime::spawn(async move {
            match prepare(&http, &route, &payload, start_ms, None, kind).await {
                Ok(media) => {
                    let _ = commands.send(Command::Load(Box::new(media)));
                }
                Err(error) => {
                    if let Some(service) = service.upgrade() {
                        let generation = service
                            .lock()
                            .session
                            .as_ref()
                            .map(|session| session.generation);
                        if let Some(generation) = generation {
                            service.on_report(
                                generation,
                                Report {
                                    state: Some(CastState::Failed),
                                    error: Some(error),
                                    ..Report::default()
                                },
                            );
                        }
                    }
                }
            }
        });
        Some(status)
    }

    /// What plays on a device, if anything.
    pub fn status(&self) -> Option<CastStatus> {
        self.lock()
            .session
            .as_ref()
            .map(|session| session.status.clone())
    }

    /// Whether a device plays the player's videos (the player's own reports are left out).
    pub fn connected(&self) -> bool {
        self.lock().session.is_some()
    }

    /// Plays the player's video on `device_id` from `position_ms`, with the subtitle file at
    /// `subtitle` (an index into the video's subtitles) on.
    pub async fn start(
        self: &Arc<Self>,
        device_id: &str,
        position_ms: f64,
        subtitle: Option<usize>,
    ) -> Result<(), String> {
        let (known, payload) = {
            let inner = self.lock();
            let known = inner
                .devices
                .get(device_id)
                .cloned()
                .ok_or("That device isn't on the network any more")?;
            let payload = inner
                .playing
                .clone()
                .ok_or("Nothing is playing to send to the TV")?;
            (known, payload)
        };
        self.start_with(known, payload, position_ms, subtitle).await
    }

    async fn start_with(
        self: &Arc<Self>,
        known: Known,
        payload: Value,
        position_ms: f64,
        subtitle: Option<usize>,
    ) -> Result<(), String> {
        if known.device.kind == CastKind::AirPlay {
            return Err("AirPlay isn't available on this device".into());
        }
        self.app
            .state::<LocalNetwork>()
            .check(
                &format!("http://{}/", SocketAddr::new(known.ip, 80)),
                CAST_DENIED,
            )
            .await?;
        let lan_ip = local_ip_towards(known.ip).ok_or("This device isn't on the TV's network")?;
        let links = self
            .app
            .try_state::<LanLinks>()
            .ok_or("Play on TV isn't available")?;
        let route = Arc::new(links.open(lan_ip).await.map_err(|error| {
            tracing::warn!(%error, %lan_ip, "couldn't open the cast route");
            "Couldn't open a connection for the TV".to_owned()
        })?);
        let media = prepare(
            &self.http,
            &route,
            &payload,
            position_ms,
            subtitle,
            known.device.kind,
        )
        .await?;
        let generation = self.generations.fetch_add(1, Ordering::SeqCst) + 1;
        let (commands, receiver) = mpsc::unbounded_channel();
        let status = CastStatus {
            device: known.device.clone(),
            key: media.key.clone(),
            title: media.title.clone(),
            subtitle: media.subtitle.clone(),
            state: CastState::Connecting,
            position_ms: media.start_ms,
            duration_ms: None,
            volume: None,
            muted: false,
            at_ms: now_ms() as f64,
            error: None,
        };
        let target = known.target.clone();
        // What played before (another device, or this one stopped) ends here; a start that
        // failed above left it as it was.
        let previous = self.lock().session.replace(Session {
            generation,
            known,
            payload,
            subtitle,
            route: Some(route),
            commands,
            status: status.clone(),
            reported_at: now_ms() as f64,
            emitted_at: Instant::now(),
        });
        if let Some(previous) = previous {
            let _ = previous.commands.send(Command::Disconnect);
        }
        self.publish(Some(status));
        let service = Arc::downgrade(self);
        let http = self.http.clone();
        tauri::async_runtime::spawn(async move {
            let reporter: Reporter = {
                let service = service.clone();
                Arc::new(move |report| {
                    if let Some(service) = service.upgrade() {
                        service.on_report(generation, report);
                    }
                })
            };
            let result = match target {
                Target::GoogleCast(address) => {
                    googlecast::run(address, media, receiver, reporter.clone()).await
                }
                Target::Dlna(renderer) => {
                    dlna::run(
                        dlna::Client::new(http, renderer),
                        media,
                        receiver,
                        reporter.clone(),
                    )
                    .await
                }
            };
            if let Err(error) = result {
                reporter(Report {
                    state: Some(CastState::Failed),
                    error: Some(error),
                    ..Report::default()
                });
            }
            if let Some(service) = service.upgrade() {
                service.runner_ended(generation);
            }
        });
        Ok(())
    }

    /// The device's runner is gone: the route closes, and Play starts a new one.
    fn runner_ended(&self, generation: u64) {
        if let Some(session) = self
            .lock()
            .session
            .as_mut()
            .filter(|session| session.generation == generation)
        {
            session.route = None;
        }
    }

    pub async fn control(self: &Arc<Self>, control: CastControl) -> Result<(), String> {
        let restart = {
            let inner = self.lock();
            let session = inner.session.as_ref().ok_or("Nothing is playing on a TV")?;
            if session.route.is_some()
                && session
                    .commands
                    .send(Command::Control(control.clone()))
                    .is_ok()
            {
                return Ok(());
            }
            (
                session.known.clone(),
                session.payload.clone(),
                live_position(&session.status, now_ms() as f64),
                session.subtitle,
            )
        };
        match control {
            CastControl::Play => {
                let (known, payload, position, subtitle) = restart;
                self.start_with(known, payload, position, subtitle).await
            }
            _ => Err("The TV isn't connected. Press Play to connect again".into()),
        }
    }

    /// Stops playing on the device. With `resume`, the player goes on from where the TV was.
    pub async fn disconnect(&self, resume: bool) -> Result<(), String> {
        let Some(session) = self.lock().session.take() else {
            return Ok(());
        };
        let _ = session.commands.send(Command::Disconnect);
        let position = live_position(&session.status, now_ms() as f64);
        if !matches!(
            session.status.state,
            CastState::Connecting | CastState::Failed
        ) {
            self.save_progress(
                &session.status.key,
                "progress",
                position,
                session.status.duration_ms,
            );
        }
        self.publish(None);
        if resume {
            let mut request = session.payload.clone();
            let finished = session.status.state == CastState::Finished;
            request["startMs"] = json!(if finished { 0.0 } else { position });
            crate::video::update_player(&self.app, &json!({ "type": "load", "request": request }))?;
        }
        Ok(())
    }

    fn on_report(self: &Arc<Self>, generation: u64, report: Report) {
        let mut progress = None;
        let mut failed = None;
        let mut publish = None;
        {
            let mut inner = self.lock();
            let Some(session) = inner
                .session
                .as_mut()
                .filter(|session| session.generation == generation)
            else {
                return;
            };
            if report
                .key
                .as_ref()
                .is_some_and(|key| *key != session.status.key)
            {
                return;
            }
            let now = now_ms() as f64;
            let before = session.status.clone();
            let status = &mut session.status;
            if let Some(state) = report.state {
                status.state = state;
            }
            match report.position_ms {
                Some(position) => {
                    status.position_ms = position;
                    status.at_ms = now;
                }
                // Paused or stopped without a position: where it was getting to.
                None if before.state == CastState::Playing
                    && status.state != CastState::Playing =>
                {
                    status.position_ms = live_position(&before, now);
                    status.at_ms = now;
                }
                None => {}
            }
            if report.duration_ms.is_some() {
                status.duration_ms = report.duration_ms;
            }
            if report.volume.is_some() {
                status.volume = report.volume;
            }
            if let Some(muted) = report.muted {
                status.muted = muted;
            }
            // A failed button stays noted until the state changes.
            status.error = report.error.clone().or_else(|| {
                (status.state == before.state)
                    .then(|| before.error.clone())
                    .flatten()
            });
            let finished =
                status.state == CastState::Finished && before.state != CastState::Finished;
            let halted = matches!(status.state, CastState::Paused | CastState::Stopped)
                && status.state != before.state;
            let due =
                status.state == CastState::Playing && now - session.reported_at >= REPORT_EVERY_MS;
            if finished || halted || due {
                session.reported_at = now;
                progress = Some((
                    status.key.clone(),
                    if finished { "ended" } else { "progress" },
                    status.position_ms,
                    status.duration_ms,
                ));
            }
            if status.state == CastState::Failed
                && before.state != CastState::Failed
                && status.error.as_deref() == Some(TV_FAILED)
            {
                failed = Some((status.key.clone(), status.position_ms));
            }
            let changed = status.state != before.state
                || status.volume != before.volume
                || status.muted != before.muted
                || status.duration_ms != before.duration_ms
                || status.error != before.error;
            let drifted = (status.position_ms - live_position(&before, now)).abs() > 2_000.0;
            if changed || drifted || session.emitted_at.elapsed() >= EMIT_EVERY {
                session.emitted_at = Instant::now();
                publish = Some(status.clone());
            }
        }
        if let Some((key, state, position, duration)) = progress {
            self.save_progress(&key, state, position, duration);
        }
        if let Some((key, position)) = failed {
            // A session tries its next source, which then plays on the TV too.
            self.send_report(json!({
                "id": key, "state": "error", "positionMs": position, "durationMs": null,
                "error": TV_FAILED, "looping": false, "cast": true,
            }));
        }
        if let Some(status) = publish {
            self.publish(Some(status));
        }
    }

    /// Saves where the TV is, through the player's report queue.
    fn save_progress(&self, key: &str, state: &str, position_ms: f64, duration_ms: Option<f64>) {
        if key.is_empty() {
            return;
        }
        self.send_report(json!({
            "id": key,
            "state": state,
            "positionMs": position_ms.max(0.0),
            "durationMs": duration_ms,
            "error": null,
            "looping": false,
            "cast": true,
        }));
    }

    fn send_report(&self, report: Value) {
        if let Some(reports) = self.app.try_state::<crate::video::VideoReports>() {
            reports.send(report.to_string());
        }
    }

    fn publish(&self, status: Option<CastStatus>) {
        self.keep_alive(status.as_ref());
        #[cfg(target_os = "android")]
        {
            let _ = crate::video::update_player(
                &self.app,
                &json!({ "type": "cast", "status": status }),
            );
        }
        let _ = crate::events::CastStatusEvent(status).emit(&self.app);
    }

    /// On Android, a foreground service keeps Raygun (and the stream the TV reads) running while
    /// a TV plays, also when the player is left; its notification names the TV and the video.
    fn keep_alive(&self, status: Option<&CastStatus>) {
        let wanted = status.map(|status| (status.device.name.clone(), status.title.clone()));
        {
            let mut shown = self.kept_alive.lock().expect("keep-alive poisoned");
            if *shown == wanted {
                return;
            }
            shown.clone_from(&wanted);
        }
        #[cfg(target_os = "android")]
        {
            let plugin = self
                .app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>();
            let result = match wanted {
                Some((device, title)) => {
                    let labels = self
                        .app
                        .state::<crate::video_session::VideoSessions>()
                        .labels();
                    let heading = labels
                        .get("castPlayingOn")
                        .and_then(Value::as_str)
                        .unwrap_or("Playing on {n}")
                        .replace("{n}", &device);
                    plugin.cast_keep_alive(true, &heading, &title)
                }
                None => plugin.cast_keep_alive(false, "", ""),
            };
            if let Err(error) = result {
                tracing::warn!(%error, "couldn't keep Raygun running for the TV");
            }
        }
        #[cfg(not(target_os = "android"))]
        let _ = wanted;
    }
}

/// The player's requests about casting (the Android player has no other way to ask).
pub async fn on_request(app: &AppHandle, action: &str, raw: &Value) {
    let Some(cast) = app.try_state::<Arc<CastService>>() else {
        return;
    };
    let cast = cast.inner().clone();
    let result = match action {
        "castDevices" => {
            cast.devices();
            cast.devices_changed();
            Ok(())
        }
        "castAdd" => cast
            .add(raw["address"].as_str().unwrap_or_default())
            .await
            .map(drop),
        "castStart" => {
            cast.start(
                raw["device"].as_str().unwrap_or_default(),
                raw["positionMs"].as_f64().unwrap_or(0.0),
                raw["subtitle"].as_u64().map(|index| index as usize),
            )
            .await
        }
        "castControl" => match serde_json::from_value::<CastControl>(raw["control"].clone()) {
            Ok(control) => cast.control(control).await,
            Err(_) => Ok(()),
        },
        "castDisconnect" => {
            cast.disconnect(raw["resume"].as_bool().unwrap_or(true))
                .await
        }
        _ => Ok(()),
    };
    if let Err(error) = result {
        let _ = crate::video::update_player(app, &json!({ "type": "castError", "text": error }));
    }
}

/// Where the video is now: it moves on from the last report while playing.
fn live_position(status: &CastStatus, now_ms: f64) -> f64 {
    let moved = if status.state == CastState::Playing {
        (now_ms - status.at_ms).max(0.0)
    } else {
        0.0
    };
    let position = status.position_ms + moved;
    status
        .duration_ms
        .map_or(position, |duration| position.min(duration))
}

/// A Cast device heard over mDNS, unless it has no screen (a speaker or a speaker group).
fn google_cast_device(heard: &raygun_remote::discovery::Heard) -> Option<Known> {
    let property = |key: &str| {
        heard
            .properties
            .get(key)
            .map(|value| value.trim())
            .filter(|value| !value.is_empty())
    };
    if !googlecast::has_screen(property("ca")) {
        return None;
    }
    let id = property("id").unwrap_or(heard.fullname.as_str()).to_owned();
    let model = property("md").map(str::to_owned);
    let name = property("fn")
        .map(str::to_owned)
        .or_else(|| model.clone())
        .unwrap_or_else(|| "Chromecast".to_owned());
    let address = SocketAddr::new(IpAddr::V4(heard.ip), heard.port);
    Some(Known {
        device: CastDevice {
            id: format!("googlecast:{id}"),
            name,
            kind: CastKind::GoogleCast,
            model,
            address: address.to_string(),
            added: false,
        },
        target: Target::GoogleCast(address),
        ip: address.ip(),
        mdns: Some(heard.fullname.clone()),
        location: None,
        misses: None,
    })
}

/// This device's address on the network that reaches `target` (nothing is sent).
fn local_ip_towards(target: IpAddr) -> Option<IpAddr> {
    let bind: SocketAddr = match target {
        IpAddr::V4(_) => (std::net::Ipv4Addr::UNSPECIFIED, 0).into(),
        IpAddr::V6(_) => (std::net::Ipv6Addr::UNSPECIFIED, 0).into(),
    };
    let socket = std::net::UdpSocket::bind(bind).ok()?;
    socket.connect((target, 9)).ok()?;
    socket
        .local_addr()
        .ok()
        .map(|address| address.ip())
        .filter(|ip| !ip.is_unspecified())
}

/// Whether something listens at `address` (a Cast device's port).
async fn port_open(address: SocketAddr) -> bool {
    tokio::time::timeout(
        Duration::from_millis(800),
        tokio::net::TcpStream::connect(address),
    )
    .await
    .is_ok_and(|connected| connected.is_ok())
}

/// `192.168.1.20`, `192.168.1.20:9197`, `tv.local`, `[fe80::1]:8009` as a host and port.
fn split_host_port(address: &str) -> Option<(String, Option<u16>)> {
    let address = address.trim().trim_end_matches('/');
    if let Some(rest) = address.strip_prefix('[') {
        let (host, rest) = rest.split_once(']')?;
        let port = match rest.strip_prefix(':') {
            Some(port) => Some(port.parse().ok()?),
            None if rest.is_empty() => None,
            None => return None,
        };
        return Some((host.to_owned(), port));
    }
    if address.parse::<std::net::Ipv6Addr>().is_ok() {
        return Some((address.to_owned(), None));
    }
    let (host, port) = match address.rsplit_once(':') {
        Some((host, port)) => (host, Some(port.parse::<u16>().ok()?)),
        None => (address, None),
    };
    let plain = !host.is_empty()
        && host
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, '.' | '-' | '_'));
    plain.then(|| (host.to_owned(), port))
}

async fn lookup(host: &str) -> Option<IpAddr> {
    if let Ok(ip) = host.parse::<IpAddr>() {
        return Some(ip);
    }
    let found = tokio::time::timeout(Duration::from_secs(4), tokio::net::lookup_host((host, 0)))
        .await
        .ok()?
        .ok()?;
    let mut addresses: Vec<SocketAddr> = found.collect();
    addresses.sort_by_key(|address| !address.is_ipv4());
    addresses.first().map(SocketAddr::ip)
}

/// The IP address of a URL's host.
async fn host_ip(url: &str) -> Option<IpAddr> {
    let host = url::Url::parse(url).ok()?.host_str()?.to_owned();
    lookup(host.trim_start_matches('[').trim_end_matches(']')).await
}

/// The player's request as the device gets it: the stream on the cast route with its type,
/// title, poster and subtitles. The stream is tried through the route first, so a link that
/// doesn't work says so here rather than as the TV's silence.
async fn prepare(
    http: &reqwest::Client,
    route: &LanRoute,
    payload: &Value,
    start_ms: f64,
    subtitle: Option<usize>,
    kind: CastKind,
) -> Result<CastMedia, String> {
    let url = payload
        .get("url")
        .and_then(Value::as_str)
        .filter(|url| !url.is_empty())
        .ok_or("Nothing is playing to send to the TV")?;
    let mut headers: BTreeMap<String, String> = payload
        .get("headers")
        .cloned()
        .and_then(|headers| serde_json::from_value(headers).ok())
        .unwrap_or_default();
    // What the source was asked with: some links play only for the same User-Agent.
    if !headers
        .keys()
        .any(|name| name.eq_ignore_ascii_case("user-agent"))
    {
        headers.insert("User-Agent".into(), raygun_net::user_agent());
    }
    let link = route
        .link(url, &headers)
        .ok_or("This video can't be sent to a TV")?;
    let format = payload.get("format").and_then(Value::as_str);
    let content_type = probe(http, &link, format, url, kind).await?;
    let subtitle_format = match kind {
        CastKind::Dlna => SubtitleFormat::SubRip,
        _ => SubtitleFormat::WebVtt,
    };
    let mut tracks = Vec::new();
    let mut active_track = None;
    let files = payload
        .get("subtitles")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    for (index, file) in files.iter().enumerate() {
        // A DLNA TV takes one file: the one chosen.
        if kind == CastKind::Dlna && subtitle != Some(index) {
            continue;
        }
        let Some(address) = file.get("url").and_then(Value::as_str) else {
            continue;
        };
        let Some(tv_url) = route.subtitle(address, &BTreeMap::new(), subtitle_format) else {
            continue;
        };
        if subtitle == Some(index) {
            active_track = Some(tracks.len());
        }
        let language = file.get("lang").and_then(Value::as_str).map(str::to_owned);
        let label = file
            .get("label")
            .and_then(Value::as_str)
            .map(str::to_owned)
            .or_else(|| language.clone())
            .unwrap_or_else(|| format!("Subtitles {}", index + 1));
        tracks.push(Track {
            url: tv_url,
            language,
            label,
        });
    }
    if kind == CastKind::Dlna {
        route.set_caption(tracks.first().map(|track| track.url.clone()));
    }
    let poster = payload
        .get("poster")
        .and_then(Value::as_str)
        .filter(|poster| poster.starts_with("https://") || poster.starts_with("http://"))
        .filter(|poster| !poster.contains("://127.0.0.1") && !poster.contains("://localhost"))
        .map(str::to_owned);
    Ok(CastMedia {
        key: payload["id"].as_str().unwrap_or_default().to_owned(),
        url: link,
        content_type,
        title: payload["title"].as_str().unwrap_or_default().to_owned(),
        subtitle: payload["subtitle"].as_str().map(str::to_owned),
        poster,
        tracks,
        active_track,
        start_ms: start_ms.max(0.0),
    })
}

/// Fetches the first bytes through the route: whether the stream works, and what it is.
async fn probe(
    http: &reqwest::Client,
    link: &str,
    format: Option<&str>,
    original: &str,
    kind: CastKind,
) -> Result<String, String> {
    let response = http
        .get(link)
        .header(reqwest::header::RANGE, "bytes=0-4095")
        .timeout(Duration::from_secs(15))
        .send()
        .await
        .map_err(|error| {
            tracing::warn!(%error, "the stream for the TV didn't answer");
            "Couldn't reach the video's server. Check the connection and try again.".to_owned()
        })?;
    let status = response.status().as_u16();
    match status {
        401 | 403 => {
            return Err("The video's server refused to send it. The link may have expired.".into());
        }
        404 | 410 => return Err("The video isn't on its server any more.".into()),
        500.. => return Err("The video's server had a problem. Try again in a while.".into()),
        _ => {}
    }
    let declared = response
        .headers()
        .get(reqwest::header::CONTENT_TYPE)
        .and_then(|value| value.to_str().ok())
        .map(str::to_owned);
    let mut head = Vec::new();
    let mut body = response.bytes_stream();
    use futures_util::StreamExt;
    while head.len() < 4096 {
        match tokio::time::timeout(Duration::from_secs(10), body.next()).await {
            Ok(Some(Ok(chunk))) => head.extend_from_slice(&chunk),
            _ => break,
        }
    }
    Ok(media_type(
        format,
        original,
        declared.as_deref(),
        &head,
        kind,
    ))
}

/// A stream's type for the device: from its first bytes, the player's format, the server's type
/// or the address. Matroska goes to Cast receivers as WebM, which their player reads.
fn media_type(
    format: Option<&str>,
    url: &str,
    declared: Option<&str>,
    head: &[u8],
    kind: CastKind,
) -> String {
    const HLS: &str = "application/x-mpegurl";
    const DASH: &str = "application/dash+xml";
    let matroska = || match kind {
        CastKind::Dlna => "video/x-matroska",
        _ => "video/webm",
    };
    match raygun_net::manifest::sniff(head) {
        Some(raygun_net::manifest::Manifest::Hls) => return HLS.into(),
        Some(raygun_net::manifest::Manifest::Dash) => return DASH.into(),
        None => {}
    }
    match format.map(str::to_ascii_lowercase).as_deref() {
        Some("hls" | "m3u8") => return HLS.into(),
        Some("dash" | "mpd") => return DASH.into(),
        _ => {}
    }
    if head.len() >= 8 && &head[4..8] == b"ftyp" {
        return "video/mp4".into();
    }
    if head.starts_with(&[0x1a, 0x45, 0xdf, 0xa3]) {
        let webm = head.windows(4).any(|window| window == b"webm");
        return if webm { "video/webm" } else { matroska() }.into();
    }
    if head.first() == Some(&0x47)
        && head.get(188).is_none_or(|byte| *byte == 0x47)
        && head.len() >= 188
    {
        return "video/mp2t".into();
    }
    if let Some(declared) = declared
        .map(|kind| {
            kind.split(';')
                .next()
                .unwrap_or_default()
                .trim()
                .to_ascii_lowercase()
        })
        .filter(|kind| kind.starts_with("video/") || kind.starts_with("audio/"))
    {
        return if declared == "video/x-matroska" {
            matroska().into()
        } else {
            declared
        };
    }
    let path = url
        .split(['?', '#'])
        .next()
        .unwrap_or_default()
        .to_ascii_lowercase();
    let extension = path.rsplit('.').next().unwrap_or_default();
    match extension {
        "m3u8" => HLS,
        "mpd" => DASH,
        "mkv" => matroska(),
        "webm" => "video/webm",
        "mov" => "video/quicktime",
        "avi" => "video/x-msvideo",
        "ts" => "video/mp2t",
        _ => "video/mp4",
    }
    .into()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn status(state: CastState, position_ms: f64, at_ms: f64) -> CastStatus {
        CastStatus {
            device: CastDevice {
                id: "dlna:uuid:1".into(),
                name: "TV".into(),
                kind: CastKind::Dlna,
                model: None,
                address: "192.168.1.20:9197".into(),
                added: false,
            },
            key: "url:a".into(),
            title: "A".into(),
            subtitle: None,
            state,
            position_ms,
            duration_ms: Some(100_000.0),
            volume: None,
            muted: false,
            at_ms,
            error: None,
        }
    }

    #[test]
    fn the_position_moves_on_only_while_playing() {
        assert_eq!(
            live_position(&status(CastState::Playing, 10_000.0, 1_000.0), 4_000.0),
            13_000.0
        );
        assert_eq!(
            live_position(&status(CastState::Paused, 10_000.0, 1_000.0), 4_000.0),
            10_000.0
        );
        assert_eq!(
            live_position(&status(CastState::Playing, 99_000.0, 0.0), 9_000.0),
            100_000.0,
            "never past the end"
        );
    }

    #[test]
    fn stream_types_for_the_tv() {
        let mp4 = b"\x00\x00\x00\x18ftypmp42";
        assert_eq!(
            media_type(None, "https://a/x", None, mp4, CastKind::GoogleCast),
            "video/mp4"
        );
        assert_eq!(
            media_type(
                None,
                "https://a/x",
                Some("text/plain"),
                b"#EXTM3U\n",
                CastKind::Dlna
            ),
            "application/x-mpegurl"
        );
        assert_eq!(
            media_type(Some("dash"), "https://a/x", None, b"", CastKind::GoogleCast),
            "application/dash+xml"
        );
        let mkv = b"\x1a\x45\xdf\xa3\x01\x00\x00\x00matroska";
        assert_eq!(
            media_type(None, "https://a/x", None, mkv, CastKind::Dlna),
            "video/x-matroska"
        );
        assert_eq!(
            media_type(None, "https://a/x", None, mkv, CastKind::GoogleCast),
            "video/webm"
        );
        assert_eq!(
            media_type(
                None,
                "https://a/film.mov?sig=1",
                Some("application/octet-stream"),
                b"",
                CastKind::Dlna
            ),
            "video/quicktime"
        );
        assert_eq!(
            media_type(
                None,
                "https://a/x",
                Some("video/webm; codecs=vp9"),
                b"",
                CastKind::Dlna
            ),
            "video/webm"
        );
        let mut ts = vec![0u8; 376];
        ts[0] = 0x47;
        ts[188] = 0x47;
        assert_eq!(
            media_type(None, "https://a/x", None, &ts, CastKind::Dlna),
            "video/mp2t"
        );
        assert_eq!(
            media_type(None, "https://a/x", None, b"", CastKind::Dlna),
            "video/mp4"
        );
    }

    #[test]
    fn typed_addresses() {
        assert_eq!(
            split_host_port("192.168.1.20"),
            Some(("192.168.1.20".into(), None))
        );
        assert_eq!(
            split_host_port(" 192.168.1.20:9197/ "),
            Some(("192.168.1.20".into(), Some(9197)))
        );
        assert_eq!(split_host_port("tv.local"), Some(("tv.local".into(), None)));
        assert_eq!(
            split_host_port("[fe80::1]:8009"),
            Some(("fe80::1".into(), Some(8009)))
        );
        assert_eq!(split_host_port("fe80::1"), Some(("fe80::1".into(), None)));
        assert_eq!(split_host_port("192.168.1.20:tv"), None);
        assert_eq!(split_host_port("a b"), None);
        assert_eq!(split_host_port(""), None);
    }

    #[test]
    fn cast_devices_from_their_txt_records() {
        let heard = |properties: &[(&str, &str)]| raygun_remote::discovery::Heard {
            fullname: "Chromecast-1._googlecast._tcp.local.".into(),
            ip: std::net::Ipv4Addr::new(192, 168, 1, 30),
            port: 8009,
            properties: properties
                .iter()
                .map(|(key, value)| ((*key).to_owned(), (*value).to_owned()))
                .collect(),
        };
        let known = google_cast_device(&heard(&[
            ("id", "8e1f6c0d"),
            ("fn", "Living Room"),
            ("md", "Chromecast Ultra"),
            ("ca", "4101"),
        ]))
        .unwrap();
        assert_eq!(known.device.id, "googlecast:8e1f6c0d");
        assert_eq!(known.device.name, "Living Room");
        assert_eq!(known.device.model.as_deref(), Some("Chromecast Ultra"));
        assert_eq!(known.device.address, "192.168.1.30:8009");
        assert_eq!(
            known.target,
            Target::GoogleCast("192.168.1.30:8009".parse().unwrap())
        );
        assert!(
            google_cast_device(&heard(&[
                ("id", "x"),
                ("fn", "Kitchen speaker"),
                ("ca", "2052")
            ]))
            .is_none()
        );
        let unnamed = google_cast_device(&heard(&[("md", "Google TV")])).unwrap();
        assert_eq!(unnamed.device.name, "Google TV");
    }

    #[test]
    fn the_route_is_opened_on_the_address_that_reaches_the_tv() {
        assert_eq!(
            local_ip_towards(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST)),
            Some(IpAddr::V4(std::net::Ipv4Addr::LOCALHOST))
        );
    }

    /// Lists what's on this network: `cargo test -p raygun-app -- --ignored cast_discovery_live --nocapture`.
    #[tokio::test]
    #[ignore = "searches the local network"]
    async fn cast_discovery_live() {
        let heard = Arc::new(Mutex::new(Vec::new()));
        let found = heard.clone();
        raygun_remote::discovery::browse_service(
            googlecast::SERVICE,
            move |service| found.lock().unwrap().push(service),
            |_| {},
        )
        .unwrap();
        let answers =
            ssdp::search(&[ssdp::MEDIA_RENDERER, ssdp::DIAL], Duration::from_secs(4)).await;
        let http = raygun_net::http_client().unwrap();
        for (answer, from) in &answers {
            match ssdp::describe(&http, &answer.location).await {
                Ok((description, dial)) => println!(
                    "SSDP {from}: {} ({:?}, {}) renderer={} dial={dial}",
                    description.name,
                    description.model,
                    description.device_type,
                    description.is_renderer()
                ),
                Err(error) => println!("SSDP {from}: {} unreadable: {error}", answer.location),
            }
        }
        for service in heard.lock().unwrap().iter() {
            let device = google_cast_device(service);
            println!(
                "mDNS {}:{} {:?} -> {:?}",
                service.ip,
                service.port,
                service.properties,
                device.map(|known| known.device)
            );
        }
    }
}
