//! Backups (M30, M46.51): making one of the categories chosen, in the usual place or a folder
//! picked (remembered for next time); finding backups wherever Raygun looks (its data folder,
//! `Download/Raygun`, the folders picked); restoring the categories chosen from one; and a
//! picked file, a Raygun backup or a Mihon one, told apart by content. Settings → Data's
//! categories (what each holds, clearing one) are `categories`, erasing everything `erase`.

pub mod categories;
pub mod erase;
mod format;
mod places;
mod values;

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};

use base64::Engine;
use raygun_db::backup as db_backup;
use raygun_model::{
    BackupDestinations, BackupEntry, BackupManifest, BackupOutcome, BackupPlace, BackupRequest,
    DataCategory,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::events::SyncChangedEvent;
use crate::export::Outputs;
use crate::manga::MangaService;
use crate::startup::AppDb;
use format::Decoded;
use places::{Folders, Found, Location, Target};

/// Backups kept in the data folder; making one more there removes the oldest. Backups elsewhere
/// are the user's to delete.
const KEEP: usize = 10;

pub struct BackupService {
    app: AppHandle,
    /// The data folder's `backups/`, which also holds the list of picked folders.
    dir: PathBuf,
    outputs: Outputs,
    /// The backups the last scan found, by id: only these are read, restored or deleted.
    found: Mutex<HashMap<String, Found>>,
    /// One change to the folder list at a time.
    folders_lock: tokio::sync::Mutex<()>,
}

impl BackupService {
    pub fn new(app: AppHandle, dir: PathBuf, outputs: Outputs) -> Self {
        Self {
            app,
            dir,
            outputs,
            found: Mutex::new(HashMap::new()),
            folders_lock: tokio::sync::Mutex::new(()),
        }
    }

    // ---- Where backups go ----

    pub async fn destinations(&self) -> BackupDestinations {
        let folders = Folders::load(&self.dir);
        let (usual_place, usual) = places::usual();
        BackupDestinations {
            usual: usual.to_owned(),
            usual_place,
            folders: folders.folders,
            last: folders.last,
        }
    }

    /// The system's folder picker; the folder chosen is remembered and used next. `None` when
    /// the picker was cancelled.
    pub async fn pick_folder(&self) -> Result<Option<BackupDestinations>, String> {
        let picked = crate::commands::pick_backup_folder(&self.app).await?;
        let Some(uri) = picked.uri else {
            return Ok(None);
        };
        let name = picked.name.unwrap_or_else(|| "Folder".into());
        {
            let _changing = self.folders_lock.lock().await;
            let mut folders = Folders::load(&self.dir);
            folders.remember(&uri, &name, now_ms() as f64);
            folders.save(&self.dir)?;
        }
        Ok(Some(self.destinations().await))
    }

    /// Stops looking in a folder (its backups stay where they are).
    pub async fn forget_folder(&self, uri: &str) -> Result<BackupDestinations, String> {
        {
            let _changing = self.folders_lock.lock().await;
            let mut folders = Folders::load(&self.dir);
            folders.forget(uri);
            folders.save(&self.dir)?;
        }
        Ok(self.destinations().await)
    }

    // ---- Making one ----

    pub async fn create(&self, request: BackupRequest) -> Result<BackupEntry, String> {
        let mut categories: Vec<DataCategory> = request
            .categories
            .into_iter()
            .filter(|category| category.backs_up())
            .collect();
        categories.sort();
        categories.dedup();
        if categories.is_empty() {
            return Err("Choose what to back up".into());
        }
        let target = match &request.folder {
            None => places::usual_target(),
            Some(uri) => {
                let folders = Folders::load(&self.dir);
                let folder = folders
                    .find(uri)
                    .ok_or("That folder isn't one Raygun saves backups in")?;
                Target::Folder {
                    uri: folder.uri.clone(),
                    name: folder.name.clone(),
                }
            }
        };
        let now = now_ms();
        let db = self.db();
        let parts: Vec<_> = categories
            .iter()
            .filter_map(|category| format::part_of(*category))
            .collect();
        let mut data = db_backup::export(&db, now, &parts)
            .await
            .map_err(to_message)?;
        let collected = values::collect(&self.app, &mut data, &categories, request.secrets).await?;
        let manifest = BackupManifest {
            version: db_backup::VERSION,
            created_at: now as f64,
            device_id: db.device_id().to_string(),
            device_name: crate::device_sync::device_name(&db).await,
            app_version: env!("CARGO_PKG_VERSION").to_owned(),
            platform: std::env::consts::OS.to_owned(),
            secrets: collected.secrets,
            sections: format::sections(&data, &categories, &collected.counts),
        };
        let name = format::file_name(now);
        let (outputs, dir, file_manifest) =
            (self.outputs.clone(), self.dir.clone(), manifest.clone());
        let found = tauri::async_runtime::spawn_blocking(move || {
            let bytes = format::encode(&file_manifest, &data)?;
            places::write(&outputs, &dir, &target, &name, &bytes)
        })
        .await
        .map_err(to_message)??;
        tracing::info!(name = %found.name, bytes = found.size, ?categories, "backup created");
        {
            let _changing = self.folders_lock.lock().await;
            let mut folders = Folders::load(&self.dir);
            match &request.folder {
                Some(uri) => {
                    let shown = found.folder.clone();
                    folders.remember(uri, &shown, now as f64);
                }
                None => folders.last = None,
            }
            folders.save(&self.dir)?;
        }
        if found.place == BackupPlace::App {
            self.prune().await;
        }
        let entry = entry(&found, Some(manifest), None);
        self.lock_found().insert(entry.id.clone(), found);
        Ok(entry)
    }

    /// Keeps the newest backups in the data folder.
    async fn prune(&self) {
        let dir = self.dir.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            let mut own: Vec<_> = places::list_dir(&dir, BackupPlace::App, "")
                .into_iter()
                .filter(|found| format::is_own_name(&found.name))
                .collect();
            // Names carry the time, so they sort by age.
            own.sort_by(|a, b| b.name.cmp(&a.name));
            for old in own.iter().skip(KEEP) {
                if let Location::Path(path) = &old.location {
                    let _ = std::fs::remove_file(path);
                }
            }
        })
        .await;
    }

    // ---- Finding them ----

    /// Every backup Raygun can find, newest first, each with what it says it holds.
    pub async fn scan(&self) -> Result<Vec<BackupEntry>, String> {
        let app = self.app.clone();
        let dir = self.dir.clone();
        let folders = Folders::load(&self.dir);
        let found = tauri::async_runtime::spawn_blocking(move || {
            let mut found = places::list_dir(&dir, BackupPlace::App, "Raygun");
            found.extend(outside(&app, &folders));
            let mut seen = std::collections::HashSet::new();
            found.retain(|file| seen.insert(file.location.id()));
            found
                .into_iter()
                .filter_map(|file| describe(&app, file))
                .collect::<Vec<_>>()
        })
        .await
        .map_err(to_message)?;
        let mut entries: Vec<BackupEntry> = found.iter().map(|(_, entry)| entry.clone()).collect();
        entries.sort_by(|a, b| {
            let time = |entry: &BackupEntry| {
                entry
                    .manifest
                    .as_ref()
                    .map_or(entry.modified_at, |manifest| manifest.created_at)
            };
            time(b).total_cmp(&time(a))
        });
        *self.lock_found() = found
            .into_iter()
            .map(|(file, entry)| (entry.id, file))
            .collect();
        Ok(entries)
    }

    // ---- Restoring ----

    /// Restores `categories` from a backup the last scan found (those it holds).
    pub async fn restore(
        &self,
        id: &str,
        categories: Vec<DataCategory>,
    ) -> Result<BackupOutcome, String> {
        let found = self.lookup(id)?;
        let app = self.app.clone();
        let bytes =
            tauri::async_runtime::spawn_blocking(move || places::read(&app, &found.location, None))
                .await
                .map_err(to_message)??;
        self.restore_bytes(bytes, Some(categories)).await
    }

    /// Restores or imports a file the user picked, sent as base64: everything a Raygun backup
    /// holds, or what a Mihon backup's library has.
    pub async fn import(&self, base64: &str) -> Result<BackupOutcome, String> {
        if base64.len() as u64 > format::MAX_FILE_BYTES / 3 * 4 + 4 {
            return Err("This file is too large to be a backup".into());
        }
        let bytes = base64::engine::general_purpose::STANDARD
            .decode(base64.trim())
            .map_err(|_| "The file couldn't be read".to_owned())?;
        self.restore_bytes(bytes, None).await
    }

    async fn restore_bytes(
        &self,
        bytes: Vec<u8>,
        categories: Option<Vec<DataCategory>>,
    ) -> Result<BackupOutcome, String> {
        let decoded = tauri::async_runtime::spawn_blocking(move || format::decode(&bytes))
            .await
            .map_err(to_message)??;
        match decoded {
            Decoded::Raygun { manifest, data } => {
                let held: Vec<DataCategory> = manifest
                    .sections
                    .iter()
                    .map(|section| section.category)
                    .collect();
                let chosen: Vec<DataCategory> = match categories {
                    Some(asked) => asked
                        .into_iter()
                        .filter(|category| held.contains(category))
                        .collect(),
                    None => held,
                };
                if chosen.is_empty() {
                    return Err("Choose what to restore".into());
                }
                self.restore_raygun(&data, &chosen).await
            }
            Decoded::Other(unpacked) => {
                if categories.is_some() {
                    return Err("This isn't a Raygun backup".into());
                }
                let mihon = crate::mihon::decode(&unpacked)?;
                let sources = self.app.state::<MangaService>().sources().await;
                let summary = crate::mihon::import(&self.db(), &mihon, &sources, now_ms())
                    .await
                    .map_err(to_message)?;
                tracing::info!(
                    titles = summary.titles,
                    skipped = summary.skipped.len(),
                    "mihon backup imported"
                );
                Ok(BackupOutcome::Mihon { summary })
            }
        }
    }

    async fn restore_raygun(
        &self,
        data: &db_backup::BackupData,
        categories: &[DataCategory],
    ) -> Result<BackupOutcome, String> {
        let parts: Vec<_> = categories
            .iter()
            .filter_map(|category| format::part_of(*category))
            .collect();
        let restored = db_backup::restore(&self.db(), data, &parts)
            .await
            .map_err(to_message)?;
        let brought = values::restore(&self.app, data, categories, &restored).await;
        tracing::info!(
            received = restored.received,
            changed = restored.changed,
            queued = brought.queued,
            ?categories,
            "backup restored"
        );
        // Cached settings, collections and history in the UI are stale, as after a sync.
        let _ = SyncChangedEvent(restored.changed.max(1) as u32).emit(&self.app);
        if restored.changed > 0 {
            // Restored settings take effect at once (the theme, sound, content filter…).
            crate::settings::apply_stored(&self.app);
        }
        Ok(BackupOutcome::Raygun {
            received: restored.received as u32,
            changed: restored.changed as u32,
            queued: brought.queued,
            problems: brought.problems,
        })
    }

    /// Deletes a backup the last scan found.
    pub async fn delete(&self, id: &str) -> Result<(), String> {
        let found = self.lookup(id)?;
        if found.place == BackupPlace::App && !format::is_own_name(&found.name) {
            return Err("Unknown backup".into());
        }
        let app = self.app.clone();
        let location = found.location.clone();
        tauri::async_runtime::spawn_blocking(move || places::delete(&app, &location))
            .await
            .map_err(to_message)??;
        self.lock_found().remove(id);
        Ok(())
    }

    fn lookup(&self, id: &str) -> Result<Found, String> {
        self.lock_found()
            .get(id)
            .cloned()
            .ok_or_else(|| "This backup isn't there anymore; look again".to_owned())
    }

    fn lock_found(&self) -> std::sync::MutexGuard<'_, HashMap<String, Found>> {
        self.found
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }
}

/// Backups outside the data folder: `Download/Raygun` and the folders picked. Blocking.
fn outside(app: &AppHandle, folders: &Folders) -> Vec<Found> {
    let mut found = Vec::new();
    #[cfg(target_os = "android")]
    {
        let (place, shown) = places::usual();
        match places::list_native(app, None, place, shown) {
            Ok(files) => found.extend(files),
            Err(error) => tracing::warn!(%error, "couldn't list Download/Raygun"),
        }
        for folder in &folders.folders {
            match places::list_native(app, Some(&folder.uri), BackupPlace::Folder, &folder.name) {
                Ok(files) => found.extend(files),
                Err(error) => {
                    tracing::warn!(%error, folder = %folder.name, "couldn't list a backup folder")
                }
            }
        }
    }
    #[cfg(not(target_os = "android"))]
    {
        let _ = app;
        if let Some(downloads) = raygun_paths::user_downloads() {
            found.extend(places::list_dir(
                &downloads.join("Raygun"),
                BackupPlace::Downloads,
                "Downloads/Raygun",
            ));
        }
        for folder in &folders.folders {
            found.extend(places::list_dir(
                std::path::Path::new(&folder.uri),
                BackupPlace::Folder,
                &folder.name,
            ));
        }
    }
    found
}

/// What a found file is: a Raygun backup with its manifest, one that couldn't be read (with
/// why), or `None` for another file named like one. Blocking.
fn describe(app: &AppHandle, file: Found) -> Option<(Found, BackupEntry)> {
    let head = match places::read(app, &file.location, Some(format::HEADER_BYTES)) {
        Ok(head) => head,
        Err(error) => {
            let entry = entry(&file, None, Some(error));
            return Some((file, entry));
        }
    };
    let described = match format::sniff(&head) {
        Ok(format::Sniffed::Described(manifest)) => Ok(manifest),
        // Version 1 files only describe themselves read in full.
        Ok(format::Sniffed::Packed) => {
            match places::read(app, &file.location, None).and_then(|bytes| format::decode(&bytes)) {
                Ok(Decoded::Raygun { manifest, .. }) => Ok(manifest),
                Ok(Decoded::Other(_)) => return None,
                Err(error) => Err(error),
            }
        }
        Ok(format::Sniffed::Other) => return None,
        Err(error) => Err(error),
    };
    let entry = match described {
        Ok(manifest) => entry(&file, Some(manifest), None),
        Err(problem) => entry(&file, None, Some(problem)),
    };
    Some((file, entry))
}

fn entry(file: &Found, manifest: Option<BackupManifest>, problem: Option<String>) -> BackupEntry {
    BackupEntry {
        id: file.location.id(),
        name: file.name.clone(),
        place: file.place,
        folder: file.folder.clone(),
        size_bytes: file.size as f64,
        modified_at: file.modified_ms,
        manifest,
        problem,
    }
}

fn now_ms() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|elapsed| elapsed.as_millis() as i64)
        .unwrap_or_default()
}

fn to_message(error: impl std::fmt::Display) -> String {
    error.to_string()
}
