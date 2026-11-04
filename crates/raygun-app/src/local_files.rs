//! Local Files (M46.27): folders the user adds, scanned for everything Raygun plays or reads.
//! Songs join the music library (the folder becomes one of its roots), comics (CBZ/ZIP, or a
//! folder of images) and books (EPUB) are imported as they are from uploads, and videos become
//! titles of the source `local.video` in Library → Videos, played from where they are.
//!
//! On Android a folder comes from the system picker. Songs and videos are read by path, which
//! the media permissions allow, and comics and books through the picker's grant (the plugin
//! lists the folder and opens each file), because Rust can't open `content://`.

use std::collections::{BTreeMap, HashSet};
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};

use raygun_db::local_folders::{self as store, FileRow};
use raygun_db::titles::{self, TitleFields, UnitFields};
use raygun_model::{LocalCounts, LocalFolder};
use raygun_resolver::video_match;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::LocalFilesEvent;
use crate::startup::AppDb;

/// The source of videos found in Local Files folders.
pub const LOCAL_VIDEO: &str = "local.video";
/// Files listed in one folder, at most.
const MAX_FILES: usize = 20_000;
/// Comics and books larger than this aren't imported.
const MAX_IMPORT_BYTES: u64 = 512 * 1024 * 1024;

const SONGS: &[&str] = &[
    "mp3", "flac", "m4a", "aac", "ogg", "oga", "opus", "wav", "wma", "aiff",
];
const VIDEOS: &[&str] = &["mp4", "mkv", "webm", "mov", "avi", "m4v", "wmv", "ts"];
const IMAGES: &[&str] = &["jpg", "jpeg", "png", "webp", "gif", "avif"];

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

/// The file path of an Android folder from the shared-storage picker, which the media
/// permissions let Raygun read by path (`primary:Music/x` → `/storage/emulated/0/Music/x`).
pub fn tree_path(location: &str) -> Option<String> {
    let rest = location.strip_prefix("content://com.android.externalstorage.documents/tree/")?;
    let document = percent_decode(rest.split('/').next()?);
    let (volume, inside) = document.split_once(':')?;
    let base = if volume == "primary" {
        "/storage/emulated/0".to_owned()
    } else {
        format!("/storage/{volume}")
    };
    Some(if inside.is_empty() {
        base
    } else {
        format!("{base}/{inside}")
    })
}

fn percent_decode(text: &str) -> String {
    let bytes = text.as_bytes();
    let mut out = Vec::with_capacity(bytes.len());
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == b'%'
            && index + 2 < bytes.len()
            && let Ok(value) = u8::from_str_radix(&text[index + 1..index + 3], 16)
        {
            out.push(value);
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
    let file = name.rsplit('/').next().unwrap_or(name);
    file.rsplit_once('.')
        .map_or(file, |(stem, _)| stem)
        .replace(['_', '.'], " ")
        .trim()
        .to_owned()
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

/// Lists a folder's files by path, hidden folders left out.
fn walk(root: &Path) -> Vec<Found> {
    let mut found = Vec::new();
    let mut pending = vec![(root.to_path_buf(), String::new())];
    while let Some((dir, prefix)) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
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
            } else if found.len() < MAX_FILES {
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
    found
}

/// What a scan found and did, for the folder's line.
#[derive(Debug, Default, serde::Serialize, serde::Deserialize)]
struct Counts {
    songs: u32,
    comics: u32,
    books: u32,
    videos: u32,
}

pub struct LocalFilesService {
    app: AppHandle,
    scanning: Mutex<HashSet<String>>,
}

impl LocalFilesService {
    pub fn new(app: AppHandle) -> Arc<Self> {
        Arc::new(Self {
            app,
            scanning: Mutex::new(HashSet::new()),
        })
    }

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
    }

    pub async fn folders(&self) -> Result<Vec<LocalFolder>, String> {
        let scanning = self
            .scanning
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
                    id: row.id,
                    name: row.name,
                    location: row.path.unwrap_or(row.location),
                    counts: LocalCounts {
                        songs: counts.songs,
                        comics: counts.comics,
                        books: counts.books,
                        videos: counts.videos,
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
            // Songs and videos are read by path, which needs the media permissions.
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
        self.scan_later(id);
        Ok(Some(self.folders().await?))
    }

    /// Forgets a folder: its songs leave the music library and its videos the video library.
    /// Imported comics and books stay (they were copied in).
    pub async fn remove(&self, id: &str) -> Result<Vec<LocalFolder>, String> {
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
        }
        store::remove(db, id)
            .await
            .map_err(|error| error.to_string())?;
        self.changed(id);
        self.folders().await
    }

    /// Scans every folder again (changed files only), in the background.
    pub async fn rescan_all(self: &Arc<Self>) -> Result<(), String> {
        for folder in store::folders(self.db())
            .await
            .map_err(|error| error.to_string())?
        {
            self.scan_later(folder.id);
        }
        Ok(())
    }

    pub fn scan_later(self: &Arc<Self>, id: String) {
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
            if let Err(error) = service.scan(&id).await {
                tracing::warn!(%error, "local folder scan failed");
            }
            service
                .scanning
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
        tauri::async_runtime::spawn_blocking(move || Ok(walk(&root)))
            .await
            .map_err(|error| error.to_string())?
    }

    fn read(&self, file: &Found) -> Result<Vec<u8>, String> {
        if file.size as u64 > MAX_IMPORT_BYTES {
            return Err(format!("{} is too large to import", file.rel));
        }
        match &file.open {
            Opener::Path(path) => std::fs::read(path).map_err(|error| error.to_string()),
            Opener::Document(_uri) => {
                #[cfg(target_os = "android")]
                {
                    use std::io::Read;
                    use std::os::fd::FromRawFd;
                    let fd = self
                        .app
                        .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                        .open_input(_uri)?;
                    // The plugin detached the descriptor for Rust, which now owns it.
                    let mut input = unsafe { std::fs::File::from_raw_fd(fd) };
                    let mut bytes = Vec::new();
                    input
                        .read_to_end(&mut bytes)
                        .map_err(|error| error.to_string())?;
                    Ok(bytes)
                }
                #[cfg(not(target_os = "android"))]
                Err("documents open only on Android".into())
            }
        }
    }

    async fn scan(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let db = self.db();
        let folder = store::folder(db, id)
            .await
            .map_err(|error| error.to_string())?
            .ok_or("This folder was removed")?;
        let found = self.list(&folder).await?;
        let known = store::files(db, id)
            .await
            .map_err(|error| error.to_string())?;
        let mut counts = Counts::default();
        let unchanged = |rel: &str, file: &Found| {
            known.get(rel).is_some_and(|row| {
                row.size == file.size && row.modified == file.modified && row.item.is_some()
            })
        };
        let mut seen = HashSet::new();

        // Songs: the folder is one of the music library's roots, which scans them.
        counts.songs = found
            .iter()
            .filter(|file| kind_of(&file.rel) == Some(Kind::Song))
            .count() as u32;
        if counts.songs > 0
            && let Some(path) = &folder.path
        {
            raygun_db::local_music::add_root(db, path)
                .await
                .map_err(|error| error.to_string())?;
            self.app
                .state::<crate::library::LibraryService>()
                .start_scan();
        }

        // Comics and books, imported once per version of the file.
        let manga = self.app.state::<crate::manga::MangaService>();
        let novels = self.app.state::<crate::novel::NovelService>();
        for file in &found {
            let kind = kind_of(&file.rel);
            if !matches!(kind, Some(Kind::Comic | Kind::Book)) {
                continue;
            }
            seen.insert(file.rel.clone());
            if unchanged(&file.rel, file) {
                if kind == Some(Kind::Comic) {
                    counts.comics += 1;
                } else {
                    counts.books += 1;
                }
                continue;
            }
            let service = self.clone();
            let copy = file.clone();
            let bytes = match tauri::async_runtime::spawn_blocking(move || service.read(&copy))
                .await
                .map_err(|error| error.to_string())?
            {
                Ok(bytes) => bytes,
                Err(error) => {
                    tracing::info!(%error, file = %file.rel, "couldn't read a local file");
                    continue;
                }
            };
            let name = file.rel.rsplit('/').next().unwrap_or(&file.rel).to_owned();
            let item = if kind == Some(Kind::Comic) {
                let temp = self.temp_file(&name)?;
                std::fs::write(&temp, &bytes).map_err(|error| error.to_string())?;
                manga
                    .import_comic(temp, &name)
                    .await
                    .map(|(source, key)| format!("{source}:{key}"))
            } else {
                novels
                    .import_epub_bytes(bytes)
                    .await
                    .map(|(source, key)| format!("{source}:{key}"))
            };
            match item {
                Ok(item) => {
                    if kind == Some(Kind::Comic) {
                        counts.comics += 1;
                    } else {
                        counts.books += 1;
                    }
                    let row = FileRow {
                        size: file.size,
                        modified: file.modified,
                        kind: if kind == Some(Kind::Comic) {
                            "comic"
                        } else {
                            "book"
                        }
                        .into(),
                        item: Some(item),
                    };
                    let _ = store::put_file(db, id, &file.rel, &row).await;
                }
                Err(error) => {
                    tracing::info!(%error, file = %file.rel, "couldn't import a local file")
                }
            }
        }

        // Folders of images are comics too: one chapter each, named after the folder.
        let mut image_dirs: BTreeMap<String, Vec<&Found>> = BTreeMap::new();
        for file in found
            .iter()
            .filter(|file| kind_of(&file.rel) == Some(Kind::Image))
        {
            image_dirs
                .entry(parent(&file.rel).to_owned())
                .or_default()
                .push(file);
        }
        for (dir, mut pages) in image_dirs {
            // A lone picture (a cover beside songs) isn't a comic.
            if pages.len() < 3 || dir.is_empty() {
                continue;
            }
            let rel = format!("{dir}/");
            seen.insert(rel.clone());
            let size: i64 = pages.iter().map(|page| page.size).sum();
            let modified = pages.iter().map(|page| page.modified).max().unwrap_or(0);
            if known.get(&rel).is_some_and(|row| {
                row.size == size && row.modified == modified && row.item.is_some()
            }) {
                counts.comics += 1;
                continue;
            }
            pages.sort_by(|a, b| crate::comics::natural_cmp(&a.rel, &b.rel));
            let name = format!("{}.cbz", dir.rsplit('/').next().unwrap_or(&dir));
            let temp = self.temp_file(&name)?;
            let service = self.clone();
            let owned: Vec<Found> = pages.into_iter().cloned().collect();
            let target = temp.clone();
            let zipped =
                tauri::async_runtime::spawn_blocking(move || service.zip_pages(&owned, &target))
                    .await
                    .map_err(|error| error.to_string())?;
            if let Err(error) = zipped {
                tracing::info!(%error, folder = %dir, "couldn't gather a folder of pages");
                continue;
            }
            match manga.import_comic(temp, &name).await {
                Ok((source, key)) => {
                    counts.comics += 1;
                    let row = FileRow {
                        size,
                        modified,
                        kind: "comic".into(),
                        item: Some(format!("{source}:{key}")),
                    };
                    let _ = store::put_file(db, id, &rel, &row).await;
                }
                Err(error) => {
                    tracing::info!(%error, folder = %dir, "couldn't import a folder of pages")
                }
            }
        }

        // Videos: a folder of episodes is a series; a lone file a movie.
        if folder.path.is_some() {
            counts.videos = self.store_videos(&folder, &found, &mut seen).await?;
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
            "local folder scanned"
        );
        Ok(())
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
    async fn store_videos(
        &self,
        folder: &store::FolderRow,
        found: &[Found],
        seen: &mut HashSet<String>,
    ) -> Result<u32, String> {
        let db = self.db();
        let now = now_ms();
        let mut count = 0;
        let mut files: Vec<&Found> = found
            .iter()
            .filter(|file| kind_of(&file.rel) == Some(Kind::Video))
            .collect();
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
        assert_eq!(episode_of("Sleep 2"), None, "\"ep\" inside a word isn't a marker");
        assert_eq!(
            episode_of("Ça Commence – Épisode 7"),
            Some((None, 7.0)),
            "text that isn't ASCII before the marker"
        );
        assert_eq!(episode_of("ジョジョ ep 3"), Some((None, 3.0)));
    }

    /// (key, name, series, [(file, season, number, name)]) of each title.
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
            tree_path("content://com.google.android.apps.docs/tree/x"),
            None
        );
    }

    #[test]
    fn walks_a_folder() {
        let temp = tempfile::TempDir::new().unwrap();
        std::fs::create_dir_all(temp.path().join("Show/.hidden")).unwrap();
        std::fs::write(temp.path().join("Show/S01E01.mp4"), b"x").unwrap();
        std::fs::write(temp.path().join("Show/.hidden/a.mp3"), b"x").unwrap();
        std::fs::write(temp.path().join("Book.epub"), b"xy").unwrap();
        let mut found: Vec<String> = walk(temp.path()).into_iter().map(|file| file.rel).collect();
        found.sort();
        assert_eq!(
            found,
            vec!["Book.epub".to_owned(), "Show/S01E01.mp4".to_owned()]
        );
    }
}
