import type { WatermarkRemoval } from '@raygun/bindings';

/**
 * The watermark remover as numbers (`raygun_model::WatermarkRemoval`), the same on Android
 * (`PlayerWatermarkMath.kt`; the tests of both check the same numbers).
 *
 * A static overlay blends into the picture as I = α·W + (1 − α)·J (J the video under it, W the
 * overlay's colour, α its opacity). Over many frames the video under it changes while the overlay
 * stays, so a pixel's change over time is (1 − α) times the video's: α comes from the box's
 * temporal variance against the ring of picture around the box (whose variance is the video's
 * own), and W from the means. `fx_watermark.glsl` then inverts the blend, J = (I − α·W) / (1 − α),
 * filling parts too opaque to invert from the box's edges.
 *
 * Grids are row by row from the top (as the pictures read back); the box is in ten-thousandths of
 * the picture from its top left.
 */

export type Watermark = Required<WatermarkRemoval>;

export const WATERMARK_SCALE = 10_000;

export const NO_WATERMARK: Watermark = {
  enabled: false,
  x: 0,
  y: 0,
  width: 0,
  height: 0,
  brightness: 0,
  contrast: 0,
  feather: 0,
};

export const withWatermarkDefaults = (watermark?: WatermarkRemoval | null): Watermark => ({
  ...NO_WATERMARK,
  ...watermark,
});

/** On, with a box to work in. */
export const isWatermarkActive = (watermark: Watermark) =>
  watermark.enabled && watermark.width > 0 && watermark.height > 0;

export const sameWatermark = (left: Watermark, right: Watermark) =>
  (Object.keys(NO_WATERMARK) as (keyof Watermark)[]).every((key) => left[key] === right[key]);

/** The most cells across the box's sample grid. */
export const MAX_CELLS = 48;
/** Samples before a mask is estimated. */
export const MIN_SAMPLES = 24;
/** The video around the box must change at least this much (luma variance) for an estimate. */
export const MIN_REFERENCE_VARIANCE = 0.0015;
/** Opacity under this is no overlay. */
export const ALPHA_FLOOR = 0.08;
/** The most opacity the remover takes out: more would amplify noise (and casts) past use. */
export const ALPHA_MAX = 0.6;
/** How far the overlay's colour may stray from grey (per channel from its luma). */
export const CHROMA_LIMIT = 0.08;
/** Past this many samples the sums halve, so the estimate keeps following. */
export const SAMPLE_CAP = 1_500;

/** A box in pixels of the picture, from its top left. */
export type PixelRect = { left: number; top: number; width: number; height: number };

/** Where the box's samples come from: the box and a ring of picture around it, on one grid. */
export type SampleLayout = {
  /** The sampled rectangle (the box and its ring), in pixels from the picture's top left. */
  rect: PixelRect;
  cols: number;
  rows: number;
  /** The box's cells within the grid. */
  innerLeft: number;
  innerTop: number;
  innerCols: number;
  innerRows: number;
};

/** The box in pixels of a `frameWidth` × `frameHeight` picture. */
export const boxPixels = (
  watermark: Pick<Watermark, 'x' | 'y' | 'width' | 'height'>,
  frameWidth: number,
  frameHeight: number,
): PixelRect => ({
  left: (watermark.x / WATERMARK_SCALE) * frameWidth,
  top: (watermark.y / WATERMARK_SCALE) * frameHeight,
  width: (watermark.width / WATERMARK_SCALE) * frameWidth,
  height: (watermark.height / WATERMARK_SCALE) * frameHeight,
});

/**
 * The grid the box is sampled on: at most [MAX_CELLS] cells across the box (at least 4), and a
 * ring about half the box wide around it, as far as the picture goes.
 */
export const sampleLayout = (
  watermark: Pick<Watermark, 'x' | 'y' | 'width' | 'height'>,
  frameWidth: number,
  frameHeight: number,
): SampleLayout => {
  const box = boxPixels(watermark, frameWidth, frameHeight);
  const scale = Math.min(1, MAX_CELLS / Math.max(box.width, box.height, 1));
  const innerCols = Math.max(4, Math.round(box.width * scale));
  const innerRows = Math.max(4, Math.round(box.height * scale));
  const cellWidth = box.width / innerCols;
  const cellHeight = box.height / innerRows;
  const ringCols = Math.ceil(innerCols / 2);
  const ringRows = Math.ceil(innerRows / 2);
  const fits = (room: number, cell: number) => (cell > 0 ? Math.floor(room / cell + 1e-6) : 0);
  const left = Math.min(ringCols, fits(box.left, cellWidth));
  const right = Math.min(ringCols, fits(frameWidth - box.left - box.width, cellWidth));
  const top = Math.min(ringRows, fits(box.top, cellHeight));
  const bottom = Math.min(ringRows, fits(frameHeight - box.top - box.height, cellHeight));
  return {
    rect: {
      left: box.left - left * cellWidth,
      top: box.top - top * cellHeight,
      width: (innerCols + left + right) * cellWidth,
      height: (innerRows + top + bottom) * cellHeight,
    },
    cols: innerCols + left + right,
    rows: innerRows + top + bottom,
    innerLeft: left,
    innerTop: top,
    innerCols,
    innerRows,
  };
};

const lumaOf = (red: number, green: number, blue: number) =>
  0.2126 * red + 0.7152 * green + 0.0722 * blue;

/** The overlay found: its opacity and colour per cell of the box, as an RGBA8 texture's bytes. */
export type WatermarkMask = { cols: number; rows: number; data: Uint8Array };

/**
 * Sums of the box's samples over time. `add` takes one sample (RGBA bytes on the layout's grid,
 * rows from the top); `estimate` gives the mask once there are enough.
 */
export class WatermarkEstimator {
  private count = 0;
  private readonly inner: Float64Array;
  private readonly ringLuma: Float64Array;
  private readonly ringSquares: Float64Array;
  private readonly ringColour = new Float64Array(3);
  private readonly ringCells: number[] = [];

  constructor(readonly layout: SampleLayout) {
    const { cols, rows, innerLeft, innerTop, innerCols, innerRows } = layout;
    // Per box cell: red, green, blue, luma and luma² sums.
    this.inner = new Float64Array(innerCols * innerRows * 5);
    for (let row = 0; row < rows; row += 1) {
      for (let col = 0; col < cols; col += 1) {
        const inside =
          col >= innerLeft &&
          col < innerLeft + innerCols &&
          row >= innerTop &&
          row < innerTop + innerRows;
        if (!inside) this.ringCells.push(row * cols + col);
      }
    }
    this.ringLuma = new Float64Array(this.ringCells.length);
    this.ringSquares = new Float64Array(this.ringCells.length);
  }

  get samples() {
    return this.count;
  }

  /** One sample of the grid: `cols × rows` RGBA bytes, rows from the top. */
  add(pixels: ArrayLike<number>) {
    const { cols, innerLeft, innerTop, innerCols, innerRows } = this.layout;
    if (this.count >= SAMPLE_CAP) this.halve();
    for (let row = 0; row < innerRows; row += 1) {
      for (let col = 0; col < innerCols; col += 1) {
        const at = ((row + innerTop) * cols + col + innerLeft) * 4;
        const red = (pixels[at] ?? 0) / 255;
        const green = (pixels[at + 1] ?? 0) / 255;
        const blue = (pixels[at + 2] ?? 0) / 255;
        const luma = lumaOf(red, green, blue);
        const sums = (row * innerCols + col) * 5;
        this.inner[sums] = (this.inner[sums] ?? 0) + red;
        this.inner[sums + 1] = (this.inner[sums + 1] ?? 0) + green;
        this.inner[sums + 2] = (this.inner[sums + 2] ?? 0) + blue;
        this.inner[sums + 3] = (this.inner[sums + 3] ?? 0) + luma;
        this.inner[sums + 4] = (this.inner[sums + 4] ?? 0) + luma * luma;
      }
    }
    this.ringCells.forEach((cell, index) => {
      const at = cell * 4;
      const red = (pixels[at] ?? 0) / 255;
      const green = (pixels[at + 1] ?? 0) / 255;
      const blue = (pixels[at + 2] ?? 0) / 255;
      const luma = lumaOf(red, green, blue);
      this.ringLuma[index] = (this.ringLuma[index] ?? 0) + luma;
      this.ringSquares[index] = (this.ringSquares[index] ?? 0) + luma * luma;
      this.ringColour[0] = (this.ringColour[0] ?? 0) + red;
      this.ringColour[1] = (this.ringColour[1] ?? 0) + green;
      this.ringColour[2] = (this.ringColour[2] ?? 0) + blue;
    });
    this.count += 1;
  }

  private halve() {
    for (const sums of [this.inner, this.ringLuma, this.ringSquares, this.ringColour]) {
      for (let index = 0; index < sums.length; index += 1) sums[index] = (sums[index] ?? 0) / 2;
    }
    this.count /= 2;
  }

  /** The video's own change around the box (mean temporal luma variance of the ring). */
  referenceVariance() {
    const n = this.count;
    if (n < 2 || this.ringCells.length === 0) return 0;
    let total = 0;
    for (let index = 0; index < this.ringCells.length; index += 1) {
      const mean = (this.ringLuma[index] ?? 0) / n;
      total += Math.max(0, (this.ringSquares[index] ?? 0) / n - mean * mean);
    }
    return total / this.ringCells.length;
  }

  /**
   * The overlay's opacity and colour per box cell, or null before there are [MIN_SAMPLES]
   * samples of a changing picture.
   */
  estimate(): WatermarkMask | null {
    const n = this.count;
    const reference = this.referenceVariance();
    if (n < MIN_SAMPLES || reference < MIN_REFERENCE_VARIANCE) return null;
    const { innerCols: cols, innerRows: rows } = this.layout;
    const ringCount = this.ringCells.length * n;
    const background = [0, 1, 2].map((channel) => (this.ringColour[channel] ?? 0) / ringCount);
    const raw = new Float64Array(cols * rows);
    for (let cell = 0; cell < cols * rows; cell += 1) {
      const mean = (this.inner[cell * 5 + 3] ?? 0) / n;
      const variance = Math.max(0, (this.inner[cell * 5 + 4] ?? 0) / n - mean * mean);
      const kept = Math.min(1, Math.sqrt(variance / reference));
      raw[cell] = 1 - kept;
    }
    // A 3 × 3 mean steadies the opacity (each cell's variance is noisy).
    const data = new Uint8Array(cols * rows * 4);
    for (let row = 0; row < rows; row += 1) {
      for (let col = 0; col < cols; col += 1) {
        let sum = 0;
        let taps = 0;
        for (let dy = -1; dy <= 1; dy += 1) {
          for (let dx = -1; dx <= 1; dx += 1) {
            const y = row + dy;
            const x = col + dx;
            if (x < 0 || y < 0 || x >= cols || y >= rows) continue;
            sum += raw[y * cols + x] ?? 0;
            taps += 1;
          }
        }
        const cell = row * cols + col;
        const texel = maskTexel(
          sum / taps,
          [0, 1, 2].map((channel) => (this.inner[cell * 5 + channel] ?? 0) / n),
          background,
        );
        data.set(texel, cell * 4);
      }
    }
    return { cols, rows, data };
  }
}

/**
 * One cell of the mask (RGBA bytes: the overlay's colour times its opacity, then the opacity) from
 * its estimated opacity, its mean colour and the background's. Unsure cells (a colour the blend
 * can't explain) are left alone; the colour stays near grey, so no cast comes from it.
 */
export const maskTexel = (rawAlpha: number, mean: number[], background: number[]) => {
  const texel = [0, 0, 0, 0];
  if (rawAlpha < ALPHA_FLOOR) return texel;
  const alpha = Math.min(ALPHA_MAX, rawAlpha);
  const [red = 0, green = 0, blue = 0] = mean;
  const [backRed = 0, backGreen = 0, backBlue = 0] = background;
  const markLuma =
    (lumaOf(red, green, blue) - (1 - alpha) * lumaOf(backRed, backGreen, backBlue)) / alpha;
  if (markLuma < -0.1 || markLuma > 1.1) return texel;
  for (let channel = 0; channel < 3; channel += 1) {
    const colour = ((mean[channel] ?? 0) - (1 - alpha) * (background[channel] ?? 0)) / alpha;
    const near = markLuma + Math.min(CHROMA_LIMIT, Math.max(-CHROMA_LIMIT, colour - markLuma));
    texel[channel] = Math.round(alpha * Math.min(1, Math.max(0, near)) * 255);
  }
  texel[3] = Math.round(alpha * 255);
  return texel;
};

/**
 * What `fx_watermark.glsl` does with a pixel (0�1 RGB) under a mask texel (bytes): the blend
 * inverted, each channel kept to 0�1.
 */
export const restorePixel = (pixel: number[], texel: number[]) => {
  const alpha = (texel[3] ?? 0) / 255;
  return pixel.map((value, channel) =>
    Math.min(1, Math.max(0, (value - (texel[channel] ?? 0) / 255) / Math.max(1 - alpha, 0.4))),
  );
};
// ---- Finding the overlay ----

/** The grid the whole picture is sampled on to find an overlay. */
export const FIND_COLS = 64;
export const FIND_ROWS = 36;
/** Samples before looking. */
export const FIND_SAMPLES = 40;
/** A still cell changes less than this share of the picture's typical change� */
const STILL_SHARE = 0.15;
/** �and less than this (luma deviation) at all. */
const STILL_MOST = 0.02;
/** The picture must change at least this much (median luma deviation) to tell still parts. */
const MIN_CHANGE = 0.04;
/** More of the picture than this still: too little moves to tell an overlay from the scene. */
const MOST_STILL = 0.15;
/** A still cell must stand out from a neighbour by this much (logos do, flat areas don't). */
const MIN_EDGE = 0.08;
/** The picture round a patch must move at least this share of the typical change. */
const RING_SHARE = 0.5;
/** The fewest cells a patch has. */
const MIN_CELLS = 6;
/** The largest box found, as a share of the picture. */
const MAX_AREA = 0.1;
/** The most of the picture's width or height a box found spans. */
const MAX_SPAN = 0.4;

/** What finding concluded: a box, none, or that the picture moved too little to tell. */
export type FindVerdict =
  | { kind: 'found'; box: Pick<Watermark, 'x' | 'y' | 'width' | 'height'> }
  | { kind: 'none' }
  | { kind: 'unsure' };

/** Luma sums of the whole picture over time, to find a static overlay. */
export class OverlayFinder {
  private count = 0;
  private readonly sums = new Float64Array(FIND_COLS * FIND_ROWS);
  private readonly squares = new Float64Array(FIND_COLS * FIND_ROWS);

  get samples() {
    return this.count;
  }

  /** One sample: `FIND_COLS � FIND_ROWS` RGBA bytes, rows from the top. */
  add(pixels: ArrayLike<number>) {
    for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
      const at = cell * 4;
      const luma = lumaOf(
        (pixels[at] ?? 0) / 255,
        (pixels[at + 1] ?? 0) / 255,
        (pixels[at + 2] ?? 0) / 255,
      );
      this.sums[cell] = (this.sums[cell] ?? 0) + luma;
      this.squares[cell] = (this.squares[cell] ?? 0) + luma * luma;
    }
    this.count += 1;
  }

  /** The box of `verdict()`, or null. */
  find(): Pick<Watermark, 'x' | 'y' | 'width' | 'height'> | null {
    const verdict = this.verdict();
    return verdict.kind === 'found' ? verdict.box : null;
  }

  /**
   * A box round the largest patch that stays put while the picture round it moves a lot, stands
   * out from its neighbours and is logo-sized; nothing unless the evidence is strong; unsure
   * when too little of the picture moved.
   */
  verdict(): FindVerdict {
    const n = this.count;
    if (n < 2) return { kind: 'unsure' };
    const cells = FIND_COLS * FIND_ROWS;
    const means = new Float64Array(cells);
    const deviations = new Float64Array(cells);
    for (let cell = 0; cell < cells; cell += 1) {
      const mean = (this.sums[cell] ?? 0) / n;
      means[cell] = mean;
      deviations[cell] = Math.sqrt(Math.max(0, (this.squares[cell] ?? 0) / n - mean * mean));
    }
    const sorted = Array.from(deviations).sort((left, right) => left - right);
    const typical = sorted[Math.floor(sorted.length / 2)] ?? 0;
    if (typical < MIN_CHANGE) return { kind: 'unsure' };
    const stillLimit = Math.min(typical * STILL_SHARE, STILL_MOST);
    let stillCount = 0;
    for (const deviation of deviations) if (deviation < stillLimit) stillCount += 1;
    if (stillCount > cells * MOST_STILL) return { kind: 'unsure' };
    const inside = (col: number, row: number) =>
      col >= 0 && row >= 0 && col < FIND_COLS && row < FIND_ROWS;
    const still = new Uint8Array(cells);
    for (let row = 0; row < FIND_ROWS; row += 1) {
      for (let col = 0; col < FIND_COLS; col += 1) {
        const cell = row * FIND_COLS + col;
        if ((deviations[cell] ?? 1) >= stillLimit) continue;
        const here = means[cell] ?? 0;
        let edge = 0;
        for (const [dx, dy] of [
          [1, 0],
          [-1, 0],
          [0, 1],
          [0, -1],
        ] as const) {
          if (!inside(col + dx, row + dy)) continue;
          edge = Math.max(edge, Math.abs((means[(row + dy) * FIND_COLS + col + dx] ?? 0) - here));
        }
        if (edge > MIN_EDGE) still[cell] = 1;
      }
    }
    // Neighbours two cells apart join (a logo's letters), then the largest group wins.
    const group = new Int32Array(cells).fill(-1);
    let best: { count: number; left: number; top: number; right: number; bottom: number } | null =
      null;
    let groups = 0;
    for (let start = 0; start < cells; start += 1) {
      if (!still[start] || (group[start] ?? -1) >= 0) continue;
      const found = { count: 0, left: FIND_COLS, top: FIND_ROWS, right: -1, bottom: -1 };
      const stack = [start];
      group[start] = groups;
      while (stack.length > 0) {
        const cell = stack.pop() ?? 0;
        const col = cell % FIND_COLS;
        const row = Math.floor(cell / FIND_COLS);
        found.count += 1;
        found.left = Math.min(found.left, col);
        found.right = Math.max(found.right, col);
        found.top = Math.min(found.top, row);
        found.bottom = Math.max(found.bottom, row);
        for (let dy = -2; dy <= 2; dy += 1) {
          for (let dx = -2; dx <= 2; dx += 1) {
            const x = col + dx;
            const y = row + dy;
            if (!inside(x, y)) continue;
            const next = y * FIND_COLS + x;
            if (!still[next] || (group[next] ?? -1) >= 0) continue;
            group[next] = groups;
            stack.push(next);
          }
        }
      }
      groups += 1;
      const width = found.right - found.left + 1;
      const height = found.bottom - found.top + 1;
      const plausible =
        found.count >= MIN_CELLS &&
        (width * height) / cells <= MAX_AREA &&
        width <= FIND_COLS * MAX_SPAN &&
        height <= FIND_ROWS * MAX_SPAN;
      if (!plausible || (best && found.count <= best.count)) continue;
      // The picture just round the patch must move: a still patch in a still corner is scenery.
      const ring: number[] = [];
      for (let row = found.top - 2; row <= found.bottom + 2; row += 1) {
        for (let col = found.left - 2; col <= found.right + 2; col += 1) {
          const onRing =
            row < found.top || row > found.bottom || col < found.left || col > found.right;
          if (onRing && inside(col, row)) ring.push(deviations[row * FIND_COLS + col] ?? 0);
        }
      }
      ring.sort((left, right) => left - right);
      const ringTypical = ring[Math.floor(ring.length / 2)] ?? 0;
      if (ringTypical >= typical * RING_SHARE) best = found;
    }
    if (!best) return { kind: 'none' };
    // One cell of margin round it, inside the picture.
    const left = Math.max(0, best.left - 1);
    const top = Math.max(0, best.top - 1);
    const right = Math.min(FIND_COLS, best.right + 2);
    const bottom = Math.min(FIND_ROWS, best.bottom + 2);
    const scaleX = (col: number) => Math.round((col / FIND_COLS) * WATERMARK_SCALE);
    const scaleY = (row: number) => Math.round((row / FIND_ROWS) * WATERMARK_SCALE);
    return {
      kind: 'found',
      box: {
        x: scaleX(left),
        y: scaleY(top),
        width: scaleX(right) - scaleX(left),
        height: scaleY(bottom) - scaleY(top),
      },
    };
  }
}
// ---- The shader's numbers ----

/** The box in the shaders' texture coordinates (origin at the bottom left): x0, y0, x1, y1. */
export const shaderBox = (watermark: Pick<Watermark, 'x' | 'y' | 'width' | 'height'>) => [
  watermark.x / WATERMARK_SCALE,
  1 - (watermark.y + watermark.height) / WATERMARK_SCALE,
  (watermark.x + watermark.width) / WATERMARK_SCALE,
  1 - watermark.y / WATERMARK_SCALE,
];

/** Leftovers' brightness: up to a quarter of full scale either way (as the picture's). */
export const watermarkBrightness = (value: number) => (value / 100) * 0.25;
/** Leftovers' contrast: halves (−100) or doubles (100) the distance from mid-grey. */
export const watermarkContrast = (value: number) => 2 ** (value / 100);
/** The feather: the share of the box (from its edge to its middle, 0.5) that blends. */
export const watermarkFeather = (value: number) => (value / 100) * 0.5;
