//! Smart Chips as commands.

use std::sync::Arc;

use raygun_model::{ChipId, ChipInfo, SettingChange};

use super::ChipsService;

/// Every chip in the catalogue, with what this device has of it.
#[tauri::command]
#[specta::specta]
pub fn chips_list(chips: tauri::State<'_, Arc<ChipsService>>) -> Vec<ChipInfo> {
    chips.list()
}

/// Downloads and installs a chip (or its update) and switches it on; progress comes as
/// `ChipsEvent`s, and the answer once it's done.
#[tauri::command]
#[specta::specta]
pub async fn chips_install(
    app: tauri::AppHandle,
    chips: tauri::State<'_, Arc<ChipsService>>,
    id: ChipId,
) -> Result<Vec<ChipInfo>, String> {
    let chips = chips.inner().clone();
    chips.install(id).await.map_err(|error| error.to_string())?;
    if !chips.prefs().is_on(id) {
        switch(&app, &chips, id, true).await?;
    }
    Ok(chips.list())
}

/// Stops a chip's download.
#[tauri::command]
#[specta::specta]
pub fn chips_cancel(chips: tauri::State<'_, Arc<ChipsService>>, id: ChipId) -> Vec<ChipInfo> {
    chips.cancel(id);
    chips.list()
}

#[tauri::command]
#[specta::specta]
pub async fn chips_remove(
    chips: tauri::State<'_, Arc<ChipsService>>,
    id: ChipId,
) -> Result<Vec<ChipInfo>, String> {
    chips.remove(id).await.map_err(|error| error.to_string())?;
    Ok(chips.list())
}

/// Switches a chip on or off (synced with linked devices).
#[tauri::command]
#[specta::specta]
pub async fn chips_set_enabled(
    app: tauri::AppHandle,
    chips: tauri::State<'_, Arc<ChipsService>>,
    id: ChipId,
    enabled: bool,
) -> Result<Vec<ChipInfo>, String> {
    switch(&app, chips.inner(), id, enabled).await?;
    Ok(chips.list())
}

/// A text file of an installed, switched-on chip (the desktop player reads its shaders so).
#[tauri::command]
#[specta::specta]
pub async fn chips_read_text(
    chips: tauri::State<'_, Arc<ChipsService>>,
    id: ChipId,
    name: String,
) -> Result<String, String> {
    chips.read_text(id, &name).await
}

async fn switch(
    app: &tauri::AppHandle,
    chips: &Arc<ChipsService>,
    id: ChipId,
    on: bool,
) -> Result<(), String> {
    use tauri::Manager;
    let prefs = chips.prefs().with(id, on);
    let db = app.state::<crate::AppDb>().inner().0.clone();
    let settings = crate::settings::update(&db, SettingChange::Chips(prefs.clone()))
        .await
        .map_err(|error| error.to_string())?;
    chips.set_prefs(prefs);
    crate::settings::apply_live(app, &settings, Some(&["chips"])).await;
    Ok(())
}
