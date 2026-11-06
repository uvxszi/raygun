//! Local Files (M46.27): folders the user adds, scanned for everything Raygun plays or reads.
//! Songs join the music library (the folder becomes one of its roots), comics (CBZ/ZIP, or a
//! folder of images) and books (EPUB) are imported as they are from uploads, and videos become
//! titles of the source `local.video` in Library → Videos, played from where they are.
//!
//! Each folder says how it's read (M46.51, [`FolderAccess`]):
//! - by path: a desktop folder, or (Android) one chosen in Raygun's own folder browser over
//!   shared storage, because Android's picker refuses the storage's top, Download and
//!   Android/data. With the media permissions Android shows songs, videos and pictures there;
//!   with access to all files (optional) every file, books and comic archives too.
//! - through the system picker's grant (Android: SD cards, USB drives, cloud folders): the plugin
//!   lists the folder and opens each comic, book and picture, because Rust can't open
//!   `content://`; songs and videos are read by path, which the media permissions allow.
//! - Raygun's own Added files folder: songs and videos added one by one with Add files, copied
//!   in (comics and books added that way are imported at once).
//!
//! A scan reports how far it got, remembers files that couldn't be added (they're tried again
//! only when they change or the scan is asked for), and leaves everything as it was when the
//! folder can't be opened (an unplugged drive, a moved folder, a lost grant or permission).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Component, Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_db::local_folders::{self as store, FileRow, FolderAccess, NewFolder};
use raygun_db::titles::{self, TitleFields, UnitFields};
use raygun_model::{
    LocalAddFailure, LocalBrowse, LocalBrowseCrumb, LocalBrowseFolder, LocalCounts,
    LocalFilesAdded, LocalFolder, LocalFolderAccess, LocalScanProgress, LocalStorageView,
};
use raygun_resolver::video_match;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::LocalFilesEvent;
use crate::startup::AppDb;

/// The source of videos found in Local Files folders.
pub const LOCAL_VIDEO: &str = "local.video";
/// Files listed in one folder, at most.
const MAX_FILES: usize = 20_000;
/// Comics larger than this aren't imported.
const MAX_IMPORT_BYTES: u64 = 512 * 1024 * 1024;
/// A folder with more pictures than this is a photo collection, not a chapter.
const MAX_COMIC_PAGES: usize = 1_500;
/// How often a running scan reports how far it got.
const PROGRESS_EVERY: Duration = Duration::from_millis(500);
/// What a file that isn't anything Raygun reads is kept as, so it isn't looked at again.
const SKIPPED: &str = "skipped";
/// Raygun's own folder of songs and videos added one by one, in its downloads.
const ADDED_DIR: &str = "added";
const ADDED_NAME: &str = "Added files";
/// The folder browser stops counting a folder's files after this many entries…
const COUNT_ENTRIES: usize = 25_000;
/// …and every folder's after this long.
const COUNT_TIME: Duration = Duration::from_secs(2);
/// What the system's file picker offers for Add files. Providers name comic archives in many
/// ways (or not at all), hence the plain binary type.
const PICK_TYPES: &[&str] = &[
    "audio/*",
    "video/*",
    "image/*",
    "application/epub+zip",
    "application/zip",
    "application/x-zip-compressed",
    "application/vnd.comicbook+zip",
    "application/x-cbz",
    "application/octet-stream",
];

const SONGS: &[&str] = &[
    "mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wma", "aiff",
];
const VIDEOS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "ts"];
const IMAGES: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif"];
/// Folders whose pictures are photos or artwork, never a comic.
const PICTURE_FOLDERS: &[&str] = &[
    "dcim",
    "camera",
    "screenshots",
    "screen recordings",
    "pictures",
    "photos",
    "wallpapers",
    "whatsapp images",
    "telegram images",
    "thumbnails",
    "artwork",
    "covers",
    "scans",
    "booklet",
];
/// How cameras and phones name their pictures.
const PHOTO_PREFIXES: &[&str] = &[
    "img_",
    "img-",
    "pxl_",
    "dsc",
    "screenshot",
    "mvimg",
    "pano_",
    "vid_",
    "photo_",
];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Kind {
    Song,
    Comic,
    Book,
    Video,
    Image,
}

fn kind_of(name: &str) -> Option<Kind> {
    let extension = name.rsplit_once('.')?.1.to_ascii_lowercase();
    let extension = extension.as_str();
    if SONGS.contains(&extension) {
        Some(Kind::Song)
    } else if VIDEOS.contains(&extension) {
        Some(Kind::Video)
    } else if IMAGES.contains(&extension) {
        Some(Kind::Image)
    } else if matches!(extension, "cbz" | "zip") {
        Some(Kind::Comic)
    } else if extension == "epub" {
        Some(Kind::Book)
    } else {
        None
    }
}

/// Every extension [`kind_of`] knows.
fn extensions() -> Vec<&'static str> {
    [SONGS, VIDEOS, IMAGES, &["cbz", "zip", "epub"]].concat()
}

/// A picked file's name with an extension Raygun knows: some providers name files without one,
/// and then the type they give says what it is.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn named_for_kind(name: &str, mime: Option<&str>) -> String {
    if kind_of(name).is_some() {
        return name.to_owned();
    }
    let extension = match mime.unwrap_or_default().to_ascii_lowercase().as_str() {
        "application/epub+zip" => "epub",
        "application/vnd.comicbook+zip" | "application/x-cbz" => "cbz",
        "application/zip" | "application/x-zip-compressed" => "zip",
        "audio/mpeg" | "audio/mp3" => "mp3",
        "audio/flac" | "audio/x-flac" => "flac",
        "audio/mp4" | "audio/x-m4a" | "audio/m4a" => "m4a",
        "audio/aac" => "aac",
        "audio/ogg" => "ogg",
        "audio/opus" => "opus",
        "audio/wav" | "audio/x-wav" | "audio/wave" => "wav",
        "video/mp4" => "mp4",
        "video/x-matroska" => "mkv",
        "video/webm" => "webm",
        "video/quicktime" => "mov",
        "image/jpeg" => "jpg",
        "image/png" => "png",
        "image/webp" => "webp",
        "image/gif" => "gif",
        "image/avif" => "avif",
        _ => return name.to_owned(),
    };
    format!("{name}.{extension}")
}

fn access_of(access: FolderAccess) -> LocalFolderAccess {
    match access {
        FolderAccess::Path => LocalFolderAccess::Path,
        FolderAccess::Tree => LocalFolderAccess::Tree,
        FolderAccess::Added => LocalFolderAccess::Added,
    }
}

fn path_string(path: &Path) -> String {
    path.to_string_lossy().into_owned()
}

/// What a restore adds of a backup's folders: those not here already, once each by location.
fn new_folders(folders: Vec<store::FolderRow>, have: &HashSet<String>) -> Vec<store::FolderRow> {
    let mut seen = have.clone();
    folders
        .into_iter()
        .filter(|folder| seen.insert(folder.location.clone()))
        .collect()
}

/// Whether a folder read by its path (Added files too) is still a folder here. Blocking.
fn path_is_here(folder: &store::FolderRow) -> bool {
    Path::new(folder.path.as_deref().unwrap_or(&folder.location)).is_dir()
}

/// `path` as a folder inside `root` (or `root` itself), when it is one: absolute, without `.` or
/// `..` steps, and under `root` component by component.
fn inside(root: &Path, path: &Path) -> Option<PathBuf> {
    if !path.is_absolute()
        || path
            .components()
            .any(|step| matches!(step, Component::CurDir | Component::ParentDir))
    {
        return None;
    }
    let clean: PathBuf = path.components().collect();
    clean.starts_with(root).then_some(clean)
}

/// The folders from below `root` down to `folder`, for the browser's trail.
fn crumbs(root: &Path, folder: &Path) -> Vec<LocalBrowseCrumb> {
    let Ok(below) = folder.strip_prefix(root) else {
        return Vec::new();
    };
    let mut path = root.to_path_buf();
    below
        .components()
        .map(|step| {
            path.push(step);
            LocalBrowseCrumb {
                name: step.as_os_str().to_string_lossy().into_owned(),
                path: path_string(&path),
            }
        })
        .collect()
}

/// Until when the folder browser goes on counting files.
struct Budget {
    until: Instant,
}

impl Budget {
    fn new(time: Duration) -> Self {
        Self {
            until: Instant::now() + time,
        }
    }

    fn spent(&self) -> bool {
        Instant::now() >= self.until
    }
}

/// The files Raygun reads under `dir`, hidden folders left out (as [`walk`] leaves them);
/// `None` when there were more than `entries` to look at or the time ran out.
fn count_items(dir: &Path, entries: usize, budget: &Budget) -> Option<u32> {
    let mut count = 0u32;
    let mut seen = 0usize;
    let mut pending = vec![dir.to_path_buf()];
    while let Some(dir) = pending.pop() {
        let Ok(listing) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in listing.filter_map(Result::ok) {
            seen += 1;
            if seen > entries || (seen.is_multiple_of(256) && budget.spent()) {
                return None;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            match entry.file_type() {
                Ok(kind) if kind.is_dir() => pending.push(entry.path()),
                Ok(kind) if kind.is_file() && kind_of(&name).is_some() => count += 1,
                _ => {}
            }
        }
    }
    Some(count)
}

/// A folder as the browser shows it when every file reads by path: its folders holding anything
/// Raygun reads (or too many files to count in time), by name, and the files right in it. `None`
/// when it can't be read.
fn list_folders(
    dir: &Path,
    entries: usize,
    budget: &Budget,
) -> Option<(Vec<LocalBrowseFolder>, u32)> {
    let listing = std::fs::read_dir(dir).ok()?;
    let mut folders = Vec::new();
    let mut items = 0;
    for entry in listing.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name.starts_with('.') {
            continue;
        }
        match entry.file_type() {
            Ok(kind) if kind.is_dir() => folders.push((name, entry.path())),
            Ok(kind) if kind.is_file() && kind_of(&name).is_some() => items += 1,
            _ => {}
        }
    }
    folders.sort_by(|(a, _), (b, _)| by_name(a, b));
    let folders = folders
        .into_iter()
        .map(|(name, path)| LocalBrowseFolder {
            // Once the time is spent, the rest go uncounted.
            items: if budget.spent() {
                None
            } else {
                count_items(&path, entries, budget)
            },
            path: path_string(&path),
            name,
        })
        .filter(|folder| folder.items != Some(0))
        .collect();
    Some((folders, items))
}

fn by_name(a: &str, b: &str) -> std::cmp::Ordering {
    crate::comics::natural_cmp(&a.to_lowercase(), &b.to_lowercase())
}

/// A name for pictures added together as one comic: what their names share, without the page
/// number they differ in ("Chapter 5 - 001.jpg", "Chapter 5 - 002.jpg": "Chapter 5"; "Saga
/// p01.png", "Saga p02.png": "Saga"). One picture keeps its own name.
fn pictures_name(names: &[&str]) -> String {
    const PAGE_WORDS: &[&str] = &["p", "pg", "page", "pages", "img", "image", "scan"];
    let separators = |c: char| c.is_whitespace() || matches!(c, '-' | '_' | '.' | '#' | '(' | '[');
    let stems: Vec<String> = names.iter().map(|name| stem(name)).collect();
    let Some(first) = stems.first() else {
        return "Pictures".into();
    };
    let shared = stems[1..]
        .iter()
        .fold(first.chars().count(), |shared, other| {
            shared.min(
                first
                    .chars()
                    .zip(other.chars())
                    .take_while(|(a, b)| a == b)
                    .count(),
            )
        });
    let mut name: String = first.chars().take(shared).collect();
    if stems.len() > 1 {
        // The start of the page number, then a page marker before it.
        name = name
            .trim_end_matches(|c: char| c.is_ascii_digit())
            .trim_end_matches(separators)
            .to_owned();
        if let Some((before, word)) = name.rsplit_once(|c: char| !c.is_alphanumeric())
            && PAGE_WORDS.contains(&word.to_lowercase().as_str())
        {
            name = before.trim_end_matches(separators).to_owned();
        }
    }
    let name = name.trim();
    if name.chars().count() < 2 || PAGE_WORDS.contains(&name.to_lowercase().as_str()) {
        "Pictures".into()
    } else {
        name.to_owned()
    }
}

/// Where a copy of an added file goes in `dir`: its own name, or `name (2).ext`… when another
/// file has it. `Ok(None)` when the same file (by name and size) is there already.
fn copy_target(dir: &Path, name: &str, size: u64) -> Option<Option<PathBuf>> {
    let safe = crate::export::names::safe_name(name);
    let (stem, extension) = safe
        .rsplit_once('.')
        .map_or((safe.as_str(), None), |(stem, extension)| {
            (stem, Some(extension))
        });
    for attempt in 1..1000 {
        let candidate = match (attempt, extension) {
            (1, _) => safe.clone(),
            (n, Some(extension)) => format!("{stem} ({n}).{extension}"),
            (n, None) => format!("{stem} ({n})"),
        };
        let path = dir.join(&candidate);
        match std::fs::metadata(&path) {
            Ok(meta) if size > 0 && meta.len() == size => return Some(None),
            Ok(_) => continue,
            Err(_) => return Some(Some(path)),
        }
    }
    None
}

/// A file found in a folder.
#[derive(Debug, Clone)]
struct Found {
    /// Its path inside the folder, with `/`.
    rel: String,
    size: i64,
    modified: i64,
    /// How to open it: a path, or (Android) a document address.
    open: Opener,
}

#[derive(Debug, Clone)]
enum Opener {
    Path(PathBuf),
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Document(String),
}

/// The file path of an Android folder from the picker, which the media permissions let Raygun
/// read by path: shared storage (`primary:Music/x` → `/storage/emulated/0/Music/x`, an SD card's
/// volume, `home:` = Documents) and the Downloads provider's own folders.
pub fn tree_path(location: &str) -> Option<String> {
    let (provider, rest) = location.strip_prefix("content://")?.split_once("/tree/")?;
    id_path(provider, &percent_decode(rest.split('/').next()?))
}

/// The file path of a single document another app shared, by the same rules as [`tree_path`]
/// (a playlist whose songs are named beside it).
pub fn document_path(location: &str) -> Option<String> {
    let (provider, rest) = location
        .strip_prefix("content://")?
        .split_once("/document/")?;
    id_path(provider, &percent_decode(rest.split('/').next()?))
}

/// A document id's file path in shared storage.
fn id_path(provider: &str, document: &str) -> Option<String> {
    const INTERNAL: &str = "/storage/emulated/0";
    match provider {
        "com.android.externalstorage.documents" => {
            let (volume, inside) = document.split_once(':')?;
            let base = match volume {
                "primary" => INTERNAL.to_owned(),
                "home" => format!("{INTERNAL}/Documents"),
                volume => format!("/storage/{volume}"),
            };
            Some(if inside.is_empty() {
                base
            } else {
                format!("{base}/{}", inside.trim_end_matches('/'))
            })
        }
        "com.android.providers.downloads.documents" => match document {
            "downloads" => Some(format!("{INTERNAL}/Download")),
            raw => raw
                .strip_prefix("raw:")
                .filter(|path| path.starts_with('/'))
                .map(|path| path.trim_end_matches('/').to_owned()),
        },
        _ => None,
    }
}

pub(crate) fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let hex = |byte: u8| (byte as char).to_digit(16);
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && let (Some(&high), Some(&low)) = (bytes.get(index + 1), bytes.get(index + 2))
            && let (Some(high), Some(low)) = (hex(high), hex(low))
        {
            out.push((high * 16 + low) as u8);
            index += 3;
        } else {
            out.push(bytes[index]);
            index += 1;
        }
    }
    String::from_utf8_lossy(&out).into_owned()
}

/// A video file's season and episode from its name: `S01E02`, `1x02`, `Episode 3`,
/// `[Group] Show - 03`, `Show II - 03`, `Show 2nd Season - 03`; see `video_match::parse_episode`.
/// (The scan groups files with `group_videos`, which also weighs numbers ending names.)
#[cfg(test)]
fn episode_of(name: &str) -> Option<(Option<f64>, f64)> {
    video_match::episode_of(name).map(|(season, episode)| (season.map(f64::from), episode))
}

/// A folder named only for a season ("Season 2", "S02", "Specials"): its videos are its parent
/// folder's show's.
fn season_folder(name: &str) -> Option<u32> {
    if matches!(name.trim().to_lowercase().as_str(), "specials" | "special") {
        return Some(0);
    }
    let info = video_match::parse_title(name);
    info.season.filter(|_| info.base.is_empty())
}

/// A folder's videos as titles: the episodes of one show are a series (season folders join
/// their show's: "Show/Season 2/…"; "Show - 01" and "Show II - 01" are its seasons 1 and 2), a
/// folder holding several shows gives each its own, and other files are movies of their own.
#[derive(Debug, Clone, PartialEq)]
struct VideoTitle {
    /// After the folder's id in the title's remote id.
    key: String,
    name: String,
    series: bool,
    /// Indices of its files in the list grouped, with their season, number and name.
    episodes: Vec<(usize, Option<u32>, Option<f64>, String)>,
}

/// Groups video files (paths inside the folder, in natural order) into titles.
fn group_videos(folder_name: &str, rels: &[&str]) -> Vec<VideoTitle> {
    struct File {
        index: usize,
        info: video_match::EpisodeInfo,
        season: Option<u32>,
    }
    // By the folder their show is in.
    let mut shows_dirs: BTreeMap<String, Vec<File>> = BTreeMap::new();
    for (index, rel) in rels.iter().enumerate() {
        let dir = parent(rel);
        let last = dir.rsplit('/').next().unwrap_or(dir);
        let (dir, folder_season) = match season_folder(last).filter(|_| !dir.is_empty()) {
            Some(season) => (parent(dir), Some(season)),
            None => (dir, None),
        };
        let info = video_match::parse_episode(&stem(rel));
        shows_dirs.entry(dir.to_owned()).or_default().push(File {
            index,
            season: info.season.or(folder_season),
            info,
        });
    }
    let mut titles = Vec::new();
    for (dir, files) in shows_dirs {
        // A number ending a name ("Show 03") counts once two names of the show have one.
        let numbered = |file: &File| {
            file.info.episode.is_some()
                && (!file.info.weak
                    || files
                        .iter()
                        .filter(|other| {
                            other.info.weak
                                && other.info.episode.is_some()
                                && other.info.show.key() == file.info.show.key()
                        })
                        .count()
                        >= 2)
        };
        // Names that are only a number ("01.mkv") are of the folder's show, whichever it is.
        let mut shows: Vec<String> = Vec::new();
        for file in files.iter().filter(|file| numbered(file)) {
            let key = file.info.show.key();
            if !key.is_empty() && !shows.contains(&key) {
                shows.push(key);
            }
        }
        let episode = |file: &File| {
            let number = numbered(file).then_some(file.info.episode).flatten();
            let name = match (number, file.info.rest.as_str()) {
                (Some(number), "") => format!("Episode {}", video_match::format_number(number)),
                (Some(_), rest) => rest.to_owned(),
                (None, _) => stem(rels[file.index]),
            };
            (file.index, file.season, number, name)
        };
        let movie = |file: &File| VideoTitle {
            key: rels[file.index].to_owned(),
            name: stem(rels[file.index]),
            series: false,
            episodes: vec![(file.index, None, None, stem(rels[file.index]))],
        };
        let named = files.iter().filter(|file| numbered(file)).count();
        if shows.len() <= 1 {
            // One show: the folder is its series when half its files or more are episodes.
            if files.len() > 1 && named * 2 >= files.len() {
                let name = if dir.is_empty() {
                    folder_name.to_owned()
                } else {
                    dir.rsplit('/').next().unwrap_or(&dir).to_owned()
                };
                titles.push(VideoTitle {
                    key: format!("{dir}/"),
                    name,
                    series: true,
                    episodes: files.iter().map(episode).collect(),
                });
            } else {
                titles.extend(files.iter().map(movie));
            }
            continue;
        }
        // Several shows in one folder: each its own series; files of none are movies.
        for show in &shows {
            let members: Vec<&File> = files
                .iter()
                .filter(|file| numbered(file) && file.info.show.key() == *show)
                .collect();
            if members.len() < 2 {
                titles.extend(members.into_iter().map(movie));
                continue;
            }
            let name = members
                .iter()
                .map(|file| file.info.show.name.clone())
                .find(|name| !name.is_empty())
                .unwrap_or_else(|| show.clone());
            titles.push(VideoTitle {
                key: format!("{dir}/{show}"),
                name,
                series: true,
                episodes: members.into_iter().map(episode).collect(),
            });
        }
        titles.extend(
            files
                .iter()
                .filter(|file| !numbered(file) || file.info.show.key().is_empty())
                .map(movie),
        );
    }
    // A series with seasons: its episodes without one are the first season's.
    for title in titles.iter_mut().filter(|title| title.series) {
        if title
            .episodes
            .iter()
            .any(|(_, season, ..)| season.is_some())
        {
            for (_, season, number, _) in &mut title.episodes {
                if season.is_none() && number.is_some() {
                    *season = Some(1);
                }
            }
        }
    }
    titles
}

fn stem(name: &str) -> String {
    let file = file_name(name);
    file.rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .replace(['_', '.'], " ")
        .trim()
        .to_owned()
}

fn file_name(rel: &str) -> &str {
    rel.rsplit('/').next().unwrap_or(rel)
}

fn parent(rel: &str) -> &str {
    rel.rsplit_once('/').map_or("", |(dir, _)| dir)
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

/// Lists a folder's files of the kinds Raygun reads, hidden folders left out. `None` when the
/// folder itself can't be read (moved, unplugged, no access).
fn walk(root: &Path) -> Option<Vec<Found>> {
    std::fs::read_dir(root).ok()?;
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            if found.len() >= MAX_FILES {
                return Some(found);
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.starts_with('.') {
                continue;
            }
            let rel = if prefix.is_empty() {
                name.clone()
            } else {
                format!("{prefix}/{name}")
            };
            let Ok(meta) = entry.metadata() else {
                continue;
            };
            if meta.is_dir() {
                pending.push((entry.path(), rel));
            } else if kind_of(&name).is_some() {
                let modified = meta
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0, |elapsed| elapsed.as_millis() as i64);
                found.push(Found {
                    rel,
                    size: meta.len() as i64,
                    modified,
                    open: Opener::Path(entry.path()),
                });
            }
        }
    }
    Some(found)
}

/// Whether a folder of pictures is a comic chapter rather than photos or an album's artwork.
fn is_comic_folder(dir: &str, pages: &[&Found], media_dirs: &HashSet<&str>) -> bool {
    if dir.is_empty() || pages.len() < 3 || pages.len() > MAX_COMIC_PAGES {
        return false;
    }
    let lower = dir.to_lowercase();
    if lower.split('/').any(|segment| segment == "dcim") {
        return false;
    }
    if PICTURE_FOLDERS.contains(&file_name(&lower)) {
        return false;
    }
    // Pictures beside songs or videos, or in a folder under them, are an album's scans.
    let above = parent(dir);
    if media_dirs.contains(dir) || (!above.is_empty() && media_dirs.contains(above)) {
        return false;
    }
    let photos = pages
        .iter()
        .filter(|page| {
            let name = file_name(&page.rel).to_lowercase();
            PHOTO_PREFIXES.iter().any(|prefix| name.starts_with(prefix))
        })
        .count();
    photos * 2 < pages.len()
}

/// What a scan does with a folder's files, however they were listed.
struct Survey<'a> {
    /// Songs, which the music library reads from the folder.
    songs: u32,
    /// Videos, grouped into titles.
    videos: Vec<&'a Found>,
    /// Comic and book files to import.
    files: Vec<(&'a Found, Kind)>,
    /// Folders of pictures that are comic chapters, by their path inside the folder.
    page_dirs: Vec<(&'a str, Vec<&'a Found>)>,
}

fn survey(found: &[Found]) -> Survey<'_> {
    let mut survey = Survey {
        songs: 0,
        videos: Vec::new(),
        files: Vec::new(),
        page_dirs: Vec::new(),
    };
    let mut media_dirs: HashSet<&str> = HashSet::new();
    let mut image_dirs: BTreeMap<&str, Vec<&Found>> = BTreeMap::new();
    for file in found {
        match kind_of(&file.rel) {
            Some(Kind::Song) => {
                survey.songs += 1;
                media_dirs.insert(parent(&file.rel));
            }
            Some(Kind::Video) => {
                survey.videos.push(file);
                media_dirs.insert(parent(&file.rel));
            }
            Some(kind @ (Kind::Comic | Kind::Book)) => survey.files.push((file, kind)),
            Some(Kind::Image) => image_dirs.entry(parent(&file.rel)).or_default().push(file),
            None => {}
        }
    }
    survey.page_dirs = image_dirs
        .into_iter()
        .filter(|(dir, pages)| is_comic_folder(dir, pages, &media_dirs))
        .collect();
    survey
}

/// Whether songs and videos listed through the picker open by path: Android lets Raygun read
/// them only with the media permissions.
fn readable_by_path(base: &str, found: &[Found]) -> bool {
    found
        .iter()
        .find(|file| matches!(kind_of(&file.rel), Some(Kind::Song | Kind::Video)))
        .is_none_or(|file| std::fs::File::open(Path::new(base).join(&file.rel)).is_ok())
}

/// Whether an archive's entries are mostly pictures: a comic, not a ZIP of something else.
fn holds_pages(file: impl std::io::Read + std::io::Seek) -> Option<bool> {
    let archive = zip::ZipArchive::new(file).ok()?;
    let names: Vec<String> = archive
        .file_names()
        .filter_map(Result::ok)
        .filter(|name| !name.ends_with('/'))
        .map(|name| name.into_owned())
        .collect();
    let pictures = names
        .iter()
        .filter(|name| kind_of(name) == Some(Kind::Image))
        .count();
    Some(pictures > 0 && pictures * 2 >= names.len())
}

/// What a scan found and did, for the folder's line.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
#[serde(default, rename_all = "camelCase")]
struct Counts {
    songs: u32,
    comics: u32,
    books: u32,
    videos: u32,
    failed: u32,
    unplayable: u32,
    needs_access: bool,
    missing: bool,
}

/// What Raygun may read of the storage a folder is on.
#[derive(Debug, Clone)]
struct Storage {
    /// Where the folder browser starts: shared storage's top (Android), the home folder (desktop).
    root: PathBuf,
    view: LocalStorageView,
    can_allow_all_files: bool,
}

/// Comic archives and books that weren't listed because Android hides them without access to
/// all files: as they were at the last scan, not gone.
fn hidden_archive(rel: &str, row: &FileRow) -> bool {
    !rel.ends_with('/') && matches!(row.kind.as_str(), "comic" | "book" | SKIPPED)
}

/// What became of a comic or book file (or a folder of pages) at an earlier scan.
enum Earlier {
    /// It's new or changed, or failed before and the scan was asked for.
    New,
    Imported,
    Failed,
    /// Not a comic after all (a ZIP of something else).
    Skipped,
}

fn earlier(known: Option<&FileRow>, size: i64, modified: i64, retry_failed: bool) -> Earlier {
    match known {
        Some(row) if row.size == size && row.modified == modified => match &row.item {
            Some(_) => Earlier::Imported,
            None if row.kind == SKIPPED => Earlier::Skipped,
            None if retry_failed => Earlier::New,
            None => Earlier::Failed,
        },
        _ => Earlier::New,
    }
}

pub struct LocalFilesService {
    app: AppHandle,
    scanning: Mutex<HashSet<String>>,
    /// Folders asked to be scanned while a scan of them ran: scanned once more after it (trying
    /// failed files again when one of the asks said so).
    again: Mutex<HashMap<String, bool>>,
    /// How far each running scan got.
    progress: Mutex<HashMap<String, LocalScanProgress>>,
    /// Folders being removed: their scans stop at the next file.
    cancelled: Mutex<HashSet<String>>,
    /// One scan or removal of a folder at a time.
    turns: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

impl LocalFilesService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            scanning: Mutex::new(HashSet::new()),
            again: Mutex::new(HashMap::new()),
            progress: Mutex::new(HashMap::new()),
            cancelled: Mutex::new(HashSet::new()),
            turns: Mutex::new(HashMap::new()),
        })
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    fn turn(&self, id: &str) -> Arc<tokio::sync::Mutex<()>> {
        self.turns
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .entry(id.to_owned())
            .or_default()
            .clone()
    }

    fn is_cancelled(&self, id: &str) -> bool {
        self.cancelled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .contains(id)
    }

    pub async fn folders(&self) -> Result<Vec<LocalFolder>, String> {
        let scanning = self
            .scanning
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        let progress = self
            .progress
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .clone();
        Ok(store::folders(self.db())
            .await
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|row| {
                let counts: Counts = serde_json::from_str(&row.counts).unwrap_or_default();
                LocalFolder {
                    scanning: scanning.contains(&row.id),
                    progress: progress.get(&row.id).copied(),
                    needs_access: counts.needs_access,
                    missing: counts.missing,
                    access: access_of(row.access),
                    id: row.id,
                    name: row.name,
                    location: row.path.unwrap_or(row.location),
                    counts: LocalCounts {
                        songs: counts.songs,
                        comics: counts.comics,
                        books: counts.books,
                        videos: counts.videos,
                        failed: counts.failed,
                        unplayable: counts.unplayable,
                    },
                    scanned_at: row.scanned_at.map(|at| at as f64),
                }
            })
            .collect())
    }

    /// Adds a folder the user picks in the system's picker (Android's folder picker, a desktop's
    /// folder dialog) and scans it. `None` when they cancelled.
    pub async fn add(self: &Arc<Self>) -> Result<Option<Vec<LocalFolder>>, String> {
        let picked = crate::commands::pick_local_folder(&self.app).await?;
        let Some(location) = picked.uri else {
            return Ok(None);
        };
        let (path, access) = if location.starts_with("content://") {
            // Songs and videos are read by path, which needs the media permissions; a scan
            // tells when they're missing.
            crate::commands::request_media_access(&self.app).await;
            (tree_path(&location), FolderAccess::Tree)
        } else {
            (Some(location.clone()), FolderAccess::Path)
        };
        let name = picked.name.unwrap_or_else(|| "Folder".into());
        let id = store::add(
            self.db(),
            NewFolder {
                id: &uuid::Uuid::now_v7().to_string(),
                location: &location,
                name: &name,
                path: path.as_deref(),
                access,
            },
            now_ms(),
        )
        .await
        .map_err(|error| error.to_string())?;
        self.scan_later(id, true);
        Ok(Some(self.folders().await?))
    }

    /// Adds a folder chosen in Raygun's own folder browser (M46.51) by its path, and scans it.
    pub async fn add_path(self: &Arc<Self>, path: &str) -> Result<Vec<LocalFolder>, String> {
        let storage = self.storage().await?;
        let folder = inside(&storage.root, Path::new(path))
            .ok_or_else(|| format!("{path} isn't a folder of this device's storage"))?;
        if !folder.is_dir() {
            return Err(format!("{path} is not a folder"));
        }
        let location = path_string(&folder);
        let name = if folder == storage.root {
            "Internal storage".to_owned()
        } else {
            folder.file_name().map_or_else(
                || location.clone(),
                |name| name.to_string_lossy().into_owned(),
            )
        };
        let id = store::add(
            self.db(),
            NewFolder {
                id: &uuid::Uuid::now_v7().to_string(),
                location: &location,
                name: &name,
                path: Some(&location),
                access: FolderAccess::Path,
            },
            now_ms(),
        )
        .await
        .map_err(|error| error.to_string())?;
        self.scan_later(id, true);
        self.folders().await
    }

    /// A folder of the device's storage for the folder browser (M46.51): its top when `path` is
    /// `None`. With access to all files it's read by path; with the media permissions only, as
    /// MediaStore lists it.
    pub async fn browse(&self, path: Option<&str>) -> Result<LocalBrowse, String> {
        let storage = self.storage().await?;
        let folder = match path.filter(|path| !path.is_empty()) {
            Some(path) => inside(&storage.root, Path::new(path))
                .ok_or_else(|| format!("{path} isn't a folder of this device's storage"))?,
            None => storage.root.clone(),
        };
        let (folders, items) = match storage.view {
            LocalStorageView::AllFiles => {
                let dir = folder.clone();
                tauri::async_runtime::spawn_blocking(move || {
                    list_folders(&dir, COUNT_ENTRIES, &Budget::new(COUNT_TIME))
                })
                .await
                .map_err(|error| error.to_string())?
                .ok_or_else(|| format!("{} can't be opened", folder.display()))?
            }
            LocalStorageView::Media => self.media_folders(&folder).await?,
            LocalStorageView::None => (Vec::new(), 0),
        };
        let location = path_string(&folder);
        let added = store::folders(self.db())
            .await
            .map_err(|error| error.to_string())?
            .iter()
            .any(|row| row.path.as_deref() == Some(location.as_str()));
        Ok(LocalBrowse {
            name: folder
                .file_name()
                .filter(|_| folder != storage.root)
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_default(),
            trail: crumbs(&storage.root, &folder),
            path: location,
            folders,
            items,
            view: storage.view,
            can_allow_all_files: storage.can_allow_all_files,
            added,
        })
    }

    /// A folder's folders and files as MediaStore lists them (Android, media permissions only).
    #[cfg(target_os = "android")]
    async fn media_folders(&self, folder: &Path) -> Result<(Vec<LocalBrowseFolder>, u32), String> {
        let app = self.app.clone();
        let path = path_string(folder);
        let listing = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .browse_media(&path, &extensions())
        })
        .await
        .map_err(|error| error.to_string())??;
        let mut folders: Vec<LocalBrowseFolder> = listing
            .folders
            .into_iter()
            .filter(|found| found.items > 0)
            .map(|found| LocalBrowseFolder {
                path: path_string(&folder.join(&found.name)),
                name: found.name,
                items: Some(found.items),
            })
            .collect();
        folders.sort_by(|a, b| by_name(&a.name, &b.name));
        Ok((folders, listing.items))
    }

    #[cfg(not(target_os = "android"))]
    async fn media_folders(&self, _folder: &Path) -> Result<(Vec<LocalBrowseFolder>, u32), String> {
        Err("MediaStore is Android's".into())
    }

    /// What Raygun may read of the device's storage.
    #[cfg(target_os = "android")]
    async fn storage(&self) -> Result<Storage, String> {
        let app = self.app.clone();
        let access = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .storage_access()
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(Storage {
            root: PathBuf::from(access.root),
            view: if access.all_files {
                LocalStorageView::AllFiles
            } else if access.media {
                LocalStorageView::Media
            } else {
                LocalStorageView::None
            },
            can_allow_all_files: access.can_ask_all_files && !access.all_files,
        })
    }

    /// Desktop: every file reads by path, and the browser starts in the home folder.
    #[cfg(not(target_os = "android"))]
    async fn storage(&self) -> Result<Storage, String> {
        let root = raygun_paths::user_home().ok_or("This device has no home folder")?;
        Ok(Storage {
            root,
            view: LocalStorageView::AllFiles,
            can_allow_all_files: false,
        })
    }

    /// Raygun's own folder for songs and videos added one by one.
    fn added_dir(&self) -> PathBuf {
        self.app
            .state::<crate::startup::AppPaths>()
            .0
            .downloads()
            .join(ADDED_DIR)
    }

    /// The Added files folder, made the first time: its id and where it is.
    async fn added_folder(&self) -> Result<(String, PathBuf), String> {
        let dir = self.added_dir();
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        let location = path_string(&dir);
        let id = store::add(
            self.db(),
            NewFolder {
                id: &uuid::Uuid::now_v7().to_string(),
                location: &location,
                name: ADDED_NAME,
                path: Some(&location),
                access: FolderAccess::Added,
            },
            now_ms(),
        )
        .await
        .map_err(|error| error.to_string())?;
        Ok((id, dir))
    }

    /// Files the user picks in the system's file picker. Empty when they cancelled.
    #[cfg(target_os = "android")]
    async fn pick_files(&self) -> Result<Vec<Found>, String> {
        let app = self.app.clone();
        let picked = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .pick_documents(PICK_TYPES)
        })
        .await
        .map_err(|error| error.to_string())??;
        Ok(picked
            .into_iter()
            .map(|file| Found {
                rel: named_for_kind(&file.name, file.mime.as_deref()),
                size: file.size,
                modified: 0,
                open: Opener::Document(file.uri),
            })
            .collect())
    }

    #[cfg(not(target_os = "android"))]
    async fn pick_files(&self) -> Result<Vec<Found>, String> {
        let _ = PICK_TYPES;
        let picked = rfd::AsyncFileDialog::new()
            .set_title("Add files to Local Files")
            .add_filter("Songs, videos, comics, books and pictures", &extensions())
            .pick_files()
            .await
            .unwrap_or_default();
        Ok(picked
            .into_iter()
            .map(|file| {
                let path = file.path().to_path_buf();
                let meta = std::fs::metadata(&path).ok();
                Found {
                    rel: file.file_name(),
                    size: meta.as_ref().map_or(0, |meta| meta.len() as i64),
                    modified: meta
                        .and_then(|meta| meta.modified().ok())
                        .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                        .map_or(0, |elapsed| elapsed.as_millis() as i64),
                    open: Opener::Path(path),
                }
            })
            .collect())
    }

    /// Add files (M46.51): files picked one by one, from anywhere. Comics and books are imported
    /// (pictures picked together become one comic), and songs and videos copied into the Added
    /// files folder, which is scanned like any other. `None` when the user cancelled.
    pub async fn add_files(self: &Arc<Self>) -> Result<Option<LocalFilesAdded>, String> {
        let picked = self.pick_files().await?;
        if picked.is_empty() {
            return Ok(None);
        }
        let manga = self.app.state::<crate::manga::MangaService>();
        let novels = self.app.state::<crate::novel::NovelService>();
        let mut added = LocalFilesAdded {
            songs: 0,
            videos: 0,
            comics: 0,
            books: 0,
            failed: Vec::new(),
            folders: Vec::new(),
        };
        let mut failed = |name: &str, error: String| {
            tracing::info!(%error, file = %name, "couldn't add a picked file");
            added.failed.push(LocalAddFailure {
                name: name.to_owned(),
                error,
            });
        };
        let mut pictures = Vec::new();
        let mut media = Vec::new();
        let (mut comics, mut books) = (0, 0);
        for file in &picked {
            match kind_of(&file.rel) {
                Some(Kind::Comic) => match self.import_comic(&manga, file).await {
                    Ok(Some(_)) => comics += 1,
                    Ok(None) => failed(&file.rel, "This archive holds no pictures".into()),
                    Err(error) => failed(&file.rel, error),
                },
                Some(Kind::Book) => match self.import_book(&novels, file).await {
                    Ok(_) => books += 1,
                    Err(error) => failed(&file.rel, error),
                },
                Some(Kind::Image) => pictures.push(file),
                Some(Kind::Song | Kind::Video) => media.push(file),
                None => failed(
                    &file.rel,
                    "Raygun doesn't play or read this kind of file".into(),
                ),
            }
        }
        if !pictures.is_empty() {
            pictures.sort_by(|a, b| crate::comics::natural_cmp(&a.rel, &b.rel));
            let names: Vec<&str> = pictures.iter().map(|file| file.rel.as_str()).collect();
            let name = format!("{}.cbz", pictures_name(&names));
            match self.import_pages(&manga, &name, pictures).await {
                Ok(_) => comics += 1,
                Err(error) => failed(&name, error),
            }
        }
        let (mut songs, mut videos) = (0, 0);
        let folder = if media.is_empty() {
            None
        } else {
            Some(self.added_folder().await)
        };
        match folder {
            None => {}
            Some(Ok((id, dir))) => {
                for file in media {
                    let service = self.clone();
                    let (copy, target) = (file.clone(), dir.clone());
                    let copied = tauri::async_runtime::spawn_blocking(move || {
                        service.copy_in(&copy, &target)
                    })
                    .await
                    .map_err(|error| error.to_string())
                    .and_then(|result| result);
                    match copied {
                        Ok(()) if kind_of(&file.rel) == Some(Kind::Song) => songs += 1,
                        Ok(()) => videos += 1,
                        Err(error) => failed(&file.rel, error),
                    }
                }
                self.scan_later(id, false);
            }
            Some(Err(error)) => {
                for file in media {
                    failed(&file.rel, error.clone());
                }
            }
        }
        added.comics = comics;
        added.books = books;
        added.songs = songs;
        added.videos = videos;
        added.folders = self.folders().await?;
        Ok(Some(added))
    }

    /// Copies a picked song or video into the Added files folder (a file already there stays).
    fn copy_in(&self, file: &Found, dir: &Path) -> Result<(), String> {
        let Some(target) = copy_target(dir, &file.rel, file.size.max(0) as u64)
            .ok_or_else(|| format!("{} has too many copies already", file.rel))?
        else {
            return Ok(());
        };
        // Written aside first, so a scan never finds half a file.
        let partial = target.with_file_name(format!(
            "{}.part",
            target
                .file_name()
                .map_or_else(String::new, |name| name.to_string_lossy().into_owned())
        ));
        let result = (|| {
            let mut input = self.open(file)?;
            let mut output = std::fs::File::create(&partial).map_err(|error| error.to_string())?;
            std::io::copy(&mut input, &mut output).map_err(|error| error.to_string())?;
            std::fs::rename(&partial, &target).map_err(|error| error.to_string())
        })();
        if result.is_err() {
            let _ = std::fs::remove_file(&partial);
        }
        result
    }

    /// Media access or access to all files changed (M46.51): folders read by path, and those
    /// that waited for access, are scanned again for what shows now.
    pub async fn access_changed(self: &Arc<Self>) -> Result<(), String> {
        for folder in store::folders(self.db())
            .await
            .map_err(|error| error.to_string())?
        {
            let counts: Counts = serde_json::from_str(&folder.counts).unwrap_or_default();
            if folder.access == FolderAccess::Path || counts.needs_access {
                self.scan_later(folder.id, false);
            }
        }
        Ok(())
    }

    /// Forgets a folder: its songs leave the music library and its videos the video library.
    /// Imported comics and books stay (they were copied in). A scan of it stops first.
    pub async fn remove(&self, id: &str) -> Result<Vec<LocalFolder>, String> {
        self.cancelled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.to_owned());
        self.again
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        let turn = self.turn(id);
        let result = {
            let _turn = turn.lock().await;
            self.forget(id).await
        };
        self.cancelled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        self.turns
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .remove(id);
        result?;
        self.changed(id);
        self.folders().await
    }

    async fn forget(&self, id: &str) -> Result<(), String> {
        let db = self.db();
        if let Some(folder) = store::folder(db, id)
            .await
            .map_err(|error| error.to_string())?
        {
            if let Some(path) = &folder.path {
                self.app
                    .state::<crate::library::LibraryService>()
                    .remove_root(path)
                    .await?;
            }
            for (_, file) in store::files(db, id)
                .await
                .map_err(|error| error.to_string())?
            {
                if file.kind == "video"
                    && let Some(title) = file
                        .item
                        .as_deref()
                        .and_then(|item| item.strip_prefix(&format!("{LOCAL_VIDEO}:")))
                    && let Ok(Some(row)) = titles::find_title(db, LOCAL_VIDEO, title).await
                {
                    let _ = titles::set_in_library(db, &row.id, None).await;
                }
            }
            self.release_grant(&folder.location).await;
            // Added files are Raygun's own copies: they go with their folder.
            let added = self.added_dir();
            if folder.access == FolderAccess::Added
                && Path::new(&folder.location).starts_with(&added)
            {
                let _ =
                    tauri::async_runtime::spawn_blocking(move || std::fs::remove_dir_all(added))
                        .await;
            }
        }
        store::remove(db, id)
            .await
            .map_err(|error| error.to_string())
    }

    /// Folders a backup listed (M46.51), added again where this device still has them: a path
    /// that's a folder (Added files while Raygun's own folder is there), a picked tree while its
    /// grant is held. Each is scanned again. Returns how many came back and how many aren't here.
    pub async fn restore(
        self: &Arc<Self>,
        folders: Vec<store::FolderRow>,
    ) -> Result<(u32, u32), String> {
        let have: HashSet<String> = store::folders(self.db())
            .await
            .map_err(|error| error.to_string())?
            .into_iter()
            .map(|folder| folder.location)
            .collect();
        let (mut added, mut missing) = (0, 0);
        for folder in new_folders(folders, &have) {
            if !self.still_here(&folder).await {
                missing += 1;
                continue;
            }
            let id = store::add(
                self.db(),
                NewFolder {
                    id: &uuid::Uuid::now_v7().to_string(),
                    location: &folder.location,
                    name: &folder.name,
                    path: folder.path.as_deref(),
                    access: folder.access,
                },
                folder.added_at,
            )
            .await
            .map_err(|error| error.to_string())?;
            self.scan_later(id, true);
            added += 1;
        }
        Ok((added, missing))
    }

    async fn still_here(&self, folder: &store::FolderRow) -> bool {
        if folder.access == FolderAccess::Tree {
            return self.tree_granted(&folder.location).await;
        }
        let folder = folder.clone();
        tauri::async_runtime::spawn_blocking(move || path_is_here(&folder))
            .await
            .unwrap_or(false)
    }

    /// Whether Raygun still holds the grant of a tree picked in Android's folder picker.
    #[cfg(target_os = "android")]
    async fn tree_granted(&self, location: &str) -> bool {
        let (app, location) = (self.app.clone(), location.to_owned());
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .folder_state(&location)
                .is_ok_and(|state| state.granted)
        })
        .await
        .unwrap_or(false)
    }

    #[cfg(not(target_os = "android"))]
    async fn tree_granted(&self, _location: &str) -> bool {
        false
    }

    /// Gives up the picker's grant of a removed folder (Android), unless downloads are saved there.
    async fn release_grant(&self, location: &str) {
        #[cfg(target_os = "android")]
        if location.starts_with("content://") {
            let save_folder = crate::settings::load(self.db())
                .await
                .ok()
                .and_then(|settings| settings.save.folder_uri);
            if save_folder.as_deref() != Some(location) {
                let app = self.app.clone();
                let location = location.to_owned();
                let _ = tauri::async_runtime::spawn_blocking(move || {
                    app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                        .release_folder(&location)
                })
                .await;
            }
        }
        #[cfg(not(target_os = "android"))]
        let _ = location;
    }

    /// Scans folders again in the background: one, or every one. Files that couldn't be added
    /// before are tried again when `retry_failed` (a scan the user asked for).
    pub async fn rescan(
        self: &Arc<Self>,
        id: Option<&str>,
        retry_failed: bool,
    ) -> Result<(), String> {
        for folder in store::folders(self.db())
            .await
            .map_err(|error| error.to_string())?
        {
            if id.is_none_or(|id| id == folder.id) {
                self.scan_later(folder.id, retry_failed);
            }
        }
        Ok(())
    }

    /// Asks for the media access songs and videos need (Android). When the system won't ask any
    /// more, Raygun's page in the system settings opens. Folders that needed it are scanned again.
    pub async fn allow_access(self: &Arc<Self>) -> Result<bool, String> {
        let granted = crate::commands::request_media_access(&self.app).await;
        if granted {
            for folder in store::folders(self.db())
                .await
                .map_err(|error| error.to_string())?
            {
                let counts: Counts = serde_json::from_str(&folder.counts).unwrap_or_default();
                if counts.needs_access {
                    self.scan_later(folder.id, true);
                }
            }
        } else {
            self.open_app_settings().await;
        }
        Ok(granted)
    }

    async fn open_app_settings(&self) {
        #[cfg(target_os = "android")]
        {
            let app = self.app.clone();
            let _ = tauri::async_runtime::spawn_blocking(move || {
                app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                    .open_app_settings()
            })
            .await;
        }
    }

    pub fn scan_later(self: &Arc<Self>, id: String, retry_failed: bool) {
        {
            let mut scanning = self.scanning.lock().unwrap_or_else(|p| p.into_inner());
            if !scanning.insert(id.clone()) {
                // A scan of it is running: once more after it, for what changed meanwhile
                // (files added while it listed).
                *self
                    .again
                    .lock()
                    .unwrap_or_else(|p| p.into_inner())
                    .entry(id)
                    .or_default() |= retry_failed;
                return;
            }
        }
        self.changed(&id);
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let turn = service.turn(&id);
            let mut retry_failed = retry_failed;
            loop {
                {
                    let _turn = turn.lock().await;
                    if let Err(error) = service.scan(&id, retry_failed).await {
                        tracing::warn!(%error, "local folder scan failed");
                    }
                }
                // Decided under the scanning lock, so an ask made now is never lost.
                let again = {
                    let mut scanning = service.scanning.lock().unwrap_or_else(|p| p.into_inner());
                    let again = service
                        .again
                        .lock()
                        .unwrap_or_else(|p| p.into_inner())
                        .remove(&id);
                    if again.is_none() {
                        scanning.remove(&id);
                    }
                    again
                };
                match again {
                    Some(retry) => retry_failed = retry,
                    None => break,
                }
            }
            service
                .progress
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&id);
            service.changed(&id);
        });
    }

    fn changed(&self, id: &str) {
        if let Err(error) = LocalFilesEvent(id.to_owned()).emit(&self.app) {
            tracing::debug!(%error, "couldn't announce local files");
        }
    }

    /// The folder's files, or an error when it can't be opened: through the picker's grant for a
    /// tree, else by path.
    async fn list(&self, folder: &store::FolderRow) -> Result<Vec<Found>, String> {
        #[cfg(target_os = "android")]
        if folder.access == FolderAccess::Tree {
            let app = self.app.clone();
            let location = folder.location.clone();
            let base = folder.path.clone();
            return tauri::async_runtime::spawn_blocking(move || {
                let files = app
                    .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                    .list_tree(&location)?;
                Ok(files
                    .into_iter()
                    .filter(|file| kind_of(&file.path).is_some())
                    .map(|file| Found {
                        // Songs and videos open by path (the media permissions allow it);
                        // the rest, pictures too, through the picker's grant.
                        open: match (&base, kind_of(&file.path)) {
                            (Some(base), Some(Kind::Song | Kind::Video)) => {
                                Opener::Path(Path::new(base).join(&file.path))
                            }
                            _ => Opener::Document(file.uri),
                        },
                        rel: file.path,
                        size: file.size,
                        modified: file.modified,
                    })
                    .collect())
            })
            .await
            .map_err(|error| error.to_string())?;
        }
        let root = PathBuf::from(
            folder
                .path
                .clone()
                .unwrap_or_else(|| folder.location.clone()),
        );
        tauri::async_runtime::spawn_blocking(move || {
            walk(&root).ok_or_else(|| format!("{} can't be opened", root.display()))
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// A file opened for reading, by path or (Android) through the picker's grant.
    fn open(&self, file: &Found) -> Result<std::fs::File, String> {
        match &file.open {
            Opener::Path(path) => std::fs::File::open(path).map_err(|error| error.to_string()),
            Opener::Document(_uri) => {
                #[cfg(target_os = "android")]
                {
                    use std::os::fd::FromRawFd;
                    let fd = self
                        .app
                        .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                        .open_input(_uri)?;
                    // The plugin detached the descriptor for Rust, which now owns it.
                    Ok(unsafe { std::fs::File::from_raw_fd(fd) })
                }
                #[cfg(not(target_os = "android"))]
                Err("documents open only on Android".into())
            }
        }
    }

    fn read(&self, file: &Found) -> Result<Vec<u8>, String> {
        let mut bytes = Vec::new();
        std::io::Read::read_to_end(&mut self.open(file)?, &mut bytes)
            .map_err(|error| error.to_string())?;
        Ok(bytes)
    }

    /// Copies a file to `target` a piece at a time (a comic can be hundreds of megabytes).
    fn copy_to(&self, file: &Found, target: &Path) -> Result<(), String> {
        if file.size as u64 > MAX_IMPORT_BYTES {
            return Err(format!("{} is too large to import", file.rel));
        }
        let mut input = self.open(file)?;
        let mut output = std::fs::File::create(target).map_err(|error| error.to_string())?;
        std::io::copy(&mut input, &mut output).map_err(|error| error.to_string())?;
        Ok(())
    }

    async fn scan(self: &Arc<Self>, id: &str, retry_failed: bool) -> Result<(), String> {
        let db = self.db();
        let folder = store::folder(db, id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or("This folder was removed")?;
        let known = store::files(db, id)
            .await
            .map_err(|error| error.to_string())?;
        // A folder read by path on Android shows what the permissions let Raygun see: nothing
        // without media access (its files aren't gone, so everything stays as it was), and none
        // of other apps' books and archives without access to all files.
        let mut hidden_archives = false;
        if cfg!(target_os = "android") && folder.access == FolderAccess::Path {
            match self.storage().await.map(|storage| storage.view) {
                Ok(LocalStorageView::None) => {
                    let mut counts: Counts =
                        serde_json::from_str(&folder.counts).unwrap_or_default();
                    counts.needs_access = true;
                    let json = serde_json::to_string(&counts).unwrap_or_else(|_| "{}".into());
                    return store::set_counts(db, id, &json)
                        .await
                        .map_err(|error| error.to_string());
                }
                Ok(LocalStorageView::Media) => hidden_archives = true,
                Ok(LocalStorageView::AllFiles) | Err(_) => {}
            }
        }
        let found = match self.list(&folder).await {
            Ok(found) => found,
            Err(error) => {
                // Nothing is taken away while the folder is gone; the last counts stay.
                tracing::info!(%error, folder = %folder.name, "a local folder couldn't be opened");
                let mut counts: Counts = serde_json::from_str(&folder.counts).unwrap_or_default();
                counts.missing = true;
                let json = serde_json::to_string(&counts).unwrap_or_else(|_| "{}".into());
                return store::set_counts(db, id, &json)
                    .await
                    .map_err(|error| error.to_string());
            }
        };
        if self.is_cancelled(id) {
            return Ok(());
        }
        let mut counts = Counts::default();
        let mut seen = HashSet::new();
        let Survey {
            songs,
            videos,
            files,
            page_dirs,
        } = survey(&found);

        // Songs: the folder is one of the music library's roots, which scans them. Without a
        // path (a cloud folder), songs and videos can't play from it.
        match &folder.path {
            Some(path) => {
                counts.songs = songs;
                counts.needs_access =
                    folder.access == FolderAccess::Tree && !readable_by_path(path, &found);
                if songs > 0 {
                    raygun_db::local_music::add_root(db, path)
                        .await
                        .map_err(|error| error.to_string())?;
                    self.app
                        .state::<crate::library::LibraryService>()
                        .start_scan();
                }
            }
            None => {
                counts.unplayable = songs + videos.len() as u32;
            }
        }

        // What to import: comic and book files, and folders of pages.
        let total = files
            .iter()
            .filter(|(file, _)| {
                matches!(
                    earlier(known.get(&file.rel), file.size, file.modified, retry_failed),
                    Earlier::New
                )
            })
            .count()
            + page_dirs
                .iter()
                .filter(|(dir, pages)| {
                    let (size, modified) = pages_stamp(pages);
                    matches!(
                        earlier(known.get(&format!("{dir}/")), size, modified, retry_failed),
                        Earlier::New
                    )
                })
                .count();
        let mut progress = ScanProgress::new(self, id, total as u32);

        // Comics and books, imported once per version of the file.
        let manga = self.app.state::<crate::manga::MangaService>();
        let novels = self.app.state::<crate::novel::NovelService>();
        for (file, kind) in files {
            seen.insert(file.rel.clone());
            let label = if kind == Kind::Comic { "comic" } else { "book" };
            match earlier(known.get(&file.rel), file.size, file.modified, retry_failed) {
                Earlier::Imported => {
                    counts.add(kind);
                    continue;
                }
                Earlier::Failed => {
                    counts.failed += 1;
                    continue;
                }
                Earlier::Skipped => continue,
                Earlier::New => {}
            }
            if self.is_cancelled(id) {
                return Ok(());
            }
            let outcome = match kind {
                Kind::Comic => self.import_comic(&manga, file).await,
                _ => self.import_book(&novels, file).await,
            };
            let item = match outcome {
                Ok(Some(item)) => {
                    counts.add(kind);
                    Some(item)
                }
                Ok(None) => {
                    let row = FileRow {
                        size: file.size,
                        modified: file.modified,
                        kind: SKIPPED.into(),
                        item: None,
                    };
                    let _ = store::put_file(db, id, &file.rel, &row).await;
                    progress.step();
                    continue;
                }
                Err(error) => {
                    tracing::info!(%error, file = %file.rel, "couldn't add a local file");
                    counts.failed += 1;
                    None
                }
            };
            let row = FileRow {
                size: file.size,
                modified: file.modified,
                kind: label.into(),
                item,
            };
            let _ = store::put_file(db, id, &file.rel, &row).await;
            progress.step();
        }

        // Folders of images are comics too: one chapter each, named after the folder.
        for (dir, mut pages) in page_dirs {
            let rel = format!("{dir}/");
            seen.insert(rel.clone());
            let (size, modified) = pages_stamp(&pages);
            match earlier(known.get(&rel), size, modified, retry_failed) {
                Earlier::Imported => {
                    counts.comics += 1;
                    continue;
                }
                Earlier::Failed => {
                    counts.failed += 1;
                    continue;
                }
                Earlier::Skipped => continue,
                Earlier::New => {}
            }
            if self.is_cancelled(id) {
                return Ok(());
            }
            pages.sort_by(|a, b| crate::comics::natural_cmp(&a.rel, &b.rel));
            let name = format!("{}.cbz", file_name(dir));
            let item = match self.import_pages(&manga, &name, pages).await {
                Ok(item) => {
                    counts.comics += 1;
                    Some(item)
                }
                Err(error) => {
                    tracing::info!(%error, folder = %dir, "couldn't add a folder of pages");
                    counts.failed += 1;
                    None
                }
            };
            let row = FileRow {
                size,
                modified,
                kind: "comic".into(),
                item,
            };
            let _ = store::put_file(db, id, &rel, &row).await;
            progress.step();
        }
        if self.is_cancelled(id) {
            return Ok(());
        }

        // Videos: a folder of episodes is a series; a lone file a movie.
        if folder.path.is_some() {
            counts.videos = self
                .store_videos(&folder, videos, &known, &mut seen)
                .await?;
        }

        // Files that are gone: their rows go. A video title no file belongs to any more (its
        // files gone, or grouped anew: a second episode makes a movie a series) leaves the
        // library.
        let mut gone_titles = HashSet::new();
        for (rel, row) in &known {
            if row.kind == "video"
                && let Some(item) = &row.item
            {
                gone_titles.insert(item.clone());
            }
            if seen.contains(rel) {
                continue;
            }
            if hidden_archives && hidden_archive(rel, row) {
                match (&row.item, row.kind.as_str()) {
                    (Some(_), "comic") => counts.comics += 1,
                    (Some(_), _) => counts.books += 1,
                    (None, SKIPPED) => {}
                    (None, _) => counts.failed += 1,
                }
                continue;
            }
            let _ = store::delete_file(db, id, rel).await;
        }
        let still: HashSet<String> = store::files(db, id)
            .await
            .map_err(|error| error.to_string())?
            .into_values()
            .filter_map(|row| row.item)
            .collect();
        for item in gone_titles.difference(&still) {
            if let Some(remote) = item.strip_prefix(&format!("{LOCAL_VIDEO}:"))
                && let Ok(Some(row)) = titles::find_title(db, LOCAL_VIDEO, remote).await
            {
                let _ = titles::set_in_library(db, &row.id, None).await;
            }
        }

        let counts_json = serde_json::to_string(&counts).unwrap_or_else(|_| "{}".into());
        store::set_scanned(db, id, &counts_json, now_ms())
            .await
            .map_err(|error| error.to_string())?;
        tracing::info!(
            folder = %folder.name,
            songs = counts.songs,
            comics = counts.comics,
            books = counts.books,
            videos = counts.videos,
            failed = counts.failed,
            unplayable = counts.unplayable,
            needs_access = counts.needs_access,
            "local folder scanned"
        );
        Ok(())
    }

    /// Imports a comic file; `None` when it's a ZIP of something other than pages.
    async fn import_comic(
        self: &Arc<Self>,
        manga: &crate::manga::MangaService,
        file: &Found,
    ) -> Result<Option<String>, String> {
        let name = file_name(&file.rel).to_owned();
        let temp = self.temp_file(&name)?;
        let service = self.clone();
        let (copy, target) = (file.clone(), temp.clone());
        let copied = tauri::async_runtime::spawn_blocking(move || {
            // A `.zip` is looked into first: only an archive of pages is a comic.
            if copy.rel.to_ascii_lowercase().ends_with(".zip")
                && let Ok(opened) = service.open(&copy)
                && holds_pages(opened) == Some(false)
            {
                return Ok(false);
            }
            service.copy_to(&copy, &target).map(|()| true)
        })
        .await
        .map_err(|error| error.to_string())
        .and_then(|result| result);
        match copied {
            Ok(true) => manga
                .import_comic(temp, &name)
                .await
                .map(|(source, key)| Some(format!("{source}:{key}"))),
            Ok(false) => Ok(None),
            Err(error) => {
                let _ = std::fs::remove_file(&temp);
                Err(error)
            }
        }
    }

    async fn import_book(
        self: &Arc<Self>,
        novels: &crate::novel::NovelService,
        file: &Found,
    ) -> Result<Option<String>, String> {
        if file.size as usize > crate::epub::MAX_FILE_BYTES {
            return Err(format!("{} is larger than 64 MB", file.rel));
        }
        let service = self.clone();
        let copy = file.clone();
        let bytes = tauri::async_runtime::spawn_blocking(move || service.read(&copy))
            .await
            .map_err(|error| error.to_string())??;
        novels
            .import_epub_bytes(bytes)
            .await
            .map(|(source, key)| Some(format!("{source}:{key}")))
    }

    async fn import_pages(
        self: &Arc<Self>,
        manga: &crate::manga::MangaService,
        name: &str,
        pages: Vec<&Found>,
    ) -> Result<String, String> {
        let temp = self.temp_file(name)?;
        let service = self.clone();
        let owned: Vec<Found> = pages.into_iter().cloned().collect();
        let target = temp.clone();
        let zipped =
            tauri::async_runtime::spawn_blocking(move || service.zip_pages(&owned, &target))
                .await
                .map_err(|error| error.to_string())
                .and_then(|result| result);
        if let Err(error) = zipped {
            let _ = std::fs::remove_file(&temp);
            return Err(error);
        }
        manga
            .import_comic(temp, name)
            .await
            .map(|(source, key)| format!("{source}:{key}"))
    }

    fn temp_file(&self, name: &str) -> Result<PathBuf, String> {
        let dir = std::env::temp_dir().join("raygun-local-files");
        std::fs::create_dir_all(&dir).map_err(|error| error.to_string())?;
        Ok(dir.join(format!(
            "{}-{}",
            uuid::Uuid::new_v4().simple(),
            crate::export::names::safe_name(name)
        )))
    }

    /// A folder's pictures as one archive, in order, for the comic importer.
    fn zip_pages(&self, pages: &[Found], target: &Path) -> Result<(), String> {
        let file = std::fs::File::create(target).map_err(|error| error.to_string())?;
        let mut zip = zip::ZipWriter::new(file);
        let options = zip::write::SimpleFileOptions::default()
            .compression_method(zip::CompressionMethod::Stored);
        for (index, page) in pages.iter().enumerate() {
            let bytes = self.read(page)?;
            let extension = page.rel.rsplit_once('.').map_or("jpg", |(_, ext)| ext);
            zip.start_file(format!("{:04}.{extension}", index + 1), options)
                .map_err(|error| error.to_string())?;
            std::io::Write::write_all(&mut zip, &bytes).map_err(|error| error.to_string())?;
        }
        zip.finish().map_err(|error| error.to_string())?;
        Ok(())
    }

    /// Stores the folder's videos as `local.video` titles in the library; returns how many files.
    /// A folder whose videos are as they were at the last scan is left as it is, so a title taken
    /// out of the library stays out.
    async fn store_videos(
        &self,
        folder: &store::FolderRow,
        mut files: Vec<&Found>,
        known: &HashMap<String, FileRow>,
        seen: &mut HashSet<String>,
    ) -> Result<u32, String> {
        let db = self.db();
        let now = now_ms();
        let mut count = 0;
        // Videos as they were at the last scan are left as they are, so a title taken out of
        // the library stays out.
        let known_videos = known.values().filter(|row| row.kind == "video").count();
        let unchanged = known_videos == files.len()
            && files.iter().all(|file| {
                known.get(&file.rel).is_some_and(|row| {
                    row.size == file.size && row.modified == file.modified && row.item.is_some()
                })
            });
        if unchanged {
            seen.extend(files.iter().map(|file| file.rel.clone()));
            return Ok(files.len() as u32);
        }
        files.sort_by(|a, b| crate::comics::natural_cmp(&a.rel, &b.rel));
        let rels: Vec<&str> = files.iter().map(|file| file.rel.as_str()).collect();
        for group in group_videos(&folder.name, &rels) {
            let remote_id = format!("{}:{}", folder.id, group.key);
            let name = group.name;
            let members: Vec<(&Found, Option<u32>, Option<f64>, String)> = group
                .episodes
                .into_iter()
                .map(|(index, season, number, name)| (files[index], season, number, name))
                .collect();
            {
                let single = !group.series;
                let title_id = titles::upsert_title(
                    db,
                    &TitleFields {
                        kind: "video".into(),
                        source_id: LOCAL_VIDEO.into(),
                        remote_id: remote_id.clone(),
                        title: name,
                        cover: None,
                        cover_headers: Default::default(),
                        authors: Vec::new(),
                        artists: Vec::new(),
                        description: None,
                        genres: Vec::new(),
                        status: "unknown".into(),
                        url: None,
                        media_type: Some(if single { "movie" } else { "series" }.into()),
                        year: None,
                    },
                    now,
                )
                .await
                .map_err(|error| error.to_string())?;
                let units: Vec<UnitFields> = members
                    .iter()
                    .map(|(file, season, number, name)| UnitFields {
                        remote_id: format!("{}/{}", folder.id, file.rel),
                        name: name.clone(),
                        number: *number,
                        volume: season.map(|season| season.to_string()),
                        scanlator: None,
                        date: None,
                    })
                    .collect();
                titles::sync_units(db, &title_id, &units, "episode", now)
                    .await
                    .map_err(|error| error.to_string())?;
                if let Ok(Some(row)) = titles::title(db, &title_id).await
                    && row.in_library_at.is_none()
                {
                    let _ = titles::set_in_library(db, &title_id, Some(now)).await;
                }
                for (file, ..) in members {
                    count += 1;
                    seen.insert(file.rel.clone());
                    let row = FileRow {
                        size: file.size,
                        modified: file.modified,
                        kind: "video".into(),
                        item: Some(format!("{LOCAL_VIDEO}:{remote_id}")),
                    };
                    let _ = store::put_file(db, &folder.id, &file.rel, &row).await;
                }
            }
        }
        Ok(count)
    }

    /// Where a `local.video` episode's file is: its folder's path and the file inside it.
    pub async fn video_path(&self, episode_remote_id: &str) -> Result<PathBuf, String> {
        let (folder_id, rel) = episode_remote_id
            .split_once('/')
            .ok_or("This video's address is broken")?;
        let folder = store::folder(self.db(), folder_id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or("Its folder was removed from Local Files")?;
        let base = folder.path.ok_or("This folder's videos can't be played")?;
        Ok(Path::new(&base).join(rel))
    }
}

impl Counts {
    fn add(&mut self, kind: Kind) {
        match kind {
            Kind::Comic => self.comics += 1,
            Kind::Book => self.books += 1,
            _ => {}
        }
    }
}

/// A folder of pages as one file: their total size and latest change.
fn pages_stamp(pages: &[&Found]) -> (i64, i64) {
    (
        pages.iter().map(|page| page.size).sum(),
        pages.iter().map(|page| page.modified).max().unwrap_or(0),
    )
}

/// A scan's progress, kept for `folders` and announced at most every [`PROGRESS_EVERY`].
struct ScanProgress<'a> {
    service: &'a LocalFilesService,
    id: &'a str,
    done: u32,
    total: u32,
    announced: Instant,
}

impl<'a> ScanProgress<'a> {
    fn new(service: &'a LocalFilesService, id: &'a str, total: u32) -> Self {
        let progress = Self {
            service,
            id,
            done: 0,
            total,
            announced: Instant::now(),
        };
        progress.keep();
        if total > 0 {
            service.changed(id);
        }
        progress
    }

    fn keep(&self) {
        self.service
            .progress
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(
                self.id.to_owned(),
                LocalScanProgress {
                    done: self.done,
                    total: self.total,
                },
            );
    }

    fn step(&mut self) {
        self.done = (self.done + 1).min(self.total);
        self.keep();
        if self.announced.elapsed() >= PROGRESS_EVERY {
            self.announced = Instant::now();
            self.service.changed(self.id);
        }
    }
}

/// Everything found in folders is counted in a map by kind, for tests.
#[cfg(test)]
fn sort_kinds(names: &[&str]) -> std::collections::HashMap<&'static str, usize> {
    let mut map = std::collections::HashMap::new();
    for name in names {
        let label = match kind_of(name) {
            Some(Kind::Song) => "song",
            Some(Kind::Comic) => "comic",
            Some(Kind::Book) => "book",
            Some(Kind::Video) => "video",
            Some(Kind::Image) => "image",
            None => "other",
        };
        *map.entry(label).or_default() += 1;
    }
    map
}

#[cfg(test)]
mod tests {
    use super::*;

    fn found(rel: &str) -> Found {
        Found {
            rel: rel.into(),
            size: 1,
            modified: 1,
            open: Opener::Path(PathBuf::from(rel)),
        }
    }

    #[test]
    fn a_restore_adds_folders_not_here_once_and_only_where_they_are() {
        let temp = tempfile::tempdir().unwrap();
        let music = temp.path().join("Music");
        std::fs::create_dir_all(&music).unwrap();
        let row = |location: &str, access: FolderAccess| store::FolderRow {
            id: format!("id-{location}"),
            location: location.into(),
            name: "Folder".into(),
            path: (access != FolderAccess::Tree).then(|| location.to_owned()),
            access,
            added_at: 1,
            scanned_at: None,
            counts: String::new(),
        };
        let here = path_string(&music);
        let gone = path_string(&temp.path().join("Gone"));
        let have: HashSet<String> = ["content://tree/kept".to_owned()].into();
        let added = new_folders(
            vec![
                row(&here, FolderAccess::Path),
                row(&here, FolderAccess::Path),
                row(&gone, FolderAccess::Added),
                row("content://tree/kept", FolderAccess::Tree),
            ],
            &have,
        );
        let locations: Vec<&str> = added
            .iter()
            .map(|folder| folder.location.as_str())
            .collect();
        assert_eq!(locations, [here.as_str(), gone.as_str()]);
        assert!(path_is_here(&added[0]));
        assert!(
            !path_is_here(&added[1]),
            "a folder that isn't here stays out"
        );
    }

    #[test]
    fn sorts_files_by_kind() {
        let kinds = sort_kinds(&[
            "a/Song.MP3",
            "b.flac",
            "Saga 01.cbz",
            "pages.zip",
            "Book.epub",
            "film.mkv",
            "show/S01E01.mp4",
            "p/001.jpg",
            "notes.txt",
        ]);
        assert_eq!(kinds["song"], 2);
        assert_eq!(kinds["comic"], 2);
        assert_eq!(kinds["book"], 1);
        assert_eq!(kinds["video"], 2);
        assert_eq!(kinds["image"], 1);
        assert_eq!(kinds["other"], 1);
    }

    #[test]
    fn reads_episode_numbers_from_names() {
        assert_eq!(episode_of("Show S02E05 1080p"), Some((Some(2.0), 5.0)));
        assert_eq!(episode_of("show 1x03"), Some((Some(1.0), 3.0)));
        assert_eq!(episode_of("Anime - Episode 12"), Some((None, 12.0)));
        assert_eq!(episode_of("Sintel"), None);
        assert_eq!(stem("Movies/Big_Buck.Bunny.mp4"), "Big Buck Bunny");
        assert_eq!(episode_of("Show II - 03"), Some((Some(2.0), 3.0)));
        assert_eq!(episode_of("Show S2 - 03"), Some((Some(2.0), 3.0)));
        assert_eq!(
            episode_of("[Group] Show 2nd Season - 03 [1080p]"),
            Some((Some(2.0), 3.0))
        );
        assert_eq!(
            episode_of("[SubsPlease] One Piece - 1071 (1080p)"),
            Some((None, 1071.0))
        );
        assert_eq!(
            episode_of("Movie 1920x1080"),
            None,
            "a resolution isn't 1920x1080"
        );
        assert_eq!(
            episode_of("Sleep 2"),
            None,
            "\"ep\" inside a word isn't a marker"
        );
        assert_eq!(
            episode_of("Ça Commence – Épisode 7"),
            Some((None, 7.0)),
            "text that isn't ASCII before the marker"
        );
        assert_eq!(episode_of("ジョジョ ep 3"), Some((None, 3.0)));
    }

    /// (key, name, series, [(file, season, number, name)]) of each title.
    #[allow(clippy::type_complexity)]
    fn grouped(
        rels: &[&str],
    ) -> Vec<(
        String,
        String,
        bool,
        Vec<(usize, Option<u32>, Option<f64>, String)>,
    )> {
        group_videos("Videos", rels)
            .into_iter()
            .map(|title| (title.key, title.name, title.series, title.episodes))
            .collect()
    }

    #[test]
    fn videos_group_into_shows_and_seasons() {
        // Season folders join their show.
        let seasons = grouped(&[
            "Show/Season 1/Show S01E01.mkv",
            "Show/Season 1/Show S01E02.mkv",
            "Show/Season 2/01.mkv",
            "Show/Season 2/02.mkv",
            "Show/Specials/Show - Recap.mkv",
        ]);
        assert_eq!(seasons.len(), 1, "{seasons:?}");
        let (key, name, series, episodes) = &seasons[0];
        assert_eq!(
            (key.as_str(), name.as_str(), *series),
            ("Show/", "Show", true)
        );
        let numbers: Vec<(Option<u32>, Option<f64>)> = episodes
            .iter()
            .map(|(_, season, number, _)| (*season, *number))
            .collect();
        assert_eq!(
            numbers,
            [
                (Some(1), Some(1.0)),
                (Some(1), Some(2.0)),
                (Some(2), Some(1.0)),
                (Some(2), Some(2.0)),
                (Some(0), None),
            ]
        );

        // "Show" and "Show II" in one folder: one series, seasons 1 and 2.
        let sequel = grouped(&[
            "Mushoku/[SubsPlease] Mushoku Tensei - 01 (1080p).mkv",
            "Mushoku/[SubsPlease] Mushoku Tensei - 02 (1080p).mkv",
            "Mushoku/[SubsPlease] Mushoku Tensei II - 01 (1080p).mkv",
        ]);
        assert_eq!(sequel.len(), 1);
        let seasons: Vec<Option<u32>> = sequel[0].3.iter().map(|episode| episode.1).collect();
        assert_eq!(seasons, [Some(1), Some(1), Some(2)]);
        assert_eq!(
            sequel[0].3[0].3, "Episode 1",
            "a release name isn't an episode's name"
        );

        // Several shows in the folder itself: a series each, movies on their own.
        let mixed = grouped(&[
            "[SubsPlease] Dandadan - 01 (1080p).mkv",
            "[SubsPlease] Dandadan - 02 (1080p).mkv",
            "[SubsPlease] Sousou no Frieren - 01 (1080p).mkv",
            "[SubsPlease] Sousou no Frieren - 02 (1080p).mkv",
            "Sintel.mkv",
        ]);
        let titles: Vec<(&str, &str, bool, usize)> = mixed
            .iter()
            .map(|(key, name, series, episodes)| {
                (key.as_str(), name.as_str(), *series, episodes.len())
            })
            .collect();
        assert_eq!(
            titles,
            [
                ("/dandadan", "Dandadan", true, 2),
                ("/sosonofrieren", "Sousou no Frieren", true, 2),
                ("Sintel.mkv", "Sintel", false, 1),
            ]
        );

        // Numbers ending names count once two of the show's have one; a lone sequel is a movie.
        let numbered = grouped(&["Show 01.mkv", "Show 02.mkv"]);
        assert_eq!(numbered.len(), 1);
        assert!(numbered[0].2);
        let films = grouped(&["Films/Big Movie.mkv", "Films/Big Movie 2.mkv"]);
        assert_eq!(films.len(), 2);
        assert!(films.iter().all(|film| !film.2));
    }

    #[test]
    fn finds_paths_of_shared_storage_folders() {
        assert_eq!(
            tree_path(
                "content://com.android.externalstorage.documents/tree/primary%3ADownload%2FRaygun%20Test"
            ),
            Some("/storage/emulated/0/Download/Raygun Test".into())
        );
        assert_eq!(
            tree_path("content://com.android.externalstorage.documents/tree/1A2B-3C4D%3AMusic"),
            Some("/storage/1A2B-3C4D/Music".into())
        );
        assert_eq!(
            tree_path("content://com.android.externalstorage.documents/tree/home%3AComics"),
            Some("/storage/emulated/0/Documents/Comics".into())
        );
        assert_eq!(
            tree_path(
                "content://com.android.providers.downloads.documents/tree/raw%3A%2Fstorage%2Femulated%2F0%2FDownload%2FShows"
            ),
            Some("/storage/emulated/0/Download/Shows".into())
        );
        assert_eq!(
            tree_path("content://com.android.providers.downloads.documents/tree/downloads"),
            Some("/storage/emulated/0/Download".into())
        );
        assert_eq!(
            tree_path("content://com.android.providers.downloads.documents/tree/msd%3A24"),
            None
        );
        assert_eq!(
            tree_path("content://com.google.android.apps.docs/tree/x"),
            None
        );
        // A broken escape is kept as it is, never a panic.
        assert_eq!(percent_decode("a%2"), "a%2");
        assert_eq!(percent_decode("%zz%C3%A9"), "%zzé");
    }

    #[test]
    fn walks_a_folder() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("Show/.hidden")).unwrap();
        std::fs::write(temp.path().join("Show/S01E01.mp4"), b"x").unwrap();
        std::fs::write(temp.path().join("Show/.hidden/a.mp3"), b"x").unwrap();
        std::fs::write(temp.path().join("Book.epub"), b"xy").unwrap();
        std::fs::write(temp.path().join("notes.txt"), b"xyz").unwrap();
        let mut found: Vec<String> = walk(temp.path())
            .unwrap()
            .into_iter()
            .map(|file| file.rel)
            .collect();
        found.sort();
        assert_eq!(
            found,
            vec!["Book.epub".to_owned(), "Show/S01E01.mp4".to_owned()],
            "only what Raygun reads"
        );
        assert!(
            walk(&temp.path().join("unplugged")).is_none(),
            "a folder that's gone isn't an empty one"
        );
    }

    #[test]
    fn photos_and_album_artwork_are_not_comics() {
        let pages = |dir: &str, names: &[&str]| -> Vec<Found> {
            names
                .iter()
                .map(|name| found(&format!("{dir}/{name}")))
                .collect()
        };
        let none = HashSet::new();
        let chapter = pages("Saga/Chapter 1", &["001.jpg", "002.jpg", "003.jpg"]);
        let refs: Vec<&Found> = chapter.iter().collect();
        assert!(is_comic_folder("Saga/Chapter 1", &refs, &none));
        assert!(!is_comic_folder("", &refs, &none), "the folder's own top");
        assert!(
            !is_comic_folder("Saga/Chapter 1", &refs[..2], &none),
            "two pictures"
        );

        let camera = pages("DCIM/Camera", &["a.jpg", "b.jpg", "c.jpg"]);
        let refs: Vec<&Found> = camera.iter().collect();
        assert!(!is_comic_folder("DCIM/Camera", &refs, &none));
        let named = pages("Holiday", &["IMG_0001.jpg", "IMG_0002.jpg", "PXL_3.jpg"]);
        let refs: Vec<&Found> = named.iter().collect();
        assert!(!is_comic_folder("Holiday", &refs, &none), "camera names");

        let album: HashSet<&str> = ["Music/Album"].into_iter().collect();
        let art = pages("Music/Album/Booklet Pages", &["1.jpg", "2.jpg", "3.jpg"]);
        let refs: Vec<&Found> = art.iter().collect();
        assert!(
            !is_comic_folder("Music/Album/Booklet Pages", &refs, &album),
            "scans under an album"
        );
        let beside = pages("Music/Album", &["cover.jpg", "back.jpg", "cd.jpg"]);
        let refs: Vec<&Found> = beside.iter().collect();
        assert!(
            !is_comic_folder("Music/Album", &refs, &album),
            "beside its songs"
        );
        let scans = pages("Scans", &["1.jpg", "2.jpg", "3.jpg"]);
        let refs: Vec<&Found> = scans.iter().collect();
        assert!(!is_comic_folder("Scans", &refs, &none), "named for artwork");
    }

    #[test]
    fn only_archives_of_pictures_are_comics() {
        let archive = |names: &[&str]| {
            let mut bytes = std::io::Cursor::new(Vec::new());
            let mut zip = zip::ZipWriter::new(&mut bytes);
            for name in names {
                zip.start_file(*name, zip::write::SimpleFileOptions::default())
                    .unwrap();
                std::io::Write::write_all(&mut zip, b"x").unwrap();
            }
            zip.finish().unwrap();
            bytes.set_position(0);
            bytes
        };
        assert_eq!(
            holds_pages(archive(&["01.jpg", "02.png", "ComicInfo.xml"])),
            Some(true)
        );
        assert_eq!(
            holds_pages(archive(&["setup.exe", "readme.txt", "icon.png"])),
            Some(false)
        );
        assert_eq!(
            holds_pages(std::io::Cursor::new(b"not a zip".to_vec())),
            None
        );
    }

    #[test]
    fn files_that_failed_wait_until_they_change_or_a_scan_is_asked_for() {
        let row = |item: Option<&str>, kind: &str| FileRow {
            size: 10,
            modified: 5,
            kind: kind.into(),
            item: item.map(str::to_owned),
        };
        let imported = row(Some("local.comics:saga"), "comic");
        let failed = row(None, "comic");
        let skipped = row(None, SKIPPED);
        assert!(matches!(
            earlier(Some(&imported), 10, 5, false),
            Earlier::Imported
        ));
        assert!(matches!(
            earlier(Some(&imported), 11, 5, false),
            Earlier::New
        ));
        assert!(matches!(
            earlier(Some(&failed), 10, 5, false),
            Earlier::Failed
        ));
        assert!(matches!(earlier(Some(&failed), 10, 5, true), Earlier::New));
        assert!(matches!(
            earlier(Some(&skipped), 10, 5, true),
            Earlier::Skipped
        ));
        assert!(matches!(earlier(None, 10, 5, false), Earlier::New));
    }

    #[test]
    fn folder_lines_written_before_keep_reading() {
        // Rows from before failures were counted have only the four kinds.
        let old: Counts =
            serde_json::from_str(r#"{"songs":2,"comics":1,"books":0,"videos":3}"#).unwrap();
        assert_eq!((old.songs, old.videos, old.failed), (2, 3, 0));
        assert!(!old.missing && !old.needs_access);
        let json = serde_json::to_string(&Counts {
            needs_access: true,
            ..Counts::default()
        })
        .unwrap();
        assert!(json.contains(r#""needsAccess":true"#));
    }

    fn touch(path: &Path, bytes: &[u8]) {
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, bytes).unwrap();
    }

    #[test]
    fn the_browser_stays_inside_the_storage() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        let download = root.join("Download");
        assert_eq!(inside(root, &download), Some(download.clone()));
        assert_eq!(inside(root, root), Some(root.to_path_buf()));
        assert_eq!(inside(root, &download.join("..").join("..")), None);
        assert_eq!(inside(root, Path::new("Download")), None, "relative");
        assert_eq!(
            inside(&root.join("Music"), &root.join("Musical")),
            None,
            "a neighbour whose name starts the same"
        );

        let trail = crumbs(root, &download.join("Shows"));
        let names: Vec<&str> = trail.iter().map(|crumb| crumb.name.as_str()).collect();
        assert_eq!(names, ["Download", "Shows"]);
        assert_eq!(trail[1].path, path_string(&download.join("Shows")));
        assert!(crumbs(root, root).is_empty(), "the top has no steps");
    }

    #[test]
    fn the_browser_lists_folders_with_what_they_hold() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        touch(&root.join("Download/Book.epub"), b"x");
        touch(&root.join("Download/Saga 01.cbz"), b"x");
        touch(&root.join("Download/notes.txt"), b"x");
        touch(&root.join("Download/Shows/S01E01.mkv"), b"x");
        touch(&root.join("music/Album/1.mp3"), b"x");
        touch(&root.join("music/Album/2.flac"), b"x");
        touch(&root.join("Documents/report.pdf"), b"x");
        touch(&root.join(".thumbnails/a.jpg"), b"x");
        std::fs::create_dir_all(root.join("Alarms")).unwrap();
        touch(&root.join("song.mp3"), b"x");

        let time = Budget::new(Duration::from_secs(30));
        let (folders, items) = list_folders(root, COUNT_ENTRIES, &time).unwrap();
        let listed: Vec<(&str, Option<u32>)> = folders
            .iter()
            .map(|folder| (folder.name.as_str(), folder.items))
            .collect();
        // By name whatever the case; nothing Raygun reads, hidden or empty: not shown.
        assert_eq!(listed, [("Download", Some(3)), ("music", Some(2))]);
        assert_eq!(items, 1, "the song right at the top");
        assert_eq!(folders[0].path, path_string(&root.join("Download")));

        // Too many files to count: shown, without a number.
        let (folders, _) = list_folders(root, 2, &time).unwrap();
        assert!(folders.iter().any(|folder| folder.items.is_none()));
        // Out of time: the rest aren't counted.
        let (folders, _) = list_folders(root, COUNT_ENTRIES, &Budget::new(Duration::ZERO)).unwrap();
        assert!(folders.iter().all(|folder| folder.items.is_none()));
        assert!(list_folders(&root.join("gone"), COUNT_ENTRIES, &time).is_none());
    }

    /// A folder chosen in the browser (a phone's Download, say) is walked by path and sorted like
    /// a picked one: songs and videos stay where they are, comics and books are imported.
    #[test]
    fn a_folder_read_by_path_scans_like_a_picked_one() {
        let temp = tempfile::TempDir::new().unwrap();
        let root = temp.path();
        for name in [
            "Book.epub",
            "Saga 01.cbz",
            "pages.zip",
            "Show/Show S01E01.mkv",
            "Show/Show S01E02.mkv",
            "Chapter 1/001.jpg",
            "Chapter 1/002.jpg",
            "Chapter 1/003.jpg",
            "DCIM/Camera/IMG_1.jpg",
            "DCIM/Camera/IMG_2.jpg",
            "DCIM/Camera/IMG_3.jpg",
            "Album/1.mp3",
            "Album/cover.jpg",
            "Album/back.jpg",
            "Album/cd.jpg",
            ".hidden/x.mp3",
        ] {
            touch(&root.join(name), b"x");
        }
        let found = walk(root).unwrap();
        let Survey {
            songs,
            videos,
            files,
            page_dirs,
        } = survey(&found);
        assert_eq!(songs, 1, "hidden folders are left out");
        let mut imports: Vec<(&str, Kind)> = files
            .iter()
            .map(|(file, kind)| (file.rel.as_str(), *kind))
            .collect();
        imports.sort_by_key(|(rel, _)| *rel);
        assert_eq!(
            imports,
            [
                ("Book.epub", Kind::Book),
                ("Saga 01.cbz", Kind::Comic),
                ("pages.zip", Kind::Comic)
            ]
        );
        let chapters: Vec<(&str, usize)> = page_dirs
            .iter()
            .map(|(dir, pages)| (*dir, pages.len()))
            .collect();
        assert_eq!(
            chapters,
            [("Chapter 1", 3)],
            "photos and an album's artwork aren't comics"
        );
        let mut rels: Vec<&str> = videos.iter().map(|file| file.rel.as_str()).collect();
        rels.sort_unstable();
        let titles = group_videos("Download", &rels);
        assert_eq!(titles.len(), 1);
        assert!(titles[0].series);
        assert_eq!(titles[0].episodes.len(), 2);
        // Every file opens by its path.
        assert!(
            found
                .iter()
                .all(|file| matches!(&file.open, Opener::Path(path) if path.is_file()))
        );
    }

    #[test]
    fn archives_android_hides_are_not_taken_for_gone() {
        let row = |kind: &str, item: Option<&str>| FileRow {
            size: 1,
            modified: 1,
            kind: kind.into(),
            item: item.map(str::to_owned),
        };
        assert!(hidden_archive(
            "Saga.cbz",
            &row("comic", Some("local.comics:saga"))
        ));
        assert!(hidden_archive("Book.epub", &row("book", None)));
        assert!(hidden_archive("setup.zip", &row(SKIPPED, None)));
        // Pictures and videos show with the media permissions: when they aren't listed, they're gone.
        assert!(!hidden_archive(
            "Chapter 1/",
            &row("comic", Some("local.comics:c"))
        ));
        assert!(!hidden_archive(
            "Show/S01E01.mkv",
            &row("video", Some("local.video:x"))
        ));
    }

    #[test]
    fn pictures_added_together_are_named_for_what_they_share() {
        assert_eq!(
            pictures_name(&["Chapter 5 - 001.jpg", "Chapter 5 - 002.jpg"]),
            "Chapter 5"
        );
        assert_eq!(pictures_name(&["Saga_p01.png", "Saga_p02.png"]), "Saga");
        assert_eq!(pictures_name(&["001.jpg", "002.jpg"]), "Pictures");
        assert_eq!(pictures_name(&["page_01.jpg", "page_02.jpg"]), "Pictures");
        assert_eq!(pictures_name(&["cat.jpg", "dog.jpg"]), "Pictures");
        assert_eq!(
            pictures_name(&["Saga p01.png"]),
            "Saga p01",
            "one keeps its name"
        );
    }

    #[test]
    fn added_files_keep_their_names() {
        let temp = tempfile::TempDir::new().unwrap();
        let dir = temp.path();
        assert_eq!(
            copy_target(dir, "Song.mp3", 10),
            Some(Some(dir.join("Song.mp3")))
        );
        std::fs::write(dir.join("Song.mp3"), [0u8; 10]).unwrap();
        assert_eq!(
            copy_target(dir, "Song.mp3", 10),
            Some(None),
            "the same file twice is one copy"
        );
        assert_eq!(
            copy_target(dir, "Song.mp3", 11),
            Some(Some(dir.join("Song (2).mp3"))),
            "another file of that name"
        );
        assert_eq!(
            copy_target(dir, "a/b:c.mp3", 1),
            Some(Some(dir.join("a_b_c.mp3")))
        );
        assert!(extensions().contains(&"epub") && extensions().contains(&"cbz"));
    }

    #[test]
    fn picked_files_without_an_extension_are_named_by_their_type() {
        assert_eq!(named_for_kind("Song.mp3", Some("audio/flac")), "Song.mp3");
        assert_eq!(
            named_for_kind("Dune", Some("application/epub+zip")),
            "Dune.epub"
        );
        assert_eq!(
            named_for_kind("Saga 01", Some("application/vnd.comicbook+zip")),
            "Saga 01.cbz"
        );
        assert_eq!(named_for_kind("clip", Some("VIDEO/MP4")), "clip.mp4");
        assert_eq!(named_for_kind("notes", Some("text/plain")), "notes");
        assert_eq!(named_for_kind("notes", None), "notes");
    }

    #[test]
    fn songs_open_by_path_only_with_access() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::write(temp.path().join("song.mp3"), b"x").unwrap();
        let base = temp.path().to_string_lossy().into_owned();
        assert!(readable_by_path(&base, &[found("song.mp3")]));
        assert!(!readable_by_path(&base, &[found("gone.mp3")]));
        assert!(
            readable_by_path(&base, &[found("Saga.cbz")]),
            "nothing read by path"
        );
    }
}
