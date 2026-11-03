import { DndContext, PointerSensor, closestCenter, useSensor, useSensors } from '@dnd-kit/core';
import type { DragEndEvent } from '@dnd-kit/core';
import { SortableContext, useSortable, verticalListSortingStrategy } from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { PlaylistSummary, RailOrder, RailPin } from '@raygun/bindings';
import { Link } from '@tanstack/react-router';
import {
  ArrowDown,
  ArrowDownUp,
  ArrowUp,
  BookAudio,
  BookOpen,
  Check,
  Clapperboard,
  Disc3,
  Folder,
  Heart,
  ListMusic,
  Mic,
  PinOff,
  Sparkles,
  UserRound,
} from 'lucide-react';
import type { LucideIcon } from 'lucide-react';
import { useEffect, useRef, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { PlaylistCover } from '../collections/PlaylistCover';
import { usePlaylists } from '../collections/useCollections';
import { usePlaylistMenuItems } from '../collections/usePlaylistMenu';
import type { MenuItem } from './contextMenuStore';
import { Artwork } from '../library/Artwork';
import { openContextMenu } from './contextMenuStore';
import { Popover } from './Popover';
import { useRail } from './useRail';

const pinKey = (pin: Pick<RailPin, 'to' | 'search'>) =>
  `${pin.to}?${Object.entries(pin.search)
    .sort(([a], [b]) => a.localeCompare(b))
    .map(([key, value]) => `${key}=${value}`)
    .join('&')}`;

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
  pin: Pick<RailPin, 'kind' | 'artworkUrl'>;
  /** A playlist pin's playlist: its cover now, not the one it had when pinned (M46.45). */
  playlist?: PlaylistSummary;
  className: string;
}> = ({ pin, playlist, className }) => {
  const fixed: Record<string, { colour: string; icon: ReactNode }> = {
    favorites: {
      colour: 'from-primary/80 to-primary/30 bg-gradient-to-br text-background',
      icon: <Heart aria-hidden size={16} fill="currentColor" />,
    },
    downloaded: {
      colour: 'bg-sky-500 text-white',
      icon: <ArrowDown aria-hidden size={18} strokeWidth={3} />,
    },
    localFiles: {
      colour: 'bg-emerald-500 text-white',
      icon: <Folder aria-hidden size={17} fill="currentColor" />,
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
  if (playlist) {
    return (
      <PlaylistCover
        artworkUrls={playlist.artworkUrls}
        coverUrl={playlist.coverUrl}
        className={`shrink-0 rounded-md ${className}`}
      />
    );
  }
  if (pin.artworkUrl) {
    return (
      <Artwork
        url={pin.artworkUrl}
        className={`shrink-0 ${pin.kind === 'artist' ? 'rounded-full' : 'rounded-md'} ${className}`}
        iconSize={16}
      />
    );
  }
  const Icon = KIND_ICON[pin.kind] ?? ListMusic;
  return (
    <span
      className={`bg-background text-muted grid shrink-0 place-items-center rounded-md ${className}`}
    >
      <Icon aria-hidden size={16} />
    </span>
  );
};

const SortablePin: FC<{
  pin: RailPin;
  name: string;
  playlist?: PlaylistSummary;
  narrow: boolean;
  onMenu: (event: React.MouseEvent) => void;
}> = ({ pin, name, playlist, narrow, onMenu }) => {
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id: pinKey(pin),
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
      <Link
        to={pin.to}
        search={pin.search as never}
        aria-label={narrow ? name : undefined}
        onContextMenu={onMenu}
        className={`text-muted hover:bg-background hover:text-foreground flex items-center gap-3 rounded-xl ${narrow ? 'justify-center p-1.5' : 'px-2 py-1.5'}`}
      >
        <PinArt pin={pin} playlist={playlist} className="size-10" />
        {!narrow && <span className="text-foreground truncate text-sm">{name}</span>}
      </Link>
    </li>
  );
};

/**
 * The sidebar's pins (M46.29): dragged into any order, or sorted by when each was last played
 * from or opened; right-click one to unpin it or move it. New playlists are pinned when made.
 */
export const RailPins: FC<{ narrow: boolean; shown: boolean }> = ({ narrow, shown }) => {
  const { t } = useTranslation();
  const rail = useRail();
  const playlists = usePlaylists();
  const playlistMenuItems = usePlaylistMenuItems();
  const [sortAnchor, setSortAnchor] = useState<HTMLButtonElement | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));

  // The first run's pins, once the playlists are known.
  useEffect(() => {
    if (shown && rail.loaded && !rail.prefs.seeded && playlists.data) rail.seed(playlists.data);
  }, [rail, playlists.data, shown]);

  // New playlists are pinned by Rust (M46.41: quick access, with its switch and limit).
  // Pins of playlists that are gone (deleted here or on another device) go too: at start those
  // the list doesn't have, later only ones the list had before (M46.45: a new playlist's pin
  // can arrive before the list that has it).
  const seen = useRef<Set<string> | null>(null);
  useEffect(() => {
    if (!shown || !playlists.data || !rail.prefs.seeded) return;
    const ids = new Set(playlists.data.map((list) => list.id));
    const first = seen.current === null;
    const before = seen.current ?? new Set<string>();
    for (const id of ids) before.add(id);
    seen.current = before;
    const gone = rail.prefs.pins.filter((pin) => {
      const id = pin.search.id ?? '';
      return pin.kind === 'playlist' && !ids.has(id) && (first || before.has(id));
    });
    if (gone.length > 0) rail.drop(gone);
  }, [playlists.data, rail, shown]);

  const nameOf = (pin: RailPin) => {
    if (pin.kind === 'favorites') return t('collections.favorites');
    if (pin.kind === 'downloaded') return t('library.view.downloaded');
    if (pin.kind === 'localFiles') return t('localFiles.title');
    if (pin.kind === 'playlist') {
      return playlists.data?.find((list) => list.id === pin.search.id)?.name ?? pin.name;
    }
    return pin.name;
  };

  const pins = rail.pins;
  const menu = (pin: RailPin, index: number) => (event: React.MouseEvent) =>
    openContextMenu(event, [
      { kind: 'heading', label: nameOf(pin) },
      ...(index > 0
        ? [
            {
              label: t('rail.moveUp'),
              icon: ArrowUp,
              onSelect: () => rail.move(pin, pins[index - 1] ?? null),
            },
          ]
        : []),
      ...(index < pins.length - 1
        ? [
            {
              label: t('rail.moveDown'),
              icon: ArrowDown,
              onSelect: () => rail.move(pin, pins[index + 2] ?? null),
            },
          ]
        : []),
      { label: t('rail.unpin'), icon: PinOff, onSelect: () => rail.unpin(pin) },
      // A playlist's own items: play, merge, delete (M46.45).
      ...playlistItemsOf(pin),
    ]);
  const playlistItemsOf = (pin: RailPin): MenuItem[] => {
    const playlist =
      pin.kind === 'playlist'
        ? playlists.data?.find((list) => list.id === pin.search.id)
        : undefined;
    return playlist
      ? [{ kind: 'separator' }, ...playlistMenuItems(playlist, { pin: false }).slice(1)]
      : [];
  };
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    const from = pins.findIndex((pin) => pinKey(pin) === active.id);
    const to = pins.findIndex((pin) => pinKey(pin) === over.id);
    const moved = pins[from];
    if (!moved) return;
    // Dropped on a pin below: after it; above: before it.
    const before = to > from ? (pins[to + 1] ?? null) : (pins[to] ?? null);
    rail.move(moved, before);
  };

  return (
    <>
      {!narrow && (
        <div className="flex items-center justify-between px-4 pb-1">
          <h2 className="text-muted text-xs font-semibold tracking-wide uppercase">
            {t('rail.library')}
          </h2>
          <button
            type="button"
            aria-label={t('rail.order')}
            data-hint={t('rail.order')}
            onClick={(event) => setSortAnchor(sortAnchor ? null : event.currentTarget)}
            className="text-muted hover:text-foreground grid size-7 place-items-center rounded-full"
          >
            <ArrowDownUp aria-hidden size={15} />
          </button>
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
            {(['custom', 'played', 'opened'] as RailOrder[]).map((order) => (
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
        </Popover>
      )}
      <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
        <SortableContext items={pins.map(pinKey)} strategy={verticalListSortingStrategy}>
          <ul data-testid="rail-pins" className="flex flex-col gap-0.5">
            {pins.map((pin, index) => (
              <SortablePin
                key={pinKey(pin)}
                pin={pin}
                name={nameOf(pin)}
                playlist={
                  pin.kind === 'playlist'
                    ? playlists.data?.find((list) => list.id === pin.search.id)
                    : undefined
                }
                narrow={narrow}
                onMenu={menu(pin, index)}
              />
            ))}
          </ul>
        </SortableContext>
      </DndContext>
      {pins.length === 0 && !narrow && (
        <p className="text-muted px-4 py-2 text-xs">{t('rail.empty')}</p>
      )}
    </>
  );
};
