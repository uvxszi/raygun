import { useSearch } from '@tanstack/react-router';
import { Play, Shuffle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { TopBar } from '../shell/TopBar';

import { Artwork } from '../library/Artwork';
import { TrackRow } from '../library/TrackRow';
import { playFromLibrary, useAlbumTracks } from '../library/useLibrary';
import { summarizeLengths, useLengthLabel } from '../music/collectionLength';
import { usePlayContext } from '../music/playContext';

export const AlbumPage: FC = () => {
  const { t } = useTranslation();
  const { artist, album } = useSearch({ from: '/library/album' });
  const tracks = useAlbumTracks(artist, album);
  const selection = { kind: 'album' as const, artist, album };
  const first = tracks.data?.[0];
  const length = useLengthLabel(
    tracks.data ? summarizeLengths(tracks.data.map((track) => track.durationMs)) : null,
  );
  usePlayContext(
    album
      ? {
          kind: 'album',
          name: album,
          subtitle: artist || null,
          artworkUrl: first?.artworkUrl ?? null,
          to: '/library/album',
          search: { artist, album },
        }
      : null,
  );

  return (
    <section data-testid="album-page" className="flex flex-col gap-5">
      <TopBar back fallback="/library" plainTitle title={artist} />
      <header className="flex flex-col gap-4 sm:flex-row sm:items-end">
        <Artwork
          url={tracks.data?.find((track) => track.artworkUrl)?.artworkUrl}
          className="w-48 self-center rounded-xl shadow-lg sm:self-auto"
          iconSize={64}
        />
        <div className="flex flex-col gap-1 text-center sm:text-left">
          <h1 className="text-2xl font-bold tracking-tight">{album}</h1>
          <p className="text-muted">{artist}</p>
          {tracks.data && (
            <p className="text-muted text-xs">
              {[first?.year, t('library.trackCount', { count: tracks.data.length }), length]
                .filter(Boolean)
                .join(' · ')}
            </p>
          )}
        </div>
      </header>
      <div className="flex justify-center gap-3 sm:justify-start">
        <button
          type="button"
          onClick={() => void playFromLibrary({ selection, startTrackId: null, shuffle: false })}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold"
        >
          <Play aria-hidden size={18} />
          {t('library.play')}
        </button>
        <button
          type="button"
          onClick={() => void playFromLibrary({ selection, startTrackId: null, shuffle: true })}
          className="bg-surface flex items-center gap-2 rounded-full px-6 py-2.5"
        >
          <Shuffle aria-hidden size={18} />
          {t('library.shuffle')}
        </button>
      </div>
      <ul className="flex flex-col">
        {tracks.data?.map((track) => (
          <li key={track.id}>
            <TrackRow
              track={track}
              variant="number"
              onPlay={() =>
                void playFromLibrary({ selection, startTrackId: track.id, shuffle: false })
              }
            />
          </li>
        ))}
      </ul>
    </section>
  );
};
