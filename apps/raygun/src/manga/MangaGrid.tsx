import { Link } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
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
  downloaded?: number;
  /** Where it opens, when not the medium's title page (Stremio titles in the video library). */
  link?: { to: string; search: Record<string, string> };
};

/** Covers per row (Settings → Library); 0 fits them to the screen. */
const COLUMNS: Record<number, string> = {
  0: 'grid-cols-3 sm:grid-cols-4 lg:grid-cols-6',
  2: 'grid-cols-2',
  3: 'grid-cols-3',
  4: 'grid-cols-4',
  5: 'grid-cols-5',
  6: 'grid-cols-6',
};

/**
 * Covers in a grid, each opening its title page. Compact puts the title over the cover's bottom
 * edge (Mihon's default); comfortable puts it underneath.
 */
export const MangaGrid: FC<{ items: MangaGridItem[]; medium?: Medium }> = ({
  items,
  medium = 'manga',
}) => {
  const { t } = useTranslation();
  const settings = useSettings().data;
  const compact = (settings?.coverStyle ?? 'compact') === 'compact';
  const unreadBadges = settings?.unreadBadges ?? true;
  const downloadBadges = settings?.downloadBadges ?? true;
  return (
    <ul className={`grid gap-2 ${COLUMNS[settings?.gridColumns ?? 0] ?? COLUMNS[0]}`}>
      {items.map((item) => (
        <li key={`${item.sourceId}:${item.id}`}>
          <Link
            to={item.link?.to ?? ROUTES[medium].title}
            search={item.link?.search ?? { source: item.sourceId, id: item.id }}
            data-testid="manga-card"
            className="flex flex-col gap-1"
          >
            <span className="relative overflow-hidden rounded-xl">
              <MangaCover url={item.coverUrl} video={medium === 'video'} className="w-full" />
              <span className="absolute top-1.5 left-1.5 flex gap-1">
                {item.inLibrary && (
                  <span className="bg-primary text-background rounded-md px-1.5 py-0.5 text-[10px] font-bold">
                    {t('manga.inLibraryBadge')}
                  </span>
                )}
                {unreadBadges && item.unread ? (
                  <span
                    aria-label={t(medium === 'video' ? 'watch.unwatched' : 'manga.unread', {
                      count: item.unread,
                    })}
                    className="bg-primary text-background rounded-md px-1.5 text-xs font-bold"
                  >
                    {item.unread}
                  </span>
                ) : null}
                {downloadBadges && item.downloaded ? (
                  <span
                    aria-label={t('manga.downloadedCount', { count: item.downloaded })}
                    className="bg-foreground text-background rounded-md px-1.5 text-xs font-bold"
                  >
                    {item.downloaded}
                  </span>
                ) : null}
              </span>
              {compact && (
                <span className="absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 via-black/50 to-transparent px-2 pt-8 pb-2">
                  <span className="line-clamp-2 text-xs font-semibold text-white">
                    {item.title}
                  </span>
                </span>
              )}
            </span>
            {!compact && <span className="line-clamp-2 text-xs font-medium">{item.title}</span>}
          </Link>
        </li>
      ))}
    </ul>
  );
};
