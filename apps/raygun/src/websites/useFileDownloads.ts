import { commands, events } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const FILES = ['websiteDownloads'] as const;

/** Files saved from sites in the built-in browser; follows their events while shown. */
export const useFileDownloads = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.websiteDownloadsEvent.listen(() => {
      void client.invalidateQueries({ queryKey: FILES });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  return useQuery({ queryKey: FILES, queryFn: () => commands.websiteDownloads() });
};

export const useFileDownloadActions = () => {
  const client = useQueryClient();
  const refresh = () => void client.invalidateQueries({ queryKey: FILES });
  return {
    cancel: useMutation({
      mutationFn: (id: string) => commands.websiteDownloadCancel(id),
      onSettled: refresh,
    }),
    retry: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.websiteDownloadRetry(id)),
      onSettled: refresh,
    }),
    remove: useMutation({
      mutationFn: ({ id, deleteFile }: { id: string; deleteFile: boolean }) =>
        unwrapResult(commands.websiteDownloadRemove(id, deleteFile)),
      onSettled: refresh,
    }),
    open: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.websiteDownloadOpen(id)),
    }),
    reveal: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.websiteDownloadReveal(id)),
    }),
  };
};
