import type { MusicAlbum, MusicArtist, OnlineTrack, ProviderSearchResult } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { LoaderCircle, UserRound } from 'lucide-react';
import { useCallback, useMemo } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { useAlbumMenuItems, useArtistMenuItems } from '../music/cardMenus';
import { songReveal, useRevealIndex } from '../music/reveal';
import { CardMenu } from '../shell/CardMenu';
import type { MenuItem } from '../shell/contextMenuStore';
import { CARD_MENU_BESIDE } from '../manga/CardPlayButton';
import { MenuButton } from '../shell/MenuButton';
import { useDrawnCount } from '../shell/useDrawnCount';
import { VirtualRows } from '../shell/VirtualRows';
import { OnlineTrackRow } from './OnlineTrackRow';
import { playResults } from './useOnlineSearch';
import type { SearchProgressCount } from './useOnlineSearch';
import { rankSongs, useAlbumFinds, useArtistFinds } from './useMusicFinds';
import type { RankedSong } from './useMusicFinds';
import { NeedsSetupNote } from '../shell/NeedsSetupNote';
import { SourceIssue } from '../shell/SourceIssue';
import { albumCover, artistCover } from '../library/coverTargets';
import { SourceTag } from '../shell/SourceTag';

/** Album tiles drawn at first, then as many more at a time (`useDrawnCount`). */
const FIRST_TILES = 24;

/** The sources artist and album searches ask; those waiting for a key are counted in one line. */
const METADATA_KINDS = ['music.metadata'];

const songName = (entry: RankedSong) => songReveal(entry.track.title, entry.track.artist);
const songRowKey = (entry: RankedSong) => `${entry.track.artist ?? ''}\n${entry.track.title}`;

/**
 * Every source's songs in one list (M46.9): the songs several sources agree on first, then the
 * best answers to the query, each once, with the source it came from. A long list draws only the
 * rows near the screen (`VirtualRows`), the playing song's ("Playing from") brought onto it.
 */
export const BestSongs: FC<{
  query: string;
  top: OnlineTrack[];
  results: ProviderSearchResult[];
}> = ({ query, top, results }) => {
  const entries = useMemo(() => rankSongs(query, top, results), [query, top, results]);
  const tracks = useMemo(() => entries.map((entry) => entry.track), [entries]);
  const play = useCallback((tapped: OnlineTrack) => void playResults(tracks, tapped), [tracks]);
  const revealAt = useRevealIndex(entries, songName);
  return (
    <VirtualRows
      data-testid="best-songs"
      className="flex flex-col"
      items={entries}
      keyOf={songRowKey}
      // A row naming its source: two lines and the source's tag.
      rowRem={4.5}
      showIndex={revealAt}
      renderRow={(entry, _index, row) => (
        <li {...row}>
          <OnlineTrackRow
            track={entry.track}
            source={entry.source}
            sourceAdult={entry.sourceAdult}
            sourceListed={entry.fromListings}
            onPlay={play}
          />
        </li>
      )}
    />
  );
};

/** "3 of 5 sources answered" while a search still waits for some, else a spinner. */
export const Asking: FC<{ progress?: SearchProgressCount }> = ({ progress }) => {
  const { t } = useTranslation();
  return (
    <p role="status" className="text-muted flex items-center gap-2 text-sm">
      <LoaderCircle aria-hidden size={14} className="animate-spin" />
      {progress
        ? t('search.sourcesAnswered', { done: progress.answered, count: progress.total })
        : t('search.searching')}
    </p>
  );
};

const artistKey = (artist: MusicArtist) => `${artist.provider.id}:${artist.id}`;

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
      <>
        <p className="text-muted text-sm">{t('search.noArtists', { query })}</p>
        <NeedsSetupNote kinds={METADATA_KINDS} />
      </>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      {/* A long list draws only the rows near the screen. */}
      <VirtualRows
        data-testid="artist-finds"
        className="flex flex-col"
        items={list}
        keyOf={artistKey}
        rowRem={4}
        renderRow={(artist, _index, row) => (
          <li {...row} className="flex items-center">
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
        )}
      />
      {artists.progress && <Asking progress={artists.progress} />}
      <NeedsSetupNote kinds={METADATA_KINDS} />
    </div>
  );
};

/** Albums for the query, from sources that search albums (M46.9). */
export const AlbumFinds: FC<{ query: string }> = ({ query }) => {
  const { t } = useTranslation();
  const albums = useAlbumFinds(query);
  const menu = useAlbumMenuItems();
  const list = albums.list;
  // Hundreds of covers: the first at once, the rest as each part is on the screen.
  const drawn = useDrawnCount(list.length, FIRST_TILES, FIRST_TILES, query.trim());
  if (albums.error) {
    return <SourceIssue>{albums.error.message}</SourceIssue>;
  }
  if (list.length === 0) {
    return albums.isFetching ? (
      <Asking progress={albums.progress} />
    ) : (
      <>
        <p className="text-muted text-sm">{t('search.noAlbums', { query })}</p>
        <NeedsSetupNote kinds={METADATA_KINDS} />
      </>
    );
  }
  return (
    <div className="flex flex-col gap-2">
      <ul
        data-testid="album-finds"
        className="grid grid-cols-[repeat(auto-fill,minmax(9rem,1fr))] gap-3"
      >
        {list.slice(0, drawn).map((album) => (
          <li key={`${album.provider.id}:${album.id}`}>
            <AlbumTile album={album} menu={() => menu(album)} />
          </li>
        ))}
      </ul>
      {albums.progress && <Asking progress={albums.progress} />}
      <NeedsSetupNote kinds={METADATA_KINDS} />
    </div>
  );
};

/**
 * An album a source knows, opening its page (M46.9); `detail` replaces the line under it. Its
 * ⋮ (`menu`, made once for the whole list) has the album's basic items.
 */
export const AlbumTile: FC<{
  album: MusicAlbum;
  detail?: string;
  menu?: () => MenuItem[];
}> = ({ album, detail, menu }) => {
  const { t } = useTranslation();
  return (
    <Link
      to="/music/album"
      search={{ provider: album.provider.id, id: album.id }}
      className="hover:bg-surface relative flex flex-col gap-1.5 rounded-xl p-2"
    >
      {menu && (
        <CardMenu
          label={t('menu.options', { name: album.title })}
          items={menu}
          className={CARD_MENU_BESIDE}
        />
      )}
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
};
