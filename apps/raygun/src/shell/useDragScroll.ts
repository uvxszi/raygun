import { useCallback } from 'react';
import type { RefCallback, RefObject } from 'react';

import { scrollMotion } from './deviceState';

/** How far a mouse moves sideways, in px, before a press on a row drags it instead of clicking. */
export const DRAG_THRESHOLD_PX = 5;
/** Only the drag's last moves set the glide's speed, ms. */
const SAMPLE_MS = 100;
/** A mouse that rested this long before its release had stopped: no glide, ms. */
const REST_MS = 40;
/** The glide keeps this share of its speed every 16 ms. */
const FRICTION = 0.95;
/** Below this speed the glide stops, px/ms. */
const MIN_SPEED = 0.02;
/** The fastest glide, px/ms. */
const MAX_SPEED = 5;
/** A glide still this fast when the row is pressed again: the press only stops it, px/ms. */
const CATCH_SPEED = 0.25;
/** Presses here start no drag: fields, and the handles dnd-kit drags itself. */
const OWN_PRESS =
  'input, textarea, select, [contenteditable], [aria-roledescription="sortable"], [aria-roledescription="draggable"], [data-no-drag-scroll]';

type Sample = { x: number; at: number };

type Press = {
  pointerId: number;
  x: number;
  scrollLeft: number;
  dragging: boolean;
  /** The press stopped a fast glide: like a finger on a flung list, it isn't a click. */
  caught: boolean;
  samples: Sample[];
};

/**
 * The mouse's speed over its last moves before `releasedAt`, in px/ms (rightwards positive), or 0
 * when it had stopped before letting go.
 */
export const releaseSpeed = (samples: Sample[], releasedAt: number) => {
  const recent = samples.filter((sample) => releasedAt - sample.at <= SAMPLE_MS);
  const first = recent[0];
  const last = recent[recent.length - 1];
  if (!first || !last || last === first || releasedAt - last.at > REST_MS) return 0;
  const elapsed = last.at - first.at;
  if (elapsed <= 0) return 0;
  return Math.max(-MAX_SPEED, Math.min(MAX_SPEED, (last.x - first.x) / elapsed));
};

/** Whether `row` has more to show than fits. */
const overflows = (row: HTMLElement) => row.scrollWidth > row.clientWidth + 1;

/**
 * Whether a finger going left from `target` scrolls a row on (it has more to show to the right),
 * so the swipe is the row's and not an edge pull's.
 */
export const swipeScrollsRow = (target: EventTarget | null) => {
  const row = target instanceof Element ? target.closest<HTMLElement>('[data-drag-scroll]') : null;
  if (!row || !overflows(row)) return false;
  // Right to left, the start is at 0 and the rest has negative offsets.
  const end = getComputedStyle(row).direction === 'rtl' ? 0 : row.scrollWidth - row.clientWidth;
  return row.scrollLeft < end - 1;
};

/** Swallows the click a drag's release makes, before anything else hears of it. */
const swallowNextClick = () => {
  const stop = () => {
    clearTimeout(timer);
    window.removeEventListener('click', swallow, true);
  };
  const swallow = (event: MouseEvent) => {
    event.preventDefault();
    event.stopImmediatePropagation();
    stop();
  };
  window.addEventListener('click', swallow, true);
  // The click comes in the same task as the release; a later one is a click of its own.
  const timer = setTimeout(stop, 0);
};

/**
 * Lets a mouse drag `row` sideways, as a finger does: past [DRAG_THRESHOLD_PX] the row follows the
 * mouse, the click the release would make is swallowed, and the row glides on and slows once let
 * go (not where motion is reduced or off). Shorter presses stay clicks; fields and dnd-kit's
 * handles keep their presses. While dragging, the page shows a closed hand and selects nothing
 * (`data-drag-scrolling` on the root); a row with more to show shows an open hand
 * (`data-drag-scroll="more"`). Fingers and pens keep the browser's own scrolling, with its momentum
 * and the row's snapping. Returns what detaches it.
 */
export const attachDragScroll = (row: HTMLElement) => {
  const root = document.documentElement;
  let press: Press | null = null;
  let frame = 0;
  let speed = 0;
  /** The row's own snapping while a drag or glide holds it off, else null. */
  let heldSnap: string | null = null;

  row.dataset.dragScroll = '';

  // Snapping would pull the row back to a card at every step.
  const holdSnap = () => {
    if (heldSnap !== null) return;
    heldSnap = row.style.scrollSnapType;
    row.style.scrollSnapType = 'none';
  };
  const releaseSnap = () => {
    if (heldSnap === null) return;
    row.style.scrollSnapType = heldSnap;
    heldSnap = null;
  };

  const stopGlide = () => {
    if (frame) cancelAnimationFrame(frame);
    frame = 0;
    speed = 0;
  };

  const glide = (start: number) => {
    speed = start;
    let last: number | null = null;
    let left: number | null = null;
    const step = (now: number) => {
      const before = row.scrollLeft;
      // Something else moved the row meanwhile (its page buttons, keys, a jump to a card): it wins.
      if (left !== null && Math.abs(before - left) > 1) {
        stopGlide();
        releaseSnap();
        return;
      }
      const elapsed = last === null ? 16 : Math.min(Math.max(now - last, 0), 64);
      last = now;
      row.scrollLeft = before - speed * elapsed;
      left = row.scrollLeft;
      speed *= FRICTION ** (elapsed / 16);
      // Slow enough, or the row's end.
      if (Math.abs(speed) < MIN_SPEED || row.scrollLeft === before) {
        stopGlide();
        releaseSnap();
        return;
      }
      frame = requestAnimationFrame(step);
    };
    frame = requestAnimationFrame(step);
  };

  const end = () => {
    if (press?.dragging) delete root.dataset.dragScrolling;
    press = null;
    window.removeEventListener('pointermove', onMove, true);
    window.removeEventListener('pointerup', onUp, true);
    window.removeEventListener('pointercancel', onCancel, true);
  };

  const onMove = (event: PointerEvent) => {
    if (!press || event.pointerId !== press.pointerId) return;
    // The button came up where the page didn't hear it.
    if ((event.buttons & 1) === 0) {
      end();
      releaseSnap();
      return;
    }
    const moved = event.clientX - press.x;
    if (!press.dragging) {
      if (Math.abs(moved) < DRAG_THRESHOLD_PX) return;
      press.dragging = true;
      holdSnap();
      root.dataset.dragScrolling = '';
      window.getSelection()?.removeAllRanges();
      // The row keeps the mouse while it drags: cards under it don't light up as it passes.
      try {
        row.setPointerCapture(event.pointerId);
      } catch {
        // The pointer is gone already, or there's no capture (tests).
      }
    }
    row.scrollLeft = press.scrollLeft - moved;
    press.samples.push({ x: event.clientX, at: event.timeStamp });
    if (press.samples.length > 32) press.samples.shift();
  };

  const onUp = (event: PointerEvent) => {
    if (!press || event.pointerId !== press.pointerId) return;
    const { dragging, caught, samples } = press;
    end();
    if (dragging || caught) swallowNextClick();
    const start = dragging ? releaseSpeed(samples, event.timeStamp) : 0;
    if (Math.abs(start) >= MIN_SPEED && scrollMotion() === 'smooth') glide(start);
    else releaseSnap();
  };

  const onCancel = (event: PointerEvent) => {
    if (!press || event.pointerId !== press.pointerId) return;
    end();
    releaseSnap();
  };

  const onDown = (event: PointerEvent) => {
    const target = event.target instanceof Element ? event.target : null;
    // A row inside another row drags itself.
    if (!target || target.closest('[data-drag-scroll]') !== row) return;
    // Any press stops a glide.
    const caught = frame !== 0 && Math.abs(speed) >= CATCH_SPEED;
    if (frame) {
      stopGlide();
      releaseSnap();
    }
    if (event.pointerType !== 'mouse' || event.button !== 0) return;
    end();
    if (target.closest(OWN_PRESS) || !overflows(row)) return;
    press = {
      pointerId: event.pointerId,
      x: event.clientX,
      scrollLeft: row.scrollLeft,
      dragging: false,
      caught,
      samples: [],
    };
    window.addEventListener('pointermove', onMove, true);
    window.addEventListener('pointerup', onUp, true);
    window.addEventListener('pointercancel', onCancel, true);
  };

  // Links and pictures would start the browser's own drag, which ends the mouse's moves.
  const onDragStart = (event: DragEvent) => {
    if (press) event.preventDefault();
  };
  const onSelectStart = (event: Event) => {
    if (press) event.preventDefault();
  };
  // The wheel or a trackpad takes over from a glide.
  const onWheel = () => {
    if (!frame) return;
    stopGlide();
    releaseSnap();
  };
  const onOver = (event: PointerEvent) => {
    if (event.pointerType !== 'mouse') return;
    row.dataset.dragScroll = overflows(row) ? 'more' : '';
  };

  row.addEventListener('pointerdown', onDown);
  row.addEventListener('pointerover', onOver);
  row.addEventListener('dragstart', onDragStart);
  row.addEventListener('selectstart', onSelectStart);
  row.addEventListener('wheel', onWheel, { passive: true });
  return () => {
    stopGlide();
    end();
    releaseSnap();
    row.removeEventListener('pointerdown', onDown);
    row.removeEventListener('pointerover', onOver);
    row.removeEventListener('dragstart', onDragStart);
    row.removeEventListener('selectstart', onSelectStart);
    row.removeEventListener('wheel', onWheel);
    delete row.dataset.dragScroll;
  };
};

/**
 * A ref for a row that scrolls sideways (chips, tabs, cards), which then drags with a mouse
 * ([attachDragScroll]). `rowRef`, when given, points at the row too.
 */
export const useDragScroll = <T extends HTMLElement>(
  rowRef?: RefObject<T | null>,
): RefCallback<T> =>
  useCallback(
    (row: T | null) => {
      if (rowRef) rowRef.current = row;
      if (!row) return;
      const detach = attachDragScroll(row);
      return () => {
        detach();
        if (rowRef) rowRef.current = null;
      };
    },
    [rowRef],
  );
