import { useRouter } from '@tanstack/react-router';
import type { HistoryLocation, RouterHistory } from '@tanstack/react-router';
import { useEffect, useRef } from 'react';
import type { RefObject } from 'react';

/**
 * Where each page was scrolled, per history entry: Back and Forward return to it, a new page
 * starts at the top. Pages scroll inside the shell's `<main>`; rows and lists that scroll on their
 * own inside it (carousels, tables) are remembered too, by their `data-scroll-key` or else by
 * their place in the page. A page still loading (or redrawn from a cached answer that grows) is
 * scrolled further as it grows, until the spot is reached, the person scrolls, taps or presses a
 * key, or `RESTORE_WAIT_MS` passes. Lists drawn a chunk at a time draw their next chunk as their
 * end comes near, so scrolling to the end of what's drawn draws further.
 */

/** Names a scroller inside a page, for pages with more than one or whose rows load in any order. */
export const SCROLL_KEY_ATTRIBUTE = 'data-scroll-key';

/** How long a page may take to grow back to where it was scrolled. */
export const RESTORE_WAIT_MS = 10_000;

/** How often a restore tries again while the page grows. */
const RETRY_MS = 120;

/** History entries remembered; the oldest are forgotten first. */
const KEPT_ENTRIES = 200;

const MAIN = 'main';

type Spot = { top: number; left: number };
type Spots = Map<string, Spot>;

type HistoryAction = Parameters<Parameters<RouterHistory['subscribe']>[0]>[0]['action'];
type EntryState = Partial<{ key: string; __TSR_key: string; __TSR_index: number }>;

const memory = new Map<string, Spots>();

/** Forgets every remembered position (tests). */
export const forgetScrollPositions = () => memory.clear();

const entryKey = (state: object | undefined) => {
  const parsed = state as EntryState | undefined;
  return parsed?.__TSR_key ?? parsed?.key ?? `index:${parsed?.__TSR_index ?? 0}`;
};

/** Names `element`, a scroller inside `main`. */
const scrollerId = (main: HTMLElement, element: HTMLElement): string | null => {
  if (element === main) return MAIN;
  const named = element.getAttribute(SCROLL_KEY_ATTRIBUTE);
  if (named) return `key:${named}`;
  const path: number[] = [];
  let node: HTMLElement = element;
  while (node !== main) {
    const parent = node.parentElement;
    if (!parent) return null;
    path.push(Array.prototype.indexOf.call(parent.children, node));
    node = parent;
  }
  return `path:${path.reverse().join('.')}:${element.tagName}`;
};

const findScroller = (main: HTMLElement, id: string): HTMLElement | null => {
  if (id === MAIN) return main;
  if (id.startsWith('key:')) {
    const name = id.slice(4);
    for (const element of main.querySelectorAll<HTMLElement>(`[${SCROLL_KEY_ATTRIBUTE}]`)) {
      if (element.getAttribute(SCROLL_KEY_ATTRIBUTE) === name) return element;
    }
    return null;
  }
  const [, path = '', tag] = id.split(':');
  let node: Element = main;
  for (const index of path.split('.').map(Number)) {
    const child = node.children[index];
    if (!child) return null;
    node = child;
  }
  return node.tagName === tag && node instanceof HTMLElement ? node : null;
};

const near = (element: HTMLElement, spot: Spot) =>
  Math.abs(element.scrollTop - spot.top) <= 1 && Math.abs(element.scrollLeft - spot.left) <= 1;

/** What the page shown next starts with: the top, a remembered spot, or as it is. */
type Next = 'top' | Spots | null;

const RETURNS: ReadonlySet<HistoryAction['type']> = new Set(['BACK', 'FORWARD', 'GO']);

const createScrollMemory = (
  main: HTMLElement,
  initial: { key: string; href: string; pathname: string },
  keepsOwn: (pathname: string) => boolean,
) => {
  let current = initial;
  let next: Next = null;
  /** Scrollers inside the page that have moved since it opened. */
  const scrolled = new Set<HTMLElement>();
  let restore: { key: string; pending: Spots; stop: () => void } | null = null;

  const onScroll = (event: Event) => {
    const target = event.target;
    if (target instanceof HTMLElement && target !== main && main.contains(target)) {
      scrolled.add(target);
    }
  };
  main.addEventListener('scroll', onScroll, { capture: true, passive: true });

  const save = () => {
    if (keepsOwn(current.pathname)) return;
    // Spots not reached yet are still where the page was.
    const spots: Spots = new Map(restore?.key === current.key ? restore.pending : []);
    const record = (element: HTMLElement) => {
      const id = scrollerId(main, element);
      if (!id || spots.has(id) || (!element.scrollTop && !element.scrollLeft)) return;
      spots.set(id, { top: element.scrollTop, left: element.scrollLeft });
    };
    record(main);
    for (const element of scrolled) if (element.isConnected) record(element);
    memory.delete(current.key);
    if (spots.size) memory.set(current.key, spots);
    while (memory.size > KEPT_ENTRIES) {
      const oldest = memory.keys().next().value;
      if (oldest === undefined) break;
      memory.delete(oldest);
    }
  };

  const startRestore = (spots: Spots) => {
    const pending = new Map(spots);
    let frame = 0;
    const attempt = () => {
      frame = 0;
      for (const [id, spot] of pending) {
        const element = findScroller(main, id);
        if (!element) continue;
        if (Math.abs(element.scrollTop - spot.top) > 1) element.scrollTop = spot.top;
        if (Math.abs(element.scrollLeft - spot.left) > 1) element.scrollLeft = spot.left;
        if (element !== main) scrolled.add(element);
        if (near(element, spot)) pending.delete(id);
      }
      if (!pending.size) stop();
    };
    const observer = new MutationObserver(() => {
      if (!frame) frame = requestAnimationFrame(attempt);
    });
    const timer = window.setInterval(attempt, RETRY_MS);
    const giveUp = window.setTimeout(() => stop(), RESTORE_WAIT_MS);
    // The person's own scrolling wins.
    const inputs = ['wheel', 'touchstart', 'pointerdown', 'keydown'] as const;
    const stop = () => {
      observer.disconnect();
      window.clearInterval(timer);
      window.clearTimeout(giveUp);
      if (frame) cancelAnimationFrame(frame);
      for (const input of inputs) window.removeEventListener(input, stop, true);
      if (restore?.pending === pending) restore = null;
    };
    restore = { key: current.key, pending, stop };
    for (const input of inputs) {
      window.addEventListener(input, stop, { capture: true, passive: true });
    }
    observer.observe(main, { childList: true, subtree: true });
    attempt();
  };

  return {
    /** A history change, before the next page is drawn: the page leaving is still on screen. */
    navigated: (location: HistoryLocation, action: HistoryAction) => {
      const key = entryKey(location.state);
      if (key === current.key) return;
      save();
      // A page that rewrites its address while coming back keeps coming back.
      const carried = action.type === 'REPLACE' && restore ? restore.pending : null;
      restore?.stop();
      scrolled.clear();
      const sameAddress = location.href === current.href;
      current = { key, href: location.href, pathname: location.pathname };
      const remembered = RETURNS.has(action.type) ? memory.get(key) : undefined;
      if (keepsOwn(location.pathname)) next = 'top';
      else if (remembered) next = remembered;
      else if (carried?.size) next = carried;
      else next = action.type === 'REPLACE' && sameAddress ? null : 'top';
    },
    /** The page for the current entry is drawn. */
    drawn: (key: string) => {
      if (key !== current.key) return;
      const start = next;
      next = null;
      if (start === 'top') main.scrollTo?.(0, 0);
      else if (start) startRestore(start);
    },
    dispose: () => {
      restore?.stop();
      main.removeEventListener('scroll', onScroll, true);
    },
  };
};

/**
 * Remembers and restores `<main>`'s scrolling (and the scrollers inside it) per history entry.
 * Pages under `keepsOwn` (the readers) keep their own positions: they start at the top and
 * restore themselves.
 */
export const useScrollMemory = (
  mainRef: RefObject<HTMLElement | null>,
  keepsOwn: (pathname: string) => boolean,
) => {
  const router = useRouter();
  const keepsOwnRef = useRef(keepsOwn);
  useEffect(() => {
    keepsOwnRef.current = keepsOwn;
  });

  useEffect(() => {
    const main = mainRef.current;
    if (!main) return;
    const { history } = router;
    const memoryControl = createScrollMemory(
      main,
      {
        key: entryKey(history.location.state),
        href: history.location.href,
        pathname: history.location.pathname,
      },
      (pathname) => keepsOwnRef.current(pathname),
    );
    const unsubscribe = history.subscribe(({ location, action }) =>
      memoryControl.navigated(location, action),
    );
    // After the new page's commit, not when the address changes: until then the page leaving is
    // still on screen.
    const unrendered = router.subscribe('onRendered', (event) =>
      memoryControl.drawn(entryKey(event.toLocation.state)),
    );
    return () => {
      unsubscribe();
      unrendered();
      memoryControl.dispose();
    };
  }, [router, mainRef]);
};
