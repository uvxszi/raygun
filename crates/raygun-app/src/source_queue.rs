//! Sources → Add several: a list of addresses, pasted or read from a text file, one a line, each
//! anything Sources → Add reads (`source_detect`). They're looked at and added one after another
//! in the order pasted, in the background: closing the sheet or reloading the UI leaves them
//! going, and every change is announced with [`SourceQueueEvent`], which the Sources page and the
//! notifications follow.
//!
//! Each address is looked at, then added unless this device has it. A server waits for its
//! sign-in, and a Stremio addon that needs setting up for its website, while the rest go on;
//! failures keep their issue until tried again or dismissed. Cancelling calls off what waits and
//! the look running now; an addition running (an extension being written) finishes. Extensions
//! installed load once, when nothing is left.
//!
//! Offline Mode holds the queue: nothing is looked at until it ends, and a look it stopped takes
//! its turn again. One host isn't asked twice in a row without a pause, and one that asks to slow
//! down is asked again later.
//!
//! [`SourceQueueEvent`]: crate::events::SourceQueueEvent

use std::collections::HashSet;
use std::future::Future;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Duration;

use raygun_model::{
    DetectedKind, DetectedSource, SourceQueueAdded, SourceQueueItem, SourceQueueState,
    SourceQueueStatus,
};
use tauri::Manager;
use tokio::sync::Notify;
use tokio_util::sync::CancellationToken;

use crate::extension_installs::rate_limited;
use crate::local_network::LocalNetwork;
use crate::source_detect::{SourceDetector, addresses};

/// The most addresses the queue holds at once (every change sends the whole list).
pub const MAX_QUEUED: usize = 500;
/// The largest list file read.
const MAX_LIST_BYTES: u64 = 1024 * 1024;

/// The pause before asking the host just asked again.
#[cfg(not(test))]
const SAME_HOST_GAP: Duration = Duration::from_millis(1_500);
#[cfg(test)]
const SAME_HOST_GAP: Duration = Duration::ZERO;
/// How long a rate-limited look or addition waits, times its attempt, and how often it's tried.
#[cfg(not(test))]
const RATE_LIMIT_WAIT: Duration = Duration::from_secs(30);
#[cfg(test)]
const RATE_LIMIT_WAIT: Duration = Duration::from_millis(1);
const RATE_LIMIT_RETRIES: u32 = 3;

// ---- Reading a list ----

/// A list's addresses in order, and how many lines repeated an address before them.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct List {
    pub addresses: Vec<String>,
    pub duplicates: u32,
}

/// The addresses in a pasted block or a list file: each line trimmed, blank lines and lines
/// starting with `#` left out, every address once however it's spelled ([`address_key`]).
pub fn read_list(text: &str) -> List {
    let mut seen = HashSet::new();
    let mut list = List::default();
    for line in text.trim_start_matches('\u{feff}').lines() {
        let line = line.trim();
        if line.is_empty() || line.starts_with('#') {
            continue;
        }
        if seen.insert(address_key(line)) {
            list.addresses.push(line.to_owned());
        } else {
            list.duplicates += 1;
        }
    }
    list
}

/// What tells addresses apart: the first address one may be (a scheme added, `stremio://` read
/// as https, the host in lower case) without a closing slash; text that isn't one, in lower case.
pub fn address_key(typed: &str) -> String {
    let typed = typed.trim();
    match addresses(typed)
        .ok()
        .and_then(|forms| forms.into_iter().next())
    {
        Some(url) => url.as_str().trim_end_matches('/').to_owned(),
        None => typed.to_lowercase(),
    }
}

/// The host an address is asked at, when it's on the web.
fn host_of(address: &str) -> Option<String> {
    let url = addresses(address.trim()).ok()?.into_iter().next()?;
    url.host_str().map(str::to_owned)
}

/// The addresses in a list file picked beside the field. A file that's a source itself (an
/// index, a package, a script) stands for its own path; a JSON array of addresses is a list.
pub fn read_list_file(path: &std::path::Path) -> Result<Vec<String>, String> {
    use std::io::Read;
    let shown = path.file_name().map_or_else(
        || path.display().to_string(),
        |name| name.to_string_lossy().into_owned(),
    );
    if !raygun_ext_host::repo::local_files_allowed(path) {
        return Err("Raygun can't read files there on this device".to_owned());
    }
    let mut bytes = Vec::new();
    std::fs::File::open(path)
        .and_then(|file| file.take(MAX_LIST_BYTES + 1).read_to_end(&mut bytes))
        .map_err(|error| format!("Couldn't read {shown}: {error}"))?;
    let itself = || vec![path.display().to_string()];
    if bytes.starts_with(b"PK") || bytes.starts_with(&[0x1f, 0x8b]) {
        return Ok(itself());
    }
    if bytes.len() as u64 > MAX_LIST_BYTES {
        return Err(format!("{shown} is too long for a list of addresses"));
    }
    if bytes.contains(&0) {
        return Err(format!("{shown} isn't a text file"));
    }
    let text = String::from_utf8_lossy(&bytes);
    let start = text.trim_start_matches('\u{feff}').trim_start();
    if start.starts_with('[') {
        return Ok(match serde_json::from_str::<Vec<String>>(start) {
            Ok(listed) => read_list(&listed.join("\n")).addresses,
            Err(_) => itself(),
        });
    }
    if start.starts_with('{') || start.starts_with('<') {
        return Ok(itself());
    }
    let list = read_list(&text);
    if list.addresses.is_empty() {
        return Err(format!("{shown} lists no addresses"));
    }
    // Lines that mostly aren't addresses are a script's (a Shosetsu or Miru source on its own).
    let readable = list
        .addresses
        .iter()
        .filter(|address| addresses(address).is_ok())
        .count();
    if readable * 2 < list.addresses.len() {
        return Ok(itself());
    }
    Ok(list.addresses)
}

/// The first address of a list that needs Android 17's local network access while Raygun
/// doesn't have it.
async fn first_local(network: &LocalNetwork, text: &str) -> Result<Option<String>, String> {
    let access = network.access(None).await?;
    if !access.needed || access.granted {
        return Ok(None);
    }
    for address in read_list(text).addresses {
        if network
            .access(Some(&address))
            .await
            .is_ok_and(|access| access.needed && !access.granted)
        {
            return Ok(Some(address));
        }
    }
    Ok(None)
}

// ---- The queue ----

/// What comes after a look.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Step {
    /// It's to be added.
    Add,
    /// It ended: this device has it, it waits for the person, or it failed.
    Done,
    /// Offline Mode stopped it: it takes its turn again.
    Offline,
    /// It was called off meanwhile.
    Skip,
}

/// Waiting, being looked at or being added.
fn busy(state: SourceQueueState) -> bool {
    matches!(
        state,
        SourceQueueState::Waiting | SourceQueueState::Checking | SourceQueueState::Adding
    )
}

/// Ended with something left for the person: a sign-in, a setup, a failure.
fn waits_for_you(state: SourceQueueState) -> bool {
    matches!(
        state,
        SourceQueueState::NeedsSignIn | SourceQueueState::NeedsSetup | SourceQueueState::Failed
    )
}

#[derive(Debug)]
struct Entry {
    item: SourceQueueItem,
    /// What tells its address from others ([`address_key`]).
    key: String,
    /// Its place in line: lower goes first.
    turn: u64,
}

/// The queue's state, without the running (tested on its own).
#[derive(Debug, Default)]
pub struct Queue {
    batch: u32,
    last_id: u32,
    next_turn: u64,
    /// In the order pasted.
    entries: Vec<Entry>,
    /// Extensions installed since they last loaded.
    unloaded: bool,
    loading: bool,
    /// Offline Mode holds the queue.
    held: bool,
}

impl Queue {
    fn active(&self) -> bool {
        self.loading || self.entries.iter().any(|entry| busy(entry.item.state))
    }

    fn waiting(&self) -> bool {
        self.entries
            .iter()
            .any(|entry| entry.item.state == SourceQueueState::Waiting)
    }

    fn entry(&mut self, id: u32, state: SourceQueueState) -> Option<&mut Entry> {
        self.entries
            .iter_mut()
            .find(|entry| entry.item.id == id && entry.item.state == state)
    }

    fn state_of(&self, id: u32) -> Option<SourceQueueState> {
        self.entries
            .iter()
            .find(|entry| entry.item.id == id)
            .map(|entry| entry.item.state)
    }

    fn turn(&mut self) -> u64 {
        self.next_turn += 1;
        self.next_turn
    }

    /// A list while nothing runs starts a batch: what ended goes, except what waits for the
    /// person.
    fn new_batch(&mut self) {
        self.batch += 1;
        self.entries.retain(|entry| waits_for_you(entry.item.state));
    }

    /// Queues a list's addresses after what waits. One already waiting or being looked at isn't
    /// queued again; one that ended is, in its new place. Returns how many were queued, left out
    /// as duplicates, and left out for [`MAX_QUEUED`].
    pub fn enqueue(&mut self, list: &List) -> (u32, u32, u32) {
        let (mut queued, mut duplicates, mut over_limit) = (0, list.duplicates, 0);
        for address in &list.addresses {
            let key = address_key(address);
            if self
                .entries
                .iter()
                .any(|entry| entry.key == key && busy(entry.item.state))
            {
                duplicates += 1;
                continue;
            }
            if !self.active() {
                self.new_batch();
            }
            self.entries.retain(|entry| entry.key != key);
            if self.entries.len() >= MAX_QUEUED {
                over_limit += 1;
                continue;
            }
            self.last_id += 1;
            let turn = self.turn();
            self.entries.push(Entry {
                item: SourceQueueItem {
                    id: self.last_id,
                    address: address.clone(),
                    state: SourceQueueState::Waiting,
                    kind: None,
                    format: None,
                    server: None,
                    name: None,
                    url: None,
                    configure_url: None,
                    error: None,
                    batch: self.batch,
                },
                key,
                turn,
            });
            queued += 1;
        }
        (queued, duplicates, over_limit)
    }

    /// The address waiting longest, now being looked at; nothing while another is.
    pub fn start_next(&mut self) -> Option<(u32, String)> {
        if self.loading
            || self.entries.iter().any(|entry| {
                matches!(
                    entry.item.state,
                    SourceQueueState::Checking | SourceQueueState::Adding
                )
            })
        {
            return None;
        }
        let entry = self
            .entries
            .iter_mut()
            .filter(|entry| entry.item.state == SourceQueueState::Waiting)
            .min_by_key(|entry| entry.turn)?;
        entry.item.state = SourceQueueState::Checking;
        entry.item.error = None;
        Some((entry.item.id, entry.item.address.clone()))
    }

    /// The host of the address looked at next.
    fn next_host(&self) -> Option<String> {
        self.entries
            .iter()
            .filter(|entry| entry.item.state == SourceQueueState::Waiting)
            .min_by_key(|entry| entry.turn)
            .and_then(|entry| host_of(&entry.item.address))
    }

    /// What a look found: added already, waiting for the person, to be added, or why not.
    pub fn looked(&mut self, id: u32, result: Result<DetectedSource, String>) -> Step {
        let Some(entry) = self.entry(id, SourceQueueState::Checking) else {
            return Step::Skip;
        };
        let item = &mut entry.item;
        match result {
            Ok(found) => {
                item.kind = Some(found.kind);
                item.format = found.format;
                item.server = found.server;
                item.name = Some(found.name.trim().to_owned()).filter(|name| !name.is_empty());
                item.url = Some(found.url.clone()).filter(|url| !url.is_empty());
                let setup = found.addon.as_ref().filter(|addon| addon.needs_setup);
                item.configure_url = setup.and_then(|addon| addon.configure_url.clone());
                item.state = if found.added {
                    SourceQueueState::AlreadyAdded
                } else if found.kind == DetectedKind::Server {
                    SourceQueueState::NeedsSignIn
                } else if setup.is_some() {
                    SourceQueueState::NeedsSetup
                } else {
                    SourceQueueState::Adding
                };
                if item.state == SourceQueueState::Adding {
                    Step::Add
                } else {
                    Step::Done
                }
            }
            Err(error) if raygun_net::network_mode::mentions_offline(&error) => {
                item.state = SourceQueueState::Waiting;
                Step::Offline
            }
            Err(error) => {
                item.state = SourceQueueState::Failed;
                item.error = Some(error);
                Step::Done
            }
        }
    }

    /// How adding it went.
    pub fn added(&mut self, id: u32, result: Result<(), String>) {
        let Some(entry) = self.entry(id, SourceQueueState::Adding) else {
            return;
        };
        let extension = entry.item.kind == Some(DetectedKind::Extension);
        match result {
            Ok(()) => {
                entry.item.state = SourceQueueState::Added;
                self.unloaded |= extension;
            }
            Err(error) if raygun_net::network_mode::mentions_offline(&error) => {
                entry.item.state = SourceQueueState::Waiting;
            }
            Err(error) => {
                entry.item.state = SourceQueueState::Failed;
                entry.item.error = Some(error);
            }
        }
    }

    /// Calls off one address waiting or being looked at, or all with `None`; one being added
    /// finishes. Returns whether the one being looked at was called off.
    pub fn cancel(&mut self, id: Option<u32>) -> bool {
        let mut looking = false;
        for entry in &mut self.entries {
            if id.is_some_and(|id| id != entry.item.id) {
                continue;
            }
            match entry.item.state {
                SourceQueueState::Waiting => entry.item.state = SourceQueueState::Cancelled,
                SourceQueueState::Checking => {
                    entry.item.state = SourceQueueState::Cancelled;
                    looking = true;
                }
                _ => {}
            }
        }
        looking
    }

    /// Queues the failed addresses again, in their places. Returns how many.
    pub fn retry(&mut self) -> usize {
        if !self
            .entries
            .iter()
            .any(|entry| entry.item.state == SourceQueueState::Failed)
        {
            return 0;
        }
        if !self.active() {
            self.new_batch();
        }
        let batch = self.batch;
        let mut count = 0;
        for index in 0..self.entries.len() {
            if self.entries[index].item.state != SourceQueueState::Failed {
                continue;
            }
            let turn = self.turn();
            let entry = &mut self.entries[index];
            entry.item.state = SourceQueueState::Waiting;
            entry.item.error = None;
            entry.item.batch = batch;
            entry.turn = turn;
            count += 1;
        }
        count
    }

    /// A server waiting for its sign-in was connected.
    pub fn signed_in(&mut self, id: u32) -> bool {
        match self.entry(id, SourceQueueState::NeedsSignIn) {
            Some(entry) => {
                entry.item.state = SourceQueueState::Added;
                true
            }
            None => false,
        }
    }

    /// Forgets addresses that ended: one, or all with `None`.
    pub fn dismiss(&mut self, id: Option<u32>) {
        self.entries
            .retain(|entry| busy(entry.item.state) || id.is_some_and(|id| id != entry.item.id));
    }

    /// Nothing waits: whether the extensions installed should load now (marked loading).
    pub fn start_loading(&mut self) -> bool {
        if self.active() || !self.unloaded {
            return false;
        }
        self.unloaded = false;
        self.loading = true;
        true
    }

    pub fn finish_loading(&mut self) {
        self.loading = false;
    }

    pub fn set_held(&mut self, held: bool) {
        self.held = held;
    }

    pub fn status(&self) -> SourceQueueStatus {
        let batch: Vec<&SourceQueueItem> = self
            .entries
            .iter()
            .map(|entry| &entry.item)
            .filter(|item| item.batch == self.batch)
            .collect();
        let count = |state: SourceQueueState| {
            batch.iter().filter(|item| item.state == state).count() as u32
        };
        let current = self
            .entries
            .iter()
            .map(|entry| &entry.item)
            .find(|item| {
                matches!(
                    item.state,
                    SourceQueueState::Checking | SourceQueueState::Adding
                )
            })
            .map(|item| item.name.clone().unwrap_or_else(|| item.address.clone()));
        SourceQueueStatus {
            batch: self.batch,
            items: self
                .entries
                .iter()
                .map(|entry| entry.item.clone())
                .collect(),
            total: batch.len() as u32,
            added: count(SourceQueueState::Added),
            already_added: count(SourceQueueState::AlreadyAdded),
            needs_sign_in: count(SourceQueueState::NeedsSignIn),
            needs_setup: count(SourceQueueState::NeedsSetup),
            failed: count(SourceQueueState::Failed),
            cancelled: count(SourceQueueState::Cancelled),
            // Installed but not loaded yet counts: loading follows at once.
            running: self.active() || self.unloaded,
            paused: self.held && self.waiting(),
            loading: self.loading,
            current,
        }
    }
}

// ---- Running it ----

/// What looks at and adds addresses: Sources → Add's detector, or a stand-in in tests.
pub trait Sources: Send + Sync + 'static {
    /// What `address` is, marked against what this device has.
    fn look(&self, address: &str) -> impl Future<Output = Result<DetectedSource, String>> + Send;

    /// Adds what `address` was found to be; an extension is installed without loading.
    fn add(&self, address: &str) -> impl Future<Output = Result<(), String>> + Send;

    /// Loads the extensions installed.
    fn load(&self) -> impl Future<Output = ()> + Send;

    /// Whether Offline Mode is on.
    fn offline(&self) -> bool {
        raygun_net::network_mode::offline()
    }

    /// Returns once Offline Mode is off.
    fn wait_online(&self) -> impl Future<Output = ()> + Send {
        raygun_net::network_mode::wait_online()
    }
}

/// The app's: Sources → Add's detector, then the extensions loaded again.
struct AppSources {
    app: tauri::AppHandle,
}

impl AppSources {
    fn detector(&self) -> Result<tauri::State<'_, SourceDetector>, String> {
        self.app
            .try_state::<SourceDetector>()
            .ok_or_else(|| "Raygun isn't ready yet".to_owned())
    }
}

impl Sources for AppSources {
    async fn look(&self, address: &str) -> Result<DetectedSource, String> {
        self.detector()?.detect(address).await
    }

    async fn add(&self, address: &str) -> Result<(), String> {
        self.detector()?.add_in_list(address).await
    }

    async fn load(&self) {
        let extensions = self
            .app
            .try_state::<Arc<crate::extensions::ExtensionService>>()
            .map(|extensions| extensions.inner().clone());
        if let Some(extensions) = extensions {
            extensions.reload().await;
        }
    }
}

/// Asks again, after a wait, while the host asks to slow down.
async fn patiently<T, F, Fut>(mut ask: F) -> Result<T, String>
where
    F: FnMut() -> Fut,
    Fut: Future<Output = Result<T, String>>,
{
    let mut attempt = 0;
    loop {
        match ask().await {
            Err(error) if rate_limited(&error) && attempt < RATE_LIMIT_RETRIES => {
                attempt += 1;
                tracing::info!(attempt, "rate limited; asking again after a wait");
                tokio::time::sleep(RATE_LIMIT_WAIT * attempt).await;
            }
            result => return result,
        }
    }
}

type Announce = Box<dyn Fn(&SourceQueueStatus) + Send + Sync>;

/// The queue and its worker.
pub struct SourceQueue {
    queue: Mutex<Queue>,
    wake: Notify,
    announce: Announce,
    /// Calls off the look running now.
    looking: Mutex<Option<CancellationToken>>,
}

impl SourceQueue {
    /// Starts the worker, announcing every change with `announce`.
    pub fn start<S: Sources>(
        sources: S,
        announce: impl Fn(&SourceQueueStatus) + Send + Sync + 'static,
    ) -> Arc<Self> {
        let queue = Arc::new(Self {
            queue: Mutex::new(Queue::default()),
            wake: Notify::new(),
            announce: Box::new(announce),
            looking: Mutex::new(None),
        });
        let worker = queue.clone();
        tauri::async_runtime::spawn(async move { worker.run(sources).await });
        queue
    }

    /// The queue running in the app.
    pub fn start_in(app: &tauri::AppHandle) -> Arc<Self> {
        use tauri_specta::Event;
        let handle = app.clone();
        Self::start(AppSources { app: app.clone() }, move |status| {
            if let Err(error) = crate::events::SourceQueueEvent(status.clone()).emit(&handle) {
                tracing::debug!(%error, "could not announce the sources being added");
            }
        })
    }

    fn lock(&self) -> MutexGuard<'_, Queue> {
        self.queue.lock().expect("source queue poisoned")
    }

    fn change<T>(&self, change: impl FnOnce(&mut Queue) -> T) -> (T, SourceQueueStatus) {
        let mut queue = self.lock();
        let result = change(&mut queue);
        (result, queue.status())
    }

    fn changed<T>(&self, change: impl FnOnce(&mut Queue) -> T) -> SourceQueueStatus {
        let (_, status) = self.change(change);
        (self.announce)(&status);
        status
    }

    pub fn status(&self) -> SourceQueueStatus {
        self.lock().status()
    }

    /// Queues a list of addresses, one a line.
    pub fn enqueue(&self, text: &str) -> SourceQueueAdded {
        let list = read_list(text);
        let ((queued, duplicates, over_limit), status) = self.change(|queue| queue.enqueue(&list));
        (self.announce)(&status);
        self.wake.notify_one();
        SourceQueueAdded {
            queued,
            duplicates,
            over_limit,
            status,
        }
    }

    pub fn cancel(&self, id: Option<u32>) -> SourceQueueStatus {
        let (looking, status) = self.change(|queue| queue.cancel(id));
        if looking && let Some(token) = self.looking.lock().expect("looking poisoned").as_ref() {
            token.cancel();
        }
        (self.announce)(&status);
        status
    }

    pub fn retry(&self) -> SourceQueueStatus {
        let status = self.changed(Queue::retry);
        self.wake.notify_one();
        status
    }

    pub fn signed_in(&self, id: u32) -> SourceQueueStatus {
        self.changed(|queue| queue.signed_in(id))
    }

    pub fn dismiss(&self, id: Option<u32>) -> SourceQueueStatus {
        self.changed(|queue| queue.dismiss(id))
    }

    async fn run<S: Sources>(self: Arc<Self>, sources: S) {
        let mut last_host: Option<String> = None;
        loop {
            let waiting = self.lock().waiting();
            if waiting && sources.offline() {
                self.changed(|queue| queue.set_held(true));
                sources.wait_online().await;
                self.changed(|queue| queue.set_held(false));
                continue;
            }
            let next_host = self.lock().next_host();
            if !SAME_HOST_GAP.is_zero() && next_host.is_some() && next_host == last_host {
                tokio::time::sleep(SAME_HOST_GAP).await;
            }
            let (next, _) = self.change(Queue::start_next);
            let Some((id, address)) = next else {
                let (load, _) = self.change(Queue::start_loading);
                if load {
                    self.changed(|_| ());
                    sources.load().await;
                    self.changed(Queue::finish_loading);
                    continue;
                }
                self.wake.notified().await;
                continue;
            };
            last_host = host_of(&address);
            let cancel = CancellationToken::new();
            *self.looking.lock().expect("looking poisoned") = Some(cancel.clone());
            // Called off before the look could be: it says so already.
            if self.lock().state_of(id) != Some(SourceQueueState::Checking) {
                continue;
            }
            self.changed(|_| ());
            // Called off wins over an answer that came at the same time.
            let looked = tokio::select! {
                biased;
                () = cancel.cancelled() => None,
                looked = patiently(|| sources.look(&address)) => Some(looked),
            };
            *self.looking.lock().expect("looking poisoned") = None;
            let Some(looked) = looked else {
                continue;
            };
            if let Err(error) = &looked {
                tracing::info!(%error, address, "a source of a list couldn't be looked at");
            }
            let (step, status) = self.change(|queue| queue.looked(id, looked));
            (self.announce)(&status);
            if step == Step::Add {
                let added = patiently(|| sources.add(&address)).await;
                if let Err(error) = &added {
                    tracing::info!(%error, address, "a source of a list wasn't added");
                }
                self.changed(|queue| queue.added(id, added));
            }
        }
    }
}

// ---- Commands ----

/// Sources → Add several: the addresses queued, as they're looked at and added.
#[tauri::command]
#[specta::specta]
pub fn sources_queue_status(queue: tauri::State<'_, Arc<SourceQueue>>) -> SourceQueueStatus {
    queue.status()
}

/// Queues a list of addresses, one a line (blank lines and `#` comments left out, each address
/// once); they're looked at and added one after another in the background.
#[tauri::command]
#[specta::specta]
pub fn sources_queue_add(
    queue: tauri::State<'_, Arc<SourceQueue>>,
    text: String,
) -> SourceQueueAdded {
    queue.enqueue(&text)
}

/// The addresses in a text file picked for the list; a file that's a source itself stands for its
/// own path.
#[tauri::command]
#[specta::specta]
pub async fn sources_queue_read_file(path: String) -> Result<Vec<String>, String> {
    tauri::async_runtime::spawn_blocking(move || read_list_file(std::path::Path::new(&path)))
        .await
        .map_err(|error| error.to_string())?
}

/// The first address of a list on the local network, while Android 17's access to it is needed
/// and not given; `null` when none is.
#[tauri::command]
#[specta::specta]
pub async fn sources_queue_local_address(
    local_network: tauri::State<'_, LocalNetwork>,
    text: String,
) -> Result<Option<String>, String> {
    first_local(&local_network, &text).await
}

/// Calls off an address waiting or being looked at, or every one when none is named.
#[tauri::command]
#[specta::specta]
pub fn sources_queue_cancel(
    queue: tauri::State<'_, Arc<SourceQueue>>,
    id: Option<u32>,
) -> SourceQueueStatus {
    queue.cancel(id)
}

/// Queues the failed addresses again.
#[tauri::command]
#[specta::specta]
pub fn sources_queue_retry(queue: tauri::State<'_, Arc<SourceQueue>>) -> SourceQueueStatus {
    queue.retry()
}

/// Forgets an address that ended, or every one when none is named.
#[tauri::command]
#[specta::specta]
pub fn sources_queue_dismiss(
    queue: tauri::State<'_, Arc<SourceQueue>>,
    id: Option<u32>,
) -> SourceQueueStatus {
    queue.dismiss(id)
}

/// A server of the list was connected with its sign-in.
#[tauri::command]
#[specta::specta]
pub fn sources_queue_signed_in(
    queue: tauri::State<'_, Arc<SourceQueue>>,
    id: u32,
) -> SourceQueueStatus {
    queue.signed_in(id)
}

#[cfg(test)]
mod tests;
