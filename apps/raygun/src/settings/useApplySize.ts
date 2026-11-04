import { useEffect } from 'react';

import { setAppZoom } from '../shell/appWindow';

/** The shorter side Raygun has: a phone's own (which the keyboard and turning don't change, and
 * split screen may narrow), a desktop window's. */
const shortSide = (desktop: boolean) =>
  desktop
    ? Math.min(window.innerWidth, window.innerHeight)
    : Math.min(
        window.screen.width || Infinity,
        window.screen.height || Infinity,
        window.innerWidth,
      );

/**
 * Sizes come from the screen (M46.48): the root font size, which every rem-based size and gap
 * follows, grows with the shorter side (`--short-side`, read by styles.css), from small phones to
 * tablets and desktops. Interface size (`zoom`, percent of that) comes on top: the desktop window
 * zooms, phones scale the root (`--ui-scale`).
 */
export const useApplySize = (desktop: boolean, zoom: number | undefined) => {
  useEffect(() => {
    const root = document.documentElement;
    let frame = 0;
    const measure = () => {
      cancelAnimationFrame(frame);
      frame = requestAnimationFrame(() => {
        const side = shortSide(desktop);
        if (Number.isFinite(side) && side > 0) {
          root.style.setProperty('--short-side', `${Math.round(side)}px`);
        }
      });
    };
    measure();
    window.addEventListener('resize', measure);
    return () => {
      cancelAnimationFrame(frame);
      window.removeEventListener('resize', measure);
    };
  }, [desktop]);

  useEffect(() => {
    const root = document.documentElement;
    const percent = zoom ?? 100;
    if (desktop) {
      root.style.removeProperty('--ui-scale');
      if (zoom) void setAppZoom(percent / 100);
    } else if (percent === 100) {
      root.style.removeProperty('--ui-scale');
    } else {
      root.style.setProperty('--ui-scale', String(percent / 100));
    }
  }, [desktop, zoom]);
};
