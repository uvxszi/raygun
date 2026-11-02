/**
 * Loudness of a song as it plays (M46.12), after ITU-R BS.1770: K-weighted power in 400 ms
 * blocks overlapping by 75 %, gated absolutely at −70 LUFS and then 10 LU below the ungated
 * mean. The engine feeds it the K-weighted mean square of every 100 ms step (both channels
 * summed); the K-weighting itself is two BiquadFilters in the graph.
 */
export class LoudnessMeter {
  private readonly steps: number[] = [];
  private readonly blocks: number[] = [];

  /** Adds one 100 ms step's K-weighted mean square, summed over channels. */
  add(power: number) {
    if (!Number.isFinite(power) || power < 0) return;
    this.steps.push(power);
    if (this.steps.length > 4) this.steps.shift();
    if (this.steps.length === 4) {
      this.blocks.push(this.steps.reduce((sum, value) => sum + value, 0) / 4);
    }
  }

  /** Seconds of sound measured so far. */
  get seconds() {
    return this.blocks.length / 10;
  }

  /** The integrated loudness in LUFS, or null while nothing above silence was heard. */
  integrated(): number | null {
    const audible = this.blocks.filter((power) => toLufs(power) > ABSOLUTE_GATE);
    if (audible.length === 0) return null;
    const relativeGate = toLufs(mean(audible)) - 10;
    const kept = audible.filter((power) => toLufs(power) > relativeGate);
    return toLufs(mean(kept.length > 0 ? kept : audible));
  }
}

const ABSOLUTE_GATE = -70;

const mean = (values: number[]) => values.reduce((sum, value) => sum + value, 0) / values.length;

/** BS.1770's loudness of a K-weighted power. */
export const toLufs = (power: number) => -0.691 + 10 * Math.log10(Math.max(power, 1e-12));

/** The mean square of a block of samples. */
export const meanSquare = (samples: ArrayLike<number>) => {
  let sum = 0;
  for (let index = 0; index < samples.length; index += 1) {
    const sample = samples[index] ?? 0;
    sum += sample * sample;
  }
  return samples.length > 0 ? sum / samples.length : 0;
};
