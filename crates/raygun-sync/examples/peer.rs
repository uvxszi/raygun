//! A headless Raygun sync peer for testing sync against a phone or emulator from a PC. It uses
//! the same engine and database code as the app, on a database in the folder you name.
//!
//! ```text
//! cargo run -p raygun-sync --example peer -- <dir> serve [--port 8835] [--host 10.0.2.2] [--bind 127.0.0.1] [--web]
//! cargo run -p raygun-sync --example peer -- <dir> join <pairing text> [--via 127.0.0.1:8830]
//! cargo run -p raygun-sync --example peer -- <dir> sync
//! cargo run -p raygun-sync --example peer -- <dir> favorite <title> <artist>
//! cargo run -p raygun-sync --example peer -- <dir> playlist <name> <title> <artist>
//! cargo run -p raygun-sync --example peer -- <dir> rename <old name> <new name>
//! cargo run -p raygun-sync --example peer -- <dir> list
//! ```
//!
//! An emulator reaches the PC at 10.0.2.2. The PC reaches the emulator's sync port through
//! `adb forward tcp:8830 tcp:8830`, so serve on another port (8835).
//!
//! `--web` also listens for browsers (Raygun on the web, docs/WEB.md) on the bind address and prints
//! the link code a browser takes: paste it into Devices → Scan link code there.

use std::net::{IpAddr, Ipv4Addr};
use std::path::PathBuf;
use std::sync::Arc;

use raygun_db::collections::{self, TrackFields};
use raygun_db::sync::{self, SyncKind};
use raygun_db::{Db, local_value, set_local_value};
use raygun_sync::protocol::Offer;
use raygun_sync::{Engine, Identity, fingerprint, to_hex};

#[tokio::main(flavor = "multi_thread")]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter("info")
        .with_writer(std::io::stderr)
        .init();
    let args: Vec<String> = std::env::args().skip(1).collect();
    let (Some(dir), Some(command)) = (args.first(), args.get(1)) else {
        eprintln!("usage: peer <dir> <serve|join|sync|favorite|playlist|rename|list> ...");
        std::process::exit(2);
    };
    let rest = &args[2..];
    let dir = PathBuf::from(dir);
    std::fs::create_dir_all(&dir)?;
    let db = Arc::new(Db::open(&dir.join("raygun.db")).await?);
    let identity = identity(&db).await?;
    let engine = Arc::new(Engine::new(
        db.clone(),
        identity,
        "PC test peer".into(),
        SyncKind::ALL.to_vec(),
    ));
    let option = |name: &str| {
        rest.iter()
            .position(|arg| arg == name)
            .and_then(|index| rest.get(index + 1).cloned())
    };

    match command.as_str() {
        "serve" => {
            let port: u16 = option("--port").map_or(Ok(8835), |port| port.parse())?;
            let host: IpAddr = option("--host")
                .map_or(Ok(Ipv4Addr::new(10, 0, 2, 2).into()), |host| host.parse())?;
            // Loopback by default: an emulator reaches it as 10.0.2.2, and a loopback listener
            // raises no firewall prompt. `--bind 0.0.0.0` serves the whole network.
            let bind: IpAddr =
                option("--bind").map_or(Ok(Ipv4Addr::LOCALHOST.into()), |bind| bind.parse())?;
            let _server = engine.serve_on(bind, port..=port).await?;
            let web = if rest.iter().any(|arg| arg == "--web") {
                let config = web_config(&db).await?;
                let hosts = if bind.is_unspecified() {
                    raygun_sync::web::local_hosts()
                } else {
                    vec![bind]
                };
                Some(
                    engine
                        .serve_web(config, hosts, raygun_sync::web::PORTS)
                        .await?,
                )
            } else {
                None
            };
            println!(
                "device {} key {}",
                engine.device_id(),
                fingerprint(&engine.identity().public)
            );
            println!("{}", engine.open_pairing(host)?);
            if let Some(web) = &web {
                let params = web.link_params(&engine).ok_or("no pairing code is open")?;
                let query: Vec<String> = params
                    .iter()
                    .map(|(key, value)| format!("{key}={}", encode(value)))
                    .collect();
                println!("raygun:link?name=PC%20test%20peer&{}", query.join("&"));
                println!("browsers on UDP port {}", web.port);
            }
            println!("serving on port {port}; Ctrl-C to stop");
            tokio::signal::ctrl_c().await?;
        }
        "join" => {
            let text = rest.first().ok_or("join needs the pairing text")?;
            let text = match option("--via") {
                // Reach the host through a forwarded port instead of the address in the text.
                Some(via) => {
                    let mut offer = Offer::parse(text).ok_or("not a pairing text")?;
                    let (host, port) = via.rsplit_once(':').ok_or("--via needs host:port")?;
                    offer.host = host.to_owned();
                    offer.port = port.parse()?;
                    offer.to_text()
                }
                None => text.clone(),
            };
            let peer = engine.join(&text).await?;
            println!("paired with {} ({})", peer.name, peer.device_id);
            let summary = engine.sync_with(&peer.device_id).await?;
            println!("synced: {summary:?}");
        }
        "sync" => {
            for peer in sync::peers(&db).await? {
                match engine.sync_with(&peer.device_id).await {
                    Ok(summary) => println!("{}: {summary:?}", peer.name),
                    Err(error) => println!("{}: failed: {error}", peer.name),
                }
            }
        }
        "favorite" => {
            let (title, artist) = (rest.first().ok_or("title")?, rest.get(1).ok_or("artist")?);
            collections::set_favorite(&db, &track(title, artist), true, now()).await?;
            println!("favorited {title}");
        }
        "playlist" => {
            let name = rest.first().ok_or("playlist name")?;
            let (title, artist) = (rest.get(1).ok_or("title")?, rest.get(2).ok_or("artist")?);
            let existing = collections::playlists(&db)
                .await?
                .into_iter()
                .find(|playlist| &playlist.name == name);
            let id = match existing {
                Some(playlist) => playlist.id,
                None => collections::create_playlist(&db, name, now()).await?,
            };
            collections::add_to_playlist(&db, &id, &[track(title, artist)], now()).await?;
            println!("added {title} to {name}");
        }
        "rename" => {
            let (old, new) = (
                rest.first().ok_or("old name")?,
                rest.get(1).ok_or("new name")?,
            );
            let playlist = collections::playlists(&db)
                .await?
                .into_iter()
                .find(|playlist| &playlist.name == old)
                .ok_or("no such playlist")?;
            collections::rename_playlist(&db, &playlist.id, new).await?;
            println!("renamed {old} to {new}");
        }
        "list" => {
            println!("favorites:");
            for favorite in collections::favorites(&db).await? {
                println!(
                    "  {} - {}",
                    favorite.track.title,
                    favorite.track.artist.unwrap_or_default()
                );
            }
            println!("playlists:");
            for playlist in collections::playlists(&db).await? {
                println!("  {} ({} tracks)", playlist.name, playlist.item_count);
                for item in collections::playlist_items(&db, &playlist.id).await? {
                    println!("    {}", item.track.title);
                }
            }
            println!("history:");
            for entry in collections::recent_history(&db, 10).await? {
                println!("  {}", entry.track.title);
            }
            println!("peers:");
            for peer in sync::peers(&db).await? {
                println!("  {} {} at {:?}", peer.name, peer.device_id, peer.address);
            }
        }
        other => return Err(format!("unknown command {other}").into()),
    }
    Ok(())
}

/// The peer's key pair, kept in the same device-local keys the app uses.
async fn identity(db: &Db) -> Result<Identity, Box<dyn std::error::Error>> {
    let private = local_value(db, "sync_private_key").await?;
    let public = local_value(db, "sync_public_key").await?;
    if let Some(identity) = private
        .zip(public)
        .and_then(|(private, public)| Identity::from_hex(&private, &public))
    {
        return Ok(identity);
    }
    let identity = Identity::generate()?;
    set_local_value(db, "sync_private_key", &to_hex(&identity.private)).await?;
    set_local_value(db, "sync_public_key", &to_hex(&identity.public)).await?;
    Ok(identity)
}

/// The browser listener's credentials and certificate, kept like the app keeps them.
async fn web_config(db: &Db) -> Result<raygun_sync::web::WebConfig, Box<dyn std::error::Error>> {
    let stored = local_value(db, "sync_web_config").await?;
    if let Some(config) = stored.and_then(|text| raygun_sync::web::WebConfig::from_stored(&text)) {
        return Ok(config);
    }
    let config = raygun_sync::web::WebConfig::generate()?;
    set_local_value(db, "sync_web_config", &config.to_stored()).await?;
    Ok(config)
}

/// Percent-encodes what isn't plain in a query value.
fn encode(value: &str) -> String {
    value
        .bytes()
        .map(|byte| match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'.' | b'_' | b'~' => {
                (byte as char).to_string()
            }
            _ => format!("%{byte:02X}"),
        })
        .collect()
}

fn track(title: &str, artist: &str) -> TrackFields {
    TrackFields {
        track_key: format!("{}|{}", artist.to_lowercase(), title.to_lowercase()),
        title: title.to_owned(),
        artist: Some(artist.to_owned()),
        album: None,
        duration_ms: None,
        artwork_url: None,
        source: None,
    }
}

fn now() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}
