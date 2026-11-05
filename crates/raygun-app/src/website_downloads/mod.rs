//! Files saved from sites in Raygun's built-in browser.
//!
//! The browser never hands a download to another app. It asks where the file goes (Download/
//! Raygun or a picked folder) and passes the request here: on Android from its own process `:web`
//! in the package-only broadcast its reports use, on desktop from the site window's
//! `DownloadStarting`. Rust downloads it with the page's cookies, User-Agent and Referer, a few
//! at a time; stops, failures and restarts carry on where they were when the server takes
//! ranges. Finished files are moved into place (on Android through the plugin's `Storage`), under
//! a name kept apart from files already there. The list is kept in `local_meta`, shown in
//! Downloads → Files and in the download notifications; the browser follows the ones it started
//! ([`browser`]).

pub mod browser;
#[cfg(not(target_os = "android"))]
pub mod desktop;
pub mod fetch;
pub mod names;
#[cfg(test)]
mod tests;

use std::path::{Path, PathBuf};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_model::{WebsiteDownload, WebsiteDownloadState};
use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::{Notify, watch};

use crate::catalog::now_ms;
use crate::events::WebsiteDownloadsEvent;
use crate::startup::AppDb;
use fetch::{Failure, Origin, Validator};

const STORE_KEY: &str = "web.downloads";
/// Downloads running at once; the rest wait.
const RUNNING: usize = 3;
/// Tries after the first for a download that broke off (2, 4, 8 s apart).
const RETRIES: u32 = 3;
/// Finished entries kept in the list.
const KEEP: usize = 200;
/// Progress events per download, at most this often.
const PROGRESS_EVERY: Duration = Duration::from_millis(250);

/// Where a finished file goes.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum Destination {
    /// `Download/Raygun` (the Downloads folder's `Raygun` on desktop).
    Downloads,
    /// A picked folder: its tree URI (Android) or path (desktop), and its name.
    Folder { uri: String, name: String },
}

impl Destination {
    fn shown(&self) -> String {
        match self {
            Self::Downloads if cfg!(target_os = "android") => "Download/Raygun".into(),
            Self::Downloads => "Downloads/Raygun".into(),
            Self::Folder { name, .. } => name.clone(),
        }
    }

    fn place(&self) -> crate::export::Place {
        match self {
            Self::Downloads => crate::export::Place::Downloads,
            Self::Folder { uri, name } => crate::export::Place::Folder {
                uri: uri.clone(),
                name: name.clone(),
            },
        }
    }
}

/// What the browser hands over when a page starts a download.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct FileRequest {
    pub url: String,
    #[serde(default)]
    pub user_agent: Option<String>,
    /// The page that started it.
    #[serde(default)]
    pub referer: Option<String>,
    #[serde(default)]
    pub cookies: Option<String>,
    #[serde(default)]
    pub content_disposition: Option<String>,
    /// The browser's guess at the name.
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub mime: Option<String>,
    #[serde(default)]
    pub size: Option<f64>,
    pub destination: Destination,
    /// The browser's name for it: its changes go back to the browser under it ([`browser`]).
    #[serde(default)]
    pub ticket: Option<String>,
}

impl FileRequest {
    /// The name it's saved under (before telling it apart from files already there).
    pub fn file_name(&self) -> String {
        names::file_name(
            &self.url,
            self.content_disposition.as_deref(),
            self.name.as_deref(),
            self.mime.as_deref(),
        )
    }
}

/// One download as kept between runs.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct Stored {
    item: WebsiteDownload,
    origin: Origin,
    destination: Destination,
    #[serde(default)]
    validator: Validator,
    /// The finished file: its path (desktop) or content URI (Android).
    #[serde(default)]
    saved: Option<String>,
    /// The browser's name for it, when the browser started it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    ticket: Option<String>,
}

struct Entry {
    stored: Stored,
    /// Set while it runs.
    cancel: Option<watch::Sender<bool>>,
}

pub struct WebsiteDownloadService {
    app: AppHandle,
    /// Partial files, by download id.
    partials: PathBuf,
    client: reqwest::Client,
    entries: Mutex<Vec<Entry>>,
    /// Wakes the task that stores the list.
    save: Notify,
    /// Finished files are put in place one at a time, so two never take the same name.
    placing: Mutex<()>,
    /// Changes the browser hasn't heard yet.
    feed: browser::Feed,
}

impl WebsiteDownloadService {
    pub fn new(app: AppHandle, dir: PathBuf, client: reqwest::Client) -> Arc<Self> {
        Arc::new(Self {
            app,
            partials: dir,
            client,
            entries: Mutex::new(Vec::new()),
            save: Notify::new(),
            placing: Mutex::new(()),
            feed: browser::Feed::default(),
        })
    }

    fn db(&self) -> Arc<raygun_db::Db> {
        self.app.state::<AppDb>().inner().0.clone()
    }

    /// Reads the kept list (downloads left running carry on) and starts storing changes and
    /// telling the browser about its downloads.
    pub fn start(self: &Arc<Self>) {
        let feed = self.clone();
        tauri::async_runtime::spawn(async move { feed.feed().await });
        let service = self.clone();
        tauri::async_runtime::spawn(async move {
            let kept: Vec<Stored> = raygun_db::local_value(&service.db(), STORE_KEY)
                .await
                .ok()
                .flatten()
                .and_then(|json| serde_json::from_str(&json).ok())
                .unwrap_or_default();
            {
                let mut entries = service.entries.lock().expect("downloads poisoned");
                let added: Vec<Entry> = std::mem::take(&mut *entries);
                *entries = kept
                    .into_iter()
                    .map(|mut stored| {
                        if stored.item.state == WebsiteDownloadState::Downloading {
                            stored.item.state = WebsiteDownloadState::Queued;
                        }
                        stored.item.bytes_per_second = None;
                        Entry {
                            stored,
                            cancel: None,
                        }
                    })
                    .collect();
                // Ones added before the list was read come first.
                for entry in added.into_iter().rev() {
                    entries.insert(0, entry);
                }
            }
            service.changed("");
            service.pump();
            loop {
                service.save.notified().await;
                // Coalesces a burst of changes.
                tokio::time::sleep(Duration::from_millis(300)).await;
                let snapshot: Vec<Stored> = service
                    .entries
                    .lock()
                    .expect("downloads poisoned")
                    .iter()
                    .map(|entry| entry.stored.clone())
                    .collect();
                match serde_json::to_string(&snapshot) {
                    Ok(json) => {
                        if let Err(error) =
                            raygun_db::set_local_value(&service.db(), STORE_KEY, &json).await
                        {
                            tracing::warn!(%error, "file downloads not stored");
                        }
                    }
                    Err(error) => tracing::warn!(%error, "file downloads not stored"),
                }
            }
        });
    }

    /// The downloads, newest first.
    pub fn list(&self) -> Vec<WebsiteDownload> {
        self.entries
            .lock()
            .expect("downloads poisoned")
            .iter()
            .map(|entry| entry.stored.item.clone())
            .collect()
    }

    /// A download the browser asked for; it starts when one of the running slots is free.
    pub fn add(self: &Arc<Self>, request: FileRequest) -> String {
        let id = uuid::Uuid::now_v7().to_string();
        let name = request.file_name();
        let site = request
            .referer
            .as_deref()
            .or(Some(request.url.as_str()))
            .and_then(|address| url::Url::parse(address).ok())
            .and_then(|address| address.host_str().map(str::to_owned));
        let item = WebsiteDownload {
            id: id.clone(),
            url: request.url.clone(),
            name,
            mime: request.mime.clone().filter(|mime| !mime.is_empty()),
            site,
            state: WebsiteDownloadState::Queued,
            done_bytes: 0.0,
            total_bytes: request.size.filter(|size| *size > 0.0),
            bytes_per_second: None,
            resumable: false,
            folder: request.destination.shown(),
            shown: None,
            error: None,
            created_at: now_ms() as f64,
        };
        let stored = Stored {
            item,
            origin: Origin {
                url: request.url,
                user_agent: request.user_agent,
                referer: request.referer,
                cookies: request.cookies,
            },
            destination: request.destination,
            validator: Validator::default(),
            saved: None,
            ticket: request.ticket.filter(|ticket| !ticket.is_empty()),
        };
        {
            let mut entries = self.entries.lock().expect("downloads poisoned");
            entries.insert(
                0,
                Entry {
                    stored,
                    cancel: None,
                },
            );
            trim(&mut entries);
        }
        self.changed(&id);
        self.pump();
        id
    }

    /// Stops a download (or takes a waiting one out); what it had is dropped.
    pub fn cancel(&self, id: &str) {
        let cancel = self.update(id, |entry| {
            if matches!(
                entry.stored.item.state,
                WebsiteDownloadState::Queued | WebsiteDownloadState::Failed
            ) {
                entry.stored.item.state = WebsiteDownloadState::Cancelled;
                entry.stored.item.bytes_per_second = None;
            }
            entry.cancel.take()
        });
        match cancel.flatten() {
            Some(cancel) => {
                let _ = cancel.send(true);
            }
            None => {
                let _ = std::fs::remove_file(self.partial(id));
                self.changed(id);
            }
        }
    }

    /// Starts a failed or cancelled download again (a failed one from where it was).
    pub fn retry(self: &Arc<Self>, id: &str) -> Result<(), String> {
        let found = self.update(id, |entry| {
            let item = &mut entry.stored.item;
            if matches!(
                item.state,
                WebsiteDownloadState::Failed | WebsiteDownloadState::Cancelled
            ) {
                item.state = WebsiteDownloadState::Queued;
                item.error = None;
                true
            } else {
                false
            }
        });
        match found {
            None => Err("That download isn't in the list any more".into()),
            Some(false) => Ok(()),
            Some(true) => {
                self.changed(id);
                self.pump();
                Ok(())
            }
        }
    }

    /// Takes a download off the list; `delete_file` deletes a finished one's file too.
    pub async fn remove(&self, id: &str, delete_file: bool) -> Result<(), String> {
        let removed = {
            let mut entries = self.entries.lock().expect("downloads poisoned");
            let index = entries
                .iter()
                .position(|entry| entry.stored.item.id == id)
                .ok_or("That download isn't in the list any more")?;
            entries.remove(index)
        };
        if let Some(cancel) = removed.cancel {
            let _ = cancel.send(true);
        }
        if let Some(ticket) = &removed.stored.ticket {
            self.feed.say(browser::BrowserDownload::gone(ticket));
        }
        let _ = std::fs::remove_file(self.partial(id));
        let mut result = Ok(());
        if delete_file && let Some(saved) = removed.stored.saved.clone() {
            result = self.delete_saved(&saved).await;
            if result.is_err() {
                // Kept, so the person can try again.
                self.entries.lock().expect("downloads poisoned").insert(
                    0,
                    Entry {
                        cancel: None,
                        ..removed
                    },
                );
            }
        }
        self.changed(id);
        result
    }

    fn saved_of(&self, id: &str) -> Result<(String, Option<String>), String> {
        self.update(id, |entry| {
            entry
                .stored
                .saved
                .clone()
                .map(|saved| (saved, entry.stored.item.mime.clone()))
        })
        .flatten()
        .ok_or_else(|| "That file isn't saved yet".to_owned())
    }

    /// A finished file (its content URI on Android, path on desktop) and its type.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub fn saved_file(&self, id: &str) -> Option<(String, Option<String>)> {
        self.saved_of(id).ok()
    }

    /// Opens a finished file in the app the system picks.
    pub fn open(&self, id: &str) -> Result<(), String> {
        let (saved, mime) = self.saved_of(id)?;
        #[cfg(target_os = "android")]
        {
            self.app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .open_file(&saved, mime.as_deref().unwrap_or("*/*"))
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = mime;
            open_path(Path::new(&saved))
        }
    }

    /// Shows a finished file in its folder.
    pub fn reveal(&self, id: &str) -> Result<(), String> {
        let (saved, _) = self.saved_of(id)?;
        #[cfg(target_os = "android")]
        {
            self.app
                .state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .show_file_folder(&saved)
        }
        #[cfg(not(target_os = "android"))]
        {
            crate::export::output::reveal(Path::new(&saved))
        }
    }

    async fn delete_saved(&self, saved: &str) -> Result<(), String> {
        #[cfg(target_os = "android")]
        {
            let app = self.app.clone();
            let saved = saved.to_owned();
            tauri::async_runtime::spawn_blocking(move || {
                app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                    .delete_file(&saved)
            })
            .await
            .map_err(|error| error.to_string())?
        }
        #[cfg(not(target_os = "android"))]
        {
            match tokio::fs::remove_file(saved).await {
                Ok(()) => Ok(()),
                Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
                Err(error) => Err(format!("The file couldn't be deleted: {error}")),
            }
        }
    }

    fn partial(&self, id: &str) -> PathBuf {
        self.partials.join(format!("{id}.part"))
    }

    /// Runs `change` on a download; `None` when it isn't in the list.
    fn update<T>(&self, id: &str, change: impl FnOnce(&mut Entry) -> T) -> Option<T> {
        self.entries
            .lock()
            .expect("downloads poisoned")
            .iter_mut()
            .find(|entry| entry.stored.item.id == id)
            .map(change)
    }

    /// Something changed that's kept: stored, and the UI told.
    fn changed(&self, id: &str) {
        self.save.notify_one();
        self.emit(id);
    }

    fn emit(&self, id: &str) {
        if let Err(error) = WebsiteDownloadsEvent(id.to_owned()).emit(&self.app) {
            tracing::debug!(%error, "file downloads event not sent");
        }
        self.feed.touch(id);
    }

    /// The download a browser's ticket names, and where it stands.
    pub fn by_ticket(&self, ticket: &str) -> Option<(String, WebsiteDownloadState)> {
        self.entries
            .lock()
            .expect("downloads poisoned")
            .iter()
            .find(|entry| entry.stored.ticket.as_deref() == Some(ticket))
            .map(|entry| (entry.stored.item.id.clone(), entry.stored.item.state))
    }

    /// The browser's question about the downloads it started. Only those are touched.
    pub fn ask(self: &Arc<Self>, ask: browser::BrowserAsk) {
        match ask {
            browser::BrowserAsk::Watch { tickets } => {
                self.feed.watch();
                for ticket in tickets {
                    match self.by_ticket(&ticket) {
                        Some((id, _)) => self.feed.touch(&id),
                        None => self.feed.say(browser::BrowserDownload::gone(&ticket)),
                    }
                }
            }
            browser::BrowserAsk::Unwatch => self.feed.unwatch(),
            browser::BrowserAsk::Cancel { ticket } => {
                if let Some((id, _)) = self.by_ticket(&ticket) {
                    self.cancel(&id);
                }
            }
            browser::BrowserAsk::Retry { ticket } => {
                if let Some((id, _)) = self.by_ticket(&ticket)
                    && let Err(error) = self.retry(&id)
                {
                    tracing::info!(%error, "a browser download wasn't retried");
                }
            }
        }
    }

    /// Tells the browser how its downloads stand, as they change: one turn at a time, a few
    /// a second at most (progress coalesces in between). What changes while no browser hears it
    /// goes unsaid: a browser asks again when it's back.
    async fn feed(self: Arc<Self>) {
        const EVERY: Duration = Duration::from_millis(300);
        loop {
            self.feed.wake.notified().await;
            let (changed, said) = self.feed.take();
            if !self.feed.heard() {
                continue;
            }
            let views = {
                let entries = self.entries.lock().expect("downloads poisoned");
                let stored: Vec<&Stored> = entries.iter().map(|entry| &entry.stored).collect();
                browser::turn(&changed, said, &stored)
            };
            if !views.is_empty() {
                browser_platform::show(&self.app, views).await;
            }
            tokio::time::sleep(EVERY).await;
        }
    }

    /// Starts waiting downloads while slots are free, oldest first.
    fn pump(self: &Arc<Self>) {
        let mut started = Vec::new();
        {
            let mut entries = self.entries.lock().expect("downloads poisoned");
            let mut running = entries
                .iter()
                .filter(|entry| entry.cancel.is_some())
                .count();
            for entry in entries.iter_mut().rev() {
                if running >= RUNNING {
                    break;
                }
                if entry.stored.item.state != WebsiteDownloadState::Queued || entry.cancel.is_some()
                {
                    continue;
                }
                let (sender, receiver) = watch::channel(false);
                entry.cancel = Some(sender);
                entry.stored.item.state = WebsiteDownloadState::Downloading;
                running += 1;
                started.push((entry.stored.clone(), receiver));
            }
        }
        for (stored, receiver) in started {
            self.changed(&stored.item.id);
            let service = self.clone();
            tauri::async_runtime::spawn(async move { service.run(stored, receiver).await });
        }
    }

    async fn run(self: Arc<Self>, stored: Stored, mut cancel: watch::Receiver<bool>) {
        let id = stored.item.id.clone();
        let partial = self.partial(&id);
        let mut validator = stored.validator.clone();
        let mut attempt = 0;
        let outcome = loop {
            let mut progress = self.progress(&id);
            let result = fetch::fetch(
                &self.client,
                &stored.origin,
                &partial,
                &mut validator,
                &mut cancel,
                &mut progress,
            )
            .await;
            // Kept as it goes, so a restart can carry on.
            let resumable = validator.ranges;
            let kept = validator.clone();
            self.update(&id, |entry| {
                entry.stored.validator = kept;
                entry.stored.item.resumable = resumable;
            });
            match result {
                Err(Failure::Transient(reason)) if attempt < RETRIES => {
                    attempt += 1;
                    tracing::info!(%reason, attempt, "file download retried");
                    let wait = Duration::from_secs(2u64.pow(attempt));
                    tokio::select! {
                        () = tokio::time::sleep(wait) => {}
                        () = fetch::cancelled(&mut cancel) => break Err(Failure::Cancelled),
                    }
                }
                other => break other,
            }
        };
        let outcome = match outcome {
            Ok(fetched) => {
                let mime = stored
                    .item
                    .mime
                    .clone()
                    .or(fetched.mime)
                    .unwrap_or_else(|| "application/octet-stream".to_owned());
                let service = self.clone();
                let name = stored.item.name.clone();
                let destination = stored.destination.clone();
                let placed = tauri::async_runtime::spawn_blocking(move || {
                    service.place(&partial, &name, &mime, &destination)
                })
                .await
                .map_err(|error| error.to_string())
                .and_then(|placed| placed);
                placed.map(|placed| (placed, fetched.bytes))
            }
            Err(failure) => Err(match failure {
                Failure::Cancelled => String::new(),
                Failure::Transient(reason) | Failure::Permanent(reason) => reason,
            }),
        };
        let cancelled = *cancel.borrow();
        self.update(&id, |entry| {
            entry.cancel = None;
            let item = &mut entry.stored.item;
            item.bytes_per_second = None;
            match outcome {
                Ok((placed, bytes)) => {
                    item.state = WebsiteDownloadState::Done;
                    item.name = placed.name;
                    item.shown = Some(placed.shown);
                    item.done_bytes = bytes as f64;
                    item.total_bytes = Some(bytes as f64);
                    entry.stored.saved = Some(placed.saved);
                }
                Err(_) if cancelled => {
                    item.state = WebsiteDownloadState::Cancelled;
                    item.done_bytes = 0.0;
                }
                Err(reason) => {
                    item.state = WebsiteDownloadState::Failed;
                    item.error = Some(reason);
                }
            }
        });
        if cancelled {
            let _ = std::fs::remove_file(self.partial(&id));
        }
        self.changed(&id);
        self.pump();
    }

    /// The progress callback for one run: the entry's bytes and speed, sent at most 4× a second.
    fn progress(self: &Arc<Self>, id: &str) -> impl FnMut(u64, Option<u64>) + Send + use<> {
        let service = self.clone();
        let id = id.to_owned();
        let mut last_sent = Instant::now() - PROGRESS_EVERY;
        let mut sample: Option<(Instant, u64)> = None;
        move |done, total| {
            let now = Instant::now();
            let speed = match sample {
                Some((at, bytes)) if now.duration_since(at) >= Duration::from_secs(1) => {
                    sample = Some((now, done));
                    let seconds = now.duration_since(at).as_secs_f64();
                    Some(Some(done.saturating_sub(bytes) as f64 / seconds))
                }
                Some(_) => None,
                None => {
                    sample = Some((now, done));
                    None
                }
            };
            service.update(&id, |entry| {
                let item = &mut entry.stored.item;
                item.done_bytes = done as f64;
                if let Some(total) = total {
                    item.total_bytes = Some(total as f64);
                }
                if let Some(speed) = speed {
                    item.bytes_per_second = speed;
                }
            });
            if now.duration_since(last_sent) >= PROGRESS_EVERY {
                last_sent = now;
                service.emit(&id);
            }
        }
    }

    /// Moves a finished file into its place. Blocking.
    fn place(
        &self,
        partial: &Path,
        name: &str,
        mime: &str,
        destination: &Destination,
    ) -> Result<Placed, String> {
        let _one = self.placing.lock().expect("placing poisoned");
        let place = destination.place();
        #[cfg(target_os = "android")]
        {
            let outputs = self
                .app
                .state::<Arc<crate::export::ExportService>>()
                .outputs()
                .clone();
            let mut output = outputs.create_new(&place, name, mime)?;
            let name = output.name.clone();
            let copied = std::fs::File::open(partial)
                .and_then(|mut source| std::io::copy(&mut source, &mut output.file));
            let saved = outputs.finish(output, copied.is_ok());
            copied.map_err(|error| format!("The file couldn't be saved: {error}"))?;
            let saved = saved?;
            let _ = std::fs::remove_file(partial);
            Ok(Placed {
                name,
                shown: saved.shown,
                saved: saved.uri.unwrap_or_default(),
            })
        }
        #[cfg(not(target_os = "android"))]
        {
            let _ = mime;
            let dir = self
                .app
                .state::<Arc<crate::export::ExportService>>()
                .outputs()
                .folder(&place);
            let path = move_into(partial, &dir, name)
                .map_err(|error| format!("The file couldn't be saved: {error}"))?;
            Ok(Placed {
                name: path
                    .file_name()
                    .map(|name| name.to_string_lossy().into_owned())
                    .unwrap_or_else(|| name.to_owned()),
                shown: path.to_string_lossy().into_owned(),
                saved: path.to_string_lossy().into_owned(),
            })
        }
    }
}

/// Where a finished file went.
struct Placed {
    name: String,
    shown: String,
    saved: String,
}

/// Keeps the newest finished entries; running and waiting ones always stay.
fn trim(entries: &mut Vec<Entry>) {
    let mut finished = 0;
    entries.retain(|entry| {
        let ended = matches!(
            entry.stored.item.state,
            WebsiteDownloadState::Done
                | WebsiteDownloadState::Cancelled
                | WebsiteDownloadState::Failed
        );
        if ended {
            finished += 1;
        }
        !ended || finished <= KEEP
    });
}

/// Moves `partial` into `dir` as `name`, or `name (1)`… when that's taken. Blocking.
#[cfg_attr(target_os = "android", allow(dead_code))]
pub fn move_into(partial: &Path, dir: &Path, name: &str) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(dir)?;
    let name = names::unique_name(name, |candidate| dir.join(candidate).exists());
    let path = dir.join(name);
    if std::fs::rename(partial, &path).is_err() {
        // Another drive: copied, then the partial goes.
        std::fs::copy(partial, &path)?;
        let _ = std::fs::remove_file(partial);
    }
    Ok(path)
}

/// Opens a file in its default app. The path is one argument, no shell.
#[cfg(not(target_os = "android"))]
fn open_path(path: &Path) -> Result<(), String> {
    if !path.exists() {
        return Err("The file isn't there any more".into());
    }
    #[cfg(windows)]
    let program = "explorer";
    #[cfg(target_os = "macos")]
    let program = "open";
    #[cfg(not(any(windows, target_os = "macos")))]
    let program = "xdg-open";
    std::process::Command::new(program)
        .arg(path)
        .spawn()
        .map(|_| ())
        .map_err(|error| format!("The file didn't open: {error}"))
}

/// Takes a download from the browser: Android's from its broadcast (`{"download": …}` reports),
/// desktop's from its window.
pub fn take_request(app: &AppHandle, request: serde_json::Value) {
    let ticket = request["ticket"].as_str().map(str::to_owned);
    let name = request["name"].as_str().unwrap_or_default().to_owned();
    match serde_json::from_value(request) {
        Ok(request) => accept(app, request),
        Err(error) => {
            tracing::warn!(%error, "unreadable download request");
            if let Some(ticket) = ticket {
                refuse(app, &ticket, &name, "This download couldn't start");
            }
        }
    }
}

/// Starts a download the person chose a place for.
pub fn accept(app: &AppHandle, request: FileRequest) {
    if !fetchable(&request.url) {
        tracing::info!("download of an address Raygun can't fetch skipped");
        if let Some(ticket) = &request.ticket {
            refuse(
                app,
                ticket,
                &request.file_name(),
                "This download can't be saved from here",
            );
        }
        return;
    }
    if let Some(service) = app.try_state::<Arc<WebsiteDownloadService>>() {
        service.add(request);
    }
}

/// The browser's question about the downloads it started (`{"downloadAsk": …}` reports).
pub fn take_ask(app: &AppHandle, ask: serde_json::Value) {
    match serde_json::from_value::<browser::BrowserAsk>(ask) {
        Ok(ask) => {
            if let Some(service) = app.try_state::<Arc<WebsiteDownloadService>>() {
                service.ask(ask);
            }
        }
        Err(error) => tracing::warn!(%error, "unreadable download question"),
    }
}

/// Addresses Raygun downloads itself.
fn fetchable(url: &str) -> bool {
    url.starts_with("https://") || url.starts_with("http://") || url.starts_with("data:")
}

/// Tells the browser a request it handed over didn't start.
fn refuse(app: &AppHandle, ticket: &str, name: &str, reason: &str) {
    if let Some(service) = app.try_state::<Arc<WebsiteDownloadService>>() {
        service
            .feed
            .say(browser::BrowserDownload::refused(ticket, name, reason));
    }
}

/// Where the browser hears about its downloads.
#[cfg(target_os = "android")]
mod browser_platform {
    use tauri::{AppHandle, Manager};

    use super::browser::BrowserDownload;

    /// In a package-only broadcast to the browser's process (through the plugin), which shows
    /// the ones it started while it's open.
    pub async fn show(app: &AppHandle, downloads: Vec<BrowserDownload>) {
        let value = serde_json::json!({ "downloads": downloads });
        let app = app.clone();
        let sent = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .website_downloads(&value)
        })
        .await;
        if let Ok(Err(error)) = sent {
            tracing::debug!(%error, "the browser wasn't told about its downloads");
        }
    }
}

#[cfg(not(target_os = "android"))]
mod browser_platform {
    use tauri::AppHandle;

    use super::browser::BrowserDownload;

    /// In the Downloads menu of the window that started each one. Menus change on the main
    /// thread, which this waits for.
    pub async fn show(app: &AppHandle, downloads: Vec<BrowserDownload>) {
        let app = app.clone();
        let _ = tauri::async_runtime::spawn_blocking(move || {
            crate::websites::show_downloads(&app, &downloads);
        })
        .await;
    }
}

// ---- Commands ----

/// Files saved from websites, newest first.
#[tauri::command]
#[specta::specta]
pub fn website_downloads(
    service: tauri::State<'_, Arc<WebsiteDownloadService>>,
) -> Vec<WebsiteDownload> {
    service.list()
}

/// Stops a file download.
#[tauri::command]
#[specta::specta]
pub fn website_download_cancel(service: tauri::State<'_, Arc<WebsiteDownloadService>>, id: String) {
    service.cancel(&id);
}

/// Starts a failed or cancelled file download again.
#[tauri::command]
#[specta::specta]
pub fn website_download_retry(
    service: tauri::State<'_, Arc<WebsiteDownloadService>>,
    id: String,
) -> Result<(), String> {
    service.inner().retry(&id)
}

/// Takes a file download off the list; `delete_file` deletes its file too.
#[tauri::command]
#[specta::specta]
pub async fn website_download_remove(
    service: tauri::State<'_, Arc<WebsiteDownloadService>>,
    id: String,
    delete_file: bool,
) -> Result<(), String> {
    service.remove(&id, delete_file).await
}

/// Opens a downloaded file.
#[tauri::command]
#[specta::specta]
pub fn website_download_open(
    service: tauri::State<'_, Arc<WebsiteDownloadService>>,
    id: String,
) -> Result<(), String> {
    service.open(&id)
}

/// Shows a downloaded file in its folder.
#[tauri::command]
#[specta::specta]
pub fn website_download_reveal(
    service: tauri::State<'_, Arc<WebsiteDownloadService>>,
    id: String,
) -> Result<(), String> {
    service.reveal(&id)
}
