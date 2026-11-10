import { commands } from '@raygun/bindings';
import type { VideoRequest } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import { WATCH_PROGRESS_KEYS } from '../shell/useLiveInvalidation';

const WATCH = ['watch'];
const CONTINUE = [...WATCH, 'continue'];
const HISTORY = [...WATCH, 'history'];

/**
 * What to continue watching, one entry per title; kept current as the player reports progress
 * (`shell/useLiveInvalidation`).
 */
export const useContinueWatching = () =>
  useQuery({
    queryKey: CONTINUE,
    queryFn: () => unwrapResult(commands.videoContinueWatching()),
  });

/**
 * Everything watched or started, newest first; kept current as the player reports progress
 * (`shell/useLiveInvalidation`).
 */
export const useWatchHistory = () =>
  useQuery({
    queryKey: HISTORY,
    queryFn: () => unwrapResult(commands.videoHistory()),
  });

export const usePlayVideo = () =>
  useMutation({ mutationFn: (request: VideoRequest) => unwrapResult(commands.videoPlay(request)) });

export const usePlayLink = () =>
  useMutation({
    mutationFn: ({ url, subtitleUrl }: { url: string; subtitleUrl: string | null }) =>
      unwrapResult(commands.videoPlayLink(url, null, subtitleUrl)),
  });

export const useResumeVideo = () =>
  useMutation({ mutationFn: (key: string) => unwrapResult(commands.videoResume(key)) });

export const useRemoveContinue = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (key: string) => unwrapResult(commands.videoRemove(key)),
    // Continue watching and the history both list it.
    onSettled: () => queryClient.invalidateQueries({ queryKey: WATCH }),
  });
};

/** Takes several entries out of Continue watching (and the history); resolves to how many. */
export const useRemoveContinueMany = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (keys: string[]) => unwrapResult(commands.videoRemoveMany(keys)),
    onSettled: () => queryClient.invalidateQueries({ queryKey: WATCH }),
  });
};

/**
 * Marks an entry watched without moving on to its next episode: it leaves Continue watching, and
 * its title's page shows the episode watched.
 */
export const useMarkContinueWatched = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (key: string) => unwrapResult(commands.videoMarkWatched(key)),
    onSettled: () =>
      Promise.all(
        WATCH_PROGRESS_KEYS.map((queryKey) => queryClient.invalidateQueries({ queryKey })),
      ),
  });
};
