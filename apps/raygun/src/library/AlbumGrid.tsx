import type { LibraryAlbum } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useAlbumMenuItems } from '../music/cardMenus';
import { Artwork } from './Artwork';
import { CardMenu } from '../shell/CardMenu';

/** The library's albums as covers; each has the album menu (play, queue, add to a playlist…). */
export const AlbumGrid: FC<{ albums: LibraryAlbum[] }> = ({ albums }) => {
  const { t } = useTranslation();
  const menu = useAlbumMenuItems();
  return (
    <ul
      data-testid="album-grid"
      className="grid grid-cols-2 gap-x-3 gap-y-5 sm:grid-cols-3 lg:grid-cols-5 xl:grid-cols-6"
    >
      {albums.map((album) => (
        <li key={`${album.artist}\u001f${album.title}`} className="relative">
          <CardMenu
            label={t('menu.options', { name: album.title })}
            items={() =>
              menu({ title: album.title, artist: album.artist, artworkUrl: album.artworkUrl })
            }
          />
          <Link
            to="/library/album"
            search={{ artist: album.artist, album: album.title }}
            className="flex flex-col gap-1.5"
          >
            <Artwork url={album.artworkUrl} className="w-full rounded-lg" iconSize={40} />
            <span className="truncate text-sm font-medium">{album.title}</span>
            <span className="text-muted -mt-1 truncate text-xs">{album.artist}</span>
          </Link>
        </li>
      ))}
    </ul>
  );
};
