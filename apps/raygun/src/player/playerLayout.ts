import type { PlayerLayout } from '@raygun/bindings';
import { useSyncExternalStore } from 'react';

import { useIsDesktop } from '../shell/useIsDesktop';
import { useIsPhone } from '../shell/useIsPhone';
import { useMediaQuery } from '../shell/useWideScreen';

/** The root font size in px: phones scale it with Settings → Interface size (M46.24). */
const rootPx = () => {
  const size = Number.parseFloat(getComputedStyle(document.documentElement).fontSize);
  return Number.isFinite(size) && size > 0 ? size : 16;
};

const subscribeRoot = (onChange: () => void) => {
  const observer = typeof MutationObserver === 'undefined' ? null : new MutationObserver(onChange);
  observer?.observe(document.documentElement, { attributes: true, attributeFilter: ['style'] });
  window.addEventListener('resize', onChange);
  return () => {
    observer?.disconnect();
    window.removeEventListener('resize', onChange);
  };
};

/**
 * One rem in px, following the interface size. Media queries measure rem against the browser's
 * default instead, so on a phone zoomed to 200 % they'd lay a 400 px screen out as if it had
 * 800 px: the player builds its queries from this.
 */
export const useRootPx = () => useSyncExternalStore(subscribeRoot, rootPx, () => 16);

/**
 * Wide enough for the desktop's player bar and the side panel: 48 rem across, and 30 rem tall
 * unless on desktop, so a phone on its side keeps the phone's bar.
 */
export const usePlayerWide = () => {
  const px = useRootPx();
  const desktop = useIsDesktop();
  const across = useMediaQuery(`(min-width: ${Math.round(48 * px)}px)`);
  const tall = useMediaQuery(`(min-height: ${Math.round(30 * px)}px)`);
  // A phone keeps its own bar on its side, whatever the interface size.
  const phone = useIsPhone();
  return across && (desktop || tall) && !phone;
};

/**
 * How the full player arranges itself: `sideBySide` (cover and controls beside the lyrics or
 * queue), `landscape` (a phone on its side or a short window: the cover, lyrics or queue beside
 * the controls) or `stacked` (one column that scrolls).
 */
export type SheetLayout = 'sideBySide' | 'landscape' | 'stacked';

export const useSheetLayout = (layout: PlayerLayout): SheetLayout => {
  const px = useRootPx();
  const wide = usePlayerWide();
  const large = useMediaQuery(`(min-width: ${Math.round(64 * px)}px)`);
  const short = useMediaQuery(
    `(orientation: landscape) and (max-height: ${Math.round(36 * px)}px)`,
  );
  if (layout === 'stacked') return 'stacked';
  if (wide && (layout === 'sideBySide' || large)) return 'sideBySide';
  return short ? 'landscape' : 'stacked';
};
