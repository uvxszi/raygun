//! Raygun across your devices (M46.56): Connect, device sync, one device link, the LAN remote, a linked browser as a viewer, and the MCP server's tools.

pub mod connect;
pub mod device_link;
pub mod device_sync;
pub mod mcp;
pub mod remote;
pub mod web_remote;

#[allow(unused_imports)]
pub(crate) use raygun_app_base::{
    device_state, events, local_network, network, picker, privacy, settings,
};

/// The start-up's handles, at the paths the moved code names them by.
#[allow(unused_imports)]
pub(crate) mod startup {
    pub use raygun_app_base::{AppDb, AppPaths};
}

/// Notices, at the paths the moved code names them by.
#[allow(unused_imports)]
pub(crate) mod notifications {
    pub use raygun_app_base::news;
    pub use raygun_app_base::notices::*;
}

#[allow(unused_imports)]
pub(crate) use raygun_app_base::network_mode;
#[allow(unused_imports)]
pub(crate) use raygun_app_media::{
    book_search, caches, export, library_actions, local_files, manga, manga_downloads, novel,
    permissions, stremio, torrents, video, video_downloads, video_library, video_links,
    video_session, video_sources, video_subtitles, website_downloads, websites,
};
#[allow(unused_imports)]
pub(crate) use raygun_app_music::{
    aniskip, answer_words, artist_profiles, background, captions, collections, cover_fallback,
    details, discovery, durations, follows, fonts, history, hls_downloads, import, last_played,
    library, logs, lyrics_store, lyrics_translation, made_for_you, miniplayer, music_downloads,
    playback, podcast_feed, podcasts, prefetch, quick_access, reader_ai, resolution, resolver,
    song_memory, sound, sponsorblock, system_ui, text_translation, tts, video_quality,
    youtube_embed, ytdlp,
};
#[allow(unused_imports)]
pub(crate) use raygun_app_net::cast;
#[allow(unused_imports)]
pub(crate) use raygun_app_reading::{dictionary, epub};
#[allow(unused_imports)]
pub(crate) use raygun_app_sources::{
    ai, browser_check, catalog, comics, crash_log, extension_installs, extensions, genres, hidden,
    mihon, recent_searches, search, search_filter, servers, source_detect, source_queue, uploads,
    web_views,
};

/// The bindings file in this checkout (for tests that read it).
#[cfg(test)]
pub(crate) fn workspace_bindings_path() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(2)
        .expect("crate lives two levels below the workspace root")
        .join("packages/bindings/src/index.ts")
}
