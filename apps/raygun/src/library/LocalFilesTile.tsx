import { Link } from '@tanstack/react-router';
import { ArrowDown, Folder } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useLocalFolders } from '../localFiles/useLocalFolders';

/** What Local Files' folders hold, all kinds together. */
const useLocalCount = () => {
  const folders = useLocalFolders();
  return (folders.data ?? []).reduce(
    (sum, folder) =>
      sum + folder.counts.songs + folder.counts.comics + folder.counts.books + folder.counts.videos,
    0,
  );
};

/** The Library's green Local files block (M46.29), always shown: it opens the folders. */
export const LocalFilesTile: FC = () => {
  const { t } = useTranslation();
  const count = useLocalCount();
  return (
    <li className="w-28 shrink-0">
      <Link to="/local-files" className="flex flex-col gap-1.5">
        <span className="grid aspect-square w-full place-items-center rounded-lg bg-emerald-500/20 text-emerald-500">
          <span className="grid size-12 place-items-center rounded-full bg-emerald-500 text-white">
            <Folder aria-hidden size={24} fill="currentColor" />
          </span>
        </span>
        <span className="truncate text-sm font-medium">{t('localFiles.title')}</span>
        <span className="text-muted -mt-1 truncate text-xs">
          {t('localFiles.items', { count })}
        </span>
      </Link>
    </li>
  );
};

/** Home's tiles for Downloaded (blue) and Local files (green), always shown (M46.29). */
export const HomeFixedTiles: FC<{ className: string }> = ({ className }) => {
  const { t } = useTranslation();
  return (
    <>
      <li>
        <Link to="/library" search={{ view: 'downloaded' }} className={className}>
          <span className="grid aspect-square place-items-center rounded-lg bg-sky-500/20 text-sky-500">
            <ArrowDown aria-hidden size={32} strokeWidth={3} />
          </span>
          <span className="truncate font-medium">{t('library.view.downloaded')}</span>
        </Link>
      </li>
      <li>
        <Link to="/local-files" className={className}>
          <span className="grid aspect-square place-items-center rounded-lg bg-emerald-500/20 text-emerald-500">
            <Folder aria-hidden size={30} fill="currentColor" />
          </span>
          <span className="truncate font-medium">{t('localFiles.title')}</span>
        </Link>
      </li>
    </>
  );
};
