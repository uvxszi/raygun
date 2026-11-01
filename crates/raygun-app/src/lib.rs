//! Wires Raygun's services together and exposes them to the UI as typed Tauri commands.
//!
//! The Tauri shell in `apps/raygun/src-tauri` stays thin: it owns the config
//! and context, and gets everything else from [`builder`].

mod ai;
mod backup;
mod catalog;
mod collections;
mod commands;
mod device_sync;
mod epub;
mod events;
mod extensions;
mod history;
mod library;
mod manga;
mod manga_downloads;
mod mcp;
mod mihon;
mod novel;
mod playback;
mod remote;
mod resolution;
mod resolver;
mod search;
mod settings;
mod startup;
mod system_ui;
mod tts;

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
            commands::library_play,
            commands::playback_replace,
            commands::extensions_list,
            commands::extensions_reload,
            commands::extension_repos,
            commands::extension_repo_add,
            commands::extension_repo_remove,
            commands::extension_catalog,
            commands::extension_install,
            commands::extension_uninstall,
            commands::extension_setting_set,
            commands::online_search,
            commands::lyrics_get,
            commands::favorites_list,
            commands::favorite_get,
            commands::favorite_set,
            commands::playlists_list,
            commands::playlist_get,
            commands::playlist_create,
            commands::playlist_rename,
            commands::playlist_delete,
            commands::playlist_add,
            commands::playlist_remove_item,
            commands::playlist_move_item,
            commands::history_recent,
            commands::history_clear,
            commands::collection_play,
            commands::mcp_status,
            commands::mcp_set_enabled,
            commands::mcp_new_token,
            commands::remote_status,
            commands::remote_set_enabled,
            commands::remote_new_code,
            commands::remote_forget_device,
            commands::sync_status,
            commands::sync_set_enabled,
            commands::sync_open_pairing,
            commands::sync_join,
            commands::sync_now,
            commands::sync_remove_peer,
            commands::sync_set_kinds,
            commands::sync_set_device_name,
            commands::sync_undo_last,
            commands::manga_sources,
            commands::manga_browse,
            commands::manga_title,
            commands::manga_set_in_library,
            commands::manga_library,
            commands::manga_chapter,
            commands::manga_set_progress,
            commands::manga_set_read,
            commands::manga_set_reader_mode,
            commands::manga_updates,
            commands::manga_update_library,
            commands::manga_update_status,
            commands::manga_downloads,
            commands::manga_download,
            commands::manga_remove_downloads,
            commands::manga_set_downloads_paused,
            commands::backup_create,
            commands::backup_list,
            commands::backup_restore,
            commands::backup_delete,
            commands::backup_import,
            commands::novel_sources,
            commands::novel_browse,
            commands::novel_title,
            commands::novel_set_in_library,
            commands::novel_library,
            commands::novel_chapter,
            commands::novel_set_progress,
            commands::novel_set_read,
            commands::novel_import_epub,
            commands::tts_available,
            commands::tts_speak,
            commands::tts_stop,
            commands::ai_providers,
            commands::ai_provider_save,
            commands::ai_provider_delete,
            commands::ai_provider_test,
            commands::ai_models,
            commands::ai_model_download,
            commands::ai_model_delete
        ])
        .events(tauri_specta::collect_events![
            events::EngineCommandEvent,
            events::PlaybackStateEvent,
            events::QueueEvent,
            events::LibraryScanEvent,
            events::ExtensionsChangedEvent,
            events::SyncChangedEvent,
            events::MangaUpdateEvent,
            events::MangaDownloadsEvent,
            events::TtsEvent,
            events::AiModelEvent
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
