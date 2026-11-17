//! Raygun's music and spoken word (M46.56): playback, songs' sources and downloads, the library and its lists, lyrics, podcasts and audiobooks, captions, reading aloud.

pub mod aniskip;
pub mod answer_words;
pub mod artist_profiles;
pub mod background;
pub mod captions;
pub mod collections;
pub mod cover_fallback;
pub mod details;
pub mod discovery;
pub mod durations;
pub mod follows;
pub mod fonts;
pub mod history;
pub mod hls_downloads;
pub mod import;
pub mod last_played;
pub mod library;
pub mod logs;
pub mod lyrics_store;
pub mod lyrics_translation;
pub mod made_for_you;
pub mod miniplayer;
pub mod music_downloads;
pub mod playback;
pub mod podcast_feed;
pub mod podcasts;
pub mod prefetch;
pub mod quick_access;
pub mod reader_ai;
pub mod resolution;
pub mod resolver;
pub mod smart_shuffle;
pub mod song_memory;
pub mod sound;
pub mod sponsorblock;
pub mod system_ui;
pub mod text_translation;
pub mod tts;
pub mod video_quality;
pub mod youtube_embed;
pub mod ytdlp;

#[allow(unused_imports)]
pub(crate) use raygun_app_base::{
    device_state, events, local_network, network, picker, privacy, settings,
};

/// The start-up's handles, at the paths the moved code names them by.
#[allow(unused_imports)]
pub(crate) mod startup {
    pub use raygun_app_base::{AppDb, AppPaths};
}

/// Exports' files, at the paths the moved code names them by.
#[allow(unused_imports)]
pub(crate) mod export {
    pub use raygun_app_export::output::{Outputs, Place};
    pub use raygun_app_export::places::{place_for, place_for_request};
    pub use raygun_app_export::{convert, names, output, remux, transcode};
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
pub(crate) use raygun_app_reading::{dictionary, epub};
#[allow(unused_imports)]
pub(crate) use raygun_app_sources::{
    ai, browser_check, catalog, comics, crash_log, extension_installs, extensions, genres, hidden,
    mihon, recent_searches, search, search_filter, servers, source_detect, source_queue, uploads,
    web_views,
};
