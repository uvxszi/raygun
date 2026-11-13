// Raygun's playback state machine for the web: the queue and the player's state, ported from
// raygun-playback (crates/raygun-playback/src/lib.rs) so queue rules are the same everywhere. It
// does no I/O: every operation returns the effects the caller carries out (load a song, forward a
// command to the engine, tell the page the state or queue changed).
import type {
  EngineCommand,
  EngineReport,
  PlayContext,
  PlaybackState,
  PlaybackStatus,
  QueueItem,
  QueueItemInput,
  QueueSnapshot,
  RepeatMode,
} from '@raygun/bindings';

/** `previous` restarts the current song instead of going back once this far in. */
const RESTART_THRESHOLD_MS = 3_000;
/** After this many failed items in a row playback stops instead of trying the rest. */
const MAX_SKIPPED_ERRORS = 3;
/** The position standing still this long (ms) while the engine says it plays is a stall. */
export const STALL_MS = 3_000;
/** An engine that has the stream but hasn't started this long (ms) after is held back. */
export const READY_STALL_MS = 3_000;
const MOVED_MS = 150;
const UNHEARD_MS = 1_000;

export type Effect =
  | {
      type: 'load';
      loadId: number;
      item: QueueItem;
      startMs: number;
      autoplay: boolean;
      fadeMs: number;
    }
  | { type: 'engine'; command: EngineCommand }
  | { type: 'stateChanged' }
  | { type: 'queueChanged' };

type Watch = {
  started: boolean;
  ready: boolean;
  positionMs: number;
  sinceMs: number | null;
  nudges: number;
  nudging: boolean;
};

const freshWatch = (): Watch => ({
  started: false,
  ready: false,
  positionMs: 0,
  sinceMs: null,
  nudges: 0,
  nudging: false,
});

const STATE_CHANGED: Effect = { type: 'stateChanged' };
const QUEUE_CHANGED: Effect = { type: 'queueChanged' };
const engine = (command: EngineCommand): Effect => ({ type: 'engine', command });

/** A small seeded generator for shuffle order, as the Rust core's xorshift. */
class XorShift {
  private value: bigint;
  constructor(seed: bigint) {
    this.value = seed === 0n ? 1n : BigInt.asUintN(64, seed);
  }
  private next() {
    let value = this.value;
    value = BigInt.asUintN(64, value ^ (value << 13n));
    value ^= value >> 7n;
    value = BigInt.asUintN(64, value ^ (value << 17n));
    this.value = value;
    return value;
  }
  below(bound: number) {
    return Number(this.next() % BigInt(bound));
  }
  shuffle<T>(items: T[]) {
    for (let last = items.length - 1; last >= 1; last -= 1) {
      const pick = this.below(last + 1);
      const held = items[last] as T;
      items[last] = items[pick] as T;
      items[pick] = held;
    }
  }
}

export class PlaybackCore {
  private items: QueueItem[] = [];
  /** Indices into `items` in play order. */
  private order: number[] = [];
  /** Position within `order` of the current item. */
  private cursor: number | null = null;
  private status: PlaybackStatus = 'idle';
  private positionMs = 0;
  private durationMs: number | null = null;
  private volume = 1;
  private repeat: RepeatMode = 'off';
  private shuffle = false;
  private error: string | null = null;
  private sourceLabel: string | null = null;
  private loadId = 0;
  private consecutiveErrors = 0;
  private readonly rng: XorShift;
  private rate = 1;
  private sleepAt: number | null = null;
  private stopAfterCurrent = false;
  private crossfadeMs = 0;
  private endMs: number | null = null;
  private held = false;
  private watch: Watch = freshWatch();
  private context: PlayContext | null = null;
  private looking = false;
  private skipped: string | null = null;
  private onDevice = false;

  constructor(
    seed: bigint,
    private readonly newId: () => string,
  ) {
    this.rng = new XorShift(seed);
  }

  get currentLoadId() {
    return this.loadId;
  }

  started() {
    return this.watch.started;
  }

  endsUnheard() {
    return this.cursor !== null && !this.watch.started && this.positionMs < UNHEARD_MS;
  }

  isHeld() {
    return this.held;
  }

  playsWhenFound() {
    return this.cursor !== null && !this.held;
  }

  setCrossfade(crossfadeMs: number) {
    this.crossfadeMs = Number.isFinite(crossfadeMs) ? Math.max(0, crossfadeMs) : 0;
  }

  setEnd(loadId: number, endMs: number | null) {
    if (loadId === this.loadId) this.endMs = endMs;
  }

  setOnDevice(loadId: number, onDevice: boolean) {
    if (loadId === this.loadId) this.onDevice = onDevice;
  }

  watchdog(nowMs: number): Effect[] {
    if (this.cursor === null || this.held) {
      this.watch.sinceMs = null;
      return [];
    }
    if (this.status === 'playing') return this.watchPlaying(nowMs);
    if ((this.status === 'loading' || this.status === 'buffering') && !this.watch.started) {
      return this.watchStart(nowMs);
    }
    this.watch.sinceMs = null;
    return [];
  }

  private watchPlaying(nowMs: number): Effect[] {
    const watch = this.watch;
    const moved = Math.abs(this.positionMs - watch.positionMs) > MOVED_MS;
    if (watch.sinceMs === null || moved) {
      watch.positionMs = this.positionMs;
      watch.sinceMs = nowMs;
      return [];
    }
    if (nowMs - watch.sinceMs < STALL_MS) return [];
    watch.sinceMs = null;
    return [
      engine({ type: 'seek', positionMs: this.positionMs }),
      engine({ type: 'play' }),
      ...this.setStatus('buffering'),
    ];
  }

  private watchStart(nowMs: number): Effect[] {
    const watch = this.watch;
    if (!watch.ready) {
      watch.sinceMs = null;
      return [];
    }
    if (watch.sinceMs === null) {
      watch.sinceMs = nowMs;
      return [];
    }
    if (nowMs - watch.sinceMs < READY_STALL_MS) return [];
    watch.sinceMs = nowMs;
    if (watch.nudges === 0) {
      watch.nudges = 1;
      watch.nudging = true;
      return [engine({ type: 'pause' }), engine({ type: 'play' })];
    }
    if (watch.nudges === 1) {
      watch.nudges = 2;
      watch.nudging = false;
      this.held = true;
      return [engine({ type: 'pause' }), ...this.setStatus('paused')];
    }
    return [];
  }

  private waiting() {
    if (this.cursor === null) return false;
    if (this.status === 'loading') return this.looking || (!this.onDevice && !this.watch.started);
    if (this.status === 'buffering') return this.looking || !this.onDevice;
    return false;
  }

  /** The items Next and Previous would load now, next first. */
  neighbors(): QueueItem[] {
    const cursor = this.cursor;
    if (cursor === null) return [];
    const count = this.order.length;
    const wraps = this.repeat === 'all' && count > 1;
    const next = cursor + 1 < count ? cursor + 1 : wraps ? 0 : null;
    const previous = cursor > 0 ? cursor - 1 : wraps ? count - 1 : null;
    const positions: number[] = [];
    for (const position of [next, previous]) {
      if (position !== null && position !== cursor && !positions.includes(position)) {
        positions.push(position);
      }
    }
    return positions.map((position) => this.itemAt(position));
  }

  state(): PlaybackState {
    return {
      status: this.status,
      currentItemId: this.currentItem()?.id ?? null,
      positionMs: this.positionMs,
      durationMs: this.durationMs,
      volume: this.volume,
      repeat: this.repeat,
      shuffle: this.shuffle,
      error: this.error,
      sourceLabel: this.sourceLabel,
      rate: this.rate,
      sleepAt: this.sleepAt,
      stopAfterCurrent: this.stopAfterCurrent,
      lookingForSource: this.looking,
      skippedItemId: this.skipped,
      waiting: this.waiting(),
    };
  }

  setRate(rate: number): Effect[] {
    if (!Number.isFinite(rate)) return [];
    this.rate = Math.min(2, Math.max(0.5, rate));
    return [engine({ type: 'setRate', rate: this.rate }), STATE_CHANGED];
  }

  getRate() {
    return this.rate;
  }

  setSleep(sleepAt: number | null, stopAfterCurrent: boolean): Effect[] {
    this.sleepAt = sleepAt;
    this.stopAfterCurrent = stopAfterCurrent;
    return [STATE_CHANGED];
  }

  sleepNow(): Effect[] {
    this.sleepAt = null;
    return [...this.pause(), STATE_CHANGED];
  }

  private newItem(input: QueueItemInput): QueueItem {
    return {
      explicit: input.explicit,
      id: this.newId(),
      title: input.title,
      artist: input.artist,
      album: input.album,
      durationMs: input.durationMs,
      artworkUrl: input.artworkUrl,
      mediaKind: input.mediaKind,
      source: input.source,
    };
  }

  /** Queues items right after the current one, in order (Play next). */
  enqueueNext(inputs: QueueItemInput[]): Effect[] {
    if (inputs.length === 0) return [];
    let slot = this.cursor === null ? 0 : this.cursor + 1;
    for (const input of inputs) {
      const index = this.items.length;
      this.items.push(this.newItem(input));
      this.order.splice(slot, 0, index);
      slot += 1;
    }
    return [QUEUE_CHANGED];
  }

  moveItem(itemId: string, beforeItemId: string | null): Effect[] {
    const index = this.items.findIndex((item) => item.id === itemId);
    if (index === -1) return [];
    const current = this.cursor === null ? null : this.order[this.cursor];
    const from = this.positionInOrder(index);
    this.order.splice(from, 1);
    const before =
      beforeItemId === null ? -1 : this.items.findIndex((item) => item.id === beforeItemId);
    const to = before !== -1 && before !== index ? this.positionInOrder(before) : this.order.length;
    this.order.splice(to, 0, index);
    this.cursor = current === null || current === undefined ? null : this.positionInOrder(current);
    return [QUEUE_CHANGED];
  }

  updateItems(update: (item: QueueItem) => boolean): Effect[] {
    let changed = false;
    for (const item of this.items) changed = update(item) || changed;
    return changed ? [QUEUE_CHANGED] : [];
  }

  setLooking(loadId: number, looking: boolean): Effect[] {
    if (loadId !== this.loadId || this.cursor === null || this.looking === looking) return [];
    this.looking = looking;
    if (looking) {
      this.watch = freshWatch();
      if (this.status === 'playing' || this.status === 'buffering') this.status = 'loading';
    }
    return [STATE_CHANGED];
  }

  setSourceLabel(loadId: number, label: string | null): Effect[] {
    if (loadId !== this.loadId || this.cursor === null || this.sourceLabel === label) return [];
    this.sourceLabel = label;
    return [STATE_CHANGED];
  }

  queue(): QueueSnapshot {
    return {
      items: this.items.map((item) => ({ ...item })),
      order: [...this.order],
      context: this.items.length > 0 ? this.context : null,
    };
  }

  setContext(context: PlayContext | null): Effect[] {
    if (JSON.stringify(this.context) === JSON.stringify(context)) return [];
    this.context = context;
    return [QUEUE_CHANGED];
  }

  getContext(): PlayContext | null {
    return this.items.length > 0 ? this.context : null;
  }

  currentItem(): QueueItem | null {
    return this.cursor === null ? null : this.itemAt(this.cursor);
  }

  /** Where the current item is in play order, and the order (for saving the queue). */
  place(): { cursor: number | null; order: number[]; positionMs: number } {
    return { cursor: this.cursor, order: [...this.order], positionMs: this.positionMs };
  }

  private itemAt(position: number): QueueItem {
    const index = this.order[position];
    const item = index === undefined ? undefined : this.items[index];
    if (!item) throw new Error('queue order out of step');
    return item;
  }

  enqueue(inputs: QueueItemInput[], playNow: boolean): Effect[] {
    if (inputs.length === 0) return [];
    const firstNew = this.items.length;
    if (firstNew === 0) this.context = null;
    for (const input of inputs) {
      const index = this.items.length;
      this.items.push(this.newItem(input));
      if (this.shuffle) {
        const earliest = this.cursor === null ? 0 : this.cursor + 1;
        const slot = earliest + this.rng.below(this.order.length - earliest + 1);
        this.order.splice(slot, 0, index);
      } else {
        this.order.push(index);
      }
    }
    const effects: Effect[] = [QUEUE_CHANGED];
    if (playNow) effects.push(...this.loadAt(this.positionInOrder(firstNew), 0, true));
    return effects;
  }

  replace(inputs: QueueItemInput[], startIndex: number | null, shuffle: boolean, startMs = 0) {
    if (inputs.length === 0) return this.clear();
    this.context = null;
    this.items = [];
    this.order = [];
    this.cursor = null;
    this.consecutiveErrors = 0;
    this.shuffle = false;
    this.enqueue(inputs, false);
    const count = this.items.length;
    const start =
      startIndex !== null ? Math.min(startIndex, count - 1) : shuffle ? this.rng.below(count) : 0;
    if (shuffle) {
      const rest: number[] = [];
      for (let index = 0; index < count; index += 1) if (index !== start) rest.push(index);
      this.rng.shuffle(rest);
      this.order = [start, ...rest];
      this.shuffle = true;
    }
    return [QUEUE_CHANGED, ...this.loadAt(this.positionInOrder(start), Math.max(0, startMs), true)];
  }

  /** Puts a saved queue back, loaded paused. */
  restore(
    inputs: QueueItemInput[],
    order: number[],
    current: number,
    positionMs: number,
    shuffle: boolean,
    repeat: RepeatMode,
  ): Effect[] {
    if (inputs.length === 0) return [];
    this.items = [];
    this.order = [];
    this.cursor = null;
    this.consecutiveErrors = 0;
    this.shuffle = false;
    this.enqueue(inputs, false);
    const count = this.items.length;
    const sorted = [...order].sort((a, b) => a - b);
    if (sorted.length === count && sorted.every((value, index) => value === index)) {
      this.order = [...order];
      this.shuffle = shuffle;
    }
    this.repeat = repeat;
    const cursor = this.positionInOrder(Math.min(current, count - 1));
    return [QUEUE_CHANGED, ...this.loadAt(cursor, Math.max(0, positionMs), false)];
  }

  play(): Effect[] {
    this.held = false;
    this.watch.sinceMs = null;
    this.watch.nudges = 0;
    const cursor = this.cursor;
    if (cursor === null) return this.items.length > 0 ? this.loadAt(0, 0, true) : [];
    if (this.status === 'ended') return this.loadAt(cursor, 0, true);
    if (this.status === 'error') return this.loadAt(cursor, this.positionMs, true);
    const effects: Effect[] = [engine({ type: 'play' })];
    if (this.status === 'paused')
      effects.push(...this.setStatus(this.onDevice ? 'playing' : 'loading'));
    return effects;
  }

  pause(): Effect[] {
    if (this.cursor === null) return [];
    this.held = true;
    const effects: Effect[] = [engine({ type: 'pause' })];
    if (this.status === 'playing' || this.status === 'loading' || this.status === 'buffering') {
      effects.push(...this.setStatus('paused'));
    }
    return effects;
  }

  toggle(): Effect[] {
    return this.status === 'playing' || this.status === 'loading' || this.status === 'buffering'
      ? this.pause()
      : this.play();
  }

  next(): Effect[] {
    const cursor = this.cursor;
    if (cursor === null) return this.items.length > 0 ? this.loadAt(0, 0, true) : [];
    if (cursor + 1 < this.order.length) return this.loadAt(cursor + 1, 0, true);
    if (this.repeat === 'all') return this.loadAt(0, 0, true);
    return [];
  }

  previous(): Effect[] {
    const cursor = this.cursor;
    if (cursor === null) return [];
    if (this.status === 'ended') return this.loadAt(cursor, 0, true);
    if (this.positionMs > RESTART_THRESHOLD_MS) return this.seek(0);
    if (cursor > 0) return this.loadAt(cursor - 1, 0, true);
    if (this.repeat === 'all' && this.order.length > 1) {
      return this.loadAt(this.order.length - 1, 0, true);
    }
    return this.seek(0);
  }

  seek(positionMs: number): Effect[] {
    if (this.cursor === null) return [];
    const upper = this.durationMs ?? Number.MAX_VALUE;
    this.positionMs = Math.min(upper, Math.max(0, positionMs));
    this.watch.sinceMs = null;
    return [engine({ type: 'seek', positionMs: this.positionMs }), STATE_CHANGED];
  }

  setVolume(volume: number): Effect[] {
    this.volume = Math.min(1, Math.max(0, volume));
    return [engine({ type: 'setVolume', volume: this.volume }), STATE_CHANGED];
  }

  setRepeat(mode: RepeatMode): Effect[] {
    this.repeat = mode;
    return [STATE_CHANGED];
  }

  setShuffle(enabled: boolean): Effect[] {
    if (enabled === this.shuffle) return [];
    this.shuffle = enabled;
    const current = this.cursor === null ? null : (this.order[this.cursor] ?? null);
    if (enabled) {
      const rest: number[] = [];
      for (let index = 0; index < this.items.length; index += 1) {
        if (index !== current) rest.push(index);
      }
      this.rng.shuffle(rest);
      this.order = current === null ? rest : [current, ...rest];
      this.cursor = current === null ? null : 0;
    } else {
      this.order = this.items.map((_, index) => index);
      this.cursor = current;
    }
    return [QUEUE_CHANGED, STATE_CHANGED];
  }

  playItem(itemId: string): Effect[] {
    const index = this.items.findIndex((item) => item.id === itemId);
    return index === -1 ? [] : this.loadAt(this.positionInOrder(index), 0, true);
  }

  remove(itemId: string): Effect[] {
    const index = this.items.findIndex((item) => item.id === itemId);
    if (index === -1) return [];
    const removedPosition = this.positionInOrder(index);
    const wasCurrent = this.cursor === removedPosition;
    const wasActive =
      this.status === 'playing' || this.status === 'loading' || this.status === 'buffering';
    this.items.splice(index, 1);
    this.order.splice(removedPosition, 1);
    this.order = this.order.map((entry) => (entry > index ? entry - 1 : entry));
    const effects: Effect[] = [QUEUE_CHANGED];
    const cursor = this.cursor;
    if (cursor !== null && removedPosition < cursor) {
      this.cursor = cursor - 1;
    } else if (cursor !== null && wasCurrent) {
      if (cursor < this.order.length) effects.push(...this.loadAt(cursor, 0, wasActive));
      else effects.push(...this.stop());
    }
    return effects;
  }

  clear(): Effect[] {
    this.context = null;
    this.items = [];
    this.order = [];
    return [QUEUE_CHANGED, ...this.stop()];
  }

  clearUpcoming(): Effect[] {
    const cursor = this.cursor;
    if (cursor === null) return this.items.length === 0 ? [] : this.clear();
    if (cursor + 1 >= this.order.length) return [];
    const kept = this.items.map(() => true);
    for (const index of this.order.splice(cursor + 1)) kept[index] = false;
    const movedTo: number[] = [];
    const items: QueueItem[] = [];
    this.items.forEach((item, index) => {
      if (!kept[index]) return;
      movedTo[index] = items.length;
      items.push(item);
    });
    this.items = items;
    this.order = this.order.map((entry) => movedTo[entry] ?? 0);
    return [QUEUE_CHANGED];
  }

  report(report: EngineReport): Effect[] {
    if (report.loadId !== this.loadId || this.cursor === null) return [];
    const event = report.event;
    switch (event.type) {
      case 'progress': {
        this.positionMs = event.positionMs ?? 0;
        if (event.durationMs !== null) this.durationMs = event.durationMs;
        this.watch.ready = true;
        return [STATE_CHANGED, ...this.crossfadeIfDue()];
      }
      case 'playing': {
        this.consecutiveErrors = 0;
        this.error = null;
        this.skipped = null;
        this.looking = false;
        this.watch.started = true;
        this.watch.nudging = false;
        if (this.held) return [engine({ type: 'pause' }), ...this.setStatus('paused')];
        return this.setStatus('playing');
      }
      case 'paused':
        return this.watch.nudging ? [] : this.setStatus('paused');
      case 'buffering':
        this.watch.ready = false;
        return this.held ? [] : this.setStatus('buffering');
      case 'ended':
        return this.onEnded();
      case 'error':
        return this.onError(event.message);
      case 'loudness':
        return [];
    }
  }

  private crossfadeIfDue(): Effect[] {
    const cursor = this.cursor;
    if (cursor === null) return [];
    if (
      this.crossfadeMs <= 0 ||
      this.status !== 'playing' ||
      this.repeat === 'one' ||
      this.stopAfterCurrent
    ) {
      return [];
    }
    const end = this.endMs ?? this.durationMs;
    if (end === null) return [];
    if (end <= this.crossfadeMs * 2 || end - this.positionMs > this.crossfadeMs) return [];
    let next: number;
    if (cursor + 1 < this.order.length) next = cursor + 1;
    else if (this.repeat === 'all' && this.order.length > 1) next = 0;
    else return [];
    const fadeMs = Math.min(this.crossfadeMs, Math.max(0, end - this.positionMs));
    return this.loadAt(next, 0, true).map((effect) =>
      effect.type === 'load' ? { ...effect, fadeMs } : effect,
    );
  }

  private onEnded(): Effect[] {
    const cursor = this.cursor as number;
    if (this.stopAfterCurrent) {
      this.stopAfterCurrent = false;
      if (cursor + 1 < this.order.length) return this.loadAt(cursor + 1, 0, false);
      this.status = 'ended';
      return [STATE_CHANGED];
    }
    if (this.repeat === 'one') return this.loadAt(cursor, 0, true);
    const effects = this.next();
    if (effects.length > 0) return effects;
    this.status = 'ended';
    if (this.durationMs !== null) this.positionMs = this.durationMs;
    return [STATE_CHANGED];
  }

  private onError(message: string): Effect[] {
    this.consecutiveErrors += 1;
    this.status = 'error';
    this.error = message;
    this.skipped = this.currentItem()?.id ?? null;
    this.looking = false;
    if (this.consecutiveErrors >= Math.min(this.items.length, MAX_SKIPPED_ERRORS)) {
      return [engine({ type: 'stop' }), STATE_CHANGED];
    }
    const effects = this.next();
    return effects.length === 0 ? [engine({ type: 'stop' }), STATE_CHANGED] : effects;
  }

  private stop(): Effect[] {
    this.cursor = null;
    this.status = 'idle';
    this.positionMs = 0;
    this.durationMs = null;
    this.error = null;
    this.sourceLabel = null;
    this.looking = false;
    this.onDevice = false;
    this.skipped = null;
    return [engine({ type: 'stop' }), STATE_CHANGED];
  }

  private loadAt(cursor: number, startMs: number, autoplay: boolean): Effect[] {
    this.cursor = cursor;
    this.held = !autoplay;
    this.loadId = (this.loadId + 1) >>> 0;
    this.status = autoplay ? 'loading' : 'paused';
    this.positionMs = startMs;
    this.sourceLabel = null;
    this.looking = false;
    this.onDevice = false;
    this.watch = freshWatch();
    const item = { ...this.itemAt(cursor) };
    this.durationMs = item.durationMs;
    this.endMs = null;
    if (this.consecutiveErrors === 0) {
      this.error = null;
      this.skipped = null;
    }
    return [
      { type: 'load', loadId: this.loadId, item, startMs, autoplay, fadeMs: 0 },
      STATE_CHANGED,
    ];
  }

  private setStatus(status: PlaybackStatus): Effect[] {
    if (this.status === status) return [];
    this.status = status;
    return [STATE_CHANGED];
  }

  private positionInOrder(index: number) {
    const position = this.order.indexOf(index);
    if (position === -1) throw new Error('every item is in the play order');
    return position;
  }
}
