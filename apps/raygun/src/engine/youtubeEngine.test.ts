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
  const fromPage = (data: object, overrides: Partial<EmbedMessage> = {}) =>
    deliver({
      source: embedWindow,
      origin: ORIGIN,
      data: { raygunEmbed: true, ...data },
      ...overrides,
    });
  const load = (videoId = 'M7lc1UVf-VE') =>
    engine.handle({
      type: 'loadYouTube',
      loadId: 3,
      embedUrl: EMBED_URL,
      videoId,
      startMs: 0,
      autoplay: true,
      volume: 0.8,
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
    fromPage({ type: 'state', state: 'playing' }, { data: { type: 'state', state: 'playing' } });

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
});
