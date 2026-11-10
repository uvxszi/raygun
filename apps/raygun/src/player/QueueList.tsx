import type { QueueItem } from '@raygun/bindings';
import { defaultRangeExtractor, observeElementRect, useVirtualizer } from '@tanstack/react-virtual';
import type { Range } from '@tanstack/react-virtual';
import { GripVertical, ListMinus, ListStart, ListX, Play, Save, Trash2, X } from 'lucide-react';
import { memo, useCallback, useEffect, useLayoutEffect, useMemo, useRef, useState } from 'react';
import type {
  FC,
  KeyboardEvent as ReactKeyboardEvent,
  PointerEvent as ReactPointerEvent,
} from 'react';
import { useTranslation } from 'react-i18next';

import { fromQueueItem, useAddToPlaylist, useCreatePlaylist } from '../collections/useCollections';
import { ExplicitMark } from '../music/ExplicitMark';
import { tidyCredit } from '../music/artists';
import { Artwork } from '../library/Artwork';
import { songCover } from '../library/coverTargets';
import { useTrackMenuItems } from '../music/useTrackMenu';
import { MenuButton } from '../shell/MenuButton';
import type { MenuItem } from '../shell/contextMenuStore';
import { useRootPx } from './playerLayout';
import { sendControl } from './usePlayback';

/** A row's height, rem (rows are measured; this is the first guess). */
const ROW_REM = 3.5;
/** How close to the list's edge (px) a dragged row scrolls it, and how fast at most (px a frame). */
const EDGE_PX = 48;
const EDGE_SPEED = 14;
/** How far (share of the row) or how fast (px/ms) a swipe takes a row out of the queue. */
const SWIPE_SHARE = 0.35;
const SWIPE_SPEED = 0.6;

/** `ids` with the one at `from` moved to `to`. */
const moved = (ids: string[], from: number, to: number) => {
  const next = [...ids];
  const [id] = next.splice(from, 1);
  if (id !== undefined) next.splice(to, 0, id);
  return next;
};

/** Moves `id` to `to` in the queue: before the item that ends up after it, or to the end. */
const moveTo = (ids: string[], id: string, to: number) => {
  const order = moved(ids, ids.indexOf(id), to);
  void sendControl({ type: 'move', itemId: id, beforeItemId: order[to + 1] ?? null });
  return order;
};

/** A row being dragged by its grip. */
type Drag = {
  id: string;
  from: number;
  /** Where the pointer and the list's scroll were when it started, and the pointer now. */
  startY: number;
  startScroll: number;
  pointerY: number;
  /** How far the row has moved, px, scrolling included. */
  offset: number;
};

/** What a row can ask of its list; one object for the list's life, so rows don't re-render. */
type RowActions = {
  grip: (item: QueueItem, event: ReactPointerEvent<HTMLButtonElement>) => void;
  gripMove: (event: ReactPointerEvent) => void;
  gripEnd: (drop: boolean) => void;
  gripKey: (item: QueueItem, event: ReactKeyboardEvent) => void;
  playNext: (item: QueueItem) => void;
};

type QueueListProps = {
  /** What plays next, in play order. */
  items: QueueItem[];
  /** `sheet`: the full player's Up next; `panel`: the side panel's Queue. */
  variant: 'sheet' | 'panel';
};

/**
 * What plays next, as a list that holds any length (only the rows on screen exist): tap a row to
 * play it, drag its grip (or press the arrow keys on it) to move it, swipe it sideways or press
 * × to take it out, long-press or right-click for the rest.
 */
export const QueueList: FC<QueueListProps> = ({ items, variant }) => {
  const { t } = useTranslation();
  const rootPx = useRootPx();
  const scrollRef = useRef<HTMLDivElement>(null);
  const [drag, setDrag] = useState<Drag | null>(null);
  // Where a moved row went, until the queue that follows arrives (or a moment passes).
  const [optimistic, setOptimistic] = useState<{ order: string[]; base: QueueItem[] } | null>(null);
  const shown = useMemo(() => {
    if (!optimistic || optimistic.base !== items) return items;
    const byId = new Map(items.map((item) => [item.id, item]));
    return optimistic.order.flatMap((id) => byId.get(id) ?? []);
  }, [items, optimistic]);
  const ids = useMemo(() => shown.map((item) => item.id), [shown]);

  useEffect(() => {
    if (!optimistic) return;
    const timer = setTimeout(() => setOptimistic(null), 2000);
    return () => clearTimeout(timer);
  }, [optimistic]);

  const dragIndex = drag ? ids.indexOf(drag.id) : -1;
  const rangeExtractor = useCallback(
    (range: Range) => {
      const indexes = defaultRangeExtractor(range);
      // The row being dragged stays, even scrolled out of sight.
      if (dragIndex >= 0 && !indexes.includes(dragIndex)) {
        indexes.push(dragIndex);
        indexes.sort((first, second) => first - second);
      }
      return indexes;
    },
    [dragIndex],
  );

  const virtualizer = useVirtualizer({
    count: shown.length,
    getScrollElement: () => scrollRef.current,
    estimateSize: () => Math.round(ROW_REM * rootPx),
    getItemKey: (index) => shown[index]?.id ?? index,
    overscan: 8,
    rangeExtractor,
    // A list not laid out yet (or measured as nothing) still shows its first rows.
    observeElementRect: (instance, report) =>
      observeElementRect(instance, (rect) =>
        report(rect.height > 0 ? rect : { width: rect.width, height: window.innerHeight }),
      ),
  });

  /** Where a dragged row would land: the row under its middle. */
  const targetOf = useCallback(
    (state: Drag, count: number) => {
      const measured = virtualizer.measurementsCache[state.from];
      const center = (measured?.start ?? 0) + state.offset + (measured?.size ?? 0) / 2;
      const over = virtualizer.getVirtualItemForOffset(Math.max(0, center));
      return Math.min(count - 1, Math.max(0, over?.index ?? state.from));
    },
    [virtualizer],
  );
  const to = drag ? targetOf(drag, ids.length) : -1;
  const dragSize = drag ? (virtualizer.measurementsCache[drag.from]?.size ?? 0) : 0;

  // The handlers read the list as it is now; the rows keep the same handlers throughout.
  const latest = useRef({ ids, items, drag });
  useLayoutEffect(() => {
    latest.current = { ids, items, drag };
  });

  const actions = useMemo<RowActions>(() => {
    const update = (next: Drag) => {
      latest.current = { ...latest.current, drag: next };
      setDrag(next);
    };
    return {
      grip: (item, event) => {
        if (event.button !== 0) return;
        event.preventDefault();
        event.currentTarget.setPointerCapture?.(event.pointerId);
        update({
          id: item.id,
          from: latest.current.ids.indexOf(item.id),
          startY: event.clientY,
          startScroll: scrollRef.current?.scrollTop ?? 0,
          pointerY: event.clientY,
          offset: 0,
        });
      },
      gripMove: (event) => {
        const state = latest.current.drag;
        if (!state) return;
        const scroll = scrollRef.current?.scrollTop ?? 0;
        update({
          ...state,
          pointerY: event.clientY,
          offset: event.clientY - state.startY + scroll - state.startScroll,
        });
      },
      gripEnd: (drop) => {
        const { drag: state, ids: order, items: base } = latest.current;
        latest.current = { ...latest.current, drag: null };
        setDrag(null);
        if (!state || !drop) return;
        const index = targetOf(state, order.length);
        if (index !== state.from) setOptimistic({ order: moveTo(order, state.id, index), base });
      },
      gripKey: (item, event) => {
        const { ids: order, items: base } = latest.current;
        const index = order.indexOf(item.id);
        const next =
          event.key === 'ArrowUp'
            ? index - 1
            : event.key === 'ArrowDown'
              ? index + 1
              : event.key === 'Home'
                ? 0
                : event.key === 'End'
                  ? order.length - 1
                  : null;
        if (next === null) return;
        event.preventDefault();
        if (next < 0 || next >= order.length || next === index) return;
        setOptimistic({ order: moveTo(order, item.id, next), base });
        virtualizer.scrollToIndex(next);
      },
      playNext: (item) => {
        const { ids: order, items: base } = latest.current;
        if (order.indexOf(item.id) > 0) setOptimistic({ order: moveTo(order, item.id, 0), base });
      },
    };
  }, [targetOf, virtualizer]);

  // Scrolls the list while a row is held near its top or bottom edge.
  const dragging = drag?.id ?? null;
  useEffect(() => {
    if (!dragging) return;
    let frame = requestAnimationFrame(function step() {
      const list = scrollRef.current;
      const state = latest.current.drag;
      if (list && state) {
        const box = list.getBoundingClientRect();
        const above = box.top + EDGE_PX - state.pointerY;
        const below = state.pointerY - (box.bottom - EDGE_PX);
        const speed =
          above > 0
            ? -Math.min(EDGE_SPEED, above / 3)
            : below > 0
              ? Math.min(EDGE_SPEED, below / 3)
              : 0;
        if (speed !== 0) {
          list.scrollTop += speed;
          const next = {
            ...state,
            offset: state.pointerY - state.startY + list.scrollTop - state.startScroll,
          };
          latest.current = { ...latest.current, drag: next };
          setDrag(next);
        }
      }
      frame = requestAnimationFrame(step);
    });
    return () => cancelAnimationFrame(frame);
  }, [dragging]);

  if (items.length === 0) {
    return <p className="text-muted px-2 py-2 text-sm">{t('player.queueEmpty')}</p>;
  }

  return (
    <div
      ref={scrollRef}
      data-testid={variant === 'sheet' ? 'up-next' : 'queue-panel-list'}
      className="no-scrollbar min-h-0 flex-1 overflow-y-auto overscroll-contain"
    >
      <ul
        aria-label={t('player.upNext')}
        className="relative w-full"
        style={{ height: virtualizer.getTotalSize() }}
      >
        {virtualizer.getVirtualItems().map((row) => {
          const item = shown[row.index];
          if (!item) return null;
          const dragged = drag?.id === item.id;
          // Rows between where the dragged one was and where it would go make room for it.
          const shift =
            !drag || dragged
              ? 0
              : drag.from < to && row.index > drag.from && row.index <= to
                ? -dragSize
                : to < drag.from && row.index >= to && row.index < drag.from
                  ? dragSize
                  : 0;
          return (
            <li
              key={row.key}
              ref={virtualizer.measureElement}
              data-index={row.index}
              className={`absolute inset-x-0 top-0 ${
                dragged ? 'z-10' : drag ? 'transition-transform duration-150' : ''
              }`}
              style={{
                transform: `translateY(${row.start + (dragged ? (drag?.offset ?? 0) : shift)}px)`,
              }}
            >
              <QueueRow item={item} dragged={dragged} first={row.index === 0} actions={actions} />
            </li>
          );
        })}
      </ul>
    </div>
  );
};

/** Save the queue as a playlist, and clear what's next (asked first): the list's actions. */
export const QueueActions: FC<{ items: QueueItem[]; current: QueueItem | undefined }> = ({
  items,
  current,
}) => {
  const { t } = useTranslation();
  const createPlaylist = useCreatePlaylist();
  const addToPlaylist = useAddToPlaylist();
  const [saved, setSaved] = useState<string | null>(null);
  const [confirming, setConfirming] = useState(false);
  const saving = createPlaylist.isPending || addToPlaylist.isPending;
  // The whole queue, playing song first, as a new playlist (M46.8).
  const saveQueue = () => {
    const name = t('player.queueSavedName', { date: new Date().toLocaleString() });
    const tracks = [...(current ? [current] : []), ...items].map(fromQueueItem);
    createPlaylist.mutate(name, {
      onSuccess: (playlist) =>
        addToPlaylist.mutate({ id: playlist.id, tracks }, { onSuccess: () => setSaved(name) }),
    });
  };
  const button =
    'text-muted hover:text-foreground flex items-center gap-1 rounded-full px-2 py-1 text-sm disabled:opacity-50';

  return (
    <div className="flex flex-col gap-1">
      <div className="flex flex-wrap items-center justify-end gap-x-1">
        <button
          type="button"
          disabled={saving || (!current && items.length === 0)}
          onClick={saveQueue}
          className={button}
        >
          <Save aria-hidden size={14} />
          {t('player.saveQueue')}
        </button>
        {items.length > 0 && !confirming && (
          <button type="button" onClick={() => setConfirming(true)} className={button}>
            <ListX aria-hidden size={14} />
            {t('player.clearQueue')}
          </button>
        )}
      </div>
      {confirming && items.length > 0 && (
        <div
          role="alertdialog"
          aria-label={t('player.clearQueueAsk', { count: items.length })}
          className="bg-surface flex flex-wrap items-center justify-end gap-2 rounded-xl px-3 py-2"
        >
          <p className="min-w-0 flex-1 text-sm">
            {t('player.clearQueueAsk', { count: items.length })}
          </p>
          <button
            type="button"
            onClick={() => setConfirming(false)}
            className="hover:bg-background rounded-full px-3 py-1 text-sm"
          >
            {t('common.cancel')}
          </button>
          <button
            type="button"
            onClick={() => {
              setConfirming(false);
              void sendControl({ type: 'clearUpcoming' });
            }}
            className="bg-primary text-background rounded-full px-3 py-1 text-sm font-semibold"
          >
            {t('player.clearQueueConfirm')}
          </button>
        </div>
      )}
      {saved && (
        <p role="status" className="text-primary px-2 text-xs">
          {t('player.queueSaved', { name: saved })}
        </p>
      )}
    </div>
  );
};

/** A swipe on a row: where it started, and how far it has gone once it's sideways. */
type Swipe = { x: number; y: number; at: number; dx: number; sideways: boolean | null };

/**
 * One queued song. Its artist is plain text: the row plays the song, and its menu has Go to the
 * artist. On touch it slides sideways to come out of the queue.
 */
const QueueRow: FC<{ item: QueueItem; dragged: boolean; first: boolean; actions: RowActions }> =
  memo(function QueueRow({ item, dragged, first, actions }) {
    const { t } = useTranslation();
    const trackMenu = useTrackMenuItems();
    const swipeRef = useRef<Swipe | null>(null);
    const swipedRef = useRef(false);
    const rowRef = useRef<HTMLDivElement>(null);
    const [dx, setDx] = useState(0);
    const [swiping, setSwiping] = useState(false);
    const [leaving, setLeaving] = useState(0);

    const play = () => void sendControl({ type: 'playItem', itemId: item.id });
    const remove = () => void sendControl({ type: 'remove', itemId: item.id });

    const menu = (): MenuItem[] => {
      // The song's own menu, less Play next and Add to queue, which would queue it again.
      const again = new Set([t('menu.playNext'), t('menu.addToQueue')]);
      const own: MenuItem[] = [
        { kind: 'heading', label: item.title, subtitle: item.artist },
        { label: t('menu.play'), icon: Play, onSelect: play },
        ...(first
          ? []
          : [
              {
                label: t('menu.playNext'),
                icon: ListStart,
                onSelect: () => actions.playNext(item),
              },
            ]),
        { label: t('player.removeFromQueueMenu'), icon: ListMinus, onSelect: remove },
        { kind: 'separator' },
      ];
      const song = trackMenu(
        fromQueueItem(item),
        {},
        { x: window.innerWidth / 2, y: window.innerHeight / 2 },
      );
      return [
        ...own,
        ...song.filter(
          (entry) =>
            entry.kind !== 'heading' && !(entry.kind !== 'separator' && again.has(entry.label)),
        ),
      ];
    };

    const onPointerDown = (event: ReactPointerEvent) => {
      // Mice have ×; the grip drags instead.
      if (event.pointerType === 'mouse' || (event.target as Element).closest('[data-grip]')) {
        return;
      }
      swipeRef.current = {
        x: event.clientX,
        y: event.clientY,
        at: event.timeStamp,
        dx: 0,
        sideways: null,
      };
      swipedRef.current = false;
    };
    const onPointerMove = (event: ReactPointerEvent) => {
      const swipe = swipeRef.current;
      if (!swipe) return;
      const moveX = event.clientX - swipe.x;
      const moveY = event.clientY - swipe.y;
      if (swipe.sideways === null) {
        // Up and down is the list scrolling; only a clearly sideways move is a swipe.
        if (Math.abs(moveY) > 10) swipe.sideways = false;
        else if (Math.abs(moveX) > 10 && Math.abs(moveX) > Math.abs(moveY) * 1.5) {
          swipe.sideways = true;
          swipedRef.current = true;
          setSwiping(true);
          (event.currentTarget as HTMLElement).setPointerCapture?.(event.pointerId);
        }
      }
      if (swipe.sideways) {
        swipe.dx = moveX;
        setDx(moveX);
      }
    };
    const onPointerUp = (event: ReactPointerEvent) => {
      const swipe = swipeRef.current;
      swipeRef.current = null;
      if (!swipe?.sideways) return;
      setSwiping(false);
      const width = rowRef.current?.offsetWidth ?? 320;
      const speed = Math.abs(swipe.dx) / Math.max(1, event.timeStamp - swipe.at);
      if (Math.abs(swipe.dx) > width * SWIPE_SHARE || speed > SWIPE_SPEED) {
        // It slides out, then leaves the queue.
        setLeaving(swipe.dx < 0 ? -width : width);
        window.setTimeout(remove, 150);
      } else {
        setDx(0);
      }
    };
    const offsetX = leaving || dx;

    return (
      <div
        ref={rowRef}
        data-testid="queue-row"
        className={`group relative overflow-hidden rounded-lg ${
          dragged ? 'bg-surface shadow-lg shadow-black/30' : ''
        }`}
      >
        {offsetX !== 0 && (
          <div
            aria-hidden
            className={`absolute inset-0 flex items-center bg-red-500/85 px-5 text-white ${
              offsetX < 0 ? 'justify-end' : 'justify-start'
            }`}
          >
            <Trash2 size={18} />
          </div>
        )}
        <div
          onPointerDown={onPointerDown}
          onPointerMove={onPointerMove}
          onPointerUp={onPointerUp}
          onPointerCancel={() => {
            swipeRef.current = null;
            setSwiping(false);
            setDx(0);
          }}
          onClickCapture={(event) => {
            // A swipe isn't a tap on the song.
            if (swipedRef.current) {
              event.preventDefault();
              event.stopPropagation();
              swipedRef.current = false;
            }
          }}
          style={offsetX ? { transform: `translateX(${offsetX}px)` } : undefined}
          className={`hover:bg-foreground/5 relative flex touch-pan-y items-center gap-0.5 rounded-lg pr-0.5 ${
            offsetX ? 'bg-surface' : ''
          } ${swiping ? '' : 'transition-transform duration-150'}`}
        >
          <button
            type="button"
            aria-label={item.title}
            onClick={play}
            className="flex min-w-0 flex-1 items-center gap-3 py-1.5 pl-2 text-left @max-xs/panel:gap-2.5"
          >
            <Artwork
              url={item.artworkUrl}
              fallback={songCover(item)}
              className="size-10 shrink-0 rounded-md @max-xs/panel:size-9"
              iconSize={18}
            />
            <span className="title-gap flex min-w-0 flex-col">
              <span className="flex min-w-0 items-center gap-1.5">
                <span className="truncate text-sm leading-tight font-medium">{item.title}</span>
                {item.explicit && <ExplicitMark />}
              </span>
              {item.artist && (
                <span className="text-muted truncate text-xs leading-tight">
                  {tidyCredit(item.artist)}
                </span>
              )}
            </span>
          </button>
          <MenuButton
            label={t('menu.options', { name: item.title })}
            items={menu}
            className="text-muted hover:text-foreground grid size-9 shrink-0 place-items-center rounded-full pointer-fine:opacity-0 pointer-fine:group-hover:opacity-100 pointer-fine:focus-visible:opacity-100"
            iconSize={16}
          />
          <button
            type="button"
            aria-label={t('player.removeFromQueue', { title: item.title })}
            title={t('player.removeFromQueueMenu')}
            onClick={remove}
            // In a narrow side panel it takes room only under the pointer or with focus.
            className="text-muted hover:text-foreground grid size-9 shrink-0 place-items-center overflow-hidden rounded-full pointer-fine:opacity-0 pointer-fine:group-hover:opacity-100 pointer-fine:focus-visible:opacity-100 @max-xs/panel:pointer-fine:w-0 @max-xs/panel:pointer-fine:group-hover:w-9 @max-xs/panel:pointer-fine:focus-visible:w-9"
          >
            <X aria-hidden size={16} />
          </button>
          <button
            type="button"
            data-grip
            aria-label={t('player.moveInQueue', { title: item.title })}
            title={t('player.moveHint')}
            onPointerDown={(event) => actions.grip(item, event)}
            onPointerMove={actions.gripMove}
            onPointerUp={() => actions.gripEnd(true)}
            onPointerCancel={() => actions.gripEnd(false)}
            onKeyDown={(event) => actions.gripKey(item, event)}
            className={`text-muted hover:text-foreground grid size-9 shrink-0 touch-none place-items-center rounded-full @max-xs/panel:w-8 ${
              dragged ? 'cursor-grabbing' : 'cursor-grab'
            }`}
          >
            <GripVertical aria-hidden size={18} />
          </button>
        </div>
      </div>
    );
  });
