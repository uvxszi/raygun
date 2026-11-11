/**
 * The shade behind the desktop player's controls (as `PlayerVignette.kt` draws it on Android):
 * the picture darkens towards its top and bottom edges and a little towards the corners. Each
 * part fades out along an eased curve that ends flat, so no line shows where it stops, and its
 * stops are shares of the player, so it stretches with the window in any shape.
 */

/** How dark the shade gets at the top edge, the bottom edge and the corners (0 to 1). */
const TOP_ALPHA = 0.6;
const BOTTOM_ALPHA = 0.7;
const CORNER_ALPHA = 0.25;
/** How far each edge's shade reaches, in percent of the player's height. */
const TOP_REACH = 28;
const BOTTOM_REACH = 36;
/** Where the corners start to darken, in percent of the way from the middle to a corner. */
const CORNER_START = 50;
const STOPS = 12;

/** 1 at `t` = 0, 0 at 1, flat at both ends. */
export const easedOut = (t: number) => {
  const x = Math.min(1, Math.max(0, t));
  return 1 - x * x * (3 - 2 * x);
};

const black = (alpha: number) => `rgba(0, 0, 0, ${Number(alpha.toFixed(3))})`;

/** Stops from the edge (`alpha`) to nothing at `reach` %, along [easedOut]. */
const edgeStops = (alpha: number, reach: number) =>
  Array.from({ length: STOPS }, (_, index) => {
    const t = index / (STOPS - 1);
    return `${black(alpha * easedOut(t))} ${Number((reach * t).toFixed(2))}%`;
  }).join(', ');

/** Stops from nothing at `start` % to `alpha` at the corners (the ellipse through them). */
const cornerStops = (alpha: number, start: number) =>
  Array.from({ length: STOPS }, (_, index) => {
    const t = index / (STOPS - 1);
    return `${black(alpha * (1 - easedOut(t)))} ${Number((start + (100 - start) * t).toFixed(2))}%`;
  }).join(', ');

/** The shade as CSS backgrounds: both edges over the corners. */
export const VIGNETTE = [
  `linear-gradient(to bottom, ${edgeStops(TOP_ALPHA, TOP_REACH)})`,
  `linear-gradient(to top, ${edgeStops(BOTTOM_ALPHA, BOTTOM_REACH)})`,
  // An ellipse with the player's own shape, reaching its corners.
  `radial-gradient(ellipse farthest-corner at 50% 50%, ${cornerStops(CORNER_ALPHA, CORNER_START)})`,
].join(', ');
