//! An installed Kodi plugin as Raygun's providers: `video` (`video.source`) for a video plugin,
//! `music` (`music.metadata`) and `stream` (`music.stream`) for a music one. Its Python runs on a
//! thread of its own, started at the first call and let go after a while without calls; calls
//! wait their turn there, as in Kodi, where a plugin runs once per folder opened.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::mpsc::{self, RecvTimeoutError};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::Duration;

use raygun_model::ProviderInfo;
use serde_json::{Value, json};
use tokio::sync::oneshot;

use super::addon::{ADDON_DIR, KodiSetting, LIBRARY_DIR, PACKAGE_INFO, PackageInfo, kodi_value};
use super::catalog::{Catalog, Content, Memory, Runner, candidates};
use super::python::{Python, RunRequest, RunResult, Sandbox};
use crate::failure::Failure;
use crate::host::{ExtensionFiles, ExtensionSource};
use crate::manifest::Manifest;

/// Longest one call may take, every run of the plugin it needs included.
pub const CALL_BUDGET: Duration = Duration::from_secs(75);
/// Longest one run of the plugin may take.
const RUN_BUDGET: Duration = Duration::from_secs(45);
/// How long the interpreter stays after its last call.
const IDLE: Duration = Duration::from_secs(5 * 60);
/// The thread's stack: Python's frames recurse on it.
const STACK_BYTES: usize = 32 * 1024 * 1024;
/// Settings the plugin changed itself, kept in its data folder.
const CHANGED_SETTINGS: &str = "kodi-settings.json";

pub const VIDEO: &str = "video";
pub const MUSIC: &str = "music";
pub const STREAM: &str = "stream";

struct Job {
    provider: String,
    method: String,
    args: Value,
    reply: oneshot::Sender<Result<Value, Failure>>,
}

pub struct KodiExtension {
    manifest: Manifest,
    info: PackageInfo,
    /// The plugin's thread, started at its first call: most installed plugins aren't used right
    /// after the app starts.
    jobs: Mutex<Option<mpsc::Sender<Job>>>,
    setup: Mutex<Option<Setup>>,
    user_settings: BTreeMap<String, Value>,
    memory: Arc<Mutex<Memory>>,
}

/// What the worker needs to start the interpreter.
struct Setup {
    label: String,
    info: PackageInfo,
    folder: PathBuf,
    data: Option<PathBuf>,
    loopback: bool,
    settings: BTreeMap<String, Value>,
    memory: Arc<Mutex<Memory>>,
}

impl KodiExtension {
    /// `data` is the extension's own data folder (`None` keeps its files in memory).
    pub async fn load(
        manifest: Manifest,
        source: &ExtensionSource,
        data: Option<PathBuf>,
        user_settings: BTreeMap<String, Value>,
    ) -> Result<Self, String> {
        Self::load_with(manifest, source, data, user_settings, false)
    }

    /// Like [`Self::load`]; `loopback` lets the plugin reach 127.0.0.1 (tests).
    pub fn load_with(
        manifest: Manifest,
        source: &ExtensionSource,
        data: Option<PathBuf>,
        user_settings: BTreeMap<String, Value>,
        loopback: bool,
    ) -> Result<Self, String> {
        let ExtensionFiles::Dir(folder) = &source.files else {
            return Err("a Kodi add-on runs from its installed folder".into());
        };
        let info: PackageInfo = serde_json::from_str(&source.read(PACKAGE_INFO)?)
            .map_err(|error| format!("its {PACKAGE_INFO} can't be read: {error}"))?;
        let user_settings: BTreeMap<String, Value> = user_settings
            .into_iter()
            .filter(|(key, value)| manifest.check_setting(key, value).is_ok())
            .collect();
        let memory = Arc::new(Mutex::new(Memory::default()));
        let setup = Setup {
            label: manifest.id.clone(),
            info: info.clone(),
            folder: folder.clone(),
            data,
            loopback,
            settings: user_settings.clone(),
            memory: memory.clone(),
        };
        Ok(Self {
            manifest,
            info,
            jobs: Mutex::new(None),
            setup: Mutex::new(Some(setup)),
            user_settings,
            memory,
        })
    }

    pub fn manifest(&self) -> &Manifest {
        &self.manifest
    }

    /// Where calls go: the plugin's thread, started the first time.
    fn sender(&self) -> Result<mpsc::Sender<Job>, String> {
        let mut jobs = self.jobs.lock().unwrap_or_else(PoisonError::into_inner);
        if let Some(jobs) = jobs.as_ref() {
            return Ok(jobs.clone());
        }
        let setup = self
            .setup
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .take()
            .ok_or("the add-on has stopped")?;
        let (sender, inbox) = mpsc::channel::<Job>();
        std::thread::Builder::new()
            .name(format!("kodi {}", self.manifest.id))
            .stack_size(STACK_BYTES)
            .spawn(move || work(setup, &inbox))
            .map_err(|error| error.to_string())?;
        *jobs = Some(sender.clone());
        Ok(sender)
    }

    pub fn user_settings(&self) -> &BTreeMap<String, Value> {
        &self.user_settings
    }

    pub fn providers(&self) -> Vec<ProviderInfo> {
        let provider = |local: &str, kind: &str| ProviderInfo {
            adult: false,
            id: format!("{}/{local}", self.manifest.id),
            extension_id: self.manifest.id.clone(),
            kind: kind.to_owned(),
            name: self.manifest.name.clone(),
            search_pill: None,
        };
        let mut providers = Vec::new();
        if self.info.provides_video() {
            providers.push(provider(VIDEO, crate::video::VIDEO_SOURCE));
        }
        if self.info.provides_audio() {
            providers.push(provider(MUSIC, crate::MUSIC_METADATA));
            providers.push(provider(STREAM, crate::MUSIC_STREAM));
        }
        providers
    }

    pub(crate) async fn call(
        &self,
        local_id: &str,
        method: &str,
        args: &Value,
    ) -> Result<Value, Failure> {
        // Streams for songs it listed are found without running the plugin: Raygun asks every
        // stream provider for each song played.
        if local_id == STREAM && method == "candidates" {
            return Ok(candidates(&self.memory, args));
        }
        let (reply, answer) = oneshot::channel();
        self.sender()
            .map_err(Failure::said)?
            .send(Job {
                provider: local_id.to_owned(),
                method: method.to_owned(),
                args: args.clone(),
                reply,
            })
            .map_err(|_| Failure::said("the add-on has stopped"))?;
        answer
            .await
            .map_err(|_| Failure::said("the add-on stopped during the call"))?
    }
}

/// The plugin's settings as Kodi stores them: its defaults, what it changed itself, what was set
/// in Raygun.
fn effective_settings(
    settings: &[KodiSetting],
    changed: &BTreeMap<String, String>,
    raygun: &BTreeMap<String, Value>,
) -> BTreeMap<String, String> {
    let mut values: BTreeMap<String, String> = settings
        .iter()
        .map(|setting| (setting.id.clone(), setting.default.clone()))
        .collect();
    for (key, value) in changed {
        values.insert(key.clone(), value.clone());
    }
    for setting in settings {
        if let Some(value) = raygun
            .get(&setting.id)
            .and_then(|value| kodi_value(setting, value))
        {
            values.insert(setting.id.clone(), value);
        }
    }
    values
}

/// The add-ons' details as `xbmcaddon` reads them, by Kodi id.
fn addons_of(
    info: &PackageInfo,
    changed: &BTreeMap<String, BTreeMap<String, String>>,
    raygun: &BTreeMap<String, Value>,
) -> BTreeMap<String, Value> {
    let mut addons = BTreeMap::new();
    let none = BTreeMap::new();
    addons.insert(
        info.addon.clone(),
        json!({
            "id": info.addon,
            "name": info.name,
            "version": info.version,
            "author": info.author,
            "summary": info.summary,
            "description": info.summary,
            "path": format!("/kodi/home/addons/{}", info.addon),
            "icon": info.icon,
            "fanart": info.fanart,
            "type": "xbmc.python.pluginsource",
            "settings": effective_settings(
                &info.settings,
                changed.get(&info.addon).unwrap_or(&none),
                raygun,
            ),
        }),
    );
    for dependency in &info.dependencies {
        let mut settings = dependency.settings.clone();
        if let Some(changed) = changed.get(&dependency.id) {
            settings.extend(changed.clone());
        }
        addons.insert(
            dependency.id.clone(),
            json!({
                "id": dependency.id,
                "name": dependency.name,
                "version": dependency.version,
                "path": format!("/kodi/home/addons/{}", dependency.id),
                "library": dependency.library,
                "type": "xbmc.python.module",
                "settings": settings,
            }),
        );
    }
    addons
}

/// Runs the plugin through the interpreter, starting it when needed.
struct Interpreter<'a> {
    setup: &'a Setup,
    python: &'a mut Option<Python>,
}

impl Runner for Interpreter<'_> {
    fn run(&mut self, request: RunRequest) -> Result<RunResult, String> {
        if self.python.is_none() {
            let setup = self.setup;
            let libraries: Vec<(String, PathBuf, String)> = setup
                .info
                .dependencies
                .iter()
                .map(|dependency| {
                    (
                        dependency.id.clone(),
                        setup.folder.join(LIBRARY_DIR).join(&dependency.id),
                        dependency.library.clone(),
                    )
                })
                .collect();
            let sandbox = Sandbox::new(
                &setup.label,
                (&setup.info.addon, setup.folder.join(ADDON_DIR)),
                &setup.info.library,
                &libraries,
                setup.data.clone(),
                setup.loopback,
            );
            let started = std::time::Instant::now();
            *self.python = Some(Python::start(Arc::new(sandbox)).map_err(|error| {
                format!("Raygun's Python couldn't start for this add-on\n{error}")
            })?);
            tracing::debug!(addon = %setup.label, took = ?started.elapsed(), "Kodi add-on's Python started");
        }
        let python = self.python.as_ref().expect("started above");
        python.run(&request, RUN_BUDGET)
    }
}

fn load_changed(data: Option<&PathBuf>) -> BTreeMap<String, BTreeMap<String, String>> {
    data.and_then(|folder| std::fs::read(folder.join(CHANGED_SETTINGS)).ok())
        .and_then(|bytes| serde_json::from_slice(&bytes).ok())
        .unwrap_or_default()
}

fn save_changed(data: Option<&PathBuf>, changed: &BTreeMap<String, BTreeMap<String, String>>) {
    let Some(folder) = data else {
        return;
    };
    if std::fs::create_dir_all(folder).is_ok()
        && let Ok(bytes) = serde_json::to_vec_pretty(changed)
        && let Err(error) = std::fs::write(folder.join(CHANGED_SETTINGS), bytes)
    {
        tracing::debug!(%error, "a Kodi add-on's settings couldn't be kept");
    }
}

/// The plugin's thread: answers calls in turn, starting Python at the first and letting it go
/// after [`IDLE`].
fn work(setup: Setup, inbox: &mpsc::Receiver<Job>) {
    let mut changed = load_changed(setup.data.as_ref());
    let template = RunRequest {
        handle: 1,
        addon: setup.info.addon.clone(),
        entry: format!(
            "/kodi/home/addons/{}/{}",
            setup.info.addon,
            setup.info.library.trim_start_matches('/')
        ),
        addons: addons_of(&setup.info, &changed, &setup.settings),
        language: "en_gb".into(),
        platform: if cfg!(target_os = "android") {
            "android"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        }
        .into(),
        ..RunRequest::default()
    };
    let mut catalog = Catalog::new(setup.info.clone(), template, setup.memory.clone());
    let mut python: Option<Python> = None;
    loop {
        let job = match inbox.recv_timeout(IDLE) {
            Ok(job) => job,
            Err(RecvTimeoutError::Timeout) => {
                if python.take().is_some() {
                    tracing::debug!(addon = %setup.label, "idle: the add-on's Python is let go");
                }
                match inbox.recv() {
                    Ok(job) => job,
                    Err(_) => return,
                }
            }
            Err(RecvTimeoutError::Disconnected) => return,
        };
        if job.reply.is_closed() {
            continue;
        }
        let answer = {
            let mut runner = Interpreter {
                setup: &setup,
                python: &mut python,
            };
            answer(
                &mut catalog,
                &mut runner,
                &job.provider,
                &job.method,
                &job.args,
            )
        };
        if !catalog.changed_settings.is_empty() {
            for (addon, values) in std::mem::take(&mut catalog.changed_settings) {
                changed.entry(addon).or_default().extend(values);
            }
            save_changed(setup.data.as_ref(), &changed);
            catalog.set_addons(addons_of(&setup.info, &changed, &setup.settings));
        }
        let _ = job.reply.send(answer.map_err(Failure::said));
    }
}

fn text<'a>(args: &'a Value, key: &str) -> &'a str {
    args.get(key).and_then(Value::as_str).unwrap_or_default()
}

fn page(args: &Value) -> u32 {
    args.get("page")
        .and_then(Value::as_u64)
        .map_or(1, |page| page.clamp(1, 10_000) as u32)
}

fn limit(args: &Value) -> usize {
    args.get("limit")
        .and_then(Value::as_u64)
        .map_or(30, |limit| limit as usize)
}

/// One call, answered by running the plugin as needed.
fn answer(
    catalog: &mut Catalog,
    runner: &mut dyn Runner,
    provider: &str,
    method: &str,
    args: &Value,
) -> Result<Value, String> {
    match (provider, method) {
        (VIDEO, "popular") => catalog.popular(runner, page(args)),
        (VIDEO, "latest") => Ok(json!({ "items": [], "hasNextPage": false })),
        (VIDEO, "filters") => catalog.filters(runner),
        (VIDEO, "search") => {
            let section = args
                .get("filters")
                .and_then(|filters| filters.get("section"))
                .and_then(Value::as_str);
            catalog.search(
                runner,
                text(args, "query"),
                page(args),
                section,
                Content::Video,
            )
        }
        (VIDEO, "details") => catalog.details(runner, text(args, "id")),
        (VIDEO, "episodes") => catalog.episodes(runner, text(args, "id")),
        (VIDEO, "servers") => catalog.servers(runner, text(args, "episodeId")),
        (VIDEO, "videos") => catalog.videos(runner, text(args, "serverId")),
        (MUSIC, "search") => catalog.music_search(runner, text(args, "query"), limit(args)),
        (MUSIC, "searchPlaylists") => catalog.playlists(runner, text(args, "query"), limit(args)),
        (MUSIC, "playlist") => catalog.playlist(runner, text(args, "id")),
        (STREAM, "resolve") => catalog.resolve_song(runner, text(args, "id")),
        (VIDEO | MUSIC | STREAM, method) => {
            Err(format!("provider {provider} has no method {method}"))
        }
        (other, _) => Err(format!("no provider {other}")),
    }
}
