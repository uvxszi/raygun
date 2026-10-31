import { commands, events } from '@raygun/bindings';
import type { PlaybackControl, PlaybackState, QueueSnapshot } from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

const PLAYBACK_STATE_KEY = ['playbackState'] as const;
const QUEUE_KEY = ['queue'] as const;

/** Keeps the query cache in sync with state pushed from Rust. Mount once, in the app shell. */
export const usePlaybackSync = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlistenState = events.playbackStateEvent.listen((event) =>
      queryClient.setQueryData(PLAYBACK_STATE_KEY, event.payload),
    );
    const unlistenQueue = events.queueEvent.listen((event) =>
      queryClient.setQueryData(QUEUE_KEY, event.payload),
    );
    return () => {
      void unlistenState.then((stop) => stop());
      void unlistenQueue.then((stop) => stop());
    };
  }, [queryClient]);
};

export const usePlaybackState = () =>
  useQuery<PlaybackState>({
    queryKey: PLAYBACK_STATE_KEY,
    queryFn: () => commands.playbackGetState(),
    staleTime: Infinity,
  });

export const useQueue = () =>
  useQuery<QueueSnapshot>({
    queryKey: QUEUE_KEY,
    queryFn: () => commands.playbackGetQueue(),
    staleTime: Infinity,
  });

/** The playing item together with the state that drives the player UI. */
export const useNowPlaying = () => {
  const state = usePlaybackState().data;
  const queue = useQueue().data;
  const currentIndex = queue?.items.findIndex((candidate) => candidate.id === state?.currentItemId);
  const item = currentIndex !== undefined ? queue?.items[currentIndex] : undefined;
  // -1 when nothing is current, so the whole queue counts as "up next".
  const position = queue && currentIndex !== undefined ? queue.order.indexOf(currentIndex) : -1;
  const upNext = queue
    ? queue.order.slice(position + 1).flatMap((index) => queue.items[index] ?? [])
    : [];
  return { state, item, upNext };
};

export const sendControl = (control: PlaybackControl) => commands.playbackControl(control);

export const isActive = (state: PlaybackState | undefined) =>
  state?.status === 'playing' || state?.status === 'loading' || state?.status === 'buffering';
