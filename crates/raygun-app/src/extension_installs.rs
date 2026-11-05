//! The extension install queue: installs from repositories (one, a selection, every update) run
//! one at a time in the order asked, in the background, whatever screen shows or whether the UI
//! reloads. Extensions load again once when the queue empties, not after each install.
//!
//! Cancelling stops the installs still waiting; the one running finishes (a package is written
//! whole or not at all). Failures stay listed with their reason until retried or dismissed.
//! Every change is announced with [`ExtensionInstallsEvent`], which the notifications follow.

use std::future::Future;
use std::sync::{Arc, Mutex};

use raygun_model::{
    ExtensionInstall, ExtensionInstallPhase, ExtensionInstallRequest, ExtensionInstallState,
    ExtensionInstallStatus,
};
use tokio::sync::Notify;

/// Shortest and longest pause between two installs, so a repository's host sees a person's
/// pace rather than a burst.
#[cfg(not(test))]
const GAP_MS: (u64, u64) = (1_200, 3_500);
#[cfg(test)]
const GAP_MS: (u64, u64) = (0, 0);
/// How long a rate-limited install waits, times its attempt, and how often it tries again.
#[cfg(not(test))]
const RATE_LIMIT_WAIT: std::time::Duration = std::time::Duration::from_secs(30);
#[cfg(test)]
const RATE_LIMIT_WAIT: std::time::Duration = std::time::Duration::from_millis(1);
const RATE_LIMIT_RETRIES: u32 = 3;

/// A pause somewhere in `GAP_MS`, varied by the clock (no random source needed for this).
fn install_gap() -> std::time::Duration {
    let (low, high) = GAP_MS;
    let nanos = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map_or(0, |time| u64::from(time.subsec_nanos()));
    std::time::Duration::from_millis(low + nanos % (high - low + 1))
}

/// Whether an install failed because the host asked to slow down.
fn rate_limited(error: &str) -> bool {
    let error = error.to_ascii_lowercase();
    error.contains("429")
        || error.contains("too many requests")
        || error.contains("rate limit")
        || error.contains("limiting requests")
}

/// What installs: the extension service, or a stand-in in tests.
pub trait Installer: Send + Sync + 'static {
    /// Installs one extension without loading it; `phase` says how far it got.
    fn install(
        &self,
        repo_id: &str,
        extension_id: &str,
        phase: &(dyn Fn(ExtensionInstallPhase) + Send + Sync),
    ) -> impl Future<Output = Result<(), String>> + Send;

    /// Loads every extension again.
    fn reload(&self) -> impl Future<Output = ()> + Send;
}

impl Installer for Arc<crate::extensions::ExtensionService> {
    fn install(
        &self,
        repo_id: &str,
        extension_id: &str,
        phase: &(dyn Fn(ExtensionInstallPhase) + Send + Sync),
    ) -> impl Future<Output = Result<(), String>> + Send {
        self.install_package(repo_id, extension_id, phase)
    }

    async fn reload(&self) {
        crate::extensions::ExtensionService::reload(self).await;
    }
}

fn same(item: &ExtensionInstall, repo_id: &str, extension_id: &str) -> bool {
    item.repo_id == repo_id && item.extension_id == extension_id
}

/// The queue's state, without the running (tested on its own).
#[derive(Debug, Default)]
pub struct Queue {
    batch: u32,
    items: Vec<ExtensionInstall>,
    cancelled: u32,
    /// Installs since extensions last loaded.
    unloaded: bool,
    loading: bool,
}

impl Queue {
    fn active(&self) -> bool {
        self.loading
            || self.items.iter().any(|item| {
                matches!(
                    item.state,
                    ExtensionInstallState::Queued | ExtensionInstallState::Installing
                )
            })
    }

    /// Adds installs at the end; ones already waiting or running aren't added twice, and a failed
    /// one is tried again. Returns how many were added.
    pub fn enqueue(&mut self, requests: &[ExtensionInstallRequest]) -> usize {
        let mut added = 0;
        for request in requests {
            let busy = self.items.iter().any(|item| {
                same(item, &request.repo_id, &request.extension_id)
                    && matches!(
                        item.state,
                        ExtensionInstallState::Queued | ExtensionInstallState::Installing
                    )
            });
            if busy {
                continue;
            }
            if !self.active() {
                // A new batch: the last one's finished installs go; its failures stay listed.
                self.batch += 1;
                self.cancelled = 0;
                self.items
                    .retain(|item| item.state == ExtensionInstallState::Failed);
            }
            self.items
                .retain(|item| !same(item, &request.repo_id, &request.extension_id));
            self.items.push(ExtensionInstall {
                repo_id: request.repo_id.clone(),
                extension_id: request.extension_id.clone(),
                name: request.name.clone(),
                state: ExtensionInstallState::Queued,
                phase: None,
                error: None,
                finished_at: None,
                batch: self.batch,
            });
            added += 1;
        }
        added
    }

    /// The next install, now marked running.
    pub fn start_next(&mut self) -> Option<ExtensionInstallRequest> {
        if self
            .items
            .iter()
            .any(|item| item.state == ExtensionInstallState::Installing)
        {
            return None;
        }
        let item = self
            .items
            .iter_mut()
            .find(|item| item.state == ExtensionInstallState::Queued)?;
        item.state = ExtensionInstallState::Installing;
        item.phase = Some(ExtensionInstallPhase::Downloading);
        Some(ExtensionInstallRequest {
            repo_id: item.repo_id.clone(),
            extension_id: item.extension_id.clone(),
            name: item.name.clone(),
        })
    }

    /// The running install moved on. Returns whether that changed anything.
    pub fn set_phase(&mut self, phase: ExtensionInstallPhase) -> bool {
        match self
            .items
            .iter_mut()
            .find(|item| item.state == ExtensionInstallState::Installing)
        {
            Some(item) if item.phase != Some(phase) => {
                item.phase = Some(phase);
                true
            }
            _ => false,
        }
    }

    /// The running install ended.
    pub fn finish(&mut self, result: Result<(), String>, now_ms: f64) {
        let Some(item) = self
            .items
            .iter_mut()
            .find(|item| item.state == ExtensionInstallState::Installing)
        else {
            return;
        };
        item.phase = None;
        item.finished_at = Some(now_ms);
        match result {
            Ok(()) => {
                item.state = ExtensionInstallState::Done;
                self.unloaded = true;
            }
            Err(error) => {
                item.state = ExtensionInstallState::Failed;
                item.error = Some(error);
            }
        }
    }

    /// Nothing waits: whether extensions should load again now (marked loading).
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

    /// Takes one waiting install out of the queue. A running one finishes.
    pub fn cancel(&mut self, repo_id: &str, extension_id: &str) -> bool {
        let before = self.items.len();
        self.items.retain(|item| {
            !(same(item, repo_id, extension_id) && item.state == ExtensionInstallState::Queued)
        });
        let removed = (before - self.items.len()) as u32;
        self.cancelled += removed;
        removed > 0
    }

    /// Takes every waiting install out of the queue. Returns how many.
    pub fn cancel_all(&mut self) -> usize {
        let before = self.items.len();
        self.items
            .retain(|item| item.state != ExtensionInstallState::Queued);
        let removed = before - self.items.len();
        self.cancelled += removed as u32;
        removed
    }

    /// Failed installs, to try again.
    pub fn failed(&self) -> Vec<ExtensionInstallRequest> {
        self.items
            .iter()
            .filter(|item| item.state == ExtensionInstallState::Failed)
            .map(|item| ExtensionInstallRequest {
                repo_id: item.repo_id.clone(),
                extension_id: item.extension_id.clone(),
                name: item.name.clone(),
            })
            .collect()
    }

    /// Forgets failures: one, or all with `None`.
    pub fn dismiss(&mut self, which: Option<(&str, &str)>) {
        self.items.retain(|item| {
            item.state != ExtensionInstallState::Failed
                || which.is_some_and(|(repo_id, id)| !same(item, repo_id, id))
        });
    }

    pub fn status(&self) -> ExtensionInstallStatus {
        let batch: Vec<&ExtensionInstall> = self
            .items
            .iter()
            .filter(|item| item.batch == self.batch)
            .collect();
        let count = |state: ExtensionInstallState| {
            batch.iter().filter(|item| item.state == state).count() as u32
        };
        let running = self
            .items
            .iter()
            .find(|item| item.state == ExtensionInstallState::Installing);
        // Running first, then waiting ones, then the rest as they ended.
        let mut items = self.items.clone();
        items.sort_by_key(|item| match item.state {
            ExtensionInstallState::Installing => 0,
            ExtensionInstallState::Queued => 1,
            ExtensionInstallState::Failed => 2,
            ExtensionInstallState::Done => 3,
        });
        ExtensionInstallStatus {
            batch: self.batch,
            total: batch.len() as u32,
            done: count(ExtensionInstallState::Done),
            failed: count(ExtensionInstallState::Failed),
            cancelled: self.cancelled,
            running: self.active(),
            current: running.map(|item| item.name.clone()),
            phase: running
                .and_then(|item| item.phase)
                .or(self.loading.then_some(ExtensionInstallPhase::Loading)),
            items,
        }
    }
}

type Announce = Box<dyn Fn(&ExtensionInstallStatus) + Send + Sync>;

/// The queue and its worker.
pub struct ExtensionInstalls {
    queue: Mutex<Queue>,
    wake: Notify,
    announce: Announce,
}

impl ExtensionInstalls {
    /// Starts the worker, announcing every change with `announce`.
    pub fn start<I: Installer>(
        installer: I,
        announce: impl Fn(&ExtensionInstallStatus) + Send + Sync + 'static,
    ) -> Arc<Self> {
        let installs = Arc::new(Self {
            queue: Mutex::new(Queue::default()),
            wake: Notify::new(),
            announce: Box::new(announce),
        });
        let worker = installs.clone();
        tauri::async_runtime::spawn(async move { worker.run(installer).await });
        installs
    }

    /// The manager running in the app.
    pub fn start_in(
        app: &tauri::AppHandle,
        extensions: Arc<crate::extensions::ExtensionService>,
    ) -> Arc<Self> {
        use tauri_specta::Event;
        let handle = app.clone();
        Self::start(extensions, move |status| {
            if let Err(error) = crate::events::ExtensionInstallsEvent(status.clone()).emit(&handle)
            {
                tracing::debug!(%error, "could not announce the extension installs");
            }
        })
    }

    fn change<T>(&self, change: impl FnOnce(&mut Queue) -> T) -> (T, ExtensionInstallStatus) {
        let mut queue = self.queue.lock().expect("install queue poisoned");
        let result = change(&mut queue);
        (result, queue.status())
    }

    fn changed<T>(&self, change: impl FnOnce(&mut Queue) -> T) -> ExtensionInstallStatus {
        let (_, status) = self.change(change);
        (self.announce)(&status);
        status
    }

    pub fn status(&self) -> ExtensionInstallStatus {
        self.queue.lock().expect("install queue poisoned").status()
    }

    pub fn enqueue(&self, requests: &[ExtensionInstallRequest]) -> ExtensionInstallStatus {
        let status = self.changed(|queue| queue.enqueue(requests));
        self.wake.notify_one();
        status
    }

    pub fn cancel(&self, repo_id: &str, extension_id: &str) -> ExtensionInstallStatus {
        self.changed(|queue| queue.cancel(repo_id, extension_id))
    }

    pub fn cancel_all(&self) -> ExtensionInstallStatus {
        self.changed(Queue::cancel_all)
    }

    pub fn retry_failed(&self) -> ExtensionInstallStatus {
        let failed = self.queue.lock().expect("install queue poisoned").failed();
        self.enqueue(&failed)
    }

    pub fn dismiss(&self, which: Option<(&str, &str)>) -> ExtensionInstallStatus {
        self.changed(|queue| queue.dismiss(which))
    }

    async fn run<I: Installer>(self: Arc<Self>, installer: I) {
        loop {
            let (next, _) = self.change(Queue::start_next);
            let Some(request) = next else {
                let (load, _) = self.change(Queue::start_loading);
                if load {
                    self.changed(|_| ());
                    installer.reload().await;
                    self.changed(Queue::finish_loading);
                    continue;
                }
                self.wake.notified().await;
                continue;
            };
            self.changed(|_| ());
            let phase = |phase: ExtensionInstallPhase| {
                let (moved, status) = self.change(|queue| queue.set_phase(phase));
                if moved {
                    (self.announce)(&status);
                }
            };
            // Repositories are often hosted where bulk downloads get rate limited (GitLab, GitHub
            // raw): installs are spaced by a varying gap, and a rate-limited one waits and tries
            // again rather than failing the rest.
            let mut attempt = 0;
            let result = loop {
                let result = installer
                    .install(&request.repo_id, &request.extension_id, &phase)
                    .await;
                match &result {
                    Err(error) if rate_limited(error) && attempt < RATE_LIMIT_RETRIES => {
                        attempt += 1;
                        tracing::info!(extension = %request.extension_id, attempt, "rate limited; waiting before trying again");
                        tokio::time::sleep(RATE_LIMIT_WAIT * attempt).await;
                    }
                    _ => break result,
                }
            };
            tokio::time::sleep(install_gap()).await;
            if let Err(error) = &result {
                tracing::info!(%error, extension = %request.extension_id, "extension didn't install");
            }
            self.changed(|queue| queue.finish(result, crate::catalog::now_ms() as f64));
        }
    }
}

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn request(id: &str) -> ExtensionInstallRequest {
        ExtensionInstallRequest {
            repo_id: "repo".into(),
            extension_id: id.into(),
            name: format!("Name {id}"),
        }
    }

    #[test]
    fn a_batch_counts_its_installs_and_keeps_earlier_failures() {
        let mut queue = Queue::default();
        assert_eq!(
            queue.enqueue(&[request("a"), request("b"), request("a")]),
            2
        );
        assert_eq!(queue.status().total, 2);
        assert!(queue.status().running);

        assert_eq!(queue.start_next().unwrap().extension_id, "a");
        // One at a time.
        assert!(queue.start_next().is_none());
        assert!(queue.set_phase(ExtensionInstallPhase::Converting));
        let status = queue.status();
        assert_eq!(status.current.as_deref(), Some("Name a"));
        assert_eq!(status.phase, Some(ExtensionInstallPhase::Converting));
        queue.finish(Ok(()), 1.0);
        queue.start_next().unwrap();
        queue.finish(Err("the package's checksum doesn't match".into()), 2.0);

        let status = queue.status();
        assert_eq!((status.done, status.failed, status.total), (1, 1, 2));
        assert!(queue.start_loading());
        assert!(queue.status().running);
        assert_eq!(queue.status().phase, Some(ExtensionInstallPhase::Loading));
        queue.finish_loading();
        assert!(!queue.status().running);

        // The next batch drops the installed one and keeps the failure, outside its counts.
        queue.enqueue(&[request("c")]);
        let status = queue.status();
        assert_eq!(status.batch, 2);
        assert_eq!((status.total, status.failed), (1, 0));
        let failed = status
            .items
            .iter()
            .find(|item| item.extension_id == "b")
            .unwrap();
        assert_eq!(
            failed.error.as_deref(),
            Some("the package's checksum doesn't match")
        );
        assert!(!status.items.iter().any(|item| item.extension_id == "a"));
    }

    #[test]
    fn cancelling_leaves_the_running_install() {
        let mut queue = Queue::default();
        queue.enqueue(&[request("a"), request("b"), request("c")]);
        queue.start_next();
        assert!(queue.cancel("repo", "c"));
        assert!(!queue.cancel("repo", "a"), "a running install finishes");
        assert_eq!(queue.cancel_all(), 1);
        let status = queue.status();
        assert_eq!((status.total, status.cancelled), (1, 2));
        queue.finish(Ok(()), 1.0);
        assert!(queue.start_next().is_none());
        assert!(queue.start_loading());
    }

    #[test]
    fn failures_are_retried_or_dismissed() {
        let mut queue = Queue::default();
        queue.enqueue(&[request("a"), request("b")]);
        for _ in 0..2 {
            queue.start_next();
            queue.finish(Err("offline".into()), 1.0);
        }
        // Nothing installed: nothing to load.
        assert!(!queue.start_loading());
        queue.dismiss(Some(("repo", "a")));
        assert_eq!(queue.failed().len(), 1);
        assert_eq!(queue.enqueue(&queue.failed()), 1);
        let status = queue.status();
        assert_eq!((status.total, status.failed), (1, 0));
        assert_eq!(status.items.len(), 1);
        queue.dismiss(None);
        assert_eq!(
            queue.status().items.len(),
            1,
            "a queued install isn't a failure"
        );
    }

    /// Installs `fails` with an error and waits on `gate` for each install.
    struct Fake {
        fails: &'static str,
        gate: Arc<tokio::sync::Semaphore>,
        installed: Arc<Mutex<Vec<String>>>,
        reloads: Arc<Mutex<u32>>,
    }

    impl Installer for Fake {
        async fn install(
            &self,
            _repo_id: &str,
            extension_id: &str,
            phase: &(dyn Fn(ExtensionInstallPhase) + Send + Sync),
        ) -> Result<(), String> {
            phase(ExtensionInstallPhase::Downloading);
            self.gate.acquire().await.unwrap().forget();
            phase(ExtensionInstallPhase::Installing);
            if extension_id == self.fails {
                return Err("the repository no longer lists it".into());
            }
            self.installed.lock().unwrap().push(extension_id.to_owned());
            Ok(())
        }

        async fn reload(&self) {
            *self.reloads.lock().unwrap() += 1;
        }
    }

    async fn until(installs: &ExtensionInstalls, done: impl Fn(&ExtensionInstallStatus) -> bool) {
        for _ in 0..500 {
            if done(&installs.status()) {
                return;
            }
            tokio::time::sleep(Duration::from_millis(5)).await;
        }
        panic!("the queue didn't get there: {:?}", installs.status());
    }

    #[tokio::test(flavor = "multi_thread")]
    async fn the_worker_installs_in_order_then_loads_once() {
        let gate = Arc::new(tokio::sync::Semaphore::new(0));
        let installed = Arc::new(Mutex::new(Vec::new()));
        let reloads = Arc::new(Mutex::new(0));
        let events = Arc::new(Mutex::new(Vec::<ExtensionInstallStatus>::new()));
        let seen = events.clone();
        let installs = ExtensionInstalls::start(
            Fake {
                fails: "b",
                gate: gate.clone(),
                installed: installed.clone(),
                reloads: reloads.clone(),
            },
            move |status| seen.lock().unwrap().push(status.clone()),
        );
        installs.enqueue(&[request("a"), request("b"), request("c"), request("d")]);
        until(&installs, |status| {
            status.current.as_deref() == Some("Name a")
        })
        .await;
        gate.add_permits(2);
        until(&installs, |status| {
            status.current.as_deref() == Some("Name c")
        })
        .await;
        // Cancel while c runs: c finishes, d never starts.
        installs.cancel_all();
        gate.add_permits(1);
        until(&installs, |status| !status.running).await;

        assert_eq!(*installed.lock().unwrap(), ["a", "c"]);
        assert_eq!(*reloads.lock().unwrap(), 1);
        let status = installs.status();
        assert_eq!(
            (status.done, status.failed, status.cancelled, status.total),
            (2, 1, 1, 3)
        );
        {
            let events = events.lock().unwrap();
            assert!(events.iter().any(|status| status.phase
                == Some(ExtensionInstallPhase::Installing)
                && status.current.as_deref() == Some("Name a")));
            assert!(
                events
                    .iter()
                    .any(|status| status.phase == Some(ExtensionInstallPhase::Loading))
            );
        }

        // Retrying the failure is a batch of its own.
        installs.retry_failed();
        gate.add_permits(1);
        until(&installs, |status| !status.running && status.batch == 2).await;
        let status = installs.status();
        assert_eq!((status.total, status.failed), (1, 1));
    }
}
