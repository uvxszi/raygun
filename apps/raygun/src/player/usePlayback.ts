import { commands, events } from '@raygun/bindings';
import type {
  ConnectStatus,
  PlaybackControl,
  PlaybackState,
  QueueItem,
  QueueSnapshot,
  RepeatMode,
} from '@raygun/bindings';
import { useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useCallback, useEffect } from 'react';

import { useConnectSession, useConnectSync } from '../connect/useConnect';
import { markReceived, positionAt } from './useLivePosition';

const PLAYBACK_STATE_KEY = ['playbackState'] as const;
const QUEUE_KEY = ['queue'] as const;
/** Connect's status (`connect/useConnect.ts`): while it plays elsewhere, that player shows. */
const CONNECT_KEY = ['connect', 'status'] as const;

/** Keeps the query cache in sync with state pushed from Rust. Mount once, in the app shell. */
export const usePlaybackSync = () => {
  const queryClient = useQueryClient();
  useConnectSync();
  useEffect(() => {
    const unlistenState = events.playbackStateEvent.listen((event) => {
      markReceived(event.payload);
      queryClient.setQueryData(PLAYBACK_STATE_KEY, event.payload);
    });
    const unlistenQueue = events.queueEvent.listen((event) =>
      queryClient.setQueryData(QUEUE_KEY, event.payload),
    );
    return () => {
      void unlistenState.then((stop) => stop());
      void unlistenQueue.then((stop) => stop());
    };
  }, [queryClient]);
};

/**
 * The player's state: this device's, or while it plays on another (M46.15) that device's, so
 * every player view shows and controls what plays there. It changes with every report; views
 * that need only part of it use {@link usePlayerSelect}.
 */
export const usePlaybackState = () => {
  const local = useQuery<PlaybackState>({
    queryKey: PLAYBACK_STATE_KEY,
    queryFn: () => commands.playbackGetState(),
    staleTime: Infinity,
  });
  const session = useConnectSession();
  return session ? { ...local, data: session.state ?? undefined } : local;
};

/** The queue, also from the other device while playing there (M46.15). */
export const useQueue = () => {
  const local = useQuery<QueueSnapshot>({
    queryKey: QUEUE_KEY,
    queryFn: () => commands.playbackGetQueue(),
    staleTime: Infinity,
  });
  const session = useConnectSession();
  return session ? { ...local, data: session.queue ?? undefined } : local;
};

/** A selected part, boxed so `undefined` can be one. */
type Picked<T> = { value: T };

/**
 * Part of the shown player's state (`select` runs on it): the component re-renders only when
 * that part changes, never for a position report it doesn't use. Equal values keep their
 * identity, so objects and lists can be selected too.
 */
export const usePlayerSelect = <T>(select: (state: PlaybackState | undefined) => T): T => {
  const remote = useQuery({
    queryKey: CONNECT_KEY,
    queryFn: () => commands.connectStatus(),
    staleTime: Infinity,
    select: (status: ConnectStatus): Picked<T> | null =>
      status.session ? { value: select(status.session.state ?? undefined) } : null,
    notifyOnChangeProps: ['data'],
  }).data;
  const local = useQuery({
    queryKey: PLAYBACK_STATE_KEY,
    queryFn: () => commands.playbackGetState(),
    staleTime: Infinity,
    select: (state: PlaybackState): Picked<T> => ({ value: select(state) }),
    notifyOnChangeProps: ['data'],
  }).data;
  if (remote) return remote.value;
  return local ? local.value : select(undefined);
};

/** Part of the shown queue, as {@link usePlayerSelect} is of the state. */
export const useQueueSelect = <T>(select: (queue: QueueSnapshot | undefined) => T): T => {
  const remote = useQuery({
    queryKey: CONNECT_KEY,
    queryFn: () => commands.connectStatus(),
    staleTime: Infinity,
    select: (status: ConnectStatus): Picked<T> | null =>
      status.session ? { value: select(status.session.queue ?? undefined) } : null,
    notifyOnChangeProps: ['data'],
  }).data;
  const local = useQuery({
    queryKey: QUEUE_KEY,
    queryFn: () => commands.playbackGetQueue(),
    staleTime: Infinity,
    select: (queue: QueueSnapshot): Picked<T> => ({ value: select(queue) }),
    notifyOnChangeProps: ['data'],
  }).data;
  if (remote) return remote.value;
  return local ? local.value : select(undefined);
};

const currentIdOf = (state: PlaybackState | undefined) => state?.currentItemId ?? null;

/** The id of the item playing (or paused), if any. */
export const useCurrentItemId = () => usePlayerSelect(currentIdOf);

/** What the playhead needs: changes with every report, so only small views should use it. */
const timingOf = (state: PlaybackState | undefined): PlaybackState | undefined => state;

/** The whole state, for views that follow the playhead (seek bars, lyrics). */
export const usePlayerTiming = () => usePlayerSelect(timingOf);

/** The items after `currentId` in play order; with nothing current, the whole queue. */
export const upNextOf = (queue: QueueSnapshot | undefined, currentId: string | null) => {
  if (!queue) return [];
  const currentIndex = queue.items.findIndex((candidate) => candidate.id === currentId);
  const position = currentIndex >= 0 ? queue.order.indexOf(currentIndex) : -1;
  return queue.order.slice(position + 1).flatMap((index) => queue.items[index] ?? []);
};

/** The item playing (or paused): re-renders when it changes, not with each report. */
export const useCurrentItem = (): QueueItem | undefined => {
  const currentId = useCurrentItemId();
  const pick = useCallback(
    (queue: QueueSnapshot | undefined) => queue?.items.find((item) => item.id === currentId),
    [currentId],
  );
  return useQueueSelect(pick);
};

/** What plays after the current item, in play order. */
export const useUpNext = (): QueueItem[] => {
  const currentId = useCurrentItemId();
  const pick = useCallback(
    (queue: QueueSnapshot | undefined) => upNextOf(queue, currentId),
    [currentId],
  );
  return useQueueSelect(pick);
};

/** Whether something plays (or is about to): what Play/Pause shows. */
export const useIsActive = () => usePlayerSelect(isActive);

/** The shown player's state as the cache has it now (Connect's while playing elsewhere). */
export const shownState = (client: QueryClient): PlaybackState | undefined => {
  const session = client.getQueryData<ConnectStatus>(CONNECT_KEY)?.session;
  return session ? (session.state ?? undefined) : client.getQueryData(PLAYBACK_STATE_KEY);
};

/** The shown queue as the cache has it now. */
export const shownQueue = (client: QueryClient): QueueSnapshot | undefined => {
  const session = client.getQueryData<ConnectStatus>(CONNECT_KEY)?.session;
  return session ? (session.queue ?? undefined) : client.getQueryData(QUEUE_KEY);
};

/**
 * Reads where playback is when called (a button that starts from here), without re-rendering
 * as it moves.
 */
export const useReadPosition = () => {
  const client = useQueryClient();
  return useCallback(() => positionAt(shownState(client), performance.now()), [client]);
};

/** The playing item together with the state that drives the player UI. */
export const useNowPlaying = () => {
  const state = usePlaybackState().data;
  const item = useCurrentItem();
  const upNext = useUpNext();
  return { state, item, upNext };
};

export const sendControl = (control: PlaybackControl) => commands.playbackControl(control);

/** What Repeat turns to from each mode: off, all, one, off. */
export const NEXT_REPEAT: Record<RepeatMode, RepeatMode> = { off: 'all', all: 'one', one: 'off' };

export const isActive = (state: PlaybackState | undefined) =>
  state?.status === 'playing' || state?.status === 'loading' || state?.status === 'buffering';

/**
 * Where the stream comes from: "finding" while the resolver looks for one (M14), else the
 * provider it picked, or `null` for items that carry their own source.
 */
export const sourceNote = (
  state: PlaybackState | undefined,
  item: QueueItem | undefined,
): { kind: 'finding' } | { kind: 'via'; source: string } | null => {
  if (state?.sourceLabel) return { kind: 'via', source: state.sourceLabel };
  if (item?.source.kind === 'lookup' && state?.status === 'loading') return { kind: 'finding' };
  return null;
};

/** {@link sourceNote} for the playing item, re-rendering only when it changes. */
export const useSourceNote = (item: QueueItem | undefined) => {
  const lookup = item?.source.kind === 'lookup';
  const label = usePlayerSelect((state) => state?.sourceLabel ?? null);
  const loading = usePlayerSelect((state) => state?.status === 'loading');
  if (label) return { kind: 'via', source: label } as const;
  if (lookup && loading) return { kind: 'finding' } as const;
  return null;
};
