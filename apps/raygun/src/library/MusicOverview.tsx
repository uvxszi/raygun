import { Link } from '@tanstack/react-router';
import { ChevronRight } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { useFavorites, usePlaylists } from '../collections/useCollections';
import type { LengthSummary } from '../music/collectionLength';
import { playlistLengthSummary, useLengthLabel } from '../music/collectionLength';
import { useDownloadedSongs } from '../music/useSongDownloads';
import { useAudiobooks } from '../audiobooks/useAudiobooks';
import { PodcastArt } from '../podcasts/PodcastArt';
import { DownloadedTile, FavoritesTile, LocalFilesTile } from './LocalFilesTile';
import {
  useAlbumMenuItems,
  useArtistMenuItems,
  useBookMenuItems,
  usePodcastMenuItems,
} from '../music/cardMenus';
import { usePlaylistMenuItems } from '../collections/usePlaylistMenu';
import { useSubscriptions } from '../podcasts/usePodcasts';
import { Artwork } from './Artwork';
import { FirstRunCard } from './FirstRunCard';
import { TrackRow } from './TrackRow';
import { playFromLibrary, useAlbums, useArtists, useFirstTracks } from './useLibrary';
import type { LibraryView } from './views';
import { Carousel } from '../shell/Carousel';
import { CardMenu } from '../shell/CardMenu';

/** How many songs the overview lists before "See all". */
const SONGS_SHOWN = 8;
/** Items a row shows; "See all" has the rest (a big library would draw thousands of covers). */
const ROW_ITEMS = 20;

const Section: FC<{ view: LibraryView; count: number; children: ReactNode }> = ({
  view,
  count,
  children,
}) => {
  const { t } = useTranslation();
  const title = t(`library.view.${view}`);
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <Link
        to="/library"
        search={{ view }}
        replace
        aria-label={t('library.overview.seeAll', { name: title })}
        className="flex items-center justify-between gap-2"
      >
        <h2 className="font-semibold">
          {title} <span className="text-muted text-sm font-normal">{count}</span>
        </h2>
        <ChevronRight aria-hidden size={20} className="text-muted" />
      </Link>
      {children}
    </section>
  );
};

const Row: FC<{ children: ReactNode }> = ({ children }) => (
  <Carousel className="-mx-4 gap-3 px-4 pb-1 lg:mx-0 lg:px-0">{children}</Carousel>
);

/**
 * Library → Audio → All: everything you have at a glance (Spotify's unfiltered library), each
 * kind in a row that leads to its own view.
 */
export const MusicOverview: FC<{
  hasFolders: boolean;
  trackCount: number;
  ready: boolean;
}> = ({ hasFolders, trackCount, ready }) => {
  const { t } = useTranslation();
  // Every card's menu has its kind's basic items (play, save, pin…).
  const playlistMenu = usePlaylistMenuItems();
  const podcastMenu = usePodcastMenuItems();
  const bookMenu = useBookMenuItems();
  const albumMenu = useAlbumMenuItems();
  const artistMenu = useArtistMenuItems();
  const playlists = usePlaylists();
  const podcasts = useSubscriptions();
  const audiobooks = useAudiobooks();
  const favorites = useFavorites();
  const downloaded = useDownloadedSongs();
  const hasTracks = hasFolders && trackCount > 0;
  const albums = useAlbums();
  const artists = useArtists();
  const tracks = useFirstTracks(SONGS_SHOWN);
  const playlistCount = playlists.data?.length ?? 0;
  const podcastList = podcasts.data ?? [];
  const bookList = audiobooks.data ?? [];
  // Nothing anywhere: Browse → Search leads, not only the device's folders.
  const nothing =
    ready &&
    !hasTracks &&
    [playlists, podcasts, audiobooks, favorites, downloaded].every(
      (list) => (list.data !== undefined || list.isError) && (list.data?.length ?? 0) === 0,
    );

  return (
    <div data-testid="music-overview" className="flex flex-col gap-6">
      {/* Favorites, Downloaded and Local files are always the first three (M46.29). */}
      <Section view="playlists" count={playlistCount}>
        <Row>
          <FavoritesTile />
          <DownloadedTile />
          <LocalFilesTile />
          {playlists.data?.slice(0, ROW_ITEMS).map((playlist) => (
            <li key={playlist.id} className="relative w-28 shrink-0">
              <CardMenu
                label={t('menu.options', { name: playlist.name })}
                items={() => playlistMenu(playlist)}
              />
              <PlaylistTile
                id={playlist.id}
                name={playlist.name}
                cover={
                  <PlaylistCover
                    artworkUrls={playlist.artworkUrls}
                    coverUrl={playlist.coverUrl}
                    className="aspect-square w-full rounded-lg"
                  />
                }
                count={playlist.itemCount}
                length={playlistLengthSummary(playlist.length ?? undefined, playlist.itemCount)}
              />
            </li>
          ))}
        </Row>
      </Section>
      {podcastList.length > 0 && (
        <Section view="podcasts" count={podcastList.length}>
          <Row>
            {podcastList.slice(0, ROW_ITEMS).map((podcast) => (
              <li key={podcast.id} className="relative w-28 shrink-0">
                <CardMenu
                  label={t('menu.options', { name: podcast.title })}
                  items={() =>
                    podcastMenu({
                      title: podcast.title,
                      author: podcast.author,
                      feedUrl: podcast.feedUrl,
                      artworkUrl: podcast.artworkUrl,
                      id: podcast.id,
                      subscribed: true,
                    })
                  }
                />
                <Link to="/podcast" search={{ id: podcast.id }} className="flex flex-col gap-1.5">
                  <PodcastArt url={podcast.artworkUrl} className="w-full rounded-lg" />
                  <span className="line-clamp-2 text-sm font-medium">{podcast.title}</span>
                </Link>
              </li>
            ))}
          </Row>
        </Section>
      )}
      {bookList.length > 0 && (
        <Section view="audiobooks" count={bookList.length}>
          <Row>
            {bookList.slice(0, ROW_ITEMS).map((book) => (
              <li key={book.id} className="relative w-28 shrink-0">
                <CardMenu
                  label={t('menu.options', { name: book.title })}
                  items={() =>
                    bookMenu({
                      title: book.title,
                      author: book.author,
                      artworkUrl: book.artworkUrl,
                      id: book.id,
                      kept: true,
                    })
                  }
                />
                <Link to="/audiobook" search={{ id: book.id }} className="flex flex-col gap-1.5">
                  <PodcastArt url={book.artworkUrl} className="w-full rounded-lg" />
                  <span className="line-clamp-2 text-sm font-medium">{book.title}</span>
                </Link>
              </li>
            ))}
          </Row>
        </Section>
      )}
      {ready && !hasFolders && <FirstRunCard empty={nothing} />}
      {hasFolders && !hasTracks && ready && <NoTracksYet />}
      {hasTracks && albums.data && albums.data.length > 0 && (
        <Section view="albums" count={albums.data.length}>
          <Row>
            {albums.data.slice(0, ROW_ITEMS).map((album) => (
              <li key={`${album.artist}\u001f${album.title}`} className="relative w-28 shrink-0">
                <CardMenu
                  label={t('menu.options', { name: album.title })}
                  items={() =>
                    albumMenu({
                      title: album.title,
                      artist: album.artist,
                      artworkUrl: album.artworkUrl,
                    })
                  }
                />
                <Link
                  to="/library/album"
                  search={{ artist: album.artist, album: album.title }}
                  className="flex flex-col gap-1.5"
                >
                  <Artwork url={album.artworkUrl} className="w-full rounded-lg" iconSize={32} />
                  <span className="truncate text-sm font-medium">{album.title}</span>
                  <span className="text-muted -mt-1 truncate text-xs">{album.artist}</span>
                </Link>
              </li>
            ))}
          </Row>
        </Section>
      )}
      {hasTracks && artists.data && artists.data.length > 0 && (
        <Section view="artists" count={artists.data.length}>
          <Row>
            {artists.data.slice(0, ROW_ITEMS).map((artist) => (
              <li key={artist.name} className="relative w-24 shrink-0">
                <CardMenu
                  label={t('menu.options', { name: artist.name })}
                  items={() => artistMenu({ name: artist.name, imageUrl: artist.artworkUrl })}
                />
                <Link
                  to="/library/artist"
                  search={{ name: artist.name }}
                  className="flex flex-col items-center gap-1.5 text-center"
                >
                  <Artwork url={artist.artworkUrl} className="w-full rounded-full" iconSize={28} />
                  <span className="w-full truncate text-sm font-medium">{artist.name}</span>
                </Link>
              </li>
            ))}
          </Row>
        </Section>
      )}
      {hasTracks && tracks.data && tracks.data.length > 0 && (
        <Section view="tracks" count={trackCount}>
          <div data-testid="track-list" className="flex flex-col">
            {tracks.data.map((track) => (
              <TrackRow
                key={track.id}
                track={track}
                onPlay={(picked) =>
                  void playFromLibrary({
                    selection: { kind: 'allTracks' },
                    startTrackId: picked.id,
                    shuffle: false,
                  })
                }
              />
            ))}
          </div>
        </Section>
      )}
    </div>
  );
};

const PlaylistTile: FC<{
  id: string;
  name: string;
  cover: ReactNode;
  count: number;
  length: LengthSummary | null;
}> = ({ id, name, cover, count, length }) => {
  const { t } = useTranslation();
  const lengthLabel = useLengthLabel(length);
  return (
    <Link to="/library/playlist" search={{ id }} className="flex flex-col gap-1.5">
      {cover}
      <span className="truncate text-sm font-medium">{name}</span>
      <span className="text-muted -mt-1 truncate text-xs">
        {t('library.trackCount', { count })}
      </span>
      {lengthLabel && <span className="text-muted -mt-1 truncate text-xs">{lengthLabel}</span>}
    </Link>
  );
};

/** Folders are set, but their scan found no songs (yet). */
const NoTracksYet: FC = () => {
  const { t } = useTranslation();
  return <p className="text-muted text-sm">{t('library.noTracksFound')}</p>;
};
