import {
  FIND_COLS,
  FIND_ROWS,
  isWatermarkActive,
  OverlayFinder,
  sampleLayout,
  WatermarkEstimator,
} from './watermarkMath';
import type { Watermark, WatermarkMask } from './watermarkMath';

/** A box sample every this many drawn frames, a Find it sample every this many. */
export const SAMPLE_EVERY = 3;
export const FIND_EVERY = 4;
/** The mask is estimated again every this many samples. */
export const ESTIMATE_EVERY = 8;

type Box = Pick<Watermark, 'x' | 'y' | 'width' | 'height'>;

/**
 * The desktop player's frames for the watermark remover (`watermarkMath.ts`): while a box is set
 * and the video plays, every few frames the box and the ring round it are drawn small into a 2D
 * canvas and added to the estimate, which gives a new mask now and then; Find it samples the whole
 * picture until it can say where a still overlay is. Android reads its frames back from the GPU
 * instead (`PlayerWatermarkGl.kt`).
 */
export class WatermarkSampler {
  private readonly canvas = document.createElement('canvas');
  private readonly context: CanvasRenderingContext2D | null;
  private watermark: Watermark | null = null;
  private estimator: WatermarkEstimator | null = null;
  private key = '';
  private finder: OverlayFinder | null = null;
  private frames = 0;

  constructor(
    private readonly onMask: (mask: WatermarkMask | null) => void,
    /** What finding found: a box, or none (unsure: too little moved to tell). */
    private readonly onFound: (box: Box | null, unsure: boolean) => void,
    /** Scene changes seen while finding: the answer waits for two (or about a minute). */
    private readonly onProgress: (cuts: number) => void = () => {},
  ) {
    let context: CanvasRenderingContext2D | null;
    try {
      context = this.canvas.getContext('2d', { willReadFrequently: true });
    } catch {
      context = null;
    }
    this.context = context;
  }

  /** The title's watermark remover (off: nothing is sampled). */
  setWatermark(watermark: Watermark | null) {
    this.watermark = watermark && isWatermarkActive(watermark) ? watermark : null;
  }

  /** Find it: the whole picture is sampled until a still overlay shows (or doesn't). */
  find() {
    this.finder = new OverlayFinder();
  }

  get finding() {
    return this.finder !== null;
  }

  /** A frame was drawn: sampled now and then while the video plays. */
  frame(video: HTMLVideoElement) {
    const { context } = this;
    if (!context || video.paused || !video.videoWidth || !video.videoHeight) return;
    this.frames += 1;
    if (this.finder && this.frames % FIND_EVERY === 0) this.sampleWhole(context, video);
    if (this.watermark && this.frames % SAMPLE_EVERY === 0) {
      this.sampleBox(context, video, this.watermark);
    }
  }

  private read(
    context: CanvasRenderingContext2D,
    video: HTMLVideoElement,
    source: [number, number, number, number],
    cols: number,
    rows: number,
  ) {
    if (this.canvas.width !== cols) this.canvas.width = cols;
    if (this.canvas.height !== rows) this.canvas.height = rows;
    try {
      context.drawImage(video, ...source, 0, 0, cols, rows);
      return context.getImageData(0, 0, cols, rows).data;
    } catch {
      // A frame that can't be read yet.
      return null;
    }
  }

  private sampleWhole(context: CanvasRenderingContext2D, video: HTMLVideoElement) {
    const finder = this.finder;
    if (!finder) return;
    const pixels = this.read(
      context,
      video,
      [0, 0, video.videoWidth, video.videoHeight],
      FIND_COLS,
      FIND_ROWS,
    );
    if (!pixels) return;
    const cuts = finder.cuts;
    finder.add(pixels);
    if (finder.cuts !== cuts) this.onProgress(finder.cuts);
    if (!finder.ready) return;
    this.finder = null;
    const verdict = finder.verdict();
    this.onFound(verdict.kind === 'found' ? verdict.box : null, verdict.kind === 'unsure');
  }

  private sampleBox(context: CanvasRenderingContext2D, video: HTMLVideoElement, box: Watermark) {
    const width = video.videoWidth;
    const height = video.videoHeight;
    const key = [box.x, box.y, box.width, box.height, width, height].join(',');
    if (key !== this.key || !this.estimator) {
      this.key = key;
      this.estimator = new WatermarkEstimator(sampleLayout(box, width, height));
      this.onMask(null);
    }
    const estimator = this.estimator;
    const { rect, cols, rows } = estimator.layout;
    const pixels = this.read(
      context,
      video,
      [rect.left, rect.top, rect.width, rect.height],
      cols,
      rows,
    );
    if (!pixels) return;
    estimator.add(pixels);
    if (estimator.samples % ESTIMATE_EVERY !== 0) return;
    const mask = estimator.estimate();
    if (mask) this.onMask(mask);
  }
}
