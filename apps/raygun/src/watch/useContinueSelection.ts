import { useCallback, useEffect, useMemo, useRef, useState } from 'react';
import type { MouseEvent, PointerEvent } from 'react';

import { useBackLayer } from '../shell/backLayers';
import { claimHold } from '../shell/holdClaims';

/** How long a finger holds an entry before it's picked, ms. */
const HOLD_MS = 450;
/** How far a finger may move during the hold before it counts as a scroll, px. */
const HOLD_SLOP = 10;

/**
 * Entries of Continue watching or the watch history picked to be removed together, by their
 * keys. Anything picked turns picking on: taps then pick instead of playing or opening, and Back
 * or Escape drops the selection.
 */
export const useContinueSelection = (keys: string[]) => {
  const [picked, setPicked] = useState<ReadonlySet<string>>(() => new Set());
  // Where a Shift-click's range starts: the entry picked last.
  const anchor = useRef<string | null>(null);
  // Entries that left the list (removed, finished elsewhere) aren't acted on.
  const selected = useMemo(() => keys.filter((key) => picked.has(key)), [keys, picked]);
  const selecting = selected.length > 0;
  const clear = useCallback(() => {
    anchor.current = null;
    setPicked(new Set());
  }, []);
  useBackLayer(selecting, clear);

  /** Picks an entry or puts it back; `range` picks everything from the entry picked last. */
  const pick = (key: string, range = false) => {
    const from = anchor.current === null ? -1 : keys.indexOf(anchor.current);
    const at = keys.indexOf(key);
    setPicked((current) => {
      const next = new Set(current);
      if (range && from >= 0 && at >= 0) {
        for (const each of keys.slice(Math.min(from, at), Math.max(from, at) + 1)) next.add(each);
      } else if (next.has(key)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
    anchor.current = key;
  };
  const allSelected = keys.length > 0 && selected.length === keys.length;
  return {
    selecting,
    selected,
    isSelected: (key: string) => picked.has(key),
    pick,
    allSelected,
    /** Select all, or none once all are. */
    toggleAll: () => setPicked(allSelected ? new Set() : new Set(keys)),
    clear,
  };
};

export type ContinueSelection = ReturnType<typeof useContinueSelection>;

/**
 * What makes an entry pickable, for its outer element: a finger held on it, or a click with Ctrl,
 * Cmd or Shift (a range), picks it; while `selecting`, any tap on it picks, before its link and
 * buttons see the click. A finger's long press is never the webview's own menu.
 */
export const usePickGestures = (selecting: boolean, onPick: (range: boolean) => void) => {
  const hold = useRef<{ timer: ReturnType<typeof setTimeout>; x: number; y: number } | null>(null);
  // The hold picked it: the click (and the touch's context menu) that follow are swallowed.
  const held = useRef(false);
  const touch = useRef(false);
  const latest = useRef(onPick);
  useEffect(() => {
    latest.current = onPick;
  });

  const cancelHold = () => {
    if (hold.current) clearTimeout(hold.current.timer);
    hold.current = null;
  };
  useEffect(
    () => () => {
      if (hold.current) clearTimeout(hold.current.timer);
    },
    [],
  );

  const pickHeld = () => {
    held.current = true;
    latest.current(false);
    navigator.vibrate?.(10);
  };

  return {
    onPointerDown: (event: PointerEvent) => {
      touch.current = event.pointerType !== 'mouse';
      held.current = false;
      if (!touch.current) return;
      cancelHold();
      claimHold(event);
      hold.current = {
        timer: setTimeout(() => {
          hold.current = null;
          pickHeld();
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
    onPointerUp: cancelHold,
    onPointerCancel: cancelHold,
    onPointerLeave: cancelHold,
    onClickCapture: (event: MouseEvent) => {
      const modified = event.ctrlKey || event.metaKey || event.shiftKey;
      if (held.current) {
        held.current = false;
      } else if (selecting || modified) {
        latest.current(event.shiftKey);
      } else {
        return;
      }
      event.preventDefault();
      event.stopPropagation();
    },
    onContextMenuCapture: (event: MouseEvent) => {
      if (!touch.current) return;
      event.preventDefault();
      event.stopPropagation();
      if (!held.current) {
        cancelHold();
        pickHeld();
      }
    },
  };
};
