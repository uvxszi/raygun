import { useSyncExternalStore } from 'react';

/** Where the layout turns from phone to desktop: Tailwind's `md`. */
const WIDE = '(min-width: 768px)';

const query = () => (typeof window.matchMedia === 'function' ? window.matchMedia(WIDE) : null);

const subscribe = (onChange: () => void) => {
  const list = query();
  list?.addEventListener('change', onChange);
  return () => list?.removeEventListener('change', onChange);
};

/** Whether the window is wide enough for desktop layouts (the player bar's full controls). */
export const useWideScreen = () =>
  useSyncExternalStore(
    subscribe,
    () => query()?.matches ?? false,
    () => false,
  );
