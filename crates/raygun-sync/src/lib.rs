//! Device sync (ARCHITECTURE §10, M21): paired devices exchange the rows that changed since they
//! last spoke, over an encrypted channel, and merge them (`raygun-db::sync`).
//!
//! A session is half-duplex: the initiator sends its changes and waits for the acknowledgement,
//! then the responder does the same. Each side remembers how far the other acknowledged it.

pub mod channel;
pub mod discovery;
pub mod protocol;

use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::sync::{Arc, Mutex, RwLock};

use raygun_db::sync::{self, Peer, SyncKind, SyncRow};
use raygun_db::{Db, DbError};
use tokio::net::{TcpListener, TcpStream};
use tokio::sync::oneshot;

use channel::{Channel, IO_TIMEOUT};
pub use channel::{Identity, fingerprint, to_hex};
use protocol::{Message, Offer};

/// Ports tried in order; the first free one is used.
pub const PORTS: std::ops::RangeInclusive<u16> = 8830..=8839;
/// How long a pairing code works.
pub const CODE_LIFETIME_MS: i64 = 10 * 60_000;
/// Wrong codes tolerated before the open code is cancelled.
pub const MAX_FAILURES: u32 = 5;
/// Rows read from the change log per batch; batches are also cut by size.
const BATCH_ROWS: i64 = 200;
const BATCH_BYTES: usize = 48 * 1024;
const CODE_ALPHABET: &[u8] = b"23456789abcdefghjkmnpqrstuvwxyz";
const CODE_LENGTH: usize = 8;

#[derive(Debug, thiserror::Error)]
pub enum SyncError {
    #[error("network error: {0}")]
    Io(#[from] std::io::Error),
    #[error("encryption error: {0}")]
    Noise(#[from] snow::Error),
    #[error("database error: {0}")]
    Db(#[from] DbError),
    #[error("the other device didn't answer in time")]
    Timeout,
    #[error("unexpected message: {0}")]
    Protocol(String),
    #[error("{0}")]
    Refused(String),
}

/// What one session moved.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct Summary {
    pub sent: usize,
    pub received: usize,
    pub changed: usize,
}

#[derive(Debug, Default)]
struct Codes {
    open: Option<(String, i64)>,
    failures: u32,
}

pub struct Engine {
    db: Arc<Db>,
    identity: Identity,
    device_id: String,
    name: RwLock<String>,
    kinds: RwLock<Vec<SyncKind>>,
    codes: Mutex<Codes>,
    port: Mutex<Option<u16>>,
    clock: Box<dyn Fn() -> i64 + Send + Sync>,
    on_synced: RwLock<Option<SyncedHook>>,
}

/// Called after each successful session, whichever device started it.
pub type SyncedHook = Arc<dyn Fn(&Peer, &Summary) + Send + Sync>;

/// The listening server; dropping it (or calling [`Running::stop`]) stops accepting.
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

impl Engine {
    pub fn new(db: Arc<Db>, identity: Identity, name: String, kinds: Vec<SyncKind>) -> Self {
        Self::with_clock(db, identity, name, kinds, Box::new(now_ms))
    }

    pub fn with_clock(
        db: Arc<Db>,
        identity: Identity,
        name: String,
        kinds: Vec<SyncKind>,
        clock: Box<dyn Fn() -> i64 + Send + Sync>,
    ) -> Self {
        Self {
            device_id: db.device_id().to_string(),
            db,
            identity,
            name: RwLock::new(name),
            kinds: RwLock::new(kinds),
            codes: Mutex::new(Codes::default()),
            port: Mutex::new(None),
            clock,
            on_synced: RwLock::new(None),
        }
    }

    /// Runs `hook` after every successful session (the app refreshes its views from it).
    pub fn on_synced(&self, hook: SyncedHook) {
        *self.on_synced.write().expect("hook lock poisoned") = Some(hook);
    }

    pub fn device_id(&self) -> &str {
        &self.device_id
    }

    pub fn identity(&self) -> &Identity {
        &self.identity
    }

    pub fn name(&self) -> String {
        self.name.read().expect("name lock poisoned").clone()
    }

    pub fn set_name(&self, name: String) {
        *self.name.write().expect("name lock poisoned") = name;
    }

    pub fn kinds(&self) -> Vec<SyncKind> {
        self.kinds.read().expect("kinds lock poisoned").clone()
    }

    /// Changes what syncs. A kind both devices now share is sent in full on the next sync, by
    /// whichever side has it (see `send_changes`).
    pub fn set_kinds(&self, kinds: Vec<SyncKind>) {
        *self.kinds.write().expect("kinds lock poisoned") = kinds;
    }

    /// The port this device listens on, while serving.
    pub fn port(&self) -> Option<u16> {
        *self.port.lock().expect("port lock poisoned")
    }

    /// Listens on every interface, on the first free port in [`PORTS`].
    pub async fn serve(self: &Arc<Self>) -> std::io::Result<Running> {
        self.serve_on(Ipv4Addr::UNSPECIFIED.into(), PORTS).await
    }

    pub async fn serve_on(
        self: &Arc<Self>,
        ip: IpAddr,
        ports: std::ops::RangeInclusive<u16>,
    ) -> std::io::Result<Running> {
        let mut last_error = None;
        for port in ports {
            match TcpListener::bind((ip, port)).await {
                Ok(listener) => {
                    // Port 0 asks the OS for any free port; record the one it chose.
                    let port = listener.local_addr()?.port();
                    *self.port.lock().expect("port lock poisoned") = Some(port);
                    let (stop, mut stopped) = oneshot::channel::<()>();
                    let engine = self.clone();
                    tokio::spawn(async move {
                        loop {
                            tokio::select! {
                                _ = &mut stopped => break,
                                accepted = listener.accept() => match accepted {
                                    Ok((stream, address)) => {
                                        let engine = engine.clone();
                                        tokio::spawn(async move {
                                            if let Err(error) = engine.handle(stream, address).await {
                                                tracing::info!(%address, %error, "sync connection ended");
                                            }
                                        });
                                    }
                                    Err(error) => tracing::warn!(%error, "sync accept failed"),
                                },
                            }
                        }
                        *engine.port.lock().expect("port lock poisoned") = None;
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

    /// Opens a pairing code (replacing any earlier one) and returns the text to show.
    pub fn open_pairing(&self, host: IpAddr) -> Result<String, SyncError> {
        let port = self
            .port()
            .ok_or_else(|| SyncError::Refused("Turn device sync on first.".into()))?;
        let code = random_code();
        *self.codes.lock().expect("codes lock poisoned") = Codes {
            open: Some((code.clone(), (self.clock)() + CODE_LIFETIME_MS)),
            failures: 0,
        };
        Ok(Offer {
            host: host.to_string(),
            port,
            code,
            device_id: self.device_id.clone(),
            public_key: self.identity.public.clone(),
        }
        .to_text())
    }

    /// The pairing text while its code is open.
    pub fn open_offer(&self, host: IpAddr) -> Option<String> {
        let port = self.port()?;
        let codes = self.codes.lock().expect("codes lock poisoned");
        let (code, expires) = codes.open.as_ref()?;
        ((self.clock)() < *expires).then(|| {
            Offer {
                host: host.to_string(),
                port,
                code: code.clone(),
                device_id: self.device_id.clone(),
                public_key: self.identity.public.clone(),
            }
            .to_text()
        })
    }

    /// Pairs with the device that showed `text`, then runs a first sync with it.
    pub async fn join(&self, text: &str) -> Result<Peer, SyncError> {
        let offer = Offer::parse(text)
            .ok_or_else(|| SyncError::Refused("That isn't a Raygun pairing text.".into()))?;
        if offer.device_id == self.device_id {
            return Err(SyncError::Refused(
                "That's this device's own pairing text.".into(),
            ));
        }
        let stream = connect(&offer.address()).await?;
        let (mut channel, remote) = Channel::initiate(stream, &self.identity).await?;
        // Checked before the code is sent: a device in the middle never learns it.
        if remote != offer.public_key {
            return Err(SyncError::Refused(
                "The other device's key doesn't match its pairing text.".into(),
            ));
        }
        channel
            .send(&Message::Pair {
                code: offer.code.clone(),
                device_id: self.device_id.clone(),
                name: self.name(),
                port: self.port(),
            })
            .await?;
        match channel.recv().await? {
            Message::Paired { device_id, name } if device_id == offer.device_id => {
                let peer = Peer {
                    device_id,
                    name,
                    public_key: to_hex(&remote),
                    address: Some(offer.address()),
                    paired_at: (self.clock)(),
                    last_sync_at: None,
                    sent_seq: 0,
                    kinds: Vec::new(),
                };
                sync::save_peer(&self.db, &peer).await?;
                tracing::info!(peer = %peer.name, "paired");
                Ok(peer)
            }
            Message::Refused { reason } => Err(SyncError::Refused(reason)),
            other => Err(SyncError::Protocol(format!("{other:?}"))),
        }
    }

    /// Syncs with one paired device at its last known address.
    pub async fn sync_with(&self, device_id: &str) -> Result<Summary, SyncError> {
        let peer = sync::peer(&self.db, device_id)
            .await?
            .ok_or_else(|| SyncError::Refused("That device isn't paired.".into()))?;
        let address = peer.address.clone().ok_or_else(|| {
            SyncError::Refused(format!("Where {} is isn't known yet.", peer.name))
        })?;
        let session =
            sync::start_session(&self.db, &peer.device_id, &peer.name, (self.clock)()).await?;
        let result = self.initiate_session(&peer, &address, session).await;
        self.finish(session, &peer, Some(&address), &result).await?;
        result
    }

    async fn initiate_session(
        &self,
        peer: &Peer,
        address: &str,
        session: i64,
    ) -> Result<Summary, SyncError> {
        let stream = connect(address).await?;
        let (mut channel, remote) = Channel::initiate(stream, &self.identity).await?;
        if to_hex(&remote) != peer.public_key {
            return Err(SyncError::Refused(format!(
                "The device at {address} isn't {} any more. Pair it again.",
                peer.name
            )));
        }
        channel.send(&self.hello()).await?;
        let kinds = match channel.recv().await? {
            Message::Hello {
                device_id, kinds, ..
            } if device_id == peer.device_id => kinds,
            Message::Refused { reason } => return Err(SyncError::Refused(reason)),
            other => return Err(SyncError::Protocol(format!("{other:?}"))),
        };
        let kinds = self.shared_kinds(&kinds);
        let sent = self.send_changes(&mut channel, peer, &kinds).await?;
        let (received, changed) = self
            .receive_changes(&mut channel, session, &kinds, &peer.device_id)
            .await?;
        Ok(Summary {
            sent,
            received,
            changed,
        })
    }

    /// One incoming connection: pairing (while a code is open) or a sync from a paired device.
    async fn handle(&self, stream: TcpStream, address: SocketAddr) -> Result<(), SyncError> {
        let (mut channel, remote) = Channel::respond(stream, &self.identity).await?;
        match channel.recv().await? {
            Message::Pair {
                code,
                device_id,
                name,
                port,
            } => {
                if let Err(reason) = self.check_code(&code) {
                    channel
                        .send(&Message::Refused {
                            reason: reason.clone(),
                        })
                        .await?;
                    return Err(SyncError::Refused(reason));
                }
                let peer = Peer {
                    device_id,
                    name,
                    public_key: to_hex(&remote),
                    address: port.map(|port| SocketAddr::new(address.ip(), port).to_string()),
                    paired_at: (self.clock)(),
                    last_sync_at: None,
                    sent_seq: 0,
                    kinds: Vec::new(),
                };
                sync::save_peer(&self.db, &peer).await?;
                tracing::info!(peer = %peer.name, "paired");
                channel
                    .send(&Message::Paired {
                        device_id: self.device_id.clone(),
                        name: self.name(),
                    })
                    .await
            }
            Message::Hello {
                device_id,
                kinds,
                port,
                ..
            } => {
                let trusted = sync::peer(&self.db, &device_id)
                    .await?
                    .filter(|peer| peer.public_key == to_hex(&remote));
                let Some(peer) = trusted else {
                    let reason = "This device isn't paired with the other one.".to_owned();
                    channel
                        .send(&Message::Refused {
                            reason: reason.clone(),
                        })
                        .await?;
                    return Err(SyncError::Refused(reason));
                };
                channel.send(&self.hello()).await?;
                let kinds = self.shared_kinds(&kinds);
                let reach = port.map(|port| SocketAddr::new(address.ip(), port).to_string());
                let session =
                    sync::start_session(&self.db, &peer.device_id, &peer.name, (self.clock)())
                        .await?;
                let result = async {
                    let (received, changed) = self
                        .receive_changes(&mut channel, session, &kinds, &peer.device_id)
                        .await?;
                    let sent = self.send_changes(&mut channel, &peer, &kinds).await?;
                    Ok(Summary {
                        sent,
                        received,
                        changed,
                    })
                }
                .await;
                self.finish(session, &peer, reach.as_deref(), &result)
                    .await?;
                result.map(|_| ())
            }
            other => Err(SyncError::Protocol(format!("{other:?}"))),
        }
    }

    /// Sends every change the peer hasn't acknowledged, then waits for its acknowledgement.
    async fn send_changes(
        &self,
        channel: &mut Channel,
        peer: &Peer,
        kinds: &[SyncKind],
    ) -> Result<usize, SyncError> {
        // When the shared kinds grew since the last sync (either device turned one on), send
        // everything again: rows of the new kind were skipped before.
        let mut seq = match sync::peer(&self.db, &peer.device_id).await? {
            Some(stored) if kinds.iter().all(|kind| stored.kinds.contains(kind)) => stored.sent_seq,
            _ => 0,
        };
        let mut sent = 0;
        let mut batch: Vec<SyncRow> = Vec::new();
        let mut batch_bytes = 0;
        loop {
            let (rows, last) =
                sync::changes_since(&self.db, seq, kinds, BATCH_ROWS, Some(&peer.device_id))
                    .await?;
            for row in rows {
                batch_bytes += row.row.to_string().len();
                batch.push(row);
                if batch_bytes >= BATCH_BYTES {
                    sent += batch.len();
                    channel
                        .send(&Message::Rows {
                            rows: std::mem::take(&mut batch),
                        })
                        .await?;
                    batch_bytes = 0;
                }
            }
            if last == seq {
                break;
            }
            seq = last;
        }
        if !batch.is_empty() {
            sent += batch.len();
            channel.send(&Message::Rows { rows: batch }).await?;
        }
        channel.send(&Message::Done { upto: seq }).await?;
        match channel.recv().await? {
            Message::Ack { upto } => {
                sync::set_sent_seq(&self.db, &peer.device_id, upto, kinds).await?;
                Ok(sent)
            }
            other => Err(SyncError::Protocol(format!("{other:?}"))),
        }
    }

    /// Takes the peer's changes until it says it's done, then acknowledges them.
    async fn receive_changes(
        &self,
        channel: &mut Channel,
        session: i64,
        kinds: &[SyncKind],
        from_peer: &str,
    ) -> Result<(usize, usize), SyncError> {
        let (mut received, mut changed) = (0, 0);
        loop {
            match channel.recv().await? {
                Message::Rows { rows } => {
                    let applied = sync::apply(&self.db, session, &rows, kinds, from_peer).await?;
                    received += applied.received;
                    changed += applied.changed;
                }
                Message::Done { upto } => {
                    channel.send(&Message::Ack { upto }).await?;
                    return Ok((received, changed));
                }
                other => return Err(SyncError::Protocol(format!("{other:?}"))),
            }
        }
    }

    async fn finish(
        &self,
        session: i64,
        peer: &Peer,
        address: Option<&str>,
        result: &Result<Summary, SyncError>,
    ) -> Result<(), SyncError> {
        let now = (self.clock)();
        let summary = result.as_ref().copied().unwrap_or_default();
        let error = result.as_ref().err().map(ToString::to_string);
        sync::finish_session(
            &self.db,
            session,
            summary.sent,
            summary.received,
            summary.changed,
            error.as_deref(),
            now,
        )
        .await?;
        if result.is_ok() {
            sync::peer_seen(&self.db, &peer.device_id, address, Some(now)).await?;
            let hook = self.on_synced.read().expect("hook lock poisoned").clone();
            if let Some(hook) = hook {
                hook(peer, &summary);
            }
            tracing::info!(peer = %peer.name, sent = summary.sent, received = summary.received,
                changed = summary.changed, "synced");
        }
        Ok(())
    }

    fn hello(&self) -> Message {
        Message::Hello {
            device_id: self.device_id.clone(),
            name: self.name(),
            kinds: self.kinds(),
            port: self.port(),
        }
    }

    fn shared_kinds(&self, theirs: &[SyncKind]) -> Vec<SyncKind> {
        self.kinds()
            .into_iter()
            .filter(|kind| theirs.contains(kind))
            .collect()
    }

    fn check_code(&self, code: &str) -> Result<(), String> {
        let mut codes = self.codes.lock().expect("codes lock poisoned");
        let now = (self.clock)();
        let open = codes
            .open
            .as_ref()
            .filter(|(_, expires)| now < *expires)
            .map(|(open, _)| open.clone());
        let Some(open) = open else {
            return Err("This pairing code expired. Show a new one on the other device.".into());
        };
        if open != code.trim().to_lowercase() {
            codes.failures += 1;
            if codes.failures >= MAX_FAILURES {
                codes.open = None;
            }
            return Err("That pairing code is wrong.".into());
        }
        *codes = Codes::default();
        Ok(())
    }
}

async fn connect(address: &str) -> Result<TcpStream, SyncError> {
    tokio::time::timeout(IO_TIMEOUT, TcpStream::connect(address))
        .await
        .map_err(|_| SyncError::Timeout)?
        .map_err(SyncError::from)
}

fn random_code() -> String {
    uuid::Uuid::new_v4()
        .into_bytes()
        .iter()
        .take(CODE_LENGTH)
        .map(|byte| CODE_ALPHABET[*byte as usize % CODE_ALPHABET.len()] as char)
        .collect()
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

#[cfg(test)]
mod tests;
