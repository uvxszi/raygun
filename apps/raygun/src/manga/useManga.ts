import { commands, events } from '@raygun/bindings';
import type { MangaBrowseMode, MangaUpdateStatus } from '@raygun/bindings';
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

/** Every manga query lives under this key. */
export const MANGA = 'manga';

/** Catalogue pages stay fresh for a while, so going back to a source doesn't refetch. */
const CATALOGUE_STALE_MS = 5 * 60_000;

export const useMangaSources = () =>
  useQuery({
    queryKey: [MANGA, 'sources'],
    queryFn: () => unwrapResult(commands.mangaSources()),
  });

export const useMangaBrowse = (sourceId: string, mode: MangaBrowseMode, query: string) =>
  useInfiniteQuery({
    queryKey: [MANGA, 'browse', sourceId, mode, query],
    queryFn: ({ pageParam }) =>
      unwrapResult(
        commands.mangaBrowse(sourceId, mode, mode === 'search' ? query : null, pageParam),
      ),
    initialPageParam: 1,
    getNextPageParam: (last, pages) => (last.hasNextPage ? pages.length + 1 : undefined),
    enabled: Boolean(sourceId) && (mode !== 'search' || query.trim().length > 0),
    staleTime: CATALOGUE_STALE_MS,
  });

export const useMangaTitle = (sourceId: string, mangaId: string) =>
  useQuery({
    queryKey: [MANGA, 'title', sourceId, mangaId],
    queryFn: () => unwrapResult(commands.mangaTitle(sourceId, mangaId, false)),
    enabled: Boolean(sourceId && mangaId),
  });

export const useMangaLibrary = () =>
  useQuery({
    queryKey: [MANGA, 'library'],
    queryFn: () => unwrapResult(commands.mangaLibrary()),
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

/** A mutation that refreshes manga titles, the library and browse badges once it lands. */
const useMangaMutation = <Variables, Result>(run: (variables: Variables) => Promise<Result>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] }),
        queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] }),
        queryClient.invalidateQueries({ queryKey: [MANGA, 'browse'] }),
      ]),
  });
};

export const useRefreshTitle = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ sourceId, mangaId }: { sourceId: string; mangaId: string }) =>
      unwrapResult(commands.mangaTitle(sourceId, mangaId, true)),
    onSuccess: (view) =>
      queryClient.setQueryData([MANGA, 'title', view.sourceId, view.remoteId], view),
  });
};

export const useSetInLibrary = () =>
  useMangaMutation(({ titleId, inLibrary }: { titleId: string; inLibrary: boolean }) =>
    unwrapResult(commands.mangaSetInLibrary(titleId, inLibrary)),
  );

export const useSetChapterRead = () =>
  useMangaMutation(({ chapterId, read }: { chapterId: string; read: boolean }) =>
    unwrapResult(commands.mangaSetRead(chapterId, read)),
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

const invalidateDownloads = (queryClient: ReturnType<typeof useQueryClient>) =>
  Promise.all([
    queryClient.invalidateQueries({ queryKey: [MANGA, 'downloads'] }),
    queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] }),
    queryClient.invalidateQueries({ queryKey: [MANGA, 'updates'] }),
  ]);

/**
 * Follows library updates and the download queue from Rust: update progress lands in the status
 * query, and a finished update or any queue change refreshes what shows chapters.
 */
export const useMangaLiveEvents = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlistenUpdate = events.mangaUpdateEvent.listen(({ payload }) => {
      queryClient.setQueryData<MangaUpdateStatus>([MANGA, 'updateStatus'], payload);
      if (!payload.running) {
        void Promise.all([
          queryClient.invalidateQueries({ queryKey: [MANGA, 'library'] }),
          queryClient.invalidateQueries({ queryKey: [MANGA, 'updates'] }),
          queryClient.invalidateQueries({ queryKey: [MANGA, 'title'] }),
        ]);
      }
    });
    const unlistenDownloads = events.mangaDownloadsEvent.listen(() => {
      void invalidateDownloads(queryClient);
    });
    return () => {
      void unlistenUpdate.then((unlisten) => unlisten());
      void unlistenDownloads.then((unlisten) => unlisten());
    };
  }, [queryClient]);
};
