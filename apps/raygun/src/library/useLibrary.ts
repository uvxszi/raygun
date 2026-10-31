import { commands, events } from '@raygun/bindings';
import type { LibraryScanProgress, PlayRequest } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';
import { create } from 'zustand';

import { unwrapResult } from '../lib/commandResult';

/** Every library query lives under this key, so a finished scan can refresh them all. */
const LIBRARY = 'library';

/** Enough for any realistic local library in one request. */
const ALL_TRACKS_LIMIT = 100_000;

type ScanState = { progress: LibraryScanProgress | null };

export const useScanStore = create<ScanState>(() => ({ progress: null }));

/** Follows scan progress from Rust and refreshes library data when a scan ends. Mount once. */
export const useLibraryScanSync = () => {
  const queryClient = useQueryClient();
  useEffect(() => {
    const unlisten = events.libraryScanEvent.listen((event) => {
      useScanStore.setState({ progress: event.payload });
      if (event.payload.phase === 'done' || event.payload.phase === 'failed') {
        void queryClient.invalidateQueries({ queryKey: [LIBRARY] });
      }
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [queryClient]);
};

export const useLibraryStatus = () =>
  useQuery({
    queryKey: [LIBRARY, 'status'],
    queryFn: () => unwrapResult(commands.libraryStatus()),
  });

export const useAlbums = (artist?: string) =>
  useQuery({
    queryKey: [LIBRARY, 'albums', artist ?? null],
    queryFn: () => unwrapResult(commands.libraryAlbums(artist ?? null)),
  });

export const useArtists = () =>
  useQuery({
    queryKey: [LIBRARY, 'artists'],
    queryFn: () => unwrapResult(commands.libraryArtists()),
  });

export const useAlbumTracks = (artist: string, album: string) =>
  useQuery({
    queryKey: [LIBRARY, 'albumTracks', artist, album],
    queryFn: () => unwrapResult(commands.libraryAlbumTracks(artist, album)),
  });

export const useAllTracks = () =>
  useQuery({
    queryKey: [LIBRARY, 'tracks'],
    queryFn: () => unwrapResult(commands.libraryTracks(0, ALL_TRACKS_LIMIT)),
  });

const useStatusMutation = (run: () => ReturnType<typeof commands.libraryStatus>) => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: () => unwrapResult(run()),
    onSuccess: (status) => queryClient.setQueryData([LIBRARY, 'status'], status),
  });
};

export const useConnectDeviceMusic = () =>
  useStatusMutation(() => commands.libraryConnectDeviceMusic());

export const useAddRoot = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => unwrapResult(commands.libraryAddRoot(path)),
    onSuccess: (status) => queryClient.setQueryData([LIBRARY, 'status'], status),
  });
};

export const useRemoveRoot = () => {
  const queryClient = useQueryClient();
  return useMutation({
    mutationFn: (path: string) => unwrapResult(commands.libraryRemoveRoot(path)),
    onSuccess: (status) => queryClient.setQueryData([LIBRARY, 'status'], status),
  });
};

export const playFromLibrary = (request: PlayRequest) =>
  unwrapResult(commands.libraryPlay(request));
