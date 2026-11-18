/**
 * Smooth motion's numbers on desktop (`motion/fx_motion_*.glsl`, the Smooth Motion chip's shaders; Android's are
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

/** A patch with fewer taps inside both frames than this matches nothing (`NO_MATCH`). */
export const FEWEST_TAPS = 3;
export const NO_MATCH = 1;

/** Whether a point (in pixels, centres at whole numbers) lies on the picture. */
export const onPicture = (image: LumaImage, x: number, y: number) =>
  x >= -0.5 && y >= -0.5 && x <= image.width - 0.5 && y <= image.height - 0.5;

/**
 * The mean difference of 3 × 3 patches, the frame before at -flow/2 and the next at +flow/2; taps
 * outside either frame don't count.
 */
export const patchCost = (
  prev: LumaImage,
  cur: LumaImage,
  x: number,
  y: number,
  flowX: number,
  flowY: number,
) => {
  let sum = 0;
  let taps = 0;
  for (let dy = -1; dy <= 1; dy += 1) {
    for (let dx = -1; dx <= 1; dx += 1) {
      const bx = x - flowX / 2 + dx;
      const by = y - flowY / 2 + dy;
      const ax = x + flowX / 2 + dx;
      const ay = y + flowY / 2 + dy;
      if (!onPicture(prev, bx, by) || !onPicture(cur, ax, ay)) continue;
      sum += Math.abs(sample(prev, bx, by) - sample(cur, ax, ay));
      taps += 1;
    }
  }
  return taps < FEWEST_TAPS ? NO_MATCH : sum / taps;
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
export const flowAt = (prev: LumaImage, cur: LumaImage, x: number, y: number): FlowMatch =>
  matchAt(flowField(prev, cur, { radii: [...RADII], refine: [0.5] }), x, y);

// ---- The models, the steadied flow and the in-between frames (fx_motion_smooth/warp.glsl) ----

/** Smooth motion's models (`MotionModel` in raygun-model), as `uModel` numbers them. */
export type MotionModel = 'blend' | 'balanced' | 'sharp';
export const MOTION_MODELS: MotionModel[] = ['blend', 'balanced', 'sharp'];
export const modelNumber = (model: MotionModel) => MOTION_MODELS.indexOf(model);

/** What each model searches: radii coarsest first, then the refinement steps (none: no flow). */
export type MotionPlan = { radii: number[]; refine: number[] } | null;
export const motionPlan = (model: MotionModel): MotionPlan =>
  model === 'blend'
    ? null
    : model === 'sharp'
      ? { radii: [3, 2, 2], refine: [0.5, 0.25] }
      : { radii: [...RADII], refine: [0.5] };

export const MAX_MOTION = 40;
export const EDGE_SIGMA = 0.06;
export const VECTOR_SIGMA = 2;
export const COST_SIGMA = 0.08;
export const SPREAD_WEIGHT = 0.02;
export const BLEND_STILL = 0.03;
export const UNSURE_FROM = 0.1;
export const UNSURE_TO = 0.2;
export const AGREE_FROM = 1;
export const AGREE_TO = 3;

/** A flow field on the CPU, a vector and cost per pixel of the flow's level (row by row). */
export type FlowField = { width: number; height: number; matches: FlowMatch[] };

/** A field's vector between its pixels (centres at whole numbers), as the GPU reads it linearly. */
export const fieldSample = (field: FlowField, x: number, y: number) => {
  const left = Math.floor(x);
  const top = Math.floor(y);
  const fx = x - left;
  const fy = y - top;
  const corner = (cx: number, cy: number) => matchAt(field, cx, cy);
  const lerp = (key: 'x' | 'y') =>
    (corner(left, top)[key] * (1 - fx) + corner(left + 1, top)[key] * fx) * (1 - fy) +
    (corner(left, top + 1)[key] * (1 - fx) + corner(left + 1, top + 1)[key] * fx) * fy;
  return { x: lerp('x'), y: lerp('y') };
};

/**
 * One search pass over a level (`fx_motion_search.glsl`): each pixel starts from the best of the
 * coarser field's vector there and at its four neighbours (a coarse texel away) and no motion,
 * then searches round it. `init` is in the finest level's pixels; `scale` this level's size in them.
 */
export const searchLevel = (
  prev: LumaImage,
  cur: LumaImage,
  init: FlowField | null,
  scale: number,
  radius: number,
  step: number,
): FlowField => {
  const matches: FlowMatch[] = [];
  for (let y = 0; y < prev.height; y += 1) {
    for (let x = 0; x < prev.width; x += 1) {
      let start = { x: 0, y: 0 };
      let startCost = patchCost(prev, cur, x, y, 0, 0);
      if (init) {
        // This pixel's place in the init field (a coarser level, or this level's own refinement).
        const ratio = init.width / prev.width;
        const ix = (x + 0.5) * ratio - 0.5;
        const iy = (y + 0.5) * ratio - 0.5;
        for (const [dx, dy] of [
          [0, 0],
          [-1, 0],
          [1, 0],
          [0, -1],
          [0, 1],
        ] as const) {
          // Two of this level's pixels away: a coarser level's neighbour.
          const found = fieldSample(init, ix + dx * 2 * ratio, iy + dy * 2 * ratio);
          const candidate = { x: found.x / scale, y: found.y / scale };
          const cost = patchCost(prev, cur, x, y, candidate.x, candidate.y);
          if (cost < startCost) {
            start = candidate;
            startCost = cost;
          }
        }
      }
      let best: FlowMatch = { ...start, cost: startCost };
      for (let sy = -radius; sy <= radius; sy += 1) {
        for (let sx = -radius; sx <= radius; sx += 1) {
          if (sx === 0 && sy === 0) continue;
          const fx = start.x + sx * step;
          const fy = start.y + sy * step;
          const cost =
            patchCost(prev, cur, x, y, fx, fy) + SEARCH_PENALTY * Math.hypot(sx * step, sy * step);
          if (cost < best.cost) best = { x: fx, y: fy, cost };
        }
      }
      const still = patchCost(prev, cur, x, y, 0, 0);
      if (still <= best.cost + ZERO_BIAS) best = { x: 0, y: 0, cost: still };
      matches.push({ x: best.x * scale, y: best.y * scale, cost: best.cost });
    }
  }
  return { width: prev.width, height: prev.height, matches };
};

/** The whole field as the search passes find it: coarse to fine, then the model's refinements. */
export const flowField = (
  prev: LumaImage,
  cur: LumaImage,
  plan: NonNullable<MotionPlan>,
): FlowField => {
  const prevs = [prev];
  const curs = [cur];
  for (let level = 1; level < LEVELS; level += 1) {
    prevs.push(half(prevs[level - 1] as LumaImage));
    curs.push(half(curs[level - 1] as LumaImage));
  }
  let field: FlowField | null = null;
  for (let level = LEVELS - 1; level >= 0; level -= 1) {
    const radius = plan.radii[LEVELS - 1 - level] ?? 1;
    field = searchLevel(
      prevs[level] as LumaImage,
      curs[level] as LumaImage,
      field,
      2 ** level,
      radius,
      1,
    );
  }
  for (const step of plan.refine) field = searchLevel(prev, cur, field, 1, 1, step);
  return field as FlowField;
};

const matchAt = (field: FlowField, x: number, y: number): FlowMatch =>
  field.matches[
    Math.min(field.height - 1, Math.max(0, y)) * field.width +
      Math.min(field.width - 1, Math.max(0, x))
  ] ?? { x: 0, y: 0, cost: 0 };

/**
 * `fx_motion_smooth.glsl`: the vector median over 3 × 3, then an edge-aware mean round it (by the
 * two frames' mean picture, closeness to the median and how well each matched), cut to MAX_MOTION.
 */
export const smoothFlow = (field: FlowField, prev: LumaImage, cur: LumaImage): FlowField => {
  const picture = (x: number, y: number) => 0.5 * (sample(prev, x, y) + sample(cur, x, y));
  const matches = field.matches.map((own, index) => {
    const x = index % field.width;
    const y = Math.floor(index / field.width);
    const around: FlowMatch[] = [];
    const pictures: number[] = [];
    for (let dy = -1; dy <= 1; dy += 1) {
      for (let dx = -1; dx <= 1; dx += 1) {
        around.push(matchAt(field, x + dx, y + dy));
        pictures.push(
          picture(
            Math.min(field.width - 1, Math.max(0, x + dx)),
            Math.min(field.height - 1, Math.max(0, y + dy)),
          ),
        );
      }
    }
    // The vector closest to the others that also carries this pixel well.
    let median = around[4] as FlowMatch;
    let least = Infinity;
    for (const each of around) {
      const spread = around.reduce(
        (sum, other) => sum + Math.hypot(each.x - other.x, each.y - other.y),
        0,
      );
      const carried = Math.abs(
        sample(prev, x - each.x / 2, y - each.y / 2) - sample(cur, x + each.x / 2, y + each.y / 2),
      );
      const score = (spread / 9) * SPREAD_WEIGHT + carried;
      if (score < least) {
        least = score;
        median = each;
      }
    }
    let sumX = 0;
    let sumY = 0;
    let weights = 0;
    around.forEach((each, at) => {
      const alike = ((pictures[at] ?? 0) - (pictures[4] ?? 0)) / EDGE_SIGMA;
      const near = Math.hypot(each.x - median.x, each.y - median.y) / VECTOR_SIGMA;
      const weight = Math.exp(-(alike * alike) - near * near - each.cost / COST_SIGMA);
      sumX += each.x * weight;
      sumY += each.y * weight;
      weights += weight;
    });
    let flowX = weights > 1e-4 ? sumX / weights : median.x;
    let flowY = weights > 1e-4 ? sumY / weights : median.y;
    const size = Math.hypot(flowX, flowY);
    if (size > MAX_MOTION) {
      flowX *= MAX_MOTION / size;
      flowY *= MAX_MOTION / size;
    }
    return { x: flowX, y: flowY, cost: own.cost };
  });
  return { ...field, matches };
};

const agreesWith = (field: FlowField, flowX: number, flowY: number, x: number, y: number) => {
  const there = fieldSample(field, x, y);
  return 1 - smoothstep(AGREE_FROM, AGREE_TO, Math.hypot(there.x - flowX, there.y - flowY));
};

/**
 * One pixel of an in-between frame (`fx_motion_warp.glsl`), on luma pictures at the flow's size:
 * the model's way, `cut` for a scene cut. The frames' pixels are the flow's (one texel each).
 */
export const warpPixel = (
  prev: LumaImage,
  cur: LumaImage,
  field: FlowField | null,
  x: number,
  y: number,
  t: number,
  model: MotionModel,
  cut = false,
) => {
  const here0 = sample(prev, x, y);
  const here1 = sample(cur, x, y);
  const nearer = t < 0.5 ? here0 : here1;
  if (model === 'blend' || !field) {
    const still = 1 - smoothstep(BLEND_STILL, 2 * BLEND_STILL, Math.abs(here0 - here1));
    return nearer + (here0 + (here1 - here0) * t - nearer) * still;
  }
  if (cut) return nearer;
  const own = matchAt(field, x, y);
  let flowX = own.x;
  let flowY = own.y;
  const size = Math.hypot(flowX, flowY);
  if (size > MAX_MOTION) {
    flowX *= MAX_MOTION / size;
    flowY *= MAX_MOTION / size;
  }
  const fromX = x - t * flowX;
  const fromY = y - t * flowY;
  const toX = x + (1 - t) * flowX;
  const toY = y + (1 - t) * flowY;
  // Inside the picture: pixel centres span -0.5 to size - 0.5 of the texture's edges.
  const inside = (px: number, py: number) =>
    px >= -0.5 && py >= -0.5 && px <= prev.width - 0.5 && py <= prev.height - 0.5 ? 1 : 0;
  const before = sample(prev, fromX, fromY);
  const after = sample(cur, toX, toY);
  const path = agreesWith(field, flowX, flowY, x + (0.5 - t) * flowX, y + (0.5 - t) * flowY);
  const weightBefore =
    (1 - t) * inside(fromX, fromY) * agreesWith(field, flowX, flowY, fromX, fromY);
  const weightAfter = t * inside(toX, toY) * agreesWith(field, flowX, flowY, toX, toY);
  const weights = weightBefore + weightAfter;
  let warped = weights > 0 ? (before * weightBefore + after * weightAfter) / weights : nearer;
  const both =
    Math.min(weightBefore, weightAfter) > 0
      ? 1 - smoothstep(TRUST_FROM, TRUST_TO, Math.abs(before - after))
      : 1;
  let nearerSide = weightBefore >= weightAfter ? before : after;
  if (weightBefore > 0 && weightAfter > 0) nearerSide = t < 0.5 ? before : after;
  warped = nearerSide + (warped - nearerSide) * both;
  // A badly matched block is believed where its two samples still agree.
  const agreed =
    Math.min(weightBefore, weightAfter) > 0
      ? 1 - smoothstep(TRUST_FROM, TRUST_TO, Math.abs(before - after))
      : 0;
  const sure =
    path *
    Math.max(1 - smoothstep(UNSURE_FROM, UNSURE_TO, own.cost), agreed) *
    (weights >= 0.05 ? 1 : 0);
  return nearer + (warped - nearer) * sure;
};

/** A whole in-between frame on the CPU: the model's flow (steadied), then each pixel warped. */
export const interpolateFrame = (
  prev: LumaImage,
  cur: LumaImage,
  t: number,
  model: MotionModel,
): LumaImage => {
  const plan = motionPlan(model);
  const field = plan ? smoothFlow(flowField(prev, cur, plan), prev, cur) : null;
  const values = new Float32Array(prev.width * prev.height);
  for (let y = 0; y < prev.height; y += 1) {
    for (let x = 0; x < prev.width; x += 1) {
      values[y * prev.width + x] = warpPixel(prev, cur, field, x, y, t, model);
    }
  }
  return { width: prev.width, height: prev.height, values };
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
