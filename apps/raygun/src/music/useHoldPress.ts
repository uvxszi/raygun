import { useEffect, useRef, useState } from 'react';
import type {
  KeyboardEvent as ReactKeyboardEvent,
  MouseEvent as ReactMouseEvent,
  PointerEvent as ReactPointerEvent,
} from 'react';

import { claimHold } from '../shell/holdClaims';

/** How long a press lasts before it counts as held, ms. */
export const HOLD_MS = 500;

type HoldPress = {
  /** A short press: a click, Enter or Space. */
  onTap: () => void;
  /** A press held for [`HOLD_MS`]: runs while still pressed, and the release does nothing more. */
  onHold: () => void;
  disabled?: boolean;
};

/**
 * A button that does one thing on a tap and another when held, by mouse, pen or finger. A press
 * that leaves the button or turns into a scroll is no hold. `holding` is true while a press
 * waits to count as held (for a fill that shows it coming).
 */
export const useHoldPress = ({ onTap, onHold, disabled = false }: HoldPress) => {
  const timer = useRef<ReturnType<typeof setTimeout> | null>(null);
  // Set when a hold ran: the click its release makes is then no tap.
  const held = useRef(false);
  const actions = useRef({ onTap, onHold });
  const [holding, setHolding] = useState(false);
  useEffect(() => {
    actions.current = { onTap, onHold };
  });

  const stop = () => {
    if (timer.current !== null) clearTimeout(timer.current);
    timer.current = null;
    setHolding(false);
  };
  useEffect(
    () => () => {
      if (timer.current !== null) clearTimeout(timer.current);
    },
    [],
  );

  return {
    holding,
    props: {
      onPointerDown: (event: ReactPointerEvent) => {
        if (disabled || event.button !== 0) return;
        // The hold is the button's own: no name shows for it (shell/ControlLabels).
        claimHold(event);
        stop();
        held.current = false;
        setHolding(true);
        timer.current = setTimeout(() => {
          timer.current = null;
          held.current = true;
          setHolding(false);
          actions.current.onHold();
        }, HOLD_MS);
      },
      onPointerUp: stop,
      onPointerLeave: stop,
      onPointerCancel: stop,
      onClick: (event: ReactMouseEvent) => {
        event.stopPropagation();
        if (held.current) {
          held.current = false;
          return;
        }
        if (!disabled) actions.current.onTap();
      },
      // A long press on a touch screen is the hold, never the webview's own menu.
      onContextMenu: (event: ReactMouseEvent) => event.preventDefault(),
      onKeyDown: (event: ReactKeyboardEvent) => {
        if (event.key !== 'Enter' && event.key !== ' ') return;
        // A press of its own (a hold's release may never have clicked: the button went
        // disabled at the end of the list).
        held.current = false;
        // Enter plays a row elsewhere in it; here it presses the button.
        event.stopPropagation();
      },
    },
  };
};
