import type { LibraryArtist } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from './Artwork';

export const ArtistList: FC<{ artists: LibraryArtist[] }> = ({ artists }) => {
  const { t } = useTranslation();
  return (
    <ul data-testid="artist-list" className="flex flex-col">
      {artists.map((artist) => (
        <li key={artist.name}>
          <Link
            to="/library/artist"
            search={{ name: artist.name }}
            className="flex items-center gap-3 py-2"
          >
            <Artwork url={artist.artworkUrl} className="size-12 rounded-full" iconSize={20} />
            <span className="flex min-w-0 flex-col">
              <span className="truncate font-medium">{artist.name}</span>
              <span className="text-muted truncate text-xs">
                {t('library.artistCounts', {
                  albums: t('library.artistAlbums', { count: artist.albumCount }),
                  tracks: t('library.artistTracks', { count: artist.trackCount }),
                })}
              </span>
            </span>
          </Link>
        </li>
      ))}
    </ul>
  );
};
