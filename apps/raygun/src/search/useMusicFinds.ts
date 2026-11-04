import { commands } from '@raygun/bindings';
import type { OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';
import { titleScore } from './rankResults';
import { SEARCH_KEYS, streamed, useStreamedAnswers } from './searchStream';
import type { StreamedAnswers } from './searchStream';
import { RESULTS_STALE_MS } from './useOnlineSearch';
import type { SearchProgressCount } from './useOnlineSearch';

const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

export type RankedSong = { track: OnlineTrack; source: string | undefined; score: number };

/** Only each source's first answers count as agreeing; deep results rarely are the song asked. */
const AGREEING_RANKS = 10;
/** What each further source listing a song adds to its score. */
const AGREEMENT_BONUS = 15;

const songKey = (track: OnlineTrack) => `${simple(track.artist)}|${simple(track.title)}`;

/**
 * Every source's songs ranked for the query, each once (M46.9; Relevant reuses it, M46.25): how
 * well the title (and artist) answer the query, with songs several sources list near the top
 * ranking higher. `top` (Rust's merge of those songs) gives the merged copy of a song, with what
 * one source lacked filled in. Ranking the same way with or without `top` keeps the list still
 * while sources are still answering.
 */
export const rankSongs = (
  query: string,
  top: OnlineTrack[],
  results: ProviderSearchResult[],
): RankedSong[] => {
  const merged = new Map(top.map((track) => [songKey(track), track]));
  const agreeing = new Map<string, number>();
  for (const result of results) {
    const listed = new Set(result.tracks.slice(0, AGREEING_RANKS).map(songKey));
    for (const key of listed) agreeing.set(key, (agreeing.get(key) ?? 0) + 1);
  }
  const words = new Set(simple(query).split(' '));
  const seen = new Set<string>();
  const entries: (RankedSong & { at: number; order: number })[] = [];
  results.forEach((result, order) =>
    result.tracks.forEach((track, index) => {
      const key = songKey(track);
      if (seen.has(key)) return;
      seen.add(key);
      // A query naming the artist ("daft punk get lucky") counts the artist's words too.
      const artistBonus = simple(track.artist)
        .split(' ')
        .some((word) => word && words.has(word))
        ? 10
        : 0;
      const agreement = Math.max(0, (agreeing.get(key) ?? 1) - 1) * AGREEMENT_BONUS;
      const score = Math.max(
        titleScore(track.title, query),
        titleScore(`${track.artist ?? ''} ${track.title}`, query),
      );
      entries.push({
        track: merged.get(key) ?? track,
        source: result.provider.name,
        score: score + artistBonus + agreement,
        at: index,
        order,
      });
    }),
  );
  // Songs only the merge knows (two sources spelled them alike, none listed them first) follow.
  top.forEach((track, index) => {
    const key = songKey(track);
    if (seen.has(key)) return;
    seen.add(key);
    entries.push({ track, source: undefined, score: AGREEMENT_BONUS, at: index, order: 0 });
  });
  entries.sort((a, b) => b.score - a.score || a.at - b.at || a.order - b.order);
  return entries.map(({ track, source, score }) => ({ track, source, score }));
};

/** What a streamed search shows: its full answer, else what its sources sent so far. */
const shownWhileAsking = <T>(
  data: T[] | undefined,
  parts: StreamedAnswers | undefined,
  pick: (parts: StreamedAnswers) => T[],
  fetching: boolean,
) => {
  const partial = !data && parts ? pick(parts) : undefined;
  const progress: SearchProgressCount | undefined =
    fetching && parts && parts.total > 0
      ? { answered: parts.answered.length, total: parts.total }
      : undefined;
  return { data: data ?? (partial && partial.length > 0 ? partial : undefined), progress };
};

/** Artists named like the query from every source that searches artists, each name once. */
export const useArtistFinds = (query: string) => {
  const wanted = query.trim();
  const key = SEARCH_KEYS.artists(wanted);
  const artists = useQuery({
    queryKey: key,
    queryFn: () => streamed(key, () => unwrapResult(commands.musicSearchArtists(wanted))),
    enabled: wanted !== '',
    staleTime: RESULTS_STALE_MS,
  });
  const parts = useStreamedAnswers(key);
  const { data, progress } = shownWhileAsking(
    artists.data,
    parts,
    (answers) => answers.artists,
    artists.isFetching,
  );
  // One row per name; the first source's picture.
  const seen = new Set<string>();
  const list = (data ?? []).filter((artist) => {
    const name = simple(artist.name);
    if (seen.has(name)) return false;
    seen.add(name);
    return true;
  });
  return { ...artists, data, list, progress };
};

/** Albums for the query from every source that searches albums, best match first. */
export const useAlbumFinds = (query: string) => {
  const wanted = query.trim();
  const key = SEARCH_KEYS.albums(wanted);
  const albums = useQuery({
    queryKey: key,
    queryFn: () => streamed(key, () => unwrapResult(commands.musicSearchAlbums(wanted))),
    enabled: wanted !== '',
    staleTime: RESULTS_STALE_MS,
  });
  const parts = useStreamedAnswers(key);
  const { data, progress } = shownWhileAsking(
    albums.data,
    parts,
    (answers) => answers.albums,
    albums.isFetching,
  );
  const score = (album: { title: string; artist: string | null }) =>
    Math.max(
      titleScore(album.title, query),
      titleScore(`${album.artist ?? ''} ${album.title}`, query),
    );
  const list = [...(data ?? [])].sort((a, b) => score(b) - score(a));
  return { ...albums, data, list, progress };
};

/**
 * The sources' playlists for a search (M46.39), best match first; `featured` keeps the
 * services' own. Without it, the services' own playlists still come first among equals.
 */
export const usePlaylistFinds = (query: string, featured: boolean) => {
  const wanted = query.trim();
  const key = SEARCH_KEYS.playlists(wanted, featured);
  const playlists = useQuery({
    queryKey: key,
    queryFn: () =>
      streamed(key, () => unwrapResult(commands.musicSearchPlaylists(wanted, featured))),
    enabled: wanted !== '',
    staleTime: RESULTS_STALE_MS,
  });
  const parts = useStreamedAnswers(key);
  const { data, progress } = shownWhileAsking(
    playlists.data,
    parts,
    (answers) => answers.playlists,
    playlists.isFetching,
  );
  const list = [...(data ?? [])].sort(
    (a, b) =>
      titleScore(b.title, query) - titleScore(a.title, query) ||
      Number(b.featured) - Number(a.featured),
  );
  return { ...playlists, data, list, progress };
};
