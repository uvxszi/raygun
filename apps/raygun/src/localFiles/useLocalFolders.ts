import { commands, events } from '@raygun/bindings';
import type { LocalFolder } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import type { QueryClient } from '@tanstack/react-query';
import { useEffect, useRef } from 'react';

import { unwrapResult } from '../lib/commandResult';

const LOCAL_FOLDERS = ['localFolders'] as const;

/**
 * The libraries a scan adds to: comics, books and videos. Songs refresh with the music library's
 * own scan (`useLibraryScanSync`).
 */
const refreshLibraries = (client: QueryClient) =>
  Promise.all(
    ['manga', 'novel', 'video'].map((medium) =>
      client.invalidateQueries({ queryKey: [medium, 'library'] }),
    ),
  );

/** The folders in Local Files (M46.27), kept current as scans move on. */
export const useLocalFolders = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.localFilesEvent.listen(() => {
      void client.invalidateQueries({ queryKey: LOCAL_FOLDERS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
  const folders = useQuery({
    queryKey: LOCAL_FOLDERS,
    queryFn: () => unwrapResult(commands.localFolders()),
  });
  // What a scan found shows in the libraries once it ends, not at every step of it.
  const scanning = (folders.data ?? [])
    .filter((folder) => folder.scanning)
    .map((folder) => folder.id)
    .join(' ');
  const before = useRef(scanning);
  useEffect(() => {
    const now = new Set(scanning.split(' '));
    if (before.current.split(' ').some((id) => id && !now.has(id))) {
      void refreshLibraries(client);
    }
    before.current = scanning;
  }, [scanning, client]);
  return folders;
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
      onSuccess: (folders) => {
        set(folders);
        // Its videos left the library.
        void refreshLibraries(client);
      },
    }),
    /** One folder again, or every one; files that couldn't be added are tried again. */
    rescan: useMutation({
      mutationFn: (id: string | null) => unwrapResult(commands.localFoldersRescan(id)),
      onSettled: () => void client.invalidateQueries({ queryKey: LOCAL_FOLDERS }),
    }),
    /** Asks Android for the media access songs and videos need. */
    allowAccess: useMutation({
      mutationFn: () => unwrapResult(commands.localFilesAllowAccess()),
      onSettled: () => void client.invalidateQueries({ queryKey: LOCAL_FOLDERS }),
    }),
  };
};
