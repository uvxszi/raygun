import { Link } from '@tanstack/react-router';
import { CheckSquare, FolderPlus, ImageIcon } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useCoverSheetStore } from '../music/coverSheetStore';
import { useSettings } from '../settings/useSettings';
import { MenuButton } from '../shell/MenuButton';
import { SourceTag } from '../shell/SourceTag';
import { CARD_MENU_CLASS, CardPlayButton } from './CardPlayButton';
import { COVER_GRID_CLASS, coverGridStyle } from './coverGrid';
import { MangaCover } from './MangaCover';
import { AdultMark } from '../shell/AdultMark';
import { groupWords } from './groupWords';
import { ROUTES } from './medium';
import { GroupPickSheet } from '../select/GroupPickSheet';
import { Selectable } from '../select/Selectable';
import { useInSelectionScope } from '../select/scopeContext';
import { useSelectionStore } from '../select/selectionStore';
import type { PickedTitle } from '../select/selectionStore';
import type { Medium } from './medium';
import { usePlayTitle, useTitleCardMenu } from './titleMenu';
import type { CardTitle } from './titleMenu';

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
  /** A line under the cover. */
  subtitle?: string;
  /** The source it came from (a search result), as a tag apart from the line. */
  source?: string;
  /** Marked adult by its source, or from a source rated for adults: an 18+ tag. */
  adult?: boolean;
};

/**
 * Covers in a grid, each opening its title page. Compact puts the title over the cover's bottom
 * edge (Mihon's default); comfortable puts it underneath. A video's play button plays it; ⋮, a
 * right click or a long press opens the title's full menu.
 */
export const MangaGrid: FC<{
  items: MangaGridItem[];
  medium?: Medium;
  /** Wide 16:9 thumbnails in wider columns (M46.45: video sites). */
  landscape?: boolean;
}> = ({ items, medium = 'manga', landscape = false }) => {
  const { t } = useTranslation();
  const settings = useSettings().data;
  const menu = useTitleCardMenu();
  const play = usePlayTitle();
  const card = (item: MangaGridItem): CardTitle => ({
    medium,
    sourceId: item.sourceId,
    id: item.id,
    title: item.title,
    coverUrl: item.coverUrl,
    subtitle: item.subtitle ?? item.source ?? null,
    ...(item.inLibrary !== undefined ? { inLibrary: item.inLibrary } : {}),
    ...(item.link ? { link: item.link } : {}),
  });
  const selectable = useInSelectionScope();
  const [grouping, setGrouping] = useState<PickedTitle | null>(null);
  // A source's result as a selection item; library covers (a link of their own) aren't picked.
  const pickOf = (item: MangaGridItem): PickedTitle => ({
    kind: 'title',
    medium,
    sourceId: item.sourceId,
    remoteId: item.id,
    title: item.title,
  });
  // The title's full menu; a source's result can also join a group, and be picked in a scope
  // (where a long press picks, so ⋮ and a right click open this).
  const itemMenu = (item: MangaGridItem) =>
    menu(
      card(item),
      item.link
        ? {}
        : {
            ...(selectable
              ? {
                  select: {
                    label: t('search.select.pick', { title: item.title }),
                    icon: CheckSquare,
                    onSelect: () => useSelectionStore.getState().toggle(pickOf(item)),
                  },
                }
              : {}),
            ...(item.coverUrl
              ? {
                  viewCover: {
                    label: t('cover.view'),
                    icon: ImageIcon,
                    onSelect: () =>
                      useCoverSheetStore
                        .getState()
                        .open({ url: item.coverUrl as string, title: item.title }),
                  },
                }
              : {}),
            addToGroup: {
              label: t('library.categories.addTo', groupWords(medium)),
              icon: FolderPlus,
              onSelect: () => setGrouping(pickOf(item)),
            },
          },
    );
  const video = medium === 'video';
  const compact = (settings?.coverStyle ?? 'compact') === 'compact';
  const unreadBadges = settings?.unreadBadges ?? true;
  const downloadBadges = settings?.downloadBadges ?? true;
  return (
    <>
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
            <Selectable picked={pickOf(item)} disabled={Boolean(item.link)}>
              <Link
                to={item.link?.to ?? ROUTES[medium].title}
                search={item.link?.search ?? { source: item.sourceId, id: item.id }}
                aria-label={item.title}
                data-testid="manga-card"
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
                    <span
                      className={`absolute inset-x-0 bottom-0 bg-gradient-to-t from-black/85 via-black/50 to-transparent pt-8 pb-2 pl-2 ${video ? 'pr-12' : 'pr-2'}`}
                    >
                      <span className="line-clamp-2 text-xs font-semibold text-white">
                        {item.title}
                      </span>
                    </span>
                  )}
                  {video && (
                    <CardPlayButton
                      label={t('menu.playTitle', { name: item.title })}
                      onPlay={() => play(card(item))}
                    />
                  )}
                  <MenuButton
                    label={t('menu.options', { name: item.title })}
                    items={() => itemMenu(item)}
                    className={CARD_MENU_CLASS}
                    iconSize={16}
                  />
                </span>
                {!compact && <span className="line-clamp-2 text-xs font-medium">{item.title}</span>}
                {item.subtitle && (
                  <span className="text-muted line-clamp-1 text-[11px]">{item.subtitle}</span>
                )}
                {(item.source || item.adult) && (
                  <span className="flex min-w-0 items-center gap-1">
                    {item.adult && <AdultMark />}
                    {item.source && <SourceTag name={item.source} />}
                  </span>
                )}
              </Link>
            </Selectable>
          </li>
        ))}
      </ul>
      {grouping && (
        <GroupPickSheet
          medium={medium}
          titles={[grouping]}
          onDone={() => setGrouping(null)}
          onClose={() => setGrouping(null)}
        />
      )}
    </>
  );
};
