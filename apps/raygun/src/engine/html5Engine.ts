import type { EngineCommand, EngineEvent, EngineReport } from '@raygun/bindings';

import { LoudnessMeter } from './loudness';
import { elementSound } from './soundGraph';
import type { DeckAudio, SoundBackend } from './soundGraph';

/** The parts of HTMLMediaElement the engine uses, so tests can supply a fake. */
export type MediaElementLike = Pick<
  HTMLMediaElement,
  | 'src'
  | 'currentTime'
  | 'volume'
  | 'playbackRate'
  | 'defaultPlaybackRate'
  | 'duration'
  | 'error'
  | 'paused'
  | 'play'
  | 'pause'
  | 'load'
  | 'removeAttribute'
  | 'addEventListener'
  | 'removeEventListener'
>;

const PROGRESS_INTERVAL_MS = 250;
/** How often loudness is sampled, ms (one 100 ms step of the meter). */
const METER_STEP_MS = 100;
/** A song measured this long reports its loudness, s ... */
const MEASURED_ENOUGH_S = 45;
/** ... and one that ends sooner does if it played at least this long. */
const MEASURED_AT_LEAST_S = 10;
/** Live levelling starts after this much sound, s, and follows the meter every second. */
const LEVEL_AFTER_S = 3;
/** Live levelling's limits, dB: never boost a quiet song more, nor cut a loud one more. */
const LEVEL_BOOST_DB = 10;
const LEVEL_CUT_DB = 20;

const MEDIA_ERROR_MESSAGES: Record<number, string> = {
  1: 'Playback was aborted',
  2: 'A network error interrupted playback',
  3: 'The media could not be decoded',
  4: 'The media format is not supported or the source is unreachable',
};

/** One of the two players: a crossfade plays both, the old one fading out. */
type Deck = {
  media: MediaElementLike;
  audio: DeckAudio;
  /** The load it plays, or null once it's stopped or fading out. */
  loadId: number | null;
  pendingStartMs: number;
  /** Its song's end trim, ms. */
  endMs: number | null;
  endTimer: ReturnType<typeof setTimeout> | null;
  /** Stops it once its fade-out is over. */
  stopTimer: ReturnType<typeof setTimeout> | null;
  /** Its song's gain from Rust (volume and known loudness), and the live levelling on top. */
  gainDb: number;
  liveDb: number;
  measure: boolean;
  meter: LoudnessMeter;
  /** It reached its end trim: the pause that follows isn't the listener's. */
  finished: boolean;
};

/**
 * Plays streams in webview media elements on behalf of the Rust playback service
 * (ARCHITECTURE §6.2). It executes engine commands and reports what the current element does,
 * tagging every report with the current load id so the service can drop reports about replaced
 * tracks. Two elements take turns (M46.12): a crossfade starts the new one while the old one
 * fades out. Sound goes through `backend`: Web Audio's graph with the equalizer and effects, or
 * the bare elements.
 */
export class Html5Engine {
  private loadId: number | null = null;
  private lastProgressAt = Number.NEGATIVE_INFINITY;
  private readonly decks: [Deck, Deck];
  /** The deck playing now; loads alternate, so the first song goes to the first deck. */
  private current = 1;
  private normalize = false;
  private target = -14;
  private steps = 0;
  /** Measures loudness while a song plays; stopped otherwise, so an idle engine never wakes. */
  private meterTimer: ReturnType<typeof setInterval> | null = null;
  private readonly cleanups: (() => void)[] = [];

  constructor(
    medias: [MediaElementLike, MediaElementLike],
    private readonly report: (report: EngineReport) => void,
    private readonly now: () => number = () => Date.now(),
    private readonly backend: SoundBackend = elementSound(),
  ) {
    this.decks = [this.makeDeck(medias[0]), this.makeDeck(medias[1])];
  }

  /** Whether the loudness meter's timer runs (tests). */
  get metering() {
    return this.meterTimer !== null;
  }

  private startMeter() {
    this.meterTimer ??= setInterval(() => this.measure(), METER_STEP_MS);
  }

  private stopMeter() {
    if (this.meterTimer === null) return;
    clearInterval(this.meterTimer);
    this.meterTimer = null;
  }

  private makeDeck(media: MediaElementLike): Deck {
    const deck: Deck = {
      media,
      audio: this.backend.deck(media as HTMLMediaElement),
      loadId: null,
      pendingStartMs: 0,
      endMs: null,
      endTimer: null,
      stopTimer: null,
      gainDb: 0,
      liveDb: 0,
      measure: false,
      meter: new LoudnessMeter(),
      finished: false,
    };
    const listeners: [string, () => void][] = [
      ['loadedmetadata', () => this.onLoadedMetadata(deck)],
      ['durationchange', () => this.sendProgress(deck, true)],
      ['timeupdate', () => this.sendProgress(deck, false)],
      ['seeked', () => this.onSeeked(deck)],
      [
        'playing',
        () => {
          // A context still asleep would play it without a sound.
          this.backend.resume();
          this.startMeter();
          this.scheduleEnd(deck);
          this.emitFrom(deck, { type: 'playing' });
        },
      ],
      [
        'pause',
        () => {
          this.clearEnd(deck);
          if (!deck.finished) this.emitFrom(deck, { type: 'paused' });
        },
      ],
      ['waiting', () => this.emitFrom(deck, { type: 'buffering' })],
      ['ended', () => this.finish(deck, false)],
      ['error', () => this.onError(deck)],
    ];
    for (const [type, listener] of listeners) {
      media.addEventListener(type, listener);
      this.cleanups.push(() => media.removeEventListener(type, listener));
    }
    return deck;
  }

  private get deck(): Deck {
    return this.decks[this.current] as Deck;
  }

  handle(command: EngineCommand) {
    switch (command.type) {
      case 'load':
        this.load(command);
        break;
      case 'play':
        this.backend.resume();
        this.play(this.deck);
        break;
      case 'pause':
        // A song still fading out stops with the one fading in.
        this.retire(this.decks[1 - this.current] as Deck, 0);
        this.deck.media.pause();
        break;
      case 'seek':
        this.deck.media.currentTime = (command.positionMs ?? 0) / 1000;
        break;
      case 'setVolume':
        this.backend.setVolume(command.volume ?? 1);
        break;
      case 'setRate':
        // The default survives new sources, which reset the rate to it (M46.8).
        for (const deck of this.decks) {
          deck.media.defaultPlaybackRate = command.rate ?? 1;
          deck.media.playbackRate = command.rate ?? 1;
        }
        this.scheduleEnd(this.deck);
        break;
      case 'setEffects':
        this.backend.apply(command.sound);
        this.normalize = command.sound.normalize ?? false;
        this.target = command.sound.normalizeTarget ?? -14;
        break;
      case 'setTrackGain':
        this.deck.gainDb = command.gainDb ?? 0;
        this.deck.audio.setGain(this.deck.gainDb + this.deck.liveDb, 300);
        break;
      case 'setEnd':
        this.deck.endMs = command.endMs ?? null;
        this.scheduleEnd(this.deck);
        break;
      case 'fadeOut':
        // Another engine (YouTube's embed) takes over; this song fades away under it.
        this.loadId = null;
        this.retire(this.deck, command.fadeMs ?? 0);
        break;
      case 'stop':
        this.loadId = null;
        for (const deck of this.decks) this.retire(deck, 0);
        break;
      case 'loadYouTube':
        break;
    }
  }

  dispose() {
    this.stopMeter();
    for (const deck of this.decks) {
      this.clearEnd(deck);
      if (deck.stopTimer) clearTimeout(deck.stopTimer);
    }
    for (const cleanup of this.cleanups) cleanup();
  }

  private load(command: Extract<EngineCommand, { type: 'load' }>) {
    const previous = this.deck;
    const fadeMs = command.fadeMs ?? 0;
    const fading = fadeMs > 0 && previous.loadId !== null && !previous.media.paused;
    this.retire(previous, fading ? fadeMs : 0);
    this.current = 1 - this.current;
    const deck = this.deck;
    this.clearEnd(deck);
    if (deck.stopTimer) clearTimeout(deck.stopTimer);
    deck.stopTimer = null;
    deck.loadId = command.loadId;
    deck.pendingStartMs = command.startMs ?? 0;
    deck.endMs = command.endMs ?? null;
    deck.gainDb = command.gainDb ?? 0;
    deck.liveDb = 0;
    deck.measure = command.measure;
    deck.meter = new LoudnessMeter();
    deck.finished = false;
    deck.audio.setGain(deck.gainDb);
    deck.audio.setFade(fading ? 0 : 1);
    this.loadId = command.loadId;
    this.lastProgressAt = Number.NEGATIVE_INFINITY;
    this.backend.setVolume(command.volume ?? 1);
    deck.media.src = command.url;
    this.emitFrom(deck, { type: 'buffering' });
    if (command.autoplay) {
      this.backend.resume();
      this.play(deck);
    }
    if (fading) deck.audio.fadeTo(1, fadeMs);
  }

  /** Lets a deck go: fades it out over `fadeMs`, then stops it (at once for 0). */
  private retire(deck: Deck, fadeMs: number) {
    deck.loadId = null;
    this.clearEnd(deck);
    if (deck.stopTimer) clearTimeout(deck.stopTimer);
    deck.stopTimer = null;
    const stop = () => {
      deck.stopTimer = null;
      deck.media.pause();
      deck.media.removeAttribute('src');
      deck.media.load();
    };
    if (fadeMs > 0 && !deck.media.paused) {
      deck.audio.fadeTo(0, fadeMs);
      deck.stopTimer = setTimeout(stop, fadeMs + 50);
    } else if (deck.media.src) {
      stop();
    }
  }

  private play(deck: Deck) {
    const loadId = deck.loadId;
    deck.media.play().catch((error: unknown) => {
      // A newer load or pause interrupting play() is normal, not a failure.
      if (error instanceof DOMException && error.name === 'AbortError') {
        return;
      }
      if (loadId === null || loadId !== deck.loadId) return;
      // The webview wants a tap before it plays: the song is fine, and waits paused, so the
      // player shows Play instead of a Pause that does nothing (and doesn't skip the song).
      if (error instanceof DOMException && error.name === 'NotAllowedError') {
        this.emitFrom(deck, { type: 'paused' });
        return;
      }
      this.emitFrom(deck, { type: 'error', message: String(error) });
    });
  }

  private onLoadedMetadata(deck: Deck) {
    if (deck.pendingStartMs > 0) {
      deck.media.currentTime = deck.pendingStartMs / 1000;
      deck.pendingStartMs = 0;
    }
    this.sendProgress(deck, true);
  }

  private onSeeked(deck: Deck) {
    this.scheduleEnd(deck);
    this.sendProgress(deck, true);
  }

  private onError(deck: Deck) {
    const code = deck.media.error?.code ?? 0;
    this.emitFrom(deck, {
      type: 'error',
      message: MEDIA_ERROR_MESSAGES[code] ?? 'Playback failed',
    });
  }

  /** Waits for the song's end trim, while it plays. */
  private scheduleEnd(deck: Deck) {
    this.clearEnd(deck);
    if (deck.endMs === null || deck.loadId === null || deck.media.paused) return;
    const rate = deck.media.playbackRate || 1;
    const remaining = (deck.endMs - deck.media.currentTime * 1000) / rate;
    if (remaining <= 0) {
      this.finish(deck, true);
      return;
    }
    deck.endTimer = setTimeout(() => this.scheduleEnd(deck), remaining);
  }

  private clearEnd(deck: Deck) {
    if (deck.endTimer) clearTimeout(deck.endTimer);
    deck.endTimer = null;
  }

  /** The song is over, at its end or its end trim: report what was measured, then the end. */
  private finish(deck: Deck, trimmed: boolean) {
    if (deck.loadId === null || deck.finished) return;
    deck.finished = true;
    this.clearEnd(deck);
    if (deck.measure && deck.meter.seconds >= MEASURED_AT_LEAST_S) this.reportLoudness(deck);
    if (trimmed) deck.media.pause();
    this.emitFrom(deck, { type: 'ended' });
  }

  /** Samples the playing deck's loudness, levels it live, and reports it once it's known. */
  private measure() {
    const deck = this.deck;
    if (this.decks.every((each) => each.media.paused)) {
      // Nothing plays: the timer starts again with the next song that does.
      this.stopMeter();
      return;
    }
    if (!deck.measure || deck.loadId === null || deck.media.paused) return;
    const power = deck.audio.power();
    if (power === null) return;
    deck.meter.add(power);
    this.steps += 1;
    if (this.normalize && deck.meter.seconds >= LEVEL_AFTER_S && this.steps % 10 === 0) {
      const lufs = deck.meter.integrated();
      if (lufs !== null) {
        deck.liveDb = Math.max(-LEVEL_CUT_DB, Math.min(LEVEL_BOOST_DB, this.target - lufs));
        deck.audio.setGain(deck.gainDb + deck.liveDb, 1500);
      }
    }
    if (deck.meter.seconds >= MEASURED_ENOUGH_S) this.reportLoudness(deck);
  }

  private reportLoudness(deck: Deck) {
    deck.measure = false;
    const lufs = deck.meter.integrated();
    if (lufs !== null) this.emitFrom(deck, { type: 'loudness', lufs });
  }

  private sendProgress(deck: Deck, force: boolean) {
    if (deck !== this.deck) return;
    const position = deck.media.currentTime * 1000;
    if (deck.endMs !== null && position >= deck.endMs) {
      this.finish(deck, true);
      return;
    }
    const now = this.now();
    if (!force && now - this.lastProgressAt < PROGRESS_INTERVAL_MS) {
      return;
    }
    this.lastProgressAt = now;
    const duration = deck.media.duration;
    this.emitFrom(deck, {
      type: 'progress',
      positionMs: position,
      durationMs: Number.isFinite(duration) ? duration * 1000 : null,
    });
  }

  /** Reports for the current deck's load; a deck fading out says nothing. */
  private emitFrom(deck: Deck, event: EngineEvent) {
    if (deck === this.deck && deck.loadId !== null && deck.loadId === this.loadId) {
      this.report({ loadId: deck.loadId, event });
    }
  }
}
