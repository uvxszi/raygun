import { Link, useSearch } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AlbumGrid } from '../library/AlbumGrid';
import { ArtistList } from '../library/ArtistList';
import { FirstRunCard } from '../library/FirstRunCard';
import { LibraryFolders } from '../library/LibraryFolders';
import { ScanBanner } from '../library/ScanBanner';
import { TrackList } from '../library/TrackList';
import { LIBRARY_VIEWS } from '../library/views';
import {
  playFromLibrary,
  useAlbums,
  useAllTracks,
  useArtists,
  useLibraryStatus,
} from '../library/useLibrary';
import { OpenMediaForm } from './OpenMediaForm';
import { PageHeader } from './PageHeader';

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

export const LibraryPage: FC = () => {
  const { t } = useTranslation();
  const { view = 'albums' } = useSearch({ from: '/library' });
  const status = useLibraryStatus();

  if (!status.data) {
    return <section data-testid="library-page" />;
  }

  const hasFolders = status.data.roots.length > 0;

  return (
    <section data-testid="library-page" className="flex flex-col gap-5">
      <PageHeader title={t('library.title')} />
      <ScanBanner />
      {!hasFolders && <FirstRunCard />}
      {hasFolders && status.data.trackCount === 0 && !status.data.scanning && (
        <p className="text-muted text-sm">{t('library.noTracksFound')}</p>
      )}
      {hasFolders && status.data.trackCount > 0 && (
        <>
          <nav className="flex gap-2" aria-label={t('library.views')}>
            {LIBRARY_VIEWS.map((option) => (
              <Link
                key={option}
                to="/library"
                search={{ view: option }}
                data-testid={`library-view-${option}`}
                aria-current={option === view ? 'page' : undefined}
                className={
                  option === view
                    ? 'bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold'
                    : 'bg-surface rounded-full px-4 py-1.5 text-sm'
                }
              >
                {t(`library.view.${option}`)}
              </Link>
            ))}
          </nav>
          {view === 'albums' && <AlbumsView />}
          {view === 'artists' && <ArtistsView />}
          {view === 'tracks' && <TracksView />}
        </>
      )}
      {hasFolders && <LibraryFolders roots={status.data.roots} />}
      <details className="bg-surface rounded-xl px-5 py-3 text-sm">
        <summary className="cursor-pointer font-medium">{t('library.openTitle')}</summary>
        <div className="mt-3">
          <OpenMediaForm />
        </div>
      </details>
    </section>
  );
};
