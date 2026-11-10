import type { LibraryDisplayMode, MangaLibraryEntry } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { FolderOpen, Pin, Play } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PickTarget } from '../select/PickTarget';
import { withSelect } from '../select/withSelect';
import { useSettings } from '../settings/useSettings';
import type { MenuItem } from '../shell/contextMenuStore';
import { MenuButton } from '../shell/MenuButton';
import { SourceTag } from '../shell/SourceTag';
import { videoTitleLink } from '../watch/useVideoLibrary';
import { CARD_MENU_CLASS } from './CardPlayButton';
import type { LibraryDisplay } from './libraryDisplay';
import { COVER_GRID_CLASS, coverGridStyle } from './coverGrid';
import { MangaCover } from './MangaCover';
import { ROUTES } from './medium';
import type { Medium } from './medium';
import { useTitleCardMenu } from './titleMenu';

/** Titles drawn at first and added as the end comes near: a library can hold thousands. */
const ITEMS_AT_ONCE = 120;

/** Which badges show, from Settings → Library. */
type BadgeChoice = { local: boolean; unread: boolean; downloaded: boolean };

/** Covers per row; 0 fits as many as have room (Mihon's adaptive columns). */
type ItemProps = {
  entry: MangaLibraryEntry;
  medium: Medium;
  mode: LibraryDisplayMode;
  selected: boolean;
  selecting: boolean;
  showContinue: boolean;
  badges: BadgeChoice;
  onToggle: (entry: MangaLibraryEntry) => void;
  onLongPress: (entry: MangaLibraryEntry) => void;
  onContinue: (entry: MangaLibraryEntry) => void;
  /** Items added at the end of the title's ⋮ (Downloads adds deleting its downloads). */
  moreItems?: (entry: MangaLibraryEntry) => MenuItem[];
  /** The title is in the library: Downloads lists titles kept out of it too. */
  inLibrary: boolean;
};

const Badges: FC<{ entry: MangaLibraryEntry; medium: Medium; badges: BadgeChoice }> = ({
  entry,
  medium,
  badges,
}) => {
  const { t } = useTranslation();
  const { local: localBadge, unread: unreadBadges, downloaded: downloadBadges } = badges;
  return (
    <>
      {(downloadBadges && entry.downloaded > 0) || (unreadBadges && entry.unread > 0) ? (
        <span className="flex overflow-hidden rounded-md text-xs font-bold">
          {downloadBadges && entry.downloaded > 0 && (
            <span
              aria-label={t('manga.downloadedCount', { count: entry.downloaded })}
              className="bg-foreground text-background px-1.5"
            >
              {entry.downloaded}
            </span>
          )}
          {unreadBadges && entry.unread > 0 && (
            <span
              aria-label={t(medium === 'video' ? 'watch.unwatched' : 'manga.unread', {
                count: entry.unread,
              })}
              className="bg-primary text-background px-1.5"
            >
              {entry.unread}
            </span>
          )}
        </span>
      ) : null}
      {entry.pinned && (
        <span
          aria-label={t('library.pinned')}
          className="bg-background/90 text-primary grid size-5 place-items-center rounded-md"
        >
          <Pin aria-hidden size={12} />
        </span>
      )}
      {localBadge && entry.local && (
        <span
          aria-label={t('library.localBadge')}
          className="bg-background/90 text-foreground grid size-5 place-items-center rounded-md"
        >
          <FolderOpen aria-hidden size={12} />
        </span>
      )}
    </>
  );
};

const ContinueButton: FC<{
  entry: MangaLibraryEntry;
  medium: Medium;
  large: boolean;
  onContinue: (entry: MangaLibraryEntry) => void;
}> = ({ entry, medium, large, onContinue }) => {
  const { t } = useTranslation();
  return (
    <button
      type="button"
      aria-label={t(medium === 'video' ? 'library.continueWatching' : 'library.continueReading', {
        title: entry.title,
      })}
      onClick={(event) => {
        event.preventDefault();
        event.stopPropagation();
        onContinue(entry);
      }}
      className={`bg-primary/90 text-background grid shrink-0 place-items-center rounded-full shadow ${large ? 'size-8' : 'size-7'}`}
    >
      <Play aria-hidden size={large ? 18 : 14} fill="currentColor" />
    </button>
  );
};

/**
 * One library title in the chosen display, linking to its page. It's picked by a finger held on
 * it, a right click, Ctrl/Shift-click or its circle (the cover's top left; its ⋮ is on the top
 * right), and while picking a tap picks instead of opening.
 */
const LibraryItem: FC<ItemProps> = ({
  entry,
  medium,
  mode,
  selected,
  selecting,
  showContinue,
  badges,
  onToggle,
  onLongPress,
  onContinue,
  moreItems,
  inLibrary,
}) => {
  const link =
    medium === 'video'
      ? videoTitleLink(entry.sourceId, entry.remoteId)
      : { to: ROUTES[medium].title, search: { source: entry.sourceId, id: entry.remoteId } };
  const continueShown = showContinue && entry.unread > 0 && !selecting;
  const { t } = useTranslation();
  const titleMenu = useTitleCardMenu();
  // The title's full menu, with Select first, then what the list adds (its groups, downloads).
  const menu = (): MenuItem[] => {
    const items = titleMenu({
      medium,
      sourceId: entry.sourceId,
      id: entry.remoteId,
      title: entry.title,
      coverUrl: entry.coverUrl,
      subtitle: entry.authors.join(', ') || null,
      inLibrary,
      titleId: entry.titleId,
    });
    const more = moreItems?.(entry) ?? [];
    return withSelect(
      [...items, ...(more.length > 0 ? [{ kind: 'separator' } as MenuItem, ...more] : [])],
      t,
      selected ? null : () => onToggle(entry),
    );
  };
  // Last in the cover, so nothing drawn over the cover covers it; it stays while picking.
  const options = (
    <MenuButton
      label={t('menu.options', { name: entry.title })}
      items={menu}
      className={
        mode === 'list'
          ? 'text-muted hover:text-foreground grid size-9 shrink-0 place-items-center rounded-full'
          : `${CARD_MENU_CLASS} z-10`
      }
      iconSize={16}
    />
  );
  const linkProps = {
    to: link.to,
    search: link.search,
    // A long press picks: the webview would start dragging the link instead.
    draggable: false,
    'aria-pressed': selecting ? selected : undefined,
    'data-testid': 'library-item',
    'aria-label': entry.title,
  };
  const pick = {
    selected,
    selecting,
    toggle: () => onToggle(entry),
    extend: () => onLongPress(entry),
  };
  const pickLabel = t('search.select.pick', { title: entry.title });

  if (mode === 'list') {
    return (
      <PickTarget pick={pick} label={pickLabel} place="row">
        <Link
          {...linkProps}
          className={`flex items-center gap-3 rounded-xl px-2 py-1.5 ${selected ? '' : 'hover:bg-surface'}`}
        >
          <span className="relative w-10 shrink-0 overflow-hidden rounded-md">
            <MangaCover url={entry.coverUrl} video={medium === 'video'} className="w-full" />
          </span>
          <span className="flex min-w-0 flex-1 flex-col">
            <span className="line-clamp-2 text-sm font-medium">{entry.title}</span>
            {entry.authors.length > 0 && (
              <span className="text-muted truncate text-xs">{entry.authors.join(', ')}</span>
            )}
            <span className="flex min-w-0">
              <SourceTag name={entry.sourceName} />
            </span>
          </span>
          <span className="flex shrink-0 items-center gap-1">
            <Badges entry={entry} medium={medium} badges={badges} />
          </span>
          {continueShown && (
            <ContinueButton entry={entry} medium={medium} large={false} onContinue={onContinue} />
          )}
          {options}
        </Link>
      </PickTarget>
    );
  }

  const compact = mode === 'compactGrid';
  const coverOnly = mode === 'coverOnlyGrid';
  return (
    <PickTarget pick={pick} label={pickLabel}>
      <Link {...linkProps} className="flex flex-col gap-1">
        <span className="relative overflow-hidden rounded-xl">
          <MangaCover
            url={entry.coverUrl}
            video={medium === 'video'}
            className={`w-full ${selected ? 'opacity-75' : ''}`}
          />
          {/* The circle takes the top left corner while it shows: the badges move over. */}
          <span
            className={`absolute top-1.5 right-11 left-1.5 flex flex-wrap gap-1 transition-[padding] ${
              selecting ? 'pl-7' : 'group-hover/pick:pl-7'
            }`}
          >
            <Badges entry={entry} medium={medium} badges={badges} />
          </span>
          {compact && (
            <span className="absolute inset-x-0 bottom-0 flex items-end gap-1 bg-gradient-to-t from-black/85 via-black/50 to-transparent px-2 pt-8 pb-2">
              <span className="line-clamp-2 flex-1 text-xs font-semibold text-white">
                {entry.title}
              </span>
              {continueShown && (
                <ContinueButton
                  entry={entry}
                  medium={medium}
                  large={false}
                  onContinue={onContinue}
                />
              )}
            </span>
          )}
          {!compact && continueShown && (
            <span className="absolute right-1.5 bottom-1.5">
              <ContinueButton entry={entry} medium={medium} large onContinue={onContinue} />
            </span>
          )}
          {options}
        </span>
        {mode === 'comfortableGrid' && (
          <span className="line-clamp-2 min-h-8 text-xs font-medium">{entry.title}</span>
        )}
        {coverOnly && <span className="sr-only">{entry.title}</span>}
      </Link>
    </PickTarget>
  );
};

/** The library's titles in a grid or list, as Settings → Library and the category say. */
export const LibraryItems: FC<{
  entries: MangaLibraryEntry[];
  medium: Medium;
  mode: LibraryDisplayMode;
  display: LibraryDisplay;
  selected: ReadonlySet<string>;
  onToggle: (entry: MangaLibraryEntry) => void;
  onLongPress: (entry: MangaLibraryEntry) => void;
  onContinue: (entry: MangaLibraryEntry) => void;
  moreItems?: (entry: MangaLibraryEntry) => MenuItem[];
  /** Whether a title is in the library; all are unless said (Downloads says). */
  inLibrary?: (entry: MangaLibraryEntry) => boolean;
  footer?: ReactNode;
}> = ({
  entries,
  medium,
  mode,
  display,
  selected,
  onToggle,
  onLongPress,
  onContinue,
  moreItems,
  inLibrary = () => true,
}) => {
  const settings = useSettings().data;
  const selecting = selected.size > 0;
  const badges: BadgeChoice = {
    local: display.localBadge,
    unread: settings?.unreadBadges ?? true,
    downloaded: settings?.downloadBadges ?? true,
  };
  const [count, setCount] = useState(ITEMS_AT_ONCE);
  const sentinel = useRef<HTMLLIElement>(null);
  useEffect(() => {
    const element = sentinel.current;
    if (!element || typeof IntersectionObserver === 'undefined') return;
    const observer = new IntersectionObserver(
      (seen) => {
        if (seen.some((entry) => entry.isIntersecting)) setCount((shown) => shown + ITEMS_AT_ONCE);
      },
      { rootMargin: '800px' },
    );
    observer.observe(element);
    return () => observer.disconnect();
  }, [count, entries.length]);

  const item = (entry: MangaLibraryEntry) => (
    <li key={entry.titleId}>
      <LibraryItem
        entry={entry}
        medium={medium}
        mode={mode}
        selected={selected.has(entry.titleId)}
        selecting={selecting}
        showContinue={display.continueButton}
        badges={badges}
        onToggle={onToggle}
        onLongPress={onLongPress}
        onContinue={onContinue}
        moreItems={moreItems}
        inLibrary={inLibrary(entry)}
      />
    </li>
  );
  const shown = entries.slice(0, count).map(item);
  const more = count < entries.length && (
    <li ref={sentinel} aria-hidden className="col-span-full h-px" />
  );
  if (mode === 'list') {
    return (
      <ul data-testid="library-list" className="flex flex-col gap-1">
        {shown}
        {more}
      </ul>
    );
  }
  return (
    <ul
      data-testid="library-grid"
      data-mode={mode}
      className={COVER_GRID_CLASS}
      style={coverGridStyle(settings?.coverSize ?? 0)}
    >
      {shown}
      {more}
    </ul>
  );
};
