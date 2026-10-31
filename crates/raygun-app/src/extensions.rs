//! Extensions (M11): the built-in first-party extensions plus any installed under `extensions/`.
//!
//! Debug builds also watch that folder and reload when it changes (M12), so `raygun-ext dev`
//! reaches the running app without a restart.

use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::time::{Duration, SystemTime};

use raygun_ext_host::{ExtensionHost, ExtensionSource, HostConfig, discover_installed};
use raygun_model::{ExtensionInfo, ProviderSearchResult};
use raygun_paths::Paths;
use tauri::AppHandle;
use tauri_specta::Event;
use tokio::sync::RwLock;

use crate::events::ExtensionsChangedEvent;

const SEARCH_LIMIT: u32 = 30;
const WATCH_INTERVAL: Duration = Duration::from_secs(1);

/// Extensions compiled into the app; an installed copy with the same id overrides one.
fn built_in() -> Vec<ExtensionSource> {
    vec![ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            include_str!("../../../extensions/internet-archive/raygun-extension.json"),
        ),
        (
            "dist/index.js",
            include_str!("../../../extensions/internet-archive/dist/index.js"),
        ),
    ])]
}

pub struct ExtensionService {
    app: AppHandle,
    host: ExtensionHost,
    extensions_dir: PathBuf,
    /// Held for writing while extensions (re)load, so calls wait instead of seeing a partial set.
    loading: Arc<RwLock<()>>,
}

impl ExtensionService {
    /// Starts the host and loads extensions in the background.
    pub fn start(app: AppHandle, paths: &Paths) -> std::io::Result<Arc<Self>> {
        let extensions_dir = paths.extensions();
        let host = ExtensionHost::start(HostConfig {
            storage_dir: Some(extensions_dir.join(".data")),
            ..HostConfig::default()
        })?;
        let service = Arc::new(Self {
            app,
            host,
            extensions_dir,
            loading: Arc::new(RwLock::new(())),
        });
        // Take the lock now, before any command can run, and load in the background.
        let guard = service
            .loading
            .clone()
            .try_write_owned()
            .expect("a new lock is free");
        let loader = service.clone();
        tauri::async_runtime::spawn(async move {
            loader.load_all().await;
            drop(guard);
            if cfg!(debug_assertions) {
                loader.watch().await;
            }
        });
        Ok(service)
    }

    pub async fn list(&self) -> Vec<ExtensionInfo> {
        let _ready = self.loading.read().await;
        self.host.list().await
    }

    pub async fn search(&self, query: &str) -> Vec<ProviderSearchResult> {
        let query = query.trim();
        if query.is_empty() {
            return Vec::new();
        }
        let _ready = self.loading.read().await;
        self.host.search_music(query, SEARCH_LIMIT).await
    }

    /// Reloads every extension from scratch (built-in and installed).
    pub async fn reload(&self) -> Vec<ExtensionInfo> {
        let _loading = self.loading.write().await;
        self.load_all().await
    }

    async fn load_all(&self) -> Vec<ExtensionInfo> {
        let mut sources = built_in();
        sources.extend(discover_installed(&self.extensions_dir));
        let infos = self.host.load(sources).await;
        tracing::info!(count = infos.len(), "extensions ready");
        if let Err(error) = ExtensionsChangedEvent(infos.clone()).emit(&self.app) {
            tracing::warn!(%error, "could not announce extension changes");
        }
        infos
    }

    /// Reloads whenever the installed files change. Polling (rather than OS file events) works the
    /// same on every platform, including files pushed into the Android sandbox over adb.
    async fn watch(&self) {
        let dir = self.extensions_dir.clone();
        let mut last = fingerprint(&dir);
        loop {
            tokio::time::sleep(WATCH_INTERVAL).await;
            let dir = dir.clone();
            let current = tauri::async_runtime::spawn_blocking(move || fingerprint(&dir))
                .await
                .unwrap_or_default();
            if current != last {
                tracing::info!("installed extensions changed, reloading");
                last = current;
                self.reload().await;
            }
        }
    }
}

/// Path, size and modification time of every file under `dir`, skipping dot-folders (storage and
/// staging copies) and `node_modules`.
pub fn fingerprint(dir: &Path) -> Vec<(PathBuf, u64, Option<SystemTime>)> {
    let mut files = Vec::new();
    let mut pending = vec![dir.to_path_buf()];
    while let Some(current) = pending.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name();
            let name = name.to_string_lossy();
            if name.starts_with('.') || name == "node_modules" {
                continue;
            }
            let Ok(metadata) = entry.metadata() else {
                continue;
            };
            if metadata.is_dir() {
                pending.push(entry.path());
            } else {
                files.push((entry.path(), metadata.len(), metadata.modified().ok()));
            }
        }
    }
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fingerprint_tracks_extension_files_only() {
        let dir = tempfile::tempdir().unwrap();
        let root = dir.path();
        let empty = fingerprint(root);
        assert!(empty.is_empty());

        std::fs::create_dir_all(root.join("dev.example.a/dist")).unwrap();
        std::fs::write(root.join("dev.example.a/dist/index.js"), "1").unwrap();
        let one = fingerprint(root);
        assert_eq!(one.len(), 1);

        // Storage writes and staging copies don't count as changes.
        std::fs::create_dir_all(root.join(".data/dev.example.a")).unwrap();
        std::fs::write(root.join(".data/dev.example.a/storage.json"), "{}").unwrap();
        std::fs::create_dir_all(root.join(".staging-dev.example.a")).unwrap();
        std::fs::write(root.join(".staging-dev.example.a/x.js"), "x").unwrap();
        assert_eq!(fingerprint(root), one);

        std::fs::write(root.join("dev.example.a/dist/index.js"), "12").unwrap();
        assert_ne!(fingerprint(root), one);
    }

    #[test]
    fn built_in_extensions_parse() {
        for source in built_in() {
            let raygun_ext_host::ExtensionFiles::Memory(files) = &source.files else {
                panic!("built-in extensions live in memory");
            };
            let manifest =
                raygun_ext_host::Manifest::parse(&files["raygun-extension.json"]).unwrap();
            assert!(
                files.contains_key(&manifest.entry),
                "{} has no entry",
                manifest.id
            );
        }
    }
}
