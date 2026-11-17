//! What Offline Mode does to the app's services (`raygun_app_base::network_mode` has the rest).

use std::sync::Arc;

pub use raygun_app_base::network_mode::*;
use tauri::{AppHandle, Manager};

/// Follows the switches once the services are up.
pub fn install(app: &AppHandle) {
    raygun_app_base::network_mode::install(
        app,
        |app| Box::pin(went_offline(app)),
        |app| Box::pin(came_online(app)),
    );
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
