import { commands, events } from '@raygun/bindings';
import type {
  AudioFileFormat,
  OnlineTrack,
  SavedTrack,
  SongDestination,
  SongDownload,
  SongDownloadState,
  TrackInput,
} from '@raygun/bindings';
import type { QueryClient } from '@tanstack/react-query';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect, useMemo } from 'react';

import { unwrapResult } from '../lib/commandResult';
import {
  applyItem,
  DOWNLOADED,
  fetchList,
  frontOfQueue,
  indexByKey,
  PAUSED,
  patchItem,
  refreshItem,
  SONGS,
} from './songDownloadCache';
import { trackKey } from './trackKey';

/** A change without its download following at once (a removal) is asked about after this. */
const CHECK_DELAY_MS = 40;
/** States of a song on its way. */
const ON_ITS_WAY: SongDownloadState[] = ['queued', 'downloading', 'recording'];

/**
 * Keeps the song downloads current from Rust (M46.10): each download as it changes, patched into
 * the one list. The shell runs it once (M46.46); everything else reads the list.
 */
export const useSongDownloadSync = () => {
  const client = useQueryClient();
  useEffect(() => {
    let stopped = false;
    const unsure = new Set<string>();
    let timer: ReturnType<typeof setTimeout> | undefined;
    const check = () => {
      timer = undefined;
      for (const id of unsure) {
        void refreshItem(client, id).catch(() => undefined);
      }
      unsure.clear();
    };
    const changes = events.songDownloadsEvent.listen(({ payload }) => {
      if (stopped) return;
      if (!payload) {
        // Many at once (a batch, the order, pause).
        void client.invalidateQueries({ queryKey: SONGS });
        void client.invalidateQueries({ queryKey: PAUSED });
        return;
      }
      // The download itself follows right away, unless it was removed.
      unsure.add(payload);
      timer ??= setTimeout(check, CHECK_DELAY_MS);
    });
    const moves = events.songDownloadProgressEvent.listen(({ payload }) => {
      if (stopped) return;
      unsure.delete(payload.id);
      applyItem(client, payload.id, payload);
    });
    return () => {
      stopped = true;
      if (timer !== undefined) clearTimeout(timer);
      void changes.then((stop) => stop());
      void moves.then((stop) => stop());
    };
  }, [client]);
};

const listQuery = (client: QueryClient) => ({
  queryKey: SONGS,
  queryFn: () => fetchList(client),
  // Kept current by the events; asked again now and then (back in the app) in case one was missed.
  staleTime: 60_000,
});

/** Every song download, newest first. */
export const useSongDownloads = () => {
  const client = useQueryClient();
  return useQuery(listQuery(client));
};

/**
 * Something about the downloads, re-rendering only when it changes (progress of other songs
 * doesn't): `select` picks it from the list.
 */
export const useSongDownloadsSelect = <Selected>(select: (list: SongDownload[]) => Selected) => {
  const client = useQueryClient();
  return useQuery({ ...listQuery(client), select });
};

/** One song's download by its key, if it has one. */
export const useSongDownloadByKey = (key: string) =>
  useSongDownloadsSelect((list) => indexByKey(list).get(key) ?? null);

/** One song's download, if it has one (M46.18: the download button beside +). */
export const useSongDownloadFor = (track: TrackInput) =>
  useSongDownloadByKey(trackKey(track.title, track.artist));

/** The state of the first of `keys` that has a download (a song's marks). */
export const useSongDownloadState = (keys: readonly string[]) =>
  useSongDownloadsSelect((list) => {
    const index = indexByKey(list);
    for (const key of keys) {
      const found = index.get(key);
      if (found) return found.state;
    }
    return null;
  }).data ?? null;

const keysIn = (list: SongDownload[], states: readonly SongDownloadState[]) =>
  list
    .filter((item) => states.includes(item.state))
    .map((item) => item.trackKey)
    .sort();

const DONE: SongDownloadState[] = ['done'];

/** Keys of downloaded songs, to mark them in song lists. */
export const useDownloadedKeys = () => {
  const keys = useSongDownloadsSelect((list) => keysIn(list, DONE)).data;
  return useMemo(() => new Set(keys ?? []), [keys]);
};

/** Keys of songs queued, downloading or being recorded (M46.46: the mark in lists). */
export const useDownloadingKeys = () => {
  const keys = useSongDownloadsSelect((list) => keysIn(list, ON_ITS_WAY)).data;
  return useMemo(() => new Set(keys ?? []), [keys]);
};

/** Songs downloaded into the app, as a collection (M46.18: Library → Audio → Downloaded). */
export const useDownloadedSongs = () =>
  useQuery<SavedTrack[]>({
    queryKey: [...DOWNLOADED, 'songs'],
    queryFn: () => unwrapResult(commands.downloadedSongs()),
  });

/** Whether downloads are paused. */
export const useSongDownloadsPaused = () =>
  useQuery({ queryKey: PAUSED, queryFn: () => commands.songDownloadsPaused() });

/** Queues songs to save into the app. */
export const downloadSongs = (tracks: TrackInput[]) => unwrapResult(commands.songsDownload(tracks));

/** A download put back first in the queue, as it shows before Rust confirms it. */
const queuedFirst = (client: QueryClient): Partial<SongDownload> => ({
  state: 'queued',
  error: null,
  errorDetail: null,
  finishedAt: null,
  queuedAt: frontOfQueue(client),
  bytesPerSecond: null,
  etaMs: null,
});

/**
 * What can be done to downloads. Each shows at once (the list changes before Rust answers), and a
 * refusal puts back what Rust has.
 */
export const useSongDownloadActions = () => {
  const client = useQueryClient();
  const refresh = (id: string) => void refreshItem(client, id).catch(() => undefined);
  const refetch = () => void client.invalidateQueries({ queryKey: SONGS });
  return {
    remove: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.songDownloadRemove(id)),
      onMutate: (id) => applyItem(client, id, null),
      onError: refetch,
    }),
    removeMany: useMutation({
      mutationFn: (ids: string[]) => unwrapResult(commands.songDownloadsRemove(ids)),
      onMutate: (ids) => {
        for (const id of ids) applyItem(client, id, null);
      },
      onError: refetch,
    }),
    retry: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.songDownloadRetry(id)),
      onMutate: (id) => patchItem(client, id, queuedFirst(client)),
      onError: (_error, id) => refresh(id),
    }),
    retryFailed: useMutation({
      mutationFn: () => unwrapResult(commands.songDownloadsRetryFailed()),
      onMutate: () => {
        const failed = (client.getQueryData<SongDownload[]>(SONGS) ?? []).filter(
          (item) => item.state === 'failed' || item.state === 'needsRecording',
        );
        const now = Date.now();
        failed.forEach((item, index) =>
          patchItem(client, item.id, {
            ...queuedFirst(client),
            queuedAt: now + index,
          }),
        );
      },
      onError: refetch,
    }),
    useSource: useMutation({
      mutationFn: (chosen: { id: string; track: OnlineTrack; source: string | null }) =>
        unwrapResult(commands.songDownloadUseSource(chosen.id, chosen.track, chosen.source)),
      onMutate: (chosen) =>
        patchItem(client, chosen.id, {
          ...queuedFirst(client),
          provider: chosen.source,
          doneBytes: 0,
          totalBytes: null,
          fraction: null,
        }),
      onError: (_error, chosen) => refresh(chosen.id),
    }),
    record: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.songDownloadRecord(id)),
      onMutate: (id) =>
        patchItem(client, id, {
          state: 'recording',
          kind: 'recording',
          error: null,
          errorDetail: null,
        }),
      onError: (_error, id) => refresh(id),
    }),
    reorder: useMutation({
      mutationFn: (ids: string[]) => unwrapResult(commands.songDownloadsReorder(ids)),
      onMutate: (ids) => {
        const front = frontOfQueue(client) - ids.length;
        ids.forEach((id, index) => patchItem(client, id, { queuedAt: front + index }));
      },
      onError: refetch,
    }),
    setPaused: useMutation({
      mutationFn: (paused: boolean) => commands.songDownloadsSetPaused(paused),
      onMutate: (paused) => client.setQueryData(PAUSED, paused),
      onSettled: () => void client.invalidateQueries({ queryKey: PAUSED }),
    }),
    save: useMutation({
      mutationFn: ({
        ids,
        format,
        destination,
      }: {
        ids: string[];
        format: AudioFileFormat;
        destination: SongDestination;
      }) => unwrapResult(commands.songsSave(ids, format, destination)),
    }),
  };
};
