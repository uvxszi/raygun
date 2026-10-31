//! An encrypted, mutually authenticated channel between two devices: a Noise `XX` handshake over
//! TCP, then JSON messages in length-prefixed Noise frames. After the handshake each side knows
//! the other's static key, which is how paired devices recognise each other.

use std::time::Duration;

use snow::{Builder, TransportState};
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::SyncError;
use crate::protocol::Message;

const PARAMS: &str = "Noise_XX_25519_ChaChaPoly_BLAKE2s";
/// Noise messages are at most this long.
const MAX_NOISE: usize = 65_535;
/// Plaintext per frame, leaving room for the flag byte and the 16-byte tag.
const CHUNK: usize = 60_000;
/// Largest message accepted, against a peer that never ends one.
const MAX_MESSAGE: usize = 64 * 1024 * 1024;
/// A peer that goes quiet this long is gone.
pub const IO_TIMEOUT: Duration = Duration::from_secs(30);

/// A device's long-term X25519 key pair, which is also its identity.
#[derive(Clone, PartialEq, Eq)]
pub struct Identity {
    pub private: Vec<u8>,
    pub public: Vec<u8>,
}

impl std::fmt::Debug for Identity {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(formatter, "Identity({})", fingerprint(&self.public))
    }
}

impl Identity {
    pub fn generate() -> Result<Self, SyncError> {
        let keypair = Builder::new(params()).generate_keypair()?;
        Ok(Self {
            private: keypair.private,
            public: keypair.public,
        })
    }

    pub fn from_hex(private: &str, public: &str) -> Option<Self> {
        Some(Self {
            private: from_hex(private).filter(|key| key.len() == 32)?,
            public: from_hex(public).filter(|key| key.len() == 32)?,
        })
    }
}

/// The first 8 bytes of a public key, as four groups of hex: easy to compare by eye.
pub fn fingerprint(public: &[u8]) -> String {
    let hex = to_hex(&public[..public.len().min(8)]);
    hex.as_bytes()
        .chunks(4)
        .map(|group| String::from_utf8_lossy(group).into_owned())
        .collect::<Vec<_>>()
        .join(" ")
}

pub fn to_hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

pub fn from_hex(text: &str) -> Option<Vec<u8>> {
    if !text.len().is_multiple_of(2) {
        return None;
    }
    (0..text.len())
        .step_by(2)
        .map(|index| u8::from_str_radix(text.get(index..index + 2)?, 16).ok())
        .collect()
}

fn params() -> snow::params::NoiseParams {
    PARAMS.parse().expect("the Noise pattern name is valid")
}

pub struct Channel {
    stream: TcpStream,
    transport: TransportState,
    buffer: Vec<u8>,
}

impl Channel {
    /// Connects as the initiator. Returns the channel and the responder's static key.
    pub async fn initiate(
        mut stream: TcpStream,
        identity: &Identity,
    ) -> Result<(Self, Vec<u8>), SyncError> {
        let mut handshake = Builder::new(params())
            .local_private_key(&identity.private)?
            .build_initiator()?;
        let mut buffer = vec![0u8; MAX_NOISE];
        let length = handshake.write_message(&[], &mut buffer)?;
        write_frame(&mut stream, &buffer[..length]).await?;
        let reply = read_frame(&mut stream).await?;
        handshake.read_message(&reply, &mut buffer)?;
        let length = handshake.write_message(&[], &mut buffer)?;
        write_frame(&mut stream, &buffer[..length]).await?;
        let remote = handshake
            .get_remote_static()
            .ok_or_else(|| SyncError::Protocol("the other device sent no key".into()))?
            .to_vec();
        let transport = handshake.into_transport_mode()?;
        Ok((
            Self {
                stream,
                transport,
                buffer,
            },
            remote,
        ))
    }

    /// Accepts as the responder. Returns the channel and the initiator's static key.
    pub async fn respond(
        mut stream: TcpStream,
        identity: &Identity,
    ) -> Result<(Self, Vec<u8>), SyncError> {
        let mut handshake = Builder::new(params())
            .local_private_key(&identity.private)?
            .build_responder()?;
        let mut buffer = vec![0u8; MAX_NOISE];
        let first = read_frame(&mut stream).await?;
        handshake.read_message(&first, &mut buffer)?;
        let length = handshake.write_message(&[], &mut buffer)?;
        write_frame(&mut stream, &buffer[..length]).await?;
        let last = read_frame(&mut stream).await?;
        handshake.read_message(&last, &mut buffer)?;
        let remote = handshake
            .get_remote_static()
            .ok_or_else(|| SyncError::Protocol("the other device sent no key".into()))?
            .to_vec();
        let transport = handshake.into_transport_mode()?;
        Ok((
            Self {
                stream,
                transport,
                buffer,
            },
            remote,
        ))
    }

    /// Sends one message, split into as many frames as it needs.
    pub async fn send(&mut self, message: &Message) -> Result<(), SyncError> {
        let bytes =
            serde_json::to_vec(message).map_err(|error| SyncError::Protocol(error.to_string()))?;
        let chunks: Vec<&[u8]> = bytes.chunks(CHUNK).collect();
        for (index, chunk) in chunks.iter().enumerate() {
            let mut plain = Vec::with_capacity(chunk.len() + 1);
            plain.push(u8::from(index + 1 < chunks.len()));
            plain.extend_from_slice(chunk);
            let length = self.transport.write_message(&plain, &mut self.buffer)?;
            write_frame(&mut self.stream, &self.buffer[..length]).await?;
        }
        Ok(())
    }

    pub async fn recv(&mut self) -> Result<Message, SyncError> {
        let mut message = Vec::new();
        loop {
            let frame = read_frame(&mut self.stream).await?;
            let length = self.transport.read_message(&frame, &mut self.buffer)?;
            let Some((&more, data)) = self.buffer[..length].split_first() else {
                return Err(SyncError::Protocol("empty frame".into()));
            };
            message.extend_from_slice(data);
            if message.len() > MAX_MESSAGE {
                return Err(SyncError::Protocol("message too large".into()));
            }
            if more == 0 {
                break;
            }
        }
        serde_json::from_slice(&message).map_err(|error| SyncError::Protocol(error.to_string()))
    }
}

async fn write_frame(stream: &mut TcpStream, data: &[u8]) -> Result<(), SyncError> {
    let length =
        u16::try_from(data.len()).map_err(|_| SyncError::Protocol("frame too large".into()))?;
    tokio::time::timeout(IO_TIMEOUT, async {
        stream.write_u16(length).await?;
        stream.write_all(data).await?;
        stream.flush().await
    })
    .await
    .map_err(|_| SyncError::Timeout)??;
    Ok(())
}

async fn read_frame(stream: &mut TcpStream) -> Result<Vec<u8>, SyncError> {
    tokio::time::timeout(IO_TIMEOUT, async {
        let length = stream.read_u16().await? as usize;
        let mut data = vec![0u8; length];
        stream.read_exact(&mut data).await?;
        Ok::<_, std::io::Error>(data)
    })
    .await
    .map_err(|_| SyncError::Timeout)?
    .map_err(SyncError::from)
}
