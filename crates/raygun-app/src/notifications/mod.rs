//! Downloads in the system's notifications, in one place for every kind: songs, videos, chapters,
//! podcast episodes, exports, AI models, Raygun's own update and yt-dlp's jobs.
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
#[cfg(test)]
mod tests;

use std::collections::{HashMap, HashSet};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_model::{
    AiModelProgress, AiModelState, DownloadState, ExportState, PermissionReason, SongDownloadState,
    UpdateState, YtDlpState,
};
use tauri::{AppHandle, Manager};
use tauri_specta::Event;
use tokio::sync::Notify;

use crate::events::{
    AiModelEvent, AppUpdateEvent, ExportEvent, MangaDownloadsEvent, OpenRouteEvent,
    PodcastsChangedEvent, SongDownloadProgressEvent, SongDownloadsEvent, VideoDownloadsEvent,
    YtDlpEvent,
};
use model::{Abilities, Download, Kind, Phase, Plan, Tracker, Unit};

/// Notifications change at most this often (Android drops updates that come faster).
const TICK: Duration = Duration::from_secs(1);

/// A podcast download that hasn't moved for this long isn't running (the app stopped under it).
const PODCAST_STALE: Duration = Duration::from_secs(120);

const ALL_KINDS: [Kind; 8] = [
    Kind::Songs,
    Kind::Videos,
    Kind::Chapters,
    Kind::Podcasts,
    Kind::Exports,
    Kind::Models,
    Kind::Update,
    Kind::YtDlp,
];

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
    /// The screen a tapped notification opens, until the UI takes it.
    route: Mutex<Option<String>>,
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
            route: Mutex::new(None),
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
            notices
                .songs
                .lock()
                .expect("songs poisoned")
                .insert(song.id.clone(), song);
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
        AiModelEvent::listen(app, move |event| {
            let progress = event.payload.0;
            notices
                .models
                .lock()
                .expect("models poisoned")
                .insert(progress.id.clone(), progress);
            notices.touch(Kind::Models);
        });
    }

    /// A kind's list changed: read it at the next turn.
    fn touch(&self, kind: Kind) {
        self.dirty.lock().expect("dirty poisoned").insert(kind);
        self.wake.notify_one();
    }

    /// A tapped notification's screen.
    #[cfg_attr(not(target_os = "android"), allow(dead_code))]
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

    async fn run(self: Arc<Self>) {
        let mut tracker = Tracker::default();
        let mut lists: HashMap<Kind, Vec<Download>> = HashMap::new();
        let mut memory = Memory::default();
        let mut shown = Plan::default();
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
                if list
                    .iter()
                    .any(|download| matches!(download.phase, Phase::Running | Phase::Recording))
                {
                    kinds.insert(*kind);
                }
            }
            for kind in kinds {
                let list = self.collect(kind, &tracker, &mut memory).await;
                lists.insert(kind, list);
            }
            let downloads: Vec<Download> = lists.values().flatten().cloned().collect();
            let update = tracker.update(crate::catalog::now_ms() as f64, &downloads);
            if update.plan != shown || update.finished.is_some() {
                platform::show(&self.app, &update.plan, update.finished.as_ref()).await;
                shown = update.plan;
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
        };
        downloads
            .into_iter()
            .filter(|download| {
                !matches!(download.phase, Phase::Done | Phase::Failed)
                    || tracker.watching(&download.key())
            })
            .collect()
    }

    /// The songs, from the last list and the progress since; the list is read again only when
    /// it changed, not for progress.
    async fn songs_now(&self) -> Vec<Download> {
        if self
            .songs_changed
            .swap(false, std::sync::atomic::Ordering::SeqCst)
            && let Ok(service) =
                state::<Arc<crate::music_downloads::SongDownloadService>>(&self.app)
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
        songs(&self.app, list)
    }

    fn models_now(&self) -> Vec<Download> {
        let names: HashMap<String, String> = self
            .app
            .try_state::<crate::ai::AiService>()
            .map(|ai| {
                ai.models()
                    .into_iter()
                    .map(|model| (model.id, model.name))
                    .collect()
            })
            .unwrap_or_default();
        let mut models = self.models.lock().expect("models poisoned");
        // Finished ones are only kept while their batch is reported.
        let list: Vec<Download> = models
            .values()
            .map(|progress| Download {
                kind: Kind::Models,
                id: progress.id.clone(),
                title: names
                    .get(&progress.id)
                    .cloned()
                    .unwrap_or_else(|| progress.id.clone()),
                subtitle: None,
                phase: match progress.state {
                    AiModelState::Downloading => Phase::Running,
                    AiModelState::Installed => Phase::Done,
                    AiModelState::Failed => Phase::Failed,
                },
                done: progress.received_bytes,
                total: Some(progress.total_bytes).filter(|total| *total > 0.0),
                unit: Unit::Bytes,
                bytes: None,
                left_ms: None,
                speed: None,
                error: progress.error.clone(),
                abilities: Abilities {
                    retry: progress.state == AiModelState::Failed,
                    ..Abilities::default()
                },
                route: Kind::Models.route().to_owned(),
            })
            .collect();
        models.retain(|_, progress| progress.state == AiModelState::Downloading);
        list
    }

    /// A notification's button: Pause, Resume, Cancel, Retry or Stop, for `keys`.
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
            // Pausing and resuming act on a whole queue: once is enough.
            if matches!(action, "pause" | "resume") && !queues_done.insert(kind) {
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
        (Kind::Songs, "cancel") => state::<Arc<SongDownloadService>>(app)?.remove(id).await,
        (Kind::Songs, "retry") => state::<Arc<SongDownloadService>>(app)?.retry(id).await,
        (Kind::Songs, "stop") => {
            state::<Arc<SongDownloadService>>(app)?.cancel_recording();
            Ok(())
        }
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
        (Kind::Podcasts, "retry") => {
            state::<Arc<crate::podcasts::PodcastService>>(app)?
                .download(id)
                .await
        }
        (Kind::Exports, "cancel") => {
            state::<Arc<crate::export::ExportService>>(app)?.cancel(id);
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
    /// Podcast downloads by episode: their bytes, and when those last changed (`None`: not since
    /// the first read, which finds the ones the last run left behind).
    podcast_moves: HashMap<String, (i64, Option<Instant>)>,
    podcasts_read: bool,
    /// Episode titles, their podcast's and its id, by episode.
    episodes: HashMap<String, (String, String, String)>,
}

fn songs(app: &AppHandle, list: Vec<raygun_model::SongDownload>) -> Vec<Download> {
    // A recording lasts as long as its song plays.
    let left = app
        .try_state::<crate::playback::PlaybackService>()
        .and_then(|playback| {
            let state = playback.state();
            state
                .duration_ms
                .map(|duration| (duration - state.position_ms).max(0.0))
        });
    list.into_iter()
        .map(|song| {
            let phase = match song.state {
                SongDownloadState::Queued => Phase::Queued,
                SongDownloadState::Downloading => Phase::Running,
                SongDownloadState::Recording => Phase::Recording,
                SongDownloadState::Done => Phase::Done,
                SongDownloadState::Failed | SongDownloadState::NeedsRecording => Phase::Failed,
            };
            let error = song.error.clone().or_else(|| {
                (song.state == SongDownloadState::NeedsRecording)
                    .then(|| "Only YouTube has this song".to_owned())
            });
            Download {
                kind: Kind::Songs,
                title: song.title,
                subtitle: song.artist,
                phase,
                done: song.done_bytes,
                total: song.total_bytes,
                unit: Unit::Bytes,
                bytes: None,
                left_ms: if phase == Phase::Recording {
                    left
                } else {
                    song.eta_ms
                },
                speed: song.bytes_per_second,
                error,
                abilities: Abilities {
                    pause: false,
                    cancel: true,
                    retry: song.state == SongDownloadState::Failed,
                },
                route: Kind::Songs.route().to_owned(),
                id: song.id,
            }
        })
        .collect()
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
    let Ok(rows) = raygun_db::podcasts::downloads(&db).await else {
        return Vec::new();
    };
    let first = !std::mem::replace(&mut memory.podcasts_read, true);
    let mut list = Vec::new();
    for row in rows {
        if row.state == "downloading" {
            // The podcast service doesn't say which downloads run: one that moves does, and
            // one the last run left behind ("downloading" at the first read) waits until it moves.
            let bytes = row.received_bytes;
            let moved = match memory.podcast_moves.get(&row.episode_id) {
                None => (!first).then(Instant::now),
                Some((before, at)) if *before == bytes => *at,
                Some(_) => Some(Instant::now()),
            };
            memory
                .podcast_moves
                .insert(row.episode_id.clone(), (bytes, moved));
            if !moved.is_some_and(|at| at.elapsed() < PODCAST_STALE) {
                continue;
            }
        } else {
            memory.podcast_moves.remove(&row.episode_id);
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
                        _ => "Writing",
                    }
                    .to_owned()
                }),
                phase,
                done: f64::from(job.downloaded + job.written),
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

#[cfg(target_os = "android")]
mod platform {
    use std::sync::Arc;

    use tauri::{AppHandle, Manager};

    use super::DownloadNotices;
    use super::model::{Finished, Plan};

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
}

#[cfg(not(target_os = "android"))]
mod platform {
    use tauri::{AppHandle, Manager};
    use tauri_plugin_notification::NotificationExt;

    use super::model::{Finished, Plan};

    pub fn install(_app: &AppHandle) {}

    /// Only a batch's end, and only while Raygun's window isn't in front.
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
            .show()
        {
            tracing::debug!(%error, "couldn't tell that downloads ended");
        }
    }
}

// ---- Commands ----

/// The screen a tapped notification opens, if one was tapped before the UI listened.
#[tauri::command]
#[specta::specta]
pub fn notification_route_take(notices: tauri::State<'_, Arc<DownloadNotices>>) -> Option<String> {
    notices.take_route()
}
