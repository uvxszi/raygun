//! Wires Raygun's services together and exposes them to the UI as typed Tauri commands.
//!
//! The Tauri shell in `apps/raygun/src-tauri` stays thin: it owns the config
//! and context, and gets everything else from [`builder`].

mod ai;
mod aniskip;
mod backup;
mod caches;
mod captions;
mod catalog;
mod collections;
mod comics;
mod commands;
mod connect;
mod cover_fallback;
mod details;
mod device_link;
mod device_sync;
mod discovery;
mod durations;
mod epub;
mod events;
mod export;
mod extensions;
mod follows;
mod genres;
mod hidden;
mod history;
mod import;
mod last_played;
mod library;
mod library_actions;
mod local_files;
mod local_network;
mod lyrics_store;
mod made_for_you;
mod manga;
mod manga_downloads;
mod mcp;
mod mihon;
mod miniplayer;
mod music_downloads;
mod network;
mod novel;
mod playback;
mod podcast_feed;
mod podcasts;
mod privacy;
mod quick_access;
mod remote;
mod resolution;
mod resolver;
mod search;
mod settings;
mod sound;
mod startup;
mod stremio;
mod system_ui;
mod tts;
mod updates;
mod uploads;
mod video;
#[cfg(not(target_os = "android"))]
mod video_desktop;
mod video_downloads;
mod video_library;
mod video_session;
mod video_sources;
mod websites;
mod ytdlp;

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
            commands::recent_contexts,
            commands::recent_contexts_add,
            commands::playback_get_state,
            commands::playback_get_queue,
            commands::playback_enqueue,
            commands::playback_queue_tracks,
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
            commands::extension_clear_data,
            commands::miniplayer_open,
            commands::music_artist_info,
            commands::music_song_credits,
            commands::music_open_info_page,
            commands::playback_restore,
            commands::dev_add_test_sources,
            commands::podcast_queue,
            commands::connect_status,
            commands::connect_devices,
            commands::connect_add,
            commands::connect_to,
            commands::connect_disconnect,
            commands::connect_forget,
            commands::connect_answer,
            commands::miniplayer_close,
            commands::extension_open_website,
            commands::catalog_open_page,
            commands::extension_repos,
            commands::extension_suggested_repos,
            commands::extension_repo_add,
            commands::extension_repo_remove,
            commands::extension_catalog,
            commands::extension_install,
            commands::extension_uninstall,
            commands::extension_setting_set,
            commands::online_search,
            commands::lyrics_get,
            commands::lyrics_options,
            commands::lyrics_transcribe,
            commands::captions_use_feed,
            commands::captions_use_ai,
            commands::stremio_ratings,
            commands::song_edit_get,
            commands::song_edit_set,
            commands::favorites_list,
            commands::downloaded_songs,
            commands::song_download_for,
            commands::collections_saves,
            commands::collections_set_saves,
            commands::music_artist,
            commands::music_charts,
            commands::music_chart,
            commands::music_tag_sources,
            commands::made_for_you,
            commands::made_for_you_mix,
            commands::made_for_you_save,
            commands::hidden_artist_set,
            commands::hidden_item_set,
            commands::hidden_clear,
            commands::import_read_link,
            commands::import_read_text,
            commands::import_reader_ready,
            commands::import_prepare_reader,
            commands::import_read_screenshots,
            commands::import_match,
            commands::track_durations,
            commands::cover_save,
            commands::detail_edit,
            commands::detail_edit_set,
            commands::detail_cover_import,
            commands::detail_cover_options,
            commands::cover_fallback,
            commands::local_folders,
            commands::local_folder_add,
            commands::local_folder_remove,
            commands::local_folders_rescan,
            commands::import_save,
            commands::track_assign_source,
            commands::tracks_unavailable,
            commands::audiobooks,
            commands::audiobook,
            commands::audiobook_open,
            commands::audiobook_play,
            commands::audiobook_directory,
            commands::captions,
            commands::captions_transcribe,
            commands::captions_stop,
            commands::captions_clear,
            commands::captions_video_transcribe,
            commands::captions_video_make,
            commands::spoken_item,
            commands::music_track_tags,
            commands::music_tags_for,
            commands::music_tag_summary,
            commands::music_tag_songs,
            commands::music_tag_vocabulary,
            commands::music_search_artists,
            commands::music_search_albums,
            commands::music_album,
            commands::music_artist_releases,
            commands::music_search_playlists,
            commands::artists_followed,
            commands::audio_outputs,
            commands::audio_output_set,
            commands::artist_follow,
            commands::artist_unfollow,
            commands::artist_set_alerts,
            commands::artist_release_news,
            commands::music_artist_events,
            commands::music_playlist,
            commands::music_find_album,
            commands::songs_download,
            commands::song_downloads,
            commands::song_download,
            commands::song_download_remove,
            commands::song_downloads_remove,
            commands::song_download_retry,
            commands::song_downloads_retry_failed,
            commands::song_downloads_reorder,
            commands::song_downloads_paused,
            commands::song_downloads_set_paused,
            commands::song_download_record,
            commands::songs_save,
            commands::song_downloaded_keys,
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
            commands::playlist_set_order,
            commands::playlists_merge,
            commands::collections_saved_keys,
            commands::song_download_cancel_recording,
            commands::song_download_use_source,
            commands::youtube_link,
            commands::youtube_play,
            commands::ytdlp_status,
            commands::ytdlp_install,
            commands::ytdlp_install_ffmpeg,
            commands::ytdlp_download,
            commands::ytdlp_jobs,
            commands::ytdlp_cancel,
            commands::ytdlp_reveal,
            commands::dev_remove_test_sources,
            commands::app_update,
            commands::connect_link,
            commands::devices_set_sharing,
            commands::app_update_install,
            commands::app_open_page,
            commands::lyrics_forget,
            commands::lyrics_cache_usage,
            commands::lyrics_cache_clear,
            commands::lyrics_transcribe_many,
            commands::favorite_move,
            commands::favorites_set_order,
            commands::history_recent,
            commands::history_clear,
            commands::incognito_status,
            commands::incognito_set_enabled,
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
            commands::novel_import_epub_upload,
            commands::tts_available,
            commands::tts_speak,
            commands::tts_stop,
            commands::ai_providers,
            commands::ai_provider_save,
            commands::ai_provider_delete,
            commands::ai_provider_test,
            commands::ai_models,
            commands::ai_model_download,
            commands::ai_model_delete,
            commands::podcast_subscriptions,
            commands::podcast_latest,
            commands::podcast_open_feed,
            commands::podcast_get,
            commands::podcast_set_subscribed,
            commands::podcast_refresh_all,
            commands::podcast_search,
            commands::podcast_top,
            commands::podcast_play,
            commands::podcast_set_played,
            commands::podcast_download,
            commands::podcast_remove_download,
            commands::video_play,
            commands::video_play_link,
            commands::video_continue_watching,
            commands::video_resume,
            commands::video_remove,
            commands::video_history,
            commands::library_categories,
            commands::library_category_create,
            commands::library_category_rename,
            commands::library_category_delete,
            commands::library_category_reorder,
            commands::library_set_title_categories,
            commands::library_title_marks,
            commands::library_set_list,
            commands::library_set_favorite,
            commands::library_set_pinned,
            commands::library_mark_read,
            commands::library_set_chapters_read,
            commands::library_mark_previous_read,
            commands::library_remove,
            commands::library_change_categories,
            commands::library_download_next,
            commands::library_continue,
            commands::library_duplicates,
            commands::library_set_category_display,
            commands::library_update_category,
            commands::upload_begin,
            commands::upload_append,
            commands::upload_cancel,
            commands::manga_import_comic,
            commands::save_pick_folder,
            commands::songs_pick_folder,
            commands::save_folder_state,
            commands::export_plan,
            commands::export_start,
            commands::export_cancel,
            commands::export_dismiss,
            commands::export_jobs,
            commands::page_save,
            commands::page_share,
            commands::page_copy,
            commands::storage_usage,
            commands::storage_clear,
            commands::website_list,
            commands::website_add,
            commands::website_edit,
            commands::website_remove,
            commands::website_reset,
            commands::website_open,
            commands::website_blocklist_status,
            commands::website_blocklist_update,
            commands::website_clear_data,
            commands::catalog_filters,
            commands::catalog_search,
            commands::catalog_global_search,
            commands::library_search,
            commands::stremio_addons,
            commands::stremio_suggestions,
            commands::stremio_preview,
            commands::stremio_add,
            commands::stremio_remove,
            commands::stremio_set_enabled,
            commands::stremio_set_order,
            commands::stremio_configure,
            commands::stremio_open_link,
            commands::stremio_catalogs,
            commands::stremio_home,
            commands::stremio_catalog,
            commands::stremio_search,
            commands::stremio_search_catalog,
            commands::stremio_meta,
            commands::stremio_streams,
            commands::stremio_play,
            commands::stremio_set_watched,
            commands::local_network_access,
            commands::local_network_request,
            commands::local_network_open_settings,
            commands::video_sources,
            commands::video_browse,
            commands::video_title,
            commands::video_servers,
            commands::video_comments,
            commands::video_links,
            commands::video_source_play,
            commands::video_set_episode_watched,
            commands::video_set_in_library,
            commands::video_library,
            commands::video_updates,
            commands::video_upcoming,
            commands::stremio_set_in_library,
            commands::video_downloads,
            commands::video_source_download,
            commands::stremio_download,
            commands::video_download_retry,
            commands::video_download_remove,
            commands::video_download_play,
            commands::video_downloads_set_paused,
            commands::video_source_watch,
            commands::stremio_watch,
            commands::video_set_player_labels,
            commands::video_player_report,
            commands::video_player_external,
            commands::video_external_players,
            commands::video_find_subtitles,
            commands::youtube_embed_url,
            commands::tts_report,
            commands::tts_set_available
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
            events::AiModelEvent,
            events::PodcastsChangedEvent,
            events::WatchChangedEvent,
            events::VideoDownloadsEvent,
            events::SongDownloadsEvent,
            events::SongDownloadProgressEvent,
            events::YtDlpEvent,
            events::AppUpdateEvent,
            events::MadeForYouEvent,
            events::TrackDurationsEvent,
            events::LocalFilesEvent,
            events::ImportEvent,
            events::CaptionsEvent,
            events::ArtistReleasesEvent,
            events::SettingsChangedEvent,
            events::IncognitoEvent,
            events::ExportEvent,
            events::VideoPlayerEvent,
            events::YouTubeVideoEvent,
            events::SpeechCommandEvent,
            events::ConnectEvent,
            events::ConnectRequestEvent
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
    let builder = tauri::Builder::default();
    // One running copy on desktop (M46.11): opening Raygun again brings its window forward, since
    // two copies would write to one database. The plugin must come first.
    #[cfg(not(any(target_os = "android", target_os = "ios")))]
    let builder = builder.plugin(tauri_plugin_single_instance::init(|app, _args, _cwd| {
        use tauri::Manager;
        if let Some(window) = app.get_webview_window("main") {
            let _ = window.unminimize();
            let _ = window.show();
            let _ = window.set_focus();
        }
    }));
    builder
        .plugin(tauri_plugin_raygun_audio::init())
        .plugin(tauri_plugin_notification::init())
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
