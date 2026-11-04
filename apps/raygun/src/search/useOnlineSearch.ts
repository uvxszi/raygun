import { commands } from '@raygun/bindings';
import type { OnlineTrack, QueueItemInput, SearchResults } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useMemo } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { rememberPlay } from '../music/playContext';
import { keepSongs, useSearchFilter } from './searchFilter';
import { SEARCH_KEYS, streamed, useStreamedAnswers } from './searchStream';

/** Results stay fresh for a while, so going back to Search doesn't re-run the query. */
export const RESULTS_STALE_MS = 5 * 60_000;

/** How many of a search's sources have answered, while it still waits for others. */
export type SearchProgressCount = { answered: number; total: number };

/**
 * Songs for a search from the library and every music source. While the sources are asked,
 * `data` holds those that answered (`progress` says how many of how many), so the first
 * answers show at once.
 */
export const useOnlineSearch = (query: string) => {
  const trimmed = query.trim();
  const key = SEARCH_KEYS.songs(trimmed);
  const search = useQuery({
    queryKey: key,
    queryFn: () => streamed(key, () => unwrapResult(commands.onlineSearch(trimmed))),
    enabled: trimmed.length > 0,
    staleTime: RESULTS_STALE_MS,
  });
  const parts = useStreamedAnswers(key);
  const partial: SearchResults | undefined =
    !search.data && parts && parts.songs.length > 0
      ? { library: [], top: [], providers: parts.songs }
      : undefined;
  const progress: SearchProgressCount | undefined =
    search.isFetching && parts && parts.total > 0
      ? { answered: parts.answered.length, total: parts.total }
      : undefined;
  // Search's "No 18+" and "No explicit" cover answers kept from before they were set.
  const filter = useSearchFilter();
  const shown = search.data ?? partial;
  const data = useMemo(() => shown && keepSongs(filter, shown), [shown, filter]);
  return { ...search, data, progress };
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
  ...(track.explicit ? { explicit: true } : {}),
});

/** Replaces the queue with a provider's results, starting at `track`. */
export const playResults = async (tracks: OnlineTrack[], track: OnlineTrack) => {
  await rememberPlay();
  return commands.playbackReplace(tracks.map(toQueueItem), Math.max(tracks.indexOf(track), 0));
};
