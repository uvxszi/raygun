import type { MusicAlbum, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { LoaderCircle, UserRound } from 'lucide-react';
import type { FC, MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { menuHandler, useAlbumMenuItems, useArtistMenuItems } from '../music/cardMenus';
import { MenuButton } from '../shell/MenuButton';
import { OnlineTrackRow } from './OnlineTrackRow';
import { playResults } from './useOnlineSearch';
import type { SearchProgressCount } from './useOnlineSearch';
import { rankSongs, useAlbumFinds, useArtistFinds } from './useMusicFinds';
import { SourceIssue } from '../shell/SourceIssue';
import { albumCover, artistCover } from '../library/coverTargets';
import { SourceTag } from '../shell/SourceTag';

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
            sourceAdult={entry.sourceAdult}
            sourceListed={entry.fromListings}
            onPlay={(tapped) => void playResults(tracks, tapped)}
          />
        </li>
      ))}
    </ul>
  );
};

/** "3 of 5 sources answered" while a search still waits for some, else a spinner. */
export const Asking: FC<{ progress?: SearchProgressCount }> = ({ progress }) => {
  const { t } = useTranslation();
  return (
    <p role="status" className="text-muted flex items-center gap-2 text-sm">
      <LoaderCircle aria-hidden size={14} className="animate-spin" />
      {progress
        ? t('search.sourcesAnswered', { done: progress.answered, total: progress.total })
        : t('search.searching')}
    </p>
  );
};

/** Artists named like the query, from sources that search artists (M46.9). */
export const ArtistFinds: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const artists = useArtistFinds(query);
  const menu = useArtistMenuItems();
  const list = artists.list;
  if (artists.error) {
    return <SourceIssue>{artists.error.message}</SourceIssue>;
  }
  if (list.length === 0) {
    return artists.isFetching ? (
      <Asking progress={artists.progress} />
    ) : (
      <p className="text-muted text-sm">{t('search.noArtists', { query })}</p>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <ul data-testid="artist-finds" className="flex flex-col">
        {list.map((artist) => (
          <li
            key={`${artist.provider.id}:${artist.id}`}
            className="flex items-center"
            onContextMenu={menuHandler(() =>
              menu({ name: artist.name, imageUrl: artist.imageUrl }),
            )}
          >
            <Link
              to="/artist"
              search={{ name: artist.name }}
              className="hover:bg-surface flex min-w-0 flex-1 items-center gap-3 rounded-xl px-1 py-2"
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
                <span className="text-muted flex min-w-0 items-center gap-1.5 text-xs">
                  {t('artist.kind')}
                  <SourceTag name={artist.provider.name} />
                </span>
              </span>
            </Link>
            <MenuButton
              label={t('menu.more', { name: artist.name })}
              items={() => menu({ name: artist.name, imageUrl: artist.imageUrl })}
            />
          </li>
        ))}
      </ul>
      {artists.progress && <Asking progress={artists.progress} />}
    </div>
  );
};

/** Albums for the query, from sources that search albums (M46.9). */
export const AlbumFinds: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const albums = useAlbumFinds(query);
  const menu = useAlbumMenuItems();
  const list = albums.list;
  if (albums.error) {
    return <SourceIssue>{albums.error.message}</SourceIssue>;
  }
  if (list.length === 0) {
    return albums.isFetching ? (
      <Asking progress={albums.progress} />
    ) : (
      <p className="text-muted text-sm">{t('search.noAlbums', { query })}</p>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <ul
        data-testid="album-finds"
        className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3"
      >
        {list.map((album) => (
          <li key={`${album.provider.id}:${album.id}`}>
            <AlbumTile album={album} onMenu={menuHandler(() => menu(album))} />
          </li>
        ))}
      </ul>
      {albums.progress && <Asking progress={albums.progress} />}
    </div>
  );
};

/**
 * An album a source knows, opening its page (M46.9); `detail` replaces the line under it. Its
 * menu (`onMenu`, made once for the whole list) has the album's basic items.
 */
export const AlbumTile: FC<{
  album: MusicAlbum;
  detail?: string;
  onMenu?: (event: MouseEvent) => void;
}> = ({ album, detail, onMenu }) => (
  <Link
    to="/music/album"
    search={{ provider: album.provider.id, id: album.id }}
    onContextMenu={onMenu}
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
      {detail ?? [album.year, album.artist].filter(Boolean).join(' · ')}
    </span>
    {!detail && (
      <span className="flex min-w-0">
        <SourceTag name={album.provider.name} />
      </span>
    )}
  </Link>
);
