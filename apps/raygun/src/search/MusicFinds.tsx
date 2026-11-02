import { commands } from '@raygun/bindings';
import type { OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link } from '@tanstack/react-router';
import { LoaderCircle, UserRound } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { OnlineTrackRow } from './OnlineTrackRow';
import { titleScore } from './rankResults';
import { playResults } from './useOnlineSearch';

const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/**
 * Every source's songs in one list (M46.9): the songs several sources agree on first, then the
 * best answers to the query, each once, with the source it came from.
 */
export const BestSongs: FC<{
  query: string;
  top: OnlineTrack[];
  results: ProviderSearchResult[];
}> = ({ query, top, results }) => {
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
  const tracks = entries.map((entry) => entry.track);
  return (
    <ul data-testid="best-songs" className="flex flex-col">
      {entries.map((entry, index) => (
        <li key={`${entry.track.title}-${index}`}>
          <OnlineTrackRow
            track={entry.track}
            source={entry.source}
            onPlay={(tapped) => void playResults(tracks, tapped)}
          />
        </li>
      ))}
    </ul>
  );
};

/** Artists named like the query, from sources that search artists (M46.9). */
export const ArtistFinds: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
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
  if (artists.isPending) {
    return <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />;
  }
  if (artists.error) {
    return (
      <p role="alert" className="text-primary text-sm wrap-anywhere">
        {artists.error.message}
      </p>
    );
  }
  if (list.length === 0) {
    return <p className="text-muted text-sm">{t('search.noArtists', { query })}</p>;
  }
  return (
    <ul data-testid="artist-finds" className="flex flex-col">
      {list.map((artist) => (
        <li key={`${artist.provider.id}:${artist.id}`}>
          <Link
            to="/artist"
            search={{ name: artist.name }}
            className="hover:bg-surface flex items-center gap-3 rounded-xl px-1 py-2"
          >
            {artist.imageUrl ? (
              <Artwork url={artist.imageUrl} className="size-12 rounded-full" iconSize={20} />
            ) : (
              <span className="bg-surface text-muted grid size-12 place-items-center rounded-full">
                <UserRound aria-hidden size={22} />
              </span>
            )}
            <span className="flex min-w-0 flex-col">
              <span className="truncate font-medium">{artist.name}</span>
              <span className="text-muted truncate text-xs">
                {t('artist.kind')} · {artist.provider.name}
              </span>
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
};

/** Albums for the query, from sources that search albums (M46.9). */
export const AlbumFinds: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
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
  if (albums.isPending) {
    return <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />;
  }
  if (albums.error) {
    return (
      <p role="alert" className="text-primary text-sm wrap-anywhere">
        {albums.error.message}
      </p>
    );
  }
  if (list.length === 0) {
    return <p className="text-muted text-sm">{t('search.noAlbums', { query })}</p>;
  }
  return (
    <ul
      data-testid="album-finds"
      className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3"
    >
      {list.map((album) => (
        <li key={`${album.provider.id}:${album.id}`}>
          <Link
            to="/music/album"
            search={{ provider: album.provider.id, id: album.id }}
            className="hover:bg-surface flex flex-col gap-1.5 rounded-xl p-2"
          >
            <Artwork url={album.artworkUrl} className="w-full rounded-lg shadow" iconSize={36} />
            <span className="truncate text-sm font-semibold">{album.title}</span>
            <span className="text-muted truncate text-xs">
              {[album.year, album.artist, album.provider.name].filter(Boolean).join(' · ')}
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
};
