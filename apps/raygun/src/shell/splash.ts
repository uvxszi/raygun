import { useEffect } from 'react';

/**
 * The start screen in index.html: Raygun's mark on the dark background, there from the first paint
 * until the shell has drawn its first real screen (the saved answers back, Settings applied, the
 * start screen open), then gone with a quick fade, or at once where motion is reduced or off.
 */
export const SPLASH_ID = 'raygun-splash';

/** The fade's length in index.html; it's taken away a moment after, whatever happened. */
const FADE_MS = 160;

/** A start that never draws (a failed load) doesn't keep the screen covered past this, ms. */
export const SPLASH_LIMIT_MS = 8_000;

/** Whether things may move now: the level in effect on the root, and the browser's setting. */
const mayFade = () => {
  const level = document.documentElement.dataset.motion;
  if (level === 'reduced' || level === 'off') return false;
  return !(
    typeof window.matchMedia === 'function' &&
    window.matchMedia('(prefers-reduced-motion: reduce)').matches
  );
};

/**
 * Takes the start screen away: a quick fade where things may move, at once otherwise or with
 * `fade: false`. Later calls do nothing.
 */
export const hideSplash = ({ fade = true }: { fade?: boolean } = {}) => {
  const splash = document.getElementById(SPLASH_ID);
  if (!splash || splash.dataset.leaving !== undefined) return;
  if (!fade || !mayFade()) {
    splash.remove();
    return;
  }
  splash.dataset.leaving = '';
  const remove = () => splash.remove();
  splash.addEventListener('transitionend', remove, { once: true });
  setTimeout(remove, FADE_MS + 100);
};

/**
 * Takes the start screen away once `ready`, after the next frame has drawn what's under it, so
 * the shell shows complete as it fades out.
 */
export const useHideSplash = (ready: boolean) => {
  useEffect(() => {
    if (!ready) return;
    let frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(() => hideSplash());
    });
    return () => cancelAnimationFrame(frame);
  }, [ready]);
};
