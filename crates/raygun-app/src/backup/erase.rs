//! Erase everything (M46.51): Raygun as after a new install (data, settings, caches, downloads,
//! extensions, secrets, the webview's storage), but with its backups and the list of folders
//! that hold more. While Raygun runs, the database, the logs and the webview profile are open,
//! so the erase is asked for with a marker in the data root and done at the next start, before
//! anything opens them. Nothing outside the data root is touched.

use std::path::{Path, PathBuf};
use std::time::Duration;

use raygun_paths::StorageMode;

/// Asks the next start to erase.
pub const MARKER: &str = "erase-pending";

/// What Raygun keeps in a data root it doesn't own whole (`RAYGUN_HOME`): only these go there.
const OWN: &[&str] = &[
    "raygun.db",
    "raygun.db-wal",
    "raygun.db-shm",
    "raygun.db-journal",
    "settings.toml",
    "secrets.db",
    "secrets.db-wal",
    "secrets.db-shm",
    "secrets.db-journal",
    "extensions",
    "models",
    "bin",
    "tools",
    "cache",
    "downloads",
    "exports",
    "webview",
    "web",
    "logs",
    "proxy-port",
];

/// Paths (from the data root, with `/`) an erase keeps: the backups, the installed-mode marker
/// (else the next start asks again where data goes), and on Android what the system keeps in the
/// app's folder: its code links, the main WebView's profile (cleared through the WebView
/// itself, since this process may already hold it) and the WebView's HTTP cache.
pub fn kept() -> Vec<&'static str> {
    let mut kept = vec!["backups", raygun_paths::INSTALLED_MARKER];
    if cfg!(target_os = "android") {
        kept.extend(["lib", "code_cache", "app_webview", "cache/WebView"]);
    }
    kept
}

/// Asks for an erase at the next start.
pub fn request(root: &Path) -> std::io::Result<()> {
    std::fs::write(
        root.join(MARKER),
        "Raygun erases its data at the next start, except its backups.\n",
    )
}

/// What an erase did.
#[derive(Debug, Default, PartialEq, Eq)]
pub struct Erased {
    pub removed: usize,
    /// What couldn't be removed (held open elsewhere).
    pub failed: Vec<PathBuf>,
}

/// At start, before anything opens the data root: when an erase was asked for, removes
/// everything but what's kept. A root Raygun doesn't own whole (`RAYGUN_HOME`) loses only
/// Raygun's own entries. The marker goes whatever happened, so a later start never erases again.
pub fn finish_pending(root: &Path, mode: StorageMode) -> Option<Erased> {
    let marker = root.join(MARKER);
    if !marker.is_file() {
        return None;
    }
    let whole = mode != StorageMode::Override;
    let erased = wipe(root, whole, &kept(), Duration::from_millis(250), 12);
    let _ = std::fs::remove_file(&marker);
    Some(erased)
}

/// Removes the entries of `root` but `keep` (paths under `root`), retrying ones still held open
/// (a webview process of the last run that hasn't ended yet) `attempts` times.
fn wipe(root: &Path, whole: bool, keep: &[&str], pause: Duration, attempts: u32) -> Erased {
    let mut erased = Erased::default();
    let mut pending = doomed(root, whole, keep);
    for attempt in 0..attempts {
        pending.retain(|path| match remove(path) {
            Ok(()) => {
                erased.removed += 1;
                false
            }
            Err(_) => true,
        });
        if pending.is_empty() {
            break;
        }
        if attempt + 1 < attempts {
            std::thread::sleep(pause);
        }
    }
    erased.failed = pending;
    erased
}

/// What goes: the entries of `dir` but kept paths, and inside a folder that holds a kept path,
/// its other entries.
fn doomed(dir: &Path, whole: bool, keep: &[&str]) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for entry in entries.filter_map(Result::ok) {
        let name = entry.file_name().to_string_lossy().into_owned();
        if name == MARKER || keep.contains(&name.as_str()) {
            continue;
        }
        if !whole && !OWN.contains(&name.as_str()) {
            continue;
        }
        let inside: Vec<&str> = keep
            .iter()
            .filter_map(|kept| kept.strip_prefix(&format!("{name}/")))
            .collect();
        let is_dir = entry
            .file_type()
            .is_ok_and(|kind| kind.is_dir() && !kind.is_symlink());
        if !inside.is_empty() && is_dir {
            found.extend(doomed(&entry.path(), true, &inside));
        } else {
            found.push(entry.path());
        }
    }
    found
}

/// Removes a file, a link (never what it points to) or a folder with everything in it.
fn remove(path: &Path) -> std::io::Result<()> {
    let metadata = std::fs::symlink_metadata(path)?;
    if metadata.is_dir() {
        std::fs::remove_dir_all(path)
    } else {
        std::fs::remove_file(path)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn touch(root: &Path, path: &str) {
        let path = root.join(path);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, b"x").unwrap();
    }

    fn left(root: &Path) -> Vec<String> {
        let mut names: Vec<String> = walk(root)
            .into_iter()
            .map(|path| {
                path.strip_prefix(root)
                    .unwrap()
                    .to_string_lossy()
                    .replace('\\', "/")
            })
            .collect();
        names.sort();
        names
    }

    fn walk(dir: &Path) -> Vec<PathBuf> {
        let mut found = Vec::new();
        for entry in std::fs::read_dir(dir).unwrap().filter_map(Result::ok) {
            if entry.file_type().unwrap().is_dir() {
                found.extend(walk(&entry.path()));
            } else {
                found.push(entry.path());
            }
        }
        found
    }

    fn a_used_root(root: &Path) {
        for path in [
            "raygun.db",
            "raygun.db-wal",
            "secrets.db",
            "settings.toml",
            "extensions/dev.example/index.js",
            "models/whisper/model.bin",
            "downloads/manga/t1/c1/001.jpg",
            "cache/covers/abc",
            "webview/EBWebView/Local State",
            "web/blocklist.bin",
            "logs/raygun.2026-11-05.log",
            "proxy-port",
            "backups/raygun-20261105-120000.raygunbk",
            "backups/folders.json",
            "installed-mode",
            "notes-of-someone-else.txt",
        ] {
            touch(root, path);
        }
    }

    #[test]
    fn an_erase_waits_for_the_next_start_and_keeps_the_backups() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        a_used_root(root);
        assert_eq!(
            finish_pending(root, StorageMode::Portable),
            None,
            "none asked"
        );
        assert!(root.join("raygun.db").exists());

        request(root).unwrap();
        let erased = finish_pending(root, StorageMode::Portable).unwrap();
        assert!(erased.failed.is_empty(), "{erased:?}");
        assert_eq!(
            left(root),
            [
                "backups/folders.json",
                "backups/raygun-20261105-120000.raygunbk",
                "installed-mode",
            ]
        );
        assert!(
            !root.join(MARKER).exists(),
            "a later start doesn't erase again"
        );
        assert_eq!(finish_pending(root, StorageMode::Portable), None);
    }

    #[test]
    fn a_root_raygun_doesnt_own_whole_loses_only_raygun_entries() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        a_used_root(root);
        request(root).unwrap();
        finish_pending(root, StorageMode::Override).unwrap();
        assert_eq!(
            left(root),
            [
                "backups/folders.json",
                "backups/raygun-20261105-120000.raygunbk",
                "installed-mode",
                "notes-of-someone-else.txt",
            ]
        );
    }

    #[test]
    fn kept_paths_inside_a_folder_keep_only_themselves() {
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path();
        touch(root, "cache/WebView/index");
        touch(root, "cache/covers/abc");
        touch(root, "app_webview/Default/Cookies");
        touch(root, "shared_prefs/prefs.xml");
        let erased = wipe(
            root,
            true,
            &["app_webview", "cache/WebView"],
            Duration::ZERO,
            1,
        );
        assert_eq!(erased.removed, 2);
        assert_eq!(
            left(root),
            ["app_webview/Default/Cookies", "cache/WebView/index"]
        );
    }

    #[cfg(unix)]
    #[test]
    fn links_are_removed_without_following_them() {
        let temp = tempfile::tempdir().unwrap();
        let outside = tempfile::tempdir().unwrap();
        touch(outside.path(), "theirs.txt");
        std::os::unix::fs::symlink(outside.path(), temp.path().join("downloads")).unwrap();
        wipe(temp.path(), true, &[], Duration::ZERO, 1);
        assert!(outside.path().join("theirs.txt").exists());
        assert!(!temp.path().join("downloads").exists());
    }
}
