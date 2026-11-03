import type { MusicAlbum, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { LoaderCircle, UserRound } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { OnlineTrackRow } from './OnlineTrackRow';
import { playResults } from './useOnlineSearch';
import { rankSongs, useAlbumFinds, useArtistFinds } from './useMusicFinds';
import { SourceIssue } from '../shell/SourceIssue';
import { albumCover, artistCover } from '../library/coverTargets';

/**
 * Every source's songs in one list (M46.9): the songs several sources agree on first, then the
 * best answers to the query, each once, with the source it came from.
 */
export const BestSongs: FC<{
  query: string;
  top: OnlineTrack[];
  results: ProviderSearchResult[];
}> = ({ query, top, results }) => {
  const entries = rankSongs(query, top, results);
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
  const artists = useArtistFinds(query);
  const list = artists.list;
  if (artists.isPending) {
    return <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />;
  }
  if (artists.error) {
    return <SourceIssue>{artists.error.message}</SourceIssue>;
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
              <Artwork
                url={artist.imageUrl}
                fallback={artistCover(artist.name)}
                className="size-12 rounded-full"
                iconSize={20}
              />
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
  const albums = useAlbumFinds(query);
  const list = albums.list;
  if (albums.isPending) {
    return <LoaderCircle aria-label={t('search.searching')} className="text-muted animate-spin" />;
  }
  if (albums.error) {
    return <SourceIssue>{albums.error.message}</SourceIssue>;
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
          <AlbumTile album={album} />
        </li>
      ))}
    </ul>
  );
};

/** An album a source knows, opening its page (M46.9); `detail` replaces the line under it. */
export const AlbumTile: FC<{ album: MusicAlbum; detail?: string }> = ({ album, detail }) => (
  <Link
    to="/music/album"
    search={{ provider: album.provider.id, id: album.id }}
    className="hover:bg-surface flex flex-col gap-1.5 rounded-xl p-2"
  >
    <Artwork
      url={album.artworkUrl}
      fallback={albumCover(album)}
      className="w-full rounded-lg shadow"
      iconSize={36}
    />
    <span className="truncate text-sm font-semibold">{album.title}</span>
    <span className="text-muted truncate text-xs">
      {detail ?? [album.year, album.artist, album.provider.name].filter(Boolean).join(' · ')}
    </span>
  </Link>
);
