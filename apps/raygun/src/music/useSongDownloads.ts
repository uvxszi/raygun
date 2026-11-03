import { commands, events } from '@raygun/bindings';
import type { AudioFileFormat, SavedTrack, SongDestination, TrackInput } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';

const SONGS = ['songs', 'downloads'];
const KEYS = ['songs', 'downloaded'];

/** Keeps the song downloads and the downloaded marks current from Rust's events (M46.10). */
const useSongDownloadEvents = (listen = true) => {
  const client = useQueryClient();
  useEffect(() => {
    if (!listen) return;
    const unlisten = events.songDownloadsEvent.listen(() => {
      void client.invalidateQueries({ queryKey: SONGS });
      void client.invalidateQueries({ queryKey: KEYS });
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, [client, listen]);
};

/** Every song download, newest first. */
export const useSongDownloads = () => {
  useSongDownloadEvents();
  return useQuery({
    queryKey: SONGS,
    queryFn: () => unwrapResult(commands.songDownloads()),
  });
};

/**
 * Keys of downloaded songs, to mark them in song lists. Rows' menus pass `listen` false (M46.36):
 * one shared answer, without an event listener per row.
 */
export const useDownloadedKeys = (listen = true) => {
  useSongDownloadEvents(listen);
  const keys = useQuery({
    queryKey: KEYS,
    queryFn: () => commands.songDownloadedKeys(),
    staleTime: 60_000,
  });
  return new Set(keys.data ?? []);
};

/** One song's download, if it has one (M46.18: the download button beside +). */
export const useSongDownloadFor = (track: TrackInput) => {
  useSongDownloadEvents();
  return useQuery({
    queryKey: [...KEYS, 'for', track.title, track.artist ?? ''],
    queryFn: () => unwrapResult(commands.songDownloadFor(track)),
  });
};

/** Songs downloaded into the app, as a collection (M46.18: Library → Audio → Downloaded). */
export const useDownloadedSongs = () => {
  useSongDownloadEvents();
  return useQuery<SavedTrack[]>({
    queryKey: [...KEYS, 'songs'],
    queryFn: () => unwrapResult(commands.downloadedSongs()),
  });
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
