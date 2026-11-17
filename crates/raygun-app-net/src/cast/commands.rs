//! Play on TV as commands.

use std::sync::Arc;

type Casting<'a> = tauri::State<'a, Arc<super::CastService>>;

/// TVs and other devices the player's video can play on. Asking searches the network, again
/// every few seconds while asked.
#[tauri::command]
#[specta::specta]
pub fn cast_devices(cast: Casting<'_>) -> raygun_model::CastDeviceList {
    let devices = cast.inner().devices();
    raygun_model::CastDeviceList {
        devices,
        searching: cast.searching(),
    }
}

/// Adds a TV by its address (`192.168.1.20`, with a port, or its description's address).
#[tauri::command]
#[specta::specta]
pub async fn cast_add_device(
    cast: Casting<'_>,
    address: String,
) -> Result<raygun_model::CastDevice, String> {
    cast.inner().add(&address).await
}

/// Plays the player's video on a device from `position_ms`, with the subtitle file at
/// `subtitle` (an index into the video's subtitles) on.
#[tauri::command]
#[specta::specta]
pub async fn cast_start(
    cast: Casting<'_>,
    device_id: String,
    position_ms: f64,
    subtitle: Option<u32>,
) -> Result<(), String> {
    cast.inner()
        .start(
            &device_id,
            position_ms,
            subtitle.map(|index| index as usize),
        )
        .await
}

/// A button of the device's remote control.
#[tauri::command]
#[specta::specta]
pub async fn cast_control(
    cast: Casting<'_>,
    control: raygun_model::CastControl,
) -> Result<(), String> {
    cast.inner().control(control).await
}

/// Stops playing on the device; with `resume`, the player goes on from where it was.
#[tauri::command]
#[specta::specta]
pub async fn cast_disconnect(cast: Casting<'_>, resume: bool) -> Result<(), String> {
    cast.inner().disconnect(resume).await
}

/// What plays on a device, if anything.
#[tauri::command]
#[specta::specta]
pub fn cast_status(cast: Casting<'_>) -> Option<raygun_model::CastStatus> {
    cast.inner().status()
}
