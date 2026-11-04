import { commands, events } from '@raygun/bindings';
import type { VideoRequest } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const CONTINUE = ['watch', 'continue'];
const HISTORY = ['watch', 'history'];

/** Videos started and not finished, kept current as the player reports progress. */
export const useContinueWatching = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.watchChangedEvent.listen(() => {
      void queryClient.invalidateQueries({ queryKey: CONTINUE });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
  return useQuery({
    queryKey: CONTINUE,
    queryFn: () => unwrapResult(commands.videoContinueWatching()),
  });
};

/** Everything watched or started, newest first, kept current as the player reports progress. */
export const useWatchHistory = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.watchChangedEvent.listen(() => {
      void queryClient.invalidateQueries({ queryKey: HISTORY });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
  return useQuery({
    queryKey: HISTORY,
    queryFn: () => unwrapResult(commands.videoHistory()),
  });
};

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
    onSettled: () =>
      Promise.all([
        queryClient.invalidateQueries({ queryKey: CONTINUE }),
        queryClient.invalidateQueries({ queryKey: HISTORY }),
      ]),
  });
};
