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
 * container shows (the side panel scrolls, M46.8).
 */
export const useVideoSurface = (active: boolean) => {
  const surfaceRef = useRef<HTMLDivElement>(null);
  useEffect(() => {
    if (!active) {
      return;
    }
    let frame = 0;
    const follow = () => {
      const surface = surfaceRef.current;
      if (!surface) {
        hideVideo();
      } else {
        const box = surface.getBoundingClientRect();
        const container = scrollParent(surface)?.getBoundingClientRect();
        const top = Math.max(box.top, container?.top ?? box.top);
        const bottom = Math.min(box.bottom, container?.bottom ?? box.bottom);
        if (bottom <= top) {
          hideVideo();
        } else {
          showVideoAt(box, { top: top - box.top, bottom: box.bottom - bottom });
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
