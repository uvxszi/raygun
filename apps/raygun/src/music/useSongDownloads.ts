import { commands, events } from '@raygun/bindings';
import type { AudioFileFormat, SongDestination, TrackInput } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const SONGS = ['songs', 'downloads'];
const KEYS = ['songs', 'downloaded'];

/** Keeps the song downloads and the downloaded marks current from Rust's events (M46.10). */
const useSongDownloadEvents = () => {
  const client = useQueryClient();
  useEffect(() => {
    const unlisten = events.songDownloadsEvent.listen(() => {
      void client.invalidateQueries({ queryKey: SONGS });
      void client.invalidateQueries({ queryKey: KEYS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client]);
};

/** Every song download, newest first. */
export const useSongDownloads = () => {
  useSongDownloadEvents();
  return useQuery({
    queryKey: SONGS,
    queryFn: () => unwrapResult(commands.songDownloads()),
  });
};

/** Keys of downloaded songs, to mark them in song lists. */
export const useDownloadedKeys = () => {
  useSongDownloadEvents();
  const keys = useQuery({
    queryKey: KEYS,
    queryFn: () => commands.songDownloadedKeys(),
    staleTime: 60_000,
  });
  return new Set(keys.data ?? []);
};

/** Queues songs to save into the app. */
export const downloadSongs = (tracks: TrackInput[]) => unwrapResult(commands.songsDownload(tracks));

export const useSongDownloadActions = () => {
  const client = useQueryClient();
  const refresh = () => void client.invalidateQueries({ queryKey: SONGS });
  return {
    remove: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.songDownloadRemove(id)),
      onSettled: refresh,
    }),
    retry: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.songDownloadRetry(id)),
      onSettled: refresh,
    }),
    record: useMutation({
      mutationFn: (id: string) => unwrapResult(commands.songDownloadRecord(id)),
      onSettled: refresh,
    }),
    save: useMutation({
      mutationFn: ({
        ids,
        format,
        destination,
      }: {
        ids: string[];
        format: AudioFileFormat;
        destination: SongDestination;
      }) => unwrapResult(commands.songsSave(ids, format, destination)),
    }),
  };
};
