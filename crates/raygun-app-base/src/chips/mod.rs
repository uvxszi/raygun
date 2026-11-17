//! Smart Chips (M46.57): optional features installed like extensions. The catalogue
//! (`catalog.json`, built in) names each chip's package, pinned by SHA-256 like `raygun-ai`'s
//! models; installing downloads it, checks it and unpacks it under `chips/<id>/`. A chip's
//! switch is `Settings::chips` (synced); what's installed is this device's own. Code a chip gates
//! asks [`enabled`]: installed and on.
//!
//! Packages are `.tar.gz` files in this repository's `chips/packages/`, served from GitHub raw and
//! rebuilt with `tools/build_chips.py`, which writes their sizes and hashes into the catalogue.

pub mod commands;
mod package;

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex, RwLock};

use raygun_model::{ChipId, ChipInfo, ChipPrefs, ChipProgress};
use serde::Deserialize;
use tauri::Manager;
use tokio_util::sync::CancellationToken;

pub use package::PackageError;

/// Written into an installed chip's folder: its package's SHA-256, so a catalogue naming another
/// package sees it as outdated.
const INSTALLED: &str = ".package";

/// A chip as the catalogue lists it.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChipEntry {
    pub id: String,
    pub name: String,
    pub description: String,
    pub kind: String,
    pub version: String,
    pub package: PackageFile,
}

/// A chip's package: a `.tar.gz` whose files become the chip's folder.
#[derive(Debug, Clone, Deserialize)]
pub struct PackageFile {
    pub url: String,
    pub sha256: String,
    pub size: u64,
}

/// The built-in catalogue.
pub fn catalog() -> Vec<ChipEntry> {
    serde_json::from_str(include_str!("catalog.json"))
        .expect("the built-in chip catalogue is valid")
}

/// What runs when chips change, set by the app at start (its services live above this crate):
/// features a chip gates follow it at once.
pub type ChipsChanged = fn(&tauri::AppHandle);

static CHANGED: std::sync::OnceLock<ChipsChanged> = std::sync::OnceLock::new();

pub fn set_changed_hook(hook: ChipsChanged) {
    let _ = CHANGED.set(hook);
}

type Notify = Box<dyn Fn(Vec<ChipInfo>) + Send + Sync>;

pub struct ChipsService {
    dir: PathBuf,
    catalog: Vec<ChipEntry>,
    client: reqwest::Client,
    prefs: RwLock<ChipPrefs>,
    /// Downloads under way (their cancel) and the last failure, by chip.
    work: Mutex<HashMap<ChipId, Work>>,
    /// Installed packages' hashes by chip, read at start and kept as installs change.
    installed: RwLock<HashMap<ChipId, String>>,
    notify: Notify,
}

#[derive(Debug, Clone)]
enum Work {
    Downloading {
        received: u64,
        total: u64,
        cancel: CancellationToken,
    },
    Failed(String),
}

impl ChipsService {
    /// `dir` is the chips' folder (`<data>/chips`); `notify` hears every change.
    pub fn new(
        dir: PathBuf,
        catalog: Vec<ChipEntry>,
        client: reqwest::Client,
        prefs: ChipPrefs,
        notify: impl Fn(Vec<ChipInfo>) + Send + Sync + 'static,
    ) -> Arc<Self> {
        let installed = catalog
            .iter()
            .filter_map(|entry| {
                let id = ChipId::parse(&entry.id)?;
                let hash = std::fs::read_to_string(dir.join(&entry.id).join(INSTALLED)).ok()?;
                Some((id, hash.trim().to_owned()))
            })
            .collect();
        Arc::new(Self {
            dir,
            catalog,
            client,
            prefs: RwLock::new(prefs),
            work: Mutex::new(HashMap::new()),
            installed: RwLock::new(installed),
            notify: Box::new(notify),
        })
    }

    /// The service for the app: the built-in catalogue, the stored switches, events to the UI.
    pub fn for_app(app: &tauri::AppHandle, dir: PathBuf, prefs: ChipPrefs) -> Arc<Self> {
        use tauri_specta::Event;
        let client = raygun_net::http_client().unwrap_or_default();
        let events = app.clone();
        // What's usable now: the hook runs only when that changes, not for a download's progress.
        let usable = Mutex::new(None::<Vec<ChipId>>);
        Self::new(dir, catalog(), client, prefs, move |chips| {
            let now: Vec<ChipId> = chips
                .iter()
                .filter(|chip| chip.installed && chip.on)
                .map(|chip| chip.id)
                .collect();
            let changed = usable
                .lock()
                .expect("usable poisoned")
                .replace(now.clone())
                .is_some_and(|before| before != now);
            if let Err(error) = crate::events::ChipsEvent(chips).emit(&events) {
                tracing::warn!(%error, "couldn't announce the chips");
            }
            if changed && let Some(hook) = CHANGED.get() {
                hook(&events);
            }
        })
    }

    fn entry(&self, chip: ChipId) -> Option<&ChipEntry> {
        self.catalog.iter().find(|entry| entry.id == chip.as_str())
    }

    /// The chip's folder (whether or not it's installed).
    pub fn folder(&self, chip: ChipId) -> PathBuf {
        self.dir.join(chip.as_str())
    }

    pub fn is_installed(&self, chip: ChipId) -> bool {
        self.installed
            .read()
            .expect("installed poisoned")
            .contains_key(&chip)
    }

    /// Installed and switched on.
    pub fn is_enabled(&self, chip: ChipId) -> bool {
        self.is_installed(chip) && self.prefs.read().expect("prefs poisoned").is_on(chip)
    }

    /// The switches changed elsewhere (a sync, a restored backup, Settings).
    pub fn set_prefs(&self, prefs: ChipPrefs) {
        let changed = {
            let mut held = self.prefs.write().expect("prefs poisoned");
            let changed = *held != prefs;
            *held = prefs;
            changed
        };
        if changed {
            self.announce();
        }
    }

    pub fn prefs(&self) -> ChipPrefs {
        self.prefs.read().expect("prefs poisoned").clone()
    }

    pub fn list(&self) -> Vec<ChipInfo> {
        let installed = self.installed.read().expect("installed poisoned").clone();
        let work = self.work.lock().expect("work poisoned").clone();
        let prefs = self.prefs();
        self.catalog
            .iter()
            .filter_map(|entry| {
                let id = ChipId::parse(&entry.id)?;
                let hash = installed.get(&id);
                Some(ChipInfo {
                    id,
                    name: entry.name.clone(),
                    description: entry.description.clone(),
                    kind: entry.kind.clone(),
                    version: entry.version.clone(),
                    size: entry.package.size as f64,
                    installed: hash.is_some(),
                    outdated: hash.is_some_and(|hash| *hash != entry.package.sha256),
                    on: prefs.is_on(id),
                    progress: match work.get(&id) {
                        None => ChipProgress::Idle,
                        Some(Work::Downloading {
                            received, total, ..
                        }) => ChipProgress::Downloading {
                            received: *received as f64,
                            total: *total as f64,
                        },
                        Some(Work::Failed(message)) => ChipProgress::Failed {
                            message: message.clone(),
                        },
                    },
                })
            })
            .collect()
    }

    fn announce(&self) {
        (self.notify)(self.list());
    }

    /// Downloads, checks and unpacks a chip's package (again, for an update). Runs until it's
    /// done; progress and the result go out as changes.
    pub async fn install(self: &Arc<Self>, chip: ChipId) -> Result<(), PackageError> {
        let entry = self
            .entry(chip)
            .cloned()
            .ok_or_else(|| PackageError::Unknown(chip.as_str().to_owned()))?;
        let cancel = CancellationToken::new();
        {
            let mut work = self.work.lock().expect("work poisoned");
            if matches!(work.get(&chip), Some(Work::Downloading { .. })) {
                return Ok(());
            }
            work.insert(
                chip,
                Work::Downloading {
                    received: 0,
                    total: entry.package.size,
                    cancel: cancel.clone(),
                },
            );
        }
        self.announce();
        let service = self.clone();
        let mut last_told = std::time::Instant::now();
        let result = package::install(
            &self.client,
            &self.dir,
            &entry.id,
            &entry.package,
            &cancel,
            |received, total| {
                if let Some(Work::Downloading {
                    received: held,
                    total: held_total,
                    ..
                }) = service.work.lock().expect("work poisoned").get_mut(&chip)
                {
                    *held = received;
                    *held_total = total;
                }
                // A few changes a second are enough for a progress bar.
                if last_told.elapsed() >= std::time::Duration::from_millis(250) {
                    last_told = std::time::Instant::now();
                    service.announce();
                }
            },
        )
        .await;
        {
            let mut work = self.work.lock().expect("work poisoned");
            match &result {
                Ok(()) | Err(PackageError::Cancelled) => work.remove(&chip),
                Err(error) => work.insert(chip, Work::Failed(error.to_string())),
            };
        }
        if result.is_ok() {
            self.installed
                .write()
                .expect("installed poisoned")
                .insert(chip, entry.package.sha256.clone());
        }
        self.announce();
        result
    }

    /// Stops a chip's download; nothing of it is kept.
    pub fn cancel(&self, chip: ChipId) {
        if let Some(Work::Downloading { cancel, .. }) =
            self.work.lock().expect("work poisoned").get(&chip)
        {
            cancel.cancel();
        }
    }

    /// Removes an installed chip (its switch stays as it was for the next install).
    pub async fn remove(&self, chip: ChipId) -> Result<(), PackageError> {
        self.cancel(chip);
        self.installed
            .write()
            .expect("installed poisoned")
            .remove(&chip);
        self.work.lock().expect("work poisoned").remove(&chip);
        let result = match tokio::fs::remove_dir_all(self.folder(chip)).await {
            Err(error) if error.kind() != std::io::ErrorKind::NotFound => Err(error.into()),
            _ => Ok(()),
        };
        self.announce();
        result
    }

    /// A file of an installed, switched-on chip, read as text (the desktop player's shaders).
    pub async fn read_text(&self, chip: ChipId, name: &str) -> Result<String, String> {
        if !self.is_enabled(chip) {
            return Err(format!("{} isn't installed and on", chip.as_str()));
        }
        let path = file_in(&self.folder(chip), name)
            .ok_or_else(|| format!("{name} isn't a file of {}", chip.as_str()))?;
        tokio::fs::read_to_string(path)
            .await
            .map_err(|error| error.to_string())
    }
}

/// `name` inside `folder` when it only goes down (`a/b.glsl`), never out of it.
pub fn file_in(folder: &Path, name: &str) -> Option<PathBuf> {
    let relative = Path::new(name);
    relative
        .components()
        .all(|component| matches!(component, std::path::Component::Normal(_)))
        .then(|| folder.join(relative))
        .filter(|_| !name.is_empty())
}

/// Whether `chip` is installed on this device and switched on: what a feature it gates checks.
pub fn enabled(app: &tauri::AppHandle, chip: ChipId) -> bool {
    app.try_state::<Arc<ChipsService>>()
        .is_some_and(|chips| chips.is_enabled(chip))
}

/// The folder of `chip` when it's installed and on.
pub fn folder_if_enabled(app: &tauri::AppHandle, chip: ChipId) -> Option<PathBuf> {
    let chips = app.try_state::<Arc<ChipsService>>()?;
    chips.is_enabled(chip).then(|| chips.folder(chip))
}

#[cfg(test)]
mod tests;
