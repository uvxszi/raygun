import { Link, useSearch } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { FavoritesView } from '../collections/FavoritesView';
import { MangaLibraryView } from '../manga/MangaLibraryView';
import { PlaylistsView } from '../collections/PlaylistsView';
import { PodcastsView } from '../podcasts/PodcastsView';
import { VideosLibraryView } from '../watch/VideosLibraryView';
import { AlbumGrid } from '../library/AlbumGrid';
import { ArtistList } from '../library/ArtistList';
import { FirstRunCard } from '../library/FirstRunCard';
import { LibraryFolders } from '../library/LibraryFolders';
import { ScanBanner } from '../library/ScanBanner';
import { TrackList } from '../library/TrackList';
import { LIBRARY_VIEWS, isLocalView } from '../library/views';
import {
  playFromLibrary,
  useAlbums,
  useAllTracks,
  useArtists,
  useLibraryStatus,
} from '../library/useLibrary';
import type { LibraryMedia } from '../router';
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

type Media = 'music' | LibraryMedia;

/** Music, manga or novels: the library's top-level choice. */
const MEDIA: Media[] = ['music', 'podcast', 'manga', 'novel', 'video'];

/** The kinds of media the library holds, as a row of tabs. */
const MediaSwitch: FC<{ media: Media }> = ({ media }) => {
  const { t } = useTranslation();
  return (
    <nav
      aria-label={t('library.media.label')}
      className="bg-surface flex gap-1 overflow-x-auto rounded-xl p-1"
    >
      {MEDIA.map((value) => (
        <Link
          key={value}
          to="/library"
          search={value === 'music' ? {} : { media: value }}
          data-testid={`library-media-${value}`}
          aria-current={media === value ? 'page' : undefined}
          className={`min-w-fit flex-1 rounded-lg px-3 py-1.5 text-center text-sm ${media === value ? 'bg-background font-semibold' : 'text-muted'}`}
        >
          {t(`library.media.${value}`)}
        </Link>
      ))}
    </nav>
  );
};

export const LibraryPage: FC = () => {
  const { t } = useTranslation();
  const { view = 'albums', media } = useSearch({ from: '/library' });
  const status = useLibraryStatus();

  if (media) {
    return (
      <section data-testid="library-page" className="flex flex-col gap-5">
        <PageHeader title={t('library.title')} />
        <MediaSwitch media={media} />
        {(media === 'manga' || media === 'novel') && (
          <MangaLibraryView key={media} medium={media} />
        )}
        {media === 'podcast' && <PodcastsView />}
        {media === 'video' && <VideosLibraryView />}
      </section>
    );
  }

  if (!status.data) {
    return <section data-testid="library-page" />;
  }

  const hasFolders = status.data.roots.length > 0;
  const hasTracks = hasFolders && status.data.trackCount > 0;
  const local = isLocalView(view);

  return (
    <section data-testid="library-page" className="flex flex-col gap-5">
      <PageHeader title={t('library.title')} />
      <MediaSwitch media="music" />
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
            data-testid={`library-view-${option}`}
            aria-current={option === view ? 'page' : undefined}
            className={
              option === view
                ? 'bg-primary text-background shrink-0 rounded-full px-4 py-1.5 text-sm font-semibold'
                : 'bg-surface shrink-0 rounded-full px-4 py-1.5 text-sm'
            }
          >
            {t(`library.view.${option}`)}
          </Link>
        ))}
      </nav>
      {view === 'playlists' && <PlaylistsView />}
      {view === 'favorites' && <FavoritesView />}
      {local && !hasFolders && <FirstRunCard />}
      {local && hasFolders && status.data.trackCount === 0 && !status.data.scanning && (
        <p className="text-muted text-sm">{t('library.noTracksFound')}</p>
      )}
      {local && hasTracks && view === 'albums' && <AlbumsView />}
      {local && hasTracks && view === 'artists' && <ArtistsView />}
      {local && hasTracks && view === 'tracks' && <TracksView />}
      {local && hasFolders && <LibraryFolders roots={status.data.roots} />}
      <details className="bg-surface rounded-xl px-5 py-3 text-sm">
        <summary className="cursor-pointer font-medium">{t('library.openTitle')}</summary>
        <div className="mt-3">
          <OpenMediaForm />
        </div>
      </details>
    </section>
  );
};
