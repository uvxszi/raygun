import type { KeyboardEvent } from 'react';

/** How far the arrow keys move the playhead. */
export const SEEK_KEY_STEP_MS = 5000;

/**
 * The seek slider moves in quarter seconds so the playhead glides (M46.7); the arrow keys still
 * jump five seconds, committed on key up as a drag is on release.
 */
export const onSeekKey =
  (positionMs: number, durationMs: number, scrub: (positionMs: number) => void) =>
  (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
    event.preventDefault();
    const delta = event.key === 'ArrowRight' ? SEEK_KEY_STEP_MS : -SEEK_KEY_STEP_MS;
    scrub(Math.min(Math.max(positionMs + delta, 0), durationMs));
  };
