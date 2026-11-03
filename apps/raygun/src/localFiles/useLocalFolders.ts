import { commands, events } from '@raygun/bindings';
import type { LocalFolder } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const LOCAL_FOLDERS = ['localFolders'] as const;

/** The folders in Local Files (M46.27), kept current as scans move on. */
export const useLocalFolders = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.localFilesEvent.listen(() => {
      void client.invalidateQueries({ queryKey: LOCAL_FOLDERS });
      // What a scan found shows in the libraries.
      void client.invalidateQueries({ queryKey: ['library'] });
      void client.invalidateQueries({ queryKey: ['manga'] });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  return useQuery({
    queryKey: LOCAL_FOLDERS,
    queryFn: () => unwrapResult(commands.localFolders()),
  });
};

export const useLocalFolderActions = () => {
  const client = useQueryClient();
  const set = (folders: LocalFolder[] | null) => {
    if (folders) client.setQueryData(LOCAL_FOLDERS, folders);
  };
  return {
    add: useMutation({
      mutationFn: () => unwrapResult(commands.localFolderAdd()),
      onSuccess: set,
    }),
    remove: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.localFolderRemove(id)),
      onSuccess: set,
    }),
    rescan: useMutation({
      mutationFn: () => unwrapResult(commands.localFoldersRescan()),
    }),
  };
};
