import { commands } from '@raygun/bindings';
import type { VideoLinkMatch } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

/** Under Watch's keys: the player's progress and Rust's watch events refresh it. */
const LINK = ['watch', 'link'];

/** Where a link played on its own opens (`/watch/link`). */
export const videoLinkPage = (key: string) => ({ to: '/watch/link', search: { key } });

/** A link played on its own: what it is, its files, its progress and its match. */
export const useVideoLinkPage = (key: string) =>
  useQuery({
    queryKey: [...LINK, key],
    queryFn: () => unwrapResult(commands.videoLinkPage(key)),
    enabled: key !== '',
    retry: false,
  });

/** Asks peers what a torrent link holds; the page shows the files once they answer. */
export const useVideoLinkFiles = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (key: string) => unwrapResult(commands.videoLinkFiles(key)),
    onSuccess: (page) => client.setQueryData([...LINK, page.key], page),
  });
};

/** What changes a link's entry: its page and Continue watching follow. */
const useLinkChange = <Variables>(run: (variables: Variables) => Promise<unknown>) => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: run,
    onSettled: () => client.invalidateQueries({ queryKey: ['watch'] }),
  });
};

export const useRenameVideoLink = () =>
  useLinkChange(({ key, title }: { key: string; title: string }) =>
    unwrapResult(commands.videoLinkRename(key, title)),
  );

/** Matches a link with a movie or series, or forgets the match (`null`). */
export const useMatchVideoLink = () =>
  useLinkChange(({ key, matched }: { key: string; matched: VideoLinkMatch | null }) =>
    unwrapResult(commands.videoLinkMatch(key, matched)),
  );

/** The movies and series a link's title may be, once asked for (`enabled`). */
export const useVideoLinkSuggestions = (key: string, enabled: boolean) =>
  useQuery({
    queryKey: [...LINK, 'suggestions', key],
    queryFn: () => unwrapResult(commands.videoLinkSuggestions(key)),
    enabled: enabled && key !== '',
    staleTime: 10 * 60 * 1000,
    retry: false,
  });

/** Plays a link: a torrent's file picked, from its start or where it was left. */
export const usePlayVideoLink = () =>
  useMutation({
    mutationFn: ({
      key,
      file = null,
      fromStart = false,
    }: {
      key: string;
      file?: number | null;
      fromStart?: boolean;
    }) => unwrapResult(commands.videoLinkPlay(key, file, fromStart)),
  });

/** Downloads a link (a torrent's file picked, else the one it opens). */
export const useDownloadVideoLink = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ key, file = null }: { key: string; file?: number | null }) =>
      unwrapResult(commands.videoLinkDownload(key, file)),
    onSettled: () => client.invalidateQueries({ queryKey: ['video', 'downloads'] }),
  });
};
