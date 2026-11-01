import { commands, events } from '@raygun/bindings';
import type { VideoSourcePlay } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const VIDEO = 'video';

export const useVideoSources = () =>
  useQuery({
    queryKey: [VIDEO, 'sources'],
    queryFn: () => unwrapResult(commands.videoSources()),
  });

/** A title's page; progress on it follows the player. */
export const useVideoTitle = (sourceId: string, remoteId: string) => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.watchChangedEvent.listen(() => {
      void queryClient.invalidateQueries({ queryKey: [VIDEO, 'title'] });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
  return useQuery({
    queryKey: [VIDEO, 'title', sourceId, remoteId],
    queryFn: () => unwrapResult(commands.videoTitle(sourceId, remoteId, false)),
    enabled: Boolean(sourceId && remoteId),
  });
};

export const useRefreshVideoTitle = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({ sourceId, remoteId }: { sourceId: string; remoteId: string }) =>
      unwrapResult(commands.videoTitle(sourceId, remoteId, true)),
    onSuccess: (view) =>
      queryClient.setQueryData([VIDEO, 'title', view.sourceId, view.remoteId], view),
  });
};

/** An episode's servers; `episodeId` is the source's id, null until one is picked. */
export const useVideoServers = (sourceId: string, episodeId: string | null) =>
  useQuery({
    queryKey: [VIDEO, 'servers', sourceId, episodeId],
    queryFn: () => unwrapResult(commands.videoServers(sourceId, episodeId ?? '')),
    enabled: episodeId !== null,
    staleTime: 60_000,
  });

/** One server's links, asked for only when the source didn't send them with the server. */
export const useVideoLinks = (sourceId: string, serverId: string, enabled: boolean) =>
  useQuery({
    queryKey: [VIDEO, 'links', sourceId, serverId],
    queryFn: () => unwrapResult(commands.videoLinks(sourceId, serverId)),
    enabled,
    staleTime: 60_000,
    retry: false,
  });

export const useVideoSourcePlay = () =>
  useMutation({
    mutationFn: (request: VideoSourcePlay) => unwrapResult(commands.videoSourcePlay(request)),
  });

export const useSetEpisodeWatched = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: ({
      titleId,
      episodeId,
      watched,
    }: {
      titleId: string;
      episodeId: string;
      watched: boolean;
    }) => unwrapResult(commands.videoSetEpisodeWatched(titleId, episodeId, watched)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: [VIDEO, 'title'] }),
  });
};
