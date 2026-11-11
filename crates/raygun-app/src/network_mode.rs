//! Offline Mode and Data Saver on this device (`Settings::network`, device-only). The switches
//! are `raygun_net::network_mode`'s, which every request and every client consults; this puts the
//! settings into effect and carries out what follows from a change:
//! - Offline Mode on: downloads wait (`device_state::downloads_held`), the local network is left
//!   alone (the remote and device sync stop listening, Connect and casting stop browsing, a cast
//!   or Connect session ends and plays here), and a song that isn't on the device stops
//!   (`PlaybackService::went_offline`). Background loops wait in `raygun_net::network_mode::wait_online`.
//! - Offline Mode off: everything above starts again as it was; queued downloads go on.
//! - Data Saver applies while it's on, or only on mobile data (looked at now and then); services
//!   read their settings through `settings::load_in_effect`.
//!
//! The UI follows `NetworkStatusEvent`.

use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_model::{NetworkPrefs, NetworkStatus, SettingChange};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

/// How often the connection is looked at while Data Saver applies on mobile data only.
const MOBILE_CHECK: Duration = Duration::from_secs(30);

/// Offline Mode was turned on or off, or Data Saver started or stopped applying.
#[derive(Debug, Clone, Serialize, Deserialize, Type, tauri_specta::Event)]
pub struct NetworkStatusEvent(pub NetworkStatus);

/// The prefs last put into effect.
static PREFS: Mutex<Option<NetworkPrefs>> = Mutex::new(None);

fn prefs() -> NetworkPrefs {
    PREFS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner())
        .unwrap_or_default()
}

fn remember(prefs: &NetworkPrefs) {
    *PREFS
        .lock()
        .unwrap_or_else(|poisoned| poisoned.into_inner()) = Some(*prefs);
    let saver = prefs.data_saver;
    raygun_net::network_mode::set_saver_parts(saver.fewer_sources, saver.smaller_covers);
    // Waits that read Data Saver's parts look again.
    raygun_net::network_mode::nudge();
}

/// Whether Data Saver applies now with `part` of it on: `saving(|saver| saver.no_prefetch)`.
pub fn saving(part: impl Fn(&raygun_model::DataSaverPrefs) -> bool) -> bool {
    raygun_net::network_mode::data_saver() && part(&prefs().data_saver)
}

/// Whether background lookups (covers, tags, lengths) rest now: Offline Mode, or Data Saver
/// with its part for them on.
pub fn lookups_rest() -> bool {
    raygun_net::network_mode::offline() || saving(|saver| saver.no_background_lookups)
}

/// Waits while background lookups rest ([`lookups_rest`]); returns at once otherwise.
pub async fn wait_for_lookups() {
    let mut changes = raygun_net::network_mode::subscribe();
    while lookups_rest() {
        if changes.changed().await.is_err() {
            return;
        }
    }
}

/// The status now.
pub fn status() -> NetworkStatus {
    NetworkStatus {
        offline: raygun_net::network_mode::offline(),
        data_saver: raygun_net::network_mode::data_saver(),
        mobile_data: crate::network::known_mobile_data().unwrap_or(false),
    }
}

/// The stored switches, before any service starts: nothing sent at start slips past Offline Mode.
/// Data Saver for mobile data only waits for the first look at the connection ([`install`]).
pub fn start_with(prefs: &NetworkPrefs) {
    remember(prefs);
    raygun_net::network_mode::set_offline(prefs.offline);
    raygun_net::network_mode::set_data_saver(prefs.data_saver.applies(false));
    if prefs.offline {
        tracing::info!("offline mode kept on from the last run");
    }
}

/// `Settings::network` changed: puts it into effect.
pub fn apply(app: &AppHandle, prefs: &NetworkPrefs) {
    remember(prefs);
    raygun_net::network_mode::set_offline(prefs.offline);
    let mobile = prefs.data_saver.enabled
        && prefs.data_saver.mobile_data_only
        && crate::network::on_mobile_data(app);
    raygun_net::network_mode::set_data_saver(prefs.data_saver.applies(mobile));
}

/// Turns Offline Mode on or off, as its quick switches do (More, menus, the badge): stored with
/// the rest of `Settings::network`, put into effect, and the settings screens told.
pub async fn set_offline(app: &AppHandle, offline: bool) -> Result<NetworkStatus, String> {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    let current = crate::settings::load(&db)
        .await
        .map_err(|error| error.to_string())?
        .network;
    if current.offline != offline {
        let next = NetworkPrefs { offline, ..current };
        crate::settings::update(&db, SettingChange::Network(next))
            .await
            .map_err(|error| error.to_string())?;
        apply(app, &next);
        let _ = crate::events::SettingsChangedEvent("network".into()).emit(app);
    }
    Ok(status())
}

/// The main webview's own gate (Android: `OfflineGate.kt`), which keeps the page from loading
/// anything from the network while Offline Mode is on. Desktop pages load only through Raygun.
#[cfg_attr(not(target_os = "android"), allow(unused_variables))]
async fn gate_webview(app: &AppHandle, offline: bool) {
    #[cfg(target_os = "android")]
    {
        let handle = app.clone();
        let set = tauri::async_runtime::spawn_blocking(move || {
            handle
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .offline_gate(offline)
        })
        .await;
        if let Ok(Err(error)) = set {
            tracing::warn!(%error, "the webview's offline gate wasn't set");
        }
    }
}

/// Follows the switches once the services are up.
pub fn install(app: &AppHandle) {
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        let mut changes = raygun_net::network_mode::subscribe();
        let mut last = *changes.borrow_and_update();
        gate_webview(&handle, last.offline).await;
        if last.offline {
            went_offline(&handle).await;
        }
        while changes.changed().await.is_ok() {
            let now = *changes.borrow_and_update();
            if now.offline != last.offline {
                tracing::info!(offline = now.offline, "offline mode");
                gate_webview(&handle, now.offline).await;
                if now.offline {
                    went_offline(&handle).await;
                } else {
                    came_online(&handle).await;
                }
            }
            if now.data_saver != last.data_saver {
                tracing::info!(data_saver = now.data_saver, "data saver");
            }
            last = now;
            if let Err(error) = NetworkStatusEvent(status()).emit(&handle) {
                tracing::debug!(%error, "network status not sent");
            }
        }
    });
    // Data Saver on mobile data only: the connection is looked at while it may apply.
    let handle = app.clone();
    tauri::async_runtime::spawn(async move {
        loop {
            let prefs = prefs();
            if prefs.data_saver.enabled
                && prefs.data_saver.mobile_data_only
                && !raygun_net::network_mode::offline()
            {
                let mobile = crate::network::on_mobile_data(&handle);
                raygun_net::network_mode::set_data_saver(prefs.data_saver.applies(mobile));
            }
            crate::device_state::rest(MOBILE_CHECK).await;
        }
    });
}

async fn went_offline(app: &AppHandle) {
    crate::device_state::poke();
    // Song downloads stop where they are (chapters and videos stop at their next piece).
    if let Some(songs) = app.try_state::<Arc<crate::music_downloads::SongDownloadService>>() {
        songs.hold_for_offline();
    }
    if let Some(playback) = app.try_state::<crate::playback::PlaybackService>() {
        playback.went_offline();
    }
    if let Some(connect) = app.try_state::<Arc<crate::connect::ConnectService>>() {
        let connect = connect.inner().clone();
        if connect.status().session.is_some() {
            connect.disconnect(true).await;
        }
        connect.stop_browsing();
    }
    if let Some(cast) = app.try_state::<Arc<crate::cast::CastService>>() {
        cast.inner().clone().go_offline().await;
    }
    if let Some(websites) = app.try_state::<Arc<crate::websites::WebsiteService>>() {
        websites.close_for_offline();
    }
    if let Some(remote) = app.try_state::<crate::remote::RemoteService>() {
        remote.follow_network(true).await;
    }
    if let Some(sync) = app.try_state::<crate::device_sync::SyncService>() {
        sync.follow_network(true).await;
    }
}

async fn came_online(app: &AppHandle) {
    crate::device_state::poke();
    if let Some(remote) = app.try_state::<crate::remote::RemoteService>() {
        remote.follow_network(false).await;
    }
    if let Some(sync) = app.try_state::<crate::device_sync::SyncService>() {
        sync.follow_network(false).await;
    }
}

/// Offline Mode and whether Data Saver applies now. Afterwards, changes arrive as
/// `NetworkStatusEvent`.
#[tauri::command]
#[specta::specta]
pub fn network_status() -> NetworkStatus {
    status()
}

/// Turns Offline Mode on or off: while it's on, Raygun sends nothing over the network.
#[tauri::command]
#[specta::specta]
pub async fn network_set_offline(app: AppHandle, offline: bool) -> Result<NetworkStatus, String> {
    set_offline(&app, offline).await
}
