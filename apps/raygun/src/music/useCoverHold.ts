import { useEffect, useRef, useState } from 'react';
import type {
  DragEvent as ReactDragEvent,
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
} from 'react';

import { useCoverSheetStore } from './coverSheetStore';
import type { CoverToSave } from './coverSheetStore';

/** How long a cover is held to open its save options, ms: a touch screen's long press. */
export const COVER_HOLD_MS = 500;
/** A press that moves this far (px) is a scroll or a drag, never a hold. */
const MOVE_PX = 10;

type CoverHold = {
  /** The picture whose save options open; `null` (none) leaves only the tap. */
  cover: CoverToSave | null;
  /**
   * What a tap or a plain click does (the phone's player: show the lyrics). Without it a tap does
   * nothing, and Enter or Space open the cover.
   */
  onTap?: () => void;
};

/**
 * A cover or picture whose save options (music/CoverSheet) open when it's held for half a second
 * or right-clicked, never on a plain tap or click: those do `onTap`. While a press may become a
 * hold the picture says so (`data-holding`, styles.css `.cover-hold`: it sinks a little and dims);
 * a press that moves is a scroll and no hold. The context-menu key opens the cover too. Spread
 * `props` on the button (or the picture's box) and give it the `cover-hold` class.
 */
export const useCoverHold = ({ cover, onTap }: CoverHold) => {
  const [holding, setHolding] = useState(false);
  const press = useRef<{ x: number; y: number; timer: ReturnType<typeof setTimeout> } | null>(null);
  // Set when a hold opened the cover: the click its release may make is then no tap.
  const opened = useRef(false);
  const latest = useRef({ cover, onTap });
  useEffect(() => {
    latest.current = { cover, onTap };
  });
  useEffect(
    () => () => {
      if (press.current) clearTimeout(press.current.timer);
    },
    [],
  );

  const release = () => {
    if (press.current) clearTimeout(press.current.timer);
    press.current = null;
    setHolding(false);
  };
  const open = () => {
    const shown = latest.current.cover;
    if (!shown) return;
    navigator.vibrate?.(15);
    useCoverSheetStore.getState().open(shown);
  };

  return {
    holding,
    props: {
      'data-holding': holding ? '' : undefined,
      onPointerDown: (event: ReactPointerEvent<HTMLElement>) => {
        opened.current = false;
        if (!latest.current.cover || event.button !== 0) return;
        release();
        const timer = setTimeout(() => {
          press.current = null;
          setHolding(false);
          opened.current = true;
          open();
        }, COVER_HOLD_MS);
        press.current = { x: event.clientX, y: event.clientY, timer };
        setHolding(true);
      },
      onPointerMove: (event: ReactPointerEvent<HTMLElement>) => {
        const start = press.current;
        if (start && Math.hypot(event.clientX - start.x, event.clientY - start.y) > MOVE_PX) {
          release();
        }
      },
      onPointerUp: release,
      onPointerCancel: release,
      onPointerLeave: release,
      onContextMenu: (event: ReactMouseEvent<HTMLElement>) => {
        // Never the webview's own menu. A right-click opens the cover at once; so does a touch
        // screen's own long press, which may come just before or just after the hold's.
        event.preventDefault();
        if (!latest.current.cover) return;
        const pressing = press.current !== null;
        release();
        if (opened.current) return;
        if (pressing) opened.current = true;
        open();
      },
      onClick: (event: ReactMouseEvent<HTMLElement>) => {
        if (opened.current) {
          opened.current = false;
          event.preventDefault();
          return;
        }
        const tap = latest.current.onTap;
        if (tap) tap();
        // Enter or Space: with nothing to tap for, the keys open the cover.
        else if (event.detail === 0) open();
      },
      onKeyDown: (event: ReactKeyboardEvent<HTMLElement>) => {
        if (event.key === 'Enter' || event.key === ' ') opened.current = false;
      },
      // Held pictures don't start dragging.
      onDragStart: (event: ReactDragEvent<HTMLElement>) => event.preventDefault(),
    },
  };
};
