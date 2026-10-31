import { useEffect, useRef } from 'react';

import { hideVideo, showVideoAt } from '../engine/videoDock';

/**
 * Keeps the video dock over the returned element while `active`. Follows it every frame, so the
 * video tracks the sheet's slide-in animation and drags.
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
      if (surface) {
        showVideoAt(surface.getBoundingClientRect());
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
