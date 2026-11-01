//! Extensions: the built-in first-party extensions plus any installed under `extensions/` (M11),
//! installed from repositories with per-extension settings (M13).
//!
//! Debug builds also watch that folder and reload when it changes (M12), so `raygun-ext dev`
//! reaches the running app without a restart.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant, SystemTime};

use raygun_db::extensions::{self as store, InstallRow};
use raygun_ext_host::nuclear;
use raygun_ext_host::package::{self, MAX_PACKAGE_BYTES};
use raygun_ext_host::repo::{self, PackageFormat, RepoEntry, RepoIndex};
use raygun_ext_host::shosetsu;
use raygun_ext_host::{
    ExtensionHost, ExtensionSource, HostConfig, ProviderCandidates, ResolvedStream, UserSettings,
    discover_installed,
};
use raygun_model::{
    CatalogEntry, CatalogState, ExtensionInfo, ExtensionOrigin, ExtensionRepo, ExtensionSetting,
    Lyrics, ProviderSearchResult, RepoCatalog, SettingKind, SettingValue, TrackQuery,
};
use raygun_paths::Paths;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::RwLock;

use crate::events::ExtensionsChangedEvent;
use crate::startup::AppDb;

const SEARCH_LIMIT: u32 = 30;
/// Candidates asked of each `music.stream` provider per track.
const CANDIDATE_LIMIT: u32 = 10;
const WATCH_INTERVAL: Duration = Duration::from_secs(1);
const INDEX_CACHE: Duration = Duration::from_secs(10 * 60);
const MAX_INDEX_BYTES: u64 = 2 * 1024 * 1024;
const REQUEST_TIMEOUT: Duration = Duration::from_secs(30);

/// Extensions compiled into the app; an installed copy with the same id overrides one.
fn built_in() -> Vec<ExtensionSource> {
    vec![
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/internet-archive/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/internet-archive/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/musicbrainz/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/musicbrainz/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/lrclib/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/lrclib/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/mangadex/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/mangadex/dist/index.js"),
            ),
        ]),
        ExtensionSource::built_in(&[
            (
                "raygun-extension.json",
                include_str!("../../../extensions/gutenberg/raygun-extension.json"),
            ),
            (
                "dist/index.js",
                include_str!("../../../extensions/gutenberg/dist/index.js"),
            ),
        ]),
    ]
}

type Fingerprint = Vec<(PathBuf, u64, Option<SystemTime>)>;

pub struct ExtensionService {
    app: AppHandle,
    host: ExtensionHost,
    http: reqwest::Client,
    extensions_dir: PathBuf,
    storage_dir: PathBuf,
    /// Held for writing while extensions (re)load, so calls wait instead of seeing a partial set.
    loading: Arc<RwLock<()>>,
    /// One install or uninstall at a time.
    changing: tokio::sync::Mutex<()>,
    /// The installed files as of the last load, so the dev watcher skips the app's own changes.
    loaded_files: Mutex<Fingerprint>,
    indexes: Mutex<HashMap<String, (Instant, RepoIndex)>>,
}

impl ExtensionService {
    /// Starts the host and loads extensions in the background.
    pub fn start(app: AppHandle, paths: &Paths) -> Result<Arc<Self>, Box<dyn std::error::Error>> {
        let extensions_dir = paths.extensions();
        let storage_dir = extensions_dir.join(".data");
        let host = ExtensionHost::start(HostConfig {
            storage_dir: Some(storage_dir.clone()),
            ..HostConfig::default()
        })?;
        let service = Arc::new(Self {
            app,
            host,
            http: raygun_net::http_client()?,
            extensions_dir,
            storage_dir,
            loading: Arc::new(RwLock::new(())),
            changing: tokio::sync::Mutex::new(()),
            loaded_files: Mutex::new(Vec::new()),
            indexes: Mutex::new(HashMap::new()),
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

    fn db(&self) -> &raygun_db::Db {
        &self.app.state::<AppDb>().inner().0
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

    /// Stream candidates for a track from every `music.stream` provider (M14).
    pub async fn stream_candidates(&self, query: &TrackQuery) -> Vec<ProviderCandidates> {
        let _ready = self.loading.read().await;
        self.host.stream_candidates(query, CANDIDATE_LIMIT).await
    }

    /// Lyrics for a track from the `lyrics` providers (M15).
    /// The host once any (re)load finished; keep the guard while calling it.
    pub async fn ready(&self) -> (tokio::sync::RwLockReadGuard<'_, ()>, &ExtensionHost) {
        (self.loading.read().await, &self.host)
    }

    pub async fn lyrics(&self, query: &TrackQuery) -> Result<Option<Lyrics>, String> {
        let _ready = self.loading.read().await;
        self.host.lyrics(query).await
    }

    /// The stream behind a candidate that came without one.
    pub async fn resolve_stream(
        &self,
        provider_id: &str,
        candidate_id: &str,
    ) -> Result<ResolvedStream, String> {
        let _ready = self.loading.read().await;
        self.host
            .resolve_stream(provider_id, candidate_id)
            .await
            .map_err(|error| error.to_string())
    }

    /// Reloads every extension from scratch (built-in and installed).
    pub async fn reload(&self) -> Vec<ExtensionInfo> {
        let _loading = self.loading.write().await;
        self.load_all().await
    }

    async fn load_all(&self) -> Vec<ExtensionInfo> {
        let dir = self.extensions_dir.clone();
        let files = tauri::async_runtime::spawn_blocking(move || fingerprint(&dir))
            .await
            .unwrap_or_default();
        *self.loaded_files.lock().unwrap() = files;

        let mut sources = built_in();
        sources.extend(discover_installed(&self.extensions_dir));
        let settings = self.user_settings().await;
        let infos = self.host.load_with_settings(sources, settings).await;
        tracing::info!(count = infos.len(), "extensions ready");
        if let Err(error) = ExtensionsChangedEvent(infos.clone()).emit(&self.app) {
            tracing::warn!(%error, "could not announce extension changes");
        }
        infos
    }

    async fn user_settings(&self) -> UserSettings {
        let rows = store::settings(self.db()).await.unwrap_or_else(|error| {
            tracing::warn!(%error, "could not read extension settings");
            Vec::new()
        });
        let mut settings = UserSettings::new();
        for (extension_id, key, json) in rows {
            if let Ok(value) = serde_json::from_str(&json) {
                settings.entry(extension_id).or_default().insert(key, value);
            }
        }
        settings
    }

    /// Reloads whenever the installed files change. Polling (rather than OS file events) works the
    /// same on every platform, including files pushed into the Android sandbox over adb.
    async fn watch(&self) {
        loop {
            tokio::time::sleep(WATCH_INTERVAL).await;
            let dir = self.extensions_dir.clone();
            let current = tauri::async_runtime::spawn_blocking(move || fingerprint(&dir))
                .await
                .unwrap_or_default();
            if *self.loaded_files.lock().unwrap() != current {
                tracing::info!("installed extensions changed, reloading");
                self.reload().await;
            }
        }
    }

    // ---- Repositories ----

    pub async fn repos(&self) -> Result<Vec<ExtensionRepo>, String> {
        let rows = store::repos(self.db())
            .await
            .map_err(|error| error.to_string())?;
        Ok(rows.into_iter().map(to_repo).collect())
    }

    /// Adds a repository after checking that its index can be read.
    pub async fn add_repo(&self, url: &str) -> Result<Vec<ExtensionRepo>, String> {
        let url = repo::check_index_url(url).map_err(|error| error.to_string())?;
        let index = self.fetch_index(url.as_str(), true).await?;
        store::add_repo(self.db(), url.as_str(), &index.name)
            .await
            .map_err(|error| error.to_string())?;
        self.repos().await
    }

    pub async fn remove_repo(&self, id: &str) -> Result<Vec<ExtensionRepo>, String> {
        store::remove_repo(self.db(), id)
            .await
            .map_err(|error| error.to_string())?;
        self.repos().await
    }

    /// Every repository's extensions, marked against what this device has.
    pub async fn catalog(&self, refresh: bool) -> Result<Vec<RepoCatalog>, String> {
        let repos = self.repos().await?;
        let local: HashMap<String, String> = self
            .list()
            .await
            .into_iter()
            .map(|info| (info.id, info.version))
            .collect();
        let listings = repos.into_iter().map(|repo| {
            let local = &local;
            async move {
                match self.fetch_index(&repo.url, refresh).await {
                    Ok(index) => {
                        if !index.name.is_empty() && index.name != repo.name {
                            let _ = store::rename_repo(self.db(), &repo.id, &index.name).await;
                        }
                        let repo = ExtensionRepo {
                            name: if index.name.is_empty() {
                                repo.name
                            } else {
                                index.name.clone()
                            },
                            ..repo
                        };
                        let entries = index
                            .extensions
                            .iter()
                            .map(|entry| catalog_entry(&repo.id, entry, local.get(&entry.id)))
                            .collect();
                        RepoCatalog {
                            repo,
                            entries,
                            error: None,
                        }
                    }
                    Err(error) => RepoCatalog {
                        repo,
                        entries: Vec::new(),
                        error: Some(error),
                    },
                }
            }
        });
        Ok(futures_util::future::join_all(listings).await)
    }

    async fn fetch_index(&self, url: &str, refresh: bool) -> Result<RepoIndex, String> {
        if !refresh
            && let Some((fetched, index)) = self.indexes.lock().unwrap().get(url)
            && fetched.elapsed() < INDEX_CACHE
        {
            return Ok(index.clone());
        }
        let bytes = self.download(url, MAX_INDEX_BYTES).await?;
        let index = repo::parse_index(&String::from_utf8_lossy(&bytes))
            .map_err(|error| error.to_string())?;
        self.indexes
            .lock()
            .unwrap()
            .insert(url.to_owned(), (Instant::now(), index.clone()));
        Ok(index)
    }

    async fn download(&self, url: &str, limit: u64) -> Result<Vec<u8>, String> {
        let mut response = self
            .http
            .get(url)
            .timeout(REQUEST_TIMEOUT)
            .send()
            .await
            .map_err(|error| format!("could not reach {url}: {error}"))?;
        if !response.status().is_success() {
            return Err(format!(
                "{url} answered HTTP {}",
                response.status().as_u16()
            ));
        }
        if response
            .content_length()
            .is_some_and(|length| length > limit)
        {
            return Err(format!("{url} is larger than {limit} bytes"));
        }
        let mut bytes = Vec::new();
        while let Some(chunk) = response
            .chunk()
            .await
            .map_err(|error| format!("download from {url} failed: {error}"))?
        {
            if bytes.len() as u64 + chunk.len() as u64 > limit {
                return Err(format!("{url} is larger than {limit} bytes"));
            }
            bytes.extend_from_slice(&chunk);
        }
        Ok(bytes)
    }

    // ---- Install, update, uninstall ----

    /// Installs (or updates to) the version a repository currently lists.
    pub async fn install(
        &self,
        repo_id: &str,
        extension_id: &str,
    ) -> Result<Vec<ExtensionInfo>, String> {
        let _changing = self.changing.lock().await;
        let repo = self
            .repos()
            .await?
            .into_iter()
            .find(|repo| repo.id == repo_id)
            .ok_or("that repository was removed")?;
        // Always the current index: a cached one could name a package that has since changed.
        let index = self.fetch_index(&repo.url, true).await?;
        let entry = index
            .extensions
            .iter()
            .find(|entry| entry.id == extension_id)
            .ok_or_else(|| format!("{} no longer lists {extension_id}", repo.name))?;
        let index_url: reqwest::Url = repo.url.parse().map_err(|_| "invalid repository URL")?;
        let package_url =
            repo::package_url(&index_url, entry).map_err(|error| error.to_string())?;
        let limit = if entry.size == 0 {
            MAX_PACKAGE_BYTES
        } else {
            entry.size.min(MAX_PACKAGE_BYTES)
        };
        let bytes = self.download(package_url.as_str(), limit).await?;

        let verified = match entry.format {
            PackageFormat::Raygun => {
                package::verify(&bytes, entry).map_err(|error| error.to_string())?
            }
            PackageFormat::Nuclear => {
                package::check_nuclear_download(&bytes, entry)
                    .map_err(|error| error.to_string())?;
                let sources = nuclear::unzip_sources(&bytes).map_err(|error| error.to_string())?;
                let converted =
                    tauri::async_runtime::spawn_blocking(move || nuclear::convert(&sources))
                        .await
                        .map_err(|error| error.to_string())??;
                let verified = package::from_converted(&converted.manifest, &converted.bundle)
                    .map_err(|error| error.to_string())?;
                if verified.manifest.id != entry.id {
                    return Err(format!(
                        "the plugin's package name gives {} rather than {}",
                        verified.manifest.id, entry.id
                    ));
                }
                tracing::info!(extension = %entry.id, hosts = ?converted.report.hosts, "Nuclear plugin converted");
                verified
            }
            PackageFormat::Shosetsu => {
                let script = shosetsu::package::verify_script(&bytes, entry)
                    .map_err(|error| error.to_string())?;
                // The libraries it needs, and theirs, each checked against the index.
                let mut queue = shosetsu::package::dependencies(&script);
                let mut libraries: Vec<(String, String)> = Vec::new();
                while let Some(name) = queue.pop() {
                    if libraries.iter().any(|(known, _)| *known == name) {
                        continue;
                    }
                    if libraries.len() >= shosetsu::package::MAX_LIBRARIES
                        || !shosetsu::package::is_library_name(&name)
                    {
                        return Err(format!("{} needs an unusable library {name}", entry.name));
                    }
                    let url = index_url
                        .join(&format!("lib/{name}.lua"))
                        .map_err(|error| error.to_string())?;
                    let bytes = self.download(url.as_str(), MAX_PACKAGE_BYTES).await?;
                    let code = shosetsu::package::verify_library(&name, &bytes, &index)
                        .map_err(|error| format!("library {name}: {error}"))?;
                    queue.extend(shosetsu::package::dependencies(&code));
                    libraries.push((name, code));
                }
                tracing::info!(extension = %entry.id, libraries = libraries.len(), "Shosetsu script fetched");
                shosetsu::package::package(entry, script, libraries)
                    .map_err(|error| error.to_string())?
            }
        };
        let extensions_dir = self.extensions_dir.clone();
        tauri::async_runtime::spawn_blocking(move || package::install(&extensions_dir, &verified))
            .await
            .map_err(|error| error.to_string())?
            .map_err(|error| error.to_string())?;
        store::record_install(
            self.db(),
            &InstallRow {
                extension_id: entry.id.clone(),
                repo_url: repo.url.clone(),
                version: entry.version.clone(),
                sha256: entry.sha256.to_ascii_lowercase(),
                installed_at: unix_ms(),
            },
        )
        .await
        .map_err(|error| error.to_string())?;
        tracing::info!(extension = %entry.id, version = %entry.version, repo = %repo.url, "extension installed");
        Ok(self.reload().await)
    }

    /// Removes an installed extension and its storage. A built-in one it overrode comes back.
    pub async fn uninstall(&self, extension_id: &str) -> Result<Vec<ExtensionInfo>, String> {
        let _changing = self.changing.lock().await;
        let installed = self
            .list()
            .await
            .into_iter()
            .any(|info| info.id == extension_id && info.origin == ExtensionOrigin::Installed);
        if !installed {
            return Err(format!("{extension_id} is not an installed extension"));
        }
        let (extensions_dir, storage_dir, id) = (
            self.extensions_dir.clone(),
            self.storage_dir.clone(),
            extension_id.to_owned(),
        );
        tauri::async_runtime::spawn_blocking(move || {
            package::uninstall(&extensions_dir, &storage_dir, &id)
        })
        .await
        .map_err(|error| error.to_string())?
        .map_err(|error| format!("could not uninstall: {error}"))?;
        store::remove_install(self.db(), extension_id)
            .await
            .map_err(|error| error.to_string())?;
        tracing::info!(extension = %extension_id, "extension uninstalled");
        Ok(self.reload().await)
    }

    // ---- Settings ----

    pub async fn set_setting(
        &self,
        extension_id: &str,
        key: &str,
        value: SettingValue,
    ) -> Result<Vec<ExtensionInfo>, String> {
        let infos = self.list().await;
        let setting = infos
            .iter()
            .find(|info| info.id == extension_id)
            .and_then(|info| info.settings.iter().find(|setting| setting.key == key))
            .ok_or_else(|| format!("{extension_id} has no setting {key:?}"))?;
        if !accepts(setting, &value) {
            return Err(format!("not a valid value for {}", setting.label));
        }
        store::put_setting(self.db(), extension_id, key, &value.to_json().to_string())
            .await
            .map_err(|error| error.to_string())?;
        Ok(self.reload().await)
    }
}

fn to_repo(row: store::RepoRow) -> ExtensionRepo {
    ExtensionRepo {
        id: row.id,
        url: row.url,
        name: row.name,
    }
}

fn catalog_entry(repo_id: &str, entry: &RepoEntry, local: Option<&String>) -> CatalogEntry {
    let state = match local {
        None => CatalogState::Available,
        Some(version) => match repo::compare_versions(&entry.version, version) {
            std::cmp::Ordering::Greater => CatalogState::UpdateAvailable,
            std::cmp::Ordering::Equal => CatalogState::Installed,
            std::cmp::Ordering::Less => CatalogState::NewerInstalled,
        },
    };
    CatalogEntry {
        repo_id: repo_id.to_owned(),
        id: entry.id.clone(),
        name: entry.name.clone(),
        version: entry.version.clone(),
        description: entry.description.clone(),
        author: entry.author.clone(),
        provides: entry.provides.clone(),
        net: entry.permissions.net.clone(),
        content_rating: entry.content_rating.clone(),
        size: u32::try_from(entry.size).unwrap_or(u32::MAX),
        nuclear: entry.format == PackageFormat::Nuclear,
        state,
        installed_version: local.cloned(),
    }
}

/// Whether `value` fits a setting: the right type, and one of the options for a select.
fn accepts(setting: &ExtensionSetting, value: &SettingValue) -> bool {
    match (setting.kind, value) {
        (SettingKind::Text, SettingValue::Text(_))
        | (SettingKind::Boolean, SettingValue::Boolean(_)) => true,
        (SettingKind::Number, SettingValue::Number(number)) => number.is_finite(),
        (SettingKind::Select, SettingValue::Text(choice)) => setting.options.contains(choice),
        _ => false,
    }
}

fn unix_ms() -> i64 {
    SystemTime::now()
        .duration_since(SystemTime::UNIX_EPOCH)
        .map_or(0, |elapsed| elapsed.as_millis() as i64)
}

/// Path, size and modification time of every file under `dir`, skipping dot-folders (storage and
/// staging copies) and `node_modules`.
pub fn fingerprint(dir: &Path) -> Fingerprint {
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

    fn entry(version: &str) -> RepoEntry {
        RepoEntry {
            id: "dev.example.a".into(),
            name: "A".into(),
            version: version.into(),
            api_version: 1,
            description: None,
            author: None,
            provides: vec![],
            permissions: Default::default(),
            content_rating: "safe".into(),
            package: "a.zip".into(),
            sha256: "0".repeat(64),
            size: 1,
            format: Default::default(),
        }
    }

    #[test]
    fn catalog_state_compares_versions() {
        let state = |repo: &str, local: Option<&str>| {
            catalog_entry("r", &entry(repo), local.map(str::to_owned).as_ref()).state
        };
        assert_eq!(state("1.0.0", None), CatalogState::Available);
        assert_eq!(state("1.0.0", Some("1.0.0")), CatalogState::Installed);
        assert_eq!(
            state("1.10.0", Some("1.9.0")),
            CatalogState::UpdateAvailable
        );
        assert_eq!(
            state("1.0.0", Some("1.0.1-dev")),
            CatalogState::NewerInstalled
        );
    }

    #[test]
    fn setting_values_must_fit_their_kind() {
        let setting = |kind, options: &[&str]| ExtensionSetting {
            key: "k".into(),
            label: "K".into(),
            kind,
            options: options.iter().map(|option| (*option).to_owned()).collect(),
            default: None,
            value: None,
        };
        let text = |value: &str| SettingValue::Text(value.into());
        assert!(accepts(&setting(SettingKind::Text, &[]), &text("x")));
        assert!(!accepts(
            &setting(SettingKind::Text, &[]),
            &SettingValue::Number(1.0)
        ));
        assert!(accepts(
            &setting(SettingKind::Number, &[]),
            &SettingValue::Number(2.5)
        ));
        assert!(!accepts(
            &setting(SettingKind::Number, &[]),
            &SettingValue::Number(f64::NAN)
        ));
        assert!(accepts(
            &setting(SettingKind::Boolean, &[]),
            &SettingValue::Boolean(true)
        ));
        assert!(accepts(
            &setting(SettingKind::Select, &["eu", "us"]),
            &text("us")
        ));
        assert!(!accepts(
            &setting(SettingKind::Select, &["eu", "us"]),
            &text("asia")
        ));
    }
}
