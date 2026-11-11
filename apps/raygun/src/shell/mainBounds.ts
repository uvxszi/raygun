import { useEffect } from 'react';
import type { RefObject } from 'react';

/**
 * Where the page area (`<main>`) sits in the window, as CSS variables on the shell's root:
 * `--main-left` and `--main-right` (its distance from each side of the window) and `--main-width`.
 * Overlays fixed to the window (the readers' bars) use them to cover only the page area, never
 * the rail or the side panel; they follow window and panel resizes. Without a page area (tests),
 * the overlays fall back to the whole window (`var(--main-left,0px)`).
 */
export const useMainBounds = (
  rootRef: RefObject<HTMLElement | null>,
  mainRef: RefObject<HTMLElement | null>,
) => {
  useEffect(() => {
    const root = rootRef.current;
    const main = mainRef.current;
    if (!root || !main) return;
    const measure = () => {
      const rect = main.getBoundingClientRect();
      // Not laid out (hidden, or no layout at all): the whole window stays the fallback.
      if (rect.width === 0) return;
      const viewport = document.documentElement.clientWidth || window.innerWidth;
      const left = Math.max(0, Math.round(rect.left));
      // The page area's own scrollbar stays clear of overlays too.
      const width = Math.round(main.clientWidth || rect.width);
      const right = Math.max(0, Math.round(viewport - left - width));
      root.style.setProperty('--main-left', `${left}px`);
      root.style.setProperty('--main-right', `${right}px`);
      root.style.setProperty('--main-width', `${width}px`);
    };
    measure();
    window.addEventListener('resize', measure);
    // The rail and the side panel resize <main> without resizing the window.
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(measure);
    observer?.observe(main);
    return () => {
      window.removeEventListener('resize', measure);
      observer?.disconnect();
      root.style.removeProperty('--main-left');
      root.style.removeProperty('--main-right');
      root.style.removeProperty('--main-width');
    };
  }, [rootRef, mainRef]);
};
