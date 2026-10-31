import { commands, events } from '@raygun/bindings';
import type { OnlineTrack, QueueItemInput } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const EXTENSIONS = ['extensions'];
const ONLINE_SEARCH = ['onlineSearch'];

/** Results stay fresh for a while, so going back to Search doesn't re-run the query. */
const RESULTS_STALE_MS = 5 * 60_000;

export const useOnlineSearch = (query: string) => {
  const trimmed = query.trim();
  return useQuery({
    queryKey: [...ONLINE_SEARCH, trimmed],
    queryFn: () => unwrapResult(commands.onlineSearch(trimmed)),
    enabled: trimmed.length > 0,
    staleTime: RESULTS_STALE_MS,
  });
};

export const useExtensions = () =>
  useQuery({
    queryKey: EXTENSIONS,
    queryFn: () => unwrapResult(commands.extensionsList()),
  });

/**
 * Follows extension reloads from Rust (a dev build reloads on every `raygun-ext dev` install):
 * refreshes the list and drops search results from the old code. Mount once.
 */
export const useExtensionsSync = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.extensionsChangedEvent.listen((event) => {
      queryClient.setQueryData(EXTENSIONS, event.payload);
      void queryClient.invalidateQueries({ queryKey: ONLINE_SEARCH });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
};

/** Reloads every extension; the change event then refreshes the UI. */
export const useReloadExtensions = () =>
  useMutation({ mutationFn: () => unwrapResult(commands.extensionsReload()) });

export const toQueueItem = (track: OnlineTrack): QueueItemInput | null =>
  track.source
    ? {
        title: track.title,
        artist: track.artist,
        album: track.album,
        durationMs: track.durationMs,
        artworkUrl: track.artworkUrl,
        mediaKind: track.source.kind === 'youTube' ? 'video' : 'audio',
        source: track.source,
      }
    : null;

/** Replaces the queue with a provider's playable results, starting at `track`. */
export const playResults = (tracks: OnlineTrack[], track: OnlineTrack) => {
  const playable = tracks.filter((candidate) => candidate.source);
  const items = playable.map(toQueueItem).filter((item) => item !== null);
  const start = playable.indexOf(track);
  return commands.playbackReplace(items, Math.max(start, 0));
};
