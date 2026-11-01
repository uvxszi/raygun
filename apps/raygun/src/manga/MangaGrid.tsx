import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { MangaCover } from './MangaCover';
import { ROUTES } from './medium';
import type { Medium } from './medium';

export type MangaGridItem = {
  sourceId: string;
  /** The source's id for the title. */
  id: string;
  title: string;
  coverUrl: string | null;
  inLibrary?: boolean;
  unread?: number;
};

/** Covers in a responsive grid, each opening its title page. */
export const MangaGrid: FC<{ items: MangaGridItem[]; medium?: Medium }> = ({
  items,
  medium = 'manga',
}) => {
  const { t } = useTranslation();
  return (
    <ul className="grid grid-cols-3 gap-3 sm:grid-cols-4 lg:grid-cols-6">
      {items.map((item) => (
        <li key={`${item.sourceId}:${item.id}`}>
          <Link
            to={ROUTES[medium].title}
            search={{ source: item.sourceId, id: item.id }}
            data-testid="manga-card"
            className="flex flex-col gap-1"
          >
            <span className="relative">
              <MangaCover url={item.coverUrl} className="w-full rounded-lg" />
              {item.inLibrary && (
                <span className="bg-primary text-background absolute top-1 left-1 rounded px-1.5 text-[10px] font-bold">
                  {t('manga.inLibraryBadge')}
                </span>
              )}
              {item.unread ? (
                <span
                  aria-label={t('manga.unread', { count: item.unread })}
                  className="bg-primary text-background absolute top-1 right-1 rounded px-1.5 text-xs font-bold"
                >
                  {item.unread}
                </span>
              ) : null}
            </span>
            <span className="line-clamp-2 text-xs font-medium">{item.title}</span>
          </Link>
        </li>
      ))}
    </ul>
  );
};
