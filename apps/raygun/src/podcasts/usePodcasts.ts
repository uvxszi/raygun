import { commands, events } from '@raygun/bindings';
import type { EpisodeOrder, PodcastView } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { rememberPlay } from '../music/playContext';

const PODCASTS = 'podcasts';
/** Audiobooks' queries (`audiobooks/useAudiobooks.ts`) change with the same commands. */
const BOOKS = 'audiobooks';
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

/** Episodes and books started and not finished, the last listened first (M46.48). */
export const useContinueListening = (limit: number) =>
  useQuery({
    queryKey: [PODCASTS, 'continue', limit],
    queryFn: () => unwrapResult(commands.podcastContinueListening(limit)),
  });

const usePodcastMutation = <Args>(run: (args: Args) => Promise<unknown>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: async () => {
      await Promise.all([
        queryClient.invalidateQueries({ queryKey: [PODCASTS] }),
        queryClient.invalidateQueries({ queryKey: [BOOKS] }),
      ]);
    },
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
  useMutation({
    mutationFn: async (id: string) => {
      await rememberPlay();
      return unwrapResult(commands.podcastPlay(id));
    },
  });

export const useSetPlayed = () =>
  usePodcastMutation(({ id, played }: { id: string; played: boolean }) =>
    unwrapResult(commands.podcastSetPlayed(id, played)),
  );

/** Every episode of a show (or chapter of a book) played or unplayed (M46.48). */
export const useSetAllPlayed = () =>
  usePodcastMutation(({ id, played }: { id: string; played: boolean }) =>
    unwrapResult(commands.podcastSetAllPlayed(id, played)),
  );

/** The episodes before this one played: older ones, or a book's earlier chapters (M46.48). */
export const useSetEarlierPlayed = () =>
  usePodcastMutation((episodeId: string) =>
    unwrapResult(commands.podcastSetEarlierPlayed(episodeId)),
  );

/** Forgets where listening stopped, without marking it played (M46.48). */
export const useForgetPosition = () =>
  usePodcastMutation((episodeId: string) =>
    unwrapResult(commands.podcastForgetPosition(episodeId)),
  );

/** How a show's or book's episodes list, kept for it; `null` is its kind's own (M46.48). */
export const useSetEpisodeOrder = () =>
  usePodcastMutation(({ id, order }: { id: string; order: EpisodeOrder | null }) =>
    unwrapResult(commands.podcastSetEpisodeOrder(id, order)),
  );

/** The speed a show's or book's episodes play at; `null` leaves the player's (M46.48). */
export const useSetShowSpeed = () =>
  usePodcastMutation(({ id, speed }: { id: string; speed: number | null }) =>
    unwrapResult(commands.podcastSetSpeed(id, speed)),
  );

export const useDownloadEpisode = () =>
  usePodcastMutation((id: string) => unwrapResult(commands.podcastDownload(id)));

export const useRemoveEpisodeDownload = () =>
  usePodcastMutation((id: string) => unwrapResult(commands.podcastRemoveDownload(id)));

/** Stops a download that's waiting or under way (M46.48). */
export const useCancelEpisodeDownload = () =>
  usePodcastMutation((id: string) => unwrapResult(commands.podcastCancelDownload(id)));

/** Deletes every download of a show or book (M46.48). */
export const useRemoveDownloads = () =>
  usePodcastMutation((id: string) => unwrapResult(commands.podcastRemoveDownloads(id)));

/**
 * Runs what a selection of shows, books, episodes or chapters does to all of them (the
 * `podcast*Picks` and `*Many` commands); its answer is what to say after. Shows and books are
 * listed again once it's done.
 */
export const usePodcastPicksAction = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (action: () => Promise<string | null>) => action(),
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: [PODCASTS] }),
        queryClient.invalidateQueries({ queryKey: [BOOKS] }),
      ]),
  });
};

/** Opens a show's or book's website, or a link from its notes, in Raygun's browser (M46.48). */
export const useOpenPodcastPage = () =>
  useMutation({ mutationFn: (url: string) => unwrapResult(commands.podcastOpenPage(url)) });
