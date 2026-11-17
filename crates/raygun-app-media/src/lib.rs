//! Raygun's videos, comics and books, exports and downloads (M46.56): the video player's sessions and sources, Stremio, torrents, the comic and book libraries, Local Files, websites and their downloads.

pub mod book_search;
pub mod caches;
pub mod export;
pub mod library_actions;
pub mod local_files;
pub mod manga;
pub mod manga_downloads;
pub mod novel;
pub mod permissions;
pub mod stremio;
pub mod torrents;
pub mod video;
#[cfg(not(target_os = "android"))]
pub mod video_desktop;
pub mod video_downloads;
pub mod video_library;
pub mod video_links;
pub mod video_session;
pub mod video_sources;
pub mod video_subtitles;
pub mod website_downloads;
pub mod websites;

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
