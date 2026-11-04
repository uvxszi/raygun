import type { EngineCommand, EngineReport } from '@raygun/bindings';

import { Html5Engine } from './html5Engine';
import type { MediaElementLike } from './html5Engine';
import { toLufs } from './loudness';
import type { DeckAudio, SoundBackend } from './soundGraph';

class FakeMedia extends EventTarget {
  src = '';
  currentTime = 0;
  volume = 1;
  playbackRate = 1;
  defaultPlaybackRate = 1;
  duration = Number.NaN;
  paused = true;
  error: { code: number } | null = null;
  play = vi.fn(() => {
    this.paused = false;
    return Promise.resolve();
  });
  pause = vi.fn(() => {
    this.paused = true;
    this.fire('pause');
  });
  load = vi.fn();
  removeAttribute = vi.fn((name: string) => {
    if (name === 'src') {
      this.src = '';
    }
  });

  fire(type: string) {
    this.dispatchEvent(new Event(type));
  }
}

/** Records what the engine asks of a deck's sound. */
class FakeDeckAudio implements DeckAudio {
  gain = 0;
  fade = 1;
  fades: [number, number][] = [];
  measured: number | null = null;
  setGain = (db: number) => {
    this.gain = db;
  };
  setFade = (level: number) => {
    this.fade = level;
  };
  fadeTo = (level: number, ms: number) => {
    this.fades.push([level, ms]);
    this.fade = level;
  };
  power = () => this.measured;
}

const fakeSound = () => {
  const decks: FakeDeckAudio[] = [];
  const backend: SoundBackend & { volume: number; applied: unknown[] } = {
    volume: 1,
    applied: [],
    deck: () => {
      const deck = new FakeDeckAudio();
      decks.push(deck);
      return deck;
    },
    apply: (sound) => backend.applied.push(sound),
    setVolume: (volume) => {
      backend.volume = volume;
    },
    resume: () => undefined,
  };
  return { backend, decks };
};

type LoadCommand = Extract<EngineCommand, { type: 'load' }>;

const setup = () => {
  const medias = [new FakeMedia(), new FakeMedia()] as const;
  const { backend, decks } = fakeSound();
  const reports: EngineReport[] = [];
  let clock = 0;
  const engine = new Html5Engine(
    medias as unknown as [MediaElementLike, MediaElementLike],
    (report) => reports.push(report),
    () => clock,
    backend,
  );
  const [media, other] = medias;
  return {
    media,
    other,
    decks,
    backend,
    reports,
    engine,
    advance: (milliseconds: number) => {
      clock += milliseconds;
    },
    load: (overrides: Partial<LoadCommand> = {}) =>
      engine.handle({
        type: 'load',
        loadId: 7,
        url: 'http://127.0.0.1:9000/stream/abc',
        title: 'Intro',
        artist: null,
        artworkUrl: null,
        startMs: 0,
        autoplay: true,
        volume: 0.5,
        gainDb: 0,
        endMs: null,
        fadeMs: 0,
        measure: false,
        ...overrides,
      }),
  };
};

describe('Html5Engine', () => {
  afterEach(() => {
    vi.useRealTimers();
  });

  it('loads the stream and starts playing when asked to autoplay', () => {
    const { media, backend, reports, load } = setup();

    load();

    expect(media.src).toBe('http://127.0.0.1:9000/stream/abc');
    expect(backend.volume).toBe(0.5);
    expect(media.play).toHaveBeenCalled();
    expect(reports).toEqual([{ loadId: 7, event: { type: 'buffering' } }]);
  });

  it('does not start playing without autoplay', () => {
    const { media, load } = setup();

    load({ autoplay: false });

    expect(media.play).not.toHaveBeenCalled();
  });

  it('reports what the media element does, tagged with the load id', () => {
    const { media, reports, load } = setup();
    load();
    reports.length = 0;

    media.fire('playing');
    media.fire('pause');
    media.fire('ended');

    expect(reports).toEqual([
      { loadId: 7, event: { type: 'playing' } },
      { loadId: 7, event: { type: 'paused' } },
      { loadId: 7, event: { type: 'ended' } },
    ]);
  });

  it('throttles progress reports and converts to milliseconds', () => {
    const { media, reports, load, advance } = setup();
    load();
    reports.length = 0;
    media.duration = 200;

    media.currentTime = 1;
    media.fire('timeupdate');
    advance(100);
    media.currentTime = 1.1;
    media.fire('timeupdate');
    advance(200);
    media.currentTime = 1.3;
    media.fire('timeupdate');

    expect(reports).toEqual([
      { loadId: 7, event: { type: 'progress', positionMs: 1000, durationMs: 200_000 } },
      { loadId: 7, event: { type: 'progress', positionMs: 1300, durationMs: 200_000 } },
    ]);
  });

  it('reports an unknown duration for live streams', () => {
    const { media, reports, load } = setup();
    load();
    media.duration = Number.POSITIVE_INFINITY;

    media.fire('durationchange');

    expect(reports.at(-1)?.event).toEqual({ type: 'progress', positionMs: 0, durationMs: null });
  });

  it('resumes from the start position once metadata is known', () => {
    const { media, load } = setup();
    load({ startMs: 42_000 });

    media.fire('loadedmetadata');

    expect(media.currentTime).toBe(42);
  });

  it('executes seek, volume and pause commands', () => {
    const { media, backend, engine, load } = setup();
    load();

    engine.handle({ type: 'seek', positionMs: 90_000 });
    engine.handle({ type: 'setVolume', volume: 0.2 });
    engine.handle({ type: 'pause' });

    expect(media.currentTime).toBe(90);
    expect(backend.volume).toBe(0.2);
    expect(media.pause).toHaveBeenCalled();
  });

  it('goes silent after stop', () => {
    const { media, reports, engine, load } = setup();
    load();
    reports.length = 0;

    engine.handle({ type: 'stop' });
    media.fire('pause');

    expect(media.src).toBe('');
    expect(media.load).toHaveBeenCalled();
    expect(reports).toEqual([]);
  });

  it('explains media errors', () => {
    const { media, reports, load } = setup();
    load();
    media.error = { code: 4 };

    media.fire('error');

    expect(reports.at(-1)?.event).toEqual({
      type: 'error',
      message: 'The media format is not supported or the source is unreachable',
    });
  });

  it('reports a rejected play() but ignores interrupted ones', async () => {
    const { media, reports, engine, load } = setup();
    media.play.mockRejectedValueOnce(new DOMException('interrupted', 'AbortError'));
    load();
    media.play.mockRejectedValueOnce(new DOMException('no source', 'NotSupportedError'));

    engine.handle({ type: 'play' });
    await vi.waitFor(() => expect(reports.at(-1)?.event.type).toBe('error'));

    expect(reports.filter((report) => report.event.type === 'error')).toHaveLength(1);
  });

  it('waits paused when the webview wants a tap first, instead of failing the song', async () => {
    const { media, reports, load } = setup();
    media.play.mockRejectedValueOnce(new DOMException('blocked', 'NotAllowedError'));
    load();

    await vi.waitFor(() => expect(reports.at(-1)?.event.type).toBe('paused'));
    expect(reports.some((report) => report.event.type === 'error')).toBe(false);
  });

  it('keeps the speed for the sources that follow', () => {
    const { media, other, engine, load } = setup();
    load();

    engine.handle({ type: 'setRate', rate: 1.5 });

    for (const element of [media, other]) {
      expect(element.playbackRate).toBe(1.5);
      expect(element.defaultPlaybackRate).toBe(1.5);
    }
  });

  it('crossfades: the next song fades in on the other deck while this one fades out', () => {
    vi.useFakeTimers();
    const { media, other, decks, reports, load } = setup();
    load({ gainDb: -3 });
    media.fire('playing');

    load({ loadId: 8, url: 'http://127.0.0.1:9000/stream/next', fadeMs: 4000, gainDb: 2 });

    expect(other.src).toBe('http://127.0.0.1:9000/stream/next');
    expect(other.play).toHaveBeenCalled();
    expect(decks[1]?.gain).toBe(2);
    expect(decks[1]?.fades).toEqual([[1, 4000]]);
    expect(decks[0]?.fades).toEqual([[0, 4000]]);
    // The old song plays on under the new one, then stops; it says nothing more.
    expect(media.src).not.toBe('');
    reports.length = 0;
    media.fire('ended');
    vi.advanceTimersByTime(4100);
    expect(media.src).toBe('');
    expect(reports).toEqual([]);
  });

  it('cuts without a fade, and when nothing was playing', () => {
    const { media, other, decks, load } = setup();
    load({ autoplay: false });
    load({ loadId: 8, fadeMs: 4000 });
    // The first song never played: nothing to fade from.
    expect(media.src).toBe('');
    expect(decks[1]?.fade).toBe(1);
    expect(decks[1]?.fades).toEqual([]);
    other.fire('playing');
    load({ loadId: 9 });
    expect(other.src).toBe('');
  });

  it('ends a song at its end trim, without reporting the pause that stops it', () => {
    const { media, reports, load } = setup();
    load({ endMs: 30_000 });
    media.fire('playing');
    reports.length = 0;

    media.currentTime = 30;
    media.fire('timeupdate');

    expect(reports).toEqual([{ loadId: 7, event: { type: 'ended' } }]);
    expect(media.pause).toHaveBeenCalled();
  });

  it('waits for an end trim set while the song plays', () => {
    vi.useFakeTimers();
    const { media, engine, reports, load } = setup();
    load();
    media.currentTime = 10;
    media.fire('playing');
    engine.handle({ type: 'setEnd', endMs: 12_000 });
    reports.length = 0;

    media.currentTime = 12;
    vi.advanceTimersByTime(2000);

    expect(reports).toEqual([{ loadId: 7, event: { type: 'ended' } }]);
  });

  it('measures loudness, levels the song live and reports it', () => {
    vi.useFakeTimers();
    const { media, engine, decks, reports, load } = setup();
    engine.handle({
      type: 'setEffects',
      sound: { normalize: true, normalizeTarget: -14 },
    });
    load({ measure: true, gainDb: 1 });
    media.fire('playing');
    const deck = decks[0];
    if (!deck) throw new Error('no deck');
    // A loud song: −8 LUFS.
    deck.measured = 10 ** ((-8 + 0.691) / 10);

    vi.advanceTimersByTime(5_000);
    // 6 dB down from the song's own +1 dB.
    expect(deck.gain).toBeCloseTo(1 - 6, 1);
    expect(reports.some((report) => report.event.type === 'loudness')).toBe(false);

    vi.advanceTimersByTime(45_000);
    const loudness = reports.find((report) => report.event.type === 'loudness');
    expect(loudness?.event).toEqual({ type: 'loudness', lufs: expect.closeTo(-8, 1) as number });
    expect(toLufs(deck.measured)).toBeCloseTo(-8, 3);
  });

  it('runs its loudness timer only while a song plays', () => {
    vi.useFakeTimers();
    const { media, engine, load } = setup();
    // Idle: no timer wakes the page.
    expect(engine.metering).toBe(false);
    load({ measure: true });
    media.fire('playing');
    expect(engine.metering).toBe(true);

    engine.handle({ type: 'pause' });
    vi.advanceTimersByTime(200);
    expect(engine.metering).toBe(false);
    expect(vi.getTimerCount()).toBe(0);
  });

  it('passes effects to the sound graph and changes the playing song’s gain', () => {
    const { engine, backend, decks, load } = setup();
    load({ gainDb: -2 });

    engine.handle({ type: 'setEffects', sound: { eqEnabled: true } });
    engine.handle({ type: 'setTrackGain', gainDb: 4 });

    expect(backend.applied).toEqual([{ eqEnabled: true }]);
    expect(decks[0]?.gain).toBe(4);
  });
});
