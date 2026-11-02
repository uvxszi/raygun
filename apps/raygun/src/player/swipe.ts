const SWIPE_UP_DISTANCE = 40;
const SWIPE_UP_VELOCITY = 300;
/** A sideways swipe this far (or this fast) changes the song (M46.13). */
const SWIPE_SIDE_DISTANCE = 60;
const SWIPE_SIDE_VELOCITY = 400;

/** What a swipe on the phone's player bar does: left is the next song, right the one before, up
 * opens the full player. */
export const swipeAction = (
  offset: { x: number; y: number },
  velocity: { x: number; y: number },
): 'next' | 'previous' | 'open' | null => {
  if (Math.abs(offset.x) > Math.abs(offset.y)) {
    if (offset.x < -SWIPE_SIDE_DISTANCE || velocity.x < -SWIPE_SIDE_VELOCITY) return 'next';
    if (offset.x > SWIPE_SIDE_DISTANCE || velocity.x > SWIPE_SIDE_VELOCITY) return 'previous';
    return null;
  }
  return offset.y < -SWIPE_UP_DISTANCE || velocity.y < -SWIPE_UP_VELOCITY ? 'open' : null;
};
