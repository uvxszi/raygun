import { commands } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
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
import { MusicOverview } from '../library/MusicOverview';
import { ScanBanner } from '../library/ScanBanner';
import { TrackList } from '../library/TrackList';
import { AudioViewChips } from '../library/AudioViewChips';
import { isLocalView } from '../library/views';
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

/** Library → Audio: everything (All), playlists, favorites, albums, artists and tracks. */
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
      <AudioViewChips view={view} />
      {connect.isError && (
        <p role="alert" className="text-primary text-sm">
          {connect.error.message}
        </p>
      )}
      {view === 'all' && (
        <MusicOverview
          hasFolders={hasFolders}
          trackCount={status.data?.trackCount ?? 0}
          ready={status.data !== undefined}
        />
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

/**
 * The library: what you have, by kind of media (one top bar, one row of media tabs): Audio
 * (music and podcasts), Comics, Books and Videos.
 */
export const LibraryPage: FC = () => {
  const { view = 'all', media } = useSearch({ from: '/library' });
  return (
    <section data-testid="library-page" className="flex min-h-full flex-col gap-4">
      {!media && view === 'podcasts' && <PodcastsView />}
      {!media && view !== 'podcasts' && <MusicLibrary view={view} />}
      {media && <MangaLibraryView key={media} medium={media} />}
    </section>
  );
};
