import { useEffect, useRef } from 'react';
import type { DragEvent, MouseEvent, PointerEvent } from 'react';

import { HOLD_MS } from './scopeContext';

/** How far a finger may move during the hold before it counts as a scroll, px. */
const HOLD_SLOP = 10;

/** After a hold's release, how long the click it may still make is waited for, ms. */
const HOLD_CLICK_MS = 400;

/**
 * Controls inside an item that keep their own presses: its ⋮, play button and circle. A row that
 * is itself a button marks that button `data-pick-main`, and it picks like the rest of the row.
 */
const CONTROLS = 'button, [role="button"], [role="checkbox"], input, select, textarea';

/** What an item needs to be picked. */
export type PickState = {
  selected: boolean;
  /** Something is picked: a tap picks instead of opening. */
  selecting: boolean;
  /** Its circle, a Ctrl or ⌘-click, or a tap while picking: adds it or takes it out. */
  toggle: () => void;
  /**
   * A finger held on it, a right click or a Shift-click: picks it, with everything since the last
   * pick where the list picks ranges.
   */
  extend: () => void;
};

const onOwnControl = (item: Element, target: EventTarget | null) => {
  const control = target instanceof Element ? target.closest(CONTROLS) : null;
  return (
    control !== null &&
    control !== item &&
    item.contains(control) &&
    !control.hasAttribute('data-pick-main')
  );
};

const swallow = (event: MouseEvent) => {
  event.preventDefault();
  event.stopPropagation();
};

/**
 * Handlers that make an element pickable. A finger held on it, a right click or a Shift-click
 * picks it (`extend`), a Ctrl or ⌘-click toggles it, and while anything is picked a tap toggles
 * instead of opening. Its own controls keep their presses. The hold is timed here rather than left
 * to the webview, whose long press on a link or a picture starts dragging it and never reports the
 * press; the drag is refused too.
 */
export const usePickPress = (pick: PickState) => {
  const latest = useRef(pick);
  useEffect(() => {
    latest.current = pick;
  });
  const hold = useRef<{ timer: ReturnType<typeof setTimeout>; x: number; y: number } | null>(null);
  // The hold picked it: the click (and the touch's context menu) that may follow are swallowed.
  const held = useRef(false);
  const heldTimer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const touch = useRef(false);

  const cancelHold = () => {
    if (hold.current) clearTimeout(hold.current.timer);
    hold.current = null;
  };
  useEffect(
    () => () => {
      cancelHold();
      clearTimeout(heldTimer.current);
    },
    [],
  );

  const pickByHold = () => {
    held.current = true;
    latest.current.extend();
    navigator.vibrate?.(10);
  };

  return {
    onPointerDown: (event: PointerEvent<HTMLElement>) => {
      touch.current = event.pointerType !== 'mouse';
      held.current = false;
      clearTimeout(heldTimer.current);
      cancelHold();
      if (!touch.current || onOwnControl(event.currentTarget, event.target)) return;
      hold.current = {
        timer: setTimeout(() => {
          hold.current = null;
          pickByHold();
        }, HOLD_MS),
        x: event.clientX,
        y: event.clientY,
      };
    },
    onPointerMove: (event: PointerEvent) => {
      const start = hold.current;
      if (start && Math.hypot(event.clientX - start.x, event.clientY - start.y) > HOLD_SLOP) {
        cancelHold();
      }
    },
    onPointerUp: () => {
      cancelHold();
      // A release a little after the hold may still click; a later click is a click of its own.
      if (held.current) {
        clearTimeout(heldTimer.current);
        heldTimer.current = setTimeout(() => {
          held.current = false;
        }, HOLD_CLICK_MS);
      }
    },
    onPointerCancel: cancelHold,
    onPointerLeave: cancelHold,
    onClickCapture: (event: MouseEvent<HTMLElement>) => {
      if (held.current) {
        held.current = false;
        swallow(event);
        return;
      }
      if (onOwnControl(event.currentTarget, event.target)) return;
      const { selecting, toggle, extend } = latest.current;
      if (event.shiftKey) extend();
      else if (selecting || event.ctrlKey || event.metaKey) toggle();
      else return;
      swallow(event);
    },
    // A finger's long press (Android reports it too) or a right click picks; the webview's own
    // menu never shows.
    onContextMenuCapture: (event: MouseEvent<HTMLElement>) => {
      if (onOwnControl(event.currentTarget, event.target)) return;
      swallow(event);
      if (held.current) return;
      cancelHold();
      if (touch.current) pickByHold();
      else latest.current.extend();
    },
    onDragStart: (event: DragEvent) => event.preventDefault(),
  };
};
