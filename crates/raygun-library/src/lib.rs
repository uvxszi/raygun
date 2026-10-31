//! Local media library scanning (ARCHITECTURE §5, M07).
//!
//! Two steps, both pure filesystem work with no database: [`discover`] lists audio files under the
//! library roots, and [`read_track`] reads one file's tags and embedded artwork. The caller diffs
//! discovered files against what it already knows (size + modification time) and only reads what
//! changed, which keeps rescans of large libraries fast.

use std::fs;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use lofty::picture::MimeType;
use lofty::prelude::*;
use walkdir::WalkDir;

/// Extensions treated as audio. Everything else is ignored without being opened.
pub const AUDIO_EXTENSIONS: [&str; 10] = [
    "mp3", "flac", "ogg", "oga", "opus", "m4a", "aac", "wav", "wma", "aiff",
];

pub const UNKNOWN_ARTIST: &str = "Unknown artist";
pub const UNKNOWN_ALBUM: &str = "Unknown album";

/// An audio file found on disk.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileEntry {
    pub path: PathBuf,
    pub size: u64,
    /// Seconds since the Unix epoch.
    pub modified: i64,
}

/// Everything the library stores about one file.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScannedTrack {
    pub file: FileEntry,
    pub title: String,
    pub artist: Option<String>,
    pub album_artist: Option<String>,
    pub album: Option<String>,
    pub track_no: Option<u32>,
    pub disc_no: Option<u32>,
    pub year: Option<u32>,
    pub genre: Option<String>,
    pub duration_ms: Option<u64>,
    /// File name of the extracted artwork inside the artwork folder.
    pub art: Option<String>,
}

impl ScannedTrack {
    /// Artist an album is filed under: the album artist, else the track artist.
    pub fn group_artist(&self) -> &str {
        self.album_artist
            .as_deref()
            .or(self.artist.as_deref())
            .unwrap_or(UNKNOWN_ARTIST)
    }

    pub fn group_album(&self) -> &str {
        self.album.as_deref().unwrap_or(UNKNOWN_ALBUM)
    }
}

/// Lists audio files under `roots`, skipping hidden folders. Unreadable folders are skipped
/// rather than failing the whole scan.
pub fn discover(roots: &[PathBuf]) -> Vec<FileEntry> {
    let mut files = Vec::new();
    for root in roots {
        let walker = WalkDir::new(root)
            .follow_links(false)
            .into_iter()
            .filter_entry(|entry| entry.depth() == 0 || !is_hidden(entry.file_name()));
        for entry in walker.filter_map(Result::ok) {
            if !entry.file_type().is_file() || !has_audio_extension(entry.path()) {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            files.push(FileEntry {
                path: entry.into_path(),
                size: metadata.len(),
                modified: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(UNIX_EPOCH).ok())
                    .map_or(0, |elapsed| elapsed.as_secs() as i64),
            });
        }
    }
    files.sort_by(|left, right| left.path.cmp(&right.path));
    files
}

/// Reads tags, duration and artwork. Files without tags still produce a track titled after the
/// file name; only files lofty can't parse at all are errors.
pub fn read_track(file: FileEntry, art_dir: &Path) -> Result<ScannedTrack, String> {
    let tagged = lofty::read_from_path(&file.path).map_err(|error| error.to_string())?;
    let duration = tagged.properties().duration();
    let duration_ms = (!duration.is_zero()).then_some(duration.as_millis() as u64);
    let tag = tagged.primary_tag().or_else(|| tagged.first_tag());

    let text = |value: Option<std::borrow::Cow<'_, str>>| {
        value
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty())
    };
    let fallback_title = file
        .path
        .file_stem()
        .map(|stem| stem.to_string_lossy().into_owned())
        .unwrap_or_default();

    let art = match tag.and_then(|tag| tag.pictures().first()) {
        Some(picture) => save_artwork(picture.data(), picture.mime_type(), art_dir),
        None => None,
    };

    Ok(ScannedTrack {
        title: text(tag.and_then(|tag| tag.title())).unwrap_or(fallback_title),
        artist: text(tag.and_then(|tag| tag.artist())),
        album_artist: tag
            .and_then(|tag| tag.get_string(ItemKey::AlbumArtist))
            .map(|text| text.trim().to_owned())
            .filter(|text| !text.is_empty()),
        album: text(tag.and_then(|tag| tag.album())),
        track_no: tag.and_then(|tag| tag.track()),
        disc_no: tag.and_then(|tag| tag.disk()),
        year: tag
            .and_then(|tag| tag.date())
            .map(|date| u32::from(date.year)),
        genre: text(tag.and_then(|tag| tag.genre())),
        duration_ms,
        art,
        file,
    })
}

/// Writes artwork once per distinct image, named by its hash, and returns the file name.
fn save_artwork(data: &[u8], mime: Option<&MimeType>, art_dir: &Path) -> Option<String> {
    if data.is_empty() {
        return None;
    }
    let extension = match mime {
        Some(MimeType::Png) => "png",
        Some(MimeType::Gif) => "gif",
        Some(MimeType::Bmp) => "bmp",
        _ => "jpg",
    };
    let name = format!("{}.{extension}", blake3::hash(data).to_hex());
    let path = art_dir.join(&name);
    if !path.exists()
        && let Err(error) = fs::create_dir_all(art_dir).and_then(|_| fs::write(&path, data))
    {
        tracing::warn!(%error, path = %path.display(), "could not save artwork");
        return None;
    }
    Some(name)
}

/// True for names produced by [`save_artwork`], so the artwork route can't be used to read
/// arbitrary files.
pub fn is_artwork_name(name: &str) -> bool {
    match name.split_once('.') {
        Some((hash, extension)) => {
            hash.len() == 64
                && hash.bytes().all(|byte| byte.is_ascii_hexdigit())
                && matches!(extension, "jpg" | "png" | "gif" | "bmp")
        }
        None => false,
    }
}

fn is_hidden(name: &std::ffi::OsStr) -> bool {
    name.to_string_lossy().starts_with('.')
}

fn has_audio_extension(path: &Path) -> bool {
    path.extension()
        .map(|extension| extension.to_string_lossy().to_ascii_lowercase())
        .is_some_and(|extension| AUDIO_EXTENSIONS.contains(&extension.as_str()))
}

pub mod fixtures;

#[cfg(test)]
mod tests;
