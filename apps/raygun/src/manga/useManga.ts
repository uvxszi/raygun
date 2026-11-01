import { commands, events } from '@raygun/bindings';
import type { MangaBrowseMode, MangaUpdateStatus } from '@raygun/bindings';
import { useInfiniteQuery, useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { browseCommands, catalogCommands } from './medium';
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
    queryFn: ({ pageParam }) =>
      unwrapResult(
        browseCommands(medium).browse(sourceId, mode, mode === 'search' ? query : null, pageParam),
      ),
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

export const useMangaLibrary = (medium: ReadingMedium = 'manga') =>
  useQuery({
    queryKey: [medium, 'library'],
    queryFn: () => unwrapResult(catalogCommands(medium).library()),
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
    // Novel chapters share the download queue.
    queryClient.invalidateQueries({ queryKey: ['novel', 'title'] }),
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
