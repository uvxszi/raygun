import { useEffect, useLayoutEffect, useRef } from 'react';
import type { RefObject } from 'react';

import { createGlide } from './lyricsGlide';
import type { Glide } from './lyricsGlide';
import { positionAt, tickNow, useLiveValue } from './useLivePosition';
import { usePlayerTiming } from './usePlayback';

/** How long after the listener scrolls the lines stay where they were put, ms. */
const HOLD_AFTER_SCROLL_MS = 4000;

/**
 * How far before its own start the line shown may still be, ms: a report from the engine can land
 * a little behind where the playhead had moved on to, and the line before mustn't flash back.
 */
const BACK_TOLERANCE_MS = 300;

/** The last line whose start (ms) is at or before `positionMs`; -1 before the first. */
export const lineAt = (starts: readonly number[], positionMs: number) => {
  let line = -1;
  for (const [index, start] of starts.entries()) {
    if (start <= positionMs) line = index;
    else break;
  }
  return line;
};

/** The line at `positionMs`, keeping `shown` when the position slips back just before it starts. */
export const followLine = (starts: readonly number[], positionMs: number, shown: number) => {
  const line = lineAt(starts, positionMs);
  const start = starts[shown];
  return line === shown - 1 && start !== undefined && positionMs >= start - BACK_TOLERANCE_MS
    ? shown
    : line;
};

/**
 * The line being sung or said, from each line's start (ms, any offset applied): it changes on the
 * line's own time, and a report a moment behind never brings the line before back.
 */
export const useCurrentLine = (starts: readonly number[]) => {
  const timing = usePlayerTiming();
  const shown = useRef(-1);
  const current = useLiveValue(timing, (position) => followLine(starts, position, shown.current));
  useLayoutEffect(() => {
    shown.current = current;
  }, [current]);

  useEffect(() => {
    // The next line starts on time, not at the shared timer's next beat.
    if (timing?.status !== 'playing') return;
    const next = starts[current + 1];
    if (next === undefined) return;
    const rate = timing.rate !== null && timing.rate > 0 ? timing.rate : 1;
    const wait = (next - positionAt(timing, performance.now())) / rate;
    const timer = setTimeout(tickNow, Math.max(0, wait) + 1);
    return () => clearTimeout(timer);
  }, [timing, current, starts]);

  return current;
};

type FollowOptions = {
  /**
   * Where the line being sung sits: in the middle (the lyrics), or leading at the top of the list's
   * place (the phone's lyrics card).
   */
  place?: 'middle' | 'top';
  /** Whether the lines scroll by hand; they then stay where they're put for a few seconds. */
  scrollable?: boolean;
};

/**
 * Keeps the line being sung in its place in `container` (the middle, as Apple Music's lyrics do):
 * it glides there as the line changes (`lyricsGlide`: a move under way bends toward the newer
 * line), goes there at once where things may not move, and starts there without moving when the
 * lines first show or become others (`linesKey` changes: a new song). After the listener scrolls,
 * the lines stay put for a few seconds. Sizes changing (a turned phone, translations coming in)
 * keep it in place. Put `lineRef` on the current line and `listRef` on the list.
 */
export const useFollowLine = <
  Line extends HTMLElement = HTMLButtonElement,
  List extends HTMLElement = HTMLOListElement,
>(
  container: RefObject<HTMLElement | null>,
  current: number,
  linesKey: unknown,
  { place = 'middle', scrollable = true }: FollowOptions = {},
) => {
  const lineRef = useRef<Line>(null);
  const listRef = useRef<List>(null);
  const glide = useRef<Glide | null>(null);
  const heldUntil = useRef(0);
  const placed = useRef<{ key: unknown } | null>(null);

  useLayoutEffect(() => {
    const box = container.current;
    if (!box) return;
    const moving = createGlide(box);
    glide.current = moving;
    // A finger on the lines stops them at once; scrolling them holds them where they're put.
    const stop = () => moving.stop();
    const hold = () => {
      moving.stop();
      heldUntil.current = performance.now() + HOLD_AFTER_SCROLL_MS;
    };
    if (scrollable) {
      box.addEventListener('touchstart', stop, { passive: true });
      box.addEventListener('touchmove', hold, { passive: true });
      box.addEventListener('wheel', hold, { passive: true });
    }
    // Sizes changing move the line from its place: it goes back there.
    const observer =
      typeof ResizeObserver === 'undefined'
        ? null
        : new ResizeObserver(() => {
            if (!placed.current || performance.now() < heldUntil.current) return;
            moving.to(spotFor(box, lineRef.current, listRef.current, place));
          });
    observer?.observe(box);
    if (listRef.current) observer?.observe(listRef.current);
    return () => {
      box.removeEventListener('touchstart', stop);
      box.removeEventListener('touchmove', hold);
      box.removeEventListener('wheel', hold);
      observer?.disconnect();
      moving.stop();
      glide.current = null;
      placed.current = null;
    };
  }, [container, scrollable, place]);

  useLayoutEffect(() => {
    const box = container.current;
    const moving = glide.current;
    if (!box || !moving) return;
    const fresh = placed.current === null || placed.current.key !== linesKey;
    placed.current = { key: linesKey };
    if (fresh) heldUntil.current = 0;
    else if (performance.now() < heldUntil.current) return;
    moving.to(spotFor(box, lineRef.current, listRef.current, place), fresh);
  }, [current, linesKey, container, place]);

  return { lineRef, listRef };
};

/**
 * Where `box` scrolls to put `line` in its place: its middle in the box's middle, or its top where
 * the list's top is. Before the first line (none), the lines rest at the top.
 */
const spotFor = (
  box: HTMLElement,
  line: HTMLElement | null,
  list: HTMLElement | null,
  place: 'middle' | 'top',
) => {
  if (!line) return 0;
  return place === 'top'
    ? line.offsetTop - (list?.offsetTop ?? 0)
    : line.offsetTop - box.clientHeight / 2 + line.offsetHeight / 2;
};
