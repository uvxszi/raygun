//! Local Files (M46.27): folders the user adds, scanned for everything Raygun plays or reads.
//! Songs join the music library (the folder becomes one of its roots), comics (CBZ/ZIP, or a
//! folder of images) and books (EPUB) are imported as they are from uploads, and videos become
//! titles of the source `local.video` in Library → Videos, played from where they are.
//!
//! On Android a folder comes from the system picker. Songs and videos are read by path, which
//! the media permissions allow, and comics and books through the picker's grant (the plugin
//! lists the folder and opens each file), because Rust can't open `content://`.
//!
//! A scan reports how far it got, remembers files that couldn't be added (they're tried again
//! only when they change or the scan is asked for), and leaves everything as it was when the
//! folder can't be opened (an unplugged drive, a moved folder, a lost grant).

use std::collections::{BTreeMap, HashMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_db::local_folders::{self as store, FileRow};
use raygun_db::titles::{self, TitleFields, UnitFields};
use raygun_model::{LocalCounts, LocalFolder, LocalScanProgress};
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
    const INTERNAL: &str = "/storage/emulated/0";
    let (provider, rest) = location.strip_prefix("content://")?.split_once("/tree/")?;
    let document = percent_decode(rest.split('/').next()?);
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
        "com.android.providers.downloads.documents" => match document.as_str() {
            "downloads" => Some(format!("{INTERNAL}/Download")),
            raw => raw
                .strip_prefix("raw:")
                .filter(|path| path.starts_with('/'))
                .map(|path| path.trim_end_matches('/').to_owned()),
        },
        _ => None,
    }
}

fn percent_decode(text: &str) -> String {
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

/// A video file's season and episode from its name (`S01E02`, `1x02`, `Episode 3`, `E05`).
pub fn episode_of(name: &str) -> Option<(Option<f64>, f64)> {
    let lower = name.to_ascii_lowercase();
    let chars: Vec<char> = lower.chars().collect();
    let number_at = |start: usize| -> Option<(f64, usize)> {
        let digits: String = chars[start..]
            .iter()
            .take_while(|c| c.is_ascii_digit())
            .collect();
        (!digits.is_empty()).then(|| (digits.parse().unwrap_or(0.0), start + digits.len()))
    };
    for (index, &c) in chars.iter().enumerate() {
        let before_ok = index == 0 || !chars[index - 1].is_ascii_alphanumeric();
        // S01E02
        if c == 's'
            && before_ok
            && let Some((season, after)) = number_at(index + 1)
            && chars.get(after) == Some(&'e')
            && let Some((episode, _)) = number_at(after + 1)
        {
            return Some((Some(season), episode));
        }
        // 1x02
        if c.is_ascii_digit()
            && before_ok
            && let Some((season, after)) = number_at(index)
            && chars.get(after) == Some(&'x')
            && let Some((episode, _)) = number_at(after + 1)
        {
            return Some((Some(season), episode));
        }
    }
    for marker in ["episode ", "episode", "ep ", "ep."] {
        if let Some(at) = lower.find(marker)
            && let Some((episode, _)) = number_at(at + marker.len())
        {
            return Some((None, episode));
        }
    }
    None
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

    /// Adds a folder the user picks and scans it. `None` when they cancelled.
    pub async fn add(self: &Arc<Self>) -> Result<Option<Vec<LocalFolder>>, String> {
        let picked = crate::commands::pick_local_folder(&self.app).await?;
        let Some(location) = picked.uri else {
            return Ok(None);
        };
        let path = if location.starts_with("content://") {
            // Songs and videos are read by path, which needs the media permissions; a scan
            // tells when they're missing.
            crate::commands::request_media_access(&self.app).await;
            tree_path(&location)
        } else {
            Some(location.clone())
        };
        let name = picked.name.unwrap_or_else(|| "Folder".into());
        let id = store::add(
            self.db(),
            &uuid::Uuid::now_v7().to_string(),
            &location,
            &name,
            path.as_deref(),
            now_ms(),
        )
        .await
        .map_err(|error| error.to_string())?;
        self.scan_later(id, true);
        Ok(Some(self.folders().await?))
    }

    /// Forgets a folder: its songs leave the music library and its videos the video library.
    /// Imported comics and books stay (they were copied in). A scan of it stops first.
    pub async fn remove(&self, id: &str) -> Result<Vec<LocalFolder>, String> {
        self.cancelled
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.to_owned());
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
        }
        store::remove(db, id)
            .await
            .map_err(|error| error.to_string())
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
        if !self
            .scanning
            .lock()
            .unwrap_or_else(|p| p.into_inner())
            .insert(id.clone())
        {
            return;
        }
        self.changed(&id);
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let turn = service.turn(&id);
            {
                let _turn = turn.lock().await;
                if let Err(error) = service.scan(&id, retry_failed).await {
                    tracing::warn!(%error, "local folder scan failed");
                }
            }
            service
                .scanning
                .lock()
                .unwrap_or_else(|p| p.into_inner())
                .remove(&id);
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

    /// The folder's files, or an error when it can't be opened.
    async fn list(&self, folder: &store::FolderRow) -> Result<Vec<Found>, String> {
        #[cfg(target_os = "android")]
        if folder.location.starts_with("content://") {
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

        // Songs: the folder is one of the music library's roots, which scans them. Without a
        // path (a cloud folder), songs and videos can't play from it.
        let songs = found
            .iter()
            .filter(|file| kind_of(&file.rel) == Some(Kind::Song))
            .count() as u32;
        match &folder.path {
            Some(path) => {
                counts.songs = songs;
                counts.needs_access =
                    folder.location.starts_with("content://") && !readable_by_path(path, &found);
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
                counts.unplayable = songs
                    + found
                        .iter()
                        .filter(|file| kind_of(&file.rel) == Some(Kind::Video))
                        .count() as u32;
            }
        }

        // What to import: comic and book files, and folders of pages.
        let files: Vec<(&Found, Kind)> = found
            .iter()
            .filter_map(|file| match kind_of(&file.rel) {
                Some(kind @ (Kind::Comic | Kind::Book)) => Some((file, kind)),
                _ => None,
            })
            .collect();
        let media_dirs: HashSet<&str> = found
            .iter()
            .filter(|file| matches!(kind_of(&file.rel), Some(Kind::Song | Kind::Video)))
            .map(|file| parent(&file.rel))
            .collect();
        let mut image_dirs: BTreeMap<&str, Vec<&Found>> = BTreeMap::new();
        for file in found
            .iter()
            .filter(|file| kind_of(&file.rel) == Some(Kind::Image))
        {
            image_dirs.entry(parent(&file.rel)).or_default().push(file);
        }
        let page_dirs: Vec<(&str, Vec<&Found>)> = image_dirs
            .into_iter()
            .filter(|(dir, pages)| is_comic_folder(dir, pages, &media_dirs))
            .collect();
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
                .store_videos(&folder, &found, &known, &mut seen)
                .await?;
        }

        // Files that are gone: their rows go; a video title with no files left leaves the library.
        let mut gone_titles = HashSet::new();
        for (rel, row) in &known {
            if seen.contains(rel) {
                continue;
            }
            if row.kind == "video"
                && let Some(item) = &row.item
            {
                gone_titles.insert(item.clone());
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
        found: &[Found],
        known: &HashMap<String, FileRow>,
        seen: &mut HashSet<String>,
    ) -> Result<u32, String> {
        let db = self.db();
        let mut by_dir: BTreeMap<String, Vec<&Found>> = BTreeMap::new();
        for file in found
            .iter()
            .filter(|file| kind_of(&file.rel) == Some(Kind::Video))
        {
            by_dir
                .entry(parent(&file.rel).to_owned())
                .or_default()
                .push(file);
        }
        let now = now_ms();
        let mut count = 0;
        for (dir, mut files) in by_dir {
            let known_here = known
                .iter()
                .filter(|(rel, row)| row.kind == "video" && parent(rel) == dir)
                .count();
            let unchanged = known_here == files.len()
                && files.iter().all(|file| {
                    known.get(&file.rel).is_some_and(|row| {
                        row.size == file.size && row.modified == file.modified && row.item.is_some()
                    })
                });
            if unchanged {
                count += files.len() as u32;
                seen.extend(files.iter().map(|file| file.rel.clone()));
                continue;
            }
            files.sort_by(|a, b| crate::comics::natural_cmp(&a.rel, &b.rel));
            let episodes = files
                .iter()
                .filter(|file| episode_of(&stem(&file.rel)).is_some())
                .count();
            let series = files.len() > 1 && episodes * 2 >= files.len();
            let groups: Vec<(String, String, Vec<&Found>)> = if series {
                let name = if dir.is_empty() {
                    folder.name.clone()
                } else {
                    file_name(&dir).to_owned()
                };
                vec![(format!("{}:{dir}/", folder.id), name, files)]
            } else {
                files
                    .into_iter()
                    .map(|file| {
                        (
                            format!("{}:{}", folder.id, file.rel),
                            stem(&file.rel),
                            vec![file],
                        )
                    })
                    .collect()
            };
            for (remote_id, name, members) in groups {
                let single = members.len() == 1;
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
                    .enumerate()
                    .map(|(index, file)| {
                        let (season, number) =
                            episode_of(&stem(&file.rel)).unwrap_or((None, (index + 1) as f64));
                        UnitFields {
                            remote_id: format!("{}/{}", folder.id, file.rel),
                            name: stem(&file.rel),
                            number: Some(number),
                            volume: season.map(|season| format!("{season}")),
                            scanlator: None,
                            date: None,
                        }
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
                for file in members {
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
