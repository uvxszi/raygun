/** How far a pointer moves on the seek bar before a press becomes a drag, px. */
export const DRAG_SLOP_PX = 4;

/**
 * Where a drag on the seek bar has got to (seek by dragging): `fromMs` moved by the pointer's
 * travel, the bar's whole `widthPx` being the whole `lengthMs`, kept within the video.
 */
export const dragTarget = (fromMs: number, movedPx: number, widthPx: number, lengthMs: number) => {
  if (lengthMs <= 0) return 0;
  const offset = (movedPx / Math.max(1, widthPx)) * lengthMs;
  return Math.round(Math.min(lengthMs, Math.max(0, fromMs + offset)));
};
