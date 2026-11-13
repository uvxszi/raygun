//! Browsers reach device sync over WebRTC (docs/WEB.md). Raygun on the web is a page served over
//! https: it can't open a socket to a device on the local network, but it can make a WebRTC
//! connection to one. This listener is an ICE-lite peer with fixed ICE credentials and a fixed
//! DTLS certificate, so everything a browser needs to connect fits in the device's link code and
//! nothing travels back before the connection is up. Each browser gets a negotiated data channel
//! (id 0) whose bytes are a sync connection like a TCP one: the Noise handshake, then messages.
//!
//! The browser's own DTLS certificate isn't known beforehand, so it isn't checked: the Noise
//! handshake inside authenticates both devices (the browser checks this device's key from the
//! link code; this device knows the browser by its key once paired with the code), as it does
//! over plain TCP.
//!
//! The crypto provider is RustCrypto through dimpl, with certificates made by rcgen on ring; it is
//! adapted from str0m-rust-crypto (str0m, MIT OR Apache-2.0), which can't be used as it is because
//! it builds aws-lc for its certificates.

use std::collections::HashMap;
use std::net::{IpAddr, Ipv4Addr, SocketAddr};
use std::ops::RangeInclusive;
use std::sync::{Arc, OnceLock};
use std::time::{Duration, Instant};

use str0m::channel::{ChannelConfig, ChannelId};
use str0m::crypto::dtls::{
    DtlsCert, DtlsImplError, DtlsInstance, DtlsOutput, DtlsProvider, DtlsVersion, ProtocolVersion,
};
use str0m::crypto::{
    AeadAes128GcmCipher, AeadAes256GcmCipher, Aes128CmSha1_80Cipher, CryptoError, CryptoProvider,
    Fingerprint, Sha1HmacProvider, Sha1Provider, Sha256Provider, SrtpProvider,
    SupportedAeadAes128Gcm, SupportedAeadAes256Gcm, SupportedAes128CmSha1_80,
};
use str0m::net::{Protocol, Receive};
use str0m::{Candidate, Event, IceConnectionState, IceCreds, Input, Output, Rtc};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::UdpSocket;
use tokio::sync::{mpsc, oneshot};

use crate::{Engine, SyncError};

/// UDP ports tried in order; the first one free on every address is used.
pub const PORTS: RangeInclusive<u16> = 8840..=8849;
/// Browsers connected at once; more wait for one to finish.
const MAX_SESSIONS: usize = 8;
/// A connection that has done nothing this long is dropped.
const IDLE: Duration = Duration::from_secs(120);
/// After its sync ended, a connection stays this long so the last bytes reach the browser.
const LINGER: Duration = Duration::from_secs(5);
/// The data channel both sides agree on without a message (`negotiated: true, id: 0`).
const CHANNEL_ID: u16 = 0;
/// Bytes read from the sync side per data channel message.
const CHUNK: usize = 16 * 1024;

const STUN_BINDING_REQUEST: [u8; 2] = [0x00, 0x01];
const STUN_MAGIC: [u8; 4] = [0x21, 0x12, 0xA4, 0x42];
const STUN_USERNAME: u16 = 0x0006;

/// What stays the same between runs, so a link code keeps working: the ICE credentials and the
/// DTLS certificate.
#[derive(Clone, PartialEq, Eq)]
pub struct WebConfig {
    pub ufrag: String,
    pub pwd: String,
    pub certificate: Vec<u8>,
    pub private_key: Vec<u8>,
}

impl std::fmt::Debug for WebConfig {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "WebConfig({})", self.fingerprint())
    }
}

/// Letters and digits only: valid ICE characters, and no escaping in a link code.
fn random_text(length: usize) -> String {
    const ALPHABET: &[u8] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789";
    (0..length.div_ceil(16))
        .flat_map(|_| uuid::Uuid::new_v4().into_bytes())
        .take(length)
        .map(|byte| ALPHABET[byte as usize % ALPHABET.len()] as char)
        .collect()
}

impl WebConfig {
    pub fn generate() -> Result<Self, SyncError> {
        let certificate = make_certificate()
            .ok_or_else(|| SyncError::Protocol("couldn't make a certificate".into()))?;
        Ok(Self {
            ufrag: random_text(8),
            pwd: random_text(24),
            certificate: certificate.certificate,
            private_key: certificate.private_key,
        })
    }

    /// As one line of text, for the app's local storage.
    pub fn to_stored(&self) -> String {
        format!(
            "1:{}:{}:{}:{}",
            self.ufrag,
            self.pwd,
            crate::to_hex(&self.certificate),
            crate::to_hex(&self.private_key)
        )
    }

    pub fn from_stored(text: &str) -> Option<Self> {
        let mut parts = text.trim().split(':');
        if parts.next()? != "1" {
            return None;
        }
        let config = Self {
            ufrag: parts.next()?.to_owned(),
            pwd: parts.next()?.to_owned(),
            certificate: crate::channel::from_hex(parts.next()?)?,
            private_key: crate::channel::from_hex(parts.next()?)?,
        };
        (parts.next().is_none()
            && config.ufrag.len() >= 4
            && config.pwd.len() >= 22
            && !config.certificate.is_empty()
            && !config.private_key.is_empty())
        .then_some(config)
    }

    /// The SHA-256 of the certificate, hex: what the browser checks the connection against.
    pub fn fingerprint(&self) -> String {
        crate::to_hex(&RustCryptoSha256.sha256(&self.certificate))
    }

    fn cert(&self) -> DtlsCert {
        DtlsCert {
            certificate: self.certificate.clone(),
            private_key: self.private_key.clone(),
        }
    }
}

/// The listener while it runs; dropping it (or [`WebRunning::stop`]) stops it.
pub struct WebRunning {
    pub port: u16,
    pub hosts: Vec<IpAddr>,
    fingerprint: String,
    ufrag: String,
    pwd: String,
    stop: Option<oneshot::Sender<()>>,
}

impl WebRunning {
    pub fn stop(mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }

    /// What a link code adds for a browser, as query pairs: this device's sync identity, its open
    /// pairing code, and how to reach the listener. None while no pairing code is open.
    pub fn link_params(&self, engine: &Engine) -> Option<Vec<(&'static str, String)>> {
        let code = engine.open_code()?;
        let hosts: Vec<String> = self.hosts.iter().map(ToString::to_string).collect();
        Some(vec![
            ("sync", engine.device_id().to_owned()),
            ("key", crate::to_hex(&engine.identity().public)),
            ("pair", code),
            ("ice", format!("{}:{}", self.ufrag, self.pwd)),
            ("fp", self.fingerprint.clone()),
            ("udp", self.port.to_string()),
            ("ips", hosts.join(",")),
        ])
    }
}

impl Drop for WebRunning {
    fn drop(&mut self) {
        if let Some(stop) = self.stop.take() {
            let _ = stop.send(());
        }
    }
}

/// The addresses a browser on the local network may reach: every IPv4 address of an interface
/// that's up, loopback last (a browser on this same computer).
pub fn local_hosts() -> Vec<IpAddr> {
    let mut hosts: Vec<IpAddr> = if_addrs::get_if_addrs()
        .unwrap_or_default()
        .into_iter()
        .filter(|interface| !interface.is_loopback())
        .map(|interface| interface.ip())
        .filter(|ip| matches!(ip, IpAddr::V4(v4) if !v4.is_link_local() && !v4.is_unspecified()))
        .collect();
    hosts.sort();
    hosts.dedup();
    hosts.push(IpAddr::V4(Ipv4Addr::LOCALHOST));
    hosts
}

impl Engine {
    /// Listens for browsers on every address in `hosts`, on the first port of `ports` free on all
    /// of them (port 0: one the system picks).
    pub async fn serve_web(
        self: &Arc<Self>,
        config: WebConfig,
        hosts: Vec<IpAddr>,
        ports: RangeInclusive<u16>,
    ) -> std::io::Result<WebRunning> {
        if hosts.is_empty() {
            return Err(std::io::Error::other("no address to listen on"));
        }
        let mut last_error = None;
        for port in ports {
            match bind_all(&hosts, port).await {
                Ok((port, sockets)) => {
                    let (stop, stopped) = oneshot::channel();
                    let running = WebRunning {
                        port,
                        hosts: hosts.clone(),
                        fingerprint: config.fingerprint(),
                        ufrag: config.ufrag.clone(),
                        pwd: config.pwd.clone(),
                        stop: Some(stop),
                    };
                    tokio::spawn(run(self.clone(), config, sockets, stopped));
                    return Ok(running);
                }
                Err(error) => last_error = Some(error),
            }
        }
        Err(last_error.unwrap_or_else(|| std::io::Error::other("no port to try")))
    }
}

/// One socket per address, all on the same port.
async fn bind_all(hosts: &[IpAddr], port: u16) -> std::io::Result<(u16, Vec<Arc<UdpSocket>>)> {
    let mut sockets = Vec::with_capacity(hosts.len());
    let mut port = port;
    for host in hosts {
        let socket = UdpSocket::bind((*host, port)).await?;
        port = socket.local_addr()?.port();
        sockets.push(Arc::new(socket));
    }
    Ok((port, sockets))
}

/// The remote ufrag of a STUN binding request addressed to `ufrag` (its USERNAME is
/// `<ufrag>:<remote>`), or None for anything else.
pub(crate) fn binding_request_ufrag(data: &[u8], ufrag: &str) -> Option<String> {
    if data.len() < 20 || data[0..2] != STUN_BINDING_REQUEST || data[4..8] != STUN_MAGIC {
        return None;
    }
    let length = u16::from_be_bytes([data[2], data[3]]) as usize;
    let body = data.get(20..20 + length)?;
    let mut at = 0;
    while at + 4 <= body.len() {
        let kind = u16::from_be_bytes([body[at], body[at + 1]]);
        let size = u16::from_be_bytes([body[at + 2], body[at + 3]]) as usize;
        let value = body.get(at + 4..at + 4 + size)?;
        if kind == STUN_USERNAME {
            let (local, remote) = std::str::from_utf8(value).ok()?.split_once(':')?;
            return (local == ufrag && !remote.is_empty()).then(|| remote.to_owned());
        }
        at += 4 + size.div_ceil(4) * 4;
    }
    None
}

/// One browser's connection.
struct Session {
    rtc: Rtc,
    channel: ChannelId,
    /// Bytes from the browser, on their way to the sync connection.
    to_sync: Option<mpsc::UnboundedSender<Vec<u8>>>,
    next: Option<Instant>,
    active_at: Instant,
    ended_at: Option<Instant>,
    alive: bool,
}

fn new_session(
    config: &WebConfig,
    locals: &[SocketAddr],
    remote_ufrag: String,
    now: Instant,
) -> Result<Session, str0m::RtcError> {
    let mut rtc = Rtc::builder()
        .set_crypto_provider(provider())
        .set_ice_lite(true)
        .set_fingerprint_verification(false)
        .set_local_ice_credentials(IceCreds {
            ufrag: config.ufrag.clone(),
            pass: config.pwd.clone(),
        })
        .set_dtls_cert(config.cert())
        .build(now);
    for local in locals {
        if let Ok(candidate) = Candidate::host(*local, "udp") {
            rtc.add_local_candidate(candidate);
        }
    }
    let mut api = rtc.direct_api();
    api.set_ice_controlling(false);
    // An ICE-lite agent only answers checks, which it signs with its own password: the
    // browser's is never needed.
    api.set_remote_ice_credentials(IceCreds {
        ufrag: remote_ufrag,
        pass: "not-used-by-an-ice-lite-agent".into(),
    });
    // Not checked (see the module's comment); str0m wants one set before DTLS.
    api.set_remote_fingerprint(Fingerprint {
        hash_func: "sha-256".into(),
        bytes: vec![0; 32],
    });
    api.start_dtls(false)?;
    api.start_sctp(false);
    let channel = api.create_data_channel(ChannelConfig {
        label: "raygun-sync".into(),
        negotiated: Some(CHANNEL_ID),
        ..Default::default()
    });
    Ok(Session {
        rtc,
        channel,
        to_sync: None,
        next: None,
        active_at: now,
        ended_at: None,
        alive: true,
    })
}

/// The listener's loop: packets in, timeouts, and bytes the sync side writes, each followed by
/// draining the connection it touched (str0m's run loop).
async fn run(
    engine: Arc<Engine>,
    config: WebConfig,
    sockets: Vec<Arc<UdpSocket>>,
    mut stopped: oneshot::Receiver<()>,
) {
    let locals: Vec<SocketAddr> = sockets
        .iter()
        .filter_map(|socket| socket.local_addr().ok())
        .collect();
    let (packets_in, mut packets) = mpsc::channel::<(SocketAddr, SocketAddr, Vec<u8>)>(256);
    let mut readers = Vec::new();
    for socket in &sockets {
        let socket = socket.clone();
        let packets_in = packets_in.clone();
        readers.push(tokio::spawn(async move {
            let Ok(local) = socket.local_addr() else {
                return;
            };
            let mut buffer = vec![0u8; 2048];
            loop {
                match socket.recv_from(&mut buffer).await {
                    Ok((size, source)) => {
                        let packet = (local, source, buffer[..size].to_vec());
                        if packets_in.send(packet).await.is_err() {
                            break;
                        }
                    }
                    // A port unreachable answer to something sent (Windows reports those here).
                    Err(error) if error.kind() == std::io::ErrorKind::ConnectionReset => {}
                    Err(error) => {
                        tracing::warn!(%error, "web sync socket failed");
                        break;
                    }
                }
            }
        }));
    }
    drop(packets_in);
    let (written, mut writes) = mpsc::unbounded_channel::<(u64, Vec<u8>)>();
    let mut sessions: HashMap<u64, Session> = HashMap::new();
    let mut next_id = 0u64;
    loop {
        let now = Instant::now();
        let deadline = sessions
            .values()
            .filter_map(|session| session.next)
            .min()
            .unwrap_or(now + Duration::from_secs(1))
            .max(now + Duration::from_millis(1));
        tokio::select! {
            _ = &mut stopped => break,
            packet = packets.recv() => {
                let Some((local, source, data)) = packet else { break };
                let now = Instant::now();
                let Ok(contents) = data.as_slice().try_into() else { continue };
                let input = Input::Receive(now, Receive {
                    proto: Protocol::Udp,
                    source,
                    destination: local,
                    contents,
                });
                let found = sessions
                    .iter()
                    .find(|(_, session)| session.rtc.accepts(&input))
                    .map(|(id, _)| *id);
                let id = match found {
                    Some(id) => id,
                    None => {
                        let Some(remote) = binding_request_ufrag(&data, &config.ufrag) else {
                            continue;
                        };
                        if sessions.len() >= MAX_SESSIONS {
                            tracing::info!("too many browsers connecting; one was turned away");
                            continue;
                        }
                        match new_session(&config, &locals, remote, now) {
                            Ok(session) => {
                                next_id += 1;
                                sessions.insert(next_id, session);
                                next_id
                            }
                            Err(error) => {
                                tracing::warn!(%error, "couldn't start a browser connection");
                                continue;
                            }
                        }
                    }
                };
                if let Some(session) = sessions.get_mut(&id) {
                    session.active_at = now;
                    if let Err(error) = session.rtc.handle_input(input) {
                        tracing::info!(%error, "browser connection failed");
                        session.alive = false;
                    }
                    drain(id, session, &sockets, &engine, &written);
                }
            }
            write = writes.recv() => {
                let Some((id, bytes)) = write else { break };
                let Some(session) = sessions.get_mut(&id) else { continue };
                session.active_at = Instant::now();
                if bytes.is_empty() {
                    // The sync connection ended.
                    session.ended_at.get_or_insert_with(Instant::now);
                    continue;
                }
                let channel = session.channel;
                let sent = session
                    .rtc
                    .channel(channel)
                    .map(|mut channel| channel.write(true, &bytes));
                if !matches!(sent, Some(Ok(true))) {
                    tracing::info!("couldn't send to a browser");
                    session.alive = false;
                }
                drain(id, session, &sockets, &engine, &written);
            }
            _ = tokio::time::sleep_until(deadline.into()) => {
                let now = Instant::now();
                for (id, session) in &mut sessions {
                    if session.next.is_some_and(|next| next <= now) {
                        if let Err(error) = session.rtc.handle_input(Input::Timeout(now)) {
                            tracing::info!(%error, "browser connection failed");
                            session.alive = false;
                        }
                        drain(*id, session, &sockets, &engine, &written);
                    }
                }
            }
        }
        let now = Instant::now();
        sessions.retain(|_, session| {
            session.alive
                && session.rtc.is_alive()
                && now.duration_since(session.active_at) < IDLE
                && session
                    .ended_at
                    .is_none_or(|ended| now.duration_since(ended) < LINGER)
        });
    }
    for reader in readers {
        reader.abort();
    }
}

/// Polls a connection until it waits for time to pass: packets go out, events are handled.
fn drain(
    id: u64,
    session: &mut Session,
    sockets: &[Arc<UdpSocket>],
    engine: &Arc<Engine>,
    written: &mpsc::UnboundedSender<(u64, Vec<u8>)>,
) {
    loop {
        let output = match session.rtc.poll_output() {
            Ok(output) => output,
            Err(error) => {
                tracing::info!(%error, "browser connection failed");
                session.alive = false;
                return;
            }
        };
        match output {
            Output::Timeout(next) => {
                session.next = Some(next);
                return;
            }
            Output::Transmit(transmit) => {
                let socket = sockets
                    .iter()
                    .find(|socket| socket.local_addr().ok() == Some(transmit.source));
                if let Some(socket) = socket {
                    // UDP: a packet that can't go now is lost and sent again by DTLS or SCTP.
                    let _ = socket.try_send_to(&transmit.contents, transmit.destination);
                }
            }
            Output::Event(event) => match event {
                Event::ChannelOpen(channel, _) if channel == session.channel => {
                    session.to_sync = Some(bridge(id, engine.clone(), written.clone()));
                }
                Event::ChannelData(data) if data.id == session.channel => {
                    if let Some(to_sync) = &session.to_sync
                        && to_sync.send(data.data).is_err()
                    {
                        session.to_sync = None;
                    }
                }
                Event::ChannelClose(channel) if channel == session.channel => {
                    session.to_sync = None;
                    session.ended_at.get_or_insert_with(Instant::now);
                }
                Event::IceConnectionStateChange(IceConnectionState::Disconnected) => {
                    session.alive = false;
                }
                _ => {}
            },
        }
    }
}

/// Runs a sync connection over a browser's data channel: bytes from the browser go in through
/// the returned sender, bytes the connection writes come back on `written` (empty when it ends).
fn bridge(
    id: u64,
    engine: Arc<Engine>,
    written: mpsc::UnboundedSender<(u64, Vec<u8>)>,
) -> mpsc::UnboundedSender<Vec<u8>> {
    let (ours, theirs) = tokio::io::duplex(256 * 1024);
    let (mut reader, mut writer) = tokio::io::split(ours);
    let (to_sync, mut incoming) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        while let Some(bytes) = incoming.recv().await {
            if writer.write_all(&bytes).await.is_err() {
                break;
            }
        }
        let _ = writer.shutdown().await;
    });
    tokio::spawn(async move {
        let mut buffer = vec![0u8; CHUNK];
        loop {
            match reader.read(&mut buffer).await {
                Ok(0) | Err(_) => {
                    let _ = written.send((id, Vec::new()));
                    break;
                }
                Ok(size) => {
                    if written.send((id, buffer[..size].to_vec())).is_err() {
                        break;
                    }
                }
            }
        }
    });
    tokio::spawn(async move {
        if let Err(error) = engine.handle_stream(theirs, None).await {
            tracing::info!(%error, "browser sync connection ended");
        }
    });
    to_sync
}

// ---- The crypto provider ----

/// RustCrypto through dimpl for str0m (adapted from str0m-rust-crypto, MIT OR Apache-2.0).
fn provider() -> Arc<CryptoProvider> {
    static PROVIDER: OnceLock<Arc<CryptoProvider>> = OnceLock::new();
    static SRTP: NoSrtp = NoSrtp;
    static SHA1_HMAC: RustCryptoSha1Hmac = RustCryptoSha1Hmac;
    static SHA1: RustCryptoSha1 = RustCryptoSha1;
    static SHA256: RustCryptoSha256 = RustCryptoSha256;
    static DTLS: RustCryptoDtls = RustCryptoDtls;
    PROVIDER
        .get_or_init(|| {
            Arc::new(CryptoProvider {
                srtp_provider: &SRTP,
                sha1_hmac_provider: &SHA1_HMAC,
                sha1_provider: &SHA1,
                sha256_provider: &SHA256,
                dtls_provider: &DTLS,
            })
        })
        .clone()
}

/// A self-signed ECDSA P-256 certificate, the kind browsers expect for DTLS.
fn make_certificate() -> Option<DtlsCert> {
    let key = rcgen::KeyPair::generate_for(&rcgen::PKCS_ECDSA_P256_SHA256).ok()?;
    let mut params = rcgen::CertificateParams::new(Vec::<String>::new()).ok()?;
    let mut name = rcgen::DistinguishedName::new();
    name.push(rcgen::DnType::CommonName, "Raygun");
    params.distinguished_name = name;
    // Unique serial numbers, which Firefox wants across every certificate it sees.
    params.serial_number = Some(uuid::Uuid::new_v4().into_bytes().to_vec().into());
    let certificate = params.self_signed(&key).ok()?;
    Some(DtlsCert {
        certificate: certificate.der().to_vec(),
        private_key: key.serialize_der(),
    })
}

#[derive(Debug)]
struct RustCryptoSha1Hmac;

impl Sha1HmacProvider for RustCryptoSha1Hmac {
    fn sha1_hmac(&self, key: &[u8], payloads: &[&[u8]]) -> [u8; 20] {
        use hmac::{KeyInit, Mac};
        let mut mac =
            hmac::Hmac::<sha1::Sha1>::new_from_slice(key).expect("HMAC takes a key of any size");
        for payload in payloads {
            mac.update(payload);
        }
        mac.finalize().into_bytes().into()
    }
}

#[derive(Debug)]
struct RustCryptoSha1;

impl Sha1Provider for RustCryptoSha1 {
    fn sha1(&self, data: &[u8]) -> [u8; 20] {
        use sha1::Digest;
        sha1::Sha1::digest(data).into()
    }
}

#[derive(Debug)]
struct RustCryptoSha256;

impl Sha256Provider for RustCryptoSha256 {
    fn sha256(&self, data: &[u8]) -> [u8; 32] {
        use sha2::Digest;
        sha2::Sha256::digest(data).into()
    }
}

#[derive(Debug)]
struct RustCryptoDtls;

impl DtlsProvider for RustCryptoDtls {
    fn generate_certificate(&self) -> Option<DtlsCert> {
        make_certificate()
    }

    fn new_dtls(
        &self,
        cert: &DtlsCert,
        now: Instant,
        dtls_version: DtlsVersion,
        mtu: Option<usize>,
    ) -> Result<Box<dyn DtlsInstance>, CryptoError> {
        // ICE checks the return route before DTLS starts, so server cookies add nothing.
        let mut builder = dimpl::Config::builder().use_server_cookie(false);
        if let Some(mtu) = mtu {
            builder = builder.mtu(mtu);
        }
        let config = Arc::new(
            builder
                .build()
                .map_err(|error| CryptoError::Other(format!("dimpl config: {error}")))?,
        );
        let certificate = dimpl::DtlsCertificate {
            certificate: cert.certificate.clone(),
            private_key: cert.private_key.clone(),
        };
        let dtls = match dtls_version {
            DtlsVersion::Dtls12 => dimpl::Dtls::new_12(config, certificate, now),
            DtlsVersion::Dtls13 => dimpl::Dtls::new_13(config, certificate, now),
            DtlsVersion::Auto => dimpl::Dtls::new_auto(config, certificate, now),
            other => {
                return Err(CryptoError::Other(format!(
                    "unsupported DTLS version: {other}"
                )));
            }
        };
        Ok(Box::new(DimplInstance { dtls }))
    }
}

struct DimplInstance {
    dtls: dimpl::Dtls,
}

impl std::fmt::Debug for DimplInstance {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.debug_struct("DimplInstance").finish()
    }
}

impl DtlsInstance for DimplInstance {
    fn set_active(&mut self, active: bool) {
        self.dtls.set_active(active);
    }

    fn handle_packet(&mut self, packet: &[u8]) -> Result<(), DtlsImplError> {
        self.dtls.handle_packet(packet)
    }

    fn poll_output<'a>(&mut self, buffer: &'a mut [u8]) -> DtlsOutput<'a> {
        self.dtls.poll_output(buffer)
    }

    fn handle_timeout(&mut self, now: Instant) -> Result<(), DtlsImplError> {
        self.dtls.handle_timeout(now)
    }

    fn send_application_data(&mut self, data: &[u8]) -> Result<usize, DtlsImplError> {
        self.dtls.send_application_data(data)
    }

    fn is_active(&self) -> bool {
        self.dtls.is_active()
    }

    fn protocol_version(&self) -> Option<ProtocolVersion> {
        self.dtls.protocol_version()
    }

    fn is_closing(&self) -> bool {
        self.dtls.is_closing()
    }

    fn is_closed(&self) -> bool {
        self.dtls.is_closed()
    }

    fn close(&mut self) -> Result<(), DtlsImplError> {
        self.dtls.close()
    }
}

/// Sync's connections carry a data channel only, never media, so SRTP is never keyed for use:
/// its ciphers refuse, and key derivation fills zeros nothing reads.
#[derive(Debug)]
struct NoSrtp;

#[derive(Debug)]
struct NoCipher;

const NO_MEDIA: &str = "browser sync connections carry no media";

impl Aes128CmSha1_80Cipher for NoCipher {
    fn encrypt(&mut self, _: &[u8; 16], _: &[u8], _: &mut [u8]) -> Result<(), CryptoError> {
        Err(CryptoError::Other(NO_MEDIA.into()))
    }

    fn decrypt(&mut self, _: &[u8; 16], _: &[u8], _: &mut [u8]) -> Result<(), CryptoError> {
        Err(CryptoError::Other(NO_MEDIA.into()))
    }
}

impl AeadAes128GcmCipher for NoCipher {
    fn encrypt(
        &mut self,
        _: &[u8; 12],
        _: &[u8],
        _: &[u8],
        _: &mut [u8],
    ) -> Result<(), CryptoError> {
        Err(CryptoError::Other(NO_MEDIA.into()))
    }

    fn decrypt(
        &mut self,
        _: &[u8; 12],
        _: &[&[u8]],
        _: &[u8],
        _: &mut [u8],
    ) -> Result<usize, CryptoError> {
        Err(CryptoError::Other(NO_MEDIA.into()))
    }
}

impl AeadAes256GcmCipher for NoCipher {
    fn encrypt(
        &mut self,
        _: &[u8; 12],
        _: &[u8],
        _: &[u8],
        _: &mut [u8],
    ) -> Result<(), CryptoError> {
        Err(CryptoError::Other(NO_MEDIA.into()))
    }

    fn decrypt(
        &mut self,
        _: &[u8; 12],
        _: &[&[u8]],
        _: &[u8],
        _: &mut [u8],
    ) -> Result<usize, CryptoError> {
        Err(CryptoError::Other(NO_MEDIA.into()))
    }
}

impl SupportedAes128CmSha1_80 for NoSrtp {
    fn create_cipher(&self, _: [u8; 16], _: bool) -> Box<dyn Aes128CmSha1_80Cipher> {
        Box::new(NoCipher)
    }
}

impl SupportedAeadAes128Gcm for NoSrtp {
    fn create_cipher(&self, _: [u8; 16], _: bool) -> Box<dyn AeadAes128GcmCipher> {
        Box::new(NoCipher)
    }
}

impl SupportedAeadAes256Gcm for NoSrtp {
    fn create_cipher(&self, _: [u8; 32], _: bool) -> Box<dyn AeadAes256GcmCipher> {
        Box::new(NoCipher)
    }
}

impl SrtpProvider for NoSrtp {
    fn aes_128_cm_sha1_80(&self) -> &'static dyn SupportedAes128CmSha1_80 {
        &NoSrtp
    }

    fn aead_aes_128_gcm(&self) -> &'static dyn SupportedAeadAes128Gcm {
        &NoSrtp
    }

    fn aead_aes_256_gcm(&self) -> &'static dyn SupportedAeadAes256Gcm {
        &NoSrtp
    }

    fn srtp_aes_128_ecb_round(&self, _: &[u8], _: &[u8], output: &mut [u8]) {
        output.fill(0);
    }

    fn srtp_aes_256_ecb_round(&self, _: &[u8], _: &[u8], output: &mut [u8]) {
        output.fill(0);
    }
}

#[cfg(test)]
#[path = "web_tests.rs"]
mod tests;
