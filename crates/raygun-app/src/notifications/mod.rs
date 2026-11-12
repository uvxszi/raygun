//! Downloads in the system's notifications, in one place for every kind: songs, videos, chapters,
//! podcast episodes, exports, AI models, Raygun's own update, yt-dlp's jobs, extension installs
//! and lists of sources being added.
//!
//! Nothing here is called by the download services: it listens to the events they already send,
//! reads their lists when one changes (at most once a second, coalescing the rest), and works out
//! the notifications in [`model`]. On Android the `raygun-audio` plugin shows them in their own
//! channel, grouped when several run, and keeps Raygun running in the background while anything
//! downloads; their buttons come back here. On desktop, a system notification says when a batch
//! ends, unless Raygun's window is in front.
//!
//! Tapping a notification opens a screen: the route waits here until the UI takes it.

mod model;
pub mod news;
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_model::{
    AiModelProgress, AiModelState, DownloadState, ExportState, PermissionReason, SongDownloadState,
    UpdateState, WebsiteDownloadState, YtDlpState,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::events::{
    AiModelEvent, AppUpdateEvent, ExportEvent, ExtensionInstallsEvent, MangaDownloadsEvent,
    OpenRouteEvent, PodcastsChangedEvent, SongDownloadProgressEvent, SongDownloadsEvent,
    SourceQueueEvent, VideoDownloadsEvent, WebsiteDownloadsEvent, YtDlpEvent,
};
use model::{Abilities, Download, Kind, Phase, Plan, Tracker, Unit};

/// Notifications change at most this often (Android drops updates that come faster).
const TICK: Duration = Duration::from_secs(1);

/// A download cancelled from its notification is left out for this long while its service
/// stops it (an export notices at its next chapter), so the notification goes at once.
const CANCEL_HIDES: Duration = Duration::from_secs(120);

const ALL_KINDS: [Kind; 11] = [
    Kind::Songs,
    Kind::Videos,
    Kind::Chapters,
    Kind::Podcasts,
    Kind::Exports,
    Kind::Models,
    Kind::Update,
    Kind::YtDlp,
    Kind::Files,
    Kind::Extensions,
    Kind::Sources,
];

/// Where a [`Notice`] goes on Android: its channel, which people can mute on its own.
pub struct NoticeChannel {
    pub id: &'static str,
    pub name: &'static str,
    pub description: &'static str,
    /// Whether its notices may make a sound, vibrate and pop up. Only news does (new releases,
    /// chapters and episodes, Raygun's own update); everything else is silent. Android keeps a
    /// channel's importance as first made, so a channel that goes quiet takes a new id and its old
    /// one is listed in [`RETIRED_CHANNELS`].
    pub alerts: bool,
}

/// Followed artists' new releases (M46.42).
pub const RELEASES: NoticeChannel = NoticeChannel {
    id: "raygun.releases",
    name: "New releases",
    description: "Releases from artists you follow",
    alerts: true,
};

/// New chapters and episodes found by library updates and feed refreshes, one per title.
pub const LIBRARY_UPDATES: NoticeChannel = NoticeChannel {
    id: "raygun.library.updates",
    name: "Library updates",
    description: "New chapters and episodes in your library",
    alerts: true,
};

/// A newer Raygun, found by a background check.
pub const APP_UPDATES: NoticeChannel = NoticeChannel {
    id: "raygun.app.updates",
    name: "Raygun updates",
    description: "A new version of Raygun",
    alerts: true,
};

/// What waits for Raygun to be open: a song in YouTube's player paused, songs to record.
pub const PLAYBACK: NoticeChannel = NoticeChannel {
    id: "raygun.playback.quiet",
    name: "Playback",
    description: "Songs that wait for Raygun to be open",
    alerts: false,
};

/// Every channel [`Notice`]s use.
#[cfg_attr(not(test), allow(dead_code))]
pub const CHANNELS: [&NoticeChannel; 4] = [&RELEASES, &LIBRARY_UPDATES, &APP_UPDATES, &PLAYBACK];

/// Channel ids no longer used (made with a sound before they went quiet): the plugin deletes them.
pub const RETIRED_CHANNELS: [&str; 1] = ["raygun.playback"];

/// A notification besides the downloads' own that opens a screen when tapped.
pub struct Notice {
    /// Tells it from the others of its channel: the same id replaces the one shown.
    pub id: i32,
    pub channel: &'static NoticeChannel,
    pub title: String,
    pub text: String,
    /// What a tap opens (see [`DownloadNotices::open`]).
    pub route: String,
}

impl Notice {
    /// This notice with both lines saying something (Android shows a notification without a
    /// title or text as an empty one): a blank title is its channel's name, a blank text its
    /// channel's description. `None` when it says nothing at all: it isn't shown.
    fn filled(self) -> Option<Notice> {
        let title = self.title.trim();
        let text = self.text.trim();
        if title.is_empty() && text.is_empty() {
            return None;
        }
        Some(Notice {
            title: if title.is_empty() {
                self.channel.name.to_owned()
            } else {
                title.to_owned()
            },
            text: if text.is_empty() {
                self.channel.description.to_owned()
            } else {
                text.to_owned()
            },
            ..self
        })
    }

    /// What the `raygun-audio` plugin shows on Android.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    fn to_json(&self) -> serde_json::Value {
        serde_json::json!({
            "id": self.id,
            "channel": self.channel.id,
            "channelName": self.channel.name,
            "channelDescription": self.channel.description,
            "alerts": self.channel.alerts,
            "retiredChannels": RETIRED_CHANNELS,
            "title": self.title,
            "text": self.text,
            "route": self.route,
        })
    }
}

/// A notification id for `key` that stays the same across runs (FNV-1a), never negative.
pub fn notice_id(key: &str) -> i32 {
    let hash = key.bytes().fold(0x811c_9dc5_u32, |hash, byte| {
        (hash ^ u32::from(byte)).wrapping_mul(0x0100_0193)
    });
    (hash & 0x7fff_ffff) as i32
}

/// Shows `notice`: on Android through the `raygun-audio` plugin, whose tap brings the route back
/// through [`DownloadNotices::open`]; elsewhere as a system notification carrying the route,
/// which a click opens while it's on screen.
pub async fn show_notice(app: &AppHandle, notice: Notice) {
    let id = notice.id;
    let Some(notice) = notice.filled() else {
        tracing::debug!(id, "a notification with nothing to say isn't shown");
        return;
    };
    if let Err(error) = platform::notice(app, &notice).await {
        tracing::debug!(%error, title = %notice.title, "couldn't show a notification");
    }
}

/// Takes away the notification [`show_notice`] showed with `id`: what it said is over (Android;
/// desktop notifications go on their own).
pub async fn cancel_notice(app: &AppHandle, id: i32) {
    if let Err(error) = platform::cancel_notice(app, id).await {
        tracing::debug!(%error, id, "couldn't take a notification away");
    }
}

/// The route a click on a desktop notification opens: the one it carries, if it was a click on
/// the notification itself.
#[cfg_attr(target_os = "android", allow(dead_code))]
fn tapped_route(performed: &tauri_plugin_notification::ActionPerformed) -> Option<String> {
    if performed.action_id() != "tap" {
        return None;
    }
    let route = performed.notification()?.extra().get("route")?.as_str()?;
    (!route.is_empty()).then(|| route.to_owned())
}

/// Follows every download for its notifications.
pub struct DownloadNotices {
    app: AppHandle,
    dirty: Mutex<HashSet<Kind>>,
    wake: Notify,
    /// AI models' progress, from their events (the service keeps only received bytes).
    models: Mutex<HashMap<String, AiModelProgress>>,
    /// Songs as their service last reported them: progress from `SongDownloadProgressEvent`,
    /// and the whole list again when it changes (`SongDownloadsEvent`).
    songs: Mutex<HashMap<String, raygun_model::SongDownload>>,
    songs_changed: std::sync::atomic::AtomicBool,
    /// Downloads cancelled from their notification, by key, and when.
    cancelled: Mutex<HashMap<String, Instant>>,
    /// The screen a tapped notification opens, until the UI takes it.
    route: Mutex<Option<String>>,
    /// Something downloads or waits to (the last plan shown keeps Raygun in the foreground).
    running: std::sync::atomic::AtomicBool,
}

impl DownloadNotices {
    pub fn start(app: &AppHandle) {
        let notices = Arc::new(Self {
            app: app.clone(),
            dirty: Mutex::new(ALL_KINDS.into_iter().collect()),
            wake: Notify::new(),
            models: Mutex::new(HashMap::new()),
            songs: Mutex::new(HashMap::new()),
            songs_changed: std::sync::atomic::AtomicBool::new(true),
            cancelled: Mutex::new(HashMap::new()),
            route: Mutex::new(None),
            running: std::sync::atomic::AtomicBool::new(false),
        });
        app.manage(notices.clone());
        notices.listen();
        platform::install(app);
        // The first read: downloads that carry on from the last run.
        notices.wake.notify_one();
        tauri::async_runtime::spawn(async move { notices.run().await });
    }

    fn listen(self: &Arc<Self>) {
        let app = &self.app;
        let notices = self.clone();
        SongDownloadsEvent::listen(app, move |_| {
            notices
                .songs_changed
                .store(true, std::sync::atomic::Ordering::SeqCst);
            notices.touch(Kind::Songs);
        });
        let notices = self.clone();
        SongDownloadProgressEvent::listen(app, move |event| {
            let song = event.payload.0;
            {
                let mut songs = notices.songs.lock().expect("songs poisoned");
                match takes_report(songs.get(&song.id).map(|known| known.state), song.state) {
                    Report::Take => {
                        songs.insert(song.id.clone(), song);
                    }
                    Report::Drop => return,
                    // Not in the last list: removed (a late report from its worker), or new;
                    // the list says which.
                    Report::ReadList => notices
                        .songs_changed
                        .store(true, std::sync::atomic::Ordering::SeqCst),
                }
            }
            notices.touch(Kind::Songs);
        });
        let notices = self.clone();
        VideoDownloadsEvent::listen(app, move |_| notices.touch(Kind::Videos));
        let notices = self.clone();
        MangaDownloadsEvent::listen(app, move |_| notices.touch(Kind::Chapters));
        let notices = self.clone();
        PodcastsChangedEvent::listen(app, move |_| notices.touch(Kind::Podcasts));
        let notices = self.clone();
        ExportEvent::listen(app, move |_| notices.touch(Kind::Exports));
        let notices = self.clone();
        AppUpdateEvent::listen(app, move |_| notices.touch(Kind::Update));
        let notices = self.clone();
        YtDlpEvent::listen(app, move |_| notices.touch(Kind::YtDlp));
        let notices = self.clone();
        WebsiteDownloadsEvent::listen(app, move |_| notices.touch(Kind::Files));
        let notices = self.clone();
        ExtensionInstallsEvent::listen(app, move |_| notices.touch(Kind::Extensions));
        let notices = self.clone();
        SourceQueueEvent::listen(app, move |_| notices.touch(Kind::Sources));
        let notices = self.clone();
        AiModelEvent::listen(app, move |event| {
            let progress = event.payload.0;
            {
                let mut models = notices.models.lock().expect("models poisoned");
                // A report sent before the model's end doesn't bring it back as running.
                let ended = models
                    .get(&progress.id)
                    .is_some_and(|known| known.state != AiModelState::Downloading);
                let running = notices
                    .app
                    .try_state::<crate::ai::AiService>()
                    .is_none_or(|ai| ai.is_downloading(&progress.id));
                if ended && progress.state == AiModelState::Downloading && !running {
                    return;
                }
                models.insert(progress.id.clone(), progress);
            }
            notices.touch(Kind::Models);
        });
    }

    /// A kind's list changed: read it at the next turn.
    fn touch(&self, kind: Kind) {
        self.dirty.lock().expect("dirty poisoned").insert(kind);
        self.wake.notify_one();
    }

    /// A tapped notification's screen. Routes on separate lines open one over another, and a
    /// `reveal` in a route's search names the row to show there (the UI's `useNotificationRoutes`).
    pub fn open(&self, route: String) {
        *self.route.lock().expect("route poisoned") = Some(route.clone());
        if let Err(error) = OpenRouteEvent(route).emit(&self.app) {
            tracing::debug!(%error, "couldn't send a notification's screen");
        }
    }

    /// The screen a tapped notification opens, taken (the UI asks when it starts).
    pub fn take_route(&self) -> Option<String> {
        self.route.lock().expect("route poisoned").take()
    }

    /// Whether something downloads or waits to, as last shown.
    pub fn running(&self) -> bool {
        self.running.load(std::sync::atomic::Ordering::SeqCst)
    }

    async fn run(self: Arc<Self>) {
        let mut tracker = Tracker::default();
        let mut lists: HashMap<Kind, Vec<Download>> = HashMap::new();
        let mut memory = Memory::default();
        let mut shown = Plan::default();
        let mut sent_at: Option<f64> = None;
        let mut last = Instant::now() - TICK;
        let mut asked = false;
        loop {
            // Woken by an event; and every second while something moves, for speeds.
            if shown.summary.is_some() {
                tokio::select! {
                    _ = self.wake.notified() => {}
                    _ = tokio::time::sleep(TICK) => {}
                }
            } else {
                self.wake.notified().await;
            }
            let since = last.elapsed();
            if since < TICK {
                tokio::time::sleep(TICK - since).await;
            }
            last = Instant::now();
            let mut kinds: HashSet<Kind> =
                std::mem::take(&mut *self.dirty.lock().expect("dirty poisoned"));
            // Ones still moving are read again: their speed and time left change.
            for (kind, list) in &lists {
                if list.iter().any(|download| download.phase == Phase::Running) {
                    kinds.insert(*kind);
                }
            }
            for kind in kinds {
                let list = self.collect(kind, &tracker, &mut memory).await;
                lists.insert(kind, list);
            }
            // Read at every turn: battery saver can come and go between two lists.
            let held = crate::device_state::downloads_held();
            let downloads: Vec<Download> = lists
                .values()
                .flatten()
                .cloned()
                .map(|download| hold(download, held))
                .collect();
            let now = crate::catalog::now_ms() as f64;
            let update = tracker.update(now, &downloads);
            self.running
                .store(update.plan.foreground, std::sync::atomic::Ordering::SeqCst);
            if model::should_send(sent_at.map(|at| (at, &shown)), now, &update) {
                platform::show(&self.app, &update.plan, update.finished.as_ref()).await;
                shown = update.plan;
                sent_at = Some(now);
            }
            if !asked && !update.started.is_empty() {
                asked = true;
                if let Some(prompts) = self
                    .app
                    .try_state::<crate::permissions::PermissionPrompts>()
                {
                    prompts.offer(PermissionReason::Download).await;
                }
            }
        }
    }

    /// One kind's downloads now: the running and waiting ones, and ones of this batch that ended.
    async fn collect(&self, kind: Kind, tracker: &Tracker, memory: &mut Memory) -> Vec<Download> {
        let downloads = match kind {
            Kind::Songs => self.songs_now().await,
            Kind::Videos => videos(&self.app).await,
            Kind::Chapters => chapters(&self.app, memory).await,
            Kind::Podcasts => podcasts(&self.app, memory).await,
            Kind::Exports => exports(&self.app),
            Kind::Models => self.models_now(),
            Kind::Update => update(&self.app),
            Kind::YtDlp => ytdlp(&self.app),
            Kind::Files => files(&self.app),
            Kind::Extensions => {
                state::<Arc<crate::extension_installs::ExtensionInstalls>>(&self.app)
                    .ok()
                    .and_then(|installs| model::extension_installs(&installs.status()))
                    .into_iter()
                    .collect()
            }
            Kind::Sources => state::<Arc<crate::source_queue::SourceQueue>>(&self.app)
                .ok()
                .and_then(|queue| model::source_queue(&queue.status()))
                .into_iter()
                .collect(),
        };
        let mut cancelled = self.cancelled.lock().expect("cancelled poisoned");
        let prefix = format!("{}:", kind.tag());
        cancelled.retain(|key, at| {
            !key.starts_with(&prefix)
                || (at.elapsed() < CANCEL_HIDES
                    && downloads
                        .iter()
                        .any(|download| download.key() == *key && download.phase.active()))
        });
        downloads
            .into_iter()
            .filter(|download| {
                !cancelled.contains_key(&download.key())
                    && (!matches!(download.phase, Phase::Done | Phase::Failed)
                        || tracker.watching(&download.key()))
            })
            .collect()
    }

    /// The songs, from the last list and the progress since; the list is read again only when
    /// it changed, not for progress.
    async fn songs_now(&self) -> Vec<Download> {
        let service = state::<Arc<crate::music_downloads::SongDownloadService>>(&self.app).ok();
        if self
            .songs_changed
            .swap(false, std::sync::atomic::Ordering::SeqCst)
            && let Some(service) = &service
            && let Ok(list) = service.list().await
        {
            let mut songs = self.songs.lock().expect("songs poisoned");
            songs.clear();
            songs.extend(list.into_iter().map(|song| (song.id.clone(), song)));
        }
        let list: Vec<raygun_model::SongDownload> = self
            .songs
            .lock()
            .expect("songs poisoned")
            .values()
            .cloned()
            .collect();
        songs(list, service.is_some_and(|service| service.paused()))
    }

    fn models_now(&self) -> Vec<Download> {
        let ai = self.app.try_state::<crate::ai::AiService>();
        let names: HashMap<String, String> = ai
            .as_ref()
            .map(|ai| {
                ai.models()
                    .into_iter()
                    .map(|model| (model.id, model.name))
                    .collect()
            })
            .unwrap_or_default();
        let mut models = self.models.lock().expect("models poisoned");
        // A progress report that arrived after the model's end (or one whose download stopped
        // without an end) isn't a download running.
        if let Some(ai) = &ai {
            models.retain(|id, progress| {
                progress.state != AiModelState::Downloading || ai.is_downloading(id)
            });
        }
        // Finished ones are only kept while their batch is reported.
        let list: Vec<Download> = models
            .values()
            .filter_map(|progress| {
                let title = names
                    .get(&progress.id)
                    .cloned()
                    .unwrap_or_else(|| progress.id.clone());
                model_download(progress, title)
            })
            .collect();
        models.retain(|_, progress| progress.state == AiModelState::Downloading);
        list
    }

    /// A notification's button: Pause, Resume, Cancel, Skip, Cancel all, Retry or Stop, for
    /// `keys`.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
    pub async fn act(&self, action: &str, keys: &[String]) {
        let mut queues_done: HashSet<Kind> = HashSet::new();
        for key in keys {
            let Some((tag, id)) = key.split_once(':') else {
                continue;
            };
            let Some(kind) = Kind::from_tag(tag) else {
                continue;
            };
            // What leaves its queue goes from the notifications at once.
            if matches!(action, "cancel" | "skip" | "stop" | "cancelAll") {
                self.cancelled
                    .lock()
                    .expect("cancelled poisoned")
                    .insert(key.clone(), Instant::now());
            }
            // Pausing, resuming and cancelling all act on a whole queue: once is enough.
            if matches!(action, "pause" | "resume" | "cancelAll") && !queues_done.insert(kind) {
                continue;
            }
            if let Err(error) = act_on(&self.app, kind, id, action).await {
                tracing::info!(%error, %key, action, "a notification's button didn't work");
            }
            self.touch(kind);
        }
    }
}

#[cfg_attr(not(target_os = "android"), allow(dead_code))]
async fn act_on(app: &AppHandle, kind: Kind, id: &str, action: &str) -> Result<(), String> {
    use crate::manga_downloads::MangaDownloadService;
    use crate::music_downloads::SongDownloadService;
    use crate::video_downloads::VideoDownloadService;

    match (kind, action) {
        (Kind::Songs, "pause" | "resume") => {
            state::<Arc<SongDownloadService>>(app)?.set_paused(action == "pause");
            Ok(())
        }
        (Kind::Songs, "cancel") => state::<Arc<SongDownloadService>>(app)?.remove(id).await,
        // The song downloading now is skipped (it can be tried again); the queue goes on.
        (Kind::Songs, "skip") => state::<Arc<SongDownloadService>>(app)?.skip(id).await,
        (Kind::Songs, "cancelAll") => state::<Arc<SongDownloadService>>(app)?.cancel_all().await,
        // A recording stops, keeping what it recorded.
        (Kind::Songs, "stop") => {
            state::<Arc<SongDownloadService>>(app)?
                .cancel_recordings(Some(id))
                .await
        }
        (Kind::Songs, "retry") => state::<Arc<SongDownloadService>>(app)?.retry(id).await,
        (Kind::Videos, "pause" | "resume") => {
            state::<Arc<VideoDownloadService>>(app)?.set_paused(action == "pause");
            Ok(())
        }
        (Kind::Videos, "cancel") => state::<Arc<VideoDownloadService>>(app)?.remove(id).await,
        (Kind::Videos, "retry") => state::<Arc<VideoDownloadService>>(app)?.retry(id).await,
        (Kind::Chapters, "pause" | "resume") => {
            state::<Arc<MangaDownloadService>>(app)?.set_paused(action == "pause");
            Ok(())
        }
        (Kind::Chapters, "cancel") => {
            state::<Arc<MangaDownloadService>>(app)?
                .remove(&[id.to_owned()])
                .await
        }
        (Kind::Chapters, "retry") => {
            state::<Arc<MangaDownloadService>>(app)?
                .enqueue(&[id.to_owned()])
                .await
        }
        (Kind::Podcasts, "cancel") => {
            state::<Arc<crate::podcasts::PodcastService>>(app)?
                .cancel_download(id)
                .await
        }
        (Kind::Podcasts, "retry") => {
            state::<Arc<crate::podcasts::PodcastService>>(app)?
                .download(id)
                .await
        }
        (Kind::Exports, "cancel") => {
            state::<Arc<crate::export::ExportService>>(app)?.cancel(id);
            Ok(())
        }
        (Kind::Models, "cancel") => {
            if let Some(ai) = app.try_state::<crate::ai::AiService>() {
                ai.cancel_model_download(id);
            }
            Ok(())
        }
        (Kind::Models, "retry") => {
            let app = app.clone();
            let id = id.to_owned();
            // Runs until the model is in; its events say how it goes.
            tauri::async_runtime::spawn(async move {
                if let Some(ai) = app.try_state::<crate::ai::AiService>()
                    && let Err(error) = ai.download_model(&id).await
                {
                    tracing::info!(%error, "the model didn't download again");
                }
            });
            Ok(())
        }
        (Kind::Update, "retry") => {
            let updates = state::<Arc<crate::updates::UpdateService>>(app)?;
            tauri::async_runtime::spawn(async move {
                if let Err(error) = updates.install().await {
                    tracing::info!(%error, "the update didn't download again");
                }
            });
            Ok(())
        }
        // The batch's Cancel stops the installs still waiting; Retry queues its failures again.
        (Kind::Extensions, "cancel") => {
            state::<Arc<crate::extension_installs::ExtensionInstalls>>(app)?.cancel_all();
            Ok(())
        }
        (Kind::Extensions, "retry") => {
            state::<Arc<crate::extension_installs::ExtensionInstalls>>(app)?.retry_failed();
            Ok(())
        }
        // A list's Cancel calls off what waits; Retry queues its failures again.
        (Kind::Sources, "cancel") => {
            state::<Arc<crate::source_queue::SourceQueue>>(app)?.cancel(None);
            Ok(())
        }
        (Kind::Sources, "retry") => {
            state::<Arc<crate::source_queue::SourceQueue>>(app)?.retry();
            Ok(())
        }
        (Kind::Files, "cancel") => {
            state::<Arc<crate::website_downloads::WebsiteDownloadService>>(app)?.cancel(id);
            Ok(())
        }
        (Kind::Files, "retry") => {
            state::<Arc<crate::website_downloads::WebsiteDownloadService>>(app)?.retry(id)
        }
        (Kind::YtDlp, "cancel") => {
            state::<Arc<crate::ytdlp::YtDlpService>>(app)?.cancel(id);
            Ok(())
        }
        _ => Err(format!("no {action} for {}", kind.tag())),
    }
}

fn state<T: Clone + Send + Sync + 'static>(app: &AppHandle) -> Result<T, String> {
    app.try_state::<T>()
        .map(|state| state.inner().clone())
        .ok_or_else(|| "not ready yet".to_owned())
}

/// What the lists don't say from one read to the next.
#[derive(Default)]
struct Memory {
    /// Chapters seen in the queue, by id: done ones leave it.
    chapters: HashMap<String, Download>,
    /// Episode titles, their podcast's and its id, by episode.
    episodes: HashMap<String, (String, String, String)>,
}

/// What's done with a song's progress report.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Report {
    Take,
    Drop,
    /// The song isn't in the last list read: the list is read again instead.
    ReadList,
}

/// What's done with a song's progress report, given what's known of it. A late report from a
/// download that already ended, or that was removed, doesn't bring it back as running (it would
/// show as downloading for good); starting it again is read from the list.
fn takes_report(known: Option<SongDownloadState>, report: SongDownloadState) -> Report {
    let ended = |state| matches!(state, SongDownloadState::Done | SongDownloadState::Failed);
    match known {
        None => Report::ReadList,
        Some(known) if ended(known) && !ended(report) => Report::Drop,
        Some(_) => Report::Take,
    }
}

/// An AI model's download as notifications show it: a running one can be cancelled, a failed one
/// tried again; a cancelled one isn't shown.
fn model_download(progress: &AiModelProgress, title: String) -> Option<Download> {
    let phase = match progress.state {
        AiModelState::Downloading => Phase::Running,
        AiModelState::Installed => Phase::Done,
        AiModelState::Failed => Phase::Failed,
        AiModelState::Cancelled => return None,
    };
    Some(Download {
        kind: Kind::Models,
        id: progress.id.clone(),
        title,
        subtitle: None,
        phase,
        done: progress.received_bytes,
        total: Some(progress.total_bytes).filter(|total| *total > 0.0),
        unit: Unit::Bytes,
        bytes: None,
        left_ms: None,
        speed: None,
        error: progress.error.clone(),
        abilities: Abilities {
            cancel: phase == Phase::Running,
            retry: phase == Phase::Failed,
            ..Abilities::default()
        },
        route: Kind::Models.route().to_owned(),
    })
}

/// A waiting download whose queue battery saver holds shows as held, not as about to start;
/// Offline Mode holds every queue.
fn hold(mut download: Download, held: bool) -> Download {
    if held
        && download.phase == Phase::Queued
        && (download.kind.waits_in_battery_saver() || raygun_net::network_mode::offline())
    {
        download.phase = Phase::Held;
    }
    download
}

/// The songs as notifications see them; `paused` when the song queue is.
fn songs(list: Vec<raygun_model::SongDownload>, paused: bool) -> Vec<Download> {
    list.into_iter()
        .map(|song| {
            let phase = match song.state {
                SongDownloadState::Queued | SongDownloadState::Downloading if paused => {
                    Phase::Paused
                }
                SongDownloadState::Queued => Phase::Queued,
                SongDownloadState::Downloading => Phase::Running,
                SongDownloadState::Done => Phase::Done,
                SongDownloadState::Failed => Phase::Failed,
            };
            if phase == Phase::Running
                && song.kind == Some(raygun_model::SongDownloadKind::Recording)
            {
                return recording(song);
            }
            // A capture (M46.49) counts the share of the song it holds.
            let capture = song.kind == Some(raygun_model::SongDownloadKind::Capture);
            let captured = (phase == Phase::Running && capture)
                .then_some(song.fraction)
                .flatten();
            let (done, total, unit) = match captured {
                Some(share) => (share, Some(1.0), Unit::Fraction),
                None => (song.done_bytes, song.total_bytes, Unit::Bytes),
            };
            // Before any of it arrives: what it's doing, rather than a bar alone.
            let doing = (phase == Phase::Running && done <= 0.0 && captured.is_none()).then(|| {
                match (&song.provider, capture) {
                    (Some(source), true) => format!("Capturing from {source}"),
                    (None, true) => "Capturing".to_owned(),
                    (Some(source), false) => format!("From {source}"),
                    (None, false) => "Finding a source".to_owned(),
                }
            });
            let subtitle = match (song.artist, doing) {
                (Some(artist), Some(doing)) => Some(format!("{artist} · {doing}")),
                (artist, doing) => artist.or(doing),
            };
            Download {
                kind: Kind::Songs,
                title: song.title,
                subtitle,
                phase,
                done,
                total,
                unit,
                bytes: captured.map(|_| song.done_bytes),
                left_ms: song.eta_ms,
                speed: song.bytes_per_second,
                error: song.error,
                abilities: Abilities {
                    pause: true,
                    cancel: true,
                    retry: song.state == SongDownloadState::Failed,
                    skip: true,
                    stop: false,
                },
                route: Kind::Songs.route().to_owned(),
                id: song.id,
            }
        })
        .collect()
}

/// A song being recorded from YouTube's player (M46.52), as its notification shows it: how much
/// of the song it has and the time it takes (a recording runs as the song plays), with Stop,
/// which keeps what it recorded for Retry.
fn recording(song: raygun_model::SongDownload) -> Download {
    let share = song.fraction.unwrap_or(0.0).clamp(0.0, 1.0);
    let left_ms = song
        .duration_ms
        .filter(|ms| *ms > 0)
        .map(|ms| f64::from(ms) * (1.0 - share));
    let subtitle = match song.artist {
        Some(artist) => format!("Recording · {artist}"),
        None => "Recording".to_owned(),
    };
    Download {
        kind: Kind::Songs,
        title: song.title,
        subtitle: Some(subtitle),
        phase: Phase::Running,
        done: share,
        total: Some(1.0),
        unit: Unit::Fraction,
        bytes: None,
        left_ms,
        speed: None,
        error: song.error,
        abilities: Abilities {
            stop: true,
            ..Abilities::default()
        },
        route: Kind::Songs.route().to_owned(),
        id: song.id,
    }
}

async fn videos(app: &AppHandle) -> Vec<Download> {
    let Ok(service) = state::<Arc<crate::video_downloads::VideoDownloadService>>(app) else {
        return Vec::new();
    };
    let Ok(list) = service.list().await else {
        return Vec::new();
    };
    list.items
        .into_iter()
        .map(|item| {
            let phase = match item.state {
                DownloadState::Queued if list.paused => Phase::Paused,
                DownloadState::Downloading if list.paused => Phase::Paused,
                DownloadState::Queued => Phase::Queued,
                DownloadState::Downloading => Phase::Running,
                DownloadState::Done => Phase::Done,
                DownloadState::Failed => Phase::Failed,
            };
            // A streamed video counts its pieces; a file, its bytes.
            let (done, total, unit, bytes) = match item.total_parts {
                Some(parts) => (
                    f64::from(item.done_parts),
                    Some(f64::from(parts)),
                    Unit::Parts,
                    Some(item.done_bytes),
                ),
                None => (item.done_bytes, item.total_bytes, Unit::Bytes, None),
            };
            Download {
                kind: Kind::Videos,
                id: item.id,
                title: item.title,
                subtitle: item.subtitle,
                phase,
                done,
                total,
                unit,
                bytes,
                left_ms: None,
                speed: None,
                error: item.error,
                abilities: Abilities {
                    pause: true,
                    cancel: true,
                    retry: item.state == DownloadState::Failed,
                    ..Abilities::default()
                },
                route: Kind::Videos.route().to_owned(),
            }
        })
        .collect()
}

async fn chapters(app: &AppHandle, memory: &mut Memory) -> Vec<Download> {
    let Ok(service) = state::<Arc<crate::manga_downloads::MangaDownloadService>>(app) else {
        return Vec::new();
    };
    let Ok(queue) = service.queue().await else {
        return Vec::new();
    };
    let mut list: Vec<Download> = queue
        .items
        .into_iter()
        .map(|item| {
            let state = item.download.state;
            let phase = match state {
                DownloadState::Queued | DownloadState::Downloading if queue.paused => Phase::Paused,
                DownloadState::Queued => Phase::Queued,
                DownloadState::Downloading => Phase::Running,
                DownloadState::Done => Phase::Done,
                DownloadState::Failed => Phase::Failed,
            };
            Download {
                kind: Kind::Chapters,
                id: item.chapter_id,
                title: item.title,
                subtitle: Some(item.chapter),
                phase,
                done: f64::from(item.download.pages_done),
                total: item.download.pages_total.map(f64::from),
                unit: Unit::Pages,
                bytes: None,
                left_ms: None,
                speed: None,
                error: item.download.error,
                abilities: Abilities {
                    pause: true,
                    cancel: true,
                    retry: state == DownloadState::Failed,
                    ..Abilities::default()
                },
                route: Kind::Chapters.route().to_owned(),
            }
        })
        .collect();
    // Done chapters leave the queue: the ones that did now are told apart from removed ones.
    let present: HashSet<String> = list.iter().map(|download| download.id.clone()).collect();
    let gone: Vec<String> = memory
        .chapters
        .keys()
        .filter(|id| !present.contains(*id))
        .cloned()
        .collect();
    if !gone.is_empty() {
        let states = service.for_chapters(&gone).await;
        for id in gone {
            let Some(mut last) = memory.chapters.remove(&id) else {
                continue;
            };
            if states
                .get(&id)
                .is_some_and(|download| download.state == DownloadState::Done)
            {
                last.phase = Phase::Done;
                if let Some(total) = last.total {
                    last.done = total;
                }
                list.push(last);
            }
        }
    }
    for download in &list {
        if download.phase != Phase::Done {
            memory
                .chapters
                .insert(download.id.clone(), download.clone());
        }
    }
    list
}

async fn podcasts(app: &AppHandle, memory: &mut Memory) -> Vec<Download> {
    let Some(db) = app
        .try_state::<crate::startup::AppDb>()
        .map(|db| db.0.clone())
    else {
        return Vec::new();
    };
    let Ok(service) = state::<Arc<crate::podcasts::PodcastService>>(app) else {
        return Vec::new();
    };
    let Ok(rows) = raygun_db::podcasts::downloads(&db).await else {
        return Vec::new();
    };
    let mut list = Vec::new();
    for row in rows {
        let unfinished = matches!(row.state.as_str(), "downloading" | "queued");
        // The service says which run: a record a run that ended left behind, or one cancelled
        // while its job hasn't noticed yet, isn't shown as downloading.
        if unfinished && !service.download_active(&row.episode_id) {
            continue;
        }
        let names = match memory.episodes.get(&row.episode_id) {
            Some(names) => names.clone(),
            None => {
                let Ok(Some(episode)) = raygun_db::podcasts::episode(&db, &row.episode_id).await
                else {
                    continue;
                };
                let names = (
                    episode.fields.title,
                    episode.podcast_title,
                    episode.podcast_id,
                );
                memory
                    .episodes
                    .insert(row.episode_id.clone(), names.clone());
                names
            }
        };
        let (title, podcast, podcast_id) = names;
        let phase = match row.state.as_str() {
            "downloading" => Phase::Running,
            // Waiting for one of the two turns.
            "queued" => Phase::Queued,
            "done" => Phase::Done,
            _ => Phase::Failed,
        };
        list.push(Download {
            kind: Kind::Podcasts,
            id: row.episode_id,
            title,
            subtitle: Some(podcast),
            phase,
            done: row.received_bytes as f64,
            total: row.total_bytes.map(|total| total as f64),
            unit: Unit::Bytes,
            bytes: None,
            left_ms: None,
            speed: None,
            error: row.error,
            abilities: Abilities {
                pause: false,
                cancel: phase.active(),
                retry: phase == Phase::Failed,
                ..Abilities::default()
            },
            route: format!("/podcast?id={podcast_id}"),
        });
    }
    list
}

fn exports(app: &AppHandle) -> Vec<Download> {
    let Ok(service) = state::<Arc<crate::export::ExportService>>(app) else {
        return Vec::new();
    };
    service
        .jobs()
        .into_iter()
        .filter_map(|job| {
            let phase = match job.state {
                ExportState::Downloading | ExportState::Writing => Phase::Running,
                ExportState::Done => Phase::Done,
                ExportState::Failed => Phase::Failed,
                ExportState::Cancelled => return None,
            };
            let total = job.to_download + job.to_write;
            Some(Download {
                kind: Kind::Exports,
                id: job.id,
                title: job.label,
                subtitle: (phase == Phase::Running).then(|| {
                    match job.state {
                        ExportState::Downloading => "Downloading chapters",
                        _ if job.progress.is_some() => "Converting",
                        _ => "Writing",
                    }
                    .to_owned()
                }),
                phase,
                // A video being converted counts its share of the file it's on.
                done: f64::from(job.downloaded + job.written) + job.progress.unwrap_or(0.0),
                total: (total > 0).then(|| f64::from(total)),
                unit: Unit::Chapters,
                bytes: None,
                left_ms: None,
                speed: None,
                error: job.message,
                abilities: Abilities {
                    cancel: true,
                    ..Abilities::default()
                },
                route: Kind::Exports.route().to_owned(),
            })
        })
        .collect()
}

fn update(app: &AppHandle) -> Vec<Download> {
    let Ok(service) = state::<Arc<crate::updates::UpdateService>>(app) else {
        return Vec::new();
    };
    let update = service.current();
    let phase = match update.state {
        UpdateState::Downloading => Phase::Running,
        UpdateState::Installing => Phase::Done,
        UpdateState::Failed | UpdateState::NeedsPermission => Phase::Failed,
        UpdateState::Idle | UpdateState::Checking => return Vec::new(),
    };
    let size = update.asset_size.filter(|size| *size > 0.0);
    let error = match update.state {
        UpdateState::NeedsPermission => {
            Some("Allow Raygun to install apps, then try again".to_owned())
        }
        _ => update.message.clone(),
    };
    vec![Download {
        kind: Kind::Update,
        id: "app".into(),
        title: match &update.latest {
            Some(version) => format!("Raygun {version}"),
            None => "Raygun's update".into(),
        },
        subtitle: None,
        phase,
        done: match size {
            Some(size) => update.progress.unwrap_or(0.0) * size,
            None => update.progress.unwrap_or(0.0),
        },
        total: size,
        unit: if size.is_some() {
            Unit::Bytes
        } else {
            Unit::Fraction
        },
        bytes: None,
        left_ms: None,
        speed: None,
        error,
        abilities: Abilities {
            retry: phase == Phase::Failed,
            ..Abilities::default()
        },
        route: Kind::Update.route().to_owned(),
    }]
}

fn ytdlp(app: &AppHandle) -> Vec<Download> {
    let Ok(service) = state::<Arc<crate::ytdlp::YtDlpService>>(app) else {
        return Vec::new();
    };
    service
        .jobs()
        .into_iter()
        .filter_map(|job| {
            let phase = match job.state {
                YtDlpState::Queued => Phase::Queued,
                YtDlpState::Downloading => Phase::Running,
                YtDlpState::Done => Phase::Done,
                YtDlpState::Failed => Phase::Failed,
                YtDlpState::Cancelled => return None,
            };
            Some(Download {
                kind: Kind::YtDlp,
                id: job.id,
                title: job.title,
                subtitle: None,
                phase,
                done: job.progress.unwrap_or(0.0),
                total: Some(1.0),
                unit: Unit::Fraction,
                bytes: None,
                left_ms: None,
                speed: None,
                error: job.message,
                abilities: Abilities {
                    cancel: true,
                    ..Abilities::default()
                },
                route: Kind::YtDlp.route().to_owned(),
            })
        })
        .collect()
}

/// Files saved from websites.
fn files(app: &AppHandle) -> Vec<Download> {
    let Ok(service) = state::<Arc<crate::website_downloads::WebsiteDownloadService>>(app) else {
        return Vec::new();
    };
    service
        .list()
        .into_iter()
        .filter_map(|file| {
            let phase = match file.state {
                WebsiteDownloadState::Queued => Phase::Queued,
                WebsiteDownloadState::Downloading => Phase::Running,
                WebsiteDownloadState::Done => Phase::Done,
                WebsiteDownloadState::Failed => Phase::Failed,
                WebsiteDownloadState::Cancelled => return None,
            };
            Some(Download {
                kind: Kind::Files,
                id: file.id,
                title: file.name,
                subtitle: file.site,
                phase,
                done: file.done_bytes,
                total: file.total_bytes,
                unit: Unit::Bytes,
                bytes: None,
                left_ms: None,
                speed: file.bytes_per_second,
                error: file.error,
                abilities: Abilities {
                    cancel: true,
                    retry: file.state == WebsiteDownloadState::Failed,
                    ..Abilities::default()
                },
                route: Kind::Files.route().to_owned(),
            })
        })
        .collect()
}

#[cfg(target_os = "android")]
mod platform {
    use std::sync::Arc;

    use tauri::{AppHandle, Manager};

    use super::DownloadNotices;
    use super::model::{Finished, Kind, Plan, key};

    /// The plugin's buttons and taps come back here.
    pub fn install(app: &AppHandle) {
        let handle = app.clone();
        tauri_plugin_raygun_audio::set_notification_action_handler(move |json| {
            let Ok(value) = serde_json::from_str::<serde_json::Value>(&json) else {
                return;
            };
            let action = value["action"].as_str().unwrap_or_default().to_owned();
            let keys: Vec<String> = value["keys"]
                .as_array()
                .map(|keys| {
                    keys.iter()
                        .filter_map(|key| key.as_str().map(str::to_owned))
                        .collect()
                })
                .unwrap_or_default();
            let app = handle.clone();
            tauri::async_runtime::spawn(async move {
                if let Some(notices) = app.try_state::<Arc<DownloadNotices>>() {
                    notices.act(&action, &keys).await;
                }
            });
        });
        let handle = app.clone();
        tauri_plugin_raygun_audio::set_open_handler(move |route| {
            if let Some(notices) = handle.try_state::<Arc<DownloadNotices>>() {
                notices.open(route);
            }
        });
    }

    pub async fn show(app: &AppHandle, plan: &Plan, finished: Option<&Finished>) {
        let finished = finished.map(|notice| {
            let file = opens(app, notice);
            let mut value = serde_json::to_value(notice).unwrap_or_default();
            if let (serde_json::Value::Object(fields), Some(file)) = (&mut value, file) {
                fields.insert("open".into(), file);
            }
            value
        });
        let value = serde_json::json!({ "plan": plan, "finished": finished });
        let app = app.clone();
        let shown = tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .downloads_notify(&value)
        })
        .await;
        if let Ok(Err(error)) = shown {
            tracing::warn!(%error, "download notifications didn't show");
        }
    }

    /// The file a lone finished download from a website saved (`{uri, mime}`), for its notice's
    /// Open button, which opens it straight from the notification.
    fn opens(app: &AppHandle, finished: &Finished) -> Option<serde_json::Value> {
        let id = finished
            .lone
            .as_deref()?
            .strip_prefix(&key(Kind::Files, ""))?;
        let service = app.try_state::<Arc<crate::website_downloads::WebsiteDownloadService>>()?;
        let (uri, mime) = service.saved_file(id)?;
        Some(serde_json::json!({ "uri": uri, "mime": mime.unwrap_or_default() }))
    }

    pub async fn notice(app: &AppHandle, notice: &super::Notice) -> Result<(), String> {
        let value = notice.to_json();
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .notice(&value)
        })
        .await
        .map_err(|error| error.to_string())?
    }

    pub async fn cancel_notice(app: &AppHandle, id: i32) -> Result<(), String> {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || {
            app.state::<tauri_plugin_raygun_audio::NativeAudio<tauri::Wry>>()
                .notice_cancel(id)
        })
        .await
        .map_err(|error| error.to_string())?
    }
}

#[cfg(not(target_os = "android"))]
mod platform {
    use std::sync::Arc;

    use tauri::{AppHandle, Manager};
    use tauri_plugin_notification::NotificationExt;

    use super::DownloadNotices;
    use super::model::{Finished, Plan};

    /// A click on a notification that carries a route opens it, with Raygun's window in front.
    pub fn install(app: &AppHandle) {
        let handle = app.clone();
        let listening = app.notification().on_action(move |performed| {
            let Some(route) = super::tapped_route(performed) else {
                return;
            };
            if let Some(window) = handle.get_webview_window("main") {
                let _ = window.unminimize();
                let _ = window.show();
                let _ = window.set_focus();
            }
            if let Some(notices) = handle.try_state::<Arc<DownloadNotices>>() {
                notices.open(route);
            }
        });
        if let Err(error) = listening {
            tracing::debug!(%error, "clicks on notifications won't open anything");
        }
    }

    /// Only a batch's end, and only while Raygun's window isn't in front; silent (no sound named).
    pub async fn show(app: &AppHandle, _plan: &Plan, finished: Option<&Finished>) {
        let Some(finished) = finished else {
            return;
        };
        let focused = app
            .get_webview_window("main")
            .and_then(|window| window.is_focused().ok())
            .unwrap_or(false);
        if focused {
            return;
        }
        if let Err(error) = app
            .notification()
            .builder()
            .title(&finished.title)
            .body(&finished.text)
            .extra("route", &finished.route)
            .show()
        {
            tracing::debug!(%error, "couldn't tell that downloads ended");
        }
    }

    /// Without a sound named, Windows shows a toast silently: only channels that alert get one.
    pub async fn notice(app: &AppHandle, notice: &super::Notice) -> Result<(), String> {
        let mut builder = app
            .notification()
            .builder()
            .id(notice.id)
            .title(&notice.title)
            .body(&notice.text)
            .extra("route", &notice.route);
        if notice.channel.alerts {
            builder = builder.sound("Default");
        }
        builder.show().map_err(|error| error.to_string())
    }

    /// The system keeps desktop notifications in its own list, which the person clears.
    pub async fn cancel_notice(_app: &AppHandle, _id: i32) -> Result<(), String> {
        Ok(())
    }
}

// ---- Commands ----

/// The screen a tapped notification opens, if one was tapped before the UI listened.
#[tauri::command]
#[specta::specta]
pub fn notification_route_take(notices: tauri::State<'_, Arc<DownloadNotices>>) -> Option<String> {
    notices.take_route()
}
