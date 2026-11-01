import { commands, events } from '@raygun/bindings';
import type { StremioPlayRequest, VideoDownloadItem, VideoSourcePlay } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

const VIDEO = 'video';
const DOWNLOADS = [VIDEO, 'downloads'];

/** The source id of titles saved from Stremio addons; their remote id is `<type>:<id>`. */
export const STREMIO_SOURCE = 'stremio';

/** The progress key a source's episode plays (and downloads) under. */
export const sourceKey = (sourceId: string, remoteId: string) => `source:${sourceId}:${remoteId}`;

/** The progress key of a Stremio movie or episode. */
export const stremioKey = (kind: string, videoId: string) => `stremio:${kind}:${videoId}`;

/** Where a saved title opens: the Stremio title page, or the video source's. */
export const videoTitleLink = (
  sourceId: string,
  remoteId: string,
): { to: string; search: Record<string, string> } => {
  if (sourceId === STREMIO_SOURCE) {
    const split = remoteId.indexOf(':');
    return {
      to: '/watch/title',
      search: { type: remoteId.slice(0, split), id: remoteId.slice(split + 1) },
    };
  }
  return { to: '/watch/video', search: { source: sourceId, id: remoteId } };
};

/** How far a download got, from 0 to 1, when it can tell. */
export const downloadShare = (item: VideoDownloadItem): number | null => {
  if (item.totalParts) return item.doneParts / item.totalParts;
  if (item.totalBytes) return (item.doneBytes ?? 0) / item.totalBytes;
  return null;
};

/** Everything that shows saved titles, their episodes or what's downloaded. */
const refreshSaved = (queryClient: QueryClient) =>
  Promise.all([
    queryClient.invalidateQueries({ queryKey: [VIDEO, 'library'] }),
    queryClient.invalidateQueries({ queryKey: [VIDEO, 'title'] }),
    queryClient.invalidateQueries({ queryKey: [VIDEO, 'updates'] }),
    queryClient.invalidateQueries({ queryKey: [VIDEO, 'upcoming'] }),
    queryClient.invalidateQueries({ queryKey: ['stremio', 'meta'] }),
  ]);

export const useVideoUpdates = () =>
  useQuery({
    queryKey: [VIDEO, 'updates'],
    queryFn: () => unwrapResult(commands.videoUpdates()),
  });

export const useVideoUpcoming = () =>
  useQuery({
    queryKey: [VIDEO, 'upcoming'],
    queryFn: () => unwrapResult(commands.videoUpcoming()),
  });

/**
 * The video downloads, kept current from Rust's events. A download finishing (or going away)
 * also refreshes the titles, so their downloaded marks follow.
 */
export const useVideoDownloads = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.videoDownloadsEvent.listen(() => {
      void queryClient.invalidateQueries({ queryKey: DOWNLOADS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
  const downloads = useQuery({
    queryKey: DOWNLOADS,
    queryFn: () => unwrapResult(commands.videoDownloads()),
  });
  const done = downloads.data?.items.filter((item) => item.state === 'done').length;
  const seen = useRef(done);
  useEffect(() => {
    if (done !== undefined && seen.current !== undefined && done !== seen.current) {
      void refreshSaved(queryClient);
    }
    seen.current = done;
  }, [done, queryClient]);
  return downloads;
};

/** Downloads by the progress key they play under. */
export const useDownloadsByKey = () => {
  const downloads = useVideoDownloads();
  return new Map((downloads.data?.items ?? []).map((item) => [item.key, item]));
};

const useDownloadMutation = <Variables>(run: (variables: Variables) => Promise<unknown>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () => queryClient.invalidateQueries({ queryKey: DOWNLOADS }),
  });
};

export const useDownloadSourceVideo = () =>
  useDownloadMutation((request: VideoSourcePlay) =>
    unwrapResult(commands.videoSourceDownload(request)),
  );

/** Downloads a direct stream, with subtitles in the subtitle language first. */
export const useDownloadStremio = () => {
  const { i18n } = useTranslation();
  const subtitleLanguage = useSettings().data?.subtitleLanguage || i18n.language;
  return useDownloadMutation(
    ({ request, titleId }: { request: StremioPlayRequest; titleId: string | null }) =>
      unwrapResult(commands.stremioDownload(request, subtitleLanguage, titleId)),
  );
};

export const useRetryVideoDownload = () =>
  useDownloadMutation((id: string) => unwrapResult(commands.videoDownloadRetry(id)));

export const useRemoveVideoDownload = () =>
  useDownloadMutation((id: string) => unwrapResult(commands.videoDownloadRemove(id)));

export const useSetVideoDownloadsPaused = () =>
  useDownloadMutation((paused: boolean) => unwrapResult(commands.videoDownloadsSetPaused(paused)));

export const usePlayVideoDownload = () =>
  useMutation({ mutationFn: (id: string) => unwrapResult(commands.videoDownloadPlay(id)) });

/** Saves a video source's title to the library, or takes it out. */
export const useSetVideoInLibrary = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ titleId, inLibrary }: { titleId: string; inLibrary: boolean }) =>
      unwrapResult(commands.videoSetInLibrary(titleId, inLibrary)),
    onSettled: () => refreshSaved(queryClient),
  });
};

/** Saves a Stremio movie or series to the library (with its episodes), or takes it out. */
export const useSetStremioInLibrary = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ kind, id, inLibrary }: { kind: string; id: string; inLibrary: boolean }) =>
      unwrapResult(commands.stremioSetInLibrary(kind, id, inLibrary)),
    onSettled: () => refreshSaved(queryClient),
  });
};
