import { commands } from '@raygun/bindings';
import type { VideoRequest } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

const WATCH = ['watch'];
const CONTINUE = [...WATCH, 'continue'];

/**
 * What to continue watching, one entry per title; kept current as the player reports progress
 * (`shell/useLiveInvalidation`).
 */
export const useContinueWatching = () =>
  useQuery({
    queryKey: CONTINUE,
    queryFn: () => unwrapResult(commands.videoContinueWatching()),
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
