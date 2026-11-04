//! Google Cast (Chromecast, Android TV, Google TV) over the open Cast v2 protocol: TLS on port
//! 8009, each message a big-endian length and a protobuf `CastMessage` carrying JSON on a
//! namespace. Raygun connects to the device, starts the Default Media Receiver (`CC1AD845`), and
//! has it load the video from the cast route; the receiver then reports its media status.
//!
//! Cast devices present self-signed certificates, so the connection is encrypted but the device
//! isn't authenticated, as with every open Cast sender.

use std::net::SocketAddr;
use std::sync::Arc;
use std::time::{Duration, Instant};

use raygun_model::{CastControl, CastState};
use serde_json::{Value, json};
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite, AsyncWriteExt};
use tokio::sync::mpsc;

use super::{CastMedia, Command, Report, Reporter};

pub const SERVICE: &str = "_googlecast._tcp.local.";
pub const PORT: u16 = 8009;
const DEFAULT_MEDIA_RECEIVER: &str = "CC1AD845";
const SENDER: &str = "sender-0";
const RECEIVER: &str = "receiver-0";
const CONNECTION: &str = "urn:x-cast:com.google.cast.tp.connection";
const HEARTBEAT: &str = "urn:x-cast:com.google.cast.tp.heartbeat";
const RECEIVER_NS: &str = "urn:x-cast:com.google.cast.receiver";
const MEDIA: &str = "urn:x-cast:com.google.cast.media";
/// Messages are small; anything this large is a broken stream.
const MAX_MESSAGE: usize = 1 << 20;
const CONNECT_TIMEOUT: Duration = Duration::from_secs(8);
/// A ping this often, and the device counts as gone after hearing nothing for three.
const HEARTBEAT_EVERY: Duration = Duration::from_secs(5);
const SILENCE_LIMIT: Duration = Duration::from_secs(16);
/// Capability bit (`ca` in the TXT record) of a device with a screen.
const VIDEO_OUT: u32 = 1;

/// Cast v2's message (`cast_channel.proto`, proto2: every field but the payloads is required, so
/// even zero values go on the wire).
#[derive(Clone, PartialEq, prost::Message)]
pub struct CastMessage {
    /// `CASTV2_1_0` = 0.
    #[prost(int32, required, tag = "1")]
    pub protocol_version: i32,
    #[prost(string, required, tag = "2")]
    pub source_id: String,
    #[prost(string, required, tag = "3")]
    pub destination_id: String,
    #[prost(string, required, tag = "4")]
    pub namespace: String,
    /// `STRING` = 0, `BINARY` = 1.
    #[prost(int32, required, tag = "5")]
    pub payload_type: i32,
    #[prost(string, optional, tag = "6")]
    pub payload_utf8: Option<String>,
    #[prost(bytes = "vec", optional, tag = "7")]
    pub payload_binary: Option<Vec<u8>>,
}

/// One JSON message, framed for the wire.
pub fn encode(destination: &str, namespace: &str, payload: &Value) -> Vec<u8> {
    use prost::Message;
    let message = CastMessage {
        protocol_version: 0,
        source_id: SENDER.to_owned(),
        destination_id: destination.to_owned(),
        namespace: namespace.to_owned(),
        payload_type: 0,
        payload_utf8: Some(payload.to_string()),
        payload_binary: None,
    };
    let body = message.encode_to_vec();
    let mut frame = Vec::with_capacity(body.len() + 4);
    frame.extend_from_slice(&(body.len() as u32).to_be_bytes());
    frame.extend_from_slice(&body);
    frame
}

/// A message's body (without its length) as the sender, namespace and JSON.
pub fn decode(body: &[u8]) -> Option<(String, String, Value)> {
    use prost::Message;
    let message = CastMessage::decode(body).ok()?;
    let payload = serde_json::from_str(message.payload_utf8.as_deref()?).ok()?;
    Some((message.source_id, message.namespace, payload))
}

/// Whether a device heard on the network can show video (`ca`): speakers and speaker groups
/// can't. Devices that don't say are given the benefit of the doubt.
pub fn has_screen(capabilities: Option<&str>) -> bool {
    capabilities
        .and_then(|value| value.trim().parse::<u32>().ok())
        .is_none_or(|bits| bits & VIDEO_OUT != 0)
}

/// `LOAD` for the Default Media Receiver: the video, its title and poster, and subtitles as
/// WebVTT tracks with the chosen one on.
pub fn load_message(media: &CastMedia, session_id: &str, request_id: u64) -> Value {
    let tracks: Vec<Value> = media
        .tracks
        .iter()
        .enumerate()
        .map(|(index, track)| {
            json!({
                "trackId": index + 1,
                "type": "TEXT",
                "subtype": "SUBTITLES",
                "trackContentId": track.url,
                "trackContentType": "text/vtt",
                "name": track.label,
                "language": track.language,
            })
        })
        .collect();
    let active: Vec<usize> = media
        .active_track
        .map(|index| index + 1)
        .into_iter()
        .collect();
    let images: Vec<Value> = media
        .poster
        .iter()
        .map(|url| json!({ "url": url }))
        .collect();
    json!({
        "type": "LOAD",
        "requestId": request_id,
        "sessionId": session_id,
        "media": {
            "contentId": media.url,
            "contentUrl": media.url,
            "streamType": "BUFFERED",
            "contentType": media.content_type,
            "metadata": {
                "metadataType": 0,
                "title": media.title,
                "subtitle": media.subtitle,
                "images": images,
            },
            "tracks": tracks,
        },
        "autoplay": true,
        "currentTime": media.start_ms.max(0.0) / 1000.0,
        "activeTrackIds": active,
    })
}

/// The Default Media Receiver running on the device: its session and transport ids.
pub fn running_app(status: &Value) -> Option<(String, String)> {
    status["status"]["applications"]
        .as_array()?
        .iter()
        .find(|app| app["appId"] == DEFAULT_MEDIA_RECEIVER)
        .and_then(|app| {
            Some((
                app["sessionId"].as_str()?.to_owned(),
                app["transportId"].as_str()?.to_owned(),
            ))
        })
}

/// A `MEDIA_STATUS` as a report, with its media session's id. `None` when it names no media.
pub fn media_report(status: &Value) -> Option<(i64, Report)> {
    let entry = status["status"].as_array()?.first()?;
    let session = entry["mediaSessionId"].as_i64()?;
    let state = match entry["playerState"].as_str().unwrap_or_default() {
        "PLAYING" => CastState::Playing,
        "PAUSED" => CastState::Paused,
        "BUFFERING" => CastState::Buffering,
        "IDLE" => match entry["idleReason"].as_str() {
            Some("FINISHED") => CastState::Finished,
            Some("ERROR") => CastState::Failed,
            // Another video was loaded over it.
            Some("INTERRUPTED") => CastState::Loading,
            Some(_) => CastState::Stopped,
            None => CastState::Loading,
        },
        _ => CastState::Loading,
    };
    let report = Report {
        state: Some(state),
        position_ms: entry["currentTime"]
            .as_f64()
            .map(|seconds| seconds * 1000.0),
        duration_ms: entry["media"]["duration"]
            .as_f64()
            .filter(|seconds| *seconds > 0.0)
            .map(|seconds| seconds * 1000.0),
        error: (state == CastState::Failed).then(|| super::TV_FAILED.to_owned()),
        ..Report::default()
    };
    Some((session, report))
}

/// The device's own volume from a `RECEIVER_STATUS`.
pub fn receiver_volume(status: &Value) -> Option<(f64, bool)> {
    let volume = &status["status"]["volume"];
    Some((
        volume["level"].as_f64()?,
        volume["muted"].as_bool().unwrap_or(false),
    ))
}

/// Accepts the device's self-signed certificate; the handshake's signatures are still checked.
#[derive(Debug)]
struct AnyCertificate(Arc<rustls::crypto::CryptoProvider>);

impl rustls::client::danger::ServerCertVerifier for AnyCertificate {
    fn verify_server_cert(
        &self,
        _end_entity: &rustls::pki_types::CertificateDer<'_>,
        _intermediates: &[rustls::pki_types::CertificateDer<'_>],
        _server_name: &rustls::pki_types::ServerName<'_>,
        _ocsp_response: &[u8],
        _now: rustls::pki_types::UnixTime,
    ) -> Result<rustls::client::danger::ServerCertVerified, rustls::Error> {
        Ok(rustls::client::danger::ServerCertVerified::assertion())
    }

    fn verify_tls12_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls12_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn verify_tls13_signature(
        &self,
        message: &[u8],
        cert: &rustls::pki_types::CertificateDer<'_>,
        dss: &rustls::DigitallySignedStruct,
    ) -> Result<rustls::client::danger::HandshakeSignatureValid, rustls::Error> {
        rustls::crypto::verify_tls13_signature(
            message,
            cert,
            dss,
            &self.0.signature_verification_algorithms,
        )
    }

    fn supported_verify_schemes(&self) -> Vec<rustls::SignatureScheme> {
        self.0.signature_verification_algorithms.supported_schemes()
    }
}

type Tls = tokio_rustls::client::TlsStream<tokio::net::TcpStream>;

async fn connect(address: SocketAddr) -> Result<Tls, String> {
    let unreachable = || "The TV didn't answer. Check that it's on and on this network".to_owned();
    let tcp = tokio::time::timeout(CONNECT_TIMEOUT, tokio::net::TcpStream::connect(address))
        .await
        .map_err(|_| unreachable())?
        .map_err(|error| {
            tracing::warn!(%error, %address, "Cast connection failed");
            unreachable()
        })?;
    let _ = tcp.set_nodelay(true);
    let provider = Arc::new(rustls::crypto::ring::default_provider());
    let config = rustls::ClientConfig::builder_with_provider(provider.clone())
        .with_safe_default_protocol_versions()
        .map_err(|error| error.to_string())?
        .dangerous()
        .with_custom_certificate_verifier(Arc::new(AnyCertificate(provider)))
        .with_no_client_auth();
    let name = rustls::pki_types::ServerName::IpAddress(address.ip().into());
    tokio::time::timeout(
        CONNECT_TIMEOUT,
        tokio_rustls::TlsConnector::from(Arc::new(config)).connect(name, tcp),
    )
    .await
    .map_err(|_| unreachable())?
    .map_err(|error| {
        tracing::warn!(%error, %address, "Cast TLS handshake failed");
        "Couldn't connect to the TV".to_owned()
    })
}

/// Reads messages off the connection into `frames` until it closes. Its own task: a read cut
/// short by `select!` would lose the framing.
async fn read_frames(
    mut reader: impl AsyncRead + Unpin,
    frames: mpsc::UnboundedSender<(String, String, Value)>,
) {
    loop {
        let mut length = [0u8; 4];
        if reader.read_exact(&mut length).await.is_err() {
            return;
        }
        let length = u32::from_be_bytes(length) as usize;
        if length > MAX_MESSAGE {
            return;
        }
        let mut body = vec![0u8; length];
        if reader.read_exact(&mut body).await.is_err() {
            return;
        }
        if let Some(message) = decode(&body)
            && frames.send(message).is_err()
        {
            return;
        }
    }
}

struct Writer<W> {
    inner: W,
    request_id: u64,
}

impl<W: AsyncWrite + Unpin> Writer<W> {
    async fn send(
        &mut self,
        destination: &str,
        namespace: &str,
        payload: Value,
    ) -> Result<(), String> {
        self.inner
            .write_all(&encode(destination, namespace, &payload))
            .await
            .map_err(|_| super::LOST.to_owned())
    }

    /// Sends a request with the next request id.
    async fn request(
        &mut self,
        destination: &str,
        namespace: &str,
        mut payload: Value,
    ) -> Result<(), String> {
        self.request_id += 1;
        payload["requestId"] = json!(self.request_id);
        self.send(destination, namespace, payload).await
    }
}

/// The receiver app's ids, once it runs.
struct App {
    session_id: String,
    transport_id: String,
}

/// Plays `media` on the device at `address` and follows it until disconnected, the receiver app
/// closes, or the connection is lost.
pub async fn run(
    address: SocketAddr,
    media: CastMedia,
    commands: mpsc::UnboundedReceiver<Command>,
    report: Reporter,
) -> Result<(), String> {
    run_over(connect(address).await?, media, commands, report).await
}

/// [`run`] over a connection already made.
async fn run_over(
    stream: impl AsyncRead + AsyncWrite + Send + 'static,
    media: CastMedia,
    mut commands: mpsc::UnboundedReceiver<Command>,
    report: Reporter,
) -> Result<(), String> {
    let (reader, writer) = tokio::io::split(stream);
    let (frames_in, mut frames) = mpsc::unbounded_channel();
    let reading = tokio::spawn(read_frames(reader, frames_in));
    let mut writer = Writer {
        inner: writer,
        request_id: 0,
    };
    let result = follow(&mut writer, &mut frames, &mut commands, media, &report).await;
    reading.abort();
    result
}

async fn follow(
    writer: &mut Writer<impl AsyncWrite + Unpin>,
    frames: &mut mpsc::UnboundedReceiver<(String, String, Value)>,
    commands: &mut mpsc::UnboundedReceiver<Command>,
    media: CastMedia,
    report: &Reporter,
) -> Result<(), String> {
    writer
        .send(RECEIVER, CONNECTION, json!({ "type": "CONNECT" }))
        .await?;
    writer
        .request(
            RECEIVER,
            RECEIVER_NS,
            json!({ "type": "LAUNCH", "appId": DEFAULT_MEDIA_RECEIVER }),
        )
        .await?;
    let mut app: Option<App> = None;
    let mut current = media;
    // Loaded once the receiver app runs.
    let mut pending: Option<CastMedia> = Some(current.clone());
    let mut media_session: Option<i64> = None;
    let mut last_position = current.start_ms;
    let mut last_state = CastState::Connecting;
    let mut heard_at = Instant::now();
    let mut heartbeat = tokio::time::interval(HEARTBEAT_EVERY);
    heartbeat.tick().await;
    loop {
        tokio::select! {
            frame = frames.recv() => {
                let Some((source, namespace, payload)) = frame else {
                    return Err(super::LOST.to_owned());
                };
                heard_at = Instant::now();
                let kind = payload["type"].as_str().unwrap_or_default();
                match (namespace.as_str(), kind) {
                    (HEARTBEAT, "PING") => {
                        writer.send(&source, HEARTBEAT, json!({ "type": "PONG" })).await?;
                    }
                    (RECEIVER_NS, "RECEIVER_STATUS") => {
                        if let Some((level, muted)) = receiver_volume(&payload) {
                            report(Report { volume: Some(level), muted: Some(muted), ..Report::default() });
                        }
                        let running = running_app(&payload);
                        match (running, app.as_ref()) {
                            (Some((session_id, transport_id)), None) => {
                                writer.send(&transport_id, CONNECTION, json!({ "type": "CONNECT" })).await?;
                                if let Some(media) = pending.take() {
                                    writer.request(&transport_id, MEDIA, load_message(&media, &session_id, writer.request_id + 1)).await?;
                                    report(Report { state: Some(CastState::Loading), ..Report::default() });
                                }
                                app = Some(App { session_id, transport_id });
                            }
                            // Closed on the TV, or another phone started its own video.
                            (Some((session_id, _)), Some(ours)) if ours.session_id != session_id => {
                                report(Report { state: Some(CastState::Stopped), position_ms: Some(last_position), ..Report::default() });
                                return Ok(());
                            }
                            (None, Some(_)) => {
                                report(Report { state: Some(CastState::Stopped), position_ms: Some(last_position), ..Report::default() });
                                return Ok(());
                            }
                            _ => {}
                        }
                    }
                    (RECEIVER_NS, "LAUNCH_ERROR") => {
                        return Err("The TV couldn't open its video player".to_owned());
                    }
                    (MEDIA, "MEDIA_STATUS") => {
                        // About the video before the one loaded last.
                        let content = payload["status"][0]["media"]["contentId"].as_str();
                        if content.is_some_and(|content| content != current.url) {
                            continue;
                        }
                        if let Some((session, update)) = media_report(&payload) {
                            media_session = Some(session);
                            if let Some(position) = update.position_ms {
                                last_position = position;
                            }
                            if let Some(state) = update.state {
                                last_state = state;
                            }
                            report(Report { key: Some(current.key.clone()), ..update });
                        }
                    }
                    (MEDIA, "LOAD_FAILED" | "LOAD_CANCELLED" | "INVALID_REQUEST") => {
                        tracing::warn!(%payload, "the Cast receiver refused the video");
                        last_state = CastState::Failed;
                        report(Report {
                            state: Some(CastState::Failed),
                            error: Some(super::TV_FAILED.to_owned()),
                            key: Some(current.key.clone()),
                            ..Report::default()
                        });
                    }
                    (CONNECTION, "CLOSE") if app.as_ref().is_some_and(|app| app.transport_id == source) => {
                        report(Report { state: Some(CastState::Stopped), position_ms: Some(last_position), ..Report::default() });
                        return Ok(());
                    }
                    _ => {}
                }
            }
            command = commands.recv() => {
                let Some(app) = app.as_ref() else {
                    match command {
                        None | Some(Command::Disconnect) => return Ok(()),
                        Some(Command::Load(media)) => {
                            current = (*media).clone();
                            pending = Some(*media);
                        }
                        Some(Command::Control(_)) => {}
                    }
                    continue;
                };
                let transport = app.transport_id.clone();
                let with_session = |payload: Value| -> Option<Value> {
                    let mut payload = payload;
                    payload["mediaSessionId"] = json!(media_session?);
                    Some(payload)
                };
                match command {
                    None | Some(Command::Disconnect) => {
                        if let Some(stop) = with_session(json!({ "type": "STOP" })) {
                            let _ = writer.request(&transport, MEDIA, stop).await;
                        }
                        let session_id = app.session_id.clone();
                        let _ = writer.request(RECEIVER, RECEIVER_NS, json!({ "type": "STOP", "sessionId": session_id })).await;
                        let _ = writer.send(&transport, CONNECTION, json!({ "type": "CLOSE" })).await;
                        let _ = writer.send(RECEIVER, CONNECTION, json!({ "type": "CLOSE" })).await;
                        return Ok(());
                    }
                    Some(Command::Load(media)) => {
                        current = *media;
                        last_position = current.start_ms;
                        let load = load_message(&current, &app.session_id, writer.request_id + 1);
                        writer.request(&transport, MEDIA, load).await?;
                        report(Report { state: Some(CastState::Loading), key: Some(current.key.clone()), position_ms: Some(last_position), ..Report::default() });
                    }
                    Some(Command::Control(control)) => {
                        let ended = matches!(last_state, CastState::Stopped | CastState::Finished | CastState::Failed);
                        let message = match control {
                            // Stopped or finished: the video is loaded again, from where it was.
                            CastControl::Play if ended || media_session.is_none() => {
                                let mut again = current.clone();
                                again.start_ms = if last_state == CastState::Finished { 0.0 } else { last_position };
                                Some((transport.as_str(), MEDIA, load_message(&again, &app.session_id, writer.request_id + 1)))
                            }
                            CastControl::Play => with_session(json!({ "type": "PLAY" })).map(|payload| (transport.as_str(), MEDIA, payload)),
                            CastControl::Pause => with_session(json!({ "type": "PAUSE" })).map(|payload| (transport.as_str(), MEDIA, payload)),
                            CastControl::Stop => with_session(json!({ "type": "STOP" })).map(|payload| (transport.as_str(), MEDIA, payload)),
                            CastControl::Seek { position_ms } => {
                                last_position = position_ms.max(0.0);
                                report(Report { position_ms: Some(last_position), ..Report::default() });
                                with_session(json!({ "type": "SEEK", "currentTime": last_position / 1000.0 }))
                                    .map(|payload| (transport.as_str(), MEDIA, payload))
                            }
                            CastControl::Volume { level } => Some((
                                RECEIVER,
                                RECEIVER_NS,
                                json!({ "type": "SET_VOLUME", "volume": { "level": level.clamp(0.0, 1.0) } }),
                            )),
                            CastControl::Mute { muted } => Some((
                                RECEIVER,
                                RECEIVER_NS,
                                json!({ "type": "SET_VOLUME", "volume": { "muted": muted } }),
                            )),
                        };
                        if let Some((destination, namespace, payload)) = message {
                            writer.request(destination, namespace, payload).await?;
                        }
                    }
                }
            }
            _ = heartbeat.tick() => {
                if heard_at.elapsed() > SILENCE_LIMIT {
                    return Err(super::LOST.to_owned());
                }
                writer.send(RECEIVER, HEARTBEAT, json!({ "type": "PING" })).await?;
                // The position only comes with changes: asked for, to keep progress saved.
                if let (Some(app), Some(session)) = (app.as_ref(), media_session) {
                    let transport = app.transport_id.clone();
                    writer.request(&transport, MEDIA, json!({ "type": "GET_STATUS", "mediaSessionId": session })).await?;
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn messages_go_on_the_wire_as_cast_v2_frames() {
        let frame = encode(RECEIVER, CONNECTION, &json!({ "type": "CONNECT" }));
        let mut expected = vec![0, 0, 0, 0x58];
        expected.extend_from_slice(&[0x08, 0x00]);
        expected.extend_from_slice(&[0x12, 0x08]);
        expected.extend_from_slice(b"sender-0");
        expected.extend_from_slice(&[0x1a, 0x0a]);
        expected.extend_from_slice(b"receiver-0");
        expected.extend_from_slice(&[0x22, 0x28]);
        expected.extend_from_slice(CONNECTION.as_bytes());
        expected.extend_from_slice(&[0x28, 0x00]);
        expected.extend_from_slice(&[0x32, 0x12]);
        expected.extend_from_slice(br#"{"type":"CONNECT"}"#);
        assert_eq!(frame, expected);
        let (source, namespace, payload) = decode(&frame[4..]).unwrap();
        assert_eq!(
            (source.as_str(), namespace.as_str()),
            ("sender-0", CONNECTION)
        );
        assert_eq!(payload, json!({ "type": "CONNECT" }));
        assert_eq!(decode(b"\xff\xff"), None);
    }

    fn media() -> CastMedia {
        CastMedia {
            key: "stremio:movie:tt1".into(),
            url: "http://192.168.1.5:50000/cast/t/media/m/https/cdn.example/film.mp4".into(),
            content_type: "video/mp4".into(),
            title: "Sintel".into(),
            subtitle: None,
            poster: Some("https://images.example/sintel.jpg".into()),
            tracks: vec![super::super::Track {
                url: "http://192.168.1.5:50000/cast/t/subtitle/0.vtt".into(),
                language: Some("en".into()),
                label: "English".into(),
            }],
            active_track: Some(0),
            start_ms: 61_500.0,
        }
    }

    #[test]
    fn the_load_names_the_video_its_subtitles_and_where_to_start() {
        let load = load_message(&media(), "session-1", 7);
        assert_eq!(load["type"], "LOAD");
        assert_eq!(load["requestId"], 7);
        assert_eq!(load["sessionId"], "session-1");
        assert_eq!(load["media"]["contentId"], media().url);
        assert_eq!(load["media"]["contentType"], "video/mp4");
        assert_eq!(load["media"]["streamType"], "BUFFERED");
        assert_eq!(load["media"]["metadata"]["title"], "Sintel");
        assert_eq!(
            load["media"]["metadata"]["images"][0]["url"],
            "https://images.example/sintel.jpg"
        );
        assert_eq!(load["media"]["tracks"][0]["trackId"], 1);
        assert_eq!(load["media"]["tracks"][0]["trackContentType"], "text/vtt");
        assert_eq!(load["activeTrackIds"], json!([1]));
        assert_eq!(load["currentTime"], 61.5);
        assert_eq!(load["autoplay"], true);
    }

    #[test]
    fn statuses_become_reports() {
        let receiver = json!({
            "type": "RECEIVER_STATUS",
            "status": {
                "applications": [
                    { "appId": "E8C28D3C", "sessionId": "other", "transportId": "web-1" },
                    { "appId": "CC1AD845", "sessionId": "s-1", "transportId": "web-5" }
                ],
                "volume": { "level": 0.35, "muted": false }
            }
        });
        assert_eq!(running_app(&receiver), Some(("s-1".into(), "web-5".into())));
        assert_eq!(receiver_volume(&receiver), Some((0.35, false)));
        assert_eq!(
            running_app(&json!({ "status": { "applications": [] } })),
            None
        );

        let playing = json!({
            "type": "MEDIA_STATUS",
            "status": [{
                "mediaSessionId": 3, "playerState": "PLAYING", "currentTime": 12.5,
                "media": { "duration": 600.0 }
            }]
        });
        let (session, report) = media_report(&playing).unwrap();
        assert_eq!(session, 3);
        assert_eq!(report.state, Some(CastState::Playing));
        assert_eq!(report.position_ms, Some(12_500.0));
        assert_eq!(report.duration_ms, Some(600_000.0));

        let finished = json!({ "status": [{ "mediaSessionId": 3, "playerState": "IDLE", "idleReason": "FINISHED" }] });
        assert_eq!(
            media_report(&finished).unwrap().1.state,
            Some(CastState::Finished)
        );
        let failed = json!({ "status": [{ "mediaSessionId": 3, "playerState": "IDLE", "idleReason": "ERROR" }] });
        let failed = media_report(&failed).unwrap().1;
        assert_eq!(failed.state, Some(CastState::Failed));
        assert!(failed.error.is_some());
        let stopped = json!({ "status": [{ "mediaSessionId": 3, "playerState": "IDLE", "idleReason": "CANCELLED" }] });
        assert_eq!(
            media_report(&stopped).unwrap().1.state,
            Some(CastState::Stopped)
        );
        assert!(media_report(&json!({ "status": [] })).is_none());
    }

    /// Reads the next frame the sender wrote, as namespace, destination and JSON.
    /// The sender's own pings and status questions, which come every few seconds, are skipped.
    async fn next_frame(stream: &mut tokio::io::DuplexStream) -> (String, String, Value) {
        use prost::Message;
        loop {
            let mut length = [0u8; 4];
            stream.read_exact(&mut length).await.unwrap();
            let mut body = vec![0u8; u32::from_be_bytes(length) as usize];
            stream.read_exact(&mut body).await.unwrap();
            let message = CastMessage::decode(body.as_slice()).unwrap();
            let payload: Value =
                serde_json::from_str(message.payload_utf8.as_deref().unwrap()).unwrap();
            if matches!(payload["type"].as_str(), Some("PING" | "GET_STATUS")) {
                continue;
            }
            return (message.namespace, message.destination_id, payload);
        }
    }

    /// What the receiver sends, from `source`.
    async fn send(
        stream: &mut tokio::io::DuplexStream,
        source: &str,
        namespace: &str,
        payload: Value,
    ) {
        use prost::Message;
        let body = CastMessage {
            protocol_version: 0,
            source_id: source.into(),
            destination_id: SENDER.into(),
            namespace: namespace.into(),
            payload_type: 0,
            payload_utf8: Some(payload.to_string()),
            payload_binary: None,
        }
        .encode_to_vec();
        stream
            .write_all(&(body.len() as u32).to_be_bytes())
            .await
            .unwrap();
        stream.write_all(&body).await.unwrap();
    }

    #[tokio::test]
    async fn a_receiver_is_launched_loaded_controlled_and_stopped() {
        let (ours, mut tv) = tokio::io::duplex(1 << 16);
        let reports = Arc::new(std::sync::Mutex::new(Vec::new()));
        let heard = reports.clone();
        let reporter: Reporter = Arc::new(move |report| heard.lock().unwrap().push(report));
        let (commands, receiver) = mpsc::unbounded_channel();
        let running = tokio::spawn(run_over(ours, media(), receiver, reporter));

        let (namespace, destination, payload) = next_frame(&mut tv).await;
        assert_eq!(
            (namespace.as_str(), destination.as_str()),
            (CONNECTION, RECEIVER)
        );
        assert_eq!(payload["type"], "CONNECT");
        let (namespace, _, launch) = next_frame(&mut tv).await;
        assert_eq!(namespace, RECEIVER_NS);
        assert_eq!(launch["type"], "LAUNCH");
        assert_eq!(launch["appId"], "CC1AD845");

        send(&mut tv, RECEIVER, HEARTBEAT, json!({ "type": "PING" })).await;
        let (namespace, _, pong) = next_frame(&mut tv).await;
        assert_eq!(
            (namespace.as_str(), pong["type"].as_str()),
            (HEARTBEAT, Some("PONG"))
        );

        let status = json!({
            "type": "RECEIVER_STATUS",
            "status": {
                "applications": [{ "appId": "CC1AD845", "sessionId": "s-1", "transportId": "web-5" }],
                "volume": { "level": 0.4, "muted": false }
            }
        });
        send(&mut tv, RECEIVER, RECEIVER_NS, status).await;
        let (namespace, destination, connect) = next_frame(&mut tv).await;
        assert_eq!(
            (namespace.as_str(), destination.as_str()),
            (CONNECTION, "web-5")
        );
        assert_eq!(connect["type"], "CONNECT");
        let (namespace, destination, load) = next_frame(&mut tv).await;
        assert_eq!((namespace.as_str(), destination.as_str()), (MEDIA, "web-5"));
        assert_eq!(load["type"], "LOAD");
        assert_eq!(load["sessionId"], "s-1");
        assert_eq!(load["currentTime"], 61.5);

        let playing = json!({
            "type": "MEDIA_STATUS",
            "status": [{ "mediaSessionId": 9, "playerState": "PLAYING", "currentTime": 62.0,
                "media": { "contentId": media().url, "duration": 888.0 } }]
        });
        send(&mut tv, "web-5", MEDIA, playing).await;
        // Statuses about another video are left out.
        let other = json!({
            "type": "MEDIA_STATUS",
            "status": [{ "mediaSessionId": 8, "playerState": "IDLE", "idleReason": "FINISHED",
                "media": { "contentId": "http://elsewhere/old.mp4" } }]
        });
        send(&mut tv, "web-5", MEDIA, other).await;

        commands
            .send(Command::Control(CastControl::Seek {
                position_ms: 120_000.0,
            }))
            .unwrap();
        let (namespace, _, seek) = next_frame(&mut tv).await;
        assert_eq!(namespace, MEDIA);
        assert_eq!(seek["type"], "SEEK");
        assert_eq!(seek["mediaSessionId"], 9);
        assert_eq!(seek["currentTime"], 120.0);
        commands
            .send(Command::Control(CastControl::Volume { level: 0.8 }))
            .unwrap();
        let (namespace, destination, volume) = next_frame(&mut tv).await;
        assert_eq!(
            (namespace.as_str(), destination.as_str()),
            (RECEIVER_NS, RECEIVER)
        );
        assert_eq!(volume["volume"]["level"], 0.8);

        commands.send(Command::Disconnect).unwrap();
        let stop = next_frame(&mut tv).await;
        assert_eq!(
            (stop.0.as_str(), stop.2["type"].as_str()),
            (MEDIA, Some("STOP"))
        );
        let close = next_frame(&mut tv).await;
        assert_eq!(
            (close.0.as_str(), close.2["type"].as_str()),
            (RECEIVER_NS, Some("STOP"))
        );
        assert_eq!(close.2["sessionId"], "s-1");
        running.await.unwrap().unwrap();

        let reports = reports.lock().unwrap();
        assert!(reports.iter().any(|report| report.volume == Some(0.4)));
        let played = reports
            .iter()
            .find(|report| report.state == Some(CastState::Playing))
            .unwrap();
        assert_eq!(played.position_ms, Some(62_000.0));
        assert_eq!(played.duration_ms, Some(888_000.0));
        assert!(
            !reports
                .iter()
                .any(|report| report.state == Some(CastState::Finished))
        );
    }

    #[test]
    fn speakers_are_left_out() {
        assert!(has_screen(Some("4101")));
        assert!(has_screen(Some("5")));
        assert!(!has_screen(Some("2052")), "a speaker");
        assert!(!has_screen(Some("2084")), "a speaker group");
        assert!(has_screen(None));
    }
}
