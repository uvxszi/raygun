//! M3U and PLS playlists (pure): their entries in order, where each one is, and whether a
//! `.m3u8` is an HLS stream instead.

use std::collections::BTreeMap;
use std::path::PathBuf;

use super::route::{self, Location};

/// Entries read from one playlist at most.
pub const MAX_ENTRIES: usize = 1000;

/// One entry, as the playlist writes it.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Entry {
    /// An address or a path, maybe relative to the playlist.
    pub location: String,
    pub title: Option<String>,
    pub artist: Option<String>,
    pub duration_ms: Option<u32>,
}

/// What relative entries are relative to: the playlist's own address or folder.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Base {
    Web(url::Url),
    Folder(PathBuf),
    /// A document with no path known: only absolute entries can be found.
    Nowhere,
}

/// An HLS playlist (a stream's variants or its segments) rather than a list of songs.
pub fn is_hls(text: &str) -> bool {
    text.lines().any(|line| {
        let line = line.trim_start();
        [
            "#EXT-X-TARGETDURATION",
            "#EXT-X-STREAM-INF",
            "#EXT-X-MEDIA-SEQUENCE",
        ]
        .iter()
        .any(|tag| line.starts_with(tag))
    })
}

/// The entries of an M3U (`#EXTINF` names and lengths) or PLS playlist, in order.
pub fn entries(text: &str) -> Vec<Entry> {
    let text = text.trim_start_matches('\u{feff}');
    let pls = text
        .lines()
        .map(str::trim)
        .find(|line| !line.is_empty())
        .is_some_and(|first| first.eq_ignore_ascii_case("[playlist]"));
    let mut entries = if pls {
        pls_entries(text)
    } else {
        m3u_entries(text)
    };
    entries.truncate(MAX_ENTRIES);
    entries
}

fn m3u_entries(text: &str) -> Vec<Entry> {
    let mut entries = Vec::new();
    let mut described: Option<Entry> = None;
    for line in text.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        if let Some(info) = strip_prefix_ignoring_case(line, "#EXTINF:") {
            described = Some(extinf(info));
            continue;
        }
        if line.starts_with('#') {
            continue;
        }
        let entry = described.take().unwrap_or_default();
        entries.push(Entry {
            location: line.to_owned(),
            ..entry
        });
    }
    entries
}

/// `#EXTINF:<seconds> <attributes>,<artist> - <title>`; IPTV lists' attributes may hold commas
/// inside quotes.
fn extinf(info: &str) -> Entry {
    let mut quoted = false;
    let comma = info.char_indices().find_map(|(index, char)| {
        if char == '"' {
            quoted = !quoted;
        }
        (char == ',' && !quoted).then_some(index)
    });
    let (head, name) = match comma {
        Some(index) => (&info[..index], &info[index + 1..]),
        None => (info, ""),
    };
    let duration_ms = head
        .split_whitespace()
        .next()
        .and_then(|seconds| seconds.parse::<f64>().ok())
        .filter(|seconds| *seconds > 0.0)
        .map(|seconds| (seconds * 1000.0).round() as u32);
    let (artist, title) = names(name);
    Entry {
        location: String::new(),
        title,
        artist,
        duration_ms,
    }
}

/// `Artist - Title`, or a title alone.
fn names(text: &str) -> (Option<String>, Option<String>) {
    let text = text.trim();
    match text.split_once(" - ") {
        Some((artist, title)) if !artist.trim().is_empty() && !title.trim().is_empty() => (
            Some(artist.trim().to_owned()),
            Some(title.trim().to_owned()),
        ),
        _ => (None, Some(text.to_owned()).filter(|text| !text.is_empty())),
    }
}

fn pls_entries(text: &str) -> Vec<Entry> {
    let mut numbered: BTreeMap<u32, Entry> = BTreeMap::new();
    for line in text.lines() {
        let Some((key, value)) = line.trim().split_once('=') else {
            continue;
        };
        let key = key.trim().to_ascii_lowercase();
        let digits = key.trim_start_matches(|char: char| char.is_ascii_alphabetic());
        let Ok(number) = digits.parse::<u32>() else {
            continue;
        };
        let value = value.trim();
        let entry = numbered.entry(number).or_default();
        match &key[..key.len() - digits.len()] {
            "file" => entry.location = value.to_owned(),
            "title" => (entry.artist, entry.title) = names(value),
            "length" => {
                entry.duration_ms = value
                    .parse::<i64>()
                    .ok()
                    .filter(|seconds| *seconds > 0)
                    .and_then(|seconds| u32::try_from(seconds * 1000).ok());
            }
            _ => {}
        }
    }
    numbered
        .into_values()
        .filter(|entry| !entry.location.is_empty())
        .collect()
}

fn strip_prefix_ignoring_case<'a>(text: &'a str, prefix: &str) -> Option<&'a str> {
    let head = text.get(..prefix.len())?;
    head.eq_ignore_ascii_case(prefix)
        .then(|| &text[prefix.len()..])
}

/// Where an entry is: an address or absolute path as written, else beside the playlist.
pub fn resolve(entry: &str, base: &Base) -> Option<Location> {
    let entry = entry.trim();
    if let Some(location) = route::location(entry) {
        return Some(location);
    }
    match base {
        Base::Web(playlist) => playlist
            .join(entry)
            .ok()
            .and_then(|url| route::location(url.as_str())),
        // Playlists made on Windows name songs with backslashes.
        Base::Folder(folder) => {
            let relative = entry.replace('\\', "/");
            let relative = relative.trim_start_matches("./");
            (!relative.is_empty()).then(|| Location::File(folder.join(relative)))
        }
        Base::Nowhere => None,
    }
}
