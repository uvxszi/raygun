//! Extensions (M11): the built-in first-party extensions plus any installed under `extensions/`.

use std::sync::Arc;

use raygun_ext_host::{ExtensionHost, ExtensionSource, HostConfig, discover_installed};
use raygun_model::{ExtensionInfo, ProviderSearchResult};
use raygun_paths::Paths;
use tokio::sync::RwLock;

const SEARCH_LIMIT: u32 = 30;

/// Extensions compiled into the app; an installed copy with the same id overrides one.
fn built_in() -> Vec<ExtensionSource> {
    vec![ExtensionSource::built_in(&[
        (
            "raygun-extension.json",
            include_str!("../../../extensions/internet-archive/raygun-extension.json"),
        ),
        (
            "index.js",
            include_str!("../../../extensions/internet-archive/index.js"),
        ),
    ])]
}

pub struct ExtensionService {
    host: ExtensionHost,
    /// Held for writing while extensions load, so early calls wait instead of seeing none.
    loading: Arc<RwLock<()>>,
}

impl ExtensionService {
    /// Starts the host and loads extensions in the background.
    pub fn start(paths: &Paths) -> std::io::Result<Self> {
        let extensions_dir = paths.extensions();
        let host = ExtensionHost::start(HostConfig {
            storage_dir: Some(extensions_dir.join(".data")),
            ..HostConfig::default()
        })?;
        let loading = Arc::new(RwLock::new(()));
        let guard = loading
            .clone()
            .try_write_owned()
            .expect("a new lock is free");
        let loader = host.clone();
        tauri::async_runtime::spawn(async move {
            let mut sources = built_in();
            sources.extend(discover_installed(&extensions_dir));
            let infos = loader.load(sources).await;
            tracing::info!(count = infos.len(), "extensions ready");
            drop(guard);
        });
        Ok(Self { host, loading })
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
}
