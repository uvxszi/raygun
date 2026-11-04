import { useEffect, useRef } from 'react';

import { hideVideo, showVideoAt } from '../engine/videoDock';

/** The nearest ancestor that scrolls, whose box the video must stay inside. */
const scrollParent = (element: HTMLElement) => {
  for (let node = element.parentElement; node; node = node.parentElement) {
    const { overflowY } = getComputedStyle(node);
    if (overflowY === 'auto' || overflowY === 'scroll') return node;
  }
  return null;
};

/**
 * Keeps the video dock over the returned element while `active`. Follows it every frame, so the
 * video tracks the sheet's slide-in animation and drags, and is cut to what its scrolling
 * container shows (the side panel scrolls, M46.8). Each frame only reads where things are; the
 * dock moves only when that changed, so a still video costs no layout.
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
    const follow = () => {
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
        }
      }
      frame = requestAnimationFrame(follow);
    };
    follow();
    return () => {
      cancelAnimationFrame(frame);
      hideVideo();
    };
  }, [active]);
  return surfaceRef;
};
