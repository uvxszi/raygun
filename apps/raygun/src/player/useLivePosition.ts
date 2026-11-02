import type { PlaybackState } from '@raygun/bindings';
import { useEffect, useState } from 'react';

/** How often the position moves between engine reports. */
const TICK_MS = 250;

/**
 * The playback position, advanced locally while playing: engines report progress every quarter to
 * half second, too coarse for line-by-line lyrics and a playhead that moves smoothly (M46.7). Each
 * report (a seek, a new track) starts it again from what the engine said.
 */
export const useLivePosition = (state: PlaybackState | undefined) => {
  const playing = state?.status === 'playing';
  const reported = state?.positionMs ?? 0;
  const loadId = state?.currentItemId ?? null;
  // How far playback has moved since `base` was reported; only the timer writes it.
  const [live, setLive] = useState({ base: reported, item: loadId, elapsedMs: 0 });

  useEffect(() => {
    if (!playing) return;
    const started = performance.now();
    const timer = setInterval(
      () => setLive({ base: reported, item: loadId, elapsedMs: performance.now() - started }),
      TICK_MS,
    );
    return () => clearInterval(timer);
  }, [playing, reported, loadId]);

  const fresh = playing && live.base === reported && live.item === loadId;
  const position = fresh ? reported + live.elapsedMs : reported;
  // Never past the end; a report decides when it's over.
  const duration = state?.durationMs ?? null;
  return duration !== null && duration > 0 ? Math.min(position, duration) : position;
};
