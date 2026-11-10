/** How far (px) a finger moves before its direction says what it's for. */
export const INTENT_SLOP_PX = 8;
/** Half a slider's thumb (px): the track runs this far in from the input's ends. */
const THUMB_HALF_PX = 8;

/** A slider's direction: `y` is vertical, its lowest value at the bottom. */
export type SliderAxis = 'x' | 'y';

/**
 * What a finger that moved (`dx`, `dy`) from where it pressed means for a slider along `axis`:
 * `slide` once it clearly moves along it, `pass` once it clearly moves across it (the page
 * scrolling, a swipe), null while it hasn't moved far enough to say.
 */
export const dragIntent = (dx: number, dy: number, axis: SliderAxis): 'slide' | 'pass' | null => {
  const along = Math.abs(axis === 'y' ? dy : dx);
  const across = Math.abs(axis === 'y' ? dx : dy);
  if (Math.max(along, across) < INTENT_SLOP_PX) return null;
  return along > across ? 'slide' : 'pass';
};

type Box = { left: number; top: number; width: number; height: number };

/**
 * How far along a slider's track a point is, 0–1: from the bottom of a vertical one, from the
 * start of a horizontal one (its right end in a right-to-left layout).
 */
export const shareAlong = (
  point: { x: number; y: number },
  box: Box,
  axis: SliderAxis,
  rtl = false,
) => {
  const length = (axis === 'y' ? box.height : box.width) - THUMB_HALF_PX * 2;
  if (length <= 0) return 0;
  const offset =
    axis === 'y'
      ? box.top + box.height - THUMB_HALF_PX - point.y
      : rtl
        ? box.left + box.width - THUMB_HALF_PX - point.x
        : point.x - box.left - THUMB_HALF_PX;
  return Math.min(1, Math.max(0, offset / length));
};

/** The slider's value `share` of the way from `min` to `max`, on its steps. */
export const valueAlong = (share: number, min: number, max: number, step: number) => {
  const stepped = min + Math.round((share * (max - min)) / step) * step;
  return Math.min(max, Math.max(min, stepped));
};
