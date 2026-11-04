import type { EngineReport } from '@raygun/bindings';

import { YouTubeEmbedEngine } from './youtubeEngine';
import type { EmbedMessage } from './youtubeEngine';

const EMBED_URL = 'http://localhost:9000/embed/youtube';
const ORIGIN = 'http://localhost:9000';

const setup = () => {
  const embedWindow = { postMessage: vi.fn() };
  const frame = { src: '', contentWindow: embedWindow };
  let deliver: (message: EmbedMessage) => void = () => {};
  const reports: EngineReport[] = [];
  const engine = new YouTubeEmbedEngine(
    frame,
    (handler) => {
      deliver = handler;
      return () => {};
    },
    (report) => reports.push(report),
  );
  /** A reply from the page; it names load 3 (the test's load) unless `data` says otherwise. */
  const fromPage = (data: object, overrides: Partial<EmbedMessage> = {}) =>
    deliver({
      source: embedWindow,
      origin: ORIGIN,
      data: { raygunEmbed: true, token: 3, ...data },
      ...overrides,
    });
  const load = (
    videoId = 'M7lc1UVf-VE',
    extra: { gainDb?: number; endMs?: number | null; fadeMs?: number } = {},
  ) =>
    engine.handle({
      type: 'loadYouTube',
      loadId: 3,
      embedUrl: EMBED_URL,
      videoId,
      startMs: 0,
      autoplay: true,
      volume: 0.8,
      gainDb: extra.gainDb ?? 0,
      endMs: extra.endMs ?? null,
      fadeMs: extra.fadeMs ?? 0,
    });
  const posted = () => embedWindow.postMessage.mock.calls.map(([message]) => message);
  return { engine, frame, embedWindow, reports, fromPage, load, posted };
};

describe('YouTubeEmbedEngine', () => {
  it('opens the embed page and waits for it before sending commands', () => {
    const { frame, posted, load, fromPage } = setup();

    load();
    expect(frame.src).toBe(EMBED_URL);
    expect(posted()).toEqual([]);

    fromPage({ type: 'ready' });

    expect(posted()).toEqual([
      {
        raygunCommand: true,
        type: 'load',
        videoId: 'M7lc1UVf-VE',
        startMs: 0,
        autoplay: true,
        volume: 0.8,
        token: 3,
      },
    ]);
    expect(posted()).toHaveLength(1);
  });

  it('keeps only the latest load while the page is starting', () => {
    const { posted, load, fromPage } = setup();

    load('aaaaaaaaaaa');
    load('bbbbbbbbbbb');
    fromPage({ type: 'ready' });

    expect(posted()).toEqual([expect.objectContaining({ type: 'load', videoId: 'bbbbbbbbbbb' })]);
  });

  it('maps player states and progress to engine reports', () => {
    const { reports, load, fromPage } = setup();
    load();
    fromPage({ type: 'ready' });
    reports.length = 0;

    fromPage({ type: 'state', state: 'buffering' });
    fromPage({ type: 'state', state: 'playing' });
    fromPage({ type: 'progress', positionMs: 1500, durationMs: 213_000 });
    fromPage({ type: 'state', state: 'cued' });
    fromPage({ type: 'state', state: 'ended' });

    expect(reports.map((report) => report.event)).toEqual([
      { type: 'buffering' },
      { type: 'playing' },
      { type: 'progress', positionMs: 1500, durationMs: 213_000 },
      { type: 'ended' },
    ]);
    expect(reports.every((report) => report.loadId === 3)).toBe(true);
  });

  it("drops the page's late news about the previous video", () => {
    const { reports, load, fromPage } = setup();
    load();
    fromPage({ type: 'ready' });
    reports.length = 0;

    // Sent before the page loaded video 3: the old video's position and its end.
    fromPage({ type: 'progress', positionMs: 150_000, durationMs: 200_000, token: 2 });
    fromPage({ type: 'state', state: 'ended', token: 2 });
    fromPage({ type: 'error', code: 150, token: null });
    expect(reports).toEqual([]);

    fromPage({ type: 'progress', positionMs: 400, durationMs: 213_000 });
    expect(reports.map((report) => report.event)).toEqual([
      { type: 'progress', positionMs: 400, durationMs: 213_000 },
    ]);
  });

  it('explains YouTube error codes', () => {
    const { reports, load, fromPage } = setup();
    load();

    fromPage({ type: 'error', code: 150 });

    expect(reports.at(-1)?.event).toEqual({
      type: 'error',
      message: "The video's owner doesn't allow it to be played in other apps",
    });
  });

  it('ignores messages from other windows or origins', () => {
    const { reports, load, fromPage } = setup();
    load();
    reports.length = 0;

    fromPage({ type: 'state', state: 'playing' }, { source: {} });
    fromPage({ type: 'state', state: 'playing' }, { origin: 'https://evil.example' });
    fromPage(
      { type: 'state', state: 'playing' },
      { data: { type: 'state', state: 'playing', token: 3 } },
    );

    expect(reports).toEqual([]);
  });

  it('forwards controls once ready and goes quiet after stop', () => {
    const { engine, posted, reports, load, fromPage } = setup();
    load();
    fromPage({ type: 'ready' });

    engine.handle({ type: 'pause' });
    engine.handle({ type: 'seek', positionMs: 30_000 });
    engine.handle({ type: 'setVolume', volume: 0.25 });
    engine.handle({ type: 'stop' });
    reports.length = 0;
    fromPage({ type: 'state', state: 'paused' });

    expect(posted().slice(1)).toEqual([
      { raygunCommand: true, type: 'pause' },
      { raygunCommand: true, type: 'seek', positionMs: 30_000 },
      { raygunCommand: true, type: 'volume', volume: 0.25 },
      { raygunCommand: true, type: 'stop' },
    ]);
    expect(reports).toEqual([]);
  });

  it('reuses the loaded page for the next video', () => {
    const { frame, posted, load, fromPage } = setup();
    load('aaaaaaaaaaa');
    fromPage({ type: 'ready' });
    frame.src = EMBED_URL;

    load('bbbbbbbbbbb');

    expect(posted().at(-1)).toEqual(
      expect.objectContaining({ type: 'load', videoId: 'bbbbbbbbbbb' }),
    );
  });

  it("turns a song down by its gain, never up, and fades the embed's volume", () => {
    vi.useFakeTimers();
    const { engine, fromPage, load, posted } = setup();
    load('M7lc1UVf-VE', { gainDb: -6, fadeMs: 1000 });
    fromPage({ type: 'ready' });
    const volumes = () =>
      posted()
        .filter((message) => (message as { type: string }).type === 'volume')
        .map((message) => (message as { volume: number }).volume);
    vi.advanceTimersByTime(1000);
    // 0.8 × 10^(−6/20) ≈ 0.4, reached in ten steps from silence.
    expect(volumes()).toHaveLength(10);
    expect(volumes().at(-1)).toBeCloseTo(0.4, 2);

    engine.handle({ type: 'setTrackGain', gainDb: 6 });
    expect(volumes().at(-1)).toBeCloseTo(0.8, 5);

    engine.handle({ type: 'fadeOut', fadeMs: 500 });
    vi.advanceTimersByTime(500);
    expect(volumes().at(-1)).toBe(0);
    expect(posted().at(-1)).toEqual({ raygunCommand: true, type: 'stop' });
    vi.useRealTimers();
  });

  it('says paused when the embed sits unstarted after being asked to play', () => {
    vi.useFakeTimers();
    const { engine, reports, load, fromPage } = setup();
    load();
    fromPage({ type: 'ready' });
    reports.length = 0;

    // The webview held the start back until a tap: the player shows Play, not a dead Pause.
    fromPage({ type: 'state', state: 'unstarted' });
    vi.advanceTimersByTime(3000);
    expect(reports.map((report) => report.event)).toEqual([{ type: 'paused' }]);

    // Asked again (the listener's Play), a start that comes in time says nothing more.
    reports.length = 0;
    engine.handle({ type: 'play' });
    fromPage({ type: 'state', state: 'unstarted' });
    fromPage({ type: 'state', state: 'playing' });
    vi.advanceTimersByTime(5000);
    expect(reports.map((report) => report.event)).toEqual([{ type: 'playing' }]);
    vi.useRealTimers();
  });

  it('ends a video at its end trim', () => {
    const { fromPage, load, posted, reports } = setup();
    load('M7lc1UVf-VE', { endMs: 60_000 });
    fromPage({ type: 'ready' });
    reports.length = 0;

    fromPage({ type: 'progress', positionMs: 59_000, durationMs: 200_000 });
    fromPage({ type: 'progress', positionMs: 60_200, durationMs: 200_000 });
    fromPage({ type: 'state', state: 'paused' });

    expect(reports.map((report) => report.event.type)).toEqual(['progress', 'ended']);
    expect(posted()).toContainEqual({ raygunCommand: true, type: 'pause' });
  });
});
