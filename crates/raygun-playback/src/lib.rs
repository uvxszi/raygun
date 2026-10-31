//! Raygun's playback state machine (ARCHITECTURE §6.2).
//!
//! [`PlaybackCore`] owns the queue and the canonical player state. It does no I/O: every
//! operation returns the [`Effect`]s the caller must carry out (load a stream into the active
//! engine, forward a command, broadcast new state). That keeps all queue rules unit-testable and
//! lets any engine (webview `<audio>`, YouTube embed, native player) sit behind the same protocol.

mod shuffle;

use raygun_model::{
    EngineCommand, EngineEvent, EngineReport, PlaybackState, PlaybackStatus, QueueItem,
    QueueItemInput, QueueSnapshot, RepeatMode,
};

use crate::shuffle::XorShift;

/// `previous` restarts the current track instead of going back once this far in.
const RESTART_THRESHOLD_MS: f64 = 3_000.0;
/// After this many failed items in a row playback stops instead of trying the rest of the queue
/// (a long list of search results could otherwise fail through dozens of items, M14).
const MAX_SKIPPED_ERRORS: usize = 3;

/// Work the caller must do after an operation.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Resolve `item`'s source to a playable URL and send `EngineCommand::Load` with this `load_id`.
    Load {
        load_id: u32,
        item: QueueItem,
        start_ms: f64,
        autoplay: bool,
    },
    /// Forward to the active engine.
    Engine(EngineCommand),
    /// [`PlaybackCore::state`] changed.
    StateChanged,
    /// [`PlaybackCore::queue`] changed.
    QueueChanged,
}

pub struct PlaybackCore {
    items: Vec<QueueItem>,
    /// Indices into `items` in play order.
    order: Vec<usize>,
    /// Position within `order` of the current item.
    cursor: Option<usize>,
    status: PlaybackStatus,
    position_ms: f64,
    duration_ms: Option<f64>,
    volume: f64,
    repeat: RepeatMode,
    shuffle: bool,
    error: Option<String>,
    /// See [`PlaybackState::source_label`].
    source_label: Option<String>,
    load_id: u32,
    consecutive_errors: usize,
    rng: XorShift,
}

impl PlaybackCore {
    /// `seed` drives shuffling; tests pass a fixed value for reproducible orders.
    pub fn new(seed: u64) -> Self {
        Self {
            items: Vec::new(),
            order: Vec::new(),
            cursor: None,
            status: PlaybackStatus::Idle,
            position_ms: 0.0,
            duration_ms: None,
            volume: 1.0,
            repeat: RepeatMode::Off,
            shuffle: false,
            error: None,
            source_label: None,
            load_id: 0,
            consecutive_errors: 0,
            rng: XorShift::new(seed),
        }
    }

    pub fn state(&self) -> PlaybackState {
        PlaybackState {
            status: self.status,
            current_item_id: self.current_item().map(|item| item.id.clone()),
            position_ms: self.position_ms,
            duration_ms: self.duration_ms,
            volume: self.volume,
            repeat: self.repeat,
            shuffle: self.shuffle,
            error: self.error.clone(),
            source_label: self.source_label.clone(),
        }
    }

    /// The id of the latest `Effect::Load`; engine reports and async resolutions must match it.
    pub fn load_id(&self) -> u32 {
        self.load_id
    }

    /// Records where the current load's stream came from (the resolver's pick). Ignored when a
    /// newer load has started.
    pub fn set_source_label(&mut self, load_id: u32, label: Option<String>) -> Vec<Effect> {
        if load_id != self.load_id || self.cursor.is_none() || self.source_label == label {
            return Vec::new();
        }
        self.source_label = label;
        vec![Effect::StateChanged]
    }

    pub fn queue(&self) -> QueueSnapshot {
        QueueSnapshot {
            items: self.items.clone(),
            order: self.order.iter().map(|&index| index as u32).collect(),
        }
    }

    pub fn current_item(&self) -> Option<&QueueItem> {
        self.cursor.map(|cursor| &self.items[self.order[cursor]])
    }

    /// Appends items. With `play_now`, the first new item starts playing immediately.
    pub fn enqueue(&mut self, inputs: Vec<QueueItemInput>, play_now: bool) -> Vec<Effect> {
        if inputs.is_empty() {
            return Vec::new();
        }
        let first_new = self.items.len();
        for input in inputs {
            let index = self.items.len();
            self.items.push(QueueItem {
                id: uuid::Uuid::now_v7().to_string(),
                title: input.title,
                artist: input.artist,
                album: input.album,
                duration_ms: input.duration_ms,
                artwork_url: input.artwork_url,
                media_kind: input.media_kind,
                source: input.source,
            });
            if self.shuffle {
                let earliest = self.cursor.map_or(0, |cursor| cursor + 1);
                let slot = earliest + self.rng.below(self.order.len() - earliest + 1);
                self.order.insert(slot, index);
            } else {
                self.order.push(index);
            }
        }

        let mut effects = vec![Effect::QueueChanged];
        if play_now {
            let cursor = self.position_in_order(first_new);
            effects.extend(self.load_at(cursor, 0.0, true));
        }
        effects
    }

    /// Replaces the whole queue and starts playing ("play this album"). With `shuffle`, the start
    /// item plays first and the rest follow in random order; without a start item, shuffle picks
    /// a random one.
    pub fn replace(
        &mut self,
        inputs: Vec<QueueItemInput>,
        start_index: Option<usize>,
        shuffle: bool,
    ) -> Vec<Effect> {
        if inputs.is_empty() {
            return self.clear();
        }
        self.items.clear();
        self.order.clear();
        self.cursor = None;
        self.consecutive_errors = 0;
        self.shuffle = false;
        self.enqueue(inputs, false);

        let count = self.items.len();
        let start = match start_index {
            Some(index) => index.min(count - 1),
            None if shuffle => self.rng.below(count),
            None => 0,
        };
        if shuffle {
            let mut rest: Vec<usize> = (0..count).filter(|&index| index != start).collect();
            self.rng.shuffle(&mut rest);
            self.order = std::iter::once(start).chain(rest).collect();
            self.shuffle = true;
        }
        let cursor = self.position_in_order(start);
        let mut effects = vec![Effect::QueueChanged];
        effects.extend(self.load_at(cursor, 0.0, true));
        effects
    }

    pub fn play(&mut self) -> Vec<Effect> {
        match self.cursor {
            None if !self.items.is_empty() => self.load_at(0, 0.0, true),
            None => Vec::new(),
            Some(cursor) if self.status == PlaybackStatus::Ended => self.load_at(cursor, 0.0, true),
            Some(cursor) if self.status == PlaybackStatus::Error => {
                self.load_at(cursor, self.position_ms, true)
            }
            Some(_) => vec![Effect::Engine(EngineCommand::Play)],
        }
    }

    pub fn pause(&mut self) -> Vec<Effect> {
        if self.cursor.is_none() {
            return Vec::new();
        }
        vec![Effect::Engine(EngineCommand::Pause)]
    }

    pub fn toggle(&mut self) -> Vec<Effect> {
        match self.status {
            PlaybackStatus::Playing | PlaybackStatus::Loading | PlaybackStatus::Buffering => {
                self.pause()
            }
            _ => self.play(),
        }
    }

    /// User skip. Ignores repeat-one; wraps around only with repeat-all.
    #[allow(clippy::should_implement_trait)] // "next track", not an iterator
    pub fn next(&mut self) -> Vec<Effect> {
        match self.cursor {
            None if !self.items.is_empty() => self.load_at(0, 0.0, true),
            None => Vec::new(),
            Some(cursor) if cursor + 1 < self.order.len() => self.load_at(cursor + 1, 0.0, true),
            Some(_) if self.repeat == RepeatMode::All => self.load_at(0, 0.0, true),
            Some(_) => Vec::new(),
        }
    }

    pub fn previous(&mut self) -> Vec<Effect> {
        let Some(cursor) = self.cursor else {
            return Vec::new();
        };
        if self.status == PlaybackStatus::Ended {
            // The engine is parked at the end; seeking alone would leave it silent.
            return self.load_at(cursor, 0.0, true);
        }
        if self.position_ms > RESTART_THRESHOLD_MS {
            return self.seek(0.0);
        }
        if cursor > 0 {
            self.load_at(cursor - 1, 0.0, true)
        } else if self.repeat == RepeatMode::All && self.order.len() > 1 {
            self.load_at(self.order.len() - 1, 0.0, true)
        } else {
            self.seek(0.0)
        }
    }

    pub fn seek(&mut self, position_ms: f64) -> Vec<Effect> {
        if self.cursor.is_none() {
            return Vec::new();
        }
        let upper = self.duration_ms.unwrap_or(f64::MAX);
        self.position_ms = position_ms.clamp(0.0, upper);
        vec![
            Effect::Engine(EngineCommand::Seek {
                position_ms: self.position_ms,
            }),
            Effect::StateChanged,
        ]
    }

    pub fn set_volume(&mut self, volume: f64) -> Vec<Effect> {
        self.volume = volume.clamp(0.0, 1.0);
        vec![
            Effect::Engine(EngineCommand::SetVolume {
                volume: self.volume,
            }),
            Effect::StateChanged,
        ]
    }

    pub fn set_repeat(&mut self, mode: RepeatMode) -> Vec<Effect> {
        self.repeat = mode;
        vec![Effect::StateChanged]
    }

    /// Turning shuffle on keeps the current item first and shuffles the rest; turning it off
    /// restores queue order and keeps playing the same item.
    pub fn set_shuffle(&mut self, enabled: bool) -> Vec<Effect> {
        if enabled == self.shuffle {
            return Vec::new();
        }
        self.shuffle = enabled;
        let current = self.cursor.map(|cursor| self.order[cursor]);
        if enabled {
            let mut rest: Vec<usize> = (0..self.items.len())
                .filter(|&index| Some(index) != current)
                .collect();
            self.rng.shuffle(&mut rest);
            self.order = current.into_iter().chain(rest).collect();
            self.cursor = current.map(|_| 0);
        } else {
            self.order = (0..self.items.len()).collect();
            self.cursor = current;
        }
        vec![Effect::QueueChanged, Effect::StateChanged]
    }

    pub fn play_item(&mut self, item_id: &str) -> Vec<Effect> {
        match self.items.iter().position(|item| item.id == item_id) {
            Some(index) => {
                let cursor = self.position_in_order(index);
                self.load_at(cursor, 0.0, true)
            }
            None => Vec::new(),
        }
    }

    /// Removes an item. Removing the playing item moves on to the next one.
    pub fn remove(&mut self, item_id: &str) -> Vec<Effect> {
        let Some(index) = self.items.iter().position(|item| item.id == item_id) else {
            return Vec::new();
        };
        let removed_position = self.position_in_order(index);
        let was_current = self.cursor == Some(removed_position);
        let was_active = matches!(
            self.status,
            PlaybackStatus::Playing | PlaybackStatus::Loading | PlaybackStatus::Buffering
        );

        self.items.remove(index);
        self.order.remove(removed_position);
        for entry in &mut self.order {
            if *entry > index {
                *entry -= 1;
            }
        }

        let mut effects = vec![Effect::QueueChanged];
        match self.cursor {
            Some(cursor) if removed_position < cursor => self.cursor = Some(cursor - 1),
            Some(cursor) if was_current => {
                if cursor < self.order.len() {
                    effects.extend(self.load_at(cursor, 0.0, was_active));
                } else {
                    effects.extend(self.stop());
                }
            }
            _ => {}
        }
        effects
    }

    pub fn clear(&mut self) -> Vec<Effect> {
        self.items.clear();
        self.order.clear();
        let mut effects = vec![Effect::QueueChanged];
        effects.extend(self.stop());
        effects
    }

    /// Applies an engine report. Reports for anything but the latest load are ignored.
    pub fn report(&mut self, report: EngineReport) -> Vec<Effect> {
        if report.load_id != self.load_id || self.cursor.is_none() {
            return Vec::new();
        }
        match report.event {
            EngineEvent::Progress {
                position_ms,
                duration_ms,
            } => {
                self.position_ms = position_ms;
                if duration_ms.is_some() {
                    self.duration_ms = duration_ms;
                }
                vec![Effect::StateChanged]
            }
            EngineEvent::Playing => {
                self.consecutive_errors = 0;
                self.error = None;
                self.set_status(PlaybackStatus::Playing)
            }
            EngineEvent::Paused => self.set_status(PlaybackStatus::Paused),
            EngineEvent::Buffering => self.set_status(PlaybackStatus::Buffering),
            EngineEvent::Ended => self.on_ended(),
            EngineEvent::Error { message } => self.on_error(message),
        }
    }

    fn on_ended(&mut self) -> Vec<Effect> {
        let cursor = self.cursor.expect("report checked cursor");
        if self.repeat == RepeatMode::One {
            return self.load_at(cursor, 0.0, true);
        }
        let effects = self.next();
        if !effects.is_empty() {
            return effects;
        }
        self.status = PlaybackStatus::Ended;
        if let Some(duration) = self.duration_ms {
            self.position_ms = duration;
        }
        vec![Effect::StateChanged]
    }

    fn on_error(&mut self, message: String) -> Vec<Effect> {
        self.consecutive_errors += 1;
        self.status = PlaybackStatus::Error;
        self.error = Some(message);
        if self.consecutive_errors >= self.items.len().min(MAX_SKIPPED_ERRORS) {
            return vec![Effect::Engine(EngineCommand::Stop), Effect::StateChanged];
        }
        let effects = self.next();
        if effects.is_empty() {
            vec![Effect::Engine(EngineCommand::Stop), Effect::StateChanged]
        } else {
            effects
        }
    }

    fn stop(&mut self) -> Vec<Effect> {
        self.cursor = None;
        self.status = PlaybackStatus::Idle;
        self.position_ms = 0.0;
        self.duration_ms = None;
        self.error = None;
        self.source_label = None;
        vec![Effect::Engine(EngineCommand::Stop), Effect::StateChanged]
    }

    fn load_at(&mut self, cursor: usize, start_ms: f64, autoplay: bool) -> Vec<Effect> {
        self.cursor = Some(cursor);
        self.load_id = self.load_id.wrapping_add(1);
        self.status = PlaybackStatus::Loading;
        self.position_ms = start_ms;
        self.source_label = None;
        let item = self.items[self.order[cursor]].clone();
        self.duration_ms = item.duration_ms.map(f64::from);
        if self.consecutive_errors == 0 {
            self.error = None;
        }
        vec![
            Effect::Load {
                load_id: self.load_id,
                item,
                start_ms,
                autoplay,
            },
            Effect::StateChanged,
        ]
    }

    fn set_status(&mut self, status: PlaybackStatus) -> Vec<Effect> {
        if self.status == status {
            return Vec::new();
        }
        self.status = status;
        vec![Effect::StateChanged]
    }

    fn position_in_order(&self, index: usize) -> usize {
        self.order
            .iter()
            .position(|&entry| entry == index)
            .expect("every item appears in the play order")
    }
}

#[cfg(test)]
mod tests;
