//! Runs the playback state machine and carries out its effects: finding streams for items that
//! need one (M14), serving sources through the stream proxy, commanding the right engine for each
//! item, broadcasting state to every client and counting plays for history (M18).

use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

use raygun_model::{
    ConnectCommand, EngineCommand, EngineEvent, EngineReport, MediaSource, PlaybackControl,
    PlaybackState, QueueItem, QueueItemInput, QueueSnapshot, SleepTimer, SoundPrefs, TrackQuery,
};
use raygun_net::StreamProxy;
use raygun_playback::{Effect, PlaybackCore};
use raygun_resolver::Pick;
use tauri::{AppHandle, Manager};
use tauri_specta::Event;

use crate::collections::CollectionsService;
use crate::events::{EngineCommandEvent, PlaybackStateEvent, QueueEvent};
use crate::history::PlayCounter;
use crate::resolution::{ErrorPlan, LoadPlan, Tracker};
use crate::resolver::ResolverService;
use crate::sound::{SoundService, Tuning};

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
}

/// Everything to emit once the core lock is released.
#[derive(Default)]
struct Outbox {
    engine: Vec<(Engine, EngineCommand)>,
    state: Option<PlaybackState>,
    queue: Option<QueueSnapshot>,
    /// Find a stream for this load.
    resolve: Option<u32>,
    verified: Option<Pick>,
    failed: Option<(Pick, String)>,
    /// The current item just counted as a play.
    played: Option<QueueItem>,
    /// The current item when `state` changed, for followers like podcasts (M37).
    current: Option<QueueItem>,
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
        }
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
            ConnectCommand::Replace {
                items,
                start_index,
                shuffle,
                start_ms,
                play,
            } => {
                self.run(|core| {
                    core.replace_at(
                        items,
                        start_index.map(|index| index as usize),
                        shuffle,
                        start_ms,
                    )
                });
                if !play {
                    self.run(|core| core.pause());
                }
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

    pub fn queue(&self) -> QueueSnapshot {
        self.lock().core.queue()
    }

    pub fn enqueue(&self, items: Vec<QueueItemInput>, play_now: bool) {
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Enqueue { items, play_now });
        }
        self.run(|core| core.enqueue(items, play_now));
    }

    /// Replaces the queue and starts playing (see `PlaybackCore::replace`).
    pub fn replace(&self, items: Vec<QueueItemInput>, start_index: Option<usize>, shuffle: bool) {
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Replace {
                items,
                start_index: start_index.map(|index| index as u32),
                shuffle,
                start_ms: 0.0,
                play: true,
            });
        }
        self.run(|core| core.replace(items, start_index, shuffle));
    }

    /// Replaces the queue and starts its first item at `start_ms` (a podcast episode resuming).
    pub fn replace_from(&self, items: Vec<QueueItemInput>, start_ms: f64) {
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Replace {
                items,
                start_index: Some(0),
                shuffle: false,
                start_ms,
                play: true,
            });
        }
        self.run(|core| core.replace_at(items, Some(0), false, start_ms));
    }

    pub fn control(&self, control: PlaybackControl) {
        if let Some(send) = self.redirect() {
            return send(ConnectCommand::Control { control });
        }
        self.control_here(control);
    }

    /// Controls this device's own player, even while it plays elsewhere.
    pub fn control_here(&self, control: PlaybackControl) {
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

    pub fn report(&self, report: EngineReport) {
        let outbox = {
            let mut inner = self.lock();
            let current = report.load_id == inner.core.load_id();
            let mut verified = None;
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
                    if current
                        && let ErrorPlan::Retry { failed } =
                            inner.tracker.engine_error(report.load_id)
                    {
                        // Another stream may work: try it before the core gives up on the item.
                        let mut outbox = Outbox {
                            resolve: Some(report.load_id),
                            failed: failed.map(|pick| (*pick, message.clone())),
                            ..Outbox::default()
                        };
                        if let Some(engine) = inner.active.take() {
                            outbox.engine.push((engine, EngineCommand::Stop));
                        }
                        drop(inner);
                        self.send(outbox);
                        return;
                    }
                }
                EngineEvent::Playing if current => {
                    verified = inner.tracker.engine_playing(report.load_id);
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
            let mut outbox = self.resolve(&mut inner, effects);
            outbox.verified = verified;
            outbox.played = played;
            outbox
        };
        self.send(outbox);
    }

    /// A resolution started by [`Self::start_resolving`] finished.
    fn on_resolved(&self, load_id: u32, result: Result<Pick, String>) {
        let outbox = {
            let mut inner = self.lock();
            if inner.core.load_id() != load_id {
                return;
            }
            match result {
                Ok(pick) => {
                    let position = inner.core.state().position_ms;
                    let Some((start_ms, autoplay)) =
                        inner.tracker.resolved(load_id, pick.clone(), position)
                    else {
                        return;
                    };
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
                    let labelled = self.resolve(&mut inner, effects);
                    outbox.state = labelled.state.or(outbox.state);
                    outbox.current = labelled.current.or(outbox.current);
                    outbox
                }
                Err(message) => {
                    if inner.tracker.request(load_id).is_none() {
                        return;
                    }
                    tracing::info!(load_id, %message, "no stream found");
                    let effects = inner.core.report(EngineReport {
                        load_id,
                        event: EngineEvent::Error { message },
                    });
                    self.resolve(&mut inner, effects)
                }
            }
        };
        self.send(outbox);
    }

    /// Resolves a stream for `load_id` in the background, then loads it if that load is still
    /// current.
    fn start_resolving(&self, load_id: u32) {
        let app = self.app.clone();
        tauri::async_runtime::spawn(async move {
            let playback = app.state::<PlaybackService>();
            let Some((query, exclusions)) = playback.lock().tracker.request(load_id) else {
                return;
            };
            let resolver = app.state::<Arc<ResolverService>>().inner().clone();
            let result = resolver.resolve(&query, &exclusions).await;
            playback.on_resolved(load_id, result);
        });
    }

    fn run(&self, operation: impl FnOnce(&mut PlaybackCore) -> Vec<Effect>) {
        let outbox = {
            let mut inner = self.lock();
            let effects = operation(&mut inner.core);
            self.resolve(&mut inner, effects)
        };
        self.send(outbox);
    }

    fn resolve(&self, inner: &mut Inner, effects: Vec<Effect>) -> Outbox {
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
                    // A downloaded song plays its saved copy (M46.10).
                    let item =
                        crate::music_downloads::offline_item(&self.app, &item).unwrap_or(item);
                    match inner.tracker.begin(load_id, &item, start_ms, autoplay) {
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
                            outbox.resolve = Some(load_id);
                        }
                    }
                }
                Effect::Engine(command) => {
                    let engine = inner.active.unwrap_or(Engine::Webview);
                    outbox.engine.push((engine, command));
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
            if let Some(podcasts) = self
                .app
                .try_state::<std::sync::Arc<crate::podcasts::PodcastService>>()
            {
                podcasts.on_playback(&state, outbox.current.as_ref());
            }
            if let Some(songs) = self
                .app
                .try_state::<std::sync::Arc<crate::music_downloads::SongDownloadService>>()
            {
                songs.on_playback(&state, outbox.current.as_ref());
            }
            log_emit(PlaybackStateEvent(state).emit(&self.app));
        }
        if outbox.verified.is_some() || outbox.failed.is_some() {
            let resolver = self.app.state::<Arc<ResolverService>>().inner().clone();
            let (verified, failed) = (outbox.verified, outbox.failed);
            tauri::async_runtime::spawn(async move {
                if let Some(pick) = verified {
                    resolver.mark_verified(&pick).await;
                }
                if let Some((pick, reason)) = failed {
                    resolver.mark_failed(&pick, &reason).await;
                }
            });
        }
        if let Some(item) = outbox.played {
            self.app.state::<CollectionsService>().record_play(item);
        }
        if let Some(load_id) = outbox.resolve {
            self.start_resolving(load_id);
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
        // Engines keep the speed across loads, but an engine just switched to may not know it.
        let rate = inner.core.rate();
        if (rate - 1.0).abs() > f64::EPSILON {
            outbox
                .engine
                .push((engine, EngineCommand::SetRate { rate }));
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner.lock().expect("playback state poisoned")
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
