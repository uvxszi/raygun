//! The server other Raygun devices on the network link with and play through (ARCHITECTURE §9;
//! Connect, M46.15; one link, M46.47). Off unless devices may find this one.
//!
//! A device asks once (this device's user allows it; a link QR code's one-time code comes
//! along when it was scanned), then sends the queue and controls and follows the player by
//! events.
//!
//! What the API does lives behind [`Backend`], implemented by the app; this crate is the server,
//! pairing and auth.

pub mod discovery;
pub mod pairing;

use std::convert::Infallible;
use std::future::Future;
use std::net::{IpAddr, Ipv4Addr, UdpSocket};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use axum::Json;
use axum::Router;
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use futures_util::Stream;
use serde::Deserialize;
use serde_json::{Value, json};
use tokio::net::TcpListener;
use tokio::sync::oneshot;

pub use pairing::{PairError, PairedDevice, Pairing};

/// Ports tried in order; the first free one is used.
pub const PORTS: std::ops::RangeInclusive<u16> = 8820..=8829;
/// How often Connect's stream checks: another Raygun shows this player as its own.
const CONNECT_INTERVAL: Duration = Duration::from_millis(300);
/// Longest name a device asking to play here may give.
const MAX_ASKER_NAME: usize = 40;
const KEEP_ALIVE: Duration = Duration::from_secs(15);

/// What linked devices can see and do here.
pub trait Backend: Send + Sync + 'static {
    /// This device as Connect shows it (M46.15): `{ id, name, kind }`.
    fn device(&self) -> Value;
    /// The player's state and queue for another Raygun: `(PlaybackState, QueueSnapshot)`.
    fn connect_snapshot(&self) -> (Value, Value);
    /// Runs a `ConnectCommand` from a paired Raygun.
    fn connect_command(&self, command: &Value) -> Result<(), String>;
    /// Asks this device's user whether `name` may play here: false when they say no or don't
    /// answer in time.
    fn ask(&self, name: &str) -> impl Future<Output = bool> + Send;
    /// The allowed device's link card, if it sent one (M46.47: sync and Connect both ways),
    /// with the id of the token it was just given; answers with this device's card.
    fn link(
        &self,
        card: Option<&Value>,
        granted: &str,
    ) -> impl Future<Output = Option<Value>> + Send {
        let _ = (card, granted);
        async { None }
    }
}

/// Pairing state shared with the app, plus how to save it after the server changes it.
#[derive(Clone)]
pub struct Access {
    pairing: Arc<Mutex<Pairing>>,
    on_change: Arc<dyn Fn(Vec<PairedDevice>) + Send + Sync>,
    clock: Arc<dyn Fn() -> i64 + Send + Sync>,
}

impl Access {
    pub fn new(
        pairing: Arc<Mutex<Pairing>>,
        on_change: impl Fn(Vec<PairedDevice>) + Send + Sync + 'static,
        clock: impl Fn() -> i64 + Send + Sync + 'static,
    ) -> Self {
        Self {
            pairing,
            on_change: Arc::new(on_change),
            clock: Arc::new(clock),
        }
    }

    fn authorize(&self, token: &str) -> bool {
        self.pairing
            .lock()
            .expect("pairing lock poisoned")
            .authorize(token)
            .is_some()
    }
}

struct Shared<B> {
    backend: B,
    access: Access,
    /// Held while this device's user is asked, so only one question is open.
    asking: tokio::sync::Mutex<()>,
}

pub fn router<B: Backend>(backend: B, access: Access) -> Router {
    Router::new()
        .route("/api/device", get(device::<B>))
        .route("/api/connect/hello", post(hello::<B>))
        .route("/api/connect/events", get(connect_events::<B>))
        .route("/api/connect/command", post(connect_command::<B>))
        .with_state(Arc::new(Shared {
            backend,
            access,
            asking: tokio::sync::Mutex::new(()),
        }))
}

/// A running server; dropping it (or calling [`Running::stop`]) shuts it down.
pub struct Running {
    pub port: u16,
    stop: Option<oneshot::Sender<()>>,
}

impl Running {
    pub fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

impl Drop for Running {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

/// Binds the first free port in [`PORTS`] on every interface and serves until stopped.
pub async fn serve(router: Router) -> std::io::Result<Running> {
    serve_on(router, Ipv4Addr::UNSPECIFIED).await
}

async fn serve_on(router: Router, ip: Ipv4Addr) -> std::io::Result<Running> {
    let mut last_error = None;
    for port in PORTS {
        match TcpListener::bind((ip, port)).await {
            Ok(listener) => {
                let (stop, stopped) = oneshot::channel::<()>();
                tokio::spawn(async move {
                    let shutdown = async {
                        let _ = stopped.await;
                    };
                    if let Err(error) = axum::serve(listener, router)
                        .with_graceful_shutdown(shutdown)
                        .await
                    {
                        tracing::warn!(%error, "remote server stopped with an error");
                    }
                });
                return Ok(Running {
                    port,
                    stop: Some(stop),
                });
            }
            Err(error) => last_error = Some(error),
        }
    }
    Err(last_error.unwrap_or_else(|| std::io::Error::other("no port to try")))
}

/// This device's address on the local network: the interface the default route uses. A UDP
/// "connect" only picks a route; nothing is sent.
pub fn lan_ip() -> Option<IpAddr> {
    let socket = UdpSocket::bind((Ipv4Addr::UNSPECIFIED, 0)).ok()?;
    socket.connect((Ipv4Addr::new(192, 0, 2, 1), 9)).ok()?;
    let ip = socket.local_addr().ok()?.ip();
    usable_lan_ip(ip).then_some(ip)
}

fn usable_lan_ip(ip: IpAddr) -> bool {
    !ip.is_unspecified() && !ip.is_loopback() && !ip.is_multicast()
}

// ---- Handlers ----

#[derive(Deserialize)]
struct EventsQuery {
    #[serde(default)]
    token: String,
}

// ---- Connect (M46.15) ----

async fn device<B: Backend>(State(shared): State<Arc<Shared<B>>>) -> Response {
    Json(shared.backend.device()).into_response()
}

#[derive(Deserialize)]
struct HelloRequest {
    #[serde(default)]
    name: String,
    /// The asking device's link card (M46.47).
    #[serde(default)]
    link: Option<Value>,
    /// The one-time code of this device's link QR code, when the asking device scanned it.
    #[serde(default)]
    code: Option<String>,
}

/// Another Raygun asks to play here: this device's user decides, and an allowed device gets a
/// token as a paired browser does.
async fn hello<B: Backend>(
    State(shared): State<Arc<Shared<B>>>,
    Json(request): Json<HelloRequest>,
) -> Response {
    let name: String = request.name.trim().chars().take(MAX_ASKER_NAME).collect();
    let name = if name.is_empty() {
        "Another device".to_owned()
    } else {
        name
    };
    let Ok(_asking) = shared.asking.try_lock() else {
        return error(
            StatusCode::TOO_MANY_REQUESTS,
            "That device is answering another request. Try again in a minute.",
        );
    };
    // A scanned QR code's code works once, and only while that code is still shown.
    if let Some(code) = request.code.as_deref() {
        let access = &shared.access;
        let now = (access.clock)();
        let redeemed = access
            .pairing
            .lock()
            .expect("pairing lock poisoned")
            .redeem(code, now);
        if let Err(problem) = redeemed {
            return error(
                StatusCode::FORBIDDEN,
                match problem {
                    PairError::NoCode => {
                        "That QR code expired or was used. Show a new one on the other device."
                    }
                    PairError::WrongCode => {
                        "That QR code isn't the one the other device shows now. Scan it again."
                    }
                },
            );
        }
    }
    if !shared.backend.ask(&name).await {
        return error(StatusCode::FORBIDDEN, "That device didn't allow it.");
    }
    let access = &shared.access;
    let now = (access.clock)();
    let (device, devices) = {
        let mut pairing = access.pairing.lock().expect("pairing lock poisoned");
        let device = pairing.admit(&name, now);
        (device, pairing.devices().to_vec())
    };
    tracing::info!(name = %device.name, "a device may play here");
    (access.on_change)(devices);
    let link = shared.backend.link(request.link.as_ref(), &device.id).await;
    Json(json!({
        "token": device.token,
        "deviceId": device.id,
        "device": shared.backend.device(),
        "link": link,
    }))
    .into_response()
}

async fn connect_events<B: Backend>(
    State(shared): State<Arc<Shared<B>>>,
    Query(query): Query<EventsQuery>,
) -> Response {
    if let Err(refusal) = check(&shared.access, &query.token) {
        return refusal;
    }
    Sse::new(connect_stream(shared, query.token))
        .keep_alive(KeepAlive::new().interval(KEEP_ALIVE))
        .into_response()
}

/// The queue and the state, each sent at once and then whenever it changes; the queue only when
/// it does, since it can be long. Ends when the device loses access.
fn connect_stream<B: Backend>(
    shared: Arc<Shared<B>>,
    token: String,
) -> impl Stream<Item = Result<Event, Infallible>> {
    struct Sent {
        state: Option<String>,
        queue: Option<String>,
        /// Something went out last time, so look again before waiting.
        busy: bool,
    }
    futures_util::stream::unfold(
        (
            shared,
            token,
            Sent {
                state: None,
                queue: None,
                busy: true,
            },
        ),
        |(shared, token, mut sent)| async move {
            loop {
                if !sent.busy {
                    tokio::time::sleep(CONNECT_INTERVAL).await;
                }
                if !shared.access.authorize(&token) {
                    return None;
                }
                let (state, queue) = shared.backend.connect_snapshot();
                let queue = queue.to_string();
                if sent.queue.as_deref() != Some(queue.as_str()) {
                    let event = Event::default().event("queue").data(queue.clone());
                    sent.queue = Some(queue);
                    sent.busy = true;
                    return Some((Ok(event), (shared, token, sent)));
                }
                let state = state.to_string();
                if sent.state.as_deref() != Some(state.as_str()) {
                    let event = Event::default().event("state").data(state.clone());
                    sent.state = Some(state);
                    sent.busy = true;
                    return Some((Ok(event), (shared, token, sent)));
                }
                sent.busy = false;
            }
        },
    )
}

async fn connect_command<B: Backend>(
    State(shared): State<Arc<Shared<B>>>,
    headers: HeaderMap,
    Json(command): Json<Value>,
) -> Response {
    if let Err(refusal) = check(&shared.access, bearer(&headers)) {
        return refusal;
    }
    match shared.backend.connect_command(&command) {
        Ok(()) => Json(json!({ "ok": true })).into_response(),
        Err(message) => error(StatusCode::BAD_REQUEST, &message),
    }
}

#[allow(clippy::result_large_err)]
fn check(access: &Access, token: &str) -> Result<(), Response> {
    if access.authorize(token) {
        Ok(())
    } else {
        Err(error(
            StatusCode::UNAUTHORIZED,
            "This device isn't linked. Link it again in Devices.",
        ))
    }
}

fn bearer(headers: &HeaderMap) -> &str {
    headers
        .get(header::AUTHORIZATION)
        .and_then(|value| value.to_str().ok())
        .and_then(|value| value.strip_prefix("Bearer "))
        .unwrap_or_default()
}

fn error(status: StatusCode, message: &str) -> Response {
    (status, Json(json!({ "error": message }))).into_response()
}

#[cfg(test)]
mod tests;
