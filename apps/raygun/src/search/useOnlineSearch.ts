import { commands } from '@raygun/bindings';
import type { OnlineTrack, QueueItemInput } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { ONLINE_SEARCH } from '../extensions/useExtensions';
import { unwrapResult } from '../lib/commandResult';
import { rememberPlay } from '../music/playContext';

/** Results stay fresh for a while, so going back to Search doesn't re-run the query. */
const RESULTS_STALE_MS = 5 * 60_000;
/**
 * The search answers without waiting on slow sources (M46.48), naming them `pending`; asking
 * again soon brings their songs (their calls went on meanwhile), a few times at most.
 */
const PENDING_REFETCH_MS = 1500;
const PENDING_REFETCHES = 8;

export const useOnlineSearch = (query: string) => {
  const trimmed = query.trim();
  return useQuery({
    queryKey: [...ONLINE_SEARCH, trimmed],
    queryFn: () => unwrapResult(commands.onlineSearch(trimmed)),
    enabled: trimmed.length > 0,
    staleTime: RESULTS_STALE_MS,
    refetchInterval: (search) =>
      (search.state.data?.pending?.length ?? 0) > 0 &&
      search.state.dataUpdateCount <= PENDING_REFETCHES
        ? PENDING_REFETCH_MS
        : false,
  });
};

/** A search result as a queue item. Results without a stream are found one when they play (M14). */
export const toQueueItem = (track: OnlineTrack): QueueItemInput => ({
  title: track.title,
  artist: track.artist,
  album: track.album,
  durationMs: track.durationMs,
  artworkUrl: track.artworkUrl,
  mediaKind: track.source?.kind === 'youTube' ? 'video' : 'audio',
  source: track.source ?? {
    kind: 'lookup',
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
  },
});

/** Replaces the queue with a provider's results, starting at `track`. */
export const playResults = (tracks: OnlineTrack[], track: OnlineTrack) => {
  rememberPlay();
  return commands.playbackReplace(tracks.map(toQueueItem), Math.max(tracks.indexOf(track), 0));
};
