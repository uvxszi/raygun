import { commands } from '@raygun/bindings';
import { useSearch } from '@tanstack/react-router';
import { Folder, FolderCog, FolderPlus, FolderSearch, Import, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AudiobooksView } from '../audiobooks/AudiobooksView';
import { useCollectionSheets } from '../collections/collectionSheets';
import { DownloadedView } from '../collections/DownloadedView';
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
import { isLibraryView, isLocalView } from '../library/views';
import type { LibraryView } from '../library/views';
import { useLayout } from '../layout/useLayout';
import {
  playFromLibrary,
  useAlbums,
  useAllTracks,
  useArtists,
  useConnectDeviceMusic,
  useLibraryStatus,
} from '../library/useLibrary';
import { AllLibrary } from '../library/AllLibrary';
import { FollowedArtistList } from '../music/FollowedArtistList';

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

/**
 * Library → Audio: everything (All), playlists, favorites, downloaded songs, albums, artists and
 * tracks.
 */
const MusicLibrary: FC<{ view: LibraryView }> = ({ view }) => {
  const { t } = useTranslation();
  const status = useLibraryStatus();
  const connect = useConnectDeviceMusic();
  const [adding, setAdding] = useState(false);
  const openImport = useCollectionSheets((state) => state.openImport);
  const local = isLocalView(view);
  const hasFolders = (status.data?.roots.length ?? 0) > 0;
  const hasTracks = hasFolders && (status.data?.trackCount ?? 0) > 0;

  return (
    <>
      <LibraryTop
        media="music"
        // Import songs from anywhere in Audio (M46.45).
        actions={
          <button
            type="button"
            aria-label={t('import.open')}
            onClick={openImport}
            className="bg-surface hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-medium"
          >
            <Import aria-hidden size={16} />
            {t('import.short')}
          </button>
        }
        menu={[
          { label: t('import.open'), icon: Import, onSelect: openImport },
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
          // Folders of songs, comics, books and videos alike (M46.27).
          { label: t('localFiles.title'), icon: Folder, to: '/local-files' },
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
      {view === 'downloaded' && <DownloadedView />}
      {view === 'audiobooks' && <AudiobooksView />}
      {local && status.data && !hasFolders && <FirstRunCard />}
      {local && hasFolders && status.data?.trackCount === 0 && !status.data.scanning && (
        <p className="text-muted text-sm">{t('library.noTracksFound')}</p>
      )}
      {local && hasTracks && view === 'albums' && <AlbumsView />}
      {view === 'artists' && <FollowedArtistList />}
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
  const search = useSearch({ from: '/library' });
  const layout = useLayout();
  // Without a tab in the address, the first one the user shows (M46.5); a view means Audio.
  const firstMedia = layout.libraryMedia.find((item) => item.visible)?.id ?? 'music';
  const media = search.media ?? (search.view ? 'music' : firstMedia);
  const firstView = layout.audioViews.find((item) => item.visible)?.id;
  const view: LibraryView = search.view ?? (isLibraryView(firstView) ? firstView : 'all');
  return (
    <section data-testid="library-page" className="flex min-h-full flex-col gap-4">
      {media === 'all' && <AllLibrary />}
      {media === 'music' && view === 'podcasts' && <PodcastsView />}
      {media === 'music' && view !== 'podcasts' && <MusicLibrary view={view} />}
      {(media === 'manga' || media === 'novel' || media === 'video') && (
        <MangaLibraryView key={media} medium={media} />
      )}
    </section>
  );
};
