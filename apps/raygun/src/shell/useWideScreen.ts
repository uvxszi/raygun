import { useSyncExternalStore } from 'react';

const list = (query: string) =>
  typeof window.matchMedia === 'function' ? window.matchMedia(query) : null;

/** Whether `query` matches the window now, following changes. False where there's no matchMedia. */
export const useMediaQuery = (query: string) =>
  useSyncExternalStore(
    (onChange) => {
      const media = list(query);
      media?.addEventListener('change', onChange);
      return () => media?.removeEventListener('change', onChange);
    },
    () => list(query)?.matches ?? false,
    () => false,
  );

/** Whether the window is wide enough for desktop layouts (the player bar's full controls): `md`. */
export const useWideScreen = () => useMediaQuery('(min-width: 768px)');

/** Whether the side rail replaces the bottom tabs: Tailwind's `lg`. */
export const useSideRail = () => useMediaQuery('(min-width: 1024px)');

/** The window's width in px, following changes (0 where there's no window size, as in tests). */
export const useWindowWidth = () =>
  useSyncExternalStore(
    (onChange) => {
      window.addEventListener('resize', onChange);
      return () => window.removeEventListener('resize', onChange);
    },
    () => window.innerWidth,
    () => 0,
  );
