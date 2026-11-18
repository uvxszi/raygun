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
    // Only the overlay itself: the still cells that make up a mark with edges of its own. Still
    // parts of the picture without one stay as they are, never darkened or lightened.
    const support = overlaySupport(raw, this.meanLumas(), this.layout);
    if (!support.some(Boolean)) return null;
    // A 3 × 3 mean over the overlay's cells steadies the opacity (each cell's variance is noisy).
    const data = new Uint8Array(cols * rows * 4);
    for (let row = 0; row < rows; row += 1) {
      for (let col = 0; col < cols; col += 1) {
        const cell = row * cols + col;
        if (!support[cell]) continue;
        let sum = 0;
        let taps = 0;
        for (let dy = -1; dy <= 1; dy += 1) {
          for (let dx = -1; dx <= 1; dx += 1) {
            const y = row + dy;
            const x = col + dx;
            if (x < 0 || y < 0 || x >= cols || y >= rows || !support[y * cols + x]) continue;
            sum += raw[y * cols + x] ?? 0;
            taps += 1;
          }
        }
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

  /** The mean luma of the box's cells, and of the ring round it in its cells, rows from the top. */
  private meanLumas() {
    const n = Math.max(1, this.count);
    const { cols, rows, innerLeft, innerTop, innerCols, innerRows } = this.layout;
    const lumas = new Float64Array(cols * rows);
    for (let row = 0; row < innerRows; row += 1) {
      for (let col = 0; col < innerCols; col += 1) {
        lumas[(row + innerTop) * cols + col + innerLeft] =
          (this.inner[(row * innerCols + col) * 5 + 3] ?? 0) / n;
      }
    }
    // The ring as the picture round the box: its mean over all its cells (one cell's own mean is
    // too noisy to tell an edge by).
    const ring =
      this.ringLuma.reduce((sum, value) => sum + value, 0) / n / Math.max(1, this.ringCells.length);
    for (const cell of this.ringCells) lumas[cell] = ring;
    return lumas;
  }
}

/** A box cell this still (estimated opacity) may be part of the overlay… */
export const SUPPORT_STILL = 0.25;
/** …which has edges: its mean luma differs from a neighbour's (box or ring) by this much. */
export const SUPPORT_EDGE = 0.05;

/**
 * The box's cells that are the overlay (1) or not (0), as Find it tells one: still cells joined
 * (side by side) into a patch that has an edge somewhere, a cell whose mean picture stands out
 * from a neighbour's, the ring round the box included (an overlay filling the box stands out from
 * the picture round it). A still part of the picture with no edge of its own is no overlay. `raw`
 * is the box's estimated opacity per cell, `lumas` the mean luma of the whole grid.
 */
export const overlaySupport = (
  raw: ArrayLike<number>,
  lumas: ArrayLike<number>,
  layout: SampleLayout,
) => {
  const { cols: gridCols, innerLeft, innerTop, innerCols: cols, innerRows: rows } = layout;
  const gridRows = layout.rows;
  const still = (col: number, row: number) => (raw[row * cols + col] ?? 0) >= SUPPORT_STILL;
  const luma = (col: number, row: number) =>
    lumas[(row + innerTop) * gridCols + col + innerLeft] ?? 0;
  const edged = (col: number, row: number) => {
    const here = luma(col, row);
    for (const [dx, dy] of [
      [1, 0],
      [-1, 0],
      [0, 1],
      [0, -1],
    ] as const) {
      const gridCol = col + dx + innerLeft;
      const gridRow = row + dy + innerTop;
      if (gridCol < 0 || gridRow < 0 || gridCol >= gridCols || gridRow >= gridRows) continue;
      if (Math.abs((lumas[gridRow * gridCols + gridCol] ?? 0) - here) > SUPPORT_EDGE) return true;
    }
    return false;
  };
  const support = new Uint8Array(cols * rows);
  const seen = new Uint8Array(cols * rows);
  for (let start = 0; start < cols * rows; start += 1) {
    if (seen[start] || !still(start % cols, Math.floor(start / cols))) continue;
    const patch: number[] = [];
    let hasEdge = false;
    const stack = [start];
    seen[start] = 1;
    while (stack.length > 0) {
      const cell = stack.pop() ?? 0;
      const col = cell % cols;
      const row = Math.floor(cell / cols);
      patch.push(cell);
      if (!hasEdge && edged(col, row)) hasEdge = true;
      for (const [dx, dy] of [
        [1, 0],
        [-1, 0],
        [0, 1],
        [0, -1],
      ] as const) {
        const x = col + dx;
        const y = row + dy;
        if (x < 0 || y < 0 || x >= cols || y >= rows) continue;
        const next = y * cols + x;
        if (seen[next] || !still(x, y)) continue;
        seen[next] = 1;
        stack.push(next);
      }
    }
    if (hasEdge) for (const cell of patch) support[cell] = 1;
  }
  return support;
};

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
 * What `fx_watermark.glsl` does with a pixel (0–1 RGB) under a mask texel (bytes): the blend
 * inverted, each channel kept to 0–1.
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
/** Samples before looking, at least. */
export const FIND_SAMPLES = 40;
/** The most samples taken (about a minute at a sample every 4th frame) before giving up. */
export const FIND_MAX_SAMPLES = 450;
/** Scene changes an overlay must stay through: scenery holds still within a shot, a logo across them. */
export const FIND_CUTS = 2;
/** A sample this different from its shot's first (mean luma difference per cell) starts a new shot. */
const CUT_DIFFERENCE = 0.08;
/** A patch's cell in each shot's first sample is within this of its mean over all of them. */
const SHOT_MATCH = 0.04;
/** The latest shots whose first samples are kept for that. */
const KEPT_SHOTS = 16;
/** A patch's box must vary at least this much (luma deviation of its mean picture): not uniform. */
const MIN_STRUCTURE = 0.06;
/** A patch's cells varying this much have a pattern that must hold in every shot… */
const PATTERN_SPREAD = 0.03;
/** …correlating at least this well (normalized cross-correlation). */
const MIN_NCC = 0.9;
/** A still cell changes less than this share of the picture's typical change… */
const STILL_SHARE = 0.15;
/** …and less than this (luma deviation) at all. */
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

/**
 * Luma sums of the whole picture over time, to find a static overlay. Samples go on until the
 * picture has changed shot at least [FIND_CUTS] times (each a sample far from its shot's first),
 * since within one held shot scenery with edges looks just like an overlay; a patch that stays
 * the same across the shots (its deviation over all samples stays tiny) is one.
 */
export class OverlayFinder {
  private count = 0;
  private readonly sums = new Float64Array(FIND_COLS * FIND_ROWS);
  private readonly squares = new Float64Array(FIND_COLS * FIND_ROWS);
  /** The current shot's first sample, to tell a scene change. */
  private shotStart: Float64Array | null = null;
  /** The first samples of the latest shots, to check a patch looks the same across each cut. */
  private readonly shotStarts: Float64Array[] = [];
  private changes = 0;

  get samples() {
    return this.count;
  }

  /** Scene changes seen so far. */
  get cuts() {
    return this.changes;
  }

  /** Enough to answer: the scene changes and samples needed, or as many samples as are taken. */
  get ready() {
    return (
      (this.changes >= FIND_CUTS && this.count >= FIND_SAMPLES) || this.count >= FIND_MAX_SAMPLES
    );
  }

  /** One sample: `FIND_COLS × FIND_ROWS` RGBA bytes, rows from the top. */
  add(pixels: ArrayLike<number>) {
    const lumas = new Float64Array(FIND_COLS * FIND_ROWS);
    let difference = 0;
    for (let cell = 0; cell < FIND_COLS * FIND_ROWS; cell += 1) {
      const at = cell * 4;
      const luma = lumaOf(
        (pixels[at] ?? 0) / 255,
        (pixels[at + 1] ?? 0) / 255,
        (pixels[at + 2] ?? 0) / 255,
      );
      lumas[cell] = luma;
      if (this.shotStart) difference += Math.abs(luma - (this.shotStart[cell] ?? 0));
      this.sums[cell] = (this.sums[cell] ?? 0) + luma;
      this.squares[cell] = (this.squares[cell] ?? 0) + luma * luma;
    }
    if (!this.shotStart || difference / (FIND_COLS * FIND_ROWS) > CUT_DIFFERENCE) {
      if (this.shotStart) this.changes += 1;
      this.shotStart = lumas;
      this.shotStarts.push(lumas);
      if (this.shotStarts.length > KEPT_SHOTS) this.shotStarts.shift();
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
   * out from its neighbours, is logo-sized and stays the same across [FIND_CUTS] scene changes
   * (nearer the picture's edges wins a tie); nothing unless the evidence is strong; unsure when
   * too little of the picture moved or the scene didn't change often enough.
   */
  verdict(): FindVerdict {
    const n = this.count;
    if (n < 2 || this.changes < FIND_CUTS) return { kind: 'unsure' };
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
        // The same before and after each cut, not just on average.
        const mean = means[cell] ?? 0;
        if (this.shotStarts.some((start) => Math.abs((start[cell] ?? 0) - mean) > SHOT_MATCH))
          continue;
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
      // Larger wins; of two the same size, the one nearer the picture's edge (where logos sit).
      const fromEdge = (patch: typeof found) =>
        Math.min(patch.left, patch.top, FIND_COLS - 1 - patch.right, FIND_ROWS - 1 - patch.bottom);
      const better =
        !best ||
        found.count > best.count ||
        (found.count === best.count && fromEdge(found) < fromEdge(best));
      if (!plausible || !better) continue;
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
      if (ringTypical < typical * RING_SHARE) continue;
      const members: number[] = [];
      for (let cell = 0; cell < cells; cell += 1)
        if (group[cell] === groups - 1) members.push(cell);
      if (this.structureHolds(members, found, means)) best = found;
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

  /**
   * A patch's own look holds across the shots: its box isn't uniform (grass, sky and walls
   * never count), it stands out from the cells round it in every shot's first sample, and its
   * pattern there correlates with its pattern over all samples (NCC, the patch's mean removed).
   */
  private structureHolds(
    members: number[],
    box: { left: number; top: number; right: number; bottom: number },
    means: Float64Array,
  ) {
    const spread = (values: number[]) => {
      const mean = values.reduce((sum, value) => sum + value, 0) / Math.max(1, values.length);
      const variance =
        values.reduce((sum, value) => sum + (value - mean) ** 2, 0) / Math.max(1, values.length);
      return { mean, deviation: Math.sqrt(variance) };
    };
    const boxed: number[] = [];
    for (let row = box.top; row <= box.bottom; row += 1) {
      for (let col = box.left; col <= box.right; col += 1) {
        boxed.push(means[row * FIND_COLS + col] ?? 0);
      }
    }
    if (spread(boxed).deviation < MIN_STRUCTURE) return false;
    const member = new Set(members);
    const pattern = members.map((cell) => means[cell] ?? 0);
    const shape = spread(pattern);
    for (const start of this.shotStarts) {
      // The patch against the cells just outside it, in this shot.
      let contrast = 0;
      let pairs = 0;
      for (const cell of members) {
        const col = cell % FIND_COLS;
        const row = Math.floor(cell / FIND_COLS);
        for (const [dx, dy] of [
          [1, 0],
          [-1, 0],
          [0, 1],
          [0, -1],
        ] as const) {
          const x = col + dx;
          const y = row + dy;
          const next = y * FIND_COLS + x;
          if (x < 0 || y < 0 || x >= FIND_COLS || y >= FIND_ROWS || member.has(next)) continue;
          contrast += Math.abs((start[cell] ?? 0) - (start[next] ?? 0));
          pairs += 1;
        }
      }
      if (pairs === 0 || contrast / pairs < MIN_EDGE) return false;
      // A patch with a pattern of its own keeps it.
      if (shape.deviation >= PATTERN_SPREAD) {
        const seen = members.map((cell) => start[cell] ?? 0);
        const shot = spread(seen);
        let product = 0;
        seen.forEach((value, index) => {
          product += (value - shot.mean) * ((pattern[index] ?? 0) - shape.mean);
        });
        const ncc = product / (members.length * shot.deviation * shape.deviation || 1);
        if (ncc < MIN_NCC) return false;
      }
    }
    return true;
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
