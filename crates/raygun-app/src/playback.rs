//! Runs the playback state machine and carries out its effects: resolving sources through the
//! stream proxy, commanding the right engine for each item and broadcasting state to every
//! client.

use std::sync::Mutex;

use raygun_model::{
    EngineCommand, EngineReport, MediaSource, PlaybackControl, PlaybackState, QueueItemInput,
    QueueSnapshot,
};
use raygun_net::StreamProxy;
use raygun_playback::{Effect, PlaybackCore};
use tauri::AppHandle;
use tauri_specta::Event;

use crate::events::{EngineCommandEvent, PlaybackStateEvent, QueueEvent};

/// Delivers one command to a native engine.
pub type NativeEngineSink = Box<dyn Fn(&EngineCommand) -> Result<(), String> + Send + Sync>;

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
}

pub struct PlaybackService {
    inner: Mutex<Inner>,
    proxy: StreamProxy,
    app: AppHandle,
    native: Option<NativeEngineSink>,
}

/// Everything to emit once the core lock is released.
#[derive(Default)]
struct Outbox {
    engine: Vec<(Engine, EngineCommand)>,
    state: Option<PlaybackState>,
    queue: Option<QueueSnapshot>,
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
            }),
            proxy,
            app,
            native,
        }
    }

    pub fn state(&self) -> PlaybackState {
        self.lock().core.state()
    }

    pub fn queue(&self) -> QueueSnapshot {
        self.lock().core.queue()
    }

    pub fn enqueue(&self, items: Vec<QueueItemInput>, play_now: bool) {
        self.run(|core| core.enqueue(items, play_now));
    }

    /// Replaces the queue and starts playing (see `PlaybackCore::replace`).
    pub fn replace(&self, items: Vec<QueueItemInput>, start_index: Option<usize>, shuffle: bool) {
        self.run(|core| core.replace(items, start_index, shuffle));
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
        if let raygun_model::EngineEvent::Error { message } = &report.event {
            tracing::info!(load_id = report.load_id, %message, "engine reported an error");
        }
        self.run(|core| core.report(report));
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
                } => {
                    let engine = engine_for(&item.source, self.native.is_some());
                    tracing::debug!(load_id, title = %item.title, ?engine, "loading");
                    if let Some(previous) = inner.active.filter(|previous| *previous != engine) {
                        outbox.engine.push((previous, EngineCommand::Stop));
                    }
                    inner.active = Some(engine);
                    let volume = inner.core.state().volume;
                    let command = match &item.source {
                        MediaSource::YouTube { video_id } => EngineCommand::LoadYouTube {
                            load_id,
                            embed_url: self.proxy.youtube_embed_url(),
                            video_id: video_id.clone(),
                            start_ms,
                            autoplay,
                            volume,
                        },
                        source => EngineCommand::Load {
                            load_id,
                            url: self.proxy.register(source),
                            title: item.title.clone(),
                            artist: item.artist.clone(),
                            start_ms,
                            autoplay,
                            volume,
                        },
                    };
                    outbox.engine.push((engine, command));
                }
                Effect::Engine(command) => {
                    let engine = inner.active.unwrap_or(Engine::Webview);
                    outbox.engine.push((engine, command));
                }
                Effect::StateChanged => outbox.state = Some(inner.core.state()),
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
            log_emit(PlaybackStateEvent(state).emit(&self.app));
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
