//! Runs the playback state machine and carries out its effects: resolving sources through the
//! stream proxy, commanding the engine and broadcasting state to every client.

use std::sync::Mutex;

use raygun_model::{
    EngineCommand, EngineReport, PlaybackControl, PlaybackState, QueueItemInput, QueueSnapshot,
};
use raygun_net::StreamProxy;
use raygun_playback::{Effect, PlaybackCore};
use tauri::AppHandle;
use tauri_specta::Event;

use crate::events::{EngineCommandEvent, PlaybackStateEvent, QueueEvent};

pub struct PlaybackService {
    core: Mutex<PlaybackCore>,
    proxy: StreamProxy,
    app: AppHandle,
}

/// Everything to emit once the core lock is released.
#[derive(Default)]
struct Outbox {
    engine: Vec<EngineCommand>,
    state: Option<PlaybackState>,
    queue: Option<QueueSnapshot>,
}

impl PlaybackService {
    pub fn new(app: AppHandle, proxy: StreamProxy) -> Self {
        let seed = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .map(|elapsed| elapsed.as_nanos() as u64)
            .unwrap_or(1);
        Self {
            core: Mutex::new(PlaybackCore::new(seed)),
            proxy,
            app,
        }
    }

    pub fn state(&self) -> PlaybackState {
        self.lock().state()
    }

    pub fn queue(&self) -> QueueSnapshot {
        self.lock().queue()
    }

    pub fn enqueue(&self, items: Vec<QueueItemInput>, play_now: bool) {
        self.run(|core| core.enqueue(items, play_now));
    }

    pub fn control(&self, control: PlaybackControl) {
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
        });
    }

    pub fn report(&self, report: EngineReport) {
        self.run(|core| core.report(report));
    }

    fn run(&self, operation: impl FnOnce(&mut PlaybackCore) -> Vec<Effect>) {
        let outbox = {
            let mut core = self.lock();
            let effects = operation(&mut core);
            self.resolve(&core, effects)
        };
        self.send(outbox);
    }

    fn resolve(&self, core: &PlaybackCore, effects: Vec<Effect>) -> Outbox {
        let mut outbox = Outbox::default();
        for effect in effects {
            match effect {
                Effect::Load {
                    load_id,
                    item,
                    start_ms,
                    autoplay,
                } => {
                    tracing::debug!(load_id, title = %item.title, "loading");
                    outbox.engine.push(EngineCommand::Load {
                        load_id,
                        url: self.proxy.register(&item.source),
                        start_ms,
                        autoplay,
                        volume: core.state().volume,
                    });
                }
                Effect::Engine(command) => outbox.engine.push(command),
                Effect::StateChanged => outbox.state = Some(core.state()),
                Effect::QueueChanged => outbox.queue = Some(core.queue()),
            }
        }
        outbox
    }

    fn send(&self, outbox: Outbox) {
        for command in outbox.engine {
            log_emit(EngineCommandEvent(command).emit(&self.app));
        }
        if let Some(queue) = outbox.queue {
            log_emit(QueueEvent(queue).emit(&self.app));
        }
        if let Some(state) = outbox.state {
            log_emit(PlaybackStateEvent(state).emit(&self.app));
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, PlaybackCore> {
        self.core.lock().expect("playback core poisoned")
    }
}

fn log_emit(result: tauri::Result<()>) {
    if let Err(error) = result {
        tracing::warn!(%error, "failed to emit playback event");
    }
}
