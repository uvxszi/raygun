import { useEffect, useLayoutEffect, useRef } from 'react';
import type { RefObject } from 'react';

import type { ExtensionList } from '../pages/BrowsePage';
import { isSourceMedia, isSourcesTab } from './sourceMedia';
import type { SourceMedia, SourcesTab } from './sourceMedia';

/**
 * What Sources remembers (M47): the tab it was on last (its kind, what it's for, the
 * catalogue's list), which it opens on again, and where each tab was scrolled this run, which
 * it goes back to when the tab shows again. The page scrolls inside the shell's `<main>`.
 */

/** A tab of Sources as its address names it; none of them is All. */
export type SourcesView = { type?: SourcesTab; media?: SourceMedia; list?: ExtensionList };

/** Where the last tab is kept between runs (a per-device convenience). */
const LAST_VIEW_KEY = 'raygun.sources.view';
/** How long a tab may take to grow back to where it was scrolled. */
const RESTORE_WAIT_MS = 6_000;
/** A spot reached is held this long, in case the page is put at its top after it's drawn. */
const HOLD_MS = 400;
/** How often a restore tries again while the page grows. */
const RETRY_MS = 100;

/** Where each tab was scrolled this run, by [viewKey]. */
const spots = new Map<string, number>();

/** Forgets every tab's place and the last tab (tests). */
export const forgetSourcesMemory = () => {
  spots.clear();
  try {
    localStorage.removeItem(LAST_VIEW_KEY);
  } catch {
    // Storage may be unavailable.
  }
};

/** The key a tab's place is kept under. */
export const viewKey = (view: SourcesView) =>
  [
    view.type ?? 'all',
    view.media ?? '',
    view.type === 'catalogue' ? (view.list ?? 'all') : '',
  ].join('|');

/** Sources opened as it is (All, nothing narrowed). */
const isBare = (view: SourcesView) => !view.type && !view.media && !view.list;

const readLast = (): SourcesView | null => {
  try {
    const parsed = JSON.parse(localStorage.getItem(LAST_VIEW_KEY) ?? 'null') as unknown;
    if (!parsed || typeof parsed !== 'object') return null;
    const { type, media, list } = parsed as Record<string, unknown>;
    return {
      ...(isSourcesTab(type) ? { type } : {}),
      ...(isSourceMedia(media) && type !== 'catalogue' ? { media } : {}),
      ...(type === 'catalogue' && typeof list === 'string' && list !== 'all'
        ? { list: list as ExtensionList }
        : {}),
    };
  } catch {
    return null;
  }
};

const writeLast = (view: SourcesView) => {
  try {
    localStorage.setItem(LAST_VIEW_KEY, JSON.stringify(view));
  } catch {
    // Storage may be unavailable: Sources then opens on All.
  }
};

/**
 * Keeps the tab shown as the last one; Sources opened as it is (from More, Settings) goes to
 * the last one instead (`go`, replacing the address). An address naming a tab, or asking for
 * Add, opens as it says.
 */
export const useLastSourcesTab = (
  view: SourcesView,
  adding: boolean,
  go: (view: SourcesView) => void,
) => {
  const opened = useRef(true);
  const { type, media, list } = view;
  useEffect(() => {
    const shown: SourcesView = {
      ...(type ? { type } : {}),
      ...(media ? { media } : {}),
      ...(list ? { list } : {}),
    };
    if (opened.current) {
      opened.current = false;
      const last = isBare(shown) && !adding ? readLast() : null;
      if (last && !isBare(last)) {
        go(last);
        return;
      }
    }
    writeLast(shown);
  }, [type, media, list, adding, go]);
};

/**
 * Puts `main` back at `top`, again as the tab grows (rows measuring themselves, lists coming in),
 * until it's there for a moment, the person scrolls, taps or presses a key, or it's waited long
 * enough. Returns whether it's still at it, and how to stop it.
 */
const restore = (main: HTMLElement, top: number) => {
  let active = true;
  let frame = 0;
  const began = performance.now();
  const attempt = () => {
    frame = 0;
    if (!active) return;
    if (Math.abs(main.scrollTop - top) > 1) main.scrollTop = top;
    if (Math.abs(main.scrollTop - top) <= 1 && performance.now() - began >= HOLD_MS) stop();
  };
  const observer = new MutationObserver(() => {
    if (!frame) frame = requestAnimationFrame(attempt);
  });
  const timer = window.setInterval(attempt, RETRY_MS);
  const giveUp = window.setTimeout(() => stop(), RESTORE_WAIT_MS);
  const inputs = ['wheel', 'touchstart', 'pointerdown', 'keydown'] as const;
  function stop() {
    active = false;
    observer.disconnect();
    window.clearInterval(timer);
    window.clearTimeout(giveUp);
    if (frame) cancelAnimationFrame(frame);
    for (const input of inputs) window.removeEventListener(input, stop, true);
  }
  for (const input of inputs)
    window.addEventListener(input, stop, { capture: true, passive: true });
  observer.observe(main, { childList: true, subtree: true });
  attempt();
  return { isActive: () => active, stop };
};

/**
 * Remembers where each tab of Sources was scrolled (`key`, [viewKey]) and goes back there when
 * the tab shows again, on a pill or when Sources opens again. `inside` is an element of the
 * page, inside the shell's `<main>`.
 */
export const useTabScroll = (key: string, inside: RefObject<HTMLElement | null>) => {
  useLayoutEffect(() => {
    const main = inside.current?.closest('main');
    if (!main) return;
    const saved = spots.get(key);
    const restoring = saved ? restore(main, saved) : null;
    const onScroll = () => {
      // Its own restoring isn't the person's scrolling.
      if (!restoring?.isActive()) spots.set(key, main.scrollTop);
    };
    main.addEventListener('scroll', onScroll, { passive: true });
    return () => {
      main.removeEventListener('scroll', onScroll);
      restoring?.stop();
    };
  }, [key, inside]);
};
