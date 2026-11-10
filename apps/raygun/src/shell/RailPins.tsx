import {
  DndContext,
  MouseSensor,
  PointerSensor,
  TouchSensor,
  closestCenter,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import type { DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { PlaylistSummary, QuickAccessItem, RailPin, Website } from '@raygun/bindings';
import { Link, useNavigate } from '@tanstack/react-router';
import { observeElementRect, useVirtualizer } from '@tanstack/react-virtual';
import {
  ArrowDown,
  ArrowDownUp,
  BookAudio,
  BookOpen,
  Check,
  Clapperboard,
  Disc3,
  EllipsisVertical,
  Folder,
  Heart,
  ListMusic,
  Mic,
  Pin,
  Settings2,
  Sparkles,
  UserRound,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useLayoutEffect, useRef, useState } from 'react';
import type { CSSProperties, FC, MouseEvent as ReactMouseEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { Artwork } from '../library/Artwork';
import { useOpenWebsite } from '../websites/useWebsites';
import { LetterAvatar } from './LetterAvatar';
import { Popover } from './Popover';
import { QuickAccessMenu } from './QuickAccessMenu';
import { pageKey, RAIL_ORDERS } from './useRail';
import { useQuickAccessItems } from './useQuickAccessItems';

/** A row's height in rem: a 2.5 rem cover with its padding, and the gap under it. */
const ROW_REM = 3.375;
/** What a row adds to its cover's size (rem): its padding and the gap under it. */
const ROW_EXTRA_REM = ROW_REM - 2.5;

const KIND_ICON: Record<string, LucideIcon> = {
  playlist: ListMusic,
  album: Disc3,
  artist: UserRound,
  podcast: Mic,
  audiobook: BookAudio,
  manga: BookOpen,
  novel: BookOpen,
  video: Clapperboard,
  mix: Sparkles,
};

/** A pin's tile: the fixed ones coloured (Favorites red, Downloaded blue, Local files green). */
export const PinArt: FC<{
  /** `name` lets an artist without a picture of their own show one another source has. */
  pin: Pick<RailPin, 'kind' | 'artworkUrl'> & { artworkUrls?: string[]; name?: string };
  /** A playlist pin's playlist: its cover now, not the one it had when pinned (M46.45). */
  playlist?: Pick<PlaylistSummary, 'artworkUrls' | 'artworkSongs' | 'coverUrl'>;
  /** A website pin's site: its icon. */
  website?: Website;
  className: string;
  /** The icons' size (px) in a tile without a picture; 16 unless the tile is larger. */
  iconSize?: number;
}> = ({ pin, playlist, website, className, iconSize = 16 }) => {
  // The fixed tiles' icons keep their sizes to each other.
  const scaled = (size: number) => Math.round((size * iconSize) / 16);
  const fixed: Record<string, { colour: string; icon: ReactNode }> = {
    favorites: {
      colour: 'from-primary/80 to-primary/30 bg-gradient-to-br text-background',
      icon: <Heart aria-hidden size={scaled(16)} fill="currentColor" />,
    },
    downloaded: {
      colour: 'bg-sky-500 text-white',
      icon: <ArrowDown aria-hidden size={scaled(18)} strokeWidth={3} />,
    },
    localFiles: {
      colour: 'bg-emerald-500 text-white',
      icon: <Folder aria-hidden size={scaled(17)} fill="currentColor" />,
    },
  };
  const special = fixed[pin.kind];
  if (special) {
    return (
      <span
        className={`grid shrink-0 place-items-center rounded-md ${special.colour} ${className}`}
      >
        {special.icon}
      </span>
    );
  }
  if (pin.kind === 'website') {
    return (
      <span className={`grid shrink-0 place-items-center ${className}`}>
        {/* The larger avatar (4 rem) once the tile holds it. */}
        <LetterAvatar
          name={website?.name ?? ''}
          logo={website?.icon}
          size={iconSize >= 26 ? 'lg' : 'md'}
        />
      </span>
    );
  }
  const mosaic =
    playlist ??
    (pin.kind === 'playlist' && (pin.artworkUrls?.length ?? 0) > 0
      ? { artworkUrls: pin.artworkUrls ?? [], artworkSongs: null, coverUrl: pin.artworkUrl }
      : undefined);
  if (mosaic) {
    return (
      <PlaylistCover
        artworkUrls={mosaic.artworkUrls}
        artworkSongs={mosaic.artworkSongs}
        coverUrl={mosaic.coverUrl}
        className={`shrink-0 rounded-md ${className}`}
      />
    );
  }
  // An artist's picture that's missing or fails is looked for elsewhere (M46.40), by name.
  if (pin.kind === 'artist' && pin.name) {
    return (
      <Artwork
        url={pin.artworkUrl}
        fallback={{ kind: 'artist', name: pin.name }}
        icon={UserRound}
        className={`shrink-0 rounded-full ${className}`}
        iconSize={iconSize}
      />
    );
  }
  if (pin.artworkUrl) {
    return (
      <Artwork
        url={pin.artworkUrl}
        className={`shrink-0 ${pin.kind === 'artist' ? 'rounded-full' : 'rounded-md'} ${className}`}
        iconSize={iconSize}
      />
    );
  }
  const Icon = KIND_ICON[pin.kind] ?? ListMusic;
  return (
    <span
      className={`bg-background text-muted grid shrink-0 place-items-center rounded-md ${className}`}
    >
      <Icon aria-hidden size={iconSize} />
    </span>
  );
};

type RowProps = {
  item: QuickAccessItem;
  name: string;
  playlist?: PlaylistSummary;
  website?: Website;
  narrow: boolean;
  /** In the phone's drawer: opening one takes the drawer's place in history; ⋮ opens its menu. */
  drawer: boolean;
  /** The cover's size in px when the drawer sizes it (else 2.5 rem). */
  cover?: number;
  onMenu: (anchor: HTMLElement) => void;
  /** A website opens in the built-in browser instead of a page. */
  onOpenSite: (id: string) => void;
};

/**
 * One item: its cover and name, a link to its page; a right click or a hold opens its menu. With
 * a cover the drawer sized, the name gets two lines, or one above what it's by.
 */
const Row: FC<RowProps> = ({
  item,
  name,
  playlist,
  website,
  narrow,
  drawer,
  cover,
  onMenu,
  onOpenSite,
}) => {
  const { t } = useTranslation();
  const look = `text-muted hover:bg-background hover:text-foreground flex min-w-0 flex-1 items-center gap-3 rounded-xl ${narrow ? 'justify-center p-1.5' : 'px-2 py-1.5'}`;
  const openMenu = (event: ReactMouseEvent<HTMLElement>) => {
    // This menu only: neither the webview's nor another one.
    event.preventDefault();
    event.stopPropagation();
    onMenu(event.currentTarget);
  };
  const byline = cover && item.subtitle ? item.subtitle : null;
  const content = (
    <>
      <PinArt
        pin={item}
        playlist={playlist}
        website={website}
        className={cover ? 'size-[var(--pin-cover)]' : 'size-10'}
        iconSize={cover ? Math.max(16, Math.round(cover * 0.4)) : undefined}
      />
      {!narrow &&
        (cover ? (
          <span className="flex min-w-0 flex-1 flex-col">
            <span
              className={`text-foreground text-[0.9375rem] leading-snug wrap-anywhere ${byline ? 'truncate' : 'line-clamp-2'}`}
            >
              {name}
            </span>
            {byline && <span className="truncate text-xs">{byline}</span>}
          </span>
        ) : (
          <span className="text-foreground min-w-0 flex-1 truncate text-sm">{name}</span>
        ))}
      {!narrow && item.pinned && (
        <Pin aria-hidden size={13} className="text-primary shrink-0 rotate-45" />
      )}
    </>
  );
  return (
    <div
      className="flex items-center"
      style={cover ? ({ '--pin-cover': `${cover}px` } as CSSProperties) : undefined}
    >
      {item.kind === 'website' && item.search.site ? (
        <button
          type="button"
          aria-label={narrow ? name : undefined}
          onClick={() => onOpenSite(item.search.site ?? '')}
          onContextMenu={openMenu}
          className={`${look} text-left`}
        >
          {content}
        </button>
      ) : (
        <Link
          to={item.to}
          search={item.search as never}
          replace={drawer}
          aria-label={narrow ? name : undefined}
          onContextMenu={openMenu}
          className={look}
        >
          {content}
        </Link>
      )}
      {drawer && !narrow && (
        <button
          type="button"
          aria-label={t('quickAccess.menu', { name })}
          aria-haspopup="menu"
          onClick={openMenu}
          className="text-muted hover:text-foreground grid size-10 shrink-0 place-items-center rounded-full"
        >
          <EllipsisVertical aria-hidden size={18} />
        </button>
      )}
    </div>
  );
};

/** A pin on top: dragged into another place among the pins. */
const SortableRow: FC<RowProps> = (props) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: pageKey(props.item),
  });
  return (
    <li
      ref={setNodeRef}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={isDragging ? 'relative z-10 opacity-80' : ''}
      {...attributes}
      {...listeners}
      role="listitem"
    >
      <Row {...props} />
    </li>
  );
};

/** The root font size in px: rows are sized in rem, and phones scale it (Interface size). */
const rootFontPx = () =>
  Number.parseFloat(getComputedStyle(document.documentElement).fontSize) || 16;

/**
 * Quick access's list (M46.29): the desktop sidebar's, and the phone's drawer (`drawer`). Pins stay
 * on top in the order they're dragged into; the audio library follows by itself, the most
 * recently played from first (Rust's `quick_access`), drawn only near the screen so a big library
 * costs nothing off it. A right click (a hold, or ⋮ in the drawer) opens an item's menu: Pin or
 * Unpin, and Remove from library for what can leave it. The phone's drawer sizes the covers from
 * its width (`cover`, px); the side rail's are 2.5 rem.
 */
export const RailPins: FC<{
  narrow: boolean;
  shown: boolean;
  drawer?: boolean;
  cover?: number;
}> = ({ narrow, shown, drawer = false, cover }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const { rail, quick, top, rest, playlistOf, siteOf, nameOf, menuItems } =
    useQuickAccessItems(shown);
  const openSite = useOpenWebsite();
  const [sortAnchor, setSortAnchor] = useState<HTMLButtonElement | null>(null);
  const [menu, setMenu] = useState<{ item: QuickAccessItem; anchor: HTMLElement } | null>(null);
  const pointer = useSensor(PointerSensor, { activationConstraint: { distance: 6 } });
  // A finger scrolls the drawer; a hold that stays put picks a pin up.
  const mouse = useSensor(MouseSensor, { activationConstraint: { distance: 6 } });
  const touch = useSensor(TouchSensor, { activationConstraint: { delay: 250, tolerance: 6 } });
  const desktopSensors = useSensors(pointer);
  const drawerSensors = useSensors(mouse, touch);
  const sensors = drawer ? drawerSensors : desktopSensors;

  const scroller = useRef<HTMLDivElement>(null);
  const list = useRef<HTMLUListElement>(null);
  const [scrollMargin, setScrollMargin] = useState(0);
  useLayoutEffect(() => {
    setScrollMargin(list.current?.offsetTop ?? 0);
  }, [top.length, rest.length, narrow, cover]);
  const rowPx = Math.round(
    cover === undefined ? ROW_REM * rootFontPx() : cover + ROW_EXTRA_REM * rootFontPx(),
  );
  // The app doesn't use the React Compiler, so the rule's memoization concern doesn't apply.
  // eslint-disable-next-line react-hooks/incompatible-library
  const virtualizer = useVirtualizer({
    count: rest.length,
    getScrollElement: () => scroller.current,
    estimateSize: () => rowPx,
    getItemKey: (index) => {
      const item = rest[index];
      return item ? pageKey(item) : index;
    },
    overscan: 6,
    scrollMargin,
    // A list not laid out yet (or measured as nothing) still shows its first rows.
    observeElementRect: (instance, report) =>
      observeElementRect(instance, (rect) =>
        report(rect.height > 0 ? rect : { width: rect.width, height: window.innerHeight }),
      ),
  });

  const rowProps = (item: QuickAccessItem): RowProps => ({
    item,
    name: nameOf(item),
    playlist: playlistOf(item),
    website: siteOf(item),
    narrow,
    drawer,
    cover,
    onMenu: (anchor) => setMenu({ item, anchor }),
    onOpenSite: (id) => openSite.mutate(id),
  });

  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    const from = top.findIndex((item) => pageKey(item) === active.id);
    const to = top.findIndex((item) => pageKey(item) === over.id);
    const moved = top[from];
    if (!moved) return;
    // Dropped on a pin below: after it; above: before it.
    const before = to > from ? (top[to + 1] ?? null) : (top[to] ?? null);
    rail.move(moved, before);
  };

  const sortButton = (size: 'small' | 'large') => (
    <button
      type="button"
      aria-label={t('rail.order')}
      data-hint={t('rail.order')}
      onClick={(event) => setSortAnchor(sortAnchor ? null : event.currentTarget)}
      className={`text-muted hover:text-foreground grid place-items-center rounded-full ${size === 'large' ? 'size-10' : 'size-7'}`}
    >
      <ArrowDownUp aria-hidden size={size === 'large' ? 17 : 15} />
    </button>
  );
  const more = quick.data?.more ?? 0;
  const empty = quick.data !== undefined && top.length === 0 && rest.length === 0;

  return (
    <>
      {narrow && drawer && (
        <div className="flex shrink-0 justify-center pb-1">{sortButton('large')}</div>
      )}
      {!narrow && (
        <div className="flex shrink-0 items-center justify-between px-4 pb-1">
          <h2 className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t('rail.library')}
          </h2>
          {sortButton(drawer ? 'large' : 'small')}
        </div>
      )}
      {sortAnchor && (
        <Popover
          anchor={sortAnchor}
          label={t('rail.order')}
          onClose={() => setSortAnchor(null)}
          width={240}
        >
          <div role="radiogroup" aria-label={t('rail.order')} className="flex flex-col py-1">
            {RAIL_ORDERS.map((order) => (
              <button
                key={order}
                type="button"
                role="radio"
                aria-checked={rail.prefs.order === order}
                onClick={() => {
                  rail.setOrder(order);
                  setSortAnchor(null);
                }}
                className="hover:bg-background flex items-center gap-2 rounded-lg px-3 py-2 text-left text-sm"
              >
                <Check
                  aria-hidden
                  size={15}
                  className={rail.prefs.order === order ? 'text-primary' : 'invisible'}
                />
                {t(`rail.orders.${order}`)}
              </button>
            ))}
          </div>
          <div className="bg-foreground/10 mx-2 my-1 h-px" />
          <button
            type="button"
            onClick={() => {
              setSortAnchor(null);
              void navigate({ to: '/settings/quickAccess' });
            }}
            className="hover:bg-background text-muted flex items-center gap-2 rounded-lg px-3 py-2 text-left text-sm"
          >
            <Settings2 aria-hidden size={15} />
            {t('quickAccess.settings')}
          </button>
        </Popover>
      )}
      <div
        ref={scroller}
        // A menu stays beside its item: scrolling the list closes it.
        onScroll={() => setMenu(null)}
        className="relative flex min-h-0 flex-1 flex-col overflow-y-auto overscroll-contain"
      >
        <div data-testid="rail-pins" className="flex flex-col">
          {top.length > 0 && (
            <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
              <SortableContext items={top.map(pageKey)} strategy={verticalListSortingStrategy}>
                <ul aria-label={t('quickAccess.pinned')} className="flex flex-col gap-0.5 pb-0.5">
                  {top.map((item) => (
                    <SortableRow key={pageKey(item)} {...rowProps(item)} />
                  ))}
                </ul>
              </SortableContext>
            </DndContext>
          )}
          <ul
            ref={list}
            className="relative w-full shrink-0"
            style={{ height: virtualizer.getTotalSize() }}
          >
            {virtualizer.getVirtualItems().map((row) => {
              const item = rest[row.index];
              if (!item) return null;
              return (
                <li
                  key={row.key}
                  className="absolute inset-x-0 top-0"
                  style={{
                    height: rowPx,
                    transform: `translateY(${row.start - scrollMargin}px)`,
                  }}
                >
                  <Row {...rowProps(item)} />
                </li>
              );
            })}
          </ul>
        </div>
        {more > 0 && !narrow && (
          <Link
            to="/library"
            search={{ view: 'all' } as never}
            replace={drawer}
            className="text-muted hover:text-foreground shrink-0 px-4 py-2 text-xs"
          >
            {t('quickAccess.more', { count: more })}
          </Link>
        )}
        {empty && !narrow && <p className="text-muted px-4 py-2 text-xs">{t('rail.empty')}</p>}
      </div>
      {menu && (
        <QuickAccessMenu
          anchor={menu.anchor}
          label={t('quickAccess.menu', { name: nameOf(menu.item) })}
          items={menuItems(menu.item)}
          onClose={() => setMenu(null)}
        />
      )}
    </>
  );
};
