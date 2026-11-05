//! Picking a file or a folder for a field that takes a link or a path (M46.52), so a path never
//! has to be typed. Desktop shows the system's dialog, which gives a path. Android shows the
//! system picker, whose item Raygun reads through the plugin and copies into `picked/` in its
//! data folder, because what the field leads to reads paths (repositories are read from there,
//! `repo::allow_local_files_in`).

use std::path::{Path, PathBuf};

use raygun_model::{PickTarget, PickedPath};
use raygun_paths::Paths;
use tauri::AppHandle;

/// Where Android's copies of picked files and folders go.
pub fn picked_dir(paths: &Paths) -> PathBuf {
    paths.root().join("picked")
}

/// A name that's one plain path part: no separators, no `..`, nothing hidden.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn plain_name(name: &str) -> String {
    let name: String = name
        .chars()
        .map(|c| {
            if matches!(c, '/' | '\\' | ':' | '\0') {
                '_'
            } else {
                c
            }
        })
        .collect();
    let name = name.trim().trim_start_matches('.').trim();
    if name.is_empty() {
        "picked".to_owned()
    } else {
        name.chars().take(120).collect()
    }
}

/// A folder of its own for each picked item, named after where it came from.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn copy_folder(picked: &Path, source: &str) -> PathBuf {
    use sha2::{Digest, Sha256};
    let digest = Sha256::digest(source.as_bytes());
    let hex: String = digest
        .iter()
        .take(8)
        .map(|byte| format!("{byte:02x}"))
        .collect();
    picked.join(hex)
}

/// A path inside a picked folder (`a/b.json`), as plain parts; `None` for one that leaves it.
#[cfg_attr(not(target_os = "android"), allow(dead_code))]
fn inside(relative: &str) -> Option<PathBuf> {
    let mut path = PathBuf::new();
    for part in relative.split('/') {
        if part.is_empty() || part == "." || part == ".." || part.contains('\\') {
            return None;
        }
        path.push(part);
    }
    (!path.as_os_str().is_empty()).then_some(path)
}

/// Shows the picker; `None` when nothing was picked.
pub async fn pick(
    app: &AppHandle,
    paths: &Paths,
    target: PickTarget,
) -> Result<Option<PickedPath>, String> {
    platform::pick(app, &picked_dir(paths), target).await
}

#[cfg(not(target_os = "android"))]
mod platform {
    use std::path::Path;

    use raygun_model::{PickTarget, PickedPath};
    use tauri::AppHandle;

    pub async fn pick(
        _app: &AppHandle,
        _picked: &Path,
        target: PickTarget,
    ) -> Result<Option<PickedPath>, String> {
        let dialog = rfd::AsyncFileDialog::new();
        let picked = match target {
            PickTarget::File => dialog.set_title("Choose a file").pick_file().await,
            PickTarget::Folder => dialog.set_title("Choose a folder").pick_folder().await,
        };
        Ok(picked.map(|handle| {
            let path = handle.path().to_path_buf();
            PickedPath {
                name: path.file_name().map_or_else(
                    || path.to_string_lossy().into_owned(),
                    |name| name.to_string_lossy().into_owned(),
                ),
                path: path.to_string_lossy().into_owned(),
            }
        }))
    }
}

#[cfg(target_os = "android")]
mod platform {
    use std::io::Read;
    use std::os::fd::FromRawFd;
    use std::path::{Path, PathBuf};

    use raygun_model::{PickTarget, PickedPath};
    use tauri::{AppHandle, Manager};
    use tauri_plugin_raygun_audio::NativeAudio;

    use super::{copy_folder, inside, plain_name};

    /// The most a picked file may be.
    const MAX_FILE_BYTES: u64 = 1024 * 1024 * 1024;
    /// The most files and bytes a picked folder's copy may hold.
    const MAX_FOLDER_FILES: usize = 5000;
    const MAX_FOLDER_BYTES: u64 = 512 * 1024 * 1024;

    pub async fn pick(
        app: &AppHandle,
        picked: &Path,
        target: PickTarget,
    ) -> Result<Option<PickedPath>, String> {
        let (app, picked) = (app.clone(), picked.to_owned());
        tauri::async_runtime::spawn_blocking(move || {
            let plugin = app.state::<NativeAudio<tauri::Wry>>();
            match target {
                PickTarget::File => pick_file(&plugin, &picked),
                PickTarget::Folder => pick_folder(&plugin, &picked),
            }
        })
        .await
        .map_err(|error| error.to_string())?
    }

    /// Copies a document the plugin opens into `to`, at most `limit` bytes; its size.
    fn copy(
        plugin: &NativeAudio<tauri::Wry>,
        uri: &str,
        to: &Path,
        limit: u64,
    ) -> Result<u64, String> {
        let fd = plugin.open_input(uri)?;
        // The plugin detached the descriptor for Rust, which now owns it.
        let file = unsafe { std::fs::File::from_raw_fd(fd) };
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|error| error.to_string())?;
        }
        let mut output = std::fs::File::create(to).map_err(|error| error.to_string())?;
        let copied = std::io::copy(&mut file.take(limit + 1), &mut output)
            .map_err(|error| format!("Couldn't copy the file: {error}"))?;
        if copied > limit {
            drop(output);
            let _ = std::fs::remove_file(to);
            return Err("That file is too large".to_owned());
        }
        Ok(copied)
    }

    fn pick_file(
        plugin: &NativeAudio<tauri::Wry>,
        picked: &Path,
    ) -> Result<Option<PickedPath>, String> {
        let chosen = plugin.pick_file()?;
        let Some(uri) = chosen.uri else {
            return Ok(None);
        };
        let name = plain_name(chosen.name.as_deref().unwrap_or("file"));
        let folder = copy_folder(picked, &uri);
        let _ = std::fs::remove_dir_all(&folder);
        let path = folder.join(&name);
        copy(plugin, &uri, &path, MAX_FILE_BYTES)?;
        tracing::info!(path = %path.display(), "picked file copied");
        Ok(Some(PickedPath {
            path: path.to_string_lossy().into_owned(),
            name,
        }))
    }

    fn pick_folder(
        plugin: &NativeAudio<tauri::Wry>,
        picked: &Path,
    ) -> Result<Option<PickedPath>, String> {
        let chosen = plugin.pick_folder()?;
        let Some(uri) = chosen.uri else {
            return Ok(None);
        };
        let name = plain_name(chosen.name.as_deref().unwrap_or("folder"));
        let result = (|| {
            let files = plugin.list_tree(&uri)?;
            if files.len() > MAX_FOLDER_FILES {
                return Err(format!(
                    "That folder holds more than {MAX_FOLDER_FILES} files"
                ));
            }
            let root: PathBuf = copy_folder(picked, &uri).join(&name);
            let _ = std::fs::remove_dir_all(&root);
            std::fs::create_dir_all(&root).map_err(|error| error.to_string())?;
            let mut total = 0u64;
            for file in &files {
                let Some(relative) = inside(&file.path) else {
                    continue;
                };
                total += copy(
                    plugin,
                    &file.uri,
                    &root.join(relative),
                    MAX_FOLDER_BYTES.saturating_sub(total),
                )?;
            }
            tracing::info!(path = %root.display(), files = files.len(), "picked folder copied");
            Ok(Some(PickedPath {
                path: root.to_string_lossy().into_owned(),
                name: name.clone(),
            }))
        })();
        // The copy is what's read from now on.
        let _ = plugin.release_folder(&uri);
        result
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn picked_names_and_paths_stay_inside() {
        assert_eq!(plain_name("index.json"), "index.json");
        assert_eq!(plain_name("../../secrets.db"), "_.._secrets.db");
        assert_eq!(plain_name(".hidden"), "hidden");
        assert_eq!(plain_name("  "), "picked");
        assert_eq!(
            inside("packages/a.zip"),
            Some(PathBuf::from("packages").join("a.zip"))
        );
        assert_eq!(inside("../a.zip"), None);
        assert_eq!(inside("a//b"), None);
        assert_eq!(inside(""), None);
        let root = Path::new("picked");
        assert_eq!(
            copy_folder(root, "content://a"),
            copy_folder(root, "content://a")
        );
        assert_ne!(
            copy_folder(root, "content://a"),
            copy_folder(root, "content://b")
        );
    }
}
