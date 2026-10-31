//! The extension host: loads extensions and routes provider calls to them.
//!
//! All extensions live on one dedicated thread running a current-thread tokio runtime and a
//! `LocalSet`. Requests arrive over a channel and each runs as a local task, so one extension
//! waiting on the network doesn't block another. Calls into the *same* extension are serialized
//! by its runtime lock (a provider can still run its own requests in parallel with `Promise.all`).

use std::cell::RefCell;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::time::Duration;

use raygun_model::{
    ExtensionInfo, ExtensionOrigin, ExtensionSetting, ExtensionStatus, MediaSource, OnlineTrack,
    ProviderInfo, ProviderSearchResult, SettingKind, SettingValue,
};
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::js::{JsExtension, Limits};
use crate::manifest::{self, MANIFEST_FILE, Manifest};
use crate::storage::KvStore;

pub const MUSIC_METADATA: &str = "music.metadata";

/// The user's setting values, per extension id, then per setting key.
pub type UserSettings = std::collections::HashMap<String, BTreeMap<String, serde_json::Value>>;

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Where per-extension storage lives (`extensions/.data`); `None` keeps storage in memory.
    pub storage_dir: Option<PathBuf>,
    pub limits: Limits,
    /// Longest a provider call may take, waiting included.
    pub call_timeout: Duration,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            storage_dir: None,
            limits: Limits::default(),
            call_timeout: Duration::from_secs(30),
        }
    }
}

/// An extension's files: compiled in, or a folder under `extensions/`.
#[derive(Debug, Clone)]
pub enum ExtensionFiles {
    Memory(BTreeMap<String, String>),
    Dir(PathBuf),
}

#[derive(Debug, Clone)]
pub struct ExtensionSource {
    pub origin: ExtensionOrigin,
    pub files: ExtensionFiles,
}

impl ExtensionSource {
    pub fn built_in(files: &[(&str, &str)]) -> Self {
        Self {
            origin: ExtensionOrigin::BuiltIn,
            files: ExtensionFiles::Memory(
                files
                    .iter()
                    .map(|(path, text)| ((*path).to_owned(), (*text).to_owned()))
                    .collect(),
            ),
        }
    }

    pub fn installed(dir: PathBuf) -> Self {
        Self {
            origin: ExtensionOrigin::Installed,
            files: ExtensionFiles::Dir(dir),
        }
    }

    fn read(&self, path: &str) -> Result<String, String> {
        match &self.files {
            ExtensionFiles::Memory(files) => files
                .get(path)
                .cloned()
                .ok_or_else(|| format!("{path} is missing")),
            ExtensionFiles::Dir(dir) => std::fs::read_to_string(dir.join(path))
                .map_err(|error| format!("could not read {path}: {error}")),
        }
    }

    /// A name for errors when the manifest can't be read.
    fn label(&self) -> String {
        match &self.files {
            ExtensionFiles::Memory(_) => "built-in extension".to_owned(),
            ExtensionFiles::Dir(dir) => dir
                .file_name()
                .map(|name| name.to_string_lossy().into_owned())
                .unwrap_or_else(|| dir.display().to_string()),
        }
    }
}

/// Every folder in `dir` holding a manifest. Folders starting with `.` (such as `.data`) are not
/// extensions.
pub fn discover_installed(dir: &Path) -> Vec<ExtensionSource> {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut dirs: Vec<PathBuf> = entries
        .filter_map(Result::ok)
        .filter(|entry| !entry.file_name().to_string_lossy().starts_with('.'))
        .map(|entry| entry.path())
        .filter(|path| path.join(MANIFEST_FILE).is_file())
        .collect();
    dirs.sort();
    dirs.into_iter().map(ExtensionSource::installed).collect()
}

#[derive(Debug, Clone, thiserror::Error, PartialEq, Eq)]
pub enum CallError {
    #[error("no provider {0}")]
    UnknownProvider(String),
    #[error("{0} did not answer in time")]
    Timeout(String),
    #[error("{0}")]
    Script(String),
    #[error("the extension host has stopped")]
    HostStopped,
}

enum Request {
    Load {
        sources: Vec<ExtensionSource>,
        settings: UserSettings,
        reply: oneshot::Sender<Vec<ExtensionInfo>>,
    },
    List {
        reply: oneshot::Sender<Vec<ExtensionInfo>>,
    },
    Call {
        provider: String,
        method: String,
        args: serde_json::Value,
        reply: oneshot::Sender<Result<serde_json::Value, CallError>>,
    },
}

/// Handle to the host thread; cheap to clone and usable from any thread or runtime.
#[derive(Clone)]
pub struct ExtensionHost {
    requests: mpsc::UnboundedSender<Request>,
}

impl ExtensionHost {
    pub fn start(config: HostConfig) -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::unbounded_channel();
        std::thread::Builder::new()
            .name("raygun-ext".to_owned())
            .spawn(move || run(config, receiver))?;
        Ok(Self { requests })
    }

    /// Replaces every loaded extension with `sources`. An installed extension overrides a
    /// built-in one with the same id. Returns what loaded (or failed), sorted by name.
    pub async fn load(&self, sources: Vec<ExtensionSource>) -> Vec<ExtensionInfo> {
        self.load_with_settings(sources, UserSettings::new()).await
    }

    /// Like [`Self::load`], with the user's setting values (invalid ones are ignored).
    pub async fn load_with_settings(
        &self,
        sources: Vec<ExtensionSource>,
        settings: UserSettings,
    ) -> Vec<ExtensionInfo> {
        self.ask(|reply| Request::Load {
            sources,
            settings,
            reply,
        })
        .await
        .unwrap_or_default()
    }

    pub async fn list(&self) -> Vec<ExtensionInfo> {
        self.ask(|reply| Request::List { reply })
            .await
            .unwrap_or_default()
    }

    pub async fn providers(&self, kind: &str) -> Vec<ProviderInfo> {
        self.list()
            .await
            .into_iter()
            .flat_map(|extension| extension.providers)
            .filter(|provider| provider.kind == kind)
            .collect()
    }

    /// Calls `method` on a provider (its full `<extension>/<provider>` id), JSON in and out.
    pub async fn call(
        &self,
        provider: &str,
        method: &str,
        args: serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        let (provider, method) = (provider.to_owned(), method.to_owned());
        self.ask(|reply| Request::Call {
            provider,
            method,
            args,
            reply,
        })
        .await
        .unwrap_or(Err(CallError::HostStopped))
    }

    /// Asks every `music.metadata` provider at once; each answers with tracks or an error.
    pub async fn search_music(&self, query: &str, limit: u32) -> Vec<ProviderSearchResult> {
        let providers = self.providers(MUSIC_METADATA).await;
        let args = serde_json::json!({ "query": query, "limit": limit });
        let searches = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let started = std::time::Instant::now();
                match self.call(&provider.id, "search", args).await {
                    Ok(value) => {
                        let tracks = parse_tracks(value);
                        tracing::info!(
                            provider = %provider.id,
                            tracks = tracks.len(),
                            elapsed_ms = started.elapsed().as_millis(),
                            "search answered"
                        );
                        ProviderSearchResult {
                            tracks,
                            provider,
                            error: None,
                        }
                    }
                    Err(error) => {
                        tracing::warn!(provider = %provider.id, %error, "search failed");
                        ProviderSearchResult {
                            provider,
                            tracks: Vec::new(),
                            error: Some(error.to_string()),
                        }
                    }
                }
            }
        });
        futures_util::future::join_all(searches).await
    }

    async fn ask<T>(&self, request: impl FnOnce(oneshot::Sender<T>) -> Request) -> Option<T> {
        let (reply, answer) = oneshot::channel();
        self.requests.send(request(reply)).ok()?;
        answer.await.ok()
    }
}

struct Loaded {
    extension: Rc<JsExtension>,
}

struct HostState {
    config: HostConfig,
    loaded: RefCell<BTreeMap<String, Loaded>>,
    infos: RefCell<Vec<ExtensionInfo>>,
}

fn run(config: HostConfig, mut receiver: mpsc::UnboundedReceiver<Request>) {
    let runtime = match tokio::runtime::Builder::new_current_thread()
        .enable_all()
        .build()
    {
        Ok(runtime) => runtime,
        Err(error) => {
            tracing::error!(%error, "extension host could not start");
            return;
        }
    };
    let local = tokio::task::LocalSet::new();
    local.block_on(&runtime, async move {
        let state = Rc::new(HostState {
            config,
            loaded: RefCell::new(BTreeMap::new()),
            infos: RefCell::new(Vec::new()),
        });
        while let Some(request) = receiver.recv().await {
            let state = state.clone();
            tokio::task::spawn_local(async move { state.handle(request).await });
        }
    });
}

impl HostState {
    async fn handle(&self, request: Request) {
        match request {
            Request::Load {
                sources,
                settings,
                reply,
            } => {
                let infos = self.load(sources, settings).await;
                let _ = reply.send(infos);
            }
            Request::List { reply } => {
                let _ = reply.send(self.infos.borrow().clone());
            }
            Request::Call {
                provider,
                method,
                args,
                reply,
            } => {
                let _ = reply.send(self.call(&provider, &method, &args).await);
            }
        }
    }

    async fn load(
        &self,
        sources: Vec<ExtensionSource>,
        mut settings: UserSettings,
    ) -> Vec<ExtensionInfo> {
        // Parse every manifest first, so installed copies can override built-in ones.
        let mut chosen: BTreeMap<String, (ExtensionSource, Manifest)> = BTreeMap::new();
        let mut infos = Vec::new();
        for source in sources {
            let manifest = source
                .read(MANIFEST_FILE)
                .and_then(|text| Manifest::parse(&text).map_err(|error| error.to_string()));
            match manifest {
                Ok(manifest) => {
                    let replace = chosen.get(&manifest.id).is_none_or(|(existing, _)| {
                        source.origin == ExtensionOrigin::Installed
                            || existing.origin == ExtensionOrigin::BuiltIn
                    });
                    if replace {
                        chosen.insert(manifest.id.clone(), (source, manifest));
                    }
                }
                Err(error) => {
                    let label = source.label();
                    tracing::warn!(extension = %label, %error, "extension manifest rejected");
                    infos.push(failed_info(&label, &label, "", source.origin, error));
                }
            }
        }

        self.loaded.borrow_mut().clear();
        for (id, (source, manifest)) in chosen {
            let origin = source.origin;
            let (name, version) = (manifest.name.clone(), manifest.version.to_string());
            let user_settings = settings.remove(&id).unwrap_or_default();
            match self.load_one(source, manifest, user_settings).await {
                Ok(extension) => {
                    let info = loaded_info(&extension, origin);
                    tracing::info!(extension = %id, providers = info.providers.len(), "extension loaded");
                    infos.push(info);
                    self.loaded.borrow_mut().insert(
                        id,
                        Loaded {
                            extension: Rc::new(extension),
                        },
                    );
                }
                Err(error) => {
                    tracing::warn!(extension = %id, %error, "extension failed to load");
                    infos.push(failed_info(&id, &name, &version, origin, error));
                }
            }
        }
        infos.sort_by_key(|info| info.name.to_lowercase());
        *self.infos.borrow_mut() = infos.clone();
        infos
    }

    async fn load_one(
        &self,
        source: ExtensionSource,
        manifest: Manifest,
        mut user_settings: BTreeMap<String, serde_json::Value>,
    ) -> Result<JsExtension, String> {
        user_settings.retain(|key, value| manifest.check_setting(key, value).is_ok());
        let code = source.read(&manifest.entry)?;
        let storage = KvStore::open(
            self.config
                .storage_dir
                .as_ref()
                .map(|dir| dir.join(&manifest.id).join("storage.json")),
        );
        let load = JsExtension::load(manifest, code, storage, user_settings, &self.config.limits);
        tokio::time::timeout(self.config.call_timeout, load)
            .await
            .map_err(|_| "loading timed out".to_owned())?
    }

    async fn call(
        &self,
        provider: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, CallError> {
        let unknown = || CallError::UnknownProvider(provider.to_owned());
        let (extension_id, local_id) = provider.split_once('/').ok_or_else(unknown)?;
        let extension = self
            .loaded
            .borrow()
            .get(extension_id)
            .map(|loaded| loaded.extension.clone())
            .ok_or_else(unknown)?;
        if !extension
            .providers()
            .iter()
            .any(|registered| registered.id == provider)
        {
            return Err(unknown());
        }
        tokio::time::timeout(
            self.config.call_timeout,
            extension.call(local_id, method, args),
        )
        .await
        .map_err(|_| CallError::Timeout(provider.to_owned()))?
        .map_err(CallError::Script)
    }
}

fn loaded_info(extension: &JsExtension, origin: ExtensionOrigin) -> ExtensionInfo {
    let manifest = extension.manifest();
    let values = extension.user_settings();
    ExtensionInfo {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        description: manifest.description.clone(),
        origin,
        status: ExtensionStatus::Loaded,
        error: None,
        provides: manifest.provides.clone(),
        net: manifest
            .net
            .patterns()
            .iter()
            .map(ToString::to_string)
            .collect(),
        providers: extension.providers(),
        settings: manifest
            .settings
            .iter()
            .map(|setting| {
                let default = setting.default.as_ref().and_then(SettingValue::from_json);
                ExtensionSetting {
                    key: setting.key.clone(),
                    label: setting.label.clone().unwrap_or_else(|| setting.key.clone()),
                    kind: match setting.kind {
                        manifest::SettingKind::Text => SettingKind::Text,
                        manifest::SettingKind::Number => SettingKind::Number,
                        manifest::SettingKind::Boolean => SettingKind::Boolean,
                        manifest::SettingKind::Select => SettingKind::Select,
                    },
                    options: setting.options.clone(),
                    value: values
                        .get(&setting.key)
                        .and_then(SettingValue::from_json)
                        .or_else(|| default.clone()),
                    default,
                }
            })
            .collect(),
    }
}

fn failed_info(
    id: &str,
    name: &str,
    version: &str,
    origin: ExtensionOrigin,
    error: String,
) -> ExtensionInfo {
    ExtensionInfo {
        id: id.to_owned(),
        name: name.to_owned(),
        version: version.to_owned(),
        description: None,
        origin,
        status: ExtensionStatus::Failed,
        error: Some(error),
        provides: Vec::new(),
        net: Vec::new(),
        providers: Vec::new(),
        settings: Vec::new(),
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawTrack {
    #[serde(default)]
    id: Option<serde_json::Value>,
    title: String,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    album: Option<String>,
    #[serde(default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    artwork_url: Option<String>,
    #[serde(default)]
    source: Option<MediaSource>,
}

fn is_web_url(url: &str) -> bool {
    reqwest::Url::parse(url).is_ok_and(|url| matches!(url.scheme(), "http" | "https"))
}

/// Reads `{ tracks: [...] }` leniently: malformed entries are skipped, not fatal. Extensions may
/// only hand out web streams and YouTube ids, never local files.
fn parse_tracks(value: serde_json::Value) -> Vec<OnlineTrack> {
    let Some(tracks) = value.get("tracks").and_then(serde_json::Value::as_array) else {
        return Vec::new();
    };
    tracks
        .iter()
        .filter_map(|track| serde_json::from_value::<RawTrack>(track.clone()).ok())
        .filter(|track| !track.title.trim().is_empty())
        .map(|track| OnlineTrack {
            external_id: track.id.map(|id| match id {
                serde_json::Value::String(id) => id,
                other => other.to_string(),
            }),
            title: track.title.trim().to_owned(),
            artist: track.artist.filter(|artist| !artist.trim().is_empty()),
            album: track.album.filter(|album| !album.trim().is_empty()),
            duration_ms: track
                .duration_ms
                .filter(|ms| ms.is_finite() && *ms > 0.0 && *ms < f64::from(u32::MAX))
                .map(|ms| ms as u32),
            artwork_url: track.artwork_url.filter(|url| is_web_url(url)),
            source: track.source.filter(|source| match source {
                MediaSource::Url { url, .. } => is_web_url(url),
                MediaSource::YouTube { .. } => true,
                MediaSource::LocalFile { .. } => false,
            }),
        })
        .collect()
}
