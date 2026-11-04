import { Link, useNavigate } from '@tanstack/react-router';
import { BookOpen, Copy, ImageIcon, Search } from 'lucide-react';
import type { FC, MouseEvent } from 'react';
import { useTranslation } from 'react-i18next';

import { useCoverSheetStore } from '../music/coverSheetStore';
import type { MenuItem } from '../shell/contextMenuStore';
import { useSettings } from '../settings/useSettings';
import { openContextMenu } from '../shell/contextMenuStore';
import { COVER_GRID_CLASS, coverGridStyle } from './coverGrid';
import { MangaCover } from './MangaCover';
import { AdultMark } from '../shell/AdultMark';
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
  /** A line under the cover, e.g. the source a search result came from (M46.9). */
  subtitle?: string;
  /** Marked adult by its source, or from a source rated for adults: an 18+ tag. */
  adult?: boolean;
};

/**
 * Covers in a grid, each opening its title page. Compact puts the title over the cover's bottom
 * edge (Mihon's default); comfortable puts it underneath.
 */
export const MangaGrid: FC<{
  items: MangaGridItem[];
  medium?: Medium;
  /** Wide 16:9 thumbnails in wider columns (M46.45: video sites). */
  landscape?: boolean;
}> = ({ items, medium = 'manga', landscape = false }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const settings = useSettings().data;
  // A cover's right-click options (M46.8): open it, find it in every source, view the cover close
  // up, copy its name.
  const coverMenu = (item: MangaGridItem) => (event: MouseEvent) => {
    const to = item.link?.to ?? ROUTES[medium].title;
    const search = item.link?.search ?? { source: item.sourceId, id: item.id };
    const viewCover: MenuItem[] = item.coverUrl
      ? [
          {
            label: t('cover.view'),
            icon: ImageIcon,
            onSelect: () =>
              useCoverSheetStore
                .getState()
                .open({ url: item.coverUrl as string, title: item.title }),
          },
        ]
      : [];
    openContextMenu(event, [
      { kind: 'heading', label: item.title },
      { label: t('menu.open'), icon: BookOpen, onSelect: () => void navigate({ to, search }) },
      {
        label: t('menu.searchEverywhere'),
        icon: Search,
        onSelect: () =>
          void navigate({
            to: '/browse',
            search: { tab: 'search', type: medium, q: item.title } as never,
          }),
      },
      ...viewCover,
      {
        label: t('menu.copyTitle'),
        icon: Copy,
        onSelect: () => void navigator.clipboard?.writeText(item.title).catch(() => undefined),
      },
    ]);
  };
  const compact = (settings?.coverStyle ?? 'compact') === 'compact';
  const unreadBadges = settings?.unreadBadges ?? true;
  const downloadBadges = settings?.downloadBadges ?? true;
  return (
    <ul
      className={COVER_GRID_CLASS}
      style={
        landscape
          ? { gridTemplateColumns: 'repeat(auto-fill, minmax(min(100%, 13rem), 1fr))' }
          : coverGridStyle(settings?.coverSize ?? 0)
      }
    >
      {items.map((item) => (
        <li key={`${item.sourceId}:${item.id}`}>
          <Link
            to={item.link?.to ?? ROUTES[medium].title}
            search={item.link?.search ?? { source: item.sourceId, id: item.id }}
            data-testid="manga-card"
            onContextMenu={coverMenu(item)}
            className="flex flex-col gap-1"
          >
            <span className="relative overflow-hidden rounded-xl">
              <MangaCover
                url={item.coverUrl}
                video={medium === 'video'}
                landscape={landscape}
                className="w-full"
              />
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
            {(item.subtitle || item.adult) && (
              <span className="flex min-w-0 items-center gap-1">
                {item.adult && <AdultMark />}
                {item.subtitle && (
                  <span className="text-muted truncate text-[11px]">{item.subtitle}</span>
                )}
              </span>
            )}
          </Link>
        </li>
      ))}
    </ul>
  );
};
