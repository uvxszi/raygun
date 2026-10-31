import { useSearch } from '@tanstack/react-router';
import { Play, Shuffle } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AlbumGrid } from '../library/AlbumGrid';
import { playFromLibrary, useAlbums } from '../library/useLibrary';

export const ArtistPage: FC = () => {
  const { t } = useTranslation();
  const { name } = useSearch({ from: '/library/artist' });
  const albums = useAlbums(name);
  const selection = { kind: 'artist' as const, artist: name };

  return (
    <section data-testid="artist-page" className="flex flex-col gap-5">
      <h1 className="text-2xl font-bold tracking-tight">{name}</h1>
      <div className="flex gap-3">
        <button
          type="button"
          onClick={() => void playFromLibrary({ selection, startTrackId: null, shuffle: false })}
          className="bg-primary text-background flex items-center gap-2 rounded-full px-6 py-2.5 font-semibold"
        >
          <Play aria-hidden size={18} />
          {t('library.playAll')}
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
      {albums.data && <AlbumGrid albums={albums.data} />}
    </section>
  );
};
