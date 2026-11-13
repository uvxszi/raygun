use std::sync::atomic::{AtomicI64, Ordering};

use raygun_db::Db;
use raygun_db::collections::{self, TrackFields};
use raygun_db::sync::{self, SyncKind};
use tempfile::TempDir;
use tokio::io::{AsyncRead, AsyncWrite};

use super::*;
use crate::Identity;
use crate::channel::Channel;
use crate::protocol::Message;

/// A STUN binding request with a USERNAME, as a browser sends it.
fn binding_request(username: &str) -> Vec<u8> {
    let mut attribute = Vec::new();
    attribute.extend_from_slice(&STUN_USERNAME.to_be_bytes());
    attribute.extend_from_slice(&(username.len() as u16).to_be_bytes());
    attribute.extend_from_slice(username.as_bytes());
    while attribute.len() % 4 != 0 {
        attribute.push(0);
    }
    // An attribute after USERNAME, as browsers add priority and the like.
    attribute.extend_from_slice(&[0x00, 0x24, 0x00, 0x04, 1, 2, 3, 4]);
    let mut message = Vec::new();
    message.extend_from_slice(&STUN_BINDING_REQUEST);
    message.extend_from_slice(&(attribute.len() as u16).to_be_bytes());
    message.extend_from_slice(&STUN_MAGIC);
    message.extend_from_slice(&[7; 12]);
    message.extend_from_slice(&attribute);
    message
}

#[test]
fn binding_requests_name_the_browser() {
    assert_eq!(
        binding_request_ufrag(&binding_request("ours1234:theirs"), "ours1234").as_deref(),
        Some("theirs")
    );
    for (packet, why) in [
        (binding_request("other:theirs"), "for another listener"),
        (binding_request("ours1234:"), "without the browser's part"),
        (binding_request("ours1234"), "without a colon"),
        (vec![0x16, 0xfe, 0xfd, 0, 0], "DTLS, not STUN"),
        (
            binding_request("ours1234:theirs")[..30].to_vec(),
            "cut short",
        ),
    ] {
        assert_eq!(binding_request_ufrag(&packet, "ours1234"), None, "{why}");
    }
}

#[test]
fn configs_keep_and_read_back() {
    let config = WebConfig::generate().unwrap();
    assert_eq!(config.ufrag.len(), 8);
    assert_eq!(config.pwd.len(), 24);
    assert_eq!(config.fingerprint().len(), 64);
    let stored = config.to_stored();
    assert_eq!(WebConfig::from_stored(&stored), Some(config.clone()));
    assert_eq!(WebConfig::from_stored("2:a:b:00:00"), None);
    assert_eq!(WebConfig::from_stored(&stored.replace(':', ";")), None);
    assert_ne!(
        WebConfig::generate().unwrap().fingerprint(),
        config.fingerprint()
    );
}

/// A browser stand-in: a full ICE agent that connects to the listener the way a page's
/// RTCPeerConnection does (controlling, DTLS client, SCTP client, a negotiated channel: 0 for sync,
/// 1 for the back end) and hands back the channel as a byte stream.
async fn browser(
    server: &WebRunning,
    config: &WebConfig,
    channel_id: u16,
) -> impl AsyncRead + AsyncWrite + Unpin + Send + 'static {
    let socket = Arc::new(UdpSocket::bind((Ipv4Addr::LOCALHOST, 0)).await.unwrap());
    let local = socket.local_addr().unwrap();
    let remote = SocketAddr::new(IpAddr::V4(Ipv4Addr::LOCALHOST), server.port);
    let now = Instant::now();
    let mut rtc = Rtc::builder()
        .set_crypto_provider(provider())
        .set_local_ice_credentials(IceCreds {
            ufrag: "brow".into(),
            pass: "browserpasswordbrowserpassword".into(),
        })
        .build(now);
    rtc.add_local_candidate(Candidate::host(local, "udp").unwrap());
    rtc.add_remote_candidate(Candidate::host(remote, "udp").unwrap());
    let mut api = rtc.direct_api();
    api.set_ice_controlling(true);
    api.set_remote_ice_credentials(IceCreds {
        ufrag: config.ufrag.clone(),
        pass: config.pwd.clone(),
    });
    api.set_remote_fingerprint(Fingerprint {
        hash_func: "sha-256".into(),
        bytes: crate::channel::from_hex(&config.fingerprint()).unwrap(),
    });
    api.start_dtls(true).unwrap();
    api.start_sctp(true);
    let channel = api.create_data_channel(ChannelConfig {
        label: "raygun-sync".into(),
        negotiated: Some(channel_id),
        ..Default::default()
    });

    let (ours, theirs) = tokio::io::duplex(256 * 1024);
    let (mut reader, mut writer) = tokio::io::split(ours);
    let (to_page, mut outgoing) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        let mut buffer = vec![0u8; CHUNK];
        while let Ok(size) = reader.read(&mut buffer).await {
            if size == 0 || to_page.send(buffer[..size].to_vec()).is_err() {
                break;
            }
        }
    });
    let (from_page, mut incoming) = mpsc::unbounded_channel::<Vec<u8>>();
    tokio::spawn(async move {
        while let Some(bytes) = incoming.recv().await {
            if writer.write_all(&bytes).await.is_err() {
                break;
            }
        }
    });
    tokio::spawn(async move {
        let mut buffer = vec![0u8; 2048];
        let mut open = false;
        let mut waiting: Vec<Vec<u8>> = Vec::new();
        loop {
            let next = loop {
                match rtc.poll_output().unwrap() {
                    Output::Timeout(next) => break next,
                    Output::Transmit(transmit) => {
                        let _ = socket.try_send_to(&transmit.contents, transmit.destination);
                    }
                    Output::Event(Event::ChannelOpen(..)) => open = true,
                    Output::Event(Event::ChannelData(data)) => {
                        let _ = from_page.send(data.data);
                    }
                    Output::Event(_) => {}
                }
            };
            if open && !waiting.is_empty() {
                for bytes in std::mem::take(&mut waiting) {
                    rtc.channel(channel).unwrap().write(true, &bytes).unwrap();
                }
                continue;
            }
            let wait = next.saturating_duration_since(Instant::now());
            tokio::select! {
                received = socket.recv_from(&mut buffer) => {
                    let Ok((size, source)) = received else { break };
                    let input = Input::Receive(Instant::now(), Receive {
                        proto: Protocol::Udp,
                        source,
                        destination: local,
                        contents: buffer[..size].try_into().unwrap(),
                    });
                    rtc.handle_input(input).unwrap();
                }
                bytes = outgoing.recv() => {
                    let Some(bytes) = bytes else { break };
                    if open {
                        rtc.channel(channel).unwrap().write(true, &bytes).unwrap();
                    } else {
                        waiting.push(bytes);
                    }
                }
                _ = tokio::time::sleep(wait.max(Duration::from_millis(1))) => {
                    rtc.handle_input(Input::Timeout(Instant::now())).unwrap();
                }
            }
        }
    });
    theirs
}

#[tokio::test]
async fn a_browser_links_over_webrtc_and_syncs() {
    let temp = TempDir::new().unwrap();
    let db = Arc::new(Db::open(&temp.path().join("phone.db")).await.unwrap());
    let clock = Arc::new(AtomicI64::new(1_000));
    let time = clock.clone();
    let phone = Arc::new(Engine::with_clock(
        db.clone(),
        Identity::generate().unwrap(),
        "Phone".into(),
        SyncKind::ALL.to_vec(),
        Box::new(move || time.load(Ordering::SeqCst)),
    ));
    let song = TrackFields {
        track_key: "artist|song".into(),
        title: "Song".into(),
        artist: Some("Artist".into()),
        album: None,
        duration_ms: Some(1000),
        artwork_url: None,
        source: None,
    };
    collections::set_favorite(&db, &song, true, 5)
        .await
        .unwrap();
    // The TCP server gives the engine a port, which pairing codes need.
    let _tcp = phone
        .serve_on(IpAddr::V4(Ipv4Addr::LOCALHOST), 0..=0)
        .await
        .unwrap();
    phone.open_pairing(IpAddr::V4(Ipv4Addr::LOCALHOST)).unwrap();
    let config = WebConfig::generate().unwrap();
    let server = phone
        .serve_web(config.clone(), vec![IpAddr::V4(Ipv4Addr::LOCALHOST)], 0..=0)
        .await
        .unwrap();
    let params = server.link_params(&phone).expect("a code is open");
    let param = |name: &str| {
        params
            .iter()
            .find(|(key, _)| *key == name)
            .map(|(_, value)| value.clone())
            .unwrap()
    };
    assert_eq!(param("udp"), server.port.to_string());
    assert_eq!(param("fp"), config.fingerprint());

    let identity = Identity::generate().unwrap();
    let pairing = tokio::time::timeout(Duration::from_secs(20), async {
        let stream = browser(&server, &config, CHANNEL_ID).await;
        let (mut channel, remote) = Channel::initiate(stream, &identity).await.unwrap();
        assert_eq!(crate::to_hex(&remote), param("key"));
        channel
            .send(&Message::Pair {
                code: param("pair"),
                device_id: "browser".into(),
                name: "Raygun in a browser".into(),
                port: None,
            })
            .await
            .unwrap();
        channel.recv().await.unwrap()
    })
    .await
    .expect("paired in time");
    assert!(matches!(pairing, Message::Paired { ref name, .. } if name == "Phone"));
    let peers = sync::peers(&db).await.unwrap();
    assert_eq!(peers[0].name, "Raygun in a browser");
    assert_eq!(peers[0].address, None, "a browser can't be reached");

    let rows = tokio::time::timeout(Duration::from_secs(20), async {
        let stream = browser(&server, &config, CHANNEL_ID).await;
        let (mut channel, _) = Channel::initiate(stream, &identity).await.unwrap();
        channel
            .send(&Message::Hello {
                device_id: "browser".into(),
                name: "Raygun in a browser".into(),
                kinds: SyncKind::ALL.to_vec(),
                port: None,
            })
            .await
            .unwrap();
        assert!(matches!(
            channel.recv().await.unwrap(),
            Message::Hello { .. }
        ));
        channel.send(&Message::Done { upto: 0 }).await.unwrap();
        assert!(matches!(
            channel.recv().await.unwrap(),
            Message::Ack { upto: 0 }
        ));
        let mut rows = Vec::new();
        loop {
            match channel.recv().await.unwrap() {
                Message::Rows { rows: more } => rows.extend(more),
                Message::Done { upto } => {
                    channel.send(&Message::Ack { upto }).await.unwrap();
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        rows
    })
    .await
    .expect("synced in time");
    let favorite = rows
        .iter()
        .find(|row| row.table == "music_favorite")
        .expect("the favorite came");
    assert_eq!(favorite.row["title"], "Song");
    server.stop();
}

#[tokio::test]
async fn a_linked_browser_reaches_the_back_end_over_webrtc() {
    let temp = TempDir::new().unwrap();
    let db = Arc::new(Db::open(&temp.path().join("laptop.db")).await.unwrap());
    let laptop = Arc::new(Engine::new(
        db,
        Identity::generate().unwrap(),
        "Laptop".into(),
        SyncKind::ALL.to_vec(),
    ));
    laptop.on_remote(Some(crate::remote::tests::echo()));
    let identity = Identity::generate().unwrap();
    crate::remote::tests::link_browser(&laptop, &identity).await;
    let config = WebConfig::generate().unwrap();
    let server = laptop
        .serve_web(config.clone(), vec![IpAddr::V4(Ipv4Addr::LOCALHOST)], 0..=0)
        .await
        .unwrap();
    tokio::time::timeout(Duration::from_secs(20), async {
        let stream = browser(&server, &config, REMOTE_CHANNEL_ID).await;
        let (mut page, _) = crate::remote::tests::Page::open(stream, &identity).await;
        assert!(matches!(
            page.recv().await,
            crate::remote::tests::Received::Json(welcome) if welcome["t"] == "welcome"
        ));
        page.send(serde_json::json!({ "t": "ping" })).await;
        assert!(matches!(
            page.recv().await,
            crate::remote::tests::Received::Json(echo) if echo["got"]["t"] == "ping"
        ));
        // More than the channel's send buffer holds at once: it waits for room, in order.
        let size = 3 * 1024 * 1024;
        page.send(serde_json::json!({ "t": "fetch", "id": 3, "size": size }))
            .await;
        let mut body = Vec::new();
        loop {
            match page.recv().await {
                crate::remote::tests::Received::Body(3, bytes) => body.extend(bytes),
                crate::remote::tests::Received::Json(end) => {
                    assert_eq!(end["t"], "end");
                    break;
                }
                other => panic!("unexpected {other:?}"),
            }
        }
        assert_eq!(body.len(), size);
        assert!(body.iter().enumerate().all(|(at, byte)| *byte == at as u8));
    })
    .await
    .expect("answered in time");
    server.stop();
}
