import { useLayoutEffect, useRef } from 'react';

/**
 * Keeps `--cover-fit` on the returned container: the height (px) its first part has left for the
 * cover once everything else in that part (the controls, the pill) has its room. The cover's own
 * size starts there (styles.css, `.player-cover`), at any window size, zoom, text size and
 * orientation; the player's cover setting moves it from there.
 *
 * Measured, not estimated: the controls are as tall as their text makes them. Writing the value
 * changes only the cover, not what's measured, so it settles at once.
 */
export const useCoverFit = <C extends HTMLElement = HTMLDivElement>() => {
  /** What scrolls or holds the column: its height is what there is. */
  const containerRef = useRef<C>(null);
  /** The part the cover sits in, with the controls under or beside it. */
  const contentRef = useRef<HTMLDivElement>(null);
  /** The cover's own box (with its padding). */
  const coverRef = useRef<HTMLDivElement>(null);

  useLayoutEffect(() => {
    const container = containerRef.current;
    const content = contentRef.current;
    if (!container || !content || typeof ResizeObserver === 'undefined') return;
    let frame = 0;
    let written = '';
    const measure = () => {
      frame = 0;
      const cover = coverRef.current;
      const others = content.offsetHeight - (cover?.offsetHeight ?? 0);
      const style = getComputedStyle(container);
      const padding = Number.parseFloat(style.paddingTop) + Number.parseFloat(style.paddingBottom);
      const fit = Math.max(0, container.clientHeight - (padding || 0) - others);
      const value = `${Math.floor(fit)}px`;
      if (value !== written) {
        written = value;
        container.style.setProperty('--cover-fit', value);
      }
    };
    // In the next frame: a size written while the observer reports would be reported again.
    const later = () => {
      if (!frame) frame = requestAnimationFrame(measure);
    };
    const observer = new ResizeObserver(later);
    observer.observe(container);
    observer.observe(content);
    measure();
    return () => {
      observer.disconnect();
      if (frame) cancelAnimationFrame(frame);
    };
  }, []);

  return { containerRef, contentRef, coverRef };
};
