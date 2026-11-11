//! Where a handed-over address plays, from its type and name, and what shared text asks for
//! (pure).

use std::path::{Path, PathBuf};

use crate::local_files::percent_decode;

/// Where an address plays.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind {
    /// A video file or stream: the video player.
    Video,
    /// An HLS or DASH stream: the video player, told its format.
    Streaming(StreamFormat),
    /// A song or other sound: the music player.
    Audio,
    /// A list of songs or radio streams (M3U, PLS), queued in order. A `.m3u8` file can turn out
    /// to be HLS once it's read.
    Playlist,
    /// A book (PDF or EPUB): imported into the library and opened in the reader.
    Book,
    /// Not something Raygun plays.
    Other,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StreamFormat {
    Hls,
    Dash,
}

impl StreamFormat {
    /// The video player's name for it.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Hls => "hls",
            Self::Dash => "dash",
        }
    }
}

/// Where an address points.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    /// `http(s)://…`, its scheme in lower case.
    Web(String),
    /// A file on this device: `file://…`, or a plain path (the desktop app's arguments).
    File(PathBuf),
    /// An Android document (`content://…`), read through the access the sharing app gave.
    Document(String),
}

impl Location {
    /// The address again, for routing by name: a path as it is.
    pub fn address(&self) -> String {
        match self {
            Self::Web(url) | Self::Document(url) => url.clone(),
            Self::File(path) => path.to_string_lossy().into_owned(),
        }
    }
}

const VIDEO_EXTENSIONS: &[&str] = &[
    "mp4", "m4v", "mkv", "webm", "mov", "avi", "flv", "3gp", "3g2", "ts", "m2ts", "mts", "mpg",
    "mpeg", "ogv",
];

const AUDIO_EXTENSIONS: &[&str] = &[
    "mp3", "m4a", "m4b", "aac", "flac", "ogg", "oga", "opus", "wav", "wave", "amr", "mka", "weba",
];

/// Types that say nothing: the name decides.
const VAGUE_TYPES: &[&str] = &[
    "",
    "*/*",
    "application/octet-stream",
    "binary/octet-stream",
    "application/unknown",
    "application/x-unknown",
    "application/binary",
    "application/force-download",
    "application/x-download",
];

/// An address's scheme in lower case; none for a Windows path (`C:\…`).
fn scheme_of(address: &str) -> Option<String> {
    let (scheme, _) = address.split_once(':')?;
    let valid = scheme.len() > 1
        && scheme.starts_with(|first: char| first.is_ascii_alphabetic())
        && scheme
            .chars()
            .all(|char| char.is_ascii_alphanumeric() || matches!(char, '+' | '-' | '.'));
    valid.then(|| scheme.to_ascii_lowercase())
}

/// Where `address` points, or `None` when it's no address Raygun opens.
pub fn location(address: &str) -> Option<Location> {
    let address = address.trim();
    match scheme_of(address).as_deref() {
        Some(scheme @ ("http" | "https")) => Some(Location::Web(format!(
            "{scheme}{}",
            &address[scheme.len()..]
        ))),
        Some("content") => Some(Location::Document(address.to_owned())),
        Some("file") => file_path(address).map(Location::File),
        Some(_) => None,
        None => {
            let path = Path::new(address);
            path.is_absolute()
                .then(|| Location::File(path.to_path_buf()))
        }
    }
}

/// The path of a `file:` address: `file:///storage/…`, `file://localhost/…`, `file:///C:/…`.
pub fn file_path(address: &str) -> Option<PathBuf> {
    let rest = &address[address.find(':')? + 1..];
    let rest = match rest.strip_prefix("//") {
        Some(after) => after.strip_prefix("localhost").unwrap_or(after),
        None => rest,
    };
    // `file://server/share/…` is another computer's file.
    if !rest.starts_with('/') {
        return None;
    }
    let decoded = percent_decode(rest.split(['?', '#']).next().unwrap_or(rest));
    let bytes = decoded.as_bytes();
    let windows_drive = bytes.len() > 2 && bytes[1].is_ascii_alphabetic() && bytes[2] == b':';
    Some(PathBuf::from(if windows_drive {
        &decoded[1..]
    } else {
        decoded.as_str()
    }))
}

/// The last part of an address or path, decoded, without its query.
fn last_part(address: &str) -> String {
    let without_query = match scheme_of(address).as_deref() {
        Some("http" | "https") => address.split(['?', '#']).next().unwrap_or(address),
        _ => address,
    };
    let decoded = percent_decode(without_query);
    let trimmed = decoded.trim_end_matches(['/', '\\']);
    trimmed
        .rsplit(['/', '\\'])
        .next()
        .unwrap_or(trimmed)
        .to_owned()
}

/// A file name's extension in lower case (`Song.MP3` → `mp3`).
fn extension(name: &str) -> Option<String> {
    let name = name.rsplit(['/', '\\']).next().unwrap_or(name);
    let (_, extension) = name.rsplit_once('.')?;
    let valid = (1..=5).contains(&extension.len())
        && extension.chars().all(|char| char.is_ascii_alphanumeric());
    valid.then(|| extension.to_ascii_lowercase())
}

/// Where a handed-over address plays: by its type (`mime`), else by its name or the address's
/// own name.
pub fn kind_of(address: &str, mime: Option<&str>, name: Option<&str>) -> Kind {
    let Some(location) = location(address) else {
        return Kind::Other;
    };
    let remote = matches!(location, Location::Web(_));
    let extension = name
        .and_then(extension)
        .or_else(|| extension(&last_part(address)));
    mime.and_then(|mime| by_type(mime, remote, extension.as_deref()))
        .unwrap_or_else(|| {
            extension
                .as_deref()
                .map_or(Kind::Other, |extension| by_extension(extension, remote))
        })
}

fn by_type(mime: &str, remote: bool, extension: Option<&str>) -> Option<Kind> {
    let essence = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    if VAGUE_TYPES.contains(&essence.as_str()) {
        return None;
    }
    let by_name = || {
        extension
            .map(|extension| by_extension(extension, remote))
            .filter(|kind| *kind != Kind::Other)
    };
    Some(match essence.as_str() {
        // HLS from the web. A file, or a list of radio streams, is read first: the text says.
        "application/vnd.apple.mpegurl" if remote => Kind::Streaming(StreamFormat::Hls),
        "application/vnd.apple.mpegurl"
        | "application/x-mpegurl"
        | "audio/mpegurl"
        | "audio/x-mpegurl" => {
            if remote && extension == Some("m3u8") {
                Kind::Streaming(StreamFormat::Hls)
            } else {
                Kind::Playlist
            }
        }
        "audio/x-scpls" | "audio/scpls" | "application/pls+xml" | "application/pls" => {
            Kind::Playlist
        }
        "application/dash+xml" => Kind::Streaming(StreamFormat::Dash),
        // Containers for either: the name says which, else what they usually hold.
        "application/ogg" | "application/x-ogg" => by_name().unwrap_or(Kind::Audio),
        "application/mp4" | "application/x-matroska" => by_name().unwrap_or(Kind::Video),
        "application/x-flac" => Kind::Audio,
        "application/pdf" | "application/x-pdf" | "application/epub+zip" => Kind::Book,
        essence if essence.starts_with("video/") => Kind::Video,
        essence if essence.starts_with("audio/") => Kind::Audio,
        // A picture, a document: the name may still say (a file manager's guess).
        _ => return None,
    })
}

fn by_extension(extension: &str, remote: bool) -> Kind {
    match extension {
        "m3u8" if remote => Kind::Streaming(StreamFormat::Hls),
        "m3u8" | "m3u" | "pls" => Kind::Playlist,
        "mpd" => Kind::Streaming(StreamFormat::Dash),
        "pdf" | "epub" => Kind::Book,
        // On the web `.ts` is as likely a script as a video.
        "ts" if remote => Kind::Other,
        extension if VIDEO_EXTENSIONS.contains(&extension) => Kind::Video,
        extension if AUDIO_EXTENSIONS.contains(&extension) => Kind::Audio,
        _ => Kind::Other,
    }
}

/// What a handed-over file is called: its name without the extension, else the file's or the
/// web address's (a document's address names nothing).
pub fn title_of(name: Option<&str>, address: &str) -> Option<String> {
    let stem = |name: &str| {
        let name = name.trim();
        let stem = match extension(name) {
            Some(extension) => &name[..name.len() - extension.len() - 1],
            None => name,
        };
        Some(stem.trim().to_owned()).filter(|stem| !stem.is_empty())
    };
    if let Some(title) = name.and_then(stem) {
        return Some(title);
    }
    match location(address)? {
        Location::Web(url) => stem(&last_part(&url)),
        Location::File(path) => path
            .file_name()
            .and_then(|name| stem(&name.to_string_lossy())),
        Location::Document(_) => None,
    }
}

/// How a handed-over item is named in a problem: its name, else its address's last part.
pub fn shown_name(name: Option<&str>, address: &str) -> String {
    name.map(str::trim)
        .filter(|name| !name.is_empty())
        .map(str::to_owned)
        .or_else(|| match location(address) {
            Some(Location::Document(_)) | None => None,
            Some(_) => Some(last_part(address)).filter(|part| !part.is_empty()),
        })
        .unwrap_or_else(|| address.trim().to_owned())
}

/// The longest search made from shared words.
const SEARCH_CHARS: usize = 200;

/// What shared text asks for.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TextTarget {
    /// A link to something Raygun plays.
    Media { url: String, kind: Kind },
    /// A Spotify playlist, album or song: the song import reads it.
    Import(String),
    /// A search for it; a YouTube link shows its card there.
    Search(String),
}

/// What shared text asks for: its first link, else a search for its words.
pub fn text_target(text: &str) -> Option<TextTarget> {
    if let Some(url) = first_link(text) {
        let kind = kind_of(&url, None, None);
        return Some(if kind != Kind::Other {
            TextTarget::Media { url, kind }
        } else if crate::import::parse::spotify_link(&url).is_some() {
            TextTarget::Import(url)
        } else {
            TextTarget::Search(url)
        });
    }
    let words = text.split_whitespace().collect::<Vec<_>>().join(" ");
    let words: String = words.chars().take(SEARCH_CHARS).collect();
    let words = words.trim();
    (!words.is_empty()).then(|| TextTarget::Search(words.to_owned()))
}

/// The first web address in `text`, without the punctuation around it.
pub fn first_link(text: &str) -> Option<String> {
    let lower = text.to_ascii_lowercase();
    let start = [lower.find("https://"), lower.find("http://")]
        .into_iter()
        .flatten()
        .min()?;
    let link: String = text[start..]
        .chars()
        .take_while(|char| !char.is_whitespace() && !matches!(char, '<' | '>' | '"' | '“' | '”'))
        .collect();
    let mut link = link.trim_end_matches(['.', ',', ';', ':', '!', '?', '\'', ']', '}']);
    // A closing bracket belongs to the text around it unless the link opened one.
    if !link.contains('(') {
        link = link.trim_end_matches(')');
    }
    let scheme = if lower[start..].starts_with("https://") {
        "https://"
    } else {
        "http://"
    };
    (link.len() > scheme.len()).then(|| link.to_owned())
}

/// A name for the copy of a shared document: its own name made safe for a file, with an extension
/// the tag reader knows it by (from its type when the name has none).
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
pub fn copy_name(name: Option<&str>, mime: Option<&str>) -> String {
    const LONGEST: usize = 100;
    let cleaned: String = name
        .unwrap_or_default()
        .chars()
        .map(|char| {
            if char.is_control()
                || matches!(char, '/' | '\\' | ':' | '*' | '?' | '"' | '<' | '>' | '|')
            {
                '_'
            } else {
                char
            }
        })
        .collect();
    let cleaned = cleaned.trim().trim_matches('.').trim();
    let (stem, extension) = match extension(cleaned) {
        Some(extension) if known_extension(&extension) => (
            cleaned[..cleaned.len() - extension.len() - 1]
                .trim()
                .to_owned(),
            extension,
        ),
        _ => (
            cleaned.to_owned(),
            mime.and_then(extension_for_type)
                .unwrap_or("mp3")
                .to_owned(),
        ),
    };
    let stem: String = stem.chars().take(LONGEST).collect();
    let stem = if stem.trim().is_empty() {
        "Song".to_owned()
    } else {
        stem.trim().to_owned()
    };
    format!("{stem}.{extension}")
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn known_extension(extension: &str) -> bool {
    AUDIO_EXTENSIONS.contains(&extension)
        || VIDEO_EXTENSIONS.contains(&extension)
        || matches!(extension, "m3u" | "m3u8" | "pls")
}

/// The usual extension for a song's or playlist's type.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn extension_for_type(mime: &str) -> Option<&'static str> {
    let essence = mime
        .split(';')
        .next()
        .unwrap_or_default()
        .trim()
        .to_ascii_lowercase();
    Some(match essence.as_str() {
        "audio/mpeg" | "audio/mp3" | "audio/mpeg3" | "audio/x-mpeg" => "mp3",
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" | "audio/mp4a-latm" => "m4a",
        "audio/aac" | "audio/x-aac" | "audio/aacp" => "aac",
        "audio/flac" | "audio/x-flac" | "application/x-flac" => "flac",
        "audio/ogg" | "application/ogg" | "audio/vorbis" => "ogg",
        "audio/opus" => "opus",
        "audio/wav" | "audio/x-wav" | "audio/wave" | "audio/vnd.wave" => "wav",
        "audio/amr" => "amr",
        "audio/x-matroska" => "mka",
        "audio/webm" => "weba",
        "audio/x-mpegurl" | "audio/mpegurl" | "application/x-mpegurl" => "m3u",
        "application/vnd.apple.mpegurl" => "m3u8",
        "audio/x-scpls" | "audio/scpls" | "application/pls+xml" | "application/pls" => "pls",
        _ => return None,
    })
}
