import {
  blend,
  decodeFlow,
  encodeFlow,
  flowAt,
  flowSize,
  isCut,
  missesRefreshes,
  progressAt,
  search,
  trust,
} from './motionMath';
import type { LumaImage } from './motionMath';

/** A textured picture (smooth waves, so linear taps between pixels read it well). */
const texture = (width: number, height: number, shiftX = 0, shiftY = 0): LumaImage => {
  const values = new Float32Array(width * height);
  for (let index = 0; index < values.length; index += 1) {
    const x = (index % width) - shiftX;
    const y = Math.floor(index / width) - shiftY;
    values[index] =
      0.5 + 0.2 * Math.sin(x * 0.45) * Math.sin(y * 0.37) + 0.15 * Math.sin(x * 0.13 + y * 0.21);
  }
  return { width, height, values };
};

describe('smooth motion as numbers', () => {
  it('keeps the flow in quarter pixels in eight bits, no motion exactly', () => {
    for (const flow of [-31.5, -7.25, 0.25, 3, 12.75, 31.5]) {
      expect(Math.abs(decodeFlow(encodeFlow(flow)) - flow)).toBeLessThan(0.13);
    }
    expect(decodeFlow(encodeFlow(0))).toBe(0);
    expect(decodeFlow(encodeFlow(80))).toBeCloseTo(31.87, 2);
  });

  it('finds a shift, and keeps a still picture still', () => {
    const prev = texture(48, 32);
    const cur = texture(48, 32, 2, -1);
    const match = search(prev, cur, { x: 24, y: 16 }, { x: 0, y: 0 }, 3, 1);
    expect(match.x).toBe(2);
    expect(match.y).toBe(-1);
    expect(match.cost).toBeLessThan(0.03);
    const still = search(prev, prev, { x: 24, y: 16 }, { x: 0, y: 0 }, 3, 1);
    expect([still.x, still.y]).toEqual([0, 0]);
  });

  it('follows larger motion coarse to fine, to half pixels', () => {
    const match = flowAt(texture(128, 96), texture(128, 96, 9.5, 4), 64, 48);
    expect(Math.abs(match.x - 9.5)).toBeLessThanOrEqual(0.51);
    expect(Math.abs(match.y - 4)).toBeLessThanOrEqual(0.51);
  });

  it('blends samples that agree and keeps the nearer one where they do not', () => {
    expect(blend(0.2, 0.7, 0.4, 0)).toBeCloseTo(0.4, 9);
    expect(blend(0.2, 0.7, 0.4, 0.5)).toBeCloseTo(0.2, 9);
    expect(blend(0.2, 0.7, 0.6, 0.5)).toBeCloseTo(0.7, 9);
    const some = blend(0.2, 0.7, 0.4, 0.1);
    expect(some).toBeGreaterThan(0.2);
    expect(some).toBeLessThan(0.4);
    expect(trust(0)).toBe(1);
    expect(trust(1)).toBe(0);
  });

  it('repeats the nearer frame at a cut', () => {
    expect(isCut(0.02, 0.05)).toBe(false);
    expect(isCut(0.2, 0.3)).toBe(true);
    expect(isCut(0.05, 0.6)).toBe(true);
  });

  it('places each refresh between the last two frames, a frame behind the video', () => {
    // 24 fps: the latest frame came 20.8 ms ago, half way from the one before.
    expect(progressAt(20.83, 1 / 24, 1)).toBeCloseTo(0.5, 2);
    expect(progressAt(0, 1 / 24, 1)).toBe(0);
    // Late for the next frame: the latest, held.
    expect(progressAt(80, 1 / 24, 1)).toBe(1);
    // Twice as fast: twice as far.
    expect(progressAt(10.42, 1 / 24, 2)).toBeCloseTo(0.5, 2);
    // A seek or a stall: no motion to follow.
    expect(progressAt(10, 0.6, 1)).toBeNull();
    expect(progressAt(10, 0, 1)).toBeNull();
  });

  it('sizes the flow by the picture', () => {
    expect(flowSize(1920, 1080)).toEqual({ width: 384, height: 216 });
    expect(flowSize(120, 68).width).toBe(120);
  });

  it('tells when refreshes are missed too often', () => {
    const steady = Array.from({ length: 120 }, () => 16.7);
    expect(missesRefreshes(steady)).toBe(false);
    const late = steady.map((interval, index) => (index % 3 === 0 ? 33.4 : interval));
    expect(missesRefreshes(late)).toBe(true);
    expect(missesRefreshes([16.7, 50, 50])).toBe(false);
  });
});
