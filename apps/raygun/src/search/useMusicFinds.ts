import { commands } from '@raygun/bindings';
import type { OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import { titleScore } from './rankResults';

const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

export type RankedSong = { track: OnlineTrack; source: string | undefined; score: number };

/** Every source's songs ranked for the query, each once (M46.9; Relevant reuses it, M46.25). */
export const rankSongs = (
  query: string,
  top: OnlineTrack[],
  results: ProviderSearchResult[],
): RankedSong[] => {
  const seen = new Set<string>();
  const entries: { track: OnlineTrack; source: string | undefined; score: number; at: number }[] =
    [];
  const add = (track: OnlineTrack, source: string | undefined, score: number, at: number) => {
    const key = `${simple(track.artist)}|${simple(track.title)}`;
    if (seen.has(key)) return;
    seen.add(key);
    entries.push({ track, source, score, at });
  };
  top.forEach((track, index) => add(track, undefined, 1000, index));
  const words = new Set(simple(query).split(' '));
  results.forEach((result) =>
    result.tracks.forEach((track, index) => {
      // A query naming the artist ("daft punk get lucky") counts the artist's words too.
      const artistBonus = simple(track.artist)
        .split(' ')
        .some((word) => word && words.has(word))
        ? 10
        : 0;
      const score = Math.max(
        titleScore(track.title, query),
        titleScore(`${track.artist ?? ''} ${track.title}`, query),
      );
      add(track, result.provider.name, score + artistBonus, index);
    }),
  );
  entries.sort((a, b) => b.score - a.score || a.at - b.at);
  return entries;
};

/** Artists named like the query from every source that searches artists, each name once. */
export const useArtistFinds = (query: string) => {
  const artists = useQuery({
    queryKey: ['music', 'searchArtists', query.trim()],
    queryFn: () => unwrapResult(commands.musicSearchArtists(query.trim())),
    enabled: query.trim() !== '',
    staleTime: 5 * 60_000,
  });
  // One row per name; the first source's picture.
  const seen = new Set<string>();
  const list = (artists.data ?? []).filter((artist) => {
    const key = simple(artist.name);
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
  return { ...artists, list };
};

/** Albums for the query from every source that searches albums, best match first. */
export const useAlbumFinds = (query: string) => {
  const albums = useQuery({
    queryKey: ['music', 'searchAlbums', query.trim()],
    queryFn: () => unwrapResult(commands.musicSearchAlbums(query.trim())),
    enabled: query.trim() !== '',
    staleTime: 5 * 60_000,
  });
  const list = [...(albums.data ?? [])].sort(
    (a, b) =>
      Math.max(titleScore(b.title, query), titleScore(`${b.artist ?? ''} ${b.title}`, query)) -
      Math.max(titleScore(a.title, query), titleScore(`${a.artist ?? ''} ${a.title}`, query)),
  );
  return { ...albums, list };
};
