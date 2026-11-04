import type { LibraryDisplayMode, MangaLibraryEntry } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import { Check, FolderOpen, Pin, Play } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, MouseEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { useSettings } from '../settings/useSettings';
import { videoTitleLink } from '../watch/useVideoLibrary';
import type { LibraryDisplay } from './libraryDisplay';
import { COVER_GRID_CLASS, coverGridStyle } from './coverGrid';
import { MangaCover } from './MangaCover';
import { ROUTES } from './medium';
import type { Medium } from './medium';

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

/** One library title in the chosen display, linking to its page (or selecting while selecting). */
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
}) => {
  const link =
    medium === 'video'
      ? videoTitleLink(entry.sourceId, entry.remoteId)
      : { to: ROUTES[medium].title, search: { source: entry.sourceId, id: entry.remoteId } };
  const continueShown = showContinue && entry.unread > 0 && !selecting;
  const onClick = (event: MouseEvent) => {
    if (!selecting) return;
    event.preventDefault();
    onToggle(entry);
  };
  const onContextMenu = (event: MouseEvent) => {
    // A long press selects (Mihon), adding everything between it and the last pick.
    event.preventDefault();
    onLongPress(entry);
  };
  const common = {
    to: link.to,
    search: link.search,
    onClick,
    onContextMenu,
    'aria-pressed': selecting ? selected : undefined,
    'data-testid': 'library-item',
    'aria-label': entry.title,
  };
  const check = selected && (
    <span className="bg-primary text-background absolute top-1.5 right-1.5 grid size-6 place-items-center rounded-full">
      <Check aria-hidden size={14} strokeWidth={3} />
    </span>
  );

  if (mode === 'list') {
    return (
      <Link
        {...common}
        className={`flex items-center gap-3 rounded-xl px-2 py-1.5 ${selected ? 'bg-primary/15' : 'hover:bg-surface'}`}
      >
        <span className="relative w-10 shrink-0 overflow-hidden rounded-md">
          <MangaCover url={entry.coverUrl} video={medium === 'video'} className="w-full" />
        </span>
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="line-clamp-2 text-sm font-medium">{entry.title}</span>
          <span className="text-muted truncate text-xs">{entry.sourceName}</span>
        </span>
        <span className="flex shrink-0 items-center gap-1">
          <Badges entry={entry} medium={medium} badges={badges} />
          {selected && <Check aria-hidden size={16} className="text-primary" />}
        </span>
        {continueShown && (
          <ContinueButton entry={entry} medium={medium} large={false} onContinue={onContinue} />
        )}
      </Link>
    );
  }

  const compact = mode === 'compactGrid';
  const coverOnly = mode === 'coverOnlyGrid';
  return (
    <Link {...common} className="flex flex-col gap-1">
      <span
        className={`relative overflow-hidden rounded-xl ${selected ? 'ring-primary ring-2' : ''}`}
      >
        <MangaCover
          url={entry.coverUrl}
          video={medium === 'video'}
          className={`w-full ${selected ? 'opacity-75' : ''}`}
        />
        <span className="absolute top-1.5 left-1.5 flex gap-1">
          <Badges entry={entry} medium={medium} badges={badges} />
        </span>
        {check}
        {compact && (
          <span className="absolute inset-x-0 bottom-0 flex items-end gap-1 bg-gradient-to-t from-black/85 via-black/50 to-transparent px-2 pt-8 pb-2">
            <span className="line-clamp-2 flex-1 text-xs font-semibold text-white">
              {entry.title}
            </span>
            {continueShown && (
              <ContinueButton entry={entry} medium={medium} large={false} onContinue={onContinue} />
            )}
          </span>
        )}
        {!compact && continueShown && (
          <span className="absolute right-1.5 bottom-1.5">
            <ContinueButton entry={entry} medium={medium} large onContinue={onContinue} />
          </span>
        )}
      </span>
      {mode === 'comfortableGrid' && (
        <span className="line-clamp-2 min-h-8 text-xs font-medium">{entry.title}</span>
      )}
      {coverOnly && <span className="sr-only">{entry.title}</span>}
    </Link>
  );
};

/** The library's titles in a grid or list, as Settings → Library and the category say. */
export const LibraryItems: FC<{
  entries: MangaLibraryEntry[];
  medium: Medium;
  mode: LibraryDisplayMode;
  display: LibraryDisplay;
  selected: Set<string>;
  onToggle: (entry: MangaLibraryEntry) => void;
  onLongPress: (entry: MangaLibraryEntry) => void;
  onContinue: (entry: MangaLibraryEntry) => void;
  footer?: ReactNode;
}> = ({ entries, medium, mode, display, selected, onToggle, onLongPress, onContinue }) => {
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
