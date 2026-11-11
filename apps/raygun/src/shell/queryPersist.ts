import { dehydrate, hydrate } from '@tanstack/react-query';
import type { DehydratedState, Query, QueryClient } from '@tanstack/react-query';

import { canCompress, gunzipText, gzipText, packQueries, unpackQueries } from './cachePack';

const DB_NAME = 'raygun-cache';
const STORE = 'queries';
const RECORD = 'client';
/** Saved answers older than this are dropped rather than shown. */
const MAX_AGE_MS = 7 * 24 * 60 * 60_000;
/** Changes are written at most this often, and when the window hides or closes. */
const SAVE_EVERY_MS = 5_000;
/** The answers kept: the most recently updated. */
const MAX_QUERIES = 800;
/** Start doesn't wait longer than this for the saved answers. */
const RESTORE_WAIT_MS = 1_500;

/** Answers that only mean something now (what's playing, progress, devices), never saved. */
const VOLATILE = new Set([
  'playbackState',
  'queue',
  'spokenItem',
  'audioOutputs',
  'connect',
  'remote',
  'mcp',
  'sync',
  'storage',
  'storageInfo',
  'lyricsCache',
  'exports',
  'backups',
  'captions',
  'tts',
  'youtubeEmbed',
  'appInfo',
  'appUpdate',
  'externalPlayers',
  'import',
  'ai',
  'localNetwork',
  'ytdlp',
  'incognito',
  // Rust keeps dictionary answers; theirs hold proxy links that last one run.
  'dictionary',
  // Offline Mode and Data Saver are Rust's to say on every start.
  'network',
  // What to continue watching and the history follow the player, which saves after this cache
  // does: a restored answer would show an episode left behind.
  'watch',
  // Whether this start shows What's new: a restored answer showed notes already seen.
  'appWhatsNew',
]);

/**
 * What's kept in IndexedDB: the answers as gzipped JSON (shell/cachePack), or as they are where
 * the browser can't compress (and as earlier versions kept them, which still read).
 */
export type Saved = { build: string; savedAt: number } & (
  { packed: Uint8Array<ArrayBuffer>; tagged: string[] } | { state: DehydratedState }
);

/** `state` as it's kept: compressed where the browser can. */
export const toSaved = async (
  state: DehydratedState,
  build: string,
  savedAt: number,
): Promise<Saved> => {
  if (!canCompress()) return { build, savedAt, state };
  const { json, tagged } = packQueries(state);
  return { build, savedAt, packed: await gzipText(json), tagged };
};

/** The answers in `saved`, whichever way they were kept. */
export const fromSaved = async (saved: Saved): Promise<DehydratedState> =>
  'packed' in saved ? unpackQueries(await gunzipText(saved.packed), saved.tagged) : saved.state;

/** Builds differ in what answers look like, so each starts from its own saved cache. */
const BUILD = typeof __RAYGUN_BUILD__ === 'string' ? __RAYGUN_BUILD__ : 'dev';

/** When incognito was on in this run (M47), in Unix ms; `to` is null while it still is. */
const incognitoSpans: { from: number; to: number | null }[] = [];

/**
 * Follows incognito (M47): answers that come while it's on (searches, titles, pages) are never
 * saved, even after it ends, until they're asked again.
 */
export const noteIncognito = (status: { on: boolean; sinceMs: number | null }) => {
  const last = incognitoSpans.at(-1);
  const running = last !== undefined && last.to === null;
  if (status.on && !running) incognitoSpans.push({ from: status.sinceMs ?? Date.now(), to: null });
  if (!status.on && running) last.to = Date.now();
};

/** Whether an answer updated at `updatedAt` came while incognito was on. */
export const cameInIncognito = (updatedAt: number) =>
  incognitoSpans.some(
    (span) => updatedAt >= span.from && (span.to === null || updatedAt <= span.to),
  );

const saveable = (query: Query) => {
  if (query.state.status !== 'success' || query.state.data === undefined) return false;
  if (cameInIncognito(query.state.dataUpdatedAt)) return false;
  const key = query.queryKey;
  if (typeof key[0] === 'string' && VOLATILE.has(key[0])) return false;
  // Download and recording progress goes stale at once.
  return !key.some((part) => part === 'downloads' || part === 'downloading');
};

const open = () =>
  new Promise<IDBDatabase>((resolve, reject) => {
    const request = indexedDB.open(DB_NAME, 1);
    request.onupgradeneeded = () => request.result.createObjectStore(STORE);
    request.onsuccess = () => resolve(request.result);
    request.onerror = () => reject(request.error ?? new Error('indexedDB failed'));
  });

const read = async (): Promise<Saved | undefined> => {
  const db = await open();
  try {
    return await new Promise<Saved | undefined>((resolve, reject) => {
      const request = db.transaction(STORE, 'readonly').objectStore(STORE).get(RECORD);
      request.onsuccess = () => resolve(request.result as Saved | undefined);
      request.onerror = () => reject(request.error ?? new Error('read failed'));
    });
  } finally {
    db.close();
  }
};

/** Deletes the saved answers; saving goes on. */
const dropSaved = () =>
  new Promise<void>((resolve) => {
    const request = indexedDB.deleteDatabase(DB_NAME);
    request.onsuccess = () => resolve();
    request.onerror = () => resolve();
    request.onblocked = () => resolve();
  });

/** A saved cache larger than this (compressed) isn't restored: a phone's page ran out of memory. */
export const MAX_RESTORE_BYTES = 4 * 1024 * 1024;

/** What Rust said of this start before the page's first script (`crash_log.rs`). */
export const safeStart = (): { active: boolean; clearCaches: boolean } => {
  const said = (window as { __RAYGUN_SAFE_START__?: { active?: unknown; clearCaches?: unknown } })
    .__RAYGUN_SAFE_START__;
  return { active: said?.active === true, clearCaches: said?.clearCaches === true };
};

/** Set by an erase (M46.51): nothing is saved again in this run. */
let forgotten = false;

/**
 * Forgets the saved answers and saves none again in this run (M46.51: Erase everything starts
 * Raygun again as new). Resolves once they're gone, or couldn't go.
 */
export const forgetQueryCache = async () => {
  forgotten = true;
  if (typeof indexedDB === 'undefined') return;
  await new Promise<void>((resolve) => {
    const request = indexedDB.deleteDatabase(DB_NAME);
    request.onsuccess = () => resolve();
    request.onerror = () => resolve();
    request.onblocked = () => resolve();
  });
};

const write = async (saved: Saved) => {
  if (forgotten) return;
  const db = await open();
  try {
    await new Promise<void>((resolve, reject) => {
      const transaction = db.transaction(STORE, 'readwrite');
      transaction.objectStore(STORE).put(saved, RECORD);
      transaction.oncomplete = () => resolve();
      transaction.onerror = () => reject(transaction.error ?? new Error('write failed'));
    });
  } finally {
    db.close();
  }
};

/**
 * Puts last time's answers back before the app draws (M46.47), so lists, charts, titles and
 * searches show at once. Each restored answer counts as out of date: whatever shows it asks again
 * in the background and swaps in the new answer when it comes. Gives up after a moment, and on
 * any failure, so a start never waits on it.
 */
export const restoreQueryCache = async (client: QueryClient) => {
  if (typeof indexedDB === 'undefined') return;
  // After a crash (a safe start), or when asked to clear caches, nothing comes back: what was
  // saved may be what ended the last start.
  if (safeStart().active) {
    await dropSaved();
    return;
  }
  const timeout = new Promise<undefined>((resolve) =>
    setTimeout(() => resolve(undefined), RESTORE_WAIT_MS),
  );
  const saved = async () => {
    const found = await read();
    if (!found || found.build !== BUILD || Date.now() - found.savedAt > MAX_AGE_MS) return;
    if ('packed' in found && found.packed.byteLength > MAX_RESTORE_BYTES) {
      await dropSaved();
      return;
    }
    return fromSaved(found);
  };
  try {
    const state = await Promise.race([saved(), timeout]);
    if (!state) return;
    hydrate(client, state);
    // Old answers show until new ones come; none counts as fresh, whatever its staleTime.
    void client.invalidateQueries({ refetchType: 'none' });
  } catch {
    // A missing or broken cache: start as before.
  }
};

/**
 * Saves the answers as they change (M46.47): a few seconds after a change, and when the window
 * hides or closes. Returns a function that stops it.
 */
export const persistQueryCache = (client: QueryClient) => {
  if (typeof indexedDB === 'undefined') return () => {};
  let timer: ReturnType<typeof setTimeout> | undefined;
  // Compressing takes a moment: a save that finishes after a later one started isn't written.
  let latest = 0;
  const save = () => {
    timer = undefined;
    const turn = ++latest;
    const kept = new Set(
      client
        .getQueryCache()
        .getAll()
        .filter(saveable)
        .sort((a, b) => b.state.dataUpdatedAt - a.state.dataUpdatedAt)
        .slice(0, MAX_QUERIES)
        .map((query) => query.queryHash),
    );
    let state: DehydratedState;
    try {
      state = dehydrate(client, {
        shouldDehydrateQuery: (query) => kept.has(query.queryHash),
        shouldDehydrateMutation: () => false,
      });
    } catch {
      return;
    }
    void toSaved(state, BUILD, Date.now())
      .then((saved) => (turn === latest ? write(saved) : undefined))
      .catch(() => {
        // Full or blocked storage: the next start just loads as before.
      });
  };
  const later = () => {
    timer ??= setTimeout(save, SAVE_EVERY_MS);
  };
  const now = () => {
    if (timer === undefined) return;
    clearTimeout(timer);
    save();
  };
  const unsubscribe = client.getQueryCache().subscribe((event) => {
    // Progress and other answers that aren't kept don't cause a save (downloads report often).
    if (event.type === 'updated' && event.action.type === 'success' && saveable(event.query)) {
      later();
    }
    if (event.type === 'removed') later();
  });
  const hidden = () => {
    if (document.visibilityState === 'hidden') now();
  };
  document.addEventListener('visibilitychange', hidden);
  window.addEventListener('pagehide', now);
  return () => {
    unsubscribe();
    document.removeEventListener('visibilitychange', hidden);
    window.removeEventListener('pagehide', now);
    if (timer !== undefined) clearTimeout(timer);
  };
};
