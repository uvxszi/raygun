// The player on the web: Rust's PlaybackService in miniature. It runs the queue
// (`PlaybackCore`), finds what each song plays from, and drives the page's HTML5 and YouTube
// engines (engine/useEngineHost) with the same `EngineCommand`s and reports Rust sends and reads.
import type {
  EngineReport,
  PlayContext,
  PlaybackControl,
  QueueItem,
  QueueItemInput,
  RepeatMode,
  Settings,
  TrackInput,
} from '@raygun/bindings';

import { uuidV7Of } from './context';
import type { CoreContext } from './context';
import { PlaybackCore } from './playbackCore';
import type { Effect } from './playbackCore';

/** What a song plays from, once found. */
export type Found =
  | { kind: 'url'; url: string; onDevice?: boolean; label?: string | null }
  | { kind: 'youTube'; videoId: string }
  | { kind: 'error'; message: string };

/** Finds what a queued song plays from (sources, files on this device, YouTube). */
export type Finder = (item: QueueItem) => Promise<Found>;

const LAST_PLAYED = 'playback.lastPlayed';
/** The saved queue's place is written at most this often while a song plays. */
const SAVE_EVERY_MS = 5_000;

type LastPlayed = {
  items: QueueItemInput[];
  order: number[];
  current: number;
  positionMs: number;
  shuffle: boolean;
  repeat: RepeatMode;
  context: PlayContext | null;
};

/** When a load counts as a play for history (raygun-app history.rs). */
class PlayCounter {
  private loadId: number | null = null;
  private last: number | null = null;
  private listened = 0;
  private short = false;
  private counted = false;

  progress(loadId: number, positionMs: number, durationMs: number | null) {
    this.follow(loadId);
    if (this.last !== null) {
      const step = positionMs - this.last;
      if (step > 0 && step <= 3_000) this.listened += step;
    }
    this.last = positionMs;
    const duration = durationMs !== null && durationMs > 0 ? durationMs : null;
    if (duration !== null && duration < 30_000) {
      this.short = true;
      return false;
    }
    const needed = duration === null ? 240_000 : Math.min(duration / 2, 240_000);
    return this.count(this.listened >= needed);
  }

  ended(loadId: number) {
    this.follow(loadId);
    return this.count(this.short);
  }

  private follow(loadId: number) {
    if (this.loadId === loadId) return;
    this.loadId = loadId;
    this.last = null;
    this.listened = 0;
    this.short = false;
    this.counted = false;
  }

  private count(reached: boolean) {
    if (!reached || this.counted) return false;
    this.counted = true;
    return true;
  }
}

const inputOf = (item: QueueItem): QueueItemInput => ({
  explicit: item.explicit,
  title: item.title,
  artist: item.artist,
  album: item.album,
  durationMs: item.durationMs,
  artworkUrl: item.artworkUrl,
  mediaKind: item.mediaKind,
  source: item.source,
});

/** A song to queue: the stream it came with, else a lookup for the resolver. */
export const queueInputOf = (track: TrackInput): QueueItemInput => {
  const source = track.source ?? {
    kind: 'lookup' as const,
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
  };
  return {
    explicit: track.explicit,
    mediaKind: source.kind === 'youTube' ? 'video' : 'audio',
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
    artworkUrl: track.artworkUrl,
    source,
  };
};

export type PlaybackDeps = {
  find: Finder;
  settings: () => Settings;
  /** A song counted as played (history). */
  played: (item: QueueItem) => void;
  /** The YouTube embed host page's address. */
  youTubeEmbed: () => string;
};

export class PlaybackService {
  readonly core: PlaybackCore;
  private readonly plays = new PlayCounter();
  private armed: PlayContext | null = null;
  private sleepTimer: ReturnType<typeof setTimeout> | null = null;
  private watchTimer: ReturnType<typeof setInterval> | null = null;
  private lastSaved = 0;

  constructor(
    private readonly context: CoreContext,
    private readonly deps: PlaybackDeps,
  ) {
    const seed = BigInt(Math.floor(Math.random() * Number.MAX_SAFE_INTEGER)) + 1n;
    this.core = new PlaybackCore(seed, () => uuidV7Of(context));
    this.core.setCrossfade(deps.settings().sound.crossfadeMs ?? 0);
  }

  /** Starts the watchdog that catches an engine that stalls (once per page). */
  start() {
    if (this.watchTimer !== null) return;
    this.watchTimer = setInterval(() => this.apply(this.core.watchdog(performance.now())), 1_000);
  }

  stop() {
    if (this.watchTimer !== null) clearInterval(this.watchTimer);
    this.watchTimer = null;
    if (this.sleepTimer !== null) clearTimeout(this.sleepTimer);
  }

  /** Sound settings changed: the crossfade follows. */
  settingsChanged(settings: Settings) {
    this.core.setCrossfade(settings.sound.crossfadeMs ?? 0);
  }

  // ---- Commands ----

  enqueue(items: QueueItemInput[], playNow: boolean) {
    this.apply(this.core.enqueue(items, playNow));
  }

  enqueueNext(items: QueueItemInput[]) {
    this.apply(this.core.enqueueNext(items));
  }

  replace(items: QueueItemInput[], startIndex: number | null, shuffle = false) {
    const context = this.armed;
    this.armed = null;
    this.apply(this.core.replace(items, startIndex, shuffle));
    this.apply(this.core.setContext(context));
  }

  /** Where the next replace was played from (a page's play button arms it first). */
  arm(context: PlayContext | null) {
    this.armed = context;
  }

  control(control: PlaybackControl) {
    const core = this.core;
    switch (control.type) {
      case 'play':
        return this.apply(core.play());
      case 'pause':
        return this.apply(core.pause());
      case 'toggle':
        return this.apply(core.toggle());
      case 'next':
        return this.apply(core.next());
      case 'previous':
        return this.apply(core.previous());
      case 'seek':
        return this.apply(core.seek(control.positionMs ?? 0));
      case 'setVolume':
        return this.apply(core.setVolume(control.volume ?? 1));
      case 'setRepeat':
        return this.apply(core.setRepeat(control.mode));
      case 'setShuffle':
        return this.apply(core.setShuffle(control.enabled));
      case 'playItem':
        return this.apply(core.playItem(control.itemId));
      case 'remove':
        return this.apply(core.remove(control.itemId));
      case 'clear':
        return this.apply(core.clear());
      case 'clearUpcoming':
        return this.apply(core.clearUpcoming());
      case 'move':
        return this.apply(core.moveItem(control.itemId, control.beforeItemId));
      case 'setRate':
        return this.apply(core.setRate(control.rate ?? 1));
      case 'setSleepTimer':
        return this.setSleep(control.timer);
    }
  }

  private setSleep(timer: Extract<PlaybackControl, { type: 'setSleepTimer' }>['timer']) {
    if (this.sleepTimer !== null) clearTimeout(this.sleepTimer);
    this.sleepTimer = null;
    if (timer.kind === 'minutes') {
      const at = this.context.now() + timer.minutes * 60_000;
      this.sleepTimer = setTimeout(() => this.apply(this.core.sleepNow()), timer.minutes * 60_000);
      return this.apply(this.core.setSleep(at, false));
    }
    return this.apply(this.core.setSleep(null, timer.kind === 'endOfTrack'));
  }

  report(report: EngineReport) {
    const current = report.loadId === this.core.currentLoadId;
    const event = report.event;
    let counted = false;
    if (current && event.type === 'progress') {
      counted = this.plays.progress(report.loadId, event.positionMs ?? 0, event.durationMs);
    } else if (current && event.type === 'ended') {
      counted = this.plays.ended(report.loadId);
    }
    const played = counted ? this.core.currentItem() : null;
    // An end before the song ever played is a file that can't play, not the song's end.
    if (current && event.type === 'ended' && this.core.endsUnheard()) {
      this.apply(
        this.core.report({
          loadId: report.loadId,
          event: { type: 'error', message: 'It ended before it played' },
        }),
      );
    } else {
      this.apply(this.core.report(report));
    }
    if (played) this.deps.played(played);
  }

  /** Puts back what played when the page closed, paused. */
  restore() {
    if (!this.deps.settings().resumeOnStart || this.core.currentItem()) return;
    const saved = this.context.meta<LastPlayed>(LAST_PLAYED);
    if (!saved || saved.items.length === 0) return;
    this.apply(
      this.core.restore(
        saved.items,
        saved.order,
        saved.current,
        saved.positionMs,
        saved.shuffle,
        saved.repeat,
      ),
    );
    this.apply(this.core.setContext(saved.context));
  }

  // ---- Effects ----

  apply(effects: Effect[]) {
    let state = false;
    let queue = false;
    for (const effect of effects) {
      switch (effect.type) {
        case 'load':
          void this.load(effect);
          break;
        case 'engine':
          this.context.emit('engineCommandEvent', effect.command);
          break;
        case 'stateChanged':
          state = true;
          break;
        case 'queueChanged':
          queue = true;
          break;
      }
    }
    if (queue) this.context.emit('queueEvent', this.core.queue());
    if (state) this.context.emit('playbackStateEvent', this.core.state());
    if (queue || state) this.save(queue);
  }

  private async load(effect: Extract<Effect, { type: 'load' }>) {
    const { loadId, item, startMs, fadeMs } = effect;
    let found: Found;
    try {
      found = await this.deps.find(item);
    } catch (error) {
      found = { kind: 'error', message: error instanceof Error ? error.message : String(error) };
    }
    // A newer load took over while this one was found.
    if (loadId !== this.core.currentLoadId) return;
    const autoplay = this.core.playsWhenFound();
    const volume = this.core.state().volume;
    switch (found.kind) {
      case 'error':
        this.report({ loadId, event: { type: 'error', message: found.message } });
        return;
      case 'youTube':
        this.context.emit('engineCommandEvent', {
          type: 'loadYouTube',
          loadId,
          embedUrl: this.deps.youTubeEmbed(),
          videoId: found.videoId,
          startMs,
          autoplay,
          volume,
          gainDb: null,
          endMs: null,
          fadeMs,
        });
        break;
      case 'url':
        this.core.setOnDevice(loadId, found.onDevice === true);
        this.apply(this.core.setSourceLabel(loadId, found.label ?? null));
        this.context.emit('engineCommandEvent', {
          type: 'load',
          loadId,
          url: found.url,
          title: item.title,
          artist: item.artist,
          artworkUrl: item.artworkUrl,
          startMs,
          autoplay,
          volume,
          gainDb: null,
          endMs: null,
          fadeMs,
          measure: false,
        });
        break;
    }
    const rate = this.core.getRate();
    if (rate !== 1) this.context.emit('engineCommandEvent', { type: 'setRate', rate });
  }

  /** Keeps the queue for next time: at once when it changed, every few seconds as it plays. */
  private save(queueChanged: boolean) {
    const now = this.context.now();
    if (!queueChanged && now - this.lastSaved < SAVE_EVERY_MS) return;
    this.lastSaved = now;
    const queue = this.core.queue();
    const place = this.core.place();
    const state = this.core.state();
    if (queue.items.length === 0 || place.cursor === null) {
      if (queueChanged) this.context.setMeta(LAST_PLAYED, undefined);
      return;
    }
    const saved: LastPlayed = {
      items: queue.items.map(inputOf),
      order: queue.order,
      current: queue.order[place.cursor] ?? 0,
      positionMs: place.positionMs,
      shuffle: state.shuffle,
      repeat: state.repeat,
      context: queue.context ?? null,
    };
    this.context.setMeta(LAST_PLAYED, saved);
  }
}
