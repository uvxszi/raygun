import { useEffect, useRef, useSyncExternalStore } from 'react';

/**
 * Back for everything that opens over a page: sheets, popovers, menus, dialogs, the full player
 * and its lyrics and queue views. The innermost open layer closes first, then the one under it,
 * and only then does Back leave the page. Android's Back, the app bar's Back, the mouse's back
 * button, Alt+Left and Escape all go through here.
 *
 * On phones, where Back is the system's, each open layer has one history entry of its own (the
 * page's address, marked) on top of the page's. Back takes the top entry off, and its layer
 * closes. A layer closed another way (a tap outside, its own button) takes its entry with it. One
 * closed by following a link leaves its entry under the new page; Back steps over entries whose
 * layer is gone. On desktop every Back is Raygun's own (the bar's button, the mouse, the keys), so
 * layers keep no entries there and Forward keeps working.
 */

const MARK = 'raygunLayer';
/** Whether layers keep history entries: where the system's Back goes through the webview. */
let withEntries =
  typeof navigator !== 'undefined' && /Android|iPhone|iPad|iPod/i.test(navigator.userAgent);
/** This run's mark: entries left from before a reload are stepped over like closed ones. */
const RUN = Math.random().toString(36).slice(2);

type Layer = { id: number; close: () => void; escape: boolean };
type Mark = { run: string; id: number };

/** Open layers, oldest first. */
const layers: Layer[] = [];
/** The layers whose entries sit on top of history now, oldest first. */
let entries: number[] = [];
let nextId = 1;
/** History is moving because Raygun asked it to (dropping entries): not the listener's Back. */
let stepping = false;
let pending: ReturnType<typeof setTimeout> | null = null;
let installed = false;
const watchers = new Set<() => void>();

const markOf = (state: unknown): Mark | null => {
  if (typeof state !== 'object' || state === null) return null;
  const mark = (state as Record<string, unknown>)[MARK];
  if (typeof mark !== 'object' || mark === null) return null;
  const { run, id } = mark as Partial<Mark>;
  return typeof run === 'string' && typeof id === 'number' ? { run, id } : null;
};

// A reload while a layer was open leaves its entry current: the next Back would do nothing.
if (typeof window !== 'undefined' && markOf(window.history.state)) {
  const state = Object.fromEntries(
    Object.entries(window.history.state as Record<string, unknown>).filter(([key]) => key !== MARK),
  );
  History.prototype.replaceState.call(window.history, state, '');
}

/** An entry of a layer that's closed now (or of an earlier run): Back passes over it. */
const isStale = (state: unknown) => {
  const mark = markOf(state);
  return mark !== null && (mark.run !== RUN || !entries.includes(mark.id));
};

/** The top of history is this run's newest entry: nothing went on top of the layers' entries. */
const onOwnTop = () => {
  const mark = markOf(window.history.state);
  return mark?.run === RUN && mark.id === entries.at(-1);
};

const announce = () => {
  for (const watcher of watchers) watcher();
};

/** Brings history in line with the open layers: one entry each, in order, nothing left over. */
const reconcile = () => {
  pending = null;
  if (stepping) return;
  if (entries.length > 0 && !onOwnTop()) {
    // A page opened over them (a link followed from a layer): they stay under it, stale.
    entries = [];
  }
  let kept = 0;
  while (kept < entries.length && entries[kept] === layers[kept]?.id) {
    kept += 1;
  }
  if (kept < entries.length) {
    stepping = true;
    window.history.go(kept - entries.length);
    return;
  }
  for (const layer of layers.slice(kept)) {
    const state = { ...(window.history.state ?? {}), [MARK]: { run: RUN, id: layer.id } };
    // The prototype's own pushState: the router keeps the page it shows (same address) instead of
    // treating each layer as a page opened.
    History.prototype.pushState.call(window.history, state, '');
    entries.push(layer.id);
  }
};

// After the current task: a router push from the same click lands first, and a layer closed and
// another opened at once (a menu item opening a sheet) settle in one step.
const schedule = () => {
  pending ??= setTimeout(reconcile, 0);
};

const closeLayer = (layer: Layer) => {
  const at = layers.indexOf(layer);
  if (at >= 0) layers.splice(at, 1);
  announce();
  layer.close();
};

const onPopState = (event: PopStateEvent) => {
  if (!withEntries) return;
  const mark = markOf(event.state);
  if (stepping) {
    stepping = false;
    entries = mark?.run === RUN ? entries.slice(0, entries.indexOf(mark.id) + 1) : [];
  } else {
    // Back (or Forward) moved off the top entries: their layers close, newest first.
    const at = mark?.run === RUN ? entries.indexOf(mark.id) : -1;
    const passed = entries.slice(at + 1);
    entries = entries.slice(0, at + 1);
    for (const id of passed.reverse()) {
      const layer = layers.find((open) => open.id === id);
      if (layer) closeLayer(layer);
    }
  }
  if (isStale(event.state)) {
    // Its layer is gone: on to the entry under it.
    stepping = true;
    window.history.back();
    return;
  }
  schedule();
};

/** The newest layer that closes on Escape, or none (one that handles Escape itself is on top). */
const escapable = () => {
  const top = layers.at(-1);
  return top?.escape ? top : null;
};

const onKeyDown = (event: KeyboardEvent) => {
  if (event.key !== 'Escape' || event.defaultPrevented) return;
  const top = escapable();
  if (!top) return;
  // A search field with text empties first (TextField); the next Escape closes.
  const field = event.target;
  if (field instanceof HTMLInputElement && field.value !== '' && field.dataset.clearsOnEscape) {
    return;
  }
  event.preventDefault();
  event.stopPropagation();
  closeTopLayer();
};

const install = () => {
  if (installed) return;
  installed = true;
  window.addEventListener('popstate', onPopState);
  window.addEventListener('keydown', onKeyDown, true);
};

/** Opens a layer; returns its id. */
const addLayer = (close: () => void, escape: boolean) => {
  install();
  const layer = { id: nextId, close, escape };
  nextId += 1;
  layers.push(layer);
  announce();
  if (withEntries) schedule();
  return layer.id;
};

/** A layer closed (any way): its entry goes, unless Back already took it. */
const removeLayer = (id: number) => {
  const at = layers.findIndex((layer) => layer.id === id);
  if (at >= 0) {
    layers.splice(at, 1);
    announce();
  }
  if (withEntries) schedule();
};

/**
 * Closes the newest layer the way Back does (its entry goes too). Returns whether one was open.
 */
export const closeTopLayer = () => {
  const top = layers.at(-1);
  if (!top) return false;
  if (withEntries && onOwnTop() && entries.at(-1) === top.id) {
    // Back itself: the popstate closes it.
    window.history.back();
  } else {
    closeLayer(top);
    if (withEntries) schedule();
  }
  return true;
};

/**
 * Goes back a page from under any open layers (a menu's Back): they close, and their entries go
 * along with the page's.
 */
export const backPastLayers = (back: () => void) => {
  if (withEntries && entries.length > 0 && onOwnTop()) {
    window.history.go(-(entries.length + 1));
    return;
  }
  for (const layer of [...layers].reverse()) closeLayer(layer);
  back();
};

/** Whether anything is open over the page (the app bar's Back has something to close). */
export const useLayersOpen = () =>
  useSyncExternalStore(
    (onChange) => {
      watchers.add(onChange);
      return () => watchers.delete(onChange);
    },
    () => layers.length > 0,
    () => false,
  );

/**
 * Makes an overlay a layer while `open`: Back (and Escape, unless `escape` is false because it
 * handles Escape itself, as menus with submenus do) closes it before anything under it.
 */
export const useBackLayer = (
  open: boolean,
  close: () => void,
  { escape = true }: { escape?: boolean } = {},
) => {
  const latest = useRef(close);
  useEffect(() => {
    latest.current = close;
  });
  useEffect(() => {
    if (!open) return;
    const id = addLayer(() => latest.current(), escape);
    return () => removeLayer(id);
  }, [open, escape]);
};

/** Back that closes the innermost layer first, else goes back a page. */
export const backFromLayers = (back: () => void) => {
  if (!closeTopLayer()) back();
};

/**
 * Desktop Back and Forward from the mouse's own buttons and Alt+Left/Right: the innermost layer
 * first, then the page. The webview's own handling of them would skip the layers.
 */
export const useDesktopBack = (back: () => void, forward: () => void) => {
  const actions = useRef({ back, forward });
  useEffect(() => {
    actions.current = { back, forward };
  });
  useEffect(() => {
    const onMouseUp = (event: MouseEvent) => {
      if (event.button !== 3 && event.button !== 4) return;
      event.preventDefault();
      if (event.button === 4) actions.current.forward();
      else backFromLayers(actions.current.back);
    };
    // Its press too, or some webviews go back on it.
    const onMouseDown = (event: MouseEvent) => {
      if (event.button === 3 || event.button === 4) event.preventDefault();
    };
    const onKeyDown = (event: KeyboardEvent) => {
      if (!event.altKey || event.ctrlKey || event.metaKey || event.shiftKey) return;
      if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;
      const target = event.target;
      // Alt+arrows move by words in a field.
      if (target instanceof HTMLElement && target.closest('input, textarea, [contenteditable]')) {
        return;
      }
      event.preventDefault();
      if (event.key === 'ArrowRight') actions.current.forward();
      else backFromLayers(actions.current.back);
    };
    window.addEventListener('mouseup', onMouseUp);
    window.addEventListener('mousedown', onMouseDown);
    window.addEventListener('keydown', onKeyDown);
    return () => {
      window.removeEventListener('mouseup', onMouseUp);
      window.removeEventListener('mousedown', onMouseDown);
      window.removeEventListener('keydown', onKeyDown);
    };
  }, []);
};

/** Forgets every layer; `entries` picks phone (history entries) or desktop behaviour (tests). */
export const resetBackLayers = (entriesMode = false) => {
  withEntries = entriesMode;
  layers.length = 0;
  entries = [];
  stepping = false;
  if (pending) clearTimeout(pending);
  pending = null;
  announce();
};
