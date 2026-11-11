//! Torrent streaming for the video player: a torrent's video is read in order as its pieces
//! arrive, the pieces around each read position fetched first, so the stream proxy can serve it
//! as a file with byte ranges (`raygun_net::TorrentLinks`). Built on librqbit (Apache-2.0).
//!
//! - [`link`]: magnet links and `.torrent` addresses, with the file or episode wanted.
//! - [`pick`]: which of a torrent's files plays.
//! - [`subtitles`]: the subtitle files that come with it, offered as subtitle tracks.
//! - [`engine`]: the session: opening, reading, progress, closing, downloading whole files.
//! - [`announce`]: a `.torrent` file's trackers kept to those the engine can use.
//!
//! Raygun suggests no torrent source; links come from sources people add, and from magnet
//! links they open.

pub mod announce;
pub mod engine;
pub mod link;
pub mod pick;
pub mod subtitles;

use std::sync::LazyLock;

pub use engine::{
    Engine, EngineConfig, Opened, OpenedSubtitle, Progress, Reader, Source, TorrentError, gigabytes,
};
pub use link::{FORMAT, FileHint, Magnet, TorrentLink, is_torrent, parse, with_episodes};
pub use pick::{TorrentFile, is_video, pick_file};
pub use subtitles::{SubtitleFile, subtitles_for};

static SEEDS: LazyLock<regex::Regex> = LazyLock::new(|| {
    regex::Regex::new(r"(?i)(?:👤|\bseed(?:er)?s?\b\s*[:=]?)\s*(\d[\d,.]*)").expect("valid pattern")
});

/// The seeds a stream's name or description says it has: Torrentio's "👤 42", "Seeders: 42".
pub fn seeds_in(text: &str) -> Option<u32> {
    let found = SEEDS.captures(text)?;
    found[1]
        .chars()
        .filter(char::is_ascii_digit)
        .collect::<String>()
        .parse()
        .ok()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn seeds_are_read_from_stream_names() {
        assert_eq!(
            seeds_in("Sintel.2010.1080p.mkv\n👤 46 💾 1.98 GB ⚙️ Example"),
            Some(46)
        );
        assert_eq!(seeds_in("Seeders: 1,204 · 2.1 GB"), Some(1204));
        assert_eq!(seeds_in("seeds 7"), Some(7));
        assert_eq!(seeds_in("1080p WEB-DL"), None);
    }
}
