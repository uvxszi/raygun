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
