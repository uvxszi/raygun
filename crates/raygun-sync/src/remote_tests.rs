use std::sync::Arc;

use raygun_db::Db;
use raygun_db::sync::{Peer, SyncKind};
use serde_json::json;
use tempfile::TempDir;
use tokio::io::{AsyncRead, AsyncWrite};

use super::*;
use crate::Identity;

/// The page's end of a back-end channel, as `apps/raygun/src/web/remote/channel.ts` speaks it.
pub(crate) struct Page<S> {
    stream: S,
    transport: Mutex<TransportState>,
}

/// One thing the app sent: a message, or a piece of a body.
#[derive(Debug, PartialEq)]
pub(crate) enum Received {
    Json(Value),
    Body(u32, Vec<u8>),
}

impl<S: AsyncRead + AsyncWrite + Unpin + Send> Page<S> {
    pub(crate) async fn open(stream: S, identity: &Identity) -> (Self, Vec<u8>) {
        let (channel, remote) = Channel::initiate(stream, identity).await.unwrap();
        let (stream, transport) = channel.into_parts();
        (
            Self {
                stream,
                transport: Mutex::new(transport),
            },
            remote,
        )
    }

    pub(crate) async fn send(&mut self, value: Value) {
        for frame in frames(&self.transport, &Outgoing::Json(value)).unwrap() {
            channel::write_frame(&mut self.stream, &frame)
                .await
                .unwrap();
        }
    }

    pub(crate) async fn recv(&mut self) -> Received {
        let mut message = Vec::new();
        loop {
            let frame = channel::read_frame(&mut self.stream).await.unwrap();
            let mut buffer = vec![0u8; MAX_NOISE];
            let length = self
                .transport
                .lock()
                .unwrap()
                .read_message(&frame, &mut buffer)
                .unwrap();
            let plain = &buffer[..length];
            match plain[0] {
                KIND_BODY => {
                    let id = u32::from_be_bytes(plain[1..5].try_into().unwrap());
                    return Received::Body(id, plain[5..].to_vec());
                }
                kind => {
                    message.extend_from_slice(&plain[1..]);
                    if kind == KIND_LAST {
                        return Received::Json(serde_json::from_slice(&message).unwrap());
                    }
                }
            }
        }
    }
}

async fn app(temp: &TempDir) -> Arc<Engine> {
    let db = Arc::new(Db::open(&temp.path().join("app.db")).await.unwrap());
    Arc::new(Engine::new(
        db,
        Identity::generate().unwrap(),
        "Laptop".into(),
        SyncKind::ALL.to_vec(),
    ))
}

pub(crate) async fn link_browser(engine: &Engine, identity: &Identity) {
    sync::save_peer(
        &engine.db,
        &Peer {
            device_id: "browser".into(),
            name: "Raygun in a browser".into(),
            public_key: to_hex(&identity.public),
            address: None,
            paired_at: 1,
            last_sync_at: None,
            sent_seq: 0,
            kinds: Vec::new(),
        },
    )
    .await
    .unwrap();
}

/// Answers every message with an echo, and a request for a body with one bigger than a frame.
pub(crate) fn echo() -> RemoteHandler {
    Arc::new(|mut session: RemoteSession| {
        tokio::spawn(async move {
            let _ = session.outgoing.send(Outgoing::Json(json!({
                "t": "welcome",
                "peer": session.peer.name,
            })));
            while let Some(message) = session.incoming.recv().await {
                if message["t"] == "fetch" {
                    let id = message["id"].as_u64().unwrap() as u32;
                    let size = message["size"]
                        .as_u64()
                        .map_or(BODY_PART + 10, |size| size as usize);
                    let bytes: Vec<u8> = (0..size).map(|at| at as u8).collect();
                    let _ = session.outgoing.send(Outgoing::Body { id, bytes });
                    let _ = session
                        .outgoing
                        .send(Outgoing::Json(json!({ "t": "end", "id": id })));
                } else {
                    let _ = session
                        .outgoing
                        .send(Outgoing::Json(json!({ "t": "echo", "got": message })));
                }
            }
        });
    })
}

#[tokio::test]
async fn a_linked_browser_talks_both_ways() {
    let temp = TempDir::new().unwrap();
    let engine = app(&temp).await;
    engine.on_remote(Some(echo()));
    let browser = Identity::generate().unwrap();
    link_browser(&engine, &browser).await;

    let (ours, theirs) = tokio::io::duplex(256 * 1024);
    let serving = engine.clone();
    tokio::spawn(async move { serving.handle_remote(theirs).await });
    let (mut page, key) = Page::open(ours, &browser).await;
    assert_eq!(key, engine.identity().public);
    assert_eq!(
        page.recv().await,
        Received::Json(json!({ "t": "welcome", "peer": "Raygun in a browser" }))
    );

    // A message bigger than a frame arrives whole.
    let long = "x".repeat(FRAME_PLAIN * 2);
    page.send(json!({ "t": "call", "id": 1, "args": { "text": long } }))
        .await;
    let Received::Json(echoed) = page.recv().await else {
        panic!("expected a message")
    };
    assert_eq!(echoed["got"]["args"]["text"], long.as_str());

    // A body comes in pieces with its request's id, then its end.
    page.send(json!({ "t": "fetch", "id": 7 })).await;
    let mut body = Vec::new();
    loop {
        match page.recv().await {
            Received::Body(7, bytes) => body.extend(bytes),
            Received::Json(end) => {
                assert_eq!(end, json!({ "t": "end", "id": 7 }));
                break;
            }
            other => panic!("unexpected {other:?}"),
        }
    }
    assert_eq!(body.len(), BODY_PART + 10);
    assert_eq!(body[BODY_PART + 9], ((BODY_PART + 9) % 256) as u8);
}

#[tokio::test]
async fn browsers_not_linked_are_refused() {
    let temp = TempDir::new().unwrap();
    let engine = app(&temp).await;
    engine.on_remote(Some(echo()));
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let serving = engine.clone();
    let served = tokio::spawn(async move { serving.handle_remote(theirs).await });
    let (mut page, _) = Page::open(ours, &Identity::generate().unwrap()).await;
    let Received::Json(refused) = page.recv().await else {
        panic!("expected a message")
    };
    assert_eq!(refused["t"], "refused");
    assert!(matches!(served.await.unwrap(), Err(SyncError::Refused(_))));
}

#[tokio::test]
async fn without_a_handler_linked_browsers_are_refused() {
    let temp = TempDir::new().unwrap();
    let engine = app(&temp).await;
    let browser = Identity::generate().unwrap();
    link_browser(&engine, &browser).await;
    let (ours, theirs) = tokio::io::duplex(64 * 1024);
    let serving = engine.clone();
    tokio::spawn(async move { serving.handle_remote(theirs).await });
    let (mut page, _) = Page::open(ours, &browser).await;
    let Received::Json(refused) = page.recv().await else {
        panic!("expected a message")
    };
    assert_eq!(refused["reason"], "This device can't answer for browsers.");
}
