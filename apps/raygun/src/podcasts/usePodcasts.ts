import { commands, events } from '@raygun/bindings';
import type { PodcastView } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const PODCASTS = 'podcasts';
const LATEST_LIMIT = 20;

/** Refreshes podcast data when Rust reports changes (refreshes, downloads, finished episodes). */
export const usePodcastEvents = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.podcastsChangedEvent.listen(() => {
      void queryClient.invalidateQueries({ queryKey: [PODCASTS] });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
};

export const useSubscriptions = () =>
  useQuery({
    queryKey: [PODCASTS, 'subscriptions'],
    queryFn: () => unwrapResult(commands.podcastSubscriptions()),
  });

export const useLatestEpisodes = () =>
  useQuery({
    queryKey: [PODCASTS, 'latest'],
    queryFn: () => unwrapResult(commands.podcastLatest(LATEST_LIMIT)),
  });

export const usePodcast = (id: string) =>
  useQuery({
    queryKey: [PODCASTS, 'podcast', id],
    queryFn: () => unwrapResult(commands.podcastGet(id, false)),
    enabled: id !== '',
  });

export const useTopPodcasts = (enabled: boolean) =>
  useQuery({
    queryKey: [PODCASTS, 'top'],
    queryFn: () => unwrapResult(commands.podcastTop()),
    enabled,
    staleTime: 60 * 60 * 1000,
  });

export const usePodcastSearch = (query: string) =>
  useQuery({
    queryKey: [PODCASTS, 'search', query],
    queryFn: () => unwrapResult(commands.podcastSearch(query)),
    enabled: query.trim() !== '',
    staleTime: 10 * 60 * 1000,
  });

const usePodcastMutation = <Args>(run: (args: Args) => Promise<unknown>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () => queryClient.invalidateQueries({ queryKey: [PODCASTS] }),
  });
};

/** Opens a feed (directory result or typed address) and stores it, returning its page data. */
export const useOpenFeed = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (feedUrl: string) => unwrapResult(commands.podcastOpenFeed(feedUrl)),
    onSuccess: (view: PodcastView) =>
      queryClient.setQueryData([PODCASTS, 'podcast', view.id], view),
  });
};

export const useRefreshPodcast = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (id: string) => unwrapResult(commands.podcastGet(id, true)),
    onSuccess: (view: PodcastView) =>
      queryClient.setQueryData([PODCASTS, 'podcast', view.id], view),
  });
};

export const useSetSubscribed = () =>
  usePodcastMutation(({ id, subscribed }: { id: string; subscribed: boolean }) =>
    unwrapResult(commands.podcastSetSubscribed(id, subscribed)),
  );

export const useRefreshAll = () =>
  usePodcastMutation(() => unwrapResult(commands.podcastRefreshAll()));

export const usePlayEpisode = () =>
  useMutation({ mutationFn: (id: string) => unwrapResult(commands.podcastPlay(id)) });

export const useSetPlayed = () =>
  usePodcastMutation(({ id, played }: { id: string; played: boolean }) =>
    unwrapResult(commands.podcastSetPlayed(id, played)),
  );

export const useDownloadEpisode = () =>
  usePodcastMutation((id: string) => unwrapResult(commands.podcastDownload(id)));

export const useRemoveEpisodeDownload = () =>
  usePodcastMutation((id: string) => unwrapResult(commands.podcastRemoveDownload(id)));
