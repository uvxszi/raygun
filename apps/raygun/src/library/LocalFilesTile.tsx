import { Link } from '@tanstack/react-router';
import { ArrowDown, Folder, Heart } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useFavorites } from '../collections/useCollections';
import { useLocalFolders } from '../localFiles/useLocalFolders';
import { useDownloadedSongs } from '../music/useSongDownloads';
import { useFixedTileMenu } from './useFixedTileMenu';

/** What Local Files' folders hold, all kinds together, and whether one is being scanned. */
const useLocalCount = () => {
  const folders = useLocalFolders().data ?? [];
  return {
    count: folders.reduce(
      (sum, folder) =>
        sum +
        folder.counts.songs +
        folder.counts.comics +
        folder.counts.books +
        folder.counts.videos,
      0,
    ),
    scanning: folders.some((folder) => folder.scanning),
  };
};

const TILE = 'flex flex-col gap-1.5';
const NAME = 'truncate text-sm font-medium';
const DETAIL = 'text-muted -mt-1 truncate text-xs';

/**
 * The Library's red Favorites block (M46.29), always shown, with how many songs it has. Each
 * block has its menu on a right-click or a hold (`useFixedTileMenu`).
 */
export const FavoritesTile: FC = () => {
  const { t } = useTranslation();
  const menu = useFixedTileMenu();
  const count = useFavorites().data?.length ?? 0;
  return (
    <li className="w-28 shrink-0">
      <Link
        to="/library"
        search={{ view: 'favorites' }}
        className={TILE}
        onContextMenu={menu('favorites')}
      >
        <span className="bg-primary/20 text-primary grid aspect-square w-full place-items-center rounded-lg">
          <Heart aria-hidden size={36} fill="currentColor" />
        </span>
        <span className={NAME}>{t('collections.favorites')}</span>
        <span className={DETAIL}>{t('library.trackCount', { count })}</span>
      </Link>
    </li>
  );
};

/** The Library's blue Downloaded block (M46.29), always shown, with how many songs it has. */
export const DownloadedTile: FC = () => {
  const { t } = useTranslation();
  const menu = useFixedTileMenu();
  const count = useDownloadedSongs().data?.length ?? 0;
  return (
    <li className="w-28 shrink-0">
      <Link
        to="/library"
        search={{ view: 'downloaded' }}
        className={TILE}
        onContextMenu={menu('downloaded')}
      >
        <span className="grid aspect-square w-full place-items-center rounded-lg bg-sky-500/20 text-sky-500">
          <span className="grid size-12 place-items-center rounded-full bg-sky-500 text-white">
            <ArrowDown aria-hidden size={28} strokeWidth={3} />
          </span>
        </span>
        <span className={NAME}>{t('library.view.downloaded')}</span>
        <span className={DETAIL}>{t('library.trackCount', { count })}</span>
      </Link>
    </li>
  );
};

/** The Library's green Local files block (M46.29), always shown: it opens the folders. */
export const LocalFilesTile: FC = () => {
  const { t } = useTranslation();
  const menu = useFixedTileMenu();
  const { count, scanning } = useLocalCount();
  return (
    <li className="w-28 shrink-0">
      <Link to="/local-files" className={TILE} onContextMenu={menu('localFiles')}>
        <span className="grid aspect-square w-full place-items-center rounded-lg bg-emerald-500/20 text-emerald-500">
          <span className="grid size-12 place-items-center rounded-full bg-emerald-500 text-white">
            <Folder aria-hidden size={24} fill="currentColor" />
          </span>
        </span>
        <span className={NAME}>{t('localFiles.title')}</span>
        <span className={DETAIL}>
          {scanning ? t('localFiles.scanning') : t('localFiles.items', { count })}
        </span>
      </Link>
    </li>
  );
};

/** Home's tiles for Downloaded (blue) and Local files (green), always shown (M46.29). */
export const HomeFixedTiles: FC<{ className: string }> = ({ className }) => {
  const { t } = useTranslation();
  const menu = useFixedTileMenu();
  return (
    <>
      <li>
        <Link
          to="/library"
          search={{ view: 'downloaded' }}
          className={className}
          onContextMenu={menu('downloaded')}
        >
          <span className="grid aspect-square place-items-center rounded-lg bg-sky-500/20 text-sky-500">
            <ArrowDown aria-hidden size={32} strokeWidth={3} />
          </span>
          <span className="truncate font-medium">{t('library.view.downloaded')}</span>
        </Link>
      </li>
      <li>
        <Link to="/local-files" className={className} onContextMenu={menu('localFiles')}>
          <span className="grid aspect-square place-items-center rounded-lg bg-emerald-500/20 text-emerald-500">
            <Folder aria-hidden size={30} fill="currentColor" />
          </span>
          <span className="truncate font-medium">{t('localFiles.title')}</span>
        </Link>
      </li>
    </>
  );
};
