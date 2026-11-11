//! One link between devices (M46.47): Connect, sync and the remote are one thing.
//!
//! Linking two devices is Connect's question ("Let <device> play here?"): when it's allowed, both
//! sides swap a [`LinkCard`]. Each saves the other as a sync peer (its key and address, so sync
//! needs no code) and as a Connect device with a token, so either can play on the other. Linking
//! turns on sharing on both (the remote server Connect talks to, and the sync server).
//!
//! The device being linked shows a QR code (a [`LinkInvite`]: its address and a one-time code);
//! the other scans it and asks there, so devices that don't find each other on the network link
//! too.
//!
//! Linked devices keep themselves in sync: a change here (the sync log grows) is sent to them
//! within seconds, and they all sync every few minutes anyway. A sync session runs both ways, so
//! the device that changed is also how the others hear of it.

use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};

use crate::connect::ConnectService;
use crate::device_sync::SyncService;
use crate::remote::RemoteService;
use crate::startup::AppDb;

/// How often the sync log is looked at for changes.
const WATCH_EVERY: Duration = Duration::from_secs(8);
/// Linked devices sync at least this often.
const SYNC_EVERY: Duration = Duration::from_secs(5 * 60);

/// What a device tells the one it links with.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct LinkCard {
    /// Its sync identity: id, public key (hex) and where its sync server listens.
    pub sync_id: String,
    pub public_key: String,
    #[serde(default)]
    pub sync_address: Option<String>,
    /// Its Connect identity, and where its remote server listens.
    pub connect_id: String,
    pub name: String,
    pub kind: String,
    #[serde(default)]
    pub connect_address: Option<String>,
    /// A token for its remote server, minted for the device it links with.
    #[serde(default)]
    pub token: Option<String>,
}

/// What a link QR code says: where the device that shows it listens, a one-time code from its
/// remote server's pairing, and its name. As text: `raygun:link?at=<host:port>&code=<code>&name=<name>`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LinkInvite {
    pub address: String,
    pub code: String,
    pub name: String,
}

/// The scheme and path of a link QR code's text.
const INVITE_PREFIX: &str = "raygun:link";

impl LinkInvite {
    pub fn to_text(&self) -> String {
        let query = url::form_urlencoded::Serializer::new(String::new())
            .append_pair("at", &self.address)
            .append_pair("code", &self.code)
            .append_pair("name", &self.name)
            .finish();
        format!("{INVITE_PREFIX}?{query}")
    }

    /// Reads a scanned or pasted code; anything else is refused in words.
    pub fn parse(text: &str) -> Result<Self, String> {
        const NOT_ONE: &str = "That isn't a Raygun link code. Open Devices on the other device and scan the code it shows.";
        let query = text
            .trim()
            .strip_prefix(INVITE_PREFIX)
            .and_then(|rest| rest.strip_prefix('?'))
            .ok_or(NOT_ONE)?;
        let mut invite = Self {
            address: String::new(),
            code: String::new(),
            name: String::new(),
        };
        for (key, value) in url::form_urlencoded::parse(query.as_bytes()) {
            match key.as_ref() {
                "at" => invite.address = value.trim().to_owned(),
                "code" => invite.code = value.trim().to_owned(),
                "name" => invite.name = value.trim().chars().take(40).collect(),
                _ => {}
            }
        }
        if invite.address.is_empty() || invite.code.is_empty() {
            return Err(NOT_ONE.into());
        }
        Ok(invite)
    }
}

/// Turns sharing on or off: the remote server Connect reaches, and the sync server.
pub async fn set_sharing(app: &AppHandle, on: bool) -> Result<(), String> {
    let remote = app.state::<RemoteService>();
    let remote_result = remote.set_enabled(on).await.map(|_| ());
    app.state::<SyncService>().set_enabled(on).await?;
    remote_result
}

/// This device's card for `other` (its name). Sharing is turned on first; with `mint`, a token
/// for this device's remote server goes with it, so `other` can play here too (its id comes
/// back beside the card, to take it back when they unlink).
pub async fn card_for(
    app: &AppHandle,
    other: &str,
    mint: bool,
) -> Option<(LinkCard, Option<String>)> {
    let sync = app.state::<SyncService>();
    if let Err(error) = sync.set_enabled(true).await {
        tracing::warn!(%error, "sync didn't start for a link");
    }
    let remote = app.state::<RemoteService>();
    // Android 17 may refuse the local network: the link still syncs one way.
    let _ = remote.ensure_running().await;
    let (sync_id, public_key, sync_address) = sync.link_info()?;
    let me = app.state::<Arc<ConnectService>>().me();
    let grant = if mint {
        remote.grant(other).await
    } else {
        None
    };
    let (granted, token) = grant.unzip();
    Some((
        LinkCard {
            sync_id,
            public_key,
            sync_address,
            connect_id: me.id,
            name: me.name,
            kind: me.kind.into(),
            connect_address: remote.address(),
            token,
        },
        granted,
    ))
}

/// Saves a linked device's card: a sync peer, and (with a token) a Connect device that plays
/// without asking; `granted` is the remote server's id for the token it got, taken back when
/// they unlink. Syncs with it right away.
pub async fn accept(app: &AppHandle, card: &LinkCard, granted: Option<&str>) {
    let sync = app.state::<SyncService>();
    if let Err(error) = sync.link_peer(card).await {
        tracing::warn!(%error, device = %card.name, "couldn't keep a linked device for sync");
    }
    app.state::<Arc<ConnectService>>()
        .remember_link(card, granted)
        .await;
    let app = app.clone();
    let device = card.sync_id.clone();
    tauri::async_runtime::spawn(async move {
        let _ = app.state::<SyncService>().sync_now(Some(&device)).await;
    });
}

/// Keeps linked devices in sync: soon after a change here, and every few minutes.
pub fn start_auto_sync(app: AppHandle) {
    tauri::async_runtime::spawn(async move {
        let db = app.state::<AppDb>().inner().0.clone();
        let mut seen = raygun_db::sync::latest_seq(&db).await.unwrap_or(0);
        let mut last_round = tokio::time::Instant::now();
        loop {
            // Every 8 s on screen or while playing; slower in the background or battery saver.
            crate::device_state::rest(WATCH_EVERY).await;
            raygun_net::network_mode::wait_online().await;
            let sync = app.state::<SyncService>();
            if !sync.has_linked().await {
                continue;
            }
            let latest = raygun_db::sync::latest_seq(&db).await.unwrap_or(seen);
            let changed = latest != seen;
            if !changed && last_round.elapsed() < SYNC_EVERY {
                continue;
            }
            seen = latest;
            last_round = tokio::time::Instant::now();
            tracing::debug!(changed, "syncing with linked devices");
            sync.sync_quietly().await;
            // What came in counts as seen: no round trip just to send it back.
            seen = raygun_db::sync::latest_seq(&db).await.unwrap_or(seen);
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn link_codes_read_back_and_other_text_is_refused() {
        let invite = LinkInvite {
            address: "192.168.1.20:8820".into(),
            code: "abcd234567".into(),
            name: "Living room & kitchen".into(),
        };
        let text = invite.to_text();
        assert!(text.starts_with("raygun:link?at=192.168.1.20%3A8820&code=abcd234567"));
        assert_eq!(LinkInvite::parse(&format!("  {text}\n")).unwrap(), invite);

        for other in [
            "",
            "https://example.com/?at=1.2.3.4:8820&code=x",
            "raygun:link?at=192.168.1.20:8820",
            "raygun:link?code=abc",
            "raygun:linked?at=1.2.3.4&code=x",
        ] {
            assert!(LinkInvite::parse(other).is_err(), "{other}");
        }
    }
}
