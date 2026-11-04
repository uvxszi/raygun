import { commands } from '@raygun/bindings';
import type { SongDownload } from '@raygun/bindings';
import type { QueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

/**
 * The song downloads as one list in the query cache, the one answer every list row, mark, button
 * and bar reads. Rust pushes each download as it changes (`songDownloadProgressEvent`) and the
 * list is patched in place; whole-list fetches only start it or follow bulk changes. Answers can
 * arrive out of order, so each download keeps a count of the changes applied to it: an answer
 * asked for before a newer change is dropped, and a list fetched while changes came keeps them.
 */
export const SONGS = ['songs', 'downloads'] as const;
/** Whether downloads are paused (only for now: it isn't kept between starts). */
export const PAUSED = ['songs', 'downloads', 'paused'] as const;
/** Lists made of finished downloads (Library → Downloaded and its tile). */
export const DOWNLOADED = ['songs', 'downloaded'] as const;

type Book = {
  /** Changes applied, by download. */
  changes: Map<string, number>;
  /** Changes made while the list is on its way, put over it when it comes. */
  since: Map<string, SongDownload | null> | null;
};

const books = new WeakMap<QueryClient, Book>();

const bookFor = (client: QueryClient) => {
  let book = books.get(client);
  if (!book) {
    book = { changes: new Map(), since: null };
    books.set(client, book);
  }
  return book;
};

/** How many changes `id` has had; an answer asked for at one count is stale at another. */
export const changesOf = (client: QueryClient, id: string) => bookFor(client).changes.get(id) ?? 0;

const doneIds = (list: readonly SongDownload[]) =>
  list
    .filter((item) => item.state === 'done')
    .map((item) => item.id)
    .sort()
    .join(' ');

/** Sets one download as it is now, or takes it out (`null`). */
export const applyItem = (client: QueryClient, id: string, item: SongDownload | null) => {
  const book = bookFor(client);
  book.changes.set(id, changesOf(client, id) + 1);
  book.since?.set(id, item);
  const before = client.getQueryData<SongDownload[]>(SONGS);
  if (!before) return;
  const at = before.findIndex((entry) => entry.id === id);
  const old = at === -1 ? null : (before[at] ?? null);
  if (item === null && old === null) return;
  let after: SongDownload[];
  if (item === null) {
    after = before.filter((_, index) => index !== at);
  } else if (at === -1) {
    after = [item, ...before];
  } else {
    after = before.slice();
    after[at] = item;
  }
  client.setQueryData(SONGS, after);
  // Finished downloads make Library → Downloaded.
  if ((old?.state === 'done') !== (item?.state === 'done')) {
    void client.invalidateQueries({ queryKey: DOWNLOADED });
  }
};

/** Changes a download in place at once, before Rust says (it says right after). */
export const patchItem = (client: QueryClient, id: string, change: Partial<SongDownload>) => {
  const item = client.getQueryData<SongDownload[]>(SONGS)?.find((entry) => entry.id === id);
  if (item) applyItem(client, id, { ...item, ...change });
};

/** Asks Rust for one download again; its answer counts unless a newer change came meanwhile. */
export const refreshItem = async (client: QueryClient, id: string) => {
  const asked = changesOf(client, id);
  const result = await commands.songDownload(id);
  if (changesOf(client, id) !== asked) return;
  if (result.status === 'ok') applyItem(client, id, result.data);
  else void client.invalidateQueries({ queryKey: SONGS });
};

/** The whole list, with whatever changed while it was on its way. */
export const fetchList = async (client: QueryClient) => {
  const book = bookFor(client);
  const since = new Map<string, SongDownload | null>();
  book.since = since;
  const before = client.getQueryData<SongDownload[]>(SONGS);
  const fresh = await unwrapResult(commands.songDownloads());
  if (book.since === since) book.since = null;
  const listed = new Set(fresh.map((item) => item.id));
  const list = fresh.flatMap((item) => {
    if (!since.has(item.id)) return [item];
    const latest = since.get(item.id);
    return latest ? [latest] : [];
  });
  for (const [id, latest] of since) {
    if (latest && !listed.has(id)) list.unshift(latest);
  }
  if (before && doneIds(before) !== doneIds(list)) {
    void client.invalidateQueries({ queryKey: DOWNLOADED });
  }
  return list;
};

/** Where a song put first in the queue goes: before the first queued one. */
export const frontOfQueue = (client: QueryClient) => {
  const queued = (client.getQueryData<SongDownload[]>(SONGS) ?? [])
    .filter((item) => item.state === 'queued')
    .map((item) => item.queuedAt ?? 0);
  return queued.length > 0 ? Math.min(...queued) - 1 : Date.now();
};

const byKeys = new WeakMap<readonly SongDownload[], Map<string, SongDownload>>();

/** The downloads by song key, made once per list. */
export const indexByKey = (list: readonly SongDownload[]) => {
  let index = byKeys.get(list);
  if (!index) {
    index = new Map(list.map((item) => [item.trackKey, item]));
    byKeys.set(list, index);
  }
  return index;
};
