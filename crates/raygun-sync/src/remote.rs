//! Raygun on the web using a linked app as its back end (docs/WEB.md, "The app as a back end").
//!
//! A browser opens a second data channel beside sync's (`web`, channel 1) and runs the same Noise
//! `XX` handshake on it. The app answers only a browser whose key belongs to a paired device; then
//! the channel stays open both ways: the page sends JSON requests (commands, picture and media
//! pieces), the app sends answers, events and bodies. What the messages mean is the app's
//! (`raygun-app` `web_remote`); this module only carries them.
//!
//! Each Noise frame's plaintext starts with a kind byte: [`KIND_MORE`] and [`KIND_LAST`] carry a
//! JSON message in parts, [`KIND_BODY`] a piece of a response body (a 4-byte request id, then
//! bytes). One writer sends whole messages, so parts of two messages never mix.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_db::sync::{self, Peer};
use serde_json::Value;
use snow::TransportState;
use tokio::io::{AsyncRead, AsyncReadExt, AsyncWrite};
use tokio::sync::mpsc;

use crate::channel::{self, Channel, MAX_NOISE};
use crate::{Engine, SyncError, to_hex};

/// A JSON message's part, more to come.
pub const KIND_MORE: u8 = 0;
/// A JSON message's last part.
pub const KIND_LAST: u8 = 1;
/// A piece of a body: a big-endian `u32` request id, then the bytes.
pub const KIND_BODY: u8 = 2;
/// Plaintext per frame, kind byte included (a Noise message holds 65,535 bytes with its tag).
pub const FRAME_PLAIN: usize = 60_000;
/// Body bytes per frame.
pub const BODY_PART: usize = FRAME_PLAIN - 5;
/// Largest JSON message taken from a browser.
const MAX_MESSAGE: usize = 16 * 1024 * 1024;
/// A browser that sends nothing this long is gone (pages ping every 20 s).
const QUIET: Duration = Duration::from_secs(75);

/// What the app sends a browser.
#[derive(Debug, Clone, PartialEq)]
pub enum Outgoing {
    Json(Value),
    Body { id: u32, bytes: Vec<u8> },
}

/// A paired browser's open channel, handed to the app's [`RemoteHandler`]. The session ends when
/// `incoming` ends; sending afterwards does nothing.
pub struct RemoteSession {
    pub peer: Peer,
    pub incoming: mpsc::UnboundedReceiver<Value>,
    pub outgoing: mpsc::UnboundedSender<Outgoing>,
}

/// Takes each paired browser's channel once it's open.
pub type RemoteHandler = Arc<dyn Fn(RemoteSession) + Send + Sync>;

impl Engine {
    /// Lets paired browsers use this device as their back end through `handler` (None: they're
    /// refused).
    pub fn on_remote(&self, handler: Option<RemoteHandler>) {
        *self.remote.write().expect("remote lock poisoned") = handler;
    }

    /// One browser's back-end channel: authenticates it, then carries messages until either side
    /// goes away.
    pub(crate) async fn handle_remote<S>(&self, stream: S) -> Result<(), SyncError>
    where
        S: AsyncRead + AsyncWrite + Unpin + Send + 'static,
    {
        let (channel, remote) = Channel::respond(stream, &self.identity).await?;
        let (stream, transport) = channel.into_parts();
        let transport = Arc::new(Mutex::new(transport));
        let (mut reader, mut writer) = tokio::io::split(stream);
        let key = to_hex(&remote);
        let peer = sync::peers(&self.db)
            .await?
            .into_iter()
            .find(|peer| peer.public_key == key);
        let handler = self.remote.read().expect("remote lock poisoned").clone();
        let (peer, handler) = match (peer, handler) {
            (Some(peer), Some(handler)) => (peer, handler),
            (peer, _) => {
                let reason = if peer.is_none() {
                    "This browser isn't linked with this device. Link it again."
                } else {
                    "This device can't answer for browsers."
                };
                let message = serde_json::json!({ "t": "refused", "reason": reason });
                for frame in frames(&transport, &Outgoing::Json(message))? {
                    channel::write_frame(&mut writer, &frame).await?;
                }
                return Err(SyncError::Refused(reason.into()));
            }
        };
        tracing::info!(peer = %peer.name, "a browser uses this device");
        let (outgoing, mut sending) = mpsc::unbounded_channel::<Outgoing>();
        let (received, incoming) = mpsc::unbounded_channel::<Value>();
        handler(RemoteSession {
            peer,
            incoming,
            outgoing,
        });
        let keys = transport.clone();
        let writing = tokio::spawn(async move {
            while let Some(item) = sending.recv().await {
                let Ok(frames) = frames(&keys, &item) else {
                    break;
                };
                for frame in frames {
                    if channel::write_frame(&mut writer, &frame).await.is_err() {
                        return;
                    }
                }
            }
        });
        let result = async {
            let mut buffer = vec![0u8; MAX_NOISE];
            let mut message = Vec::new();
            loop {
                let frame = tokio::time::timeout(QUIET, read_frame(&mut reader))
                    .await
                    .map_err(|_| SyncError::Timeout)??;
                let length = transport
                    .lock()
                    .expect("keys lock poisoned")
                    .read_message(&frame, &mut buffer)?;
                let Some((&kind, data)) = buffer[..length].split_first() else {
                    return Err(SyncError::Protocol("empty frame".into()));
                };
                match kind {
                    KIND_MORE | KIND_LAST => {
                        message.extend_from_slice(data);
                        if message.len() > MAX_MESSAGE {
                            return Err(SyncError::Protocol("message too large".into()));
                        }
                        if kind == KIND_LAST {
                            let value = serde_json::from_slice(&std::mem::take(&mut message))
                                .map_err(|error| SyncError::Protocol(error.to_string()))?;
                            if received.send(value).is_err() {
                                return Ok(());
                            }
                        }
                    }
                    // Browsers send no bodies; anything else is from a newer page.
                    _ => {}
                }
            }
        }
        .await;
        writing.abort();
        result
    }
}

/// Reads one length-prefixed frame, however long the browser takes (the caller times it).
async fn read_frame(stream: &mut (impl AsyncRead + Unpin)) -> Result<Vec<u8>, SyncError> {
    let length = stream.read_u16().await? as usize;
    let mut data = vec![0u8; length];
    stream.read_exact(&mut data).await?;
    Ok(data)
}

/// The encrypted frames of one message, in order.
pub(crate) fn frames(
    transport: &Mutex<TransportState>,
    item: &Outgoing,
) -> Result<Vec<Vec<u8>>, SyncError> {
    let plains: Vec<Vec<u8>> = match item {
        Outgoing::Json(value) => {
            let bytes = serde_json::to_vec(value)
                .map_err(|error| SyncError::Protocol(error.to_string()))?;
            let parts: Vec<&[u8]> = if bytes.is_empty() {
                vec![&[]]
            } else {
                bytes.chunks(FRAME_PLAIN - 1).collect()
            };
            let last = parts.len() - 1;
            parts
                .into_iter()
                .enumerate()
                .map(|(index, part)| {
                    let mut plain = Vec::with_capacity(part.len() + 1);
                    plain.push(if index == last { KIND_LAST } else { KIND_MORE });
                    plain.extend_from_slice(part);
                    plain
                })
                .collect()
        }
        Outgoing::Body { id, bytes } => bytes
            .chunks(BODY_PART)
            .map(|part| {
                let mut plain = Vec::with_capacity(part.len() + 5);
                plain.push(KIND_BODY);
                plain.extend_from_slice(&id.to_be_bytes());
                plain.extend_from_slice(part);
                plain
            })
            .collect(),
    };
    let mut transport = transport.lock().expect("keys lock poisoned");
    let mut buffer = vec![0u8; MAX_NOISE];
    plains
        .iter()
        .map(|plain| {
            let length = transport.write_message(plain, &mut buffer)?;
            Ok(buffer[..length].to_vec())
        })
        .collect()
}

#[cfg(test)]
#[path = "remote_tests.rs"]
pub(crate) mod tests;
