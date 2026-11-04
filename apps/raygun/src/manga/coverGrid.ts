import type { CSSProperties } from 'react';

/**
 * Cover grids (M46.7; sized by the screen since M46.48): as many covers as fit across, each the
 * screen's own width (6.5 rem on phones, 8 rem from 768 px, set by {@link COVER_GRID_CLASS}; rem
 * follows the screen too) made `coverSize` percent bigger or smaller (Settings → Library).
 */
export const coverGridStyle = (coverSize: number): CSSProperties => ({
  gridTemplateColumns: `repeat(auto-fill, minmax(min(100%, calc(var(--cover-width) * ${1 + coverSize / 100})), 1fr))`,
});

/** The screen's own cover width. */
export const COVER_GRID_CLASS = 'grid gap-2 [--cover-width:6.5rem] md:[--cover-width:8rem]';
