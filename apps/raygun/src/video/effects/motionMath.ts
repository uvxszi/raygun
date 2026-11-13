/**
 * Smooth motion's numbers on desktop (`glsl/fx_motion_*.glsl`; Android's are
 * `PlayerEffectsMotionMath.kt`, its tests checking the same): the flow's encoding, its search (the
 * same as the shader's, for tests), how in-between frames blend, where between two frames the
 * picture is at a screen refresh, and when the computer can't keep up.
 */

export const FLOW_RANGE = 64;
export const COST_RANGE = 0.5;
export const FLOW_ZERO = 128 / 255;
export const SEARCH_PENALTY = 0.004;
export const ZERO_BIAS = 0.003;
export const CUT_COST = 0.09;
export const CUT_SHARE = 0.45;
export const TRUST_FROM = 0.05;
export const TRUST_TO = 0.18;

/** The finest flow level's width: the desktop's GPUs keep up with the best of Android's. */
export const FLOW_WIDTH = 384;
/** Levels in each pyramid, finest first. */
export const LEVELS = 3;
/** Steps searched either way at each level, coarsest first; the finest is then refined by half steps. */
export const RADII = [3, 1, 1] as const;
/** A longer gap between two frames (in seconds) is no motion to follow. */
export const MAX_GAP = 0.25;

/** A value as an 8-bit channel keeps it. */
export const channel = (value: number) => Math.round(Math.min(1, Math.max(0, value)) * 255) / 255;

export const encodeFlow = (flow: number) => channel(flow / FLOW_RANGE + FLOW_ZERO);
export const decodeFlow = (stored: number) => (stored - FLOW_ZERO) * FLOW_RANGE;

/** A cut between scenes: the in-between frames repeat the nearer frame. */
export const isCut = (meanCost: number, badShare: number) =>
  meanCost > CUT_COST || badShare > CUT_SHARE;

const smoothstep = (edge0: number, edge1: number, value: number) => {
  const x = Math.min(1, Math.max(0, (value - edge0) / (edge1 - edge0)));
  return x * x * (3 - 2 * x);
};

/** How much of the blend a pixel gets: 1 where the two samples agree, 0 where they don't. */
export const trust = (differ: number) => 1 - smoothstep(TRUST_FROM, TRUST_TO, differ);

/** One channel of an in-between frame (`fx_motion_warp.glsl`). */
export const blend = (before: number, after: number, t: number, differ: number) => {
  const nearer = t < 0.5 ? before : after;
  const both = before + (after - before) * t;
  return nearer + (both - nearer) * trust(differ);
};

/**
 * Where the picture is between the frame before and the latest at a screen refresh: the latest
 * frame came `sinceMs` ago and follows the one before by `gap` seconds of media. The in-between
 * frames trail the video by one frame, so the latest is reached as the next one is due; `null`
 * when the gap is no motion to follow.
 */
export const progressAt = (sinceMs: number, gap: number, rate: number) => {
  if (!(gap > 0) || gap > MAX_GAP * Math.max(rate, 1)) return null;
  return Math.min(1, Math.max(0, ((sinceMs / 1000) * Math.max(rate, 0.1)) / gap));
};

/** A luma picture for checking the search, a value per pixel (row by row). */
export type LumaImage = { width: number; height: number; values: Float32Array | number[] };

const pixel = (image: LumaImage, x: number, y: number) =>
  image.values[
    Math.min(image.height - 1, Math.max(0, y)) * image.width +
      Math.min(image.width - 1, Math.max(0, x))
  ] ?? 0;

/** Linear between pixels (their centres at whole numbers), the edges repeated, as the GPU reads. */
export const sample = (image: LumaImage, x: number, y: number) => {
  const left = Math.floor(x);
  const top = Math.floor(y);
  const fx = x - left;
  const fy = y - top;
  const upper = pixel(image, left, top) * (1 - fx) + pixel(image, left + 1, top) * fx;
  const lower = pixel(image, left, top + 1) * (1 - fx) + pixel(image, left + 1, top + 1) * fx;
  return upper * (1 - fy) + lower * fy;
};

/** Half the size, each pixel the mean of four (`fx_motion_luma.glsl` from a level). */
export const half = (image: LumaImage): LumaImage => {
  const width = Math.max(1, Math.floor(image.width / 2));
  const height = Math.max(1, Math.floor(image.height / 2));
  const values = new Float32Array(width * height);
  for (let y = 0; y < height; y += 1) {
    for (let x = 0; x < width; x += 1) {
      values[y * width + x] =
        (pixel(image, x * 2, y * 2) +
          pixel(image, x * 2 + 1, y * 2) +
          pixel(image, x * 2, y * 2 + 1) +
          pixel(image, x * 2 + 1, y * 2 + 1)) /
        4;
    }
  }
  return { width, height, values };
};

export type FlowMatch = { x: number; y: number; cost: number };

/** The mean difference of 3 × 3 patches, the frame before at -flow/2 and the next at +flow/2. */
export const patchCost = (
  prev: LumaImage,
  cur: LumaImage,
  x: number,
  y: number,
  flowX: number,
  flowY: number,
) => {
  let sum = 0;
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      sum += Math.abs(
        sample(prev, x - flowX / 2 + dx, y - flowY / 2 + dy) -
          sample(cur, x + flowX / 2 + dx, y + flowY / 2 + dy),
      );
    }
  }
  return sum / 9;
};

/** The search at one pixel of a level (`fx_motion_search.glsl`), from `start`. */
export const search = (
  prev: LumaImage,
  cur: LumaImage,
  at: { x: number; y: number },
  start: { x: number; y: number },
  radius: number,
  step: number,
): FlowMatch => {
  let best: FlowMatch = {
    x: start.x,
    y: start.y,
    cost: patchCost(prev, cur, at.x, at.y, start.x, start.y),
  };
  for (let sy = -radius; sy <= radius; sy += 1) {
    for (let sx = -radius; sx <= radius; sx += 1) {
      if (sx === 0 && sy === 0) continue;
      const fx = start.x + sx * step;
      const fy = start.y + sy * step;
      const cost =
        patchCost(prev, cur, at.x, at.y, fx, fy) +
        SEARCH_PENALTY * Math.hypot(sx * step, sy * step);
      if (cost < best.cost) best = { x: fx, y: fy, cost };
    }
  }
  const still = patchCost(prev, cur, at.x, at.y, 0, 0);
  return still <= best.cost + ZERO_BIAS ? { x: 0, y: 0, cost: still } : best;
};

/** The flow at a pixel of the finest level, coarse to fine as the passes find it (without the median). */
export const flowAt = (prev: LumaImage, cur: LumaImage, x: number, y: number): FlowMatch => {
  const prevs = [prev];
  const curs = [cur];
  for (let level = 1; level < LEVELS; level += 1) {
    prevs.push(half(prevs[level - 1] as LumaImage));
    curs.push(half(curs[level - 1] as LumaImage));
  }
  let flow = { x: 0, y: 0 };
  for (let level = LEVELS - 1; level >= 0; level -= 1) {
    const scale = 2 ** level;
    const levelPrev = prevs[level] as LumaImage;
    const at = {
      x: Math.min(Math.floor(x / scale), levelPrev.width - 1),
      y: Math.min(Math.floor(y / scale), levelPrev.height - 1),
    };
    const radius = RADII[LEVELS - 1 - level] ?? 1;
    const match = search(
      levelPrev,
      curs[level] as LumaImage,
      at,
      {
        x: flow.x / scale,
        y: flow.y / scale,
      },
      radius,
      1,
    );
    flow = { x: match.x * scale, y: match.y * scale };
  }
  return search(prev, cur, { x, y }, flow, 1, 0.5);
};

/** The finest level's size for a frame. */
export const flowSize = (width: number, height: number) => {
  const flowWidth = Math.min(FLOW_WIDTH, Math.max(width, 16));
  return {
    width: flowWidth,
    height: Math.max(8, Math.round((flowWidth * height) / Math.max(width, 1))),
  };
};

/** A screen refresh much later than the usual one: the drawing missed a refresh. */
export const SLOW_REFRESH = 1.7;
/** The share of missed refreshes (and the fewest) that's too many for smooth motion. */
export const SLOW_SHARE = 0.2;
export const SLOW_LEAST = 10;

/** Whether refreshes came late too often while smooth motion drew (intervals in ms). */
export const missesRefreshes = (intervals: number[]) => {
  if (intervals.length < SLOW_LEAST) return false;
  const usual =
    [...intervals].sort((left, right) => left - right)[Math.floor(intervals.length / 4)] ?? 0;
  const late = intervals.filter((interval) => interval > usual * SLOW_REFRESH).length;
  return late >= SLOW_LEAST && late > intervals.length * SLOW_SHARE;
};
