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
    ExtensionInfo, ExtensionOrigin, ExtensionSetting, ExtensionStatus, LyricLine, Lyrics,
    MediaSource, OnlineTrack, ProviderInfo, ProviderSearchResult, SettingKind, SettingValue,
    TrackQuery,
};
use raygun_resolver::Candidate;
use serde::Deserialize;
use tokio::sync::{mpsc, oneshot};

use crate::content::ContentFilter;
use crate::js::{JsExtension, Limits};
use crate::manifest::{self, MANIFEST_FILE, Manifest, Runtime};
use crate::shosetsu::ShosetsuExtension;
use crate::storage::KvStore;

pub const MUSIC_METADATA: &str = "music.metadata";
pub const MUSIC_STREAM: &str = "music.stream";
pub const LYRICS: &str = "lyrics";

/// One `music.stream` provider's answer: its candidates, or why it failed.
#[derive(Debug, Clone)]
pub struct ProviderCandidates {
    pub provider: ProviderInfo,
    pub result: Result<Vec<Candidate>, String>,
}

/// A stream a provider resolved a candidate to.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ResolvedStream {
    pub source: MediaSource,
    /// Unix milliseconds after which the URL stops working (signed URLs), if the provider knows.
    pub expires_at: Option<i64>,
}

/// The user's setting values, per extension id, then per setting key.
pub type UserSettings = std::collections::HashMap<String, BTreeMap<String, serde_json::Value>>;

#[derive(Debug, Clone)]
pub struct HostConfig {
    /// Where per-extension storage lives (`extensions/.data`); `None` keeps storage in memory.
    pub storage_dir: Option<PathBuf>,
    pub limits: Limits,
    /// Longest a provider call may take, waiting included.
    pub call_timeout: Duration,
    /// The NSFW switch (M35), shared with the app.
    pub content: ContentFilter,
}

impl Default for HostConfig {
    fn default() -> Self {
        Self {
            storage_dir: None,
            limits: Limits::default(),
            call_timeout: Duration::from_secs(30),
            content: ContentFilter::default(),
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

    pub(crate) fn read(&self, path: &str) -> Result<String, String> {
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
    content: ContentFilter,
}

impl ExtensionHost {
    pub fn start(config: HostConfig) -> std::io::Result<Self> {
        let (requests, receiver) = mpsc::unbounded_channel();
        let content = config.content.clone();
        std::thread::Builder::new()
            .name("raygun-ext".to_owned())
            .spawn(move || run(config, receiver))?;
        Ok(Self { requests, content })
    }

    /// The NSFW switch: off hides providers of `nsfw`-rated extensions and tells extensions to
    /// filter their results.
    pub fn content(&self) -> &ContentFilter {
        &self.content
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

    /// Providers of `kind`; those of NSFW-rated extensions only while NSFW content is shown.
    pub async fn providers(&self, kind: &str) -> Vec<ProviderInfo> {
        let show_nsfw = self.content.show_nsfw();
        self.list()
            .await
            .into_iter()
            .filter(|extension| show_nsfw || extension.content_rating != "nsfw")
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

    /// Asks every `music.stream` provider at once for candidates for `query`.
    pub async fn stream_candidates(
        &self,
        query: &TrackQuery,
        limit: u32,
    ) -> Vec<ProviderCandidates> {
        let providers = self.providers(MUSIC_STREAM).await;
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
            "limit": limit,
        });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let result = self
                    .call(&provider.id, "candidates", args)
                    .await
                    .map(|value| parse_candidates(&provider, value))
                    .map_err(|error| error.to_string());
                ProviderCandidates { provider, result }
            }
        });
        futures_util::future::join_all(requests).await
    }

    /// Lyrics for `query` from every `lyrics` provider at once: synced lyrics beat plain ones,
    /// then providers keep their order. `Ok(None)` when no provider has any; an error only when
    /// every provider failed.
    pub async fn lyrics(&self, query: &TrackQuery) -> Result<Option<Lyrics>, String> {
        let providers = self.providers(LYRICS).await;
        if providers.is_empty() {
            return Ok(None);
        }
        let args = serde_json::json!({
            "title": query.title,
            "artist": query.artist,
            "album": query.album,
            "durationMs": query.duration_ms,
        });
        let requests = providers.into_iter().map(|provider| {
            let args = args.clone();
            async move {
                let answer = self.call(&provider.id, "lyrics", args).await;
                (provider, answer)
            }
        });
        let answers = futures_util::future::join_all(requests).await;
        let mut found: Vec<Lyrics> = Vec::new();
        let mut errors = Vec::new();
        for (provider, answer) in answers {
            match answer {
                Ok(value) => found.extend(parse_lyrics(&provider, &value)),
                Err(error) => {
                    tracing::warn!(provider = %provider.id, %error, "lyrics provider failed");
                    errors.push(error.to_string());
                }
            }
        }
        if found.is_empty() && !errors.is_empty() {
            return Err(errors.remove(0));
        }
        // Stable sort keeps provider order among equals.
        found.sort_by_key(|lyrics| lyrics.synced.is_empty());
        Ok(found.into_iter().next())
    }

    /// Asks a provider for the stream behind one of its candidates.
    pub async fn resolve_stream(
        &self,
        provider_id: &str,
        candidate_id: &str,
    ) -> Result<ResolvedStream, CallError> {
        let value = self
            .call(
                provider_id,
                "resolve",
                serde_json::json!({ "id": candidate_id }),
            )
            .await?;
        let source = value
            .get("source")
            .cloned()
            .and_then(|source| serde_json::from_value::<MediaSource>(source).ok())
            .filter(allowed_source)
            .ok_or_else(|| CallError::Script("resolve returned no playable source".to_owned()))?;
        let expires_at = value.get("expiresAt").and_then(serde_json::Value::as_f64);
        Ok(ResolvedStream {
            source,
            expires_at: expires_at.filter(|ms| ms.is_finite()).map(|ms| ms as i64),
        })
    }

    async fn ask<T>(&self, request: impl FnOnce(oneshot::Sender<T>) -> Request) -> Option<T> {
        let (reply, answer) = oneshot::channel();
        self.requests.send(request(reply)).ok()?;
        answer.await.ok()
    }
}

struct Loaded {
    extension: Rc<Extension>,
}

/// A loaded extension: JavaScript, or a Shosetsu script (M32).
enum Extension {
    Js(JsExtension),
    Shosetsu(Box<ShosetsuExtension>),
}

impl Extension {
    fn manifest(&self) -> &Manifest {
        match self {
            Self::Js(extension) => extension.manifest(),
            Self::Shosetsu(extension) => extension.manifest(),
        }
    }

    fn user_settings(&self) -> &BTreeMap<String, serde_json::Value> {
        match self {
            Self::Js(extension) => extension.user_settings(),
            Self::Shosetsu(extension) => extension.user_settings(),
        }
    }

    fn providers(&self) -> Vec<ProviderInfo> {
        match self {
            Self::Js(extension) => extension.providers(),
            Self::Shosetsu(extension) => extension.providers(),
        }
    }

    async fn call(
        &self,
        local_id: &str,
        method: &str,
        args: &serde_json::Value,
    ) -> Result<serde_json::Value, String> {
        match self {
            Self::Js(extension) => extension.call(local_id, method, args).await,
            Self::Shosetsu(extension) => extension.call(local_id, method, args).await,
        }
    }
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
    ) -> Result<Extension, String> {
        if manifest.runtime == Runtime::Shosetsu {
            let load = ShosetsuExtension::load(manifest, source);
            return tokio::time::timeout(self.config.call_timeout, load)
                .await
                .map_err(|_| "loading timed out".to_owned())?
                .map(|extension| Extension::Shosetsu(Box::new(extension)));
        }
        user_settings.retain(|key, value| manifest.check_setting(key, value).is_ok());
        let code = source.read(&manifest.entry)?;
        let storage = KvStore::open(
            self.config
                .storage_dir
                .as_ref()
                .map(|dir| dir.join(&manifest.id).join("storage.json")),
        );
        let load = JsExtension::load(
            manifest,
            code,
            storage,
            user_settings,
            &self.config.limits,
            self.config.content.clone(),
        );
        tokio::time::timeout(self.config.call_timeout, load)
            .await
            .map_err(|_| "loading timed out".to_owned())?
            .map(Extension::Js)
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
        // Shosetsu scripts block on each request and may page through long chapter lists.
        let timeout = match extension.as_ref() {
            Extension::Js(_) => self.config.call_timeout,
            Extension::Shosetsu(_) => self
                .config
                .call_timeout
                .max(crate::shosetsu::runtime::CALL_BUDGET + Duration::from_secs(5)),
        };
        tokio::time::timeout(timeout, extension.call(local_id, method, args))
            .await
            .map_err(|_| CallError::Timeout(provider.to_owned()))?
            .map_err(CallError::Script)
    }
}

fn loaded_info(extension: &Extension, origin: ExtensionOrigin) -> ExtensionInfo {
    let manifest = extension.manifest();
    let values = extension.user_settings();
    ExtensionInfo {
        id: manifest.id.clone(),
        name: manifest.name.clone(),
        version: manifest.version.to_string(),
        description: manifest.description.clone(),
        content_rating: manifest.content_rating.as_str().to_owned(),
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
        compatibility: manifest.nuclear.as_ref().and_then(compat_report),
        icon: None,
        settings: manifest
            .settings
            .iter()
            .map(|setting| {
                let default = setting.default.as_ref().and_then(SettingValue::from_json);
                ExtensionSetting {
                    key: setting.key.clone(),
                    label: setting
                        .label
                        .clone()
                        .unwrap_or_else(|| key_label(&setting.key)),
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

/// The report the Nuclear converter stored in the manifest (`nuclear: { package, report }`).
fn compat_report(nuclear: &serde_json::Value) -> Option<raygun_model::CompatReport> {
    let report = nuclear.get("report")?;
    let list = |key: &str| -> Vec<String> {
        report
            .get(key)
            .and_then(serde_json::Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| item.as_str().map(str::to_owned))
                    .collect()
            })
            .unwrap_or_default()
    };
    Some(raygun_model::CompatReport {
        package: nuclear.get("package")?.as_str()?.to_owned(),
        supported: list("supported"),
        unsupported_categories: list("unsupportedCategories"),
        stubbed_apis: list("stubbedApis"),
        ui_imports: list("uiImports"),
    })
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
        content_rating: "safe".to_owned(),
        origin,
        status: ExtensionStatus::Failed,
        error: Some(error),
        provides: Vec::new(),
        net: Vec::new(),
        providers: Vec::new(),
        settings: Vec::new(),
        compatibility: None,
        icon: None,
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
            source: track.source.filter(allowed_source),
        })
        .collect()
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawLine {
    time_ms: f64,
    text: String,
}

/// Reads `{ synced?: [{ timeMs, text }], plain? }`. Lines with bad times are dropped and the rest
/// sorted; an answer with neither synced lines nor plain text counts as "no lyrics".
fn parse_lyrics(provider: &ProviderInfo, value: &serde_json::Value) -> Option<Lyrics> {
    if value.is_null() {
        return None;
    }
    let mut synced: Vec<LyricLine> = value
        .get("synced")
        .and_then(serde_json::Value::as_array)
        .map(|lines| {
            lines
                .iter()
                .filter_map(|line| serde_json::from_value::<RawLine>(line.clone()).ok())
                .filter(|line| line.time_ms.is_finite() && line.time_ms >= 0.0)
                .map(|line| LyricLine {
                    time_ms: line.time_ms.min(f64::from(u32::MAX)) as u32,
                    text: line.text.trim().to_owned(),
                })
                .collect()
        })
        .unwrap_or_default();
    synced.sort_by_key(|line| line.time_ms);
    let plain = value
        .get("plain")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|text| !text.is_empty())
        .map(str::to_owned);
    if synced.iter().all(|line| line.text.is_empty()) && plain.is_none() {
        return None;
    }
    Some(Lyrics {
        provider_name: provider.name.clone(),
        synced,
        plain,
    })
}

/// Sources an extension may hand out: web streams and YouTube ids, never local files or lookups.
fn allowed_source(source: &MediaSource) -> bool {
    match source {
        MediaSource::Url { url, .. } => is_web_url(url),
        MediaSource::YouTube { .. } => true,
        MediaSource::LocalFile { .. } | MediaSource::Lookup { .. } => false,
    }
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct RawCandidate {
    id: serde_json::Value,
    title: String,
    #[serde(default)]
    artist: Option<String>,
    #[serde(default)]
    duration_ms: Option<f64>,
    #[serde(default)]
    quality: Option<String>,
    #[serde(default)]
    source: Option<MediaSource>,
}

/// Reads `{ candidates: [...] }` like [`parse_tracks`]: malformed entries are skipped, and a
/// candidate whose inline source isn't allowed is dropped (it would never play).
fn parse_candidates(provider: &ProviderInfo, value: serde_json::Value) -> Vec<Candidate> {
    let Some(candidates) = value
        .get("candidates")
        .and_then(serde_json::Value::as_array)
    else {
        return Vec::new();
    };
    candidates
        .iter()
        .filter_map(|candidate| serde_json::from_value::<RawCandidate>(candidate.clone()).ok())
        .filter(|candidate| candidate.source.as_ref().is_none_or(allowed_source))
        .filter(|candidate| !candidate.title.trim().is_empty())
        .map(|candidate| Candidate {
            provider_id: provider.id.clone(),
            provider_name: provider.name.clone(),
            id: match candidate.id {
                serde_json::Value::String(id) => id,
                other => other.to_string(),
            },
            title: candidate.title.trim().to_owned(),
            artist: candidate.artist.filter(|artist| !artist.trim().is_empty()),
            duration_ms: candidate
                .duration_ms
                .filter(|ms| ms.is_finite() && *ms > 0.0 && *ms < f64::from(u32::MAX))
                .map(|ms| ms as u32),
            quality: candidate.quality,
            source: candidate.source,
        })
        .collect()
}

/// A label for a setting that has none: `itemsPerSearch` and `items_per_search` read as
/// "Items per search".
pub(crate) fn key_label(key: &str) -> String {
    let mut words = String::new();
    let mut previous_lower = false;
    for character in key.chars() {
        if character == '_' || character == '-' {
            words.push(' ');
            previous_lower = false;
            continue;
        }
        if character.is_uppercase() && previous_lower {
            words.push(' ');
        }
        previous_lower = character.is_lowercase() || character.is_ascii_digit();
        words.extend(character.to_lowercase());
    }
    let mut chars = words.trim().chars();
    chars
        .next()
        .map(|first| first.to_uppercase().chain(chars).collect())
        .unwrap_or_default()
}
