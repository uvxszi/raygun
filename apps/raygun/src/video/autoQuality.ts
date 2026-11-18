/**
 * Automatic quality that keeps a video playing: while the quality is automatic, playback that
 * keeps stalling to buffer (two stalls within 30 s, or one longer than 4 s) steps down to the next
 * lower quality; after two minutes without a stall an adaptive stream steps back up one at a time.
 * The same rules as the Android player's `AutoQuality` (`PlayerAutoQuality.kt`).
 */

import type { QualityRow } from './protocol';

/** Stalls within this long of each other count together. */
export const STALL_WINDOW_MS = 30_000;
/** This many stalls within the window step down. */
export const STALLS_TO_LOWER = 2;
/** A stall this long steps down by itself. */
export const LONG_STALL_MS = 4_000;
/** After a step, stalls while the new quality settles in don't count. */
export const SETTLE_MS = 8_000;
/** Playing this long without a stall steps an adaptive stream back up. */
export const SMOOTH_MS = 120_000;

export type StallAction = 'lower' | 'raise' | null;

/**
 * Watches the player's stalls, sampled on a timer: `tick` with whether it plays and whether it's
 * stalled (buffering while playing, not after a seek or before the first frame) says when to
 * step down or back up; `stepped` after acting starts the count again.
 */
export class StallWatch {
  private stalls: number[] = [];
  private stallStart: number | null = null;
  private longActed = false;
  private settleUntil = 0;
  private smooth = 0;
  private lastTick: number | null = null;

  /** A new video: nothing counts from before. */
  reset(now: number): void {
    this.stalls = [];
    this.stallStart = null;
    this.longActed = false;
    this.settleUntil = now + SETTLE_MS;
    this.smooth = 0;
    this.lastTick = now;
  }

  /** After a step down or up: the count starts again once the new quality settles. */
  stepped(now: number): void {
    this.reset(now);
  }

  /** `canRaise`: a step down is in effect that could be lifted (adaptive streams only). */
  tick(now: number, playing: boolean, stalled: boolean, canRaise: boolean): StallAction {
    const since = this.lastTick === null ? 0 : Math.max(0, now - this.lastTick);
    this.lastTick = now;
    const settling = now < this.settleUntil;
    if (stalled) {
      this.smooth = 0;
      if (this.stallStart === null) {
        this.stallStart = now;
        this.longActed = false;
        if (!settling) this.stalls.push(now);
      }
      this.stalls = this.stalls.filter((at) => now - at <= STALL_WINDOW_MS);
      if (settling) return null;
      if (!this.longActed && now - this.stallStart >= LONG_STALL_MS) {
        this.longActed = true;
        return 'lower';
      }
      if (this.stalls.length >= STALLS_TO_LOWER) return 'lower';
      return null;
    }
    this.stallStart = null;
    if (playing) this.smooth += since;
    if (canRaise && this.smooth >= SMOOTH_MS) {
      this.smooth = 0;
      return 'raise';
    }
    return null;
  }
}

/** The tier a picture counts as for capping: its long side against a 16:9 one's, or its height. */
export const pictureHeight = (width: number, height: number): number => {
  const short = Math.min(width, height);
  const long = Math.max(width, height);
  return Math.max(short, Math.floor((long * 9 + 8) / 16));
};

/**
 * The row to step down to from the one playing: the largest below it. An adaptive stream steps
 * only among its own variants (never below its lowest); otherwise another link plays. The same
 * range as the one playing first, then SDR. Null when there's nothing lower.
 */
export const lowerStep = (rows: QualityRow[], adaptive: boolean): QualityRow | null => {
  const playing = rows.find((row) => row.state === 'playing');
  if (!playing) return null;
  const lower = rows.filter(
    (row) =>
      row.height < playing.height &&
      (adaptive
        ? row.state === 'variant'
        : row.state === 'thisSource' || row.state === 'otherSource'),
  );
  if (lower.length === 0) return null;
  const top = Math.max(...lower.map((row) => row.height));
  const atTop = lower.filter((row) => row.height === top);
  return (
    atTop.find((row) => row.range === playing.range) ??
    atTop.find((row) => row.range === 'sdr') ??
    atTop[0] ??
    null
  );
};

/** The cap for a row's variants: the largest picture among them, else its tier. */
export const capOf = (row: QualityRow): number =>
  Math.max(row.height, ...(row.sizes ?? []).map((size) => pictureHeight(size.width, size.height)));

/**
 * One step back up from `cap`: the next variant row above it, or null when none is (the cap
 * goes, back to the preferred resolution's).
 */
export const raiseStep = (rows: QualityRow[], cap: number): QualityRow | null => {
  const above = rows
    .filter((row) => row.state === 'variant' && row.height > cap)
    .sort((a, b) => a.height - b.height);
  return above[0] ?? null;
};
