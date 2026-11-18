import {
  capOf,
  LONG_STALL_MS,
  lowerStep,
  pictureHeight,
  raiseStep,
  SETTLE_MS,
  SMOOTH_MS,
  StallWatch,
} from './autoQuality';
import type { QualityRow } from './protocol';

/** Ticks every 250 ms from `from` to `to`, with what `stalled` says at each; the actions seen. */
const run = (
  watch: StallWatch,
  from: number,
  to: number,
  stalled: (now: number) => boolean,
  canRaise = false,
) => {
  const actions: [number, string][] = [];
  for (let now = from; now <= to; now += 250) {
    const action = watch.tick(now, true, stalled(now), canRaise);
    if (action) {
      actions.push([now, action]);
      watch.stepped(now);
    }
  }
  return actions;
};

describe('stalls and automatic quality', () => {
  it('steps down after two stalls within 30 s', () => {
    const watch = new StallWatch();
    watch.reset(0);
    // Stalls at 20 s and 35 s (a second each): the second one steps down.
    const stalled = (now: number) =>
      (now >= 20_000 && now < 21_000) || (now >= 35_000 && now < 36_000);
    expect(run(watch, 0, 60_000, stalled)).toEqual([[35_000, 'lower']]);
  });

  it('lets stalls far apart be, and steps down for one long stall once', () => {
    const watch = new StallWatch();
    watch.reset(0);
    const apart = (now: number) =>
      (now >= 10_000 && now < 11_000) || (now >= 50_000 && now < 51_000);
    expect(run(watch, 0, 60_000, apart)).toEqual([]);
    const fresh = new StallWatch();
    fresh.reset(0);
    const long = (now: number) => now >= 20_000 && now < 30_000;
    expect(run(fresh, 0, 40_000, long)).toEqual([[20_000 + LONG_STALL_MS, 'lower']]);
  });

  it('doesn’t count stalls while a new quality settles in', () => {
    const watch = new StallWatch();
    watch.reset(0);
    const early = (now: number) => now < SETTLE_MS && now % 2_000 < 1_000;
    expect(run(watch, 0, SETTLE_MS + 5_000, early)).toEqual([]);
  });

  it('steps back up only after a long smooth stretch, when it can', () => {
    const watch = new StallWatch();
    watch.reset(0);
    expect(run(watch, 0, SMOOTH_MS + 1_000, () => false)).toEqual([]);
    const lowered = new StallWatch();
    lowered.reset(0);
    expect(run(lowered, 0, SMOOTH_MS + 1_000, () => false, true)).toEqual([[SMOOTH_MS, 'raise']]);
    // A stall starts the stretch again.
    const again = new StallWatch();
    again.reset(0);
    const once = (now: number) => now >= 60_000 && now < 60_500;
    expect(run(again, 0, SMOOTH_MS + 1_000, once, true)).toEqual([]);
  });
});

const row = (height: number, state: QualityRow['state'], extra: Partial<QualityRow> = {}) =>
  ({
    key: `${height}:sdr`,
    height,
    range: 'sdr',
    codec: null,
    state,
    ...(state === 'thisSource' || state === 'otherSource'
      ? { group: 0, link: height, source: 'Beta' }
      : {}),
    ...extra,
  }) as QualityRow;

describe('the step down and back up', () => {
  it('steps an adaptive stream down among its own variants, never below the lowest', () => {
    const rows = [
      row(1080, 'playing', { preferred: true }),
      row(2160, 'otherSource'),
      row(720, 'variant', { sizes: [{ width: 1280, height: 720 }] }),
      row(480, 'variant'),
      row(360, 'thisSource'),
    ];
    expect(lowerStep(rows, true)?.height).toBe(720);
    expect(lowerStep([row(480, 'playing'), row(360, 'thisSource')], true)).toBeNull();
  });

  it('steps a single link down to another link’s lower step', () => {
    const rows = [row(1080, 'playing'), row(720, 'otherSource'), row(480, 'thisSource')];
    expect(lowerStep(rows, false)?.height).toBe(720);
    expect(lowerStep([row(480, 'playing')], false)).toBeNull();
    expect(lowerStep([row(720, 'variant')], false)).toBeNull();
  });

  it('caps by a row’s pictures and lifts a step at a time', () => {
    expect(pictureHeight(1920, 800)).toBe(1080);
    expect(pictureHeight(1280, 720)).toBe(720);
    expect(capOf(row(1080, 'variant', { sizes: [{ width: 1920, height: 816 }] }))).toBe(1080);
    const rows = [row(1080, 'variant'), row(720, 'variant'), row(480, 'playing')];
    expect(raiseStep(rows, 480)?.height).toBe(720);
    expect(raiseStep(rows, 1080)).toBeNull();
  });
});
