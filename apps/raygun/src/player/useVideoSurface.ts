import { useEffect, useRef } from 'react';

import { hideVideo, showVideoAt } from '../engine/videoDock';
import { isAppActive, onAppActiveChange } from '../shell/deviceState';

/** The nearest ancestor that scrolls, whose box the video must stay inside. */
const scrollParent = (element: HTMLElement) => {
  for (let node = element.parentElement; node; node = node.parentElement) {
    const { overflowY } = getComputedStyle(node);
    if (overflowY === 'auto' || overflowY === 'scroll') return node;
  }
  return null;
};

/** Frames without a move after which following stops until something may move it again. */
const STILL_FRAMES = 45;

/**
 * Keeps the video dock over the returned element while `active`. Follows it every frame while it
 * moves, so the video tracks the sheet's slide-in animation and drags, and is cut to what its
 * scrolling container shows (the side panel scrolls, M46.8). Each frame only reads where things
 * are; the dock moves only when that changed. Once it has stood still for a while the frames stop
 * (a video playing for an hour costs no frames) and start again on a scroll, a resize, a pointer,
 * a key or Raygun coming back on screen; none run while Raygun is in the background.
 */
export const useVideoSurface = (active: boolean) => {
  const surfaceRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!active) {
      return;
    }
    let frame = 0;
    // Found once the surface is there: walking up computed styles every frame is costly.
    let container: HTMLElement | null | undefined;
    let placed = '';
    let still = 0;
    const wake = () => {
      still = 0;
      if (!frame && isAppActive()) frame = requestAnimationFrame(follow);
    };
    const follow = () => {
      frame = 0;
      const surface = surfaceRef.current;
      if (!surface) {
        if (placed !== 'hidden') hideVideo();
        placed = 'hidden';
      } else {
        if (container === undefined) container = scrollParent(surface);
        const box = surface.getBoundingClientRect();
        const bounds = container?.getBoundingClientRect();
        const top = Math.max(box.top, bounds?.top ?? box.top);
        const bottom = Math.min(box.bottom, bounds?.bottom ?? box.bottom);
        const clip = { top: top - box.top, bottom: box.bottom - bottom };
        const next =
          bottom <= top
            ? 'hidden'
            : [box.left, box.top, box.width, box.height, clip.top, clip.bottom].join(' ');
        if (next !== placed) {
          if (next === 'hidden') hideVideo();
          else showVideoAt(box, clip);
          placed = next;
          still = 0;
        }
      }
      still += 1;
      if (still < STILL_FRAMES && isAppActive()) frame = requestAnimationFrame(follow);
    };
    follow();
    const options = { capture: true, passive: true } as const;
    document.addEventListener('scroll', wake, options);
    window.addEventListener('resize', wake);
    window.addEventListener('pointermove', wake, options);
    window.addEventListener('pointerup', wake, options);
    window.addEventListener('keydown', wake, options);
    const stopFollowing = onAppActiveChange((now) => {
      if (now) wake();
    });
    const observer = typeof ResizeObserver === 'undefined' ? null : new ResizeObserver(wake);
    if (surfaceRef.current) observer?.observe(surfaceRef.current);
    return () => {
      if (frame) cancelAnimationFrame(frame);
      document.removeEventListener('scroll', wake, options);
      window.removeEventListener('resize', wake);
      window.removeEventListener('pointermove', wake, options);
      window.removeEventListener('pointerup', wake, options);
      window.removeEventListener('keydown', wake, options);
      stopFollowing();
      observer?.disconnect();
      hideVideo();
    };
  }, [active]);
  return surfaceRef;
};
