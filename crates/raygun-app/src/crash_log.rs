//! Crash records and the safe start after a crash.
//!
//! Records live in `logs/crashes/` in the data root, one text file each, and never leave the
//! device unless someone saves or shares them (About → Crash logs). Rust writes panics (the panic
//! hook: message, place, thread, backtrace, what sources were doing); on Android the Kotlin side
//! writes its own into the same folder in the same shape (`CrashLog.kt`: uncaught exceptions, the
//! webview's renderer gone, and the system's account of how earlier runs ended: native crashes,
//! ANRs, low-memory kills). Every record starts with the same lines (`Kind:`, `Time:` in
//! milliseconds, `Version:`, `Summary:`), which the list reads.
//!
//! The safe start: each start writes [`MARKER`] and removes it once Raygun ran [`HEALTHY_AFTER`]
//! (or at a clean exit). Found at the next start, or with a flag the Kotlin side or the error page
//! left (the renderer gone, "Start in safe mode", "Clear caches"), that start is a safe one: the UI
//! doesn't restore its saved pages, queues and the last queue resume only once this start proved
//! healthy, extensions load one at a time, and the sources that were loading or running when the
//! last one stopped ([`RUNNING`], written while a start is watched) are named. When two starts in
//! a row stop with the same source at work, that source is held back (not loaded) until turned on
//! again.

use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Mutex, OnceLock, PoisonError};
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use raygun_ext_host::activity::{Activity, Step};
use serde::{Deserialize, Serialize};
use specta::Type;
use tauri::{AppHandle, Manager};

/// The folder in `logs/`.
pub const DIR: &str = "crashes";
/// Written at each start, removed once it proved healthy or Raygun ended cleanly.
const MARKER: &str = ".starting";
/// What sources were doing, while a start is watched: one line each.
const RUNNING: &str = ".running";
/// When the last start began (milliseconds): what went wrong since is that run's.
const LAST_START: &str = ".last-start";
/// Left by the Kotlin side when the webview's renderer went.
const RENDERER_GONE: &str = "renderer-gone";
/// Left by "Start in safe mode" (the error pages).
const SAFE_ASKED: &str = "safe-start";
/// Left by "Clear caches" (the error pages).
const CLEAR_CACHES: &str = "clear-caches";
/// How long a start must run before it counts as healthy.
pub const HEALTHY_AFTER: Duration = Duration::from_secs(20);
/// Records kept, newest first.
const KEPT: usize = 60;
/// The most of one record read back.
const MAX_READ: u64 = 2 * 1024 * 1024;
/// The extensions held back, by id (`local_value`, JSON).
const HELD_BACK_KEY: &str = "crash.heldBack";
/// How often one panic (by place) is recorded in a run.
const SAME_PANIC_KEPT: u32 = 3;

/// Why this start is a safe one.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub enum SafeStartReason {
    /// The last start stopped before it proved healthy.
    Stopped,
    /// The page's renderer went.
    Renderer,
    /// Someone asked for it on an error page.
    Asked,
}

/// How this start began.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct SafeStart {
    pub reason: Option<SafeStartReason>,
    /// Starts in a row that stopped before this one.
    pub crashes: u32,
    /// Extensions loading or running when the last start stopped.
    pub suspects: Vec<String>,
    /// Extensions this start holds back: at work both times the last two starts stopped.
    pub held_back: Vec<String>,
    /// The error page asked to clear the caches.
    pub clear_caches: bool,
    /// When the last start began, and this one (milliseconds).
    pub since: u64,
    pub started_at: u64,
}

impl SafeStart {
    pub fn active(&self) -> bool {
        self.reason.is_some()
    }

    /// What the page learns before its first script (`window.__RAYGUN_SAFE_START__`).
    pub fn script(&self) -> String {
        format!(
            "window.__RAYGUN_SAFE_START__ = {};",
            serde_json::json!({ "active": self.active(), "clearCaches": self.clear_caches })
        )
    }
}

/// The previous start's marker.
#[derive(Debug, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
struct Marker {
    started_at: u64,
    version: String,
    crashes: u32,
    suspects: Vec<String>,
}

struct Crashes {
    dir: PathBuf,
    /// While this start is watched, what sources do is written to [`RUNNING`].
    watching: AtomicBool,
    /// What sources do now: `id` while loading, `id provider.method` while called.
    running: Mutex<Vec<String>>,
    /// Panics recorded this run, by place.
    seen: Mutex<HashMap<String, u32>>,
    started_at: u64,
}

static CRASHES: OnceLock<Crashes> = OnceLock::new();

fn now_ms() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0, |since| {
            u64::try_from(since.as_millis()).unwrap_or(u64::MAX)
        })
}

fn platform() -> &'static str {
    if cfg!(target_os = "android") {
        "android"
    } else {
        std::env::consts::OS
    }
}

/// Takes a flag file: whether it was there.
fn take(dir: &Path, name: &str) -> bool {
    fs::remove_file(dir.join(name)).is_ok()
}

/// The extension a running line is about.
fn extension_of(line: &str) -> &str {
    line.split_once(' ').map_or(line, |(id, _)| id)
}

/// The crash folder of the data root at `logs`.
pub fn dir_in(logs: &Path) -> PathBuf {
    logs.join(DIR)
}

/// Starts watching this start; decides whether it's a safe one. Call once, right after logging.
pub fn begin(logs: &Path) -> SafeStart {
    let dir = dir_in(logs);
    let _ = fs::create_dir_all(&dir);
    let previous: Option<Marker> = fs::read(dir.join(MARKER))
        .ok()
        .map(|bytes| serde_json::from_slice(&bytes).unwrap_or_default());
    let mut start = decide(&dir, previous);
    let started_at = now_ms();
    start.started_at = started_at;
    let _ = fs::write(dir.join(LAST_START), started_at.to_string());
    let marker = Marker {
        started_at,
        version: env!("CARGO_PKG_VERSION").to_owned(),
        crashes: start.crashes,
        suspects: start.suspects.clone(),
    };
    if let Ok(json) = serde_json::to_vec(&marker) {
        let _ = fs::write(dir.join(MARKER), json);
    }
    prune(&dir);
    let _ = CRASHES.set(Crashes {
        dir,
        watching: AtomicBool::new(true),
        running: Mutex::new(Vec::new()),
        seen: Mutex::new(HashMap::new()),
        started_at,
    });
    raygun_ext_host::activity::set_hook(heard);
    if start.active() {
        tracing::warn!(
            ?start,
            "safe start: the last start stopped or one was asked for"
        );
    }
    start
}

/// How this start begins, from the last one's marker and the flags left in `dir` (which it takes).
fn decide(dir: &Path, previous: Option<Marker>) -> SafeStart {
    let since = fs::read_to_string(dir.join(LAST_START))
        .ok()
        .and_then(|text| text.trim().parse().ok())
        .unwrap_or(0);
    let renderer = take(dir, RENDERER_GONE);
    let asked = take(dir, SAFE_ASKED);
    let clear_caches = take(dir, CLEAR_CACHES);
    let running = fs::read_to_string(dir.join(RUNNING)).unwrap_or_default();
    let _ = fs::remove_file(dir.join(RUNNING));
    let stopped = previous.is_some();
    let mut suspects: Vec<String> = running
        .lines()
        .map(|line| extension_of(line).to_owned())
        .filter(|id| !id.is_empty())
        .collect();
    suspects.sort();
    suspects.dedup();
    if !stopped {
        suspects.clear();
    }
    let crashes = if stopped || renderer {
        previous.as_ref().map_or(0, |marker| marker.crashes) + 1
    } else {
        0
    };
    let held_back: Vec<String> = match &previous {
        Some(marker) if crashes >= 2 => suspects
            .iter()
            .filter(|id| marker.suspects.contains(id))
            .cloned()
            .collect(),
        _ => Vec::new(),
    };
    let reason = if asked || clear_caches {
        Some(SafeStartReason::Asked)
    } else if renderer {
        Some(SafeStartReason::Renderer)
    } else if stopped {
        Some(SafeStartReason::Stopped)
    } else {
        None
    };
    if let Some(marker) = &previous {
        note_stopped_start(dir, marker, &running);
    }
    SafeStart {
        reason,
        crashes,
        suspects,
        held_back,
        clear_caches,
        since,
        started_at: now_ms(),
    }
}

/// What went wrong in the last run (between `since` and `until`): the newest record, one of
/// Raygun's own or Android's account rather than the `start` record written when none was there.
fn last_crash(records: &[CrashRecord], since: u64, until: u64) -> Option<CrashRecord> {
    let (since, until) = (since as f64, until as f64);
    let in_run = |record: &&CrashRecord| record.at >= since && record.at <= until;
    records
        .iter()
        .filter(in_run)
        .find(|record| record.kind != "start")
        .or_else(|| records.iter().find(in_run))
        .cloned()
}
/// A start that stopped without leaving a record of its own (a native crash, the system ending
/// it): one is written now, naming what sources were doing. Android's own account of it, when
/// there is one, comes as another record (`CrashLog.kt`).
fn note_stopped_start(dir: &Path, marker: &Marker, running: &str) {
    let explained = records(dir)
        .iter()
        .any(|record| record.at >= marker.started_at as f64);
    if explained {
        return;
    }
    let at_work = if running.trim().is_empty() {
        "nothing from sources".to_owned()
    } else {
        running.trim().to_owned()
    };
    write_record(
        dir,
        "start",
        marker.started_at.max(1),
        &format!("Raygun {} stopped while it started", marker.version),
        &format!(
            "Raygun stopped before it had run {} s, and nothing recorded why (a native crash, \
             or the system ended it).\n\nAt work then:\n{at_work}\n",
            HEALTHY_AFTER.as_secs()
        ),
    );
}

/// What the extension host says sources do.
fn heard(activity: Activity<'_>, step: Step) {
    let Some(crashes) = CRASHES.get() else {
        return;
    };
    let line = match activity {
        Activity::Loading(id) => id.to_owned(),
        Activity::Calling(id, call) => format!("{id} {call}"),
    };
    let mut running = crashes
        .running
        .lock()
        .unwrap_or_else(PoisonError::into_inner);
    match step {
        Step::Began => running.push(line),
        Step::Ended => {
            if let Some(index) = running.iter().position(|entry| *entry == line) {
                running.swap_remove(index);
            }
        }
    }
    if crashes.watching.load(Ordering::Relaxed) {
        let _ = fs::write(crashes.dir.join(RUNNING), running.join("\n"));
    }
}

/// This start ran long enough: the marker goes, and what sources do isn't written any more.
fn settle() {
    if let Some(crashes) = CRASHES.get() {
        crashes.watching.store(false, Ordering::Relaxed);
        let _ = fs::remove_file(crashes.dir.join(MARKER));
        let _ = fs::remove_file(crashes.dir.join(RUNNING));
        tracing::info!("the start went well");
    }
}

/// Settles this start after [`HEALTHY_AFTER`].
pub fn settle_later() {
    let spawned = std::thread::Builder::new()
        .name("raygun-start-watch".into())
        .spawn(|| {
            std::thread::sleep(HEALTHY_AFTER);
            settle();
        });
    if spawned.is_err() {
        settle();
    }
}

/// Raygun ends as it should: the next start is a normal one.
pub fn clean_exit() {
    if let Some(crashes) = CRASHES.get() {
        crashes.watching.store(false, Ordering::Relaxed);
        let _ = fs::remove_file(crashes.dir.join(MARKER));
        let _ = fs::remove_file(crashes.dir.join(RUNNING));
    }
}

/// Runs `work` now, or on a safe start once this start proved healthy: what a crash may have
/// come from (queues, the last queue) waits until then.
pub fn when_healthy(app: &AppHandle, work: impl FnOnce() + Send + 'static) {
    let safe = app
        .try_state::<SafeStartState>()
        .is_some_and(|state| state.0.active());
    if !safe {
        work();
        return;
    }
    let spawned = std::thread::Builder::new()
        .name("raygun-safe-start".into())
        .spawn(move || {
            std::thread::sleep(HEALTHY_AFTER);
            work();
        });
    if let Err(error) = spawned {
        tracing::warn!(%error, "couldn't wait for a healthy start");
    }
}

/// This start, as Tauri state.
pub struct SafeStartState(pub SafeStart);

/// Whether this start is a safe one.
pub fn is_safe(app: &AppHandle) -> bool {
    app.try_state::<SafeStartState>()
        .is_some_and(|state| state.0.active())
}

/// Records a panic (the panic hook calls this; it never panics itself).
pub fn record_panic(info: &std::panic::PanicHookInfo<'_>) {
    let Some(crashes) = CRASHES.get() else {
        return;
    };
    let place = info.location().map_or_else(
        || "an unknown place".to_owned(),
        |location| {
            format!(
                "{}:{}:{}",
                location.file(),
                location.line(),
                location.column()
            )
        },
    );
    let message = info
        .payload()
        .downcast_ref::<&str>()
        .map(|text| (*text).to_owned())
        .or_else(|| info.payload().downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "(no message)".to_owned());
    {
        let mut seen = crashes.seen.lock().unwrap_or_else(PoisonError::into_inner);
        let count = seen.entry(place.clone()).or_default();
        *count += 1;
        if *count > SAME_PANIC_KEPT {
            return;
        }
    }
    let thread = std::thread::current()
        .name()
        .unwrap_or("unnamed")
        .to_owned();
    let running = crashes
        .running
        .lock()
        .map(|running| running.join("\n"))
        .unwrap_or_default();
    let backtrace = std::backtrace::Backtrace::force_capture();
    let first_line = message.lines().next().unwrap_or_default();
    write_record(
        &crashes.dir,
        "panic",
        now_ms(),
        &format!("{first_line} ({place})"),
        &format!(
            "A part of Raygun stopped (a Rust panic). Raygun may have gone on without it.\n\n\
             Thread: {thread}\nPlace: {place}\nRunning for: {} s\n\nMessage:\n{message}\n\n\
             Sources at work:\n{}\n\nBacktrace:\n{backtrace}\n",
            now_ms().saturating_sub(crashes.started_at) / 1000,
            if running.is_empty() { "none" } else { &running },
        ),
    );
}

/// Writes one record: its name.
fn write_record(dir: &Path, kind: &str, at: u64, summary: &str, body: &str) -> Option<String> {
    let summary: String = summary
        .chars()
        .map(|character| {
            if character.is_control() {
                ' '
            } else {
                character
            }
        })
        .take(300)
        .collect();
    let text = format!(
        "Raygun crash record\nKind: {kind}\nTime: {at}\nVersion: {} ({}, {})\nPid: {}\nSummary: \
         {summary}\n\n{body}",
        env!("CARGO_PKG_VERSION"),
        platform(),
        if cfg!(debug_assertions) {
            "debug"
        } else {
            "release"
        },
        std::process::id(),
    );
    let mut name = format!("{at}-{kind}.txt");
    let mut tries = 1;
    while dir.join(&name).exists() && tries < 20 {
        name = format!("{at}-{kind}-{tries}.txt");
        tries += 1;
    }
    let mut file = fs::File::create(dir.join(&name)).ok()?;
    file.write_all(text.as_bytes()).ok()?;
    let _ = file.sync_all();
    Some(name)
}

/// One record, as the list shows it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CrashRecord {
    /// Its file name.
    pub id: String,
    /// `panic`, `java`, `renderer`, `native`, `anr`, `memory`, `exit` or `start`.
    pub kind: String,
    /// When, in milliseconds since 1970.
    pub at: f64,
    pub summary: String,
}

/// Every record in `dir`, newest first.
fn records(dir: &Path) -> Vec<CrashRecord> {
    let Ok(entries) = fs::read_dir(dir) else {
        return Vec::new();
    };
    let mut records: Vec<CrashRecord> = entries
        .filter_map(Result::ok)
        .filter_map(|entry| {
            let name = entry.file_name().to_string_lossy().into_owned();
            if !name.ends_with(".txt") {
                return None;
            }
            let mut head = String::new();
            fs::File::open(entry.path())
                .ok()?
                .take(4096)
                .read_to_string(&mut head)
                .ok()?;
            parse_head(&name, &head)
        })
        .collect();
    records.sort_by(|a, b| b.at.total_cmp(&a.at).then_with(|| b.id.cmp(&a.id)));
    records
}

fn parse_head(id: &str, head: &str) -> Option<CrashRecord> {
    let mut fields = BTreeMap::new();
    for line in head.lines().take(12) {
        if let Some((key, value)) = line.split_once(": ") {
            fields.insert(key, value.trim());
        }
    }
    let at: u64 = fields
        .get("Time")
        .and_then(|time| time.parse().ok())
        .or_else(|| id.split('-').next()?.parse().ok())?;
    Some(CrashRecord {
        id: id.to_owned(),
        kind: fields.get("Kind").unwrap_or(&"crash").to_string(),
        at: at as f64,
        summary: fields.get("Summary").unwrap_or(&"").to_string(),
    })
}

/// Keeps the newest [`KEPT`] records (and the files beside them, a native crash's tombstone).
fn prune(dir: &Path) {
    for old in records(dir).into_iter().skip(KEPT) {
        let _ = fs::remove_file(dir.join(&old.id));
        if let Some(stem) = old.id.strip_suffix(".txt") {
            let _ = fs::remove_file(dir.join(format!("{stem}.tombstone.pb")));
        }
    }
}

/// A record's name must be one of the folder's files, nothing else.
fn checked(dir: &Path, id: &str) -> Result<PathBuf, String> {
    let plain = !id.is_empty()
        && !id.starts_with('.')
        && id
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || "-_.".contains(character));
    if !plain || !id.ends_with(".txt") {
        return Err("That isn't a crash record".into());
    }
    let path = dir.join(id);
    if path.is_file() {
        Ok(path)
    } else {
        Err("That crash record is gone".into())
    }
}

/// The extensions held back, by id, with when.
async fn held_back(db: &raygun_db::Db) -> BTreeMap<String, u64> {
    raygun_db::local_value(db, HELD_BACK_KEY)
        .await
        .ok()
        .flatten()
        .and_then(|json| serde_json::from_str(&json).ok())
        .unwrap_or_default()
}

async fn set_held_back(db: &raygun_db::Db, held: &BTreeMap<String, u64>) -> Result<(), String> {
    let json = serde_json::to_string(held).map_err(|error| error.to_string())?;
    raygun_db::set_local_value(db, HELD_BACK_KEY, &json)
        .await
        .map_err(|error| error.to_string())
}

/// What a held-back extension's row says.
pub const HELD_BACK_REASON: &str = "Held back: Raygun stopped twice while this source was at \
                                    work as it started. Turn it on again in About → Crash logs.";

/// The extensions to hold back from loading: those held before, and those this start adds.
pub async fn held_back_now(db: &raygun_db::Db, start: &SafeStart) -> HashMap<String, String> {
    let mut held = held_back(db).await;
    if !start.held_back.is_empty() {
        for id in &start.held_back {
            held.entry(id.clone()).or_insert_with(now_ms);
        }
        if let Err(error) = set_held_back(db, &held).await {
            tracing::warn!(%error, "couldn't keep the held-back sources");
        }
        tracing::warn!(held = ?start.held_back, "sources held back after two stopped starts");
    }
    held.into_keys()
        .map(|id| (id, HELD_BACK_REASON.to_owned()))
        .collect()
}

// ---- Commands ----

/// An extension named in the safe start or held back.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SourceRef {
    pub id: String,
    pub name: String,
}

/// This start, as the UI shows it.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct SafeStartInfo {
    pub active: bool,
    pub reason: Option<SafeStartReason>,
    pub crashes: u32,
    pub suspects: Vec<SourceRef>,
    /// Held back by this start.
    pub held_now: Vec<SourceRef>,
    /// What went wrong in the last run, when a record says.
    pub last_crash: Option<CrashRecord>,
}

/// About → Crash logs.
#[derive(Debug, Clone, Serialize, Type)]
#[serde(rename_all = "camelCase")]
pub struct CrashLogs {
    pub records: Vec<CrashRecord>,
    /// Every extension held back.
    pub held_back: Vec<SourceRef>,
    /// The logs folder, as a path.
    pub folder: String,
    pub safe_start: SafeStartInfo,
}

fn crash_dir(app: &AppHandle) -> PathBuf {
    dir_in(&app.state::<crate::startup::AppPaths>().0.logs())
}

async fn names(app: &AppHandle, ids: impl IntoIterator<Item = String>) -> Vec<SourceRef> {
    let infos = match app.try_state::<std::sync::Arc<crate::extensions::ExtensionService>>() {
        Some(service) => service.list().await,
        None => Vec::new(),
    };
    ids.into_iter()
        .map(|id| SourceRef {
            name: infos
                .iter()
                .find(|info| info.id == id)
                .map_or_else(|| id.clone(), |info| info.name.clone()),
            id,
        })
        .collect()
}

async fn safe_start_info(app: &AppHandle) -> SafeStartInfo {
    let start = app
        .try_state::<SafeStartState>()
        .map(|state| state.0.clone())
        .unwrap_or_default();
    let last_crash = if start.active() {
        record_exits(app).await;
        let dir = crash_dir(app);
        let list = tauri::async_runtime::spawn_blocking(move || records(&dir))
            .await
            .unwrap_or_default();
        last_crash(&list, start.since, start.started_at)
    } else {
        None
    };
    SafeStartInfo {
        active: start.active(),
        reason: start.reason,
        crashes: start.crashes,
        suspects: names(app, start.suspects.clone()).await,
        held_now: names(app, start.held_back.clone()).await,
        last_crash,
    }
}

/// Android's account of how Raygun's processes ended lately, recorded first (`CrashLog.kt`):
/// one that ended while Raygun ran on (the browser's, the error screen's) has no start of its own.
async fn record_exits(app: &AppHandle) {
    #[cfg(target_os = "android")]
    {
        let app = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .crash_record_exits()
        })
        .await;
    }
    #[cfg(not(target_os = "android"))]
    let _ = app;
}

/// How this start began: a safe start after a crash, and the sources it names.
#[tauri::command]
#[specta::specta]
pub async fn safe_start_status(app: AppHandle) -> SafeStartInfo {
    safe_start_info(&app).await
}

/// The crash records, the held-back sources, and this start.
#[tauri::command]
#[specta::specta]
pub async fn crash_logs(app: AppHandle) -> Result<CrashLogs, String> {
    record_exits(&app).await;
    let dir = crash_dir(&app);
    let list = {
        let dir = dir.clone();
        tauri::async_runtime::spawn_blocking(move || records(&dir))
            .await
            .map_err(|error| error.to_string())?
    };
    let db = app.state::<crate::startup::AppDb>().0.clone();
    let held = held_back(&db).await;
    Ok(CrashLogs {
        records: list,
        held_back: names(&app, held.into_keys()).await,
        folder: app
            .state::<crate::startup::AppPaths>()
            .0
            .logs()
            .display()
            .to_string(),
        safe_start: safe_start_info(&app).await,
    })
}

/// One record's text.
#[tauri::command]
#[specta::specta]
pub async fn crash_log(app: AppHandle, id: String) -> Result<String, String> {
    let path = checked(&crash_dir(&app), &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let mut text = String::new();
        fs::File::open(&path)
            .map_err(|error| error.to_string())?
            .take(MAX_READ)
            .read_to_string(&mut text)
            .map_err(|error| error.to_string())?;
        Ok(text)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Records an error the page couldn't get past (its error screen shows): the record's id.
#[tauri::command]
#[specta::specta]
pub async fn crash_log_write(
    app: AppHandle,
    summary: String,
    detail: String,
) -> Result<String, String> {
    let dir = crash_dir(&app);
    let detail: String = detail.chars().take(64 * 1024).collect();
    tauri::async_runtime::spawn_blocking(move || {
        write_record(
            &dir,
            "ui",
            now_ms(),
            &summary,
            &format!("Raygun's page met an error it couldn't get past.\n\n{detail}\n"),
        )
        .ok_or_else(|| "The crash log couldn't be written".to_owned())
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Removes every crash record.
#[tauri::command]
#[specta::specta]
pub async fn crash_logs_clear(app: AppHandle) -> Result<(), String> {
    let dir = crash_dir(&app);
    tauri::async_runtime::spawn_blocking(move || {
        for record in records(&dir) {
            let _ = fs::remove_file(dir.join(&record.id));
            if let Some(stem) = record.id.strip_suffix(".txt") {
                let _ = fs::remove_file(dir.join(format!("{stem}.tombstone.pb")));
            }
        }
    })
    .await
    .map_err(|error| error.to_string())
}

fn outputs(app: &AppHandle) -> crate::export::Outputs {
    let paths = &app.state::<crate::startup::AppPaths>().0;
    crate::export::Outputs::new(
        app.clone(),
        paths.exports(),
        paths.cache(raygun_paths::CacheKind::Share),
    )
}

/// Copies a file to `place` under `logs/`, or to the share folder: what it became.
fn copy_out(
    app: &AppHandle,
    place: &crate::export::Place,
    name: &str,
    mime: &str,
    bytes: &[u8],
) -> Result<crate::export::output::Saved, String> {
    let outputs = outputs(app);
    let folders = if *place == crate::export::Place::Share {
        Vec::new()
    } else {
        vec!["logs".to_owned()]
    };
    let mut output = outputs.create(place, &folders, name, mime)?;
    let written = output.file.write_all(bytes);
    let saved = outputs.finish(output, written.is_ok())?;
    written.map_err(|error| error.to_string())?;
    Ok(saved)
}

/// Saves one record in Download/Raygun/logs: where it went.
#[tauri::command]
#[specta::specta]
pub async fn crash_log_save(app: AppHandle, id: String) -> Result<String, String> {
    let path = checked(&crash_dir(&app), &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let name = format!("raygun-{id}");
        copy_out(
            &app,
            &crate::export::Place::Downloads,
            &name,
            "text/plain",
            &bytes,
        )
        .map(|saved| saved.shown)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Opens the share sheet for one record (desktop: shows it in the file manager).
#[tauri::command]
#[specta::specta]
pub async fn crash_log_share(app: AppHandle, id: String) -> Result<(), String> {
    let path = checked(&crash_dir(&app), &id)?;
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = fs::read(&path).map_err(|error| error.to_string())?;
        let name = format!("raygun-{id}");
        let saved = copy_out(
            &app,
            &crate::export::Place::Share,
            &name,
            "text/plain",
            &bytes,
        )?;
        let shared = saved.path.or(saved.local).ok_or("Nothing to share")?;
        outputs(&app).share(&shared, "text/plain", "Raygun crash log")
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Every log (the daily logs and the crash records) in one zip.
fn logs_zip(logs: &Path) -> Result<Vec<u8>, String> {
    let mut zip = zip::ZipWriter::new(std::io::Cursor::new(Vec::new()));
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    let mut files = Vec::new();
    for (folder, prefix) in [(logs.to_path_buf(), ""), (dir_in(logs), "crashes/")] {
        let Ok(entries) = fs::read_dir(&folder) else {
            continue;
        };
        for entry in entries.filter_map(Result::ok) {
            let name = entry.file_name().to_string_lossy().into_owned();
            if entry.path().is_file() && !name.starts_with('.') {
                files.push((format!("{prefix}{name}"), entry.path()));
            }
        }
    }
    files.sort();
    for (name, path) in files {
        let Ok(bytes) = fs::read(&path) else {
            continue;
        };
        zip.start_file(name, options)
            .map_err(|error| error.to_string())?;
        zip.write_all(&bytes).map_err(|error| error.to_string())?;
    }
    zip.finish()
        .map(std::io::Cursor::into_inner)
        .map_err(|error| error.to_string())
}

/// Saves every log as one zip in Download/Raygun/logs: where it went.
#[tauri::command]
#[specta::specta]
pub async fn logs_save_all(app: AppHandle) -> Result<String, String> {
    let logs = app.state::<crate::startup::AppPaths>().0.logs();
    tauri::async_runtime::spawn_blocking(move || {
        let bytes = logs_zip(&logs)?;
        let name = format!("raygun-logs-{}.zip", now_ms());
        copy_out(
            &app,
            &crate::export::Place::Downloads,
            &name,
            "application/zip",
            &bytes,
        )
        .map(|saved| saved.shown)
    })
    .await
    .map_err(|error| error.to_string())?
}

/// Shows the logs folder in the file manager (desktop).
#[tauri::command]
#[specta::specta]
pub async fn logs_open_folder(app: AppHandle) -> Result<(), String> {
    let logs = app.state::<crate::startup::AppPaths>().0.logs();
    #[cfg(not(target_os = "android"))]
    {
        crate::export::output::reveal(&dir_in(&logs))
    }
    #[cfg(target_os = "android")]
    {
        let _ = logs;
        Err("Android has no file manager for Raygun's own folder: save the logs instead".into())
    }
}

/// Holds an extension back from loading, or lets it load again; extensions reload.
#[tauri::command]
#[specta::specta]
pub async fn extension_hold_back(app: AppHandle, id: String, held: bool) -> Result<(), String> {
    let db = app.state::<crate::startup::AppDb>().0.clone();
    let mut list = held_back(&db).await;
    if held {
        list.entry(id).or_insert_with(now_ms);
    } else {
        list.remove(&id);
    }
    set_held_back(&db, &list).await?;
    if let Some(service) = app.try_state::<std::sync::Arc<crate::extensions::ExtensionService>>() {
        service.set_held_back(
            list.into_keys()
                .map(|id| (id, HELD_BACK_REASON.to_owned()))
                .collect(),
        );
        service.reload().await;
    }
    Ok(())
}

/// Starts Raygun again as a safe start, its caches cleared first when asked (the error page).
#[tauri::command]
#[specta::specta]
pub async fn app_restart_safe(app: AppHandle, clear_caches: bool) -> Result<(), String> {
    let dir = crash_dir(&app);
    let _ = fs::create_dir_all(&dir);
    fs::write(dir.join(SAFE_ASKED), b"").map_err(|error| error.to_string())?;
    if clear_caches {
        fs::write(dir.join(CLEAR_CACHES), b"").map_err(|error| error.to_string())?;
    }
    restart(&app)
}

#[cfg(target_os = "android")]
fn restart(app: &AppHandle) -> Result<(), String> {
    app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
        .restart_app()
}

#[cfg(not(target_os = "android"))]
fn restart(app: &AppHandle) -> Result<(), String> {
    app.request_restart();
    Ok(())
}

/// What a safe start clears when the error page asked: the caches Raygun fills again by itself.
pub fn clear_caches(paths: &raygun_paths::Paths) {
    for kind in [
        raygun_paths::CacheKind::Artists,
        raygun_paths::CacheKind::Captions,
        raygun_paths::CacheKind::Dictionary,
    ] {
        let _ = fs::remove_dir_all(paths.cache(kind));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn folder() -> tempfile::TempDir {
        tempfile::tempdir().unwrap()
    }

    #[test]
    fn a_start_that_never_settled_makes_the_next_one_safe_and_names_its_sources() {
        let logs = folder();
        let dir = dir_in(logs.path());
        fs::create_dir_all(&dir).unwrap();
        // Nothing before: a normal start.
        let first = begin_without_hook(logs.path());
        assert_eq!(first.reason, None);
        // It stopped with two sources at work.
        fs::write(dir.join(RUNNING), "dev.a\ndev.b music.search").unwrap();
        let second = begin_without_hook(logs.path());
        assert_eq!(second.reason, Some(SafeStartReason::Stopped));
        assert_eq!(second.crashes, 1);
        assert_eq!(second.suspects, ["dev.a", "dev.b"]);
        assert!(second.held_back.is_empty(), "once isn't enough");
        // A record says so, since nothing else did.
        let list = records(&dir);
        assert_eq!(list.len(), 1);
        assert_eq!(list[0].kind, "start");
        // Stopped again with one of them at work: that one is held back.
        fs::write(dir.join(RUNNING), "dev.b music.search").unwrap();
        let third = begin_without_hook(logs.path());
        assert_eq!(third.crashes, 2);
        assert_eq!(third.held_back, ["dev.b"]);
        // It went well this time: the next start is a normal one.
        fs::remove_file(dir.join(MARKER)).unwrap();
        let fourth = begin_without_hook(logs.path());
        assert_eq!((fourth.reason, fourth.crashes), (None, 0));
    }

    #[test]
    fn flags_from_the_error_pages_make_a_safe_start_once() {
        let logs = folder();
        let dir = dir_in(logs.path());
        fs::create_dir_all(&dir).unwrap();
        fs::write(dir.join(RENDERER_GONE), "").unwrap();
        let start = begin_without_hook(logs.path());
        assert_eq!(start.reason, Some(SafeStartReason::Renderer));
        fs::remove_file(dir.join(MARKER)).unwrap();
        fs::write(dir.join(CLEAR_CACHES), "").unwrap();
        let start = begin_without_hook(logs.path());
        assert_eq!(start.reason, Some(SafeStartReason::Asked));
        assert!(start.clear_caches);
        assert!(start.script().contains("\"clearCaches\":true"));
        fs::remove_file(dir.join(MARKER)).unwrap();
        assert_eq!(begin_without_hook(logs.path()).reason, None);
    }

    #[test]
    fn records_list_newest_first_and_names_are_checked() {
        let logs = folder();
        let dir = dir_in(logs.path());
        fs::create_dir_all(&dir).unwrap();
        write_record(&dir, "panic", 1_000, "first", "body");
        write_record(&dir, "java", 2_000, "second\nline", "body");
        let list = records(&dir);
        assert_eq!(
            list.iter()
                .map(|record| (record.kind.as_str(), record.summary.as_str()))
                .collect::<Vec<_>>(),
            [("java", "second line"), ("panic", "first")]
        );
        assert!(checked(&dir, &list[0].id).is_ok());
        assert!(checked(&dir, "../secrets.db").is_err());
        assert!(checked(&dir, ".starting").is_err());
        let zip = logs_zip(logs.path()).unwrap();
        assert!(zip.len() > 100);
    }

    #[test]
    fn the_last_crash_is_the_last_runs_own_record_rather_than_the_fallback() {
        let record = |kind: &str, at: u64| CrashRecord {
            id: format!("{at}-{kind}.txt"),
            kind: kind.into(),
            at: at as f64,
            summary: String::new(),
        };
        // Newest first, as listed: this run's renderer record, then the last run's.
        let list = [
            record("renderer", 9_000),
            record("start", 5_000),
            record("java", 4_000),
            record("panic", 1_000),
        ];
        let last = |since, until| last_crash(&list, since, until).map(|record| record.kind);
        assert_eq!(last(2_000, 8_000).as_deref(), Some("java"));
        assert_eq!(last(4_500, 8_000).as_deref(), Some("start"));
        assert_eq!(last(6_000, 8_000), None);
        assert_eq!(last(0, 10_000).as_deref(), Some("renderer"));
    }

    /// [`begin`] without touching the process's global state (tests run side by side).
    fn begin_without_hook(logs: &Path) -> SafeStart {
        let dir = dir_in(logs);
        let previous: Option<Marker> = fs::read(dir.join(MARKER))
            .ok()
            .map(|bytes| serde_json::from_slice(&bytes).unwrap_or_default());
        let start = decide(&dir, previous);
        fs::write(dir.join(LAST_START), start.started_at.to_string()).unwrap();
        let marker = Marker {
            started_at: now_ms(),
            version: "test".into(),
            crashes: start.crashes,
            suspects: start.suspects.clone(),
        };
        fs::write(dir.join(MARKER), serde_json::to_vec(&marker).unwrap()).unwrap();
        start
    }
}
