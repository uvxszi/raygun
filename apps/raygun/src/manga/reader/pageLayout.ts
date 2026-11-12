import type { PagedScale } from '@raygun/bindings';
import type { CSSProperties } from 'react';

/** A loaded page's shape: its width over its height, and its own width in pixels. */
export type PageShape = { ratio: number; width: number };

/** `zoom` times the reader's screen's width or height (it's a size container), to two places. */
const share = (zoom: number, unit: 'cqw' | 'cqh') => `${Math.round(zoom * 10000) / 100}${unit}`;

/**
 * How a page sits in the paged modes: the chosen fit, made `zoom` times bigger (M46.7). Sizes are
 * the reader's screen's own (`cqw`, `cqh`: the page area is a size container), and the page's box
 * is the picture itself, so a zoom scales it evenly about the point zoomed at, the screen scrolls
 * exactly to the picture's edges, and a picture smaller than the screen sits in its middle. Fit
 * screen never enlarges a picture past its own size at the fit, as before; its shape is needed for
 * that, so until it loads the page is held to the screen.
 */
export const pageLayout = (
  scale: PagedScale,
  zoom: number,
  shape?: PageShape,
): { className: string; style: CSSProperties } => {
  switch (scale) {
    case 'fitWidth':
      return { className: 'h-auto max-w-none', style: { width: share(zoom, 'cqw') } };
    case 'fitHeight':
      return { className: 'w-auto max-w-none', style: { height: share(zoom, 'cqh') } };
    case 'original':
      return { className: 'max-w-none', style: { zoom } };
    default:
      if (shape) {
        const fit = `min(${Math.round(shape.width)}px, 100cqw, 100cqh * ${shape.ratio})`;
        return {
          className: 'h-auto max-w-none',
          style: { width: zoom === 1 ? fit : `calc(${fit} * ${zoom})` },
        };
      }
      return zoom === 1
        ? { className: 'max-h-full max-w-full', style: {} }
        : { className: 'h-auto max-w-none', style: { width: share(zoom, 'cqw') } };
  }
};
