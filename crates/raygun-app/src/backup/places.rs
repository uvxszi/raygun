//! Where backups are saved and found (M46.51): Raygun's data folder, `Download/Raygun`, and
//! folders picked for backups. The picked folders are remembered in `backups/folders.json`,
//! beside the backups, so an erase (which keeps that folder) still finds them.
//!
//! On Android the files outside the app are made, listed, read and deleted through the
//! `raygun-audio` plugin (MediaStore for `Download/Raygun`, the Storage Access Framework for a
//! picked folder), since Rust can't open `content://` addresses.

use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use raygun_model::{BackupFolder, BackupPlace};
use serde::{Deserialize, Serialize};
use tauri::AppHandle;

use super::format;
use crate::export::{Outputs, Place};

pub const FOLDERS_FILE: &str = "folders.json";
const MIME: &str = "application/octet-stream";
/// Folders remembered; picking one more forgets the one used longest ago.
const FOLDERS_KEPT: usize = 12;

/// The folders picked for backups and the place used last.
#[derive(Debug, Default, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Folders {
    pub folders: Vec<BackupFolder>,
    /// A folder's `uri`, or `None` for the usual place.
    pub last: Option<String>,
}

impl Folders {
    pub fn load(dir: &Path) -> Self {
        std::fs::read(dir.join(FOLDERS_FILE))
            .ok()
            .and_then(|bytes| serde_json::from_slice(&bytes).ok())
            .unwrap_or_default()
    }

    pub fn save(&self, dir: &Path) -> Result<(), String> {
        std::fs::create_dir_all(dir).map_err(to_message)?;
        let json = serde_json::to_vec_pretty(self).map_err(to_message)?;
        let partial = dir.join(format!("{FOLDERS_FILE}.part"));
        std::fs::write(&partial, json).map_err(to_message)?;
        std::fs::rename(&partial, dir.join(FOLDERS_FILE)).map_err(to_message)
    }

    /// Remembers a folder (or when it was used again) as the place used last.
    pub fn remember(&mut self, uri: &str, name: &str, now_ms: f64) {
        match self.folders.iter_mut().find(|folder| folder.uri == uri) {
            Some(folder) => {
                folder.used_at = now_ms;
                folder.name = name.to_owned();
            }
            None => self.folders.push(BackupFolder {
                uri: uri.to_owned(),
                name: name.to_owned(),
                used_at: now_ms,
            }),
        }
        self.folders.sort_by(|a, b| b.used_at.total_cmp(&a.used_at));
        self.folders.truncate(FOLDERS_KEPT);
        self.last = Some(uri.to_owned());
    }

    pub fn forget(&mut self, uri: &str) {
        self.folders.retain(|folder| folder.uri != uri);
        if self.last.as_deref() == Some(uri) {
            self.last = None;
        }
    }

    pub fn find(&self, uri: &str) -> Option<&BackupFolder> {
        self.folders.iter().find(|folder| folder.uri == uri)
    }
}

/// Where a backup file is.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Location {
    Path(PathBuf),
    /// A `content://` (or, on old Android, `file://`) address the plugin opens.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Uri(String),
}

impl Location {
    pub fn id(&self) -> String {
        match self {
            Location::Path(path) => path.to_string_lossy().into_owned(),
            Location::Uri(uri) => uri.clone(),
        }
    }
}

/// A backup file found or written.
#[derive(Debug, Clone, PartialEq)]
pub struct Found {
    pub location: Location,
    pub name: String,
    pub place: BackupPlace,
    /// The folder as shown.
    pub folder: String,
    pub size: u64,
    /// Unix ms.
    pub modified_ms: f64,
}

/// Where a new backup goes.
pub enum Target {
    /// The data folder's `backups/` (the usual place on a computer).
    App,
    /// `Download/Raygun` (the usual place on Android).
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    Downloads,
    Folder {
        uri: String,
        name: String,
    },
}

/// The usual place, as shown: on a phone `Download/Raygun`, which an uninstall leaves; on a
/// computer the portable data folder, which travels with the app.
pub fn usual() -> (BackupPlace, &'static str) {
    if cfg!(target_os = "android") {
        (BackupPlace::Downloads, "Download/Raygun")
    } else {
        (BackupPlace::App, "Raygun")
    }
}

pub fn usual_target() -> Target {
    if cfg!(target_os = "android") {
        Target::Downloads
    } else {
        Target::App
    }
}

/// Backups in a plain folder, not its sub-folders: named as backups (the content is checked
/// when they're read).
pub fn list_dir(dir: &Path, place: BackupPlace, folder: &str) -> Vec<Found> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            let metadata = entry.metadata().ok()?;
            (metadata.is_file() && format::is_backup_name(&name)).then(|| Found {
                location: Location::Path(entry.path()),
                name,
                place,
                folder: folder.to_owned(),
                size: metadata.len(),
                modified_ms: metadata
                    .modified()
                    .ok()
                    .and_then(|time| time.duration_since(std::time::UNIX_EPOCH).ok())
                    .map_or(0.0, |elapsed| elapsed.as_millis() as f64),
            })
        })
        .collect()
}

/// Backups Android lists in `Download/Raygun` (`tree` `None`) or a picked folder. Blocking.
#[cfg(target_os = "android")]
pub fn list_native(
    app: &AppHandle,
    tree: Option<&str>,
    place: BackupPlace,
    folder: &str,
) -> Result<Vec<Found>, String> {
    use tauri::Manager;
    let files = app
        .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .list_saved(
            if tree.is_some() { "tree" } else { "downloads" },
            tree,
            &format!(".{}", format::EXTENSION),
        )?;
    Ok(files
        .into_iter()
        .filter(|file| format::is_backup_name(&file.name))
        .map(|file| Found {
            location: Location::Uri(file.uri),
            name: file.name,
            place,
            folder: folder.to_owned(),
            size: file.size.max(0) as u64,
            modified_ms: file.modified as f64,
        })
        .collect())
}

/// Reads a backup file, at most `limit` bytes (the whole file, up to the largest a backup may
/// be, without one). Blocking.
pub fn read(app: &AppHandle, location: &Location, limit: Option<usize>) -> Result<Vec<u8>, String> {
    let file = open(app, location)?;
    let most = limit.map_or(format::MAX_FILE_BYTES + 1, |limit| limit as u64);
    let mut bytes = Vec::new();
    file.take(most)
        .read_to_end(&mut bytes)
        .map_err(|error| format!("The file couldn't be read ({error})"))?;
    if limit.is_none() && bytes.len() as u64 > format::MAX_FILE_BYTES {
        return Err("This file is too large to be a backup".into());
    }
    Ok(bytes)
}

fn open(_app: &AppHandle, location: &Location) -> Result<std::fs::File, String> {
    match location {
        Location::Path(path) => std::fs::File::open(path)
            .map_err(|error| format!("The file couldn't be opened ({error})")),
        #[cfg(target_os = "android")]
        Location::Uri(uri) => {
            use std::os::fd::FromRawFd;
            use tauri::Manager;
            let fd = _app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .open_input(uri)?;
            // SAFETY: Kotlin detached this descriptor for us (`detachFd`); nothing else owns it.
            Ok(unsafe { std::fs::File::from_raw_fd(fd) })
        }
        #[cfg(not(target_os = "android"))]
        Location::Uri(_) => Err("This file can't be opened here".into()),
    }
}

/// Writes a new backup file. Blocking.
pub fn write(
    outputs: &Outputs,
    app_dir: &Path,
    target: &Target,
    name: &str,
    bytes: &[u8],
) -> Result<Found, String> {
    let (place, folder) = match target {
        Target::App => {
            std::fs::create_dir_all(app_dir).map_err(to_message)?;
            let path = app_dir.join(name);
            let partial = app_dir.join(format!("{name}.part"));
            std::fs::write(&partial, bytes).map_err(to_message)?;
            std::fs::rename(&partial, &path).map_err(to_message)?;
            return Ok(Found {
                location: Location::Path(path),
                name: name.to_owned(),
                place: BackupPlace::App,
                folder: "Raygun".into(),
                size: bytes.len() as u64,
                modified_ms: now_ms(),
            });
        }
        Target::Downloads => (Place::Downloads, BackupPlace::Downloads),
        Target::Folder { uri, name } => (
            Place::Folder {
                uri: uri.clone(),
                name: name.clone(),
            },
            BackupPlace::Folder,
        ),
    };
    let mut output = outputs.create(&place, &[], name, MIME)?;
    let written = output.file.write_all(bytes);
    let given = output.name.clone();
    let saved = outputs.finish(output, written.is_ok())?;
    written.map_err(|error| format!("The backup couldn't be written ({error})"))?;
    let location = match (saved.uri, saved.local) {
        (_, Some(path)) => Location::Path(path),
        (Some(uri), None) => Location::Uri(uri),
        (None, None) => return Err("The backup was written but can't be found".into()),
    };
    let shown_folder = match target {
        Target::Folder { name, .. } => name.clone(),
        _ => usual().1.to_owned(),
    };
    Ok(Found {
        location,
        name: if given.is_empty() {
            name.to_owned()
        } else {
            given
        },
        place: folder,
        folder: shown_folder,
        size: bytes.len() as u64,
        modified_ms: now_ms(),
    })
}

/// Deletes a backup file. Blocking.
pub fn delete(_app: &AppHandle, location: &Location) -> Result<(), String> {
    match location {
        Location::Path(path) => std::fs::remove_file(path).map_err(to_message),
        #[cfg(target_os = "android")]
        Location::Uri(uri) => {
            use tauri::Manager;
            _app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .delete_file(uri)
        }
        #[cfg(not(target_os = "android"))]
        Location::Uri(_) => Err("This file can't be deleted here".into()),
    }
}

fn now_ms() -> f64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0.0, |elapsed| elapsed.as_millis() as f64)
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn folders_are_remembered_newest_first_and_forgotten() {
        let temp = tempfile::tempdir().unwrap();
        let mut folders = Folders::load(temp.path());
        assert_eq!(folders, Folders::default());
        folders.remember("/a", "A", 1.0);
        folders.remember("/b", "B", 2.0);
        folders.remember("/a", "A", 3.0);
        folders.save(temp.path()).unwrap();

        let mut read = Folders::load(temp.path());
        let uris: Vec<_> = read
            .folders
            .iter()
            .map(|folder| folder.uri.as_str())
            .collect();
        assert_eq!(uris, ["/a", "/b"]);
        assert_eq!(read.last.as_deref(), Some("/a"));
        read.forget("/a");
        assert_eq!(read.last, None);
        assert!(read.find("/a").is_none() && read.find("/b").is_some());
    }

    #[test]
    fn a_folder_lists_only_backups_beside_it() {
        let temp = tempfile::tempdir().unwrap();
        let dir = temp.path();
        std::fs::write(dir.join("raygun-20261105-120000.raygunbk"), b"x").unwrap();
        std::fs::write(dir.join("raygun-20261105-120001.raygunbk.part"), b"x").unwrap();
        std::fs::write(dir.join("photo.jpg"), b"x").unwrap();
        std::fs::create_dir_all(dir.join("old")).unwrap();
        std::fs::write(dir.join("old/raygun-1.raygunbk"), b"x").unwrap();
        let found = list_dir(dir, BackupPlace::Folder, "Backups");
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].name, "raygun-20261105-120000.raygunbk");
        assert_eq!(
            (found[0].place, found[0].folder.as_str()),
            (BackupPlace::Folder, "Backups")
        );
    }
}
