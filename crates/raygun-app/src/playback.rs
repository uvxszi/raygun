//! Runs the playback state machine and carries out its effects: finding streams for items that
//! need one (M14), serving sources through the stream proxy, commanding the right engine for each
//! item, broadcasting state to every client and counting plays for history (M18). A song whose
//! stream stops working isn't skipped at once: it looks for another source as `resolution`
//! plans, and the player says so meanwhile.

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use raygun_model::{
    ConnectCommand, EngineCommand, EngineEvent, EngineReport, MediaSource, PlayContext,
    PlaybackControl, PlaybackState, QueueItem, QueueItemInput, QueueSnapshot, SleepTimer,
    SoundPrefs, TrackInput, TrackQuery,
};
use raygun_net::StreamProxy;
use raygun_playback::{Effect, PlaybackCore};
use raygun_resolver::Pick;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::collections::CollectionsService;
use crate::events::{EngineCommandEvent, PlaybackStateEvent, QueueEvent};
use crate::history::PlayCounter;
use crate::resolution::{Cause, ErrorPlan, Failed, LoadPlan, Notes, Tracker, Unresolved};
use crate::resolver::ResolverService;
use crate::sound::{SoundService, Tuning};

/// How long a context said before a play waits for the queue it's about: plays that look up
/// songs first (an audiobook's feed) take a while, a play that failed never comes.
const ARMED_FOR: Duration = Duration::from_secs(60);

/// The notification saying a song in YouTube's player paused until Raygun is open.
const NEEDS_WINDOW_NOTICE: &str = "playback.needsWindow";

/// Delivers one command to a native engine.
pub type NativeEngineSink = Box<dyn Fn(&EngineCommand) -> Result<(), String> + Send + Sync>;

/// Where queue changes and controls go while this device plays on another one (M46.15).
pub type Redirect = Arc<dyn Fn(ConnectCommand) + Send + Sync>;

/// Which engine plays an item.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Engine {
    /// The webview: `<audio>` for streams and files, YouTube's embedded player for YouTube.
    Webview,
    /// A native player (Android's Media3 engine, M06).
    Native,
}

/// YouTube can only play in its embedded player, which lives in the webview. Everything else
/// prefers the native engine where there is one.
pub fn engine_for(source: &MediaSource, native_available: bool) -> Engine {
    match source {
        MediaSource::YouTube { .. } => Engine::Webview,
        _ if native_available => Engine::Native,
        _ => Engine::Webview,
    }
}

struct Inner {
    core: PlaybackCore,
    /// Engine holding the current item; play/pause/seek go here.
    active: Option<Engine>,
    tracker: Tracker,
    plays: PlayCounter,
    /// The equalizer, effects, normalization and crossfade (M46.12).
    sound: SoundPrefs,
    /// A crossfade waiting for its stream to be found: the load and how long it fades.
    pending_fade: Option<(u32, f64)>,
}

pub struct PlaybackService {
    inner: Mutex<Inner>,
    proxy: StreamProxy,
    app: AppHandle,
    native: Option<NativeEngineSink>,
    /// Bumped by every sleep timer setting; a waiting timer only fires if it's still the latest.
    sleep_generation: Arc<AtomicU64>,
    /// Set while playing on another device (M46.15): what would change this player goes there.
    redirect: Mutex<Option<Redirect>>,
    /// What was last saved for reopening (M46.16), to skip writing it again unchanged.
    saved: Mutex<Option<String>>,
    /// The saved queue was put back (or there was none): only once per run.
    restored: std::sync::atomic::AtomicBool,
    /// The state the webviews last got, and when: see [`worth_sending`].
    sent: Mutex<Option<(PlaybackState, Instant)>>,
    /// Where the next queue is played from, said by the page just before it plays
    /// ([`Self::arm_context`]); the next replace takes it.
    armed: Mutex<Option<(PlayContext, Instant)>>,
    /// The clock looking for another source runs on.
    clock: Instant,
    /// Raygun's webview is there, with YouTube's embedded player in it. On Android it goes with
    /// Raygun's activity while music plays on without it ([`Self::webview_gone`]).
    webview: std::sync::atomic::AtomicBool,
}

/// Everything to emit once the core lock is released.
#[derive(Default)]
struct Outbox {
    engine: Vec<(Engine, EngineCommand)>,
    state: Option<PlaybackState>,
    /// Someone asked for this change (a control, a new queue): its state goes out at once, even
    /// when only the position moved (a seek while paused).
    asked: bool,
    queue: Option<QueueSnapshot>,
    /// Find a stream for this load, after this long.
    resolve: Option<(u32, Duration)>,
    verified: Option<Pick>,
    failed: Option<(Pick, String)>,
    /// A remembered pick whose stream stopped working: its stream is forgotten.
    stale: Option<Pick>,
    /// The song's own stream stopped working: picks remembered with it are forgotten.
    dead: Option<(QueueItem, MediaSource)>,
    /// The song's downloaded copy didn't play: its download is checked.
    broken_download: Option<QueueItem>,
    /// The song plays again after looking for another source, from that source: its marks go,
    /// its saved copies leave the stream that stopped working, its download is checked.
    rescued: Option<(QueueItem, Option<MediaSource>)>,
    /// Nothing played the song: it's skipped, and marked as one no source has when that's known.
    gave_up: Option<(QueueItem, bool)>,
    /// The current item just counted as a play.
    played: Option<QueueItem>,
    /// The current item when `state` changed, for followers like podcasts (M37).
    current: Option<QueueItem>,
    /// This load plays in YouTube's player, and Raygun's webview is gone (Android): it pauses.
    needs_webview: Option<(u32, QueueItem)>,
}

impl Outbox {
    /// What failed for a load, for the stream cache and the downloads to learn.
    fn note(&mut self, failed: Failed, item: Option<QueueItem>, message: String) {
        match failed {
            Failed::Pick(pick) => self.failed = Some((*pick, message)),
            Failed::Stale(pick) => self.stale = Some(*pick),
            Failed::Own(source) => self.dead = item.map(|item| (item, source)),
            Failed::Download => self.broken_download = item,
        }
    }
}

impl PlaybackService {
    pub fn new(app: AppHandle, proxy: StreamProxy, native: Option<NativeEngineSink>) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as u64)
            .unwrap_or(1);
        Self {
            inner: Mutex::new(Inner {
                core: PlaybackCore::new(seed),
                active: None,
                tracker: Tracker::default(),
                plays: PlayCounter::default(),
                sound: SoundPrefs::default(),
                pending_fade: None,
            }),
            proxy,
            app,
            native,
            sleep_generation: Arc::new(AtomicU64::new(0)),
            redirect: Mutex::new(None),
            saved: Mutex::new(None),
            restored: std::sync::atomic::AtomicBool::new(false),
            sent: Mutex::new(None),
            armed: Mutex::new(None),
            clock: Instant::now(),
            webview: std::sync::atomic::AtomicBool::new(true),
        }
    }

    /// Now on the clock looking for another source runs on, ms.
    fn now_ms(&self) -> f64 {
        self.clock.elapsed().as_secs_f64() * 1_000.0
    }

    /// Says where the next queue is played from (a page's `PlayContext`), or that it's from
    /// nowhere in particular (`None`). The next replace of the queue takes it as the queue's
    /// context; a replace nothing was said for clears it.
    pub fn arm_context(&self, context: Option<PlayContext>) {
        *self.armed.lock().expect("armed poisoned") =
            context.map(|context| (context, Instant::now()));
    }

    fn take_context(&self) -> Option<PlayContext> {
        self.armed
            .lock()
            .expect("armed poisoned")
            .take()
            .filter(|(_, at)| at.elapsed() < ARMED_FOR)
            .map(|(context, _)| context)
    }

    /// Saves the queue and where it is every few seconds (M46.16), so Raygun reopens on it.
    pub fn start_saving(app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            loop {
                // Slower while idle in the background; at once when Raygun leaves the screen.
                crate::device_state::rest(SAVE_EVERY).await;
                let playback = app.state::<PlaybackService>();
                // Nothing is saved before the last queue is back, or it would be overwritten.
                if !playback.restored.load(Ordering::SeqCst) {
                    continue;
                }
                let images = app.state::<raygun_net::ImageLinks>();
                let json = {
                    let inner = playback.lock();
                    crate::last_played::snapshot(&inner.core, &|url| images.original(url))
                };
                let changed = {
                    let mut saved = playback.saved.lock().expect("saved poisoned");
                    let changed = saved.as_deref() != Some(json.as_str());
                    if changed {
                        *saved = Some(json.clone());
                    }
                    changed
                };
                // Incognito (M47): nothing is kept for reopening, and the queue it leaves stays
                // unsaved after it ends until that queue changes.
                if changed && !crate::privacy::incognito(&app) {
                    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
                    crate::last_played::save(&db, &json).await;
                }
            }
        });
    }

    /// Checks every second that what the state says plays does (`PlaybackCore::watchdog`): an
    /// engine that stands still while "playing", or has the stream and never starts, is nudged,
    /// and the state stops claiming it plays.
    pub fn start_watchdog(app: AppHandle) {
        tauri::async_runtime::spawn(async move {
            let clock = Instant::now();
            let mut every = tokio::time::interval(WATCH_EVERY);
            every.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
            loop {
                // Nothing to watch while nothing plays or loads: no wake-ups meanwhile.
                crate::device_state::wait_until_busy().await;
                every.tick().await;
                let playback = app.state::<PlaybackService>();
                let outbox = {
                    let mut inner = playback.lock();
                    // The native engine watches itself: the plugin's watchdog sees the player's
                    // own position and audio focus. This one covers the webview's engines.
                    if inner.active == Some(Engine::Native) {
                        continue;
                    }
                    let before = inner.core.state().status;
                    let effects = inner.core.watchdog(clock.elapsed().as_secs_f64() * 1_000.0);
                    if effects.is_empty() {
                        continue;
                    }
                    let after = inner.core.state().status;
                    tracing::warn!(
                        load_id = inner.core.load_id(),
                        ?before,
                        ?after,
                        "the engine doesn't play what the state says plays: nudged it",
                    );
                    playback.resolve(&mut inner, effects, Cause::Advance)
                };
                playback.send(outbox);
            }
        });
    }

    /// Puts back what played when Raygun closed, paused where it was (M46.16). Once per run,
    /// when the player is ready (the webview's engines listen by then), and only into an empty
    /// queue: something played since is kept.
    pub async fn restore_last(&self, app: &AppHandle) {
        if self.restored.swap(true, Ordering::SeqCst) {
            // A later window's page asks (Android: Raygun's activity came back while the music
            // played on): its engines listen now.
            self.webview_back();
            return;
        }
        let db = app.state::<crate::startup::AppDb>().inner().0.clone();
        let on = crate::settings::load(&db)
            .await
            .map(|settings| settings.resume_on_start)
            .unwrap_or(true);
        let Some(last) = crate::last_played::load(&db).await.filter(|_| on) else {
            return;
        };
        let images = app.state::<raygun_net::ImageLinks>();
        let items = crate::last_played::arrived(last.items, &images);
        let context = last.context.map(|context| PlayContext {
            artwork_url: context
                .artwork_url
                .map(|url| images.cover(&url, &Default::default())),
            ..context
        });
        let effects_ran = {
            let mut inner = self.lock();
            if !inner.core.queue().items.is_empty() {
                return;
            }
            let mut effects = inner.core.restore(
                items,
                last.order,
                last.current,
                last.position_ms,
                last.shuffle,
                last.repeat,
            );
            effects.extend(inner.core.set_context(context));
            self.resolve(&mut inner, effects, Cause::Chosen)
        };
        self.send(effects_ran);
    }

    /// Whether anything is queued: Raygun carries on without its window then (Android).
    pub fn holds_queue(&self) -> bool {
        let inner = self.lock();
        inner.core.current_item().is_some() || !inner.core.queue().items.is_empty()
    }

    /// Raygun's webview went (Android: its activity, while the music plays on). A song playing in
    /// YouTube's embedded player there moves to a stream the native player plays, from where it
    /// was, when a source has one; else it pauses, with a notification saying why.
    pub fn webview_gone(&self) {
        self.webview.store(false, Ordering::SeqCst);
        self.move_off_webview();
    }

    /// Whether Raygun's webview is there (Android: it goes with Raygun's activity).
    pub fn webview_present(&self) -> bool {
        self.webview.load(Ordering::SeqCst)
    }

    /// A webview is back and its engines listen: a song left in YouTube's player loads there
    /// again, as it was (paused).
    pub fn webview_back(&self) {
        if self.webview.swap(true, Ordering::SeqCst) {
            return;
        }
        // What the notification asked (open Raygun) is done.
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let id = crate::notifications::notice_id(NEEDS_WINDOW_NOTICE);
            crate::notifications::cancel_notice(&app, id).await;
        });
        let mut outbox = Outbox::default();
        {
            let mut inner = self.lock();
            if inner.active != Some(Engine::Webview) {
                return;
            }
            let Some(item) = inner.core.current_item().cloned() else {
                return;
            };
            let load_id = inner.core.load_id();
            let source = match inner.tracker.playing(load_id) {
                Some(source) => source.clone(),
                None if matches!(item.source, MediaSource::YouTube { .. }) => item.source.clone(),
                None => return,
            };
            let position_ms = inner.core.state().position_ms;
            let autoplay = inner.core.plays_when_found();
            self.load_into(
                &mut inner,
                &mut outbox,
                load_id,
                &item,
                &source,
                position_ms,
                autoplay,
                0.0,
            );
        }
        self.send(outbox);
    }

    /// The song playing in the webview, now without one: another stream for it, or a pause.
    fn move_off_webview(&self) {
        let (load_id, item, request, outbox) = {
            let mut inner = self.lock();
            let playing = matches!(
                inner.core.state().status,
                raygun_model::PlaybackStatus::Playing
                    | raygun_model::PlaybackStatus::Loading
                    | raygun_model::PlaybackStatus::Buffering
            );
            // A paused one waits for the webview, where it loads again.
            if inner.active != Some(Engine::Webview) || !playing {
                return;
            }
            let Some(item) = inner.core.current_item().cloned() else {
                return;
            };
            let load_id = inner.core.load_id();
            let request = inner
                .tracker
                .without_webview(load_id, crate::resolver::YOUTUBE_STREAMS);
            let effects = inner.core.set_looking(load_id, true);
            let outbox = self.resolve(&mut inner, effects, Cause::Advance);
            (load_id, item, request, outbox)
        };
        self.send(outbox);
        tracing::info!(load_id, title = %item.title, "the webview went: looking for a stream the native player plays");
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let found = match request {
                Some((query, exclusions)) => {
                    let resolver = app.state::<Arc<ResolverService>>().inner().clone();
                    Some(resolver.find(&query, &exclusions).await)
                }
                None => None,
            };
            let playback = app.state::<PlaybackService>();
            let native = playback.native.is_some();
            match found {
                Some((Ok(pick), notes)) if engine_for(&pick.source, native) == Engine::Native => {
                    playback.on_resolved(load_id, Ok(pick), notes, false);
                }
                _ => playback.hold_for_webview(load_id, &item).await,
            }
        });
    }

    /// The song `load_id` can only play in YouTube's player, which needs Raygun's window: it
    /// pauses where it is, and a notification says why.
    async fn hold_for_webview(&self, load_id: u32, item: &QueueItem) {
        let outbox = {
            let mut inner = self.lock();
            if inner.core.load_id() != load_id {
                return;
            }
            let mut effects = inner.core.set_looking(load_id, false);
            effects.extend(inner.core.pause());
            self.resolve(&mut inner, effects, Cause::Chosen)
        };
        self.send(outbox);
        tracing::info!(load_id, title = %item.title, "paused: YouTube's player needs Raygun's window");
        crate::notifications::show_notice(
            &self.app,
            crate::notifications::Notice {
                id: crate::notifications::notice_id(NEEDS_WINDOW_NOTICE),
                channel: &crate::notifications::PLAYBACK,
                title: format!("{} is paused", item.title),
                text:
                    "It plays in YouTube's player, which needs Raygun open. Open Raygun to go on."
                        .to_owned(),
                route: "/".to_owned(),
            },
        )
        .await;
    }

    /// Sends what would change this player to another device instead, or (`None`) back here.
    pub fn set_redirect(&self, redirect: Option<Redirect>) {
        *self.redirect.lock().expect("redirect poisoned") = redirect;
    }

    fn redirect(&self) -> Option<Redirect> {
        self.redirect.lock().expect("redirect poisoned").clone()
    }

    /// Runs what another device sent through Connect (M46.15). It plays here even if this device
    /// itself plays elsewhere.
    pub fn apply(&self, command: ConnectCommand) {
        match command {
            // A whole player moved here (Connect's transfer): its order and repeat as they were,
            // loaded paused, then started, so it starts once at the right place.
            ConnectCommand::Replace {
                items,
                start_index,
                shuffle,
                start_ms,
                play,
                context,
                order: Some(order),
                repeat,
            } => {
                self.run(|core| {
                    let repeat = repeat.unwrap_or(core.state().repeat);
                    let mut effects = core.restore(
                        items,
                        order.into_iter().map(|index| index as usize).collect(),
                        start_index.unwrap_or(0) as usize,
                        start_ms,
                        shuffle,
                        repeat,
                    );
                    effects.extend(core.set_context(context));
                    effects
                });
                if play {
                    self.control_here(PlaybackControl::Play);
                }
            }
            ConnectCommand::Replace {
                items,
                start_index,
                shuffle,
                start_ms,
                play,
                context,
                order: None,
                repeat,
            } => {
                self.run(|core| {
                    let mut effects = core.replace_at(
                        items,
                        start_index.map(|index| index as usize),
                        shuffle,
                        start_ms,
                    );
                    effects.extend(core.set_context(context));
                    if let Some(mode) = repeat {
                        effects.extend(core.set_repeat(mode));
                    }
                    if !play {
                        effects.extend(core.pause());
                    }
                    effects
                });
            }
            ConnectCommand::Enqueue { items, play_now } => {
                self.run(|core| core.enqueue(items, play_now));
            }
            ConnectCommand::EnqueueNext { items } => self.run(|core| core.enqueue_next(items)),
            ConnectCommand::Control { control } => self.control_here(control),
        }
    }

    pub fn state(&self) -> PlaybackState {
        self.lock().core.state()
    }

    /// A playable address for `source` through the stream proxy (Local Files' videos, M46.27).
    /// Queue items with the user's own details (M46.30).
    fn with_details(&self, mut items: Vec<QueueItemInput>) -> Vec<QueueItemInput> {
        if let Some(details) = self.app.try_state::<Arc<crate::details::DetailService>>() {
            details.apply_queue(&mut items);
        }
        items
    }

    pub fn register_source(&self, source: &MediaSource) -> String {
        self.proxy.register(source)
    }

    /// The YouTube embed host page the player uses (a recording captures songs from hidden ones of
    /// its own, M46.52).
    pub fn youtube_embed_url(&self) -> String {
        self.proxy.youtube_embed_url()
    }

    /// Changes queued items in place (M46.48: a song given a source or names); `update` says
    /// whether it changed one. The playing item keeps its stream until it loads again.
    pub fn update_items(&self, update: impl FnMut(&mut QueueItem) -> bool) {
        self.run(|core| core.update_items(update));
    }

    pub fn queue(&self) -> QueueSnapshot {
        self.lock().core.queue()
    }

    pub fn enqueue(&self, items: Vec<QueueItemInput>, play_now: bool) {
        let items = self.with_details(items);
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Enqueue { items, play_now });
        }
        self.run(|core| core.enqueue(items, play_now));
    }

    /// Replaces the queue and starts playing (see `PlaybackCore::replace`).
    pub fn replace(&self, items: Vec<QueueItemInput>, start_index: Option<usize>, shuffle: bool) {
        let items = self.with_details(items);
        let context = self.take_context();
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Replace {
                items,
                start_index: start_index.map(|index| index as u32),
                shuffle,
                start_ms: 0.0,
                play: true,
                context,
                order: None,
                repeat: None,
            });
        }
        self.run(|core| {
            let mut effects = core.replace(items, start_index, shuffle);
            effects.extend(core.set_context(context));
            effects
        });
    }

    /// Replaces the queue and starts its first item at `start_ms` (a podcast episode resuming).
    pub fn replace_from(&self, items: Vec<QueueItemInput>, start_ms: f64) {
        let items = self.with_details(items);
        let context = self.take_context();
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Replace {
                items,
                start_index: Some(0),
                shuffle: false,
                start_ms,
                play: true,
                context,
                order: None,
                repeat: None,
            });
        }
        self.run(|core| {
            let mut effects = core.replace_at(items, Some(0), false, start_ms);
            effects.extend(core.set_context(context));
            effects
        });
    }

    pub fn control(&self, control: PlaybackControl) {
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Control { control });
        }
        self.control_here(control);
    }

    /// Controls this device's own player, even while it plays elsewhere.
    pub fn control_here(&self, control: PlaybackControl) {
        let resumes = matches!(control, PlaybackControl::Play | PlaybackControl::Toggle);
        if resumes && !self.webview.load(Ordering::SeqCst) {
            // Play from the lock screen for a song left in YouTube's player, with Raygun's window
            // gone: a stream the native player plays, if a source has one now.
            let in_embed = {
                let inner = self.lock();
                inner.active == Some(Engine::Webview)
                    && inner.core.state().status == raygun_model::PlaybackStatus::Paused
            };
            if in_embed {
                self.control_core(control);
                self.move_off_webview();
                return;
            }
        }
        self.control_core(control);
        if resumes {
            // Pressing play must start something (M46.16), or the next stream gets its turn.
            let (load_id, loading) = {
                let inner = self.lock();
                let state = inner.core.state();
                (
                    inner.core.load_id(),
                    state.status == raygun_model::PlaybackStatus::Loading && !inner.core.held(),
                )
            };
            if loading {
                self.watch_start(load_id);
            }
        }
    }

    fn control_core(&self, control: PlaybackControl) {
        self.run(|core| match control {
            PlaybackControl::Play => core.play(),
            PlaybackControl::Pause => core.pause(),
            PlaybackControl::Toggle => core.toggle(),
            PlaybackControl::Next => core.next(),
            PlaybackControl::Previous => core.previous(),
            PlaybackControl::Seek { position_ms } => core.seek(position_ms),
            PlaybackControl::SetVolume { volume } => core.set_volume(volume),
            PlaybackControl::SetRepeat { mode } => core.set_repeat(mode),
            PlaybackControl::SetShuffle { enabled } => core.set_shuffle(enabled),
            PlaybackControl::PlayItem { item_id } => core.play_item(&item_id),
            PlaybackControl::Remove { item_id } => core.remove(&item_id),
            PlaybackControl::Clear => core.clear(),
            PlaybackControl::ClearUpcoming => core.clear_upcoming(),
            PlaybackControl::Move {
                item_id,
                before_item_id,
            } => core.move_item(&item_id, before_item_id.as_deref()),
            PlaybackControl::SetRate { rate } => core.set_rate(rate),
            PlaybackControl::SetSleepTimer { timer } => self.sleep_timer(core, timer),
        });
    }

    /// Queues items right after the current one (Play next, M46.8).
    pub fn enqueue_next(&self, items: Vec<QueueItemInput>) {
        let items = self.with_details(items);
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::EnqueueNext { items });
        }
        self.run(|core| core.enqueue_next(items));
    }

    /// New sound settings (M46.12): the crossfade for the core, the effects for the engine.
    pub fn set_sound(&self, sound: SoundPrefs) {
        let mut outbox = Outbox::default();
        {
            let mut inner = self.lock();
            inner.core.set_crossfade(f64::from(sound.crossfade_ms));
            inner.sound = sound.clone();
            if let Some(engine) = inner.active {
                outbox
                    .engine
                    .push((engine, EngineCommand::SetEffects { sound }));
            }
        }
        self.send(outbox);
    }

    /// A YouTube song's video shown or hidden (M46.51). YouTube songs play as their audio file,
    /// and in YouTube's embed while the video shows: showing it moves the song playing now there,
    /// from where it is. Hiding it leaves the song where it plays; the next one plays its file.
    pub fn set_video(&self, shown: bool) {
        let mut outbox = Outbox::default();
        {
            let mut inner = self.lock();
            if !inner.tracker.set_video(shown) || !shown {
                return;
            }
            let Some(item) = inner.core.current_item().cloned() else {
                return;
            };
            let load_id = inner.core.load_id();
            let in_embed = matches!(
                inner.tracker.playing(load_id),
                Some(MediaSource::YouTube { .. })
            );
            if !matches!(item.source, MediaSource::YouTube { .. }) || in_embed {
                return;
            }
            let position_ms = inner.core.state().position_ms;
            // It plays on if it played (a paused song stays paused there).
            let autoplay = inner.core.plays_when_found();
            if inner
                .tracker
                .begin(load_id, &item, position_ms, Cause::Chosen)
                == LoadPlan::Direct
            {
                let source = item.source.clone();
                self.load_into(
                    &mut inner,
                    &mut outbox,
                    load_id,
                    &item,
                    &source,
                    position_ms,
                    autoplay,
                    0.0,
                );
            }
        }
        self.send(outbox);
    }

    /// A song's edit changed (M46.12): if it's the one playing, its gain and end change now.
    pub fn song_edited(&self, key: &str) {
        let mut outbox = Outbox::default();
        {
            let mut inner = self.lock();
            let Some(item) = inner.core.current_item().cloned() else {
                return;
            };
            if crate::sound::key(&TrackQuery::for_item(&item)) != key {
                return;
            }
            let tuning = self.tuning(&inner.sound, &item);
            let load_id = inner.core.load_id();
            inner.core.set_end(load_id, tuning.end_ms);
            if let Some(engine) = inner.active {
                outbox.engine.push((
                    engine,
                    EngineCommand::SetTrackGain {
                        gain_db: tuning.gain_db,
                    },
                ));
                outbox.engine.push((
                    engine,
                    EngineCommand::SetEnd {
                        end_ms: tuning.end_ms,
                    },
                ));
            }
        }
        self.send(outbox);
    }

    /// How `item` plays: its trims and gain (M46.12).
    fn tuning(&self, sound: &SoundPrefs, item: &QueueItem) -> Tuning {
        let key = crate::sound::key(&TrackQuery::for_item(item));
        match self.app.try_state::<Arc<SoundService>>() {
            Some(service) => service.tuning(&key, sound),
            None => Tuning {
                start_ms: 0.0,
                end_ms: None,
                gain_db: 0.0,
                measure: false,
            },
        }
    }

    /// Sets the sleep timer (M46.8). A time starts a task that pauses then, unless the timer was
    /// changed meanwhile (each setting has its own generation).
    fn sleep_timer(&self, core: &mut PlaybackCore, timer: SleepTimer) -> Vec<Effect> {
        let generation = self.sleep_generation.fetch_add(1, Ordering::SeqCst) + 1;
        match timer {
            SleepTimer::Off => core.set_sleep(None, false),
            SleepTimer::EndOfTrack => core.set_sleep(None, true),
            SleepTimer::Minutes { minutes } => {
                let minutes = minutes.clamp(1, 24 * 60);
                let wait = Duration::from_secs(u64::from(minutes) * 60);
                let at = crate::catalog::now_ms() as f64 + wait.as_millis() as f64;
                let app = self.app.clone();
                let generations = self.sleep_generation.clone();
                tauri::async_runtime::spawn(async move {
                    tokio::time::sleep(wait).await;
                    if generations.load(Ordering::SeqCst) == generation {
                        app.state::<PlaybackService>().run(PlaybackCore::sleep_now);
                    }
                });
                core.set_sleep(Some(at), false)
            }
        }
    }

    pub fn report(&self, mut report: EngineReport) {
        let outbox = {
            let mut inner = self.lock();
            let current = report.load_id == inner.core.load_id();
            // The song after one that couldn't play is given less time to look (a run of them).
            let cause = if matches!(report.event, EngineEvent::Error { .. }) {
                Cause::AfterFailure
            } else {
                Cause::Advance
            };
            let mut verified = None;
            let mut rescued = None;
            let mut gave_up = None;
            let mut lost = None;
            let counted = current
                && match &report.event {
                    EngineEvent::Progress {
                        position_ms,
                        duration_ms,
                    } => inner
                        .plays
                        .progress(report.load_id, *position_ms, *duration_ms),
                    EngineEvent::Ended => inner.plays.ended(report.load_id),
                    _ => false,
                };
            let played = counted
                .then(|| inner.core.current_item().cloned())
                .flatten();
            match &report.event {
                EngineEvent::Error { message } => {
                    tracing::info!(load_id = report.load_id, %message, "engine reported an error");
                    let plan = if current {
                        inner.tracker.engine_error(report.load_id, self.now_ms())
                    } else {
                        ErrorPlan::Forward
                    };
                    match plan {
                        // Another stream may work: try it before the core gives up on the item.
                        ErrorPlan::Retry { failed } => {
                            let item = inner.core.current_item().cloned();
                            let mut outbox = Outbox {
                                resolve: Some((report.load_id, Duration::ZERO)),
                                ..Outbox::default()
                            };
                            outbox.note(failed, item, message.clone());
                            if let Some(engine) = inner.active.take() {
                                outbox.engine.push((engine, EngineCommand::Stop));
                            }
                            let effects = inner.core.set_looking(report.load_id, true);
                            let looking = self.resolve(&mut inner, effects, cause);
                            outbox.state = looking.state;
                            outbox.current = looking.current;
                            drop(inner);
                            self.send(outbox);
                            return;
                        }
                        ErrorPlan::GiveUp { mark, failed } => {
                            let item = inner.core.current_item().cloned();
                            if let Some(failed) = failed {
                                lost = Some((failed, item.clone(), message.clone()));
                            }
                            if let Some(item) = item {
                                tracing::info!(load_id = report.load_id, title = %item.title, "no source played the song");
                                report.event = EngineEvent::Error {
                                    message: unplayable(&item.title),
                                };
                                gave_up = Some((item, mark));
                            }
                        }
                        ErrorPlan::Forward => {}
                    }
                }
                EngineEvent::Playing if current => {
                    if let Some(played) = inner.tracker.engine_playing(report.load_id) {
                        if played.rescued
                            && let Some(item) = inner.core.current_item().cloned()
                        {
                            tracing::info!(load_id = report.load_id, provider = %played.pick.provider_name, "the song plays again from another source");
                            rescued = Some((item, played.replaced));
                        }
                        verified = Some(played.pick);
                    }
                }
                EngineEvent::Loudness { lufs } if current => {
                    if let (Some(item), Some(sound)) = (
                        inner.core.current_item(),
                        self.app.try_state::<Arc<SoundService>>(),
                    ) {
                        let key = crate::sound::key(&TrackQuery::for_item(item));
                        let (sound, lufs) = (sound.inner().clone(), *lufs);
                        tauri::async_runtime::spawn(async move {
                            sound.record_loudness(key, lufs).await;
                        });
                    }
                }
                _ => {}
            }
            let effects = inner.core.report(report);
            let mut outbox = self.resolve(&mut inner, effects, cause);
            outbox.verified = verified;
            outbox.played = played;
            outbox.rescued = rescued;
            outbox.gave_up = gave_up;
            if let Some((failed, item, message)) = lost {
                outbox.note(failed, item, message);
            }
            outbox
        };
        self.send(outbox);
    }

    /// A resolution started by [`Self::start_resolving`] finished; `marked` says the song is
    /// marked as one no source had (only looked up when nothing was found).
    fn on_resolved(&self, load_id: u32, result: Result<Pick, String>, notes: Notes, marked: bool) {
        let outbox = {
            let mut inner = self.lock();
            if inner.core.load_id() != load_id {
                return;
            }
            match result {
                Ok(pick) => {
                    let position = inner.core.state().position_ms;
                    let Some(start_ms) =
                        inner
                            .tracker
                            .resolved(load_id, pick.clone(), &notes, position)
                    else {
                        return;
                    };
                    // Paused while it was being found (M46.16), it loads and waits; played
                    // meanwhile (a song reopened paused), it starts.
                    let autoplay = inner.core.plays_when_found();
                    let Some(item) = inner.core.current_item().cloned() else {
                        return;
                    };
                    let mut outbox = Outbox::default();
                    let fade_ms = match inner.pending_fade.take() {
                        Some((pending, fade_ms)) if pending == load_id => fade_ms,
                        _ => 0.0,
                    };
                    self.load_into(
                        &mut inner,
                        &mut outbox,
                        load_id,
                        &item,
                        &pick.source,
                        start_ms,
                        autoplay,
                        fade_ms,
                    );
                    let effects = inner
                        .core
                        .set_source_label(load_id, Some(pick.provider_name));
                    let labelled = self.resolve(&mut inner, effects, Cause::Advance);
                    outbox.state = labelled.state.or(outbox.state);
                    outbox.current = labelled.current.or(outbox.current);
                    outbox
                }
                Err(message) => {
                    let now = self.now_ms();
                    let Some(next) = inner.tracker.unresolved(load_id, &notes, now, marked) else {
                        return;
                    };
                    match next {
                        // Look again, harder, after a while; the player says it looks meanwhile.
                        Unresolved::Retry { wait } => {
                            tracing::info!(load_id, %message, ?wait, "no stream found; looking harder");
                            let effects = inner.core.set_looking(load_id, true);
                            Outbox {
                                resolve: Some((load_id, wait)),
                                ..self.resolve(&mut inner, effects, Cause::Advance)
                            }
                        }
                        Unresolved::GiveUp { mark } => {
                            tracing::info!(load_id, %message, "no stream found");
                            let gave_up =
                                inner.core.current_item().cloned().map(|item| (item, mark));
                            let effects = inner.core.report(EngineReport {
                                load_id,
                                event: EngineEvent::Error { message },
                            });
                            Outbox {
                                gave_up,
                                ..self.resolve(&mut inner, effects, Cause::AfterFailure)
                            }
                        }
                    }
                }
            }
        };
        self.send(outbox);
    }

    /// Resolves a stream for `load_id` in the background after `wait`, then loads it if that load
    /// is still current.
    fn start_resolving(&self, load_id: u32, wait: Duration) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            if !wait.is_zero() {
                tokio::time::sleep(wait).await;
            }
            let playback = app.state::<PlaybackService>();
            let (request, cause) = {
                let mut inner = playback.lock();
                (inner.tracker.request(load_id), inner.tracker.cause(load_id))
            };
            let Some((query, exclusions)) = request else {
                return;
            };
            let resolver = app.state::<Arc<ResolverService>>().inner().clone();
            let (result, notes) = resolver.find(&query, &exclusions).await;
            // A song the queue moved on to that's marked as one no source had is skipped when
            // its first search comes up empty, without looking harder.
            let marked = result.is_err()
                && !exclusions.deep
                && cause != Some(Cause::Chosen)
                && marked_unavailable(&app, &query).await;
            playback.on_resolved(load_id, result, notes, marked);
        });
    }

    fn run(&self, operation: impl FnOnce(&mut PlaybackCore) -> Vec<Effect>) {
        let outbox = {
            let mut inner = self.lock();
            let effects = operation(&mut inner.core);
            Outbox {
                asked: true,
                ..self.resolve(&mut inner, effects, Cause::Chosen)
            }
        };
        self.send(outbox);
    }

    /// Whether `state` goes to the webviews now; remembers it when it does.
    fn should_send(&self, state: &PlaybackState, asked: bool) -> bool {
        let mut sent = self.sent.lock().expect("sent state poisoned");
        let now = Instant::now();
        let send = match sent.as_ref() {
            Some((last, at)) if !asked => worth_sending(last, state, now.duration_since(*at)),
            Some((last, _)) => last != state,
            None => true,
        };
        if send {
            *sent = Some((state.clone(), now));
        }
        send
    }

    /// Carries out the core's effects; loads among them came about by `cause`.
    fn resolve(&self, inner: &mut Inner, effects: Vec<Effect>, cause: Cause) -> Outbox {
        let mut outbox = Outbox::default();
        for effect in effects {
            match effect {
                Effect::Load {
                    load_id,
                    item,
                    start_ms,
                    autoplay,
                    fade_ms,
                } => {
                    // A downloaded song plays its saved copy (M46.10), and its stream when the
                    // copy doesn't play.
                    let (item, plan) = match crate::music_downloads::offline_item(&self.app, &item)
                    {
                        Some(copy) => {
                            let file = copy.source.clone();
                            let plan = inner
                                .tracker
                                .begin_download(load_id, &item, file, start_ms, cause);
                            (copy, plan)
                        }
                        None => {
                            let plan = inner.tracker.begin(load_id, &item, start_ms, cause);
                            (item, plan)
                        }
                    };
                    match plan {
                        LoadPlan::Direct => {
                            let source = item.source.clone();
                            self.load_into(
                                inner,
                                &mut outbox,
                                load_id,
                                &item,
                                &source,
                                start_ms,
                                autoplay,
                                fade_ms,
                            );
                        }
                        LoadPlan::Resolve => {
                            tracing::debug!(load_id, title = %item.title, "finding a stream");
                            if fade_ms > 0.0 {
                                // A crossfade: the song ending plays on until the next is found.
                                inner.pending_fade = Some((load_id, fade_ms));
                            } else if let Some(previous) = inner.active.take() {
                                // Nothing to play until a stream is found; silence the previous item.
                                outbox.engine.push((previous, EngineCommand::Stop));
                            }
                            outbox.resolve = Some((load_id, Duration::ZERO));
                        }
                    }
                }
                Effect::Engine(command) => {
                    // With no engine holding the current item (its stream is still being found),
                    // a play or seek would only wake what the last song left behind: a stopped
                    // YouTube video would start again unseen. Volume and speed go with each load.
                    if let Some(engine) = inner.active {
                        outbox.engine.push((engine, command));
                    }
                }
                Effect::StateChanged => {
                    outbox.state = Some(inner.core.state());
                    outbox.current = inner.core.current_item().cloned();
                }
                Effect::QueueChanged => outbox.queue = Some(inner.core.queue()),
            }
        }
        outbox
    }

    fn send(&self, outbox: Outbox) {
        for (engine, command) in outbox.engine {
            match (engine, &self.native) {
                (Engine::Native, Some(send)) => {
                    if let Err(error) = send(&command) {
                        tracing::warn!(%error, "native engine rejected a command");
                    }
                }
                _ => log_emit(EngineCommandEvent(command).emit(&self.app)),
            }
        }
        if let Some(queue) = outbox.queue {
            log_emit(QueueEvent(queue).emit(&self.app));
        }
        if let Some(state) = outbox.state {
            crate::device_state::set_busy(matches!(
                state.status,
                raygun_model::PlaybackStatus::Loading
                    | raygun_model::PlaybackStatus::Buffering
                    | raygun_model::PlaybackStatus::Playing
            ));
            if let Some(podcasts) = self
                .app
                .try_state::<std::sync::Arc<crate::podcasts::PodcastService>>()
            {
                podcasts.on_playback(&state, outbox.current.as_ref());
            }
            if let Some(durations) = self
                .app
                .try_state::<std::sync::Arc<crate::durations::DurationService>>()
            {
                durations.on_playback(&state, outbox.current.as_ref());
            }
            // Followers here see every report; the webviews move the playhead on their own.
            if self.should_send(&state, outbox.asked) {
                log_emit(PlaybackStateEvent(state).emit(&self.app));
            }
        }
        let found = Found {
            verified: outbox.verified,
            failed: outbox.failed,
            stale: outbox.stale,
            dead: outbox.dead,
            broken_download: outbox.broken_download,
            rescued: outbox.rescued,
            gave_up: outbox.gave_up,
        };
        if !found.is_empty() {
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move { found.keep(&app).await });
        }
        if let Some(item) = outbox.played {
            self.app.state::<CollectionsService>().record_play(item);
        }
        if let Some((load_id, wait)) = outbox.resolve {
            self.start_resolving(load_id, wait);
        }
        if let Some((load_id, item)) = outbox.needs_webview {
            let app = self.app.clone();
            tauri::async_runtime::spawn(async move {
                let playback = app.state::<PlaybackService>();
                playback.hold_for_webview(load_id, &item).await;
            });
        }
    }

    /// Commands the right engine to load `source` for the current item.
    #[allow(clippy::too_many_arguments)]
    fn load_into(
        &self,
        inner: &mut Inner,
        outbox: &mut Outbox,
        load_id: u32,
        item: &QueueItem,
        source: &MediaSource,
        start_ms: f64,
        autoplay: bool,
        fade_ms: f64,
    ) {
        let engine = engine_for(source, self.native.is_some());
        tracing::debug!(load_id, title = %item.title, ?engine, fade_ms, "loading");
        if engine == Engine::Webview
            && self.native.is_some()
            && !self.webview.load(Ordering::SeqCst)
        {
            // YouTube's player lives in Raygun's webview, gone with its window: the song waits
            // there, paused, for the window to come back.
            if let Some(previous) = inner.active.filter(|previous| *previous != engine) {
                outbox.engine.push((previous, EngineCommand::Stop));
            }
            inner.active = Some(engine);
            outbox.needs_webview = Some((load_id, item.clone()));
            return;
        }
        if let Some(previous) = inner.active.filter(|previous| *previous != engine) {
            // In a crossfade the other engine fades its song out instead of cutting it (M46.12).
            let command = if fade_ms > 0.0 {
                EngineCommand::FadeOut { fade_ms }
            } else {
                EngineCommand::Stop
            };
            outbox.engine.push((previous, command));
        }
        inner.active = Some(engine);
        let volume = inner.core.state().volume;
        // The song's trims and gain (M46.12): a song from its top starts at its start trim.
        let tuning = self.tuning(&inner.sound, item);
        inner.core.set_end(load_id, tuning.end_ms);
        let start_ms = start_ms.max(tuning.start_ms);
        outbox.engine.push((
            engine,
            EngineCommand::SetEffects {
                sound: inner.sound.clone(),
            },
        ));
        let command = match source {
            MediaSource::YouTube { video_id } => EngineCommand::LoadYouTube {
                load_id,
                embed_url: self.proxy.youtube_embed_url(),
                video_id: video_id.clone(),
                start_ms,
                autoplay,
                volume,
                gain_db: tuning.gain_db,
                end_ms: tuning.end_ms,
                fade_ms,
            },
            source => EngineCommand::Load {
                load_id,
                url: self.proxy.register(source),
                title: item.title.clone(),
                artist: item.artist.clone(),
                artwork_url: item.artwork_url.clone(),
                start_ms,
                autoplay,
                volume,
                gain_db: tuning.gain_db,
                end_ms: tuning.end_ms,
                fade_ms,
                measure: tuning.measure,
            },
        };
        outbox.engine.push((engine, command));
        if autoplay {
            self.watch_start(load_id);
        }
        // Engines keep the speed across loads, but an engine just switched to may not know it.
        let rate = inner.core.rate();
        if (rate - 1.0).abs() > f64::EPSILON {
            outbox
                .engine
                .push((engine, EngineCommand::SetRate { rate }));
        }
    }

    /// An engine that never starts a load must not leave the player stuck (M46.16): after
    /// [`START_TIMEOUT`] without having played (loading, or buffering: engines say they buffer
    /// as soon as they're given a stream), it counts as the stream failing, so the next stream
    /// or the next song plays.
    fn watch_start(&self, load_id: u32) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(START_TIMEOUT).await;
            let playback = app.state::<PlaybackService>();
            let stuck = {
                let inner = playback.lock();
                let state = inner.core.state();
                // While another source is looked for, the engine has nothing to start.
                inner.core.load_id() == load_id
                    && matches!(
                        state.status,
                        raygun_model::PlaybackStatus::Loading
                            | raygun_model::PlaybackStatus::Buffering
                    )
                    && !inner.core.started()
                    && !inner.core.held()
                    && inner.tracker.loaded(load_id)
            };
            if stuck {
                tracing::warn!(load_id, "the stream didn't start in time");
                playback.report(EngineReport {
                    load_id,
                    event: EngineEvent::Error {
                        message: "This stream didn't start in time".into(),
                    },
                });
            }
        });
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("playback state poisoned")
    }
}

/// What the stream cache, the saved songs and the downloads learn from a load, written once the
/// playback lock is released.
struct Found {
    verified: Option<Pick>,
    failed: Option<(Pick, String)>,
    stale: Option<Pick>,
    dead: Option<(QueueItem, MediaSource)>,
    broken_download: Option<QueueItem>,
    rescued: Option<(QueueItem, Option<MediaSource>)>,
    gave_up: Option<(QueueItem, bool)>,
}

impl Found {
    fn is_empty(&self) -> bool {
        self.verified.is_none()
            && self.failed.is_none()
            && self.stale.is_none()
            && self.dead.is_none()
            && self.broken_download.is_none()
            && self.rescued.is_none()
            && self.gave_up.is_none()
    }

    async fn keep(self, app: &AppHandle) {
        let resolver = app.state::<Arc<ResolverService>>().inner().clone();
        if let Some(pick) = &self.verified {
            resolver.mark_verified(pick).await;
        }
        if let Some((pick, reason)) = &self.failed {
            resolver.mark_failed(pick, reason).await;
        }
        if let Some(pick) = &self.stale {
            resolver.forget_stream(pick).await;
        }
        if let Some((item, source)) = &self.dead {
            resolver.forget_source(&song_keys(item), source).await;
        }
        let collections = app.state::<CollectionsService>();
        let downloads = app
            .try_state::<Arc<crate::music_downloads::SongDownloadService>>()
            .map(|service| service.inner().clone());
        if let Some((item, replaced)) = &self.rescued {
            let song = song_of(item);
            collections.source_found(&song, replaced.as_ref()).await;
            // A downloaded song's copy may be from the stream that stopped working.
            if let Some(downloads) = &downloads {
                downloads.check_song(&song).await;
            }
        }
        if let (Some(item), Some(downloads)) = (&self.broken_download, &downloads) {
            downloads.check_song(&song_of(item)).await;
        }
        if let Some((item, true)) = &self.gave_up {
            collections.mark_unplayable(&song_of(item)).await;
        }
    }
}

/// A queue item as the song it is, for its marks, saved copies and download.
fn song_of(item: &QueueItem) -> TrackInput {
    TrackInput {
        title: item.title.clone(),
        artist: item.artist.clone(),
        album: item.album.clone(),
        duration_ms: item.duration_ms,
        artwork_url: None,
        source: None,
        explicit: item.explicit,
    }
}

/// The keys a queue item's song is remembered under: the resolver's and the saved songs'.
fn song_keys(item: &QueueItem) -> Vec<String> {
    let mut keys = crate::collections::identity::keys_of(&item.title, item.artist.as_deref());
    keys.push(raygun_resolver::track_key(&TrackQuery::for_item(item)));
    keys.sort_unstable();
    keys.dedup();
    keys
}

/// Whether the song `query` names is marked as one no source had.
async fn marked_unavailable(app: &AppHandle, query: &TrackQuery) -> bool {
    let db = &app.state::<crate::startup::AppDb>().inner().0;
    let mut keys = crate::collections::identity::keys_of(&query.title, query.artist.as_deref());
    keys.push(raygun_resolver::track_key(query));
    raygun_db::collections::unavailable(db, &keys)
        .await
        .is_ok_and(|marked| !marked.is_empty())
}

/// Why a song was skipped after looking for another source for it.
fn unplayable(title: &str) -> String {
    format!("No source could play \u{201c}{title}\u{201d}")
}

/// How often the queue is saved for reopening (M46.16).
const SAVE_EVERY: Duration = Duration::from_secs(5);

/// A position that only moves on with playback reaches the webviews at most this often: engines
/// report two to four times a second, and each report re-rendered every player view. The views
/// move the playhead themselves between reports (`player/useLivePosition.ts`).
const POSITION_EVERY: Duration = Duration::from_secs(1);

/// A position this far (ms) from where playback would have moved to is a jump (an engine's own
/// seek, a stream starting over), sent at once.
const POSITION_JUMP_MS: f64 = 1_000.0;

/// Whether the webviews need `next`, `since` after they got `last`: any change but the position
/// goes at once; a position moving on as it plays waits for [`POSITION_EVERY`].
fn worth_sending(last: &PlaybackState, next: &PlaybackState, since: Duration) -> bool {
    if last == next {
        return false;
    }
    let only_position = PlaybackState {
        position_ms: last.position_ms,
        ..next.clone()
    } == *last;
    if !only_position || since >= POSITION_EVERY {
        return true;
    }
    let expected = if next.status == raygun_model::PlaybackStatus::Playing {
        last.position_ms + since.as_secs_f64() * 1_000.0 * next.rate
    } else {
        last.position_ms
    };
    (next.position_ms - expected).abs() > POSITION_JUMP_MS
}

/// How long a load may take to start playing before another stream is tried (M46.16).
const START_TIMEOUT: Duration = Duration::from_secs(25);

/// How often the watchdog checks that what plays moves on.
const WATCH_EVERY: Duration = Duration::from_secs(1);

/// Another kind of media starts here (a video, a trailer, read-aloud): music pauses, unless
/// Settings → Playback keeps it playing (M46.16). Music playing on another device plays on.
pub async fn other_media_started(app: &AppHandle) {
    let db = app.state::<crate::startup::AppDb>().inner().0.clone();
    let keep = crate::settings::load(&db)
        .await
        .map(|settings| settings.keep_music_playing)
        .unwrap_or(false);
    if !keep {
        app.state::<PlaybackService>()
            .control_here(PlaybackControl::Pause);
    }
}

fn log_emit(result: tauri::Result<()>) {
    if let Err(error) = result {
        tracing::warn!(%error, "failed to emit playback event");
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use super::*;

    #[test]
    fn youtube_always_plays_in_the_webview() {
        let source = MediaSource::YouTube {
            video_id: "M7lc1UVf-VE".into(),
        };
        assert_eq!(engine_for(&source, true), Engine::Webview);
        assert_eq!(engine_for(&source, false), Engine::Webview);
    }

    fn playing_at(position_ms: f64) -> PlaybackState {
        PlaybackState {
            status: raygun_model::PlaybackStatus::Playing,
            current_item_id: Some("a".into()),
            position_ms,
            duration_ms: Some(180_000.0),
            volume: 1.0,
            repeat: raygun_model::RepeatMode::Off,
            shuffle: false,
            error: None,
            source_label: None,
            rate: 1.0,
            sleep_at: None,
            stop_after_current: false,
            looking_for_source: false,
            skipped_item_id: None,
        }
    }

    #[test]
    fn positions_moving_on_reach_the_webviews_once_a_second() {
        let last = playing_at(10_000.0);
        let quarter = Duration::from_millis(250);

        // The same state, or the position moving on as it plays, waits.
        assert!(!worth_sending(&last, &last, quarter));
        assert!(!worth_sending(&last, &playing_at(10_250.0), quarter));
        // A second on, it goes.
        assert!(worth_sending(
            &last,
            &playing_at(11_000.0),
            Duration::from_secs(1)
        ));
        // A jump the engine made goes at once, as does any other change.
        assert!(worth_sending(&last, &playing_at(42_000.0), quarter));
        let paused = PlaybackState {
            status: raygun_model::PlaybackStatus::Paused,
            ..playing_at(10_250.0)
        };
        assert!(worth_sending(&last, &paused, quarter));
        let louder = PlaybackState {
            volume: 0.5,
            ..last.clone()
        };
        assert!(worth_sending(&last, &louder, quarter));
        // At double speed, two seconds a second is still moving on.
        let fast = PlaybackState {
            rate: 2.0,
            ..last.clone()
        };
        let fast_later = PlaybackState {
            position_ms: 11_000.0,
            ..fast.clone()
        };
        assert!(!worth_sending(
            &fast,
            &fast_later,
            Duration::from_millis(500)
        ));
        // Paused, any move is a seek.
        let paused_later = PlaybackState {
            position_ms: 10_900.0,
            ..paused.clone()
        };
        assert!(!worth_sending(&paused, &paused, quarter));
        assert!(worth_sending(
            &paused,
            &PlaybackState {
                position_ms: 12_000.0,
                ..paused_later
            },
            quarter
        ));
    }

    #[test]
    fn streams_and_files_prefer_the_native_engine() {
        let stream = MediaSource::Url {
            url: "https://example.com/a.mp3".into(),
            headers: BTreeMap::new(),
        };
        let file = MediaSource::LocalFile {
            path: "/sdcard/Music/a.mp3".into(),
        };
        assert_eq!(engine_for(&stream, true), Engine::Native);
        assert_eq!(engine_for(&file, true), Engine::Native);
        assert_eq!(engine_for(&file, false), Engine::Webview);
    }
}
