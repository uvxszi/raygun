// Adapted from nonbili/NouTube (https://github.com/nonbili/NouTube), AGPL-3.0.
// From content/utils.ts (nouStorage) and content/player-formats.ts (ytcfgValue).
//
// What every feature shares: the extension's settings, which of YouTube's sites this is, the
// browser's state, and small helpers for a page that is still loading when the script starts.

import type { PageState } from '@raygun/sdk/site';

declare global {
  interface Window {
    /** YouTube's last-activity time; its "Continue watching?" prompt goes by it. */
    _lact?: number;
    ytcfg?: { get?: (key: string) => unknown; data_?: Record<string, unknown> };
    ytInitialData?: unknown;
    ytInitialPlayerResponse?: unknown;
    /** Set once the extension's script ran in this document. */
    __nouRaygun?: boolean;
  }
}

/** A switch of the extension's settings; `fallback` when it isn't set to true or false. */
export const flag = (key: string, fallback: boolean): boolean => {
  const value = raygunPage.settings[key];
  return typeof value === 'boolean' ? value : fallback;
};

/** A text or choice setting. */
export const textSetting = (key: string, fallback = ''): string => {
  const value = raygunPage.settings[key];
  return typeof value === 'string' ? value : fallback;
};

export const isMusic = location.hostname === 'music.youtube.com';
export const isMobileSite = location.hostname === 'm.youtube.com';
export const isAndroid = raygunPage.platform === 'android';
/** A phone's or tablet's browser, which YouTube Music serves its mobile site (unless it asks for the desktop one). */
export const isMobileAgent = /Android|Mobile/i.test(navigator.userAgent);

/** YouTube's own fetch, taken before ../intercept.ts wraps it: the features' requests skip the filters. */
export const pageFetch: typeof fetch = window.fetch.bind(window);

// The browser's latest word, kept from its reports in case `raygunPage.state` is a snapshot.
let latestState: Readonly<PageState> = raygunPage.state ?? {
  background: false,
  call: false,
  audioActive: false,
};
raygunPage.on('state', (state) => {
  latestState = state;
});

/** Whether the browser is off screen, a call is on, and something plays sound on the device. */
export const pageState = (): Readonly<PageState> => latestState;

/** Runs `run` once the document has its root element, which a script at document start may not see yet. */
export function whenRoot(run: (root: HTMLElement) => void) {
  const root = document.documentElement;
  if (root) {
    run(root);
    return;
  }
  const observer = new MutationObserver(() => {
    const element = document.documentElement;
    if (!element) return;
    observer.disconnect();
    run(element);
  });
  observer.observe(document, { childList: true });
}

/** Runs `run` once the document has its body. */
export function whenBody(run: (body: HTMLElement) => void) {
  if (document.body) {
    run(document.body);
    return;
  }
  document.addEventListener('DOMContentLoaded', () => run(document.body), { once: true });
}

/** A style element of this extension, made (or found again) by id. */
export function addStyle(id: string, css: string) {
  whenRoot((root) => {
    let style = document.getElementById(id);
    if (!style) {
      style = document.createElement('style');
      style.id = id;
      (document.head || root).appendChild(style);
    }
    style.textContent = css;
  });
}

// The events YouTube fires when it moved to another page without loading it: the desktop site's
// and YouTube Music's, the mobile site's, and the browser's own for Back and Forward.
const NAVIGATION_EVENTS = [
  'yt-navigate-finish',
  'yt-page-data-updated',
  'state-navigateend',
  'popstate',
];

/**
 * Calls `listener` (at most once a tick) when YouTube moved to another page without loading
 * one. The events reach the document or the window depending on the site, so both are heard.
 */
export function onNavigate(listener: () => void) {
  let scheduled = false;
  const schedule = () => {
    if (scheduled) return;
    scheduled = true;
    setTimeout(() => {
      scheduled = false;
      listener();
    }, 0);
  };
  for (const name of NAVIGATION_EVENTS) {
    window.addEventListener(name, schedule);
    document.addEventListener(name, schedule);
  }
}

/** Calls `run` `wait` ms after the first of a burst of calls, once for the whole burst. */
export function coalesce(run: () => void, wait: number) {
  let timer: ReturnType<typeof setTimeout> | undefined;
  return () => {
    if (timer !== undefined) return;
    timer = setTimeout(() => {
      timer = undefined;
      run();
    }, wait);
  };
}

/** A value of YouTube's page configuration (`ytcfg`). */
export const ytcfgValue = (key: string): unknown => {
  const ytcfg = window.ytcfg;
  try {
    return ytcfg?.get?.(key) ?? ytcfg?.data_?.[key];
  } catch {
    return ytcfg?.data_?.[key];
  }
};

export const VIDEO_ID = /^[A-Za-z0-9_-]{11}$/;

/** The video id on a watch page (`/watch?v=`); empty elsewhere or when it isn't one. */
export const watchVideoId = (address: string = location.href): string => {
  try {
    const url = new URL(address, location.href);
    if (url.pathname !== '/watch') return '';
    const id = url.searchParams.get('v') ?? '';
    return VIDEO_ID.test(id) ? id : '';
  } catch {
    return '';
  }
};

// localStorage, or memory when the page may not use it.
const memoryStorage = new Map<string, string>();
export const nouStorage = {
  getItem(key: string): string | null {
    try {
      return localStorage.getItem(key);
    } catch {
      return memoryStorage.get(key) ?? null;
    }
  },
  setItem(key: string, value: string) {
    try {
      localStorage.setItem(key, value);
    } catch {
      memoryStorage.set(key, value);
    }
  },
  removeItem(key: string) {
    try {
      localStorage.removeItem(key);
    } catch {
      memoryStorage.delete(key);
    }
  },
};
