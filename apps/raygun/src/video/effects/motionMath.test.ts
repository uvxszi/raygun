import {
  blend,
  decodeFlow,
  encodeFlow,
  flowAt,
  flowSize,
  interpolateFrame,
  isCut,
  missesRefreshes,
  modelNumber,
  MOTION_MODELS,
  motionPlan,
  progressAt,
  sample,
  search,
  trust,
  warpPixel,
} from './motionMath';
import type { FlowField, LumaImage } from './motionMath';

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

/**
 * A pan with an occluding object, as Android's test draws it: the background (waves) moves right
 * 3 px a frame, a bright square with a dark cross on it moves left 4 px over it. `time` 0 and 1
 * are two frames, 0.5 the true picture between them.
 */
/** Smooth noise without repeats (a hashed lattice, smoothly joined), the same as Android's test. */
const lattice = (column: number, row: number) =>
  (((Math.imul(column, 73_856_093) ^ Math.imul(row, 19_349_663)) >>> 0) % 1024) / 1023;
const valueNoise = (x: number, y: number) => {
  const left = Math.floor(x);
  const top = Math.floor(y);
  const ease = (value: number) => value * value * (3 - 2 * value);
  const fx = ease(x - left);
  const fy = ease(y - top);
  const upper = lattice(left, top) * (1 - fx) + lattice(left + 1, top) * fx;
  const lower = lattice(left, top + 1) * (1 - fx) + lattice(left + 1, top + 1) * fx;
  return upper * (1 - fy) + lower * fy;
};
const SCENE_WIDTH = 48;
const SCENE_HEIGHT = 32;
const scene = (width: number, height: number, time: number): LumaImage => {
  const values = new Float32Array(width * height);
  for (let index = 0; index < values.length; index += 1) {
    const x = index % width;
    const y = Math.floor(index / width);
    const bx = x - 3 * time;
    const background = 0.2 + 0.5 * valueNoise(bx / 3, y / 3);
    const left = 26 - 4 * time;
    const inSquare = x >= left && x < left + 10 && y >= 10 && y < 20;
    const cross = Math.abs(x - (left + 5)) < 1.5 || Math.abs(y - 15) < 1.5;
    values[index] = inSquare ? (cross ? 0.2 : 0.95) : background;
  }
  return { width, height, values };
};
const errors = (made: LumaImage, truth: LumaImage) =>
  Array.from(made.values, (value, index) => Math.abs(value - (truth.values[index] ?? 0)));
const meanError = (made: LumaImage, truth: LumaImage) =>
  errors(made, truth).reduce((sum, error) => sum + error, 0) / made.values.length;
/** The share of pixels more than 0.15 off. */
const farOff = (made: LumaImage, truth: LumaImage) =>
  errors(made, truth).filter((error) => error > 0.15).length / made.values.length;
/** The mean error in the three columns at each side. */
const borderError = (made: LumaImage, truth: LumaImage) => {
  let sum = 0;
  let count = 0;
  made.values.forEach((value, index) => {
    const x = index % made.width;
    if (x >= 3 && x < made.width - 3) return;
    sum += Math.abs(value - (truth.values[index] ?? 0));
    count += 1;
  });
  return sum / count;
};
/** The in-between frames' error against the truth (the same bounds as Android's test). */
const MOTION_ERROR_BOUND = 0.032;
const FAR_OFF_SHARE = 0.08;

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

  it('makes in-between frames near the truth round an occluding object, edges unstretched', () => {
    const [prev, mid, cur] = [0, 0.5, 1].map((time) => scene(SCENE_WIDTH, SCENE_HEIGHT, time)) as [
      LumaImage,
      LumaImage,
      LumaImage,
    ];
    const balanced = interpolateFrame(prev, cur, 0.5, 'balanced');
    const sharp = interpolateFrame(prev, cur, 0.5, 'sharp');
    const blended = interpolateFrame(prev, cur, 0.5, 'blend');
    // A plain cross-fade of the two: what doubles and smears.
    const crossFade: LumaImage = {
      ...prev,
      values: Float32Array.from(
        prev.values,
        (value, index) => (value + (cur.values[index] ?? 0)) / 2,
      ),
    };
    const balancedError = meanError(balanced, mid);
    expect(balancedError).toBeLessThan(MOTION_ERROR_BOUND);
    expect(meanError(sharp, mid)).toBeLessThan(MOTION_ERROR_BOUND);
    expect(balancedError).toBeLessThan(meanError(crossFade, mid) * 0.75);
    // No stretched streaks: few pixels far off, and the picture's edges (where new picture comes
    // in) as near the truth as the rest.
    expect(farOff(balanced, mid)).toBeLessThan(FAR_OFF_SHARE);
    expect(borderError(balanced, mid)).toBeLessThan(MOTION_ERROR_BOUND);
    // Blend is the cross-fade where little changed and the nearer frame where much did: never
    // beyond the two.
    blended.values.forEach((value, index) => {
      const nearer = cur.values[index] ?? 0;
      const faded = crossFade.values[index] ?? 0;
      expect(value).toBeGreaterThanOrEqual(Math.min(nearer, faded) - 1e-6);
      expect(value).toBeLessThanOrEqual(Math.max(nearer, faded) + 1e-6);
    });
  });

  it('leaves a side out where it falls outside the picture or on other motion', () => {
    const prev = texture(16, 8);
    const cur = texture(16, 8, 2, 0);
    const field: FlowField = {
      width: 16,
      height: 8,
      matches: Array.from({ length: 16 * 8 }, () => ({ x: 2, y: 0, cost: 0.01 })),
    };
    // At the left edge the frame before's sample lies outside: the next frame's alone.
    expect(warpPixel(prev, cur, field, 0, 4, 0.5, 'balanced')).toBeCloseTo(sample(cur, 1, 4), 9);
    // A badly matched block whose samples don't agree repeats the nearer frame; one whose do is
    // believed.
    const unsure: FlowField = {
      ...field,
      matches: field.matches.map((match) => ({ ...match, cost: 0.3 })),
    };
    const unlike: LumaImage = { width: 16, height: 8, values: new Float32Array(16 * 8).fill(0.05) };
    expect(warpPixel(prev, unlike, unsure, 8, 4, 0.4, 'balanced')).toBeCloseTo(
      sample(prev, 8, 4),
      9,
    );
    expect(warpPixel(prev, cur, unsure, 8, 4, 0.4, 'balanced')).toBeCloseTo(
      sample(prev, 7.2, 4),
      6,
    );
    // The same numbers as Android's (`PlayerEffectsMotionTest`).
    expect(warpPixel(prev, cur, field, 8, 4, 0.25, 'balanced')).toBeCloseTo(0.600335, 6);
    expect(warpPixel(prev, cur, null, 8, 4, 0.25, 'blend')).toBeCloseTo(0.554747, 6);
    expect(warpPixel(prev, cur, null, 8, 4, 0.25, 'blend')).toBeCloseTo(
      warpPixel(prev, cur, field, 8, 4, 0.25, 'blend'),
      9,
    );
  });

  it('plans each model’s search', () => {
    expect(motionPlan('blend')).toBeNull();
    expect(motionPlan('balanced')).toEqual({ radii: [3, 1, 1], refine: [0.5] });
    expect(motionPlan('sharp')).toEqual({ radii: [3, 2, 2], refine: [0.5, 0.25] });
    expect(MOTION_MODELS.map(modelNumber)).toEqual([0, 1, 2]);
  });

  it('tells when refreshes are missed too often', () => {
    const steady = Array.from({ length: 120 }, () => 16.7);
    expect(missesRefreshes(steady)).toBe(false);
    const late = steady.map((interval, index) => (index % 3 === 0 ? 33.4 : interval));
    expect(missesRefreshes(late)).toBe(true);
    expect(missesRefreshes([16.7, 50, 50])).toBe(false);
  });
});
