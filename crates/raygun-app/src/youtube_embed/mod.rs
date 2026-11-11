//! What YouTube's embedded players in Raygun's window are told: the video's original audio track
//! over dubbed ones (`YouTubePrefs::original_audio`) and the codec (`VideoPlayerPrefs::video_codec`).
//! They go in the embed host page's address (`audio=`, `codec=`); the page passes them on as player
//! parameters, so they reach YouTube's frame in its own address, where Raygun's player script
//! ([`PLAYER_SCRIPT`]) reads them at document start, before YouTube's player looks at codecs. A
//! changed setting changes the address, and the next load opens the page again with it.
//!
//! The same switch picks the original track among the audio files Raygun's YouTube module hands
//! out (`raygun_ext_host::youtube::set_original_audio`).

use std::sync::Mutex;

use raygun_model::{Settings, VideoCodec};

/// Raygun's player script, run in every frame of Raygun's main window: idle except in YouTube's
/// embedded player inside Raygun's embed host page.
pub const PLAYER_SCRIPT: &str = include_str!("player.js");

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct EmbedPrefs {
    original_audio: bool,
    codec: VideoCodec,
}

static PREFS: Mutex<EmbedPrefs> = Mutex::new(EmbedPrefs {
    original_audio: true,
    codec: VideoCodec::Auto,
});

fn prefs() -> EmbedPrefs {
    *PREFS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner)
}

/// Takes the settings YouTube's players follow (at start and when they change).
pub fn apply(settings: &Settings) {
    let next = EmbedPrefs {
        original_audio: settings.youtube.original_audio,
        codec: settings.video_player.video_codec,
    };
    *PREFS
        .lock()
        .unwrap_or_else(std::sync::PoisonError::into_inner) = next;
    raygun_ext_host::youtube::set_original_audio(next.original_audio);
}

/// The embed host page's address `base` (`http://localhost:<port>/embed/youtube`, with or without
/// its `controls`), saying what YouTube's player is to do.
pub fn url(base: &str) -> String {
    with_prefs(base, prefs())
}

fn with_prefs(base: &str, prefs: EmbedPrefs) -> String {
    let Ok(mut url) = url::Url::parse(base) else {
        return base.to_owned();
    };
    url.query_pairs_mut()
        .append_pair(
            "audio",
            if prefs.original_audio {
                "original"
            } else {
                "default"
            },
        )
        .append_pair(
            "codec",
            match prefs.codec {
                VideoCodec::Auto => "auto",
                VideoCodec::H264 => "h264",
            },
        );
    url.into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_embed_page_is_told_the_audio_track_and_codec_in_its_address() {
        let prefs = EmbedPrefs {
            original_audio: true,
            codec: VideoCodec::H264,
        };
        assert_eq!(
            with_prefs("http://localhost:4100/embed/youtube?controls=1", prefs),
            "http://localhost:4100/embed/youtube?controls=1&audio=original&codec=h264"
        );
        let plain = EmbedPrefs {
            original_audio: false,
            codec: VideoCodec::Auto,
        };
        assert_eq!(
            with_prefs("http://localhost:4100/embed/youtube", plain),
            "http://localhost:4100/embed/youtube?audio=default&codec=auto"
        );
    }
}
