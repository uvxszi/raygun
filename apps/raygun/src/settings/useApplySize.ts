import { useEffect } from 'react';

import { isWebBuild } from '../lib/webBuild';
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

/** Whether Interface size zooms the window: Raygun's desktop app only. A web page can't zoom
 * itself, so there, as on phones, it scales the root font size. */
export const desktopZooms = (desktop: boolean) => desktop && !isWebBuild();

/**
 * Sizes come from the screen (M46.48): the root font size, which every rem-based size and gap
 * follows, grows with the shorter side (`--short-side`, read by styles.css), from small phones to
 * tablets and desktops. Interface size (`zoom`, percent of that) comes on top: the desktop window
 * zooms; phones and the web scale the root (`--ui-scale`), and icons drawn at a size in px follow
 * the root there (`data-root-sized`, shell/iconSizes). Text size (`textSize`, percent more or less)
 * scales text alone (`--text-scale`).
 */
export const useApplySize = (
  desktop: boolean,
  zoom: number | undefined,
  textSize: number | undefined,
) => {
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
    const zooms = desktopZooms(desktop);
    // Phones' buttons follow the root font size, which the screen sets: icons drawn in px follow
    // it too. The web's desktop layout keeps px icons, scaled by Interface size alone.
    if (desktop) delete root.dataset.rootSized;
    else root.dataset.rootSized = '';
    if (zooms || percent === 100) {
      root.style.removeProperty('--ui-scale');
      if (zooms && zoom) void setAppZoom(percent / 100);
    } else {
      root.style.setProperty('--ui-scale', String(percent / 100));
    }
  }, [desktop, zoom]);

  useEffect(() => {
    const root = document.documentElement;
    const offset = textSize ?? 0;
    if (offset === 0) root.style.removeProperty('--text-scale');
    else root.style.setProperty('--text-scale', String(1 + offset / 100));
  }, [textSize]);
};
