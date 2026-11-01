import { commands } from '@raygun/bindings';
import { Link, useSearch } from '@tanstack/react-router';
import { FolderCog, FolderPlus, FolderSearch, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { FavoritesView } from '../collections/FavoritesView';
import { MangaLibraryView } from '../manga/MangaLibraryView';
import { PlaylistsView } from '../collections/PlaylistsView';
import { PodcastsView } from '../podcasts/PodcastsView';
import { AlbumGrid } from '../library/AlbumGrid';
import { ArtistList } from '../library/ArtistList';
import { FirstRunCard } from '../library/FirstRunCard';
import { AddFolderSheet } from '../library/AddFolderForm';
import { LibraryTop } from '../library/LibraryTop';
import { ScanBanner } from '../library/ScanBanner';
import { TrackList } from '../library/TrackList';
import { LIBRARY_VIEWS, isLocalView } from '../library/views';
import type { LibraryView } from '../library/views';
import {
  playFromLibrary,
  useAlbums,
  useAllTracks,
  useArtists,
  useConnectDeviceMusic,
  useLibraryStatus,
} from '../library/useLibrary';

const AlbumsView: FC = () => {
  const albums = useAlbums();
  return albums.data ? <AlbumGrid albums={albums.data} /> : null;
};

const ArtistsView: FC = () => {
  const artists = useArtists();
  return artists.data ? <ArtistList artists={artists.data} /> : null;
};

const TracksView: FC = () => {
  const tracks = useAllTracks();
  if (!tracks.data) {
    return null;
  }
  return (
    <TrackList
      tracks={tracks.data}
      onPlay={(track) =>
        void playFromLibrary({
          selection: { kind: 'allTracks' },
          startTrackId: track.id,
          shuffle: false,
        })
      }
    />
  );
};

/** Library → Music: albums, artists, tracks, playlists and favorites. */
const MusicLibrary: FC<{ view: LibraryView }> = ({ view }) => {
  const { t } = useTranslation();
  const status = useLibraryStatus();
  const connect = useConnectDeviceMusic();
  const [adding, setAdding] = useState(false);
  const local = isLocalView(view);
  const hasFolders = (status.data?.roots.length ?? 0) > 0;
  const hasTracks = hasFolders && (status.data?.trackCount ?? 0) > 0;

  return (
    <>
      <LibraryTop
        media="music"
        menu={[
          {
            label: t('library.findDeviceMusic'),
            icon: FolderSearch,
            onSelect: () => connect.mutate(),
          },
          { label: t('library.addFolder'), icon: FolderPlus, onSelect: () => setAdding(true) },
          ...(hasFolders
            ? [
                {
                  label: t('library.rescan'),
                  icon: RefreshCw,
                  onSelect: () => void commands.libraryRescan(),
                },
              ]
            : []),
          { label: t('library.manageFolders'), icon: FolderCog, to: '/settings/library' },
        ]}
      />
      <ScanBanner />
      <nav
        className="-mx-4 flex gap-2 overflow-x-auto px-4 lg:mx-0 lg:px-0"
        aria-label={t('library.views')}
      >
        {LIBRARY_VIEWS.map((option) => (
          <Link
            key={option}
            to="/library"
            search={{ view: option }}
            replace
            data-testid={`library-view-${option}`}
            aria-current={option === view ? 'page' : undefined}
            className={
              option === view
                ? 'bg-primary/20 text-primary shrink-0 rounded-full px-4 py-1.5 text-sm font-semibold'
                : 'bg-surface shrink-0 rounded-full px-4 py-1.5 text-sm'
            }
          >
            {t(`library.view.${option}`)}
          </Link>
        ))}
      </nav>
      {connect.isError && (
        <p role="alert" className="text-primary text-sm">
          {connect.error.message}
        </p>
      )}
      {view === 'playlists' && <PlaylistsView />}
      {view === 'favorites' && <FavoritesView />}
      {local && status.data && !hasFolders && <FirstRunCard />}
      {local && hasFolders && status.data?.trackCount === 0 && !status.data.scanning && (
        <p className="text-muted text-sm">{t('library.noTracksFound')}</p>
      )}
      {local && hasTracks && view === 'albums' && <AlbumsView />}
      {local && hasTracks && view === 'artists' && <ArtistsView />}
      {local && hasTracks && view === 'tracks' && <TracksView />}
      {adding && <AddFolderSheet onClose={() => setAdding(false)} />}
    </>
  );
};

/** The library: what you have, by kind of media (one top bar, one row of media tabs). */
export const LibraryPage: FC = () => {
  const { view = 'albums', media } = useSearch({ from: '/library' });
  return (
    <section data-testid="library-page" className="flex flex-col gap-4">
      {!media && <MusicLibrary view={view} />}
      {(media === 'manga' || media === 'novel' || media === 'video') && (
        <MangaLibraryView key={media} medium={media} />
      )}
      {media === 'podcast' && <PodcastsView />}
    </section>
  );
};
