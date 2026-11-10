const SWIPE_UP_DISTANCE = 40;
const SWIPE_UP_VELOCITY = 300;
/** Down this far (or this fast) closes the player (M46.24). */
const SWIPE_DOWN_DISTANCE = 40;
const SWIPE_DOWN_VELOCITY = 300;
/** A sideways swipe this far (or this fast) changes the song (M46.13). */
const SWIPE_SIDE_DISTANCE = 60;
const SWIPE_SIDE_VELOCITY = 400;

/** What a swipe on the phone's player bar does: left is the next song, right the one before, up
 * opens the full player, down closes the player. */
export const swipeAction = (
  offset: { x: number; y: number },
  velocity: { x: number; y: number },
): 'next' | 'previous' | 'open' | 'close' | null => {
  if (Math.abs(offset.x) > Math.abs(offset.y)) {
    if (offset.x < -SWIPE_SIDE_DISTANCE || velocity.x < -SWIPE_SIDE_VELOCITY) return 'next';
    if (offset.x > SWIPE_SIDE_DISTANCE || velocity.x > SWIPE_SIDE_VELOCITY) return 'previous';
    return null;
  }
  if (offset.y < -SWIPE_UP_DISTANCE || velocity.y < -SWIPE_UP_VELOCITY) return 'open';
  if (offset.y > SWIPE_DOWN_DISTANCE || velocity.y > SWIPE_DOWN_VELOCITY) return 'close';
  return null;
};

/** Dragged down this far (px) or this fast (px/s), the full player closes. */
const DISMISS_DISTANCE = 120;
const DISMISS_VELOCITY = 500;

/** Whether a drag of the full player let go here closes it. */
export const sheetDismissed = (offset: { y: number }, velocity: { y: number }) =>
  offset.y > DISMISS_DISTANCE || velocity.y > DISMISS_VELOCITY;

/** A drag left held longer than this (ms) lets the player's live values go anyway. */
export const SHEET_HOLD_LIMIT_MS = 10_000;

/**
 * The full player's drag and slide, frame by frame only a transform of the sheet (its own layer,
 * `will-change`): from a drag's start until the sheet is back in place, and from the moment it
 * starts sliding away until it has gone, the live values in it (seek bar, lyrics, progress) hold
 * where they are (`hold`, which returns what lets them go), so no frame re-renders anything. The
 * sheet's motion calls each step: `start` as a drag starts, `end` as it's let go (saying whether
 * that closes the player), `settled` once it's back in place, `leaving` as it starts sliding away
 * (however it was closed), `gone` once it has, or is taken down.
 */
export const sheetDrag = (hold: () => () => void) => {
  let release: (() => void) | null = null;
  let closing = false;
  let limit: ReturnType<typeof setTimeout> | undefined;
  const letGo = () => {
    clearTimeout(limit);
    const held = release;
    release = null;
    closing = false;
    held?.();
  };
  const holdNow = () => {
    if (release) return;
    release = hold();
    limit = setTimeout(letGo, SHEET_HOLD_LIMIT_MS);
  };
  return {
    start: holdNow,
    /** Let go: far or fast enough closes the player, and the values hold until it has gone. */
    end: (offset: { y: number }, velocity: { y: number }) => {
      const dismissed = sheetDismissed(offset, velocity);
      if (dismissed) closing = true;
      return dismissed;
    },
    settled: () => {
      if (!closing) letGo();
    },
    leaving: () => {
      holdNow();
      closing = true;
    },
    gone: letGo,
    /** Whether the live values are held for it now. */
    holding: () => release !== null,
  };
};

export type SheetDrag = ReturnType<typeof sheetDrag>;

/** A press on the player's handle moving less than this (px) ... */
const TAP_SLOP = 8;
/** ... and let go sooner than this (ms) is a tap: it closes the player. */
const TAP_TIME_MS = 400;

/**
 * Whether a press on the full player's handle was a tap rather than a drag: a short drag that
 * snaps back moved too far, a long press held too long.
 */
export const isHandleTap = (
  from: { x: number; y: number; at: number },
  to: { x: number; y: number; at: number },
) => Math.hypot(to.x - from.x, to.y - from.y) < TAP_SLOP && to.at - from.at < TAP_TIME_MS;
