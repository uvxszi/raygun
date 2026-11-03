import { Link } from '@tanstack/react-router';
import { ArrowDown, ChevronRight, Heart } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { useFavorites, usePlaylists } from '../collections/useCollections';
import { useDownloadedSongs } from '../music/useSongDownloads';
import { useAudiobooks } from '../audiobooks/useAudiobooks';
import { PodcastArt } from '../podcasts/PodcastArt';
import { LocalFilesTile } from './LocalFilesTile';
import { openContextMenu } from '../shell/contextMenuStore';
import { usePinMenuItem } from '../shell/usePinMenuItem';
import { usePlaylistMenu } from '../collections/usePlaylistMenu';
import { useSubscriptions } from '../podcasts/usePodcasts';
import { Artwork } from './Artwork';
import { FirstRunCard } from './FirstRunCard';
import { TrackList } from './TrackList';
import { playFromLibrary, useAlbums, useAllTracks, useArtists } from './useLibrary';
import type { LibraryView } from './views';
import { Carousel } from '../shell/Carousel';

/** How many songs the overview lists before "See all". */
const SONGS_SHOWN = 8;

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
  // Right-click a playlist, show or book: pin it to the sidebar (M46.29).
  const pinItems = usePinMenuItem();
  const playlistMenu = usePlaylistMenu();
  const playlists = usePlaylists();
  const podcasts = useSubscriptions();
  const audiobooks = useAudiobooks();
  const favorites = useFavorites();
  const downloaded = useDownloadedSongs();
  const albums = useAlbums();
  const artists = useArtists();
  const tracks = useAllTracks();
  const hasTracks = hasFolders && trackCount > 0;
  const favoriteCount = favorites.data?.length ?? 0;
  const downloadedCount = downloaded.data?.length ?? 0;
  const playlistCount = playlists.data?.length ?? 0;

  return (
    <div data-testid="music-overview" className="flex flex-col gap-6">
      {/* Favorites, Downloaded and Local files are always the first three (M46.29). */}
      {
        <Section view="playlists" count={playlistCount}>
          <Row>
            {
              <li className="w-28 shrink-0">
                <Link
                  to="/library"
                  search={{ view: 'favorites' }}
                  replace
                  className="flex flex-col gap-1.5"
                >
                  <span className="bg-primary/20 text-primary grid aspect-square w-full place-items-center rounded-lg">
                    <Heart aria-hidden size={36} fill="currentColor" />
                  </span>
                  <span className="truncate text-sm font-medium">{t('collections.favorites')}</span>
                  <span className="text-muted -mt-1 truncate text-xs">
                    {t('library.trackCount', { count: favoriteCount })}
                  </span>
                </Link>
              </li>
            }
            {
              <li className="w-28 shrink-0">
                <Link
                  to="/library"
                  search={{ view: 'downloaded' }}
                  replace
                  className="flex flex-col gap-1.5"
                >
                  <span className="grid aspect-square w-full place-items-center rounded-lg bg-sky-500/20 text-sky-500">
                    <span className="grid size-12 place-items-center rounded-full bg-sky-500 text-white">
                      <ArrowDown aria-hidden size={28} strokeWidth={3} />
                    </span>
                  </span>
                  <span className="truncate text-sm font-medium">
                    {t('library.view.downloaded')}
                  </span>
                  <span className="text-muted -mt-1 truncate text-xs">
                    {t('library.trackCount', { count: downloadedCount })}
                  </span>
                </Link>
              </li>
            }
            <LocalFilesTile />
            {playlists.data?.map((playlist) => (
              <li
                key={playlist.id}
                className="w-28 shrink-0"
                onContextMenu={playlistMenu(playlist)}
              >
                <Link
                  to="/library/playlist"
                  search={{ id: playlist.id }}
                  className="flex flex-col gap-1.5"
                >
                  <PlaylistCover
                    artworkUrls={playlist.artworkUrls}
                    coverUrl={playlist.coverUrl}
                    className="aspect-square w-full rounded-lg"
                  />
                  <span className="truncate text-sm font-medium">{playlist.name}</span>
                  <span className="text-muted -mt-1 truncate text-xs">
                    {t('library.trackCount', { count: playlist.itemCount })}
                  </span>
                </Link>
              </li>
            ))}
          </Row>
        </Section>
      }
      {podcasts.data && podcasts.data.length > 0 && (
        <Section view="podcasts" count={podcasts.data.length}>
          <Row>
            {podcasts.data.map((podcast) => (
              <li
                key={podcast.id}
                className="w-28 shrink-0"
                onContextMenu={(event) =>
                  openContextMenu(
                    event,
                    pinItems({
                      kind: 'podcast',
                      to: '/podcast',
                      search: { id: podcast.id },
                      name: podcast.title,
                      artworkUrl: podcast.artworkUrl ?? null,
                    }),
                  )
                }
              >
                <Link to="/podcast" search={{ id: podcast.id }} className="flex flex-col gap-1.5">
                  <PodcastArt url={podcast.artworkUrl} className="w-full rounded-lg" />
                  <span className="line-clamp-2 text-sm font-medium">{podcast.title}</span>
                </Link>
              </li>
            ))}
          </Row>
        </Section>
      )}
      {audiobooks.data && audiobooks.data.length > 0 && (
        <Section view="audiobooks" count={audiobooks.data.length}>
          <Row>
            {audiobooks.data.map((book) => (
              <li
                key={book.id}
                className="w-28 shrink-0"
                onContextMenu={(event) =>
                  openContextMenu(
                    event,
                    pinItems({
                      kind: 'audiobook',
                      to: '/audiobook',
                      search: { id: book.id },
                      name: book.title,
                      artworkUrl: book.artworkUrl ?? null,
                    }),
                  )
                }
              >
                <Link to="/audiobook" search={{ id: book.id }} className="flex flex-col gap-1.5">
                  <PodcastArt url={book.artworkUrl} className="w-full rounded-lg" />
                  <span className="line-clamp-2 text-sm font-medium">{book.title}</span>
                </Link>
              </li>
            ))}
          </Row>
        </Section>
      )}
      {ready && !hasFolders && <FirstRunCard />}
      {hasFolders && !hasTracks && ready && (
        <p className="text-muted text-sm">{t('library.noTracksFound')}</p>
      )}
      {hasTracks && albums.data && albums.data.length > 0 && (
        <Section view="albums" count={albums.data.length}>
          <Row>
            {albums.data.map((album) => (
              <li key={`${album.artist}\u001f${album.title}`} className="w-28 shrink-0">
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
            {artists.data.map((artist) => (
              <li key={artist.name} className="w-24 shrink-0">
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
          <TrackList
            tracks={tracks.data.slice(0, SONGS_SHOWN)}
            onPlay={(track) =>
              void playFromLibrary({
                selection: { kind: 'allTracks' },
                startTrackId: track.id,
                shuffle: false,
              })
            }
          />
        </Section>
      )}
    </div>
  );
};
