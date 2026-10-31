//! Wires Raygun's services together and exposes them to the UI as typed Tauri commands.
//!
//! The Tauri shell in `apps/raygun/src-tauri` stays thin: it owns the config
//! and context, and gets everything else from [`builder`].

mod commands;
mod events;
mod library;
mod playback;
mod settings;
mod startup;

use std::path::Path;

/// Location of the generated TypeScript bindings, relative to the workspace root.
pub const BINDINGS_PATH: &str = "packages/bindings/src/index.ts";

/// Collects every command and event exposed to the UI.
pub fn specta_builder() -> tauri_specta::Builder<tauri::Wry> {
    tauri_specta::Builder::<tauri::Wry>::new()
        .commands(tauri_specta::collect_commands![
            commands::app_info,
            commands::storage_info,
            commands::settings_get,
            commands::settings_update,
            commands::playback_get_state,
            commands::playback_get_queue,
            commands::playback_enqueue,
            commands::playback_control,
            commands::playback_engine_report,
            commands::library_status,
            commands::library_connect_device_music,
            commands::library_add_root,
            commands::library_remove_root,
            commands::library_rescan,
            commands::library_albums,
            commands::library_artists,
            commands::library_album_tracks,
            commands::library_tracks,
            commands::library_play
        ])
        .events(tauri_specta::collect_events![
            events::EngineCommandEvent,
            events::PlaybackStateEvent,
            events::QueueEvent,
            events::LibraryScanEvent
        ])
}

/// Writes the TypeScript bindings for all commands to `path`.
pub fn export_bindings(path: &Path) -> Result<(), String> {
    specta_builder()
        .export(
            specta_typescript::Typescript::default().header("/* eslint-disable */"),
            path,
        )
        .map_err(|error| format!("failed to export bindings: {error}"))
}

/// Returns a Tauri builder with all Raygun commands registered.
pub fn builder() -> tauri::Builder<tauri::Wry> {
    let specta = specta_builder();
    tauri::Builder::default()
        .plugin(tauri_plugin_raygun_audio::init())
        .invoke_handler(specta.invoke_handler())
        .setup(move |app| {
            specta.mount_events(app);
            startup::init(app)
        })
}

/// Absolute path of the bindings file in this checkout.
pub fn workspace_bindings_path() -> std::path::PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the workspace root")
        .join(BINDINGS_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_are_up_to_date() {
        let committed = std::fs::read_to_string(workspace_bindings_path())
            .expect("bindings missing: run `pnpm bindings`");
        let fresh_path =
            std::env::temp_dir().join(format!("raygun-bindings-{}.ts", std::process::id()));
        export_bindings(&fresh_path).unwrap();
        let fresh = std::fs::read_to_string(&fresh_path).unwrap();
        let _ = std::fs::remove_file(&fresh_path);
        assert!(
            normalize(&committed) == normalize(&fresh),
            "bindings are stale: run `pnpm bindings`"
        );
    }

    fn normalize(text: &str) -> String {
        text.replace("\r\n", "\n")
    }
}
