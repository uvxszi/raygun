//! Raygun's playback state machine (ARCHITECTURE §6.2).
//!
//! [`PlaybackCore`] owns the queue and the canonical player state. It does no I/O: every
//! operation returns the [`Effect`]s the caller must carry out (load a stream into the active
//! engine, forward a command, broadcast new state). That keeps all queue rules unit-testable and
//! lets any engine (webview `<audio>`, YouTube embed, native player) sit behind the same protocol.

mod shuffle;

use raygun_model::{
    EngineCommand, EngineEvent, EngineReport, PlayContext, PlaybackState, PlaybackStatus,
    QueueItem, QueueItemInput, QueueSnapshot, RepeatMode,
};

use crate::shuffle::XorShift;

/// `previous` restarts the current track instead of going back once this far in.
const RESTART_THRESHOLD_MS: f64 = 3_000.0;
/// After this many failed items in a row playback stops instead of trying the rest of the queue
/// (a long list of search results could otherwise fail through dozens of items, M14).
const MAX_SKIPPED_ERRORS: usize = 3;
/// The position standing still this long (ms) while the engine says it plays is a stall.
pub const STALL_MS: f64 = 3_000.0;
/// An engine that has the stream but hasn't started this long (ms) after is held back.
pub const READY_STALL_MS: f64 = 3_000.0;
/// The position moving less than this (ms) isn't moving.
const MOVED_MS: f64 = 150.0;

/// What the watchdog knows of the current load ([`PlaybackCore::watchdog`]).
#[derive(Debug, Clone, Copy, Default, PartialEq)]
struct Watch {
    /// The engine reported `Playing` for this load.
    started: bool,
    /// The engine's latest report about the stream was progress: it has it, ready to play (a
    /// buffering report says it's still waiting for it).
    ready: bool,
    /// Where the position was when the current stretch began, and when (the caller's clock).
    position_ms: f64,
    since_ms: Option<f64>,
    /// Nudges in this stretch.
    nudges: u8,
    /// A nudge's own pause is on its way: the "paused" that follows isn't the listener's.
    nudging: bool,
}

/// Work the caller must do after an operation.
#[derive(Debug, Clone, PartialEq)]
pub enum Effect {
    /// Resolve `item`'s source to a playable URL and send `EngineCommand::Load` with this `load_id`.
    Load {
        load_id: u32,
        item: QueueItem,
        start_ms: f64,
        autoplay: bool,
        /// Fade from the song playing into this one over this long, ms (a crossfade, M46.12); 0 cuts.
        fade_ms: f64,
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
    /// See [`PlaybackState::rate`].
    rate: f64,
    /// See [`PlaybackState::sleep_at`]; the service's timer pauses then.
    sleep_at: Option<f64>,
    /// See [`PlaybackState::stop_after_current`].
    stop_after_current: bool,
    /// How long songs fade into the next, ms (M46.12); 0 waits for each to end.
    crossfade_ms: f64,
    /// Where the current load stops (its song's end trim), ms; set by the service.
    end_ms: Option<f64>,
    /// The user paused (M46.16): nothing plays until they play again, whatever an engine or a
    /// stream found late does.
    held: bool,
    watch: Watch,
    /// Where the queue was played from; see [`QueueSnapshot::context`].
    context: Option<PlayContext>,
    /// See [`PlaybackState::looking_for_source`]; set by the service.
    looking: bool,
    /// See [`PlaybackState::skipped_item_id`].
    skipped: Option<String>,
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
            rate: 1.0,
            sleep_at: None,
            stop_after_current: false,
            crossfade_ms: 0.0,
            end_ms: None,
            held: false,
            watch: Watch::default(),
            context: None,
            looking: false,
            skipped: None,
        }
    }

    /// Whether the engine has started the current load (reported it plays) since it was given.
    pub fn started(&self) -> bool {
        self.watch.started
    }

    /// Run about every second with a monotonic clock (ms). It catches an engine that doesn't
    /// play when the state says it does:
    /// - one that says it plays while the position stands still for [`STALL_MS`] is nudged (a
    ///   seek to where it is, then play), and the state says buffering until it moves again;
    /// - one that has the stream but hasn't started [`READY_STALL_MS`] later is nudged (pause,
    ///   then play: a player held back, say by audio focus, asks again), and if that doesn't start
    ///   it either, the state says paused, so the listener's next Play starts it (a webview that
    ///   wanted a tap first gets one).
    ///
    /// One still waiting for its stream is left to the service's start timeout; nothing that
    /// played and is buffering again is touched.
    pub fn watchdog(&mut self, now_ms: f64) -> Vec<Effect> {
        if self.cursor.is_none() || self.held {
            self.watch.since_ms = None;
            return Vec::new();
        }
        match self.status {
            PlaybackStatus::Playing => self.watch_playing(now_ms),
            PlaybackStatus::Loading | PlaybackStatus::Buffering if !self.watch.started => {
                self.watch_start(now_ms)
            }
            _ => {
                self.watch.since_ms = None;
                Vec::new()
            }
        }
    }

    fn watch_playing(&mut self, now_ms: f64) -> Vec<Effect> {
        let watch = &mut self.watch;
        let moved = (self.position_ms - watch.position_ms).abs() > MOVED_MS;
        match watch.since_ms {
            Some(since) if !moved => {
                if now_ms - since < STALL_MS {
                    return Vec::new();
                }
            }
            _ => {
                watch.position_ms = self.position_ms;
                watch.since_ms = Some(now_ms);
                return Vec::new();
            }
        }
        // It says it plays and doesn't: it's told again, and the state stops saying it plays.
        watch.since_ms = None;
        let mut effects = vec![
            Effect::Engine(EngineCommand::Seek {
                position_ms: self.position_ms,
            }),
            Effect::Engine(EngineCommand::Play),
        ];
        effects.extend(self.set_status(PlaybackStatus::Buffering));
        effects
    }

    fn watch_start(&mut self, now_ms: f64) -> Vec<Effect> {
        let watch = &mut self.watch;
        if !watch.ready {
            watch.since_ms = None;
            return Vec::new();
        }
        let Some(since) = watch.since_ms else {
            watch.since_ms = Some(now_ms);
            return Vec::new();
        };
        if now_ms - since < READY_STALL_MS {
            return Vec::new();
        }
        watch.since_ms = Some(now_ms);
        match watch.nudges {
            0 => {
                watch.nudges = 1;
                watch.nudging = true;
                vec![
                    Effect::Engine(EngineCommand::Pause),
                    Effect::Engine(EngineCommand::Play),
                ]
            }
            1 => {
                watch.nudges = 2;
                watch.nudging = false;
                self.held = true;
                let mut effects = vec![Effect::Engine(EngineCommand::Pause)];
                effects.extend(self.set_status(PlaybackStatus::Paused));
                effects
            }
            _ => Vec::new(),
        }
    }

    /// Whether the user paused: a stream found now loads without playing.
    pub fn held(&self) -> bool {
        self.held
    }

    /// Whether a stream found now for the current load starts at once. It follows the player as
    /// it stands, not what the load asked for when its search began: a song reopened paused and
    /// played while its stream is found starts once found (loaded paused, the engine would report
    /// paused and turn the press into a pause), and one paused meanwhile waits.
    pub fn plays_when_found(&self) -> bool {
        self.cursor.is_some() && !self.held
    }

    /// How long songs fade into the next (M46.12); 0 turns crossfading off.
    pub fn set_crossfade(&mut self, crossfade_ms: f64) {
        self.crossfade_ms = if crossfade_ms.is_finite() {
            crossfade_ms.max(0.0)
        } else {
            0.0
        };
    }

    /// Where the load `load_id` stops (its song's end trim, M46.12). Ignored for older loads.
    pub fn set_end(&mut self, load_id: u32, end_ms: Option<f64>) {
        if load_id == self.load_id {
            self.end_ms = end_ms;
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
            rate: self.rate,
            sleep_at: self.sleep_at,
            stop_after_current: self.stop_after_current,
            looking_for_source: self.looking,
            skipped_item_id: self.skipped.clone(),
        }
    }

    /// Playback speed, kept between 0.5 and 2 (M46.8). Engines keep it across loads, and the
    /// service sends it again after each load.
    pub fn set_rate(&mut self, rate: f64) -> Vec<Effect> {
        if !rate.is_finite() {
            return Vec::new();
        }
        self.rate = rate.clamp(0.5, 2.0);
        vec![
            Effect::Engine(EngineCommand::SetRate { rate: self.rate }),
            Effect::StateChanged,
        ]
    }

    pub fn rate(&self) -> f64 {
        self.rate
    }

    /// The sleep timer (M46.8): a time the service pauses at, or the end of the current song.
    pub fn set_sleep(&mut self, sleep_at: Option<f64>, stop_after_current: bool) -> Vec<Effect> {
        self.sleep_at = sleep_at;
        self.stop_after_current = stop_after_current;
        vec![Effect::StateChanged]
    }

    /// The timer's time came: pause, and forget it.
    pub fn sleep_now(&mut self) -> Vec<Effect> {
        self.sleep_at = None;
        let mut effects = self.pause();
        effects.push(Effect::StateChanged);
        effects
    }

    /// Queues items to play right after the current one, in the order given (Play next, M46.8);
    /// with nothing playing, at the front.
    pub fn enqueue_next(&mut self, inputs: Vec<QueueItemInput>) -> Vec<Effect> {
        if inputs.is_empty() {
            return Vec::new();
        }
        let first = self.cursor.map_or(0, |cursor| cursor + 1);
        for (slot, input) in (first..).zip(inputs) {
            let index = self.items.len();
            self.items.push(QueueItem {
                explicit: input.explicit,
                id: uuid::Uuid::now_v7().to_string(),
                title: input.title,
                artist: input.artist,
                album: input.album,
                duration_ms: input.duration_ms,
                artwork_url: input.artwork_url,
                media_kind: input.media_kind,
                source: input.source,
            });
            self.order.insert(slot, index);
        }
        vec![Effect::QueueChanged]
    }

    /// Moves an item in the play order: before `before_item_id`, or to the end (M46.8). The
    /// current item stays current.
    pub fn move_item(&mut self, item_id: &str, before_item_id: Option<&str>) -> Vec<Effect> {
        let Some(index) = self.items.iter().position(|item| item.id == item_id) else {
            return Vec::new();
        };
        let current = self.cursor.map(|cursor| self.order[cursor]);
        let from = self.position_in_order(index);
        self.order.remove(from);
        let to = match before_item_id
            .and_then(|before| self.items.iter().position(|item| item.id == before))
        {
            Some(before) if before != index => self.position_in_order(before),
            _ => self.order.len(),
        };
        self.order.insert(to, index);
        self.cursor = current.map(|current| self.position_in_order(current));
        vec![Effect::QueueChanged]
    }

    /// Changes queued items in place (a song given a source or names, M46.48): `update` says
    /// whether it changed an item. The current item keeps what it loaded until its next load.
    pub fn update_items(&mut self, mut update: impl FnMut(&mut QueueItem) -> bool) -> Vec<Effect> {
        let mut changed = false;
        for item in &mut self.items {
            changed |= update(item);
        }
        if changed {
            vec![Effect::QueueChanged]
        } else {
            Vec::new()
        }
    }

    /// The id of the latest `Effect::Load`; engine reports and async resolutions must match it.
    pub fn load_id(&self) -> u32 {
        self.load_id
    }

    /// Records where the current load's stream came from (the resolver's pick). Ignored when a
    /// newer load has started.
    /// What load `load_id` had didn't play, and the service looks for another stream (or stopped
    /// looking). While it looks the player says so and reads as loading (a pause stays a pause),
    /// and the next stream it loads is watched from its start, as a new load's is.
    pub fn set_looking(&mut self, load_id: u32, looking: bool) -> Vec<Effect> {
        if load_id != self.load_id || self.cursor.is_none() || self.looking == looking {
            return Vec::new();
        }
        self.looking = looking;
        if looking {
            self.watch = Watch::default();
            if matches!(
                self.status,
                PlaybackStatus::Playing | PlaybackStatus::Buffering
            ) {
                self.status = PlaybackStatus::Loading;
            }
        }
        vec![Effect::StateChanged]
    }

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
            context: self.context.clone().filter(|_| !self.items.is_empty()),
        }
    }

    /// Says where the queue was played from (an album, a playlist, search...). Set after a
    /// replace or restore; a replace without one, or a queue that empties, forgets it.
    pub fn set_context(&mut self, context: Option<PlayContext>) -> Vec<Effect> {
        if self.context == context {
            return Vec::new();
        }
        self.context = context;
        vec![Effect::QueueChanged]
    }

    pub fn context(&self) -> Option<&PlayContext> {
        self.context.as_ref().filter(|_| !self.items.is_empty())
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
        if first_new == 0 {
            // Songs added to an empty queue weren't played from anywhere in particular.
            self.context = None;
        }
        for input in inputs {
            let index = self.items.len();
            self.items.push(QueueItem {
                explicit: input.explicit,
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
        self.replace_at(inputs, start_index, shuffle, 0.0)
    }

    /// Like [`Self::replace`], starting the first item at `start_ms` (resuming an episode, M37).
    pub fn replace_at(
        &mut self,
        inputs: Vec<QueueItemInput>,
        start_index: Option<usize>,
        shuffle: bool,
        start_ms: f64,
    ) -> Vec<Effect> {
        if inputs.is_empty() {
            return self.clear();
        }
        self.context = None;
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
        effects.extend(self.load_at(cursor, start_ms.max(0.0), true));
        effects
    }

    /// Puts a saved queue back (M46.16, Raygun reopened): its play order, the song it was on and
    /// where, loaded paused. An order that doesn't fit the items falls back to queue order.
    pub fn restore(
        &mut self,
        inputs: Vec<QueueItemInput>,
        order: Vec<usize>,
        current: usize,
        position_ms: f64,
        shuffle: bool,
        repeat: RepeatMode,
    ) -> Vec<Effect> {
        if inputs.is_empty() {
            return Vec::new();
        }
        self.items.clear();
        self.order.clear();
        self.cursor = None;
        self.consecutive_errors = 0;
        self.shuffle = false;
        self.enqueue(inputs, false);
        let count = self.items.len();
        let mut sorted = order.clone();
        sorted.sort_unstable();
        if sorted == (0..count).collect::<Vec<_>>() {
            self.order = order;
            self.shuffle = shuffle;
        }
        self.repeat = repeat;
        let cursor = self.position_in_order(current.min(count - 1));
        let mut effects = vec![Effect::QueueChanged];
        effects.extend(self.load_at(cursor, position_ms.max(0.0), false));
        effects
    }

    pub fn play(&mut self) -> Vec<Effect> {
        self.held = false;
        // A new try: the watchdog waits for it as for a new load.
        self.watch.since_ms = None;
        self.watch.nudges = 0;
        match self.cursor {
            None if !self.items.is_empty() => self.load_at(0, 0.0, true),
            None => Vec::new(),
            Some(cursor) if self.status == PlaybackStatus::Ended => self.load_at(cursor, 0.0, true),
            Some(cursor) if self.status == PlaybackStatus::Error => {
                self.load_at(cursor, self.position_ms, true)
            }
            Some(_) => {
                // Answers at once (M46.16); the engine's report says when sound comes.
                let mut effects = vec![Effect::Engine(EngineCommand::Play)];
                if self.status == PlaybackStatus::Paused {
                    effects.extend(self.set_status(PlaybackStatus::Loading));
                }
                effects
            }
        }
    }

    /// Pauses at once (M46.16): the state says paused before the engine answers, and a song
    /// still being found, or an engine that starts anyway, stays paused.
    pub fn pause(&mut self) -> Vec<Effect> {
        if self.cursor.is_none() {
            return Vec::new();
        }
        self.held = true;
        let mut effects = vec![Effect::Engine(EngineCommand::Pause)];
        if matches!(
            self.status,
            PlaybackStatus::Playing | PlaybackStatus::Loading | PlaybackStatus::Buffering
        ) {
            effects.extend(self.set_status(PlaybackStatus::Paused));
        }
        effects
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
        self.watch.since_ms = None;
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
        self.context = None;
        self.items.clear();
        self.order.clear();
        let mut effects = vec![Effect::QueueChanged];
        effects.extend(self.stop());
        effects
    }

    /// Removes everything after the current item in play order (Up next's Clear) in one change,
    /// so a long queue doesn't empty one item and one queue broadcast at a time. The current
    /// item and those before it stay. With nothing current there's nothing to keep.
    pub fn clear_upcoming(&mut self) -> Vec<Effect> {
        let Some(cursor) = self.cursor else {
            return if self.items.is_empty() {
                Vec::new()
            } else {
                self.clear()
            };
        };
        if cursor + 1 >= self.order.len() {
            return Vec::new();
        }
        let mut kept = vec![true; self.items.len()];
        for index in self.order.split_off(cursor + 1) {
            kept[index] = false;
        }
        // Where each kept item ends up once the others are gone.
        let mut moved_to = vec![0; kept.len()];
        let mut items = Vec::with_capacity(self.order.len());
        for (index, item) in std::mem::take(&mut self.items).into_iter().enumerate() {
            if kept[index] {
                moved_to[index] = items.len();
                items.push(item);
            }
        }
        self.items = items;
        for entry in &mut self.order {
            *entry = moved_to[*entry];
        }
        vec![Effect::QueueChanged]
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
                self.watch.ready = true;
                let mut effects = vec![Effect::StateChanged];
                effects.extend(self.crossfade_if_due());
                effects
            }
            EngineEvent::Playing => {
                self.consecutive_errors = 0;
                self.error = None;
                self.skipped = None;
                self.looking = false;
                self.watch.started = true;
                self.watch.nudging = false;
                if self.held {
                    // It started after the user paused: pause it again.
                    let mut effects = vec![Effect::Engine(EngineCommand::Pause)];
                    effects.extend(self.set_status(PlaybackStatus::Paused));
                    return effects;
                }
                self.set_status(PlaybackStatus::Playing)
            }
            // The watchdog's own pause, on its way to playing again.
            EngineEvent::Paused if self.watch.nudging => Vec::new(),
            EngineEvent::Paused => self.set_status(PlaybackStatus::Paused),
            EngineEvent::Buffering if self.held => {
                self.watch.ready = false;
                Vec::new()
            }
            EngineEvent::Buffering => {
                self.watch.ready = false;
                self.set_status(PlaybackStatus::Buffering)
            }
            EngineEvent::Ended => self.on_ended(),
            EngineEvent::Error { message } => self.on_error(message),
            // The service stores it; the queue doesn't change.
            EngineEvent::Loudness { .. } => Vec::new(),
        }
    }

    /// Starts the next song early when the playing one is within the crossfade of its end (or end
    /// trim), so the engine fades one into the other (M46.12). Not with repeat one (the song
    /// restarts), the sleep timer's "end of song", or nothing next; nor for songs no longer than
    /// two crossfades, which would barely be heard.
    fn crossfade_if_due(&mut self) -> Vec<Effect> {
        let Some(cursor) = self.cursor else {
            return Vec::new();
        };
        if self.crossfade_ms <= 0.0
            || self.status != PlaybackStatus::Playing
            || self.repeat == RepeatMode::One
            || self.stop_after_current
        {
            return Vec::new();
        }
        let Some(end) = self.end_ms.or(self.duration_ms) else {
            return Vec::new();
        };
        if end <= self.crossfade_ms * 2.0 || end - self.position_ms > self.crossfade_ms {
            return Vec::new();
        }
        let next = if cursor + 1 < self.order.len() {
            cursor + 1
        } else if self.repeat == RepeatMode::All && self.order.len() > 1 {
            0
        } else {
            return Vec::new();
        };
        let fade_ms = (end - self.position_ms).clamp(0.0, self.crossfade_ms);
        let mut effects = self.load_at(next, 0.0, true);
        for effect in &mut effects {
            if let Effect::Load { fade_ms: fade, .. } = effect {
                *fade = fade_ms;
            }
        }
        effects
    }

    fn on_ended(&mut self) -> Vec<Effect> {
        let cursor = self.cursor.expect("report checked cursor");
        if self.stop_after_current {
            // The sleep timer's "end of song": the next one waits, paused, to be resumed.
            self.stop_after_current = false;
            if cursor + 1 < self.order.len() {
                return self.load_at(cursor + 1, 0.0, false);
            }
            self.status = PlaybackStatus::Ended;
            return vec![Effect::StateChanged];
        }
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
        self.skipped = self.current_item().map(|item| item.id.clone());
        self.looking = false;
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
        self.looking = false;
        self.skipped = None;
        vec![Effect::Engine(EngineCommand::Stop), Effect::StateChanged]
    }

    fn load_at(&mut self, cursor: usize, start_ms: f64, autoplay: bool) -> Vec<Effect> {
        self.cursor = Some(cursor);
        self.held = !autoplay;
        self.load_id = self.load_id.wrapping_add(1);
        // A song loaded to wait (reopening, the sleep timer's end of song) reads as paused: an
        // engine that loads paused has nothing to report.
        self.status = if autoplay {
            PlaybackStatus::Loading
        } else {
            PlaybackStatus::Paused
        };
        self.position_ms = start_ms;
        self.source_label = None;
        self.looking = false;
        self.watch = Watch::default();
        let item = self.items[self.order[cursor]].clone();
        self.duration_ms = item.duration_ms.map(f64::from);
        self.end_ms = None;
        if self.consecutive_errors == 0 {
            self.error = None;
            self.skipped = None;
        }
        vec![
            Effect::Load {
                load_id: self.load_id,
                item,
                start_ms,
                autoplay,
                fade_ms: 0.0,
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
