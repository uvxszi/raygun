import { Link } from '@tanstack/react-router';
import { ArrowDown, Folder, Heart } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useFavorites } from '../collections/useCollections';
import { useLocalFolders } from '../localFiles/useLocalFolders';
import { summarizeLengths, useLengthLabel } from '../music/collectionLength';
import { useDownloadedSongs } from '../music/useSongDownloads';
import { useFixedTileMenuItems } from './useFixedTileMenu';
import type { FixedTile } from './useFixedTileMenu';
import { useHoldMenu } from './useHoldMenu';
import { CardMenu } from '../shell/CardMenu';

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

/** Library → All opens a block's menu on a hold or a right click too. */
type TileProps = { menuOnHold?: boolean };

/** The block's menu, and the hold and right click that open it where asked. */
const useTileMenu = (tile: FixedTile, menuOnHold: boolean) => {
  const menu = useFixedTileMenuItems();
  const items = () => menu(tile);
  const hold = useHoldMenu(items);
  return { items, hold: menuOnHold ? hold : {} };
};

const NAME = 'truncate text-sm font-medium';
const DETAIL = 'text-muted -mt-1 truncate text-xs';

/**
 * The Library's red Favorites block (M46.29), always shown, with how many songs it has. Each
 * block has its menu (`useFixedTileMenu`) on its ⋮, and in Library → All on a hold or a right
 * click too.
 */
export const FavoritesTile: FC<TileProps> = ({ menuOnHold = false }) => {
  const { t } = useTranslation();
  const { items, hold } = useTileMenu('favorites', menuOnHold);
  const favorites = useFavorites().data;
  const count = favorites?.length ?? 0;
  const length = useLengthLabel(
    favorites ? summarizeLengths(favorites.map((song) => song.durationMs)) : null,
  );
  return (
    <li className="w-28 shrink-0">
      <Link to="/library" search={{ view: 'favorites' }} className={`relative ${TILE}`} {...hold}>
        <CardMenu label={t('menu.options', { name: t('collections.favorites') })} items={items} />
        <span className="bg-primary/20 text-primary grid aspect-square w-full place-items-center rounded-lg">
          <Heart aria-hidden size={36} fill="currentColor" />
        </span>
        <span className={NAME}>{t('collections.favorites')}</span>
        <span className={DETAIL}>{t('library.trackCount', { count })}</span>
        {length && <span className={DETAIL}>{length}</span>}
      </Link>
    </li>
  );
};

/** The Library's blue Downloaded block (M46.29), always shown, with how many songs it has. */
export const DownloadedTile: FC<TileProps> = ({ menuOnHold = false }) => {
  const { t } = useTranslation();
  const { items, hold } = useTileMenu('downloaded', menuOnHold);
  const downloaded = useDownloadedSongs().data;
  const count = downloaded?.length ?? 0;
  const length = useLengthLabel(
    downloaded ? summarizeLengths(downloaded.map((song) => song.durationMs)) : null,
  );
  return (
    <li className="w-28 shrink-0">
      <Link to="/library" search={{ view: 'downloaded' }} className={`relative ${TILE}`} {...hold}>
        <CardMenu label={t('menu.options', { name: t('library.view.downloaded') })} items={items} />
        <span className="grid aspect-square w-full place-items-center rounded-lg bg-sky-500/20 text-sky-500">
          <span className="grid size-12 place-items-center rounded-full bg-sky-500 text-white">
            <ArrowDown aria-hidden size={28} strokeWidth={3} />
          </span>
        </span>
        <span className={NAME}>{t('library.view.downloaded')}</span>
        <span className={DETAIL}>{t('library.trackCount', { count })}</span>
        {length && <span className={DETAIL}>{length}</span>}
      </Link>
    </li>
  );
};

/** The Library's green Local files block (M46.29), always shown: it opens the folders. */
export const LocalFilesTile: FC<TileProps> = ({ menuOnHold = false }) => {
  const { t } = useTranslation();
  const { items, hold } = useTileMenu('localFiles', menuOnHold);
  const { count, scanning } = useLocalCount();
  return (
    <li className="w-28 shrink-0">
      <Link to="/local-files" className={`relative ${TILE}`} {...hold}>
        <CardMenu label={t('menu.options', { name: t('localFiles.title') })} items={items} />
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
