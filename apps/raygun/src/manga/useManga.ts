import { commands, events } from '@raygun/bindings';
import type { MangaBrowseMode, MangaUpdateStatus } from '@raygun/bindings';
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { useSourceHealth } from '../search/sourceHealth';
import { browseCommands, catalogCommands, libraryCommand } from './medium';
import type { Medium, ReadingMedium } from './medium';

/** Every manga query lives under this key. */
export const MANGA = 'manga';

/** Catalogue pages stay fresh for a while, so going back to a source doesn't refetch. */
const CATALOGUE_STALE_MS = 5 * 60_000;

export const useMangaSources = (medium: Medium = 'manga') =>
  useQuery({
    queryKey: [medium, 'sources'],
    queryFn: () => unwrapResult(browseCommands(medium).sources()),
  });

export const useMangaBrowse = (
  sourceId: string,
  mode: MangaBrowseMode,
  query: string,
  medium: Medium = 'manga',
) =>
  useInfiniteQuery({
    queryKey: [medium, 'browse', sourceId, mode, query],
    queryFn: async ({ pageParam }) => {
      // Whether the source answers, so lists can put failing sources last (M46.9).
      const health = useSourceHealth.getState();
      try {
        const page = await unwrapResult(
          browseCommands(medium).browse(
            sourceId,
            mode,
            mode === 'search' ? query : null,
            pageParam,
          ),
        );
        health.worked(sourceId);
        return page;
      } catch (error) {
        health.failed(sourceId, error instanceof Error ? error.message : String(error));
        throw error;
      }
    },
    initialPageParam: 1,
    getNextPageParam: (last, pages) => (last.hasNextPage ? pages.length + 1 : undefined),
    enabled: Boolean(sourceId) && (mode !== 'search' || query.trim().length > 0),
    staleTime: CATALOGUE_STALE_MS,
  });

export const useMangaTitle = (sourceId: string, mangaId: string, medium: ReadingMedium = 'manga') =>
  useQuery({
    queryKey: [medium, 'title', sourceId, mangaId],
    queryFn: () => unwrapResult(catalogCommands(medium).title(sourceId, mangaId, false)),
    enabled: Boolean(sourceId && mangaId),
  });

export const useMangaLibrary = (medium: Medium = 'manga') =>
  useQuery({
    queryKey: [medium, 'library'],
    queryFn: () => unwrapResult(libraryCommand(medium)()),
  });

export const useMangaChapter = (titleId: string, chapterId: string) =>
  useQuery({
    queryKey: [MANGA, 'chapter', titleId, chapterId],
    queryFn: () => unwrapResult(commands.mangaChapter(titleId, chapterId)),
    enabled: Boolean(titleId && chapterId),
    // Page links come from a short-lived server list; refetch when coming back later.
    staleTime: 0,
    gcTime: 60_000,
  });

/** A mutation that refreshes titles, the library and browse badges of a medium once it lands. */
const useMangaMutation = <Variables, Result>(
  medium: ReadingMedium,
  run: (variables: Variables) => Promise<Result>,
) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: [medium, 'title'] }),
        queryClient.invalidateQueries({ queryKey: [medium, 'library'] }),
        queryClient.invalidateQueries({ queryKey: [medium, 'browse'] }),
      ]),
  });
};

export const useRefreshTitle = (medium: ReadingMedium = 'manga') => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ sourceId, mangaId }: { sourceId: string; mangaId: string }) =>
      unwrapResult(catalogCommands(medium).title(sourceId, mangaId, true)),
    onSuccess: (view) =>
      queryClient.setQueryData([medium, 'title', view.sourceId, view.remoteId], view),
  });
};

export const useSetInLibrary = (medium: ReadingMedium = 'manga') =>
  useMangaMutation(medium, ({ titleId, inLibrary }: { titleId: string; inLibrary: boolean }) =>
    unwrapResult(catalogCommands(medium).setInLibrary(titleId, inLibrary)),
  );

export const useSetChapterRead = (medium: ReadingMedium = 'manga') =>
  useMangaMutation(medium, ({ chapterId, read }: { chapterId: string; read: boolean }) =>
    unwrapResult(catalogCommands(medium).setRead(chapterId, read)),
  );

/** Marks chosen chapters read or unread together. */
export const useSetChaptersRead = (medium: ReadingMedium = 'manga') =>
  useMangaMutation(medium, ({ chapterIds, read }: { chapterIds: string[]; read: boolean }) =>
    unwrapResult(commands.librarySetChaptersRead(medium, chapterIds, read)),
  );

/** Marks every chapter before one read (Mihon's "Mark previous as read"). */
export const useMarkPreviousRead = (medium: ReadingMedium = 'manga') =>
  useMangaMutation(medium, ({ titleId, chapterId }: { titleId: string; chapterId: string }) =>
    unwrapResult(commands.libraryMarkPreviousRead(medium, titleId, chapterId)),
  );

/** Saves reader progress without refreshing anything; the title page refetches when shown. */
export const saveProgress = (chapterId: string, page: number, total: number) =>
  unwrapResult(commands.mangaSetProgress(chapterId, page, total));

export const useMangaUpdates = () =>
  useQuery({
    queryKey: [MANGA, 'updates'],
    queryFn: () => unwrapResult(commands.mangaUpdates()),
  });

export const useMangaUpdateStatus = () =>
  useQuery({
    queryKey: [MANGA, 'updateStatus'],
    queryFn: () => unwrapResult(commands.mangaUpdateStatus()),
  });

export const useUpdateLibrary = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(commands.mangaUpdateLibrary()),
    onSuccess: (status) => queryClient.setQueryData([MANGA, 'updateStatus'], status),
  });
};

export const useMangaDownloads = () =>
  useQuery({
    queryKey: [MANGA, 'downloads'],
    queryFn: () => unwrapResult(commands.mangaDownloads()),
  });

/** A mutation on the download queue; the queue and download badges refresh once it lands. */
const useDownloadMutation = <Variables>(run: (variables: Variables) => Promise<unknown>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () => invalidateDownloads(queryClient),
  });
};

export const useDownloadChapters = () =>
  useDownloadMutation((chapterIds: string[]) => unwrapResult(commands.mangaDownload(chapterIds)));

export const useRemoveDownloads = () =>
  useDownloadMutation((chapterIds: string[]) =>
    unwrapResult(commands.mangaRemoveDownloads(chapterIds)),
  );

export const useSetDownloadsPaused = () =>
  useDownloadMutation((paused: boolean) => unwrapResult(commands.mangaSetDownloadsPaused(paused)));

const invalidateDownloads = (queryClient: QueryClient) =>
  Promise.all([
    queryClient.invalidateQueries({ queryKey: [MANGA, 'downloads'] }),
    queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] }),
    queryClient.invalidateQueries({ queryKey: [MANGA, 'updates'] }),
    // Novel chapters share the download queue.
    queryClient.invalidateQueries({ queryKey: ['novel', 'title'] }),
  ]);

/** The queue and titles are read again at most this often while chapters download. */
const DOWNLOADS_REFRESH_MS = 500;

/** Listens to Rust's update and download events for one query client; returns how to stop. */
const listenLive = (queryClient: QueryClient) => {
  const unlistenUpdate = events.mangaUpdateEvent.listen(({ payload }) => {
    queryClient.setQueryData<MangaUpdateStatus>([MANGA, 'updateStatus'], payload);
    if (!payload.running) {
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] }),
        queryClient.invalidateQueries({ queryKey: [MANGA, 'updates'] }),
        queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] }),
        // Books from sources are updated too.
        queryClient.invalidateQueries({ queryKey: ['novel', 'library'] }),
        queryClient.invalidateQueries({ queryKey: ['novel', 'title'] }),
        // The update covers saved movies and series too (M43).
        queryClient.invalidateQueries({ queryKey: ['video', 'library'] }),
        queryClient.invalidateQueries({ queryKey: ['video', 'updates'] }),
        queryClient.invalidateQueries({ queryKey: ['video', 'upcoming'] }),
      ]);
    }
  });
  // A chapter downloading reports every few pages: the queue and title pages follow at most twice
  // a second (the first report at once), the library's badges once the queue settles.
  let lastRefresh = 0;
  let downloadsRefresh: ReturnType<typeof setTimeout> | undefined;
  let libraryRefresh: ReturnType<typeof setTimeout> | undefined;
  const refreshDownloads = () => {
    lastRefresh = Date.now();
    downloadsRefresh = undefined;
    void invalidateDownloads(queryClient);
  };
  const unlistenDownloads = events.mangaDownloadsEvent.listen(() => {
    if (downloadsRefresh === undefined) {
      const wait = lastRefresh + DOWNLOADS_REFRESH_MS - Date.now();
      if (wait <= 0) refreshDownloads();
      else downloadsRefresh = setTimeout(refreshDownloads, wait);
    }
    clearTimeout(libraryRefresh);
    libraryRefresh = setTimeout(() => {
      void Promise.all([
        queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] }),
        queryClient.invalidateQueries({ queryKey: ['novel', 'library'] }),
      ]);
    }, 1000);
  });
  return () => {
    clearTimeout(downloadsRefresh);
    clearTimeout(libraryRefresh);
    void unlistenUpdate.then((unlisten) => unlisten());
    void unlistenDownloads.then((unlisten) => unlisten());
  };
};

/** One set of listeners per query client, however many screens ask for them. */
const live = new WeakMap<QueryClient, { users: number; stop: () => void }>();

/**
 * Follows library updates and the download queue from Rust: update progress lands in the status
 * query, and a finished update or any queue change refreshes what shows chapters. Screens that
 * are open together (the desktop bar's popovers and a page) share one listener.
 */
export const useMangaLiveEvents = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const known = live.get(queryClient);
    if (known) known.users += 1;
    else live.set(queryClient, { users: 1, stop: listenLive(queryClient) });
    return () => {
      const entry = live.get(queryClient);
      if (!entry) return;
      entry.users -= 1;
      if (entry.users === 0) {
        entry.stop();
        live.delete(queryClient);
      }
    };
  }, [queryClient]);
};
