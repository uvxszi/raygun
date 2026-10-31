import type { EngineReport } from '@raygun/bindings';

import { Html5Engine } from './html5Engine';
import type { MediaElementLike } from './html5Engine';

class FakeMedia extends EventTarget {
  src = '';
  currentTime = 0;
  volume = 1;
  duration = Number.NaN;
  error: { code: number } | null = null;
  play = vi.fn(() => Promise.resolve());
  pause = vi.fn();
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

const setup = () => {
  const media = new FakeMedia();
  const reports: EngineReport[] = [];
  let clock = 0;
  const engine = new Html5Engine(
    media as unknown as MediaElementLike,
    (report) => reports.push(report),
    () => clock,
  );
  return {
    media,
    reports,
    engine,
    advance: (milliseconds: number) => {
      clock += milliseconds;
    },
    load: (overrides: Partial<{ startMs: number; autoplay: boolean }> = {}) =>
      engine.handle({
        type: 'load',
        loadId: 7,
        url: 'http://127.0.0.1:9000/stream/abc',
        startMs: overrides.startMs ?? 0,
        autoplay: overrides.autoplay ?? true,
        volume: 0.5,
      }),
  };
};

describe('Html5Engine', () => {
  it('loads the stream and starts playing when asked to autoplay', () => {
    const { media, reports, load } = setup();

    load();

    expect(media.src).toBe('http://127.0.0.1:9000/stream/abc');
    expect(media.volume).toBe(0.5);
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
    const { media, engine, load } = setup();
    load();

    engine.handle({ type: 'seek', positionMs: 90_000 });
    engine.handle({ type: 'setVolume', volume: 0.2 });
    engine.handle({ type: 'pause' });

    expect(media.currentTime).toBe(90);
    expect(media.volume).toBe(0.2);
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
    media.play.mockRejectedValueOnce(new DOMException('blocked', 'NotAllowedError'));

    engine.handle({ type: 'play' });
    await vi.waitFor(() => expect(reports.at(-1)?.event.type).toBe('error'));

    expect(reports.filter((report) => report.event.type === 'error')).toHaveLength(1);
  });
});
