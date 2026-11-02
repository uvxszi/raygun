import type { CSSProperties } from 'react';

/**
 * Cover grids (M46.7): a fixed number of columns when Settings says so, else as many covers as fit
 * at the chosen width. "By screen" is three across a phone (6.5 rem) and 8 rem from 768 px, set by
 * {@link COVER_GRID_CLASS} on the grid.
 */
export const coverGridStyle = (columns: number, coverWidth: number): CSSProperties => ({
  gridTemplateColumns:
    columns > 0
      ? `repeat(${columns}, minmax(0, 1fr))`
      : `repeat(auto-fill, minmax(${coverWidth > 0 ? `${coverWidth}px` : 'var(--cover-width)'}, 1fr))`,
});

/** The automatic cover width, by screen. */
export const COVER_GRID_CLASS = 'grid gap-2 [--cover-width:6.5rem] md:[--cover-width:8rem]';
