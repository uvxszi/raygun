/** A point on screen or in a page, or a scroll position, in CSS pixels. */
export type Point = { x: number; y: number };

/**
 * The point of the content under `at` (on screen), in the content's own unzoomed pixels: what a
 * zoom made at `at` keeps under the finger. `box` is the content's top left on screen at `zoom`.
 */
export const contentPoint = (box: Point, at: Point, zoom: number): Point => ({
  x: (at.x - box.x) / zoom,
  y: (at.y - box.y) / zoom,
});

/** A scroll kept within 0 and `max`: never past the page's edges (none when it fits the screen). */
export const clampScroll = (value: number, max: number) =>
  Math.min(Math.max(value, 0), Math.max(max, 0));

export type AnchorAt = {
  /** The scroller's scroll once the content is laid out at the new zoom. */
  scroll: Point;
  /** The content's top left on screen then. */
  box: Point;
  /** The content point to keep (`contentPoint` before the zoom). */
  point: Point;
  /** The new zoom. */
  zoom: number;
  /** Where on screen the point was. */
  at: Point;
  /** The furthest the scroller goes each way (its scroll size less its own). */
  max: Point;
};

/**
 * The scroll that puts the zoomed point back where it was on screen: the point under the finger
 * stays there, unless that would scroll past the page's edges, where the page stops instead. A
 * page smaller than the screen that way sits in its middle (laid out so) and doesn't scroll.
 */
export const anchoredScroll = ({ scroll, box, point, zoom, at, max }: AnchorAt): Point => ({
  x: clampScroll(scroll.x + box.x + point.x * zoom - at.x, max.x),
  y: clampScroll(scroll.y + box.y + point.y * zoom - at.y, max.y),
});
