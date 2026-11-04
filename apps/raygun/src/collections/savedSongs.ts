import { commands } from '@raygun/bindings';
import type { SavedSong, TrackSaves } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useCallback } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { keysOf, sameSong, titleOf } from '../music/trackKey';
import { COLLECTIONS } from './useCollections';

/** Where a song is saved, and whether anywhere. */
export type SongSaves = TrackSaves & { saved: boolean };

export type SongNames = { title: string; artist: string | null | undefined };

type Index = { byTitle: Map<string, SavedSong[]>; byKey: Map<string, SavedSong> };

const indexes = new WeakMap<SavedSong[], Index>();

/** Saved songs by their keys' titles, built once per answer from Rust (rows share it). */
const indexOf = (songs: SavedSong[]): Index => {
  const known = indexes.get(songs);
  if (known) return known;
  const index: Index = { byTitle: new Map(), byKey: new Map() };
  for (const song of songs) {
    index.byKey.set(song.trackKey, song);
    for (const key of song.songKeys) {
      const title = titleOf(key);
      const bucket = index.byTitle.get(title) ?? [];
      if (!bucket.includes(song)) bucket.push(song);
      index.byTitle.set(title, bucket);
    }
  }
  indexes.set(songs, index);
  return index;
};

/**
 * Where the song with these names is saved, whatever names its copies were saved with (M46.48):
 * Rust lists every key each saved song answers to, and `sameSong` finds the shown names among
 * them. `knownKey` is the key a saved row already has.
 */
export const savesIn = (songs: SavedSong[], song: SongNames, knownKey?: string): SongSaves => {
  const index = indexOf(songs);
  const found = new Set<SavedSong>();
  const known = knownKey ? index.byKey.get(knownKey) : undefined;
  if (known) found.add(known);
  for (const key of keysOf(song.title, song.artist)) {
    for (const saved of index.byTitle.get(titleOf(key)) ?? []) {
      if (saved.songKeys.some((other) => sameSong(other, key))) found.add(saved);
    }
  }
  let favorite = false;
  const playlistIds: string[] = [];
  for (const saved of found) {
    favorite ||= saved.favorite;
    for (const id of saved.playlistIds) if (!playlistIds.includes(id)) playlistIds.push(id);
  }
  return { favorite, playlistIds, saved: favorite || playlistIds.length > 0 };
};

const SAVED_SONGS = [COLLECTIONS, 'savedSongs'] as const;

const options = {
  queryKey: SAVED_SONGS,
  queryFn: () => unwrapResult(commands.collectionsSavedIndex()),
  staleTime: 60_000,
};

/** Every saved song: one list for every mark and Save button, refreshed with the collections. */
export const useSavedSongs = () => useQuery(options);

/**
 * Where one song is saved; `undefined` until the list loads. Each row selects only its own answer,
 * so a save re-renders only the rows it changed.
 */
export const useSongSaves = (song: SongNames | null, knownKey?: string): SongSaves | undefined => {
  const title = song?.title ?? '';
  const artist = song?.artist ?? null;
  const select = useCallback(
    (songs: SavedSong[]) => savesIn(songs, { title, artist }, knownKey),
    [title, artist, knownKey],
  );
  return useQuery({ ...options, select, enabled: song !== null }).data;
};
