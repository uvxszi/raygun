import type { PlaybackState } from '@raygun/bindings';
import { useSyncExternalStore } from 'react';

import { isAppActive, onAppActiveChange } from '../shell/deviceState';

/** How often the position moves between reports. */
const TICK_MS = 250;
/**
 * How far past the last report the position moves on its own, ms: Rust sends a moving position
 * at least once a second, so a report this late means playback isn't moving (the watchdog in
 * raygun-playback then says so), and the playhead stops instead of running on.
 */
const MOVE_ON_AT_MOST_MS = 2500;

/** When each state arrived: its position moves on from that moment. */
const arrivals = new WeakMap<PlaybackState, number>();

/** Notes that `state` just came from Rust, so its position moves on from now. */
export const markReceived = (state: PlaybackState) => {
  arrivals.set(state, performance.now());
};

const arrivedAt = (state: PlaybackState) => {
  let at = arrivals.get(state);
  if (at === undefined) {
    at = performance.now();
    arrivals.set(state, at);
  }
  return at;
};

/**
 * Where playback is at `now` (a `performance.now()` time): the reported position, moved on at
 * the playback speed while playing (for a moment at most), and never past the end (a report says
 * when it's over).
 */
export const positionAt = (state: PlaybackState | undefined, now: number) => {
  if (!state) return 0;
  const reported = state.positionMs ?? 0;
  if (state.status !== 'playing') return reported;
  const rate = state.rate !== null && state.rate > 0 ? state.rate : 1;
  const moved = Math.min(MOVE_ON_AT_MOST_MS, Math.max(0, now - arrivedAt(state))) * rate;
  const duration = state.durationMs ?? null;
  const position = reported + moved;
  return duration !== null && duration > 0 ? Math.min(position, duration) : position;
};

// One timer for every live position on screen: it runs only while something that plays listens
// and Raygun is on screen (shell/deviceState); back on screen, positions catch up at once.
const listeners = new Set<() => void>();
let timer: ReturnType<typeof setInterval> | null = null;
let tickAt = 0;
let stopFollowing: (() => void) | null = null;

const tick = () => {
  tickAt = performance.now();
  for (const listener of [...listeners]) listener();
};

const startTimer = () => {
  if (timer !== null || listeners.size === 0 || !isAppActive()) return;
  tickAt = performance.now();
  timer = setInterval(tick, TICK_MS);
};

const stopTimer = () => {
  if (timer === null) return;
  clearInterval(timer);
  timer = null;
};

/** Whether the shared timer runs now (tests). */
export const ticking = () => timer !== null;

const subscribeTicks = (listener: () => void) => {
  listeners.add(listener);
  stopFollowing ??= onAppActiveChange((active) => {
    if (active) {
      tick();
      startTimer();
    } else {
      stopTimer();
    }
  });
  startTimer();
  return () => {
    listeners.delete(listener);
    if (listeners.size === 0) {
      stopTimer();
      stopFollowing?.();
      stopFollowing = null;
    }
  };
};

const noTicks = () => () => {};

/**
 * Something that follows the playhead (the lyric line, the time shown, a progress step), worked
 * out from the live position: the component re-renders only when it changes, not on every
 * report or tick. `derive` gets the position in ms.
 */
export const useLiveValue = <T>(
  state: PlaybackState | undefined,
  derive: (position: number) => T,
) =>
  useSyncExternalStore(
    state?.status === 'playing' ? subscribeTicks : noTicks,
    () => derive(positionAt(state, tickAt)),
    () => derive(positionAt(state, tickAt)),
  );

/**
 * The playback position, moved on locally while playing (M46.7): Rust sends the position about
 * once a second, too coarse for a playhead that glides. Each report (a seek, a new song) starts it
 * again from what the engine said. `stepMs` rounds it down, so a time shown in seconds re-renders
 * once a second.
 */
export const useLivePosition = (state: PlaybackState | undefined, stepMs = TICK_MS) =>
  useLiveValue(state, (position) =>
    state?.status === 'playing' && stepMs > 1 ? Math.floor(position / stepMs) * stepMs : position,
  );
