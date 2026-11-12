//! One Kodi add-on's Python: a RustPython interpreter without the host's environment (its
//! `host_env` feature is off: no files, sockets, processes or environment of its own), given
//! only what [`Sandbox`] holds: the add-on's files ([`files`]), Raygun's HTTP ([`net`]), SQLite
//! ([`sqlite`]) and a clock that ends each run on time. Raygun's `xbmc*` modules and stand-ins
//! for the rest of Kodi are `../lib`, compiled in.

pub(crate) mod files;
mod native;
pub(crate) mod net;
mod sqlite;

use std::collections::{BTreeMap, HashMap, VecDeque};
use std::path::PathBuf;
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex, PoisonError};
use std::time::{Duration, Instant};

use rustpython_vm::compiler::Mode;
use rustpython_vm::signal::{UserSignalSender, user_signal_channel};
use rustpython_vm::{AsObject, Interpreter, PyObjectRef, PyResult, Settings, VirtualMachine};
use serde::{Deserialize, Serialize};

use files::{ADDONS_ROOT, EXTRAS, Files, Found, RAYGUN_ROOT, STDLIB_ROOT};

/// What a run that ran out of time says.
pub(crate) const TIMED_OUT: &str = "the add-on took too long";
/// Lines of the add-on's log kept for an error's details.
const LOG_LINES: usize = 40;

/// Everything one add-on's interpreter may reach.
pub(crate) struct Sandbox {
    pub files: Files,
    pub fetcher: net::Fetcher,
    /// The extension's id, for Raygun's log.
    label: String,
    /// Where module search starts: Raygun's modules, the add-on, its libraries.
    roots: Vec<String>,
    /// When the current run must end.
    deadline: Mutex<Option<Instant>>,
    log: Mutex<VecDeque<String>>,
    requests: AtomicU64,
    databases: Mutex<HashMap<i64, sqlite::Database>>,
    next_database: AtomicI64,
}

impl Sandbox {
    /// `addon` is the plugin's folder and `entry` its script within it; `libraries` its
    /// dependencies' folders with the folder of each that goes on `sys.path`; `data` its writable
    /// folder (`None` keeps files in memory).
    pub fn new(
        label: &str,
        addon: (&str, PathBuf),
        entry: &str,
        libraries: &[(String, PathBuf, String)],
        data: Option<PathBuf>,
        loopback: bool,
    ) -> Self {
        let mut mounts = vec![(addon.0.to_owned(), addon.1.clone())];
        // As Python runs a script: its own folder first, then the add-on's (Kodi's).
        let mut roots = Vec::new();
        if let Some((folder, _)) = entry.trim_matches('/').rsplit_once('/') {
            roots.push(format!("{ADDONS_ROOT}/{}/{folder}", addon.0));
        }
        roots.push(format!("{ADDONS_ROOT}/{}", addon.0));
        for (id, folder, library) in libraries {
            mounts.push((id.clone(), folder.clone()));
            let library = library.trim_matches('/');
            roots.push(if library.is_empty() || library == "." {
                format!("{ADDONS_ROOT}/{id}")
            } else {
                format!("{ADDONS_ROOT}/{id}/{library}")
            });
        }
        let files = Files::new(&mounts, data.as_deref());
        // The folders Kodi makes for an add-on before it runs.
        let _ = files.mkdir("/kodi/home/userdata/addon_data");
        for (id, _) in &mounts {
            let _ = files.mkdir(&format!("/kodi/home/userdata/addon_data/{id}"));
        }
        Self {
            files,
            fetcher: net::Fetcher::new(loopback),
            label: label.to_owned(),
            roots,
            deadline: Mutex::new(None),
            log: Mutex::new(VecDeque::new()),
            requests: AtomicU64::new(0),
            databases: Mutex::new(HashMap::new()),
            next_database: AtomicI64::new(1),
        }
    }

    /// A top-level module: Raygun's (shadowed by nothing), then the add-on's and its libraries',
    /// then Raygun's stand-ins for libraries ([`EXTRAS`]), then the standard library.
    pub fn find_top(&self, name: &str) -> Option<Found> {
        let raygun = [RAYGUN_ROOT.to_owned()];
        if !EXTRAS.contains(&name)
            && let Some(found) = self.files.find_in(&raygun, name)
        {
            return Some(found);
        }
        let mut namespace = Vec::new();
        for found in [
            self.files.find_in(&self.roots, name),
            EXTRAS
                .contains(&name)
                .then(|| self.files.find_in(&raygun, name))
                .flatten(),
            self.files.find_in(&[STDLIB_ROOT.to_owned()], name),
        ]
        .into_iter()
        .flatten()
        {
            match found {
                Found::Namespace(folders) => namespace.extend(folders),
                regular => return Some(regular),
            }
        }
        (!namespace.is_empty()).then_some(Found::Namespace(namespace))
    }

    /// How long a wait of `asked` may take within the run: all of it, what's left of the run, or
    /// `None` when nothing is left, or no run is going on (a thread the add-on left running
    /// doesn't reach the network or wait between runs).
    pub fn time_left(&self, asked: Duration) -> Option<Duration> {
        let deadline = (*self.deadline.lock().unwrap_or_else(PoisonError::into_inner))?;
        let left = deadline.saturating_duration_since(Instant::now());
        (!left.is_zero()).then(|| asked.min(left))
    }

    fn set_deadline(&self, deadline: Option<Instant>) {
        *self.deadline.lock().unwrap_or_else(PoisonError::into_inner) = deadline;
    }

    /// A line of the add-on's log (Kodi's levels: 0 debug ... 4 fatal).
    pub fn note(&self, level: i64, line: &str) {
        let line: String = line.chars().take(1000).collect();
        if level >= 3 {
            tracing::info!(addon = %self.label, "{line}");
        } else {
            tracing::debug!(addon = %self.label, "{line}");
        }
        let mut log = self.log.lock().unwrap_or_else(PoisonError::into_inner);
        if log.len() >= LOG_LINES {
            log.pop_front();
        }
        log.push_back(line);
    }

    fn take_log(&self) -> Vec<String> {
        self.log
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .drain(..)
            .collect()
    }

    pub fn note_request(&self, url: &str) {
        self.requests.fetch_add(1, Ordering::Relaxed);
        tracing::trace!(addon = %self.label, %url, "request");
    }

    pub fn keep_database(&self, database: sqlite::Database) -> i64 {
        let handle = self.next_database.fetch_add(1, Ordering::Relaxed);
        self.databases
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .insert(handle, database);
        handle
    }

    pub fn drop_database(&self, handle: i64) {
        self.databases
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .remove(&handle);
    }

    pub fn with_database<T>(
        &self,
        handle: i64,
        work: impl FnOnce(&sqlite::Database) -> T,
    ) -> Option<T> {
        let databases = self
            .databases
            .lock()
            .unwrap_or_else(PoisonError::into_inner);
        databases.get(&handle).map(work)
    }
}

/// One run of the plugin, as `lib/_raygun_boot.py` reads it.
#[derive(Debug, Clone, Default, Serialize)]
pub(crate) struct RunRequest {
    /// The `plugin://` address.
    pub url: String,
    pub handle: i64,
    /// The plugin's Kodi id.
    pub addon: String,
    /// Its script, as a Kodi path.
    pub entry: String,
    /// The plugin and its libraries: details and settings, by Kodi id.
    pub addons: BTreeMap<String, serde_json::Value>,
    pub language: String,
    pub platform: String,
    /// The words a keyboard or input dialog is answered with.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub keyboard: Option<String>,
    /// The choices `Dialog().select` is answered with, in turn.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub select: Vec<i64>,
    /// The first unanswered choice is noted and declined (its options become servers).
    pub probe: bool,
}

/// One listed item (`addDirectoryItem`), resolved item or played item.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Item {
    pub url: String,
    pub folder: bool,
    pub label: String,
    pub label2: String,
    pub path: String,
    pub art: BTreeMap<String, String>,
    pub info: serde_json::Map<String, serde_json::Value>,
    pub properties: BTreeMap<String, String>,
    pub mime: String,
    pub subtitles: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Resolved {
    pub ok: bool,
    pub item: Item,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Played {
    pub items: Vec<Item>,
}

/// A choice the plugin asked for (`Dialog().select`).
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct Select {
    pub heading: String,
    pub options: Vec<String>,
}

#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct PyError {
    #[serde(rename = "type")]
    pub kind: String,
    pub message: String,
    pub traceback: String,
}

/// What a run did.
#[derive(Debug, Clone, Default, PartialEq, Deserialize)]
#[serde(default)]
pub(crate) struct RunResult {
    pub items: Vec<Item>,
    pub ended: bool,
    pub succeeded: bool,
    pub content: String,
    pub category: String,
    pub resolved: Option<Resolved>,
    pub redirect: Option<String>,
    pub played: Option<Played>,
    pub selects: Vec<Select>,
    pub messages: Vec<String>,
    /// Settings the plugin changed, by add-on.
    pub settings: BTreeMap<String, BTreeMap<String, String>>,
    pub drm: Option<String>,
    pub error: Option<PyError>,
    /// The add-on's last log lines (filled in Rust).
    #[serde(skip)]
    pub log: Vec<String>,
}

/// A running interpreter.
pub(crate) struct Python {
    interpreter: Interpreter,
    boot: PyObjectRef,
    sandbox: Arc<Sandbox>,
    interrupt: UserSignalSender,
    /// Which run an interrupt is for.
    generation: Arc<AtomicU64>,
}

/// The source of `lib/_raygun_boot.py`.
const BOOT: &str = include_str!("../lib/_raygun_boot.py");
const BOOT_PATH: &str = "/kodi/python/raygun/_raygun_boot.py";

impl Python {
    /// Starts an interpreter for one add-on.
    pub fn start(sandbox: Arc<Sandbox>) -> Result<Self, String> {
        let mut settings = Settings::default();
        settings.isolated = true;
        settings.ignore_environment = true;
        settings.import_site = false;
        settings.install_signal_handlers = false;
        settings.write_bytecode = false;
        settings.safe_path = true;
        settings.utf8_mode = 1;
        settings.user_site_directory = false;
        settings.argv = vec![String::new()];
        settings.path_list = Vec::new();
        let builder = Interpreter::builder(settings);
        let mut modules = rustpython_stdlib::stdlib_module_defs(&builder.ctx);
        modules.push(native::raygun::module_def(&builder.ctx));
        modules.push(native::posix::module_def(&builder.ctx));
        modules.push(native::raygun_sqlite::module_def(&builder.ctx));
        let (interrupt, receiver) = user_signal_channel();
        let interpreter = builder
            .add_native_modules(&modules)
            .init_hook(move |vm: &mut VirtualMachine| vm.set_user_signal_channel(receiver))
            .build();
        let boot = interpreter
            .enter(|vm| -> PyResult<PyObjectRef> {
                native::attach(vm, sandbox.clone())?;
                let module = vm.new_module("_raygun_boot", vm.ctx.new_dict(), None);
                module.set_attr("__file__", vm.ctx.new_str(BOOT_PATH), vm)?;
                let modules = vm.sys_module.get_attr("modules", vm)?;
                modules.set_item("_raygun_boot", module.clone().into(), vm)?;
                let code = vm
                    .compile(BOOT, Mode::Exec, BOOT_PATH)
                    .map_err(|error| error.into_pyexception(vm, Some(BOOT)))?;
                vm.run_code_obj(
                    code,
                    rustpython_vm::scope::Scope::with_builtins(None, module.dict(), vm),
                )?;
                Ok(module.into())
            })
            .map_err(|error| interpreter.enter(|vm| describe(vm, &error)))?;
        Ok(Self {
            interpreter,
            boot,
            sandbox,
            interrupt,
            generation: Arc::new(AtomicU64::new(0)),
        })
    }

    /// Runs the plugin once, for at most `budget`.
    pub fn run(&self, request: &RunRequest, budget: Duration) -> Result<RunResult, String> {
        let text = serde_json::to_string(request).map_err(|error| error.to_string())?;
        let generation = self.generation.fetch_add(1, Ordering::SeqCst) + 1;
        let deadline = Instant::now() + budget;
        self.sandbox.set_deadline(Some(deadline));
        let _ = self.sandbox.take_log();
        // Code that never waits on Raygun (a loop) is stopped from outside once the run is over.
        let watchdog = Watchdog::start(
            self.interrupt.clone(),
            self.generation.clone(),
            generation,
            deadline,
        );
        let answer = self.interpreter.enter(|vm| -> Result<String, String> {
            let run = self
                .boot
                .get_attr("run", vm)
                .map_err(|error| describe(vm, &error))?;
            let value = run
                .call((vm.ctx.new_str(text),), vm)
                .map_err(|error| describe(vm, &error))?;
            value
                .str(vm)
                .map(|text| text.as_wtf8().to_string())
                .map_err(|error| describe(vm, &error))
        });
        drop(watchdog);
        self.generation.fetch_add(1, Ordering::SeqCst);
        self.sandbox.set_deadline(None);
        let timed_out = Instant::now() >= deadline;
        let mut result: RunResult = match answer {
            Ok(text) => serde_json::from_str(&text)
                .map_err(|error| format!("the add-on's answer couldn't be read: {error}"))?,
            Err(error) if timed_out => return Err(format!("{TIMED_OUT}\n{error}")),
            Err(error) => return Err(error),
        };
        result.log = self.sandbox.take_log();
        if timed_out && let Some(error) = result.error.as_mut() {
            error.message = format!("{TIMED_OUT}: {}", error.message);
        }
        Ok(result)
    }
}

/// An exception as text, with its traceback.
fn describe(vm: &VirtualMachine, error: &rustpython_vm::builtins::PyBaseExceptionRef) -> String {
    let mut text = String::new();
    if vm.write_exception(&mut text, error).is_err() || text.is_empty() {
        text = error
            .as_object()
            .str(vm)
            .map(|text| text.as_wtf8().to_string())
            .unwrap_or_else(|_| "an error in the add-on".to_owned());
    }
    text
}

/// Interrupts the interpreter once the run's time is up, until the run ends.
struct Watchdog {
    stop: Option<std::sync::mpsc::Sender<()>>,
    thread: Option<std::thread::JoinHandle<()>>,
}

impl Watchdog {
    fn start(
        interrupt: UserSignalSender,
        current: Arc<AtomicU64>,
        generation: u64,
        deadline: Instant,
    ) -> Self {
        let (stop, stopped) = std::sync::mpsc::channel::<()>();
        let thread = std::thread::Builder::new()
            .name("kodi watchdog".into())
            .spawn(move || {
                let wait = deadline.saturating_duration_since(Instant::now());
                if stopped.recv_timeout(wait).is_ok() {
                    return;
                }
                // Signals are checked between instructions of whichever interpreter looks
                // first: send again until the run notices.
                loop {
                    let current = current.clone();
                    let sent = interrupt.send(Box::new(move |vm: &VirtualMachine| {
                        if current.load(Ordering::SeqCst) == generation {
                            Err(vm.new_exception_msg(
                                vm.ctx.exceptions.keyboard_interrupt.to_owned(),
                                TIMED_OUT.into(),
                            ))
                        } else {
                            Ok(())
                        }
                    }));
                    if sent.is_err() {
                        return;
                    }
                    match stopped.recv_timeout(Duration::from_millis(250)) {
                        Err(std::sync::mpsc::RecvTimeoutError::Timeout) => {}
                        _ => return,
                    }
                }
            })
            .ok();
        Self {
            stop: Some(stop),
            thread,
        }
    }
}

impl Drop for Watchdog {
    fn drop(&mut self) {
        drop(self.stop.take());
        if let Some(thread) = self.thread.take() {
            let _ = thread.join();
        }
    }
}
