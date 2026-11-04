import {
  DndContext,
  KeyboardSensor,
  PointerSensor,
  closestCenter,
  useSensor,
  useSensors,
} from '@dnd-kit/core';
import type { DragEndEvent } from '@dnd-kit/core';
import {
  SortableContext,
  arrayMove,
  horizontalListSortingStrategy,
  sortableKeyboardCoordinates,
  useSortable,
  verticalListSortingStrategy,
} from '@dnd-kit/sortable';
import { CSS } from '@dnd-kit/utilities';
import type { TrackColumnId, TrackSortKey } from '@raygun/bindings';
import {
  ArrowDown,
  ArrowUp,
  AudioLines,
  Check,
  ChevronDown,
  CircleSlash,
  Clock3,
  Download,
  GripVertical,
  Heart,
  HeartOff,
  List,
  ListChecks,
  ListEnd,
  ListStart,
  Pencil,
  Play,
  Rows3,
  Search,
  Trash2,
  X,
} from 'lucide-react';
import { useMemo, useRef, useState } from 'react';
import type { FC, MouseEvent, PointerEvent as ReactPointerEvent, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';
import { commands } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';

import { SaveButton } from '../collections/SaveButton';
import { savesIn, useSavedSongs } from '../collections/savedSongs';
import { COLLECTIONS } from '../collections/useCollections';
import { TrackMoreButton } from '../collections/TrackMoreButton';
import { unwrapResult } from '../lib/commandResult';
import { Artwork } from '../library/Artwork';
import { formatTime } from '../player/formatTime';
import { Sheet } from '../shell/Sheet';
import { TextField } from '../shell/TextField';
import { useCoarsePointer, useWideScreen } from '../shell/useWideScreen';
import { ArtistLinks } from './ArtistLinks';
import {
  COLUMN_TRACKS,
  SORT_KEYS,
  dragOrder,
  filterTracks,
  sortTracks,
  useTableView,
} from './tableView';
import { useTagsFor } from './useDiscovery';
import { downloadSongs, useDownloadedKeys, useDownloadingKeys } from './useSongDownloads';
import type { TableReorder, TableTrack, TableView, TrackTableKind } from './tableView';
import { useSourcePickerStore } from './sourcePickerStore';
import { useTrackLengths } from './useTrackLengths';
import { useUnavailable } from './useUnavailable';
import { useTrackMenu } from './useTrackMenu';
import { SongMarks } from './SongMarks';
import { ExplicitMark } from './ExplicitMark';
import { NowPlayingBars } from '../player/NowPlayingBars';
import { usePlayingMatch } from '../player/usePlayingMatch';
import { songCover } from '../library/coverTargets';
import { songReveal } from './reveal';

const relative = new Intl.RelativeTimeFormat(undefined, { numeric: 'auto' });

/** "6 days ago", "2 weeks ago": when a song was saved. */
const ago = (at: number | null) => {
  if (!at) return '';
  const days = Math.round((at - Date.now()) / 86_400_000);
  if (days > -7) return relative.format(days, 'day');
  if (days > -60) return relative.format(Math.round(days / 7), 'week');
  if (days > -730) return relative.format(Math.round(days / 30), 'month');
  return relative.format(Math.round(days / 365), 'year');
};

/** A column's narrowest and widest, in px. */
const MIN_COLUMN = 40;
const MAX_COLUMN = 800;
const clampColumn = (width: number) =>
  Math.round(Math.min(MAX_COLUMN, Math.max(MIN_COLUMN, width)));

/**
 * A header cell: sorts on click, drags to reorder, and the divider on its left edge drags to
 * resize it. The columns to its right keep their widths, so the divider follows the pointer.
 */
const HeaderCell: FC<{
  id: TrackColumnId;
  label: string;
  sorted: 'asc' | 'desc' | null;
  onSort: () => void;
  onResizeStart: (event: ReactPointerEvent) => void;
  onResizeBy: (by: number) => void;
}> = ({ id, label, sorted, onSort, onResizeStart, onResizeBy }) => {
  const { t } = useTranslation();
  const { attributes, listeners, setNodeRef, transform, transition, isDragging } = useSortable({
    id,
  });
  return (
    <div
      ref={setNodeRef}
      data-column-id={id}
      role="columnheader"
      aria-label={label}
      aria-sort={sorted === 'asc' ? 'ascending' : sorted === 'desc' ? 'descending' : undefined}
      style={{ transform: CSS.Translate.toString(transform), transition }}
      className={`group relative flex min-w-0 items-center ${isDragging ? 'z-10 opacity-70' : ''}`}
    >
      <button
        type="button"
        {...attributes}
        {...listeners}
        onClick={onSort}
        className={`flex min-w-0 items-center gap-1 truncate text-left ${sorted ? 'text-primary' : 'hover:text-foreground'}`}
      >
        {id === 'duration' ? (
          <Clock3 aria-label={label} size={14} />
        ) : (
          <span className="truncate">{label}</span>
        )}
        {sorted === 'asc' && <ArrowUp aria-hidden size={12} />}
        {sorted === 'desc' && <ArrowDown aria-hidden size={12} />}
      </button>
      <span
        role="separator"
        tabIndex={0}
        aria-orientation="vertical"
        aria-label={t('tracks.resizeColumn', { name: label })}
        onPointerDown={(event) => {
          event.stopPropagation();
          event.preventDefault();
          onResizeStart(event);
        }}
        onKeyDown={(event) => {
          // The divider sits on the left: moving it left makes the column wider.
          if (event.key === 'ArrowLeft') onResizeBy(16);
          else if (event.key === 'ArrowRight') onResizeBy(-16);
          else return;
          event.preventDefault();
        }}
        className="absolute inset-y-0 -left-3 w-3 cursor-col-resize opacity-0 outline-none group-hover:opacity-100 after:absolute after:inset-y-1 after:left-1.5 after:w-px after:bg-white/20 focus-visible:opacity-100"
      />
    </div>
  );
};

type RowSortable = ReturnType<typeof useSortable>;

/** A row's drag state (M46.44), handed to the row it wraps; null when rows don't move. */
const SortableRow: FC<{
  id: string;
  enabled: boolean;
  children: (sortable: RowSortable | null) => ReactNode;
}> = ({ id, enabled, children }) => {
  const sortable = useSortable({ id, disabled: !enabled });
  return children(enabled ? sortable : null);
};

type TrackTableProps = {
  kind: TrackTableKind;
  tracks: TableTrack[];
  /** Plays `ordered` (the table's shown order) from `index`. */
  onPlay: (ordered: TableTrack[], index: number) => void;
  /** What "custom" order is called here ("Custom order", "Recently played"...). */
  customLabel?: string;
  /** Takes rows out of this list (a playlist, Favorites). */
  onRemove?: (rows: TableTrack[]) => void;
  /**
   * Rows keep the order they're dragged into (M46.44). They move only in edit mode, which the
   * table's Edit/Done button turns on (M46.49).
   */
  onReorder?: (change: TableReorder) => void;
  /** Edit mode, when the page holds it (a playlist's Edit sheet turns it on too). */
  editing?: boolean;
  onEditingChange?: (editing: boolean) => void;
  /** Replaces a row's Plus and more button (a playlist's edit controls). */
  actions?: (track: TableTrack, index: number) => ReactNode;
  toolbar?: ReactNode;
};

/** A column's width when the window can't measure it (tests). */
const COLUMN_FALLBACK = 120;

/** Tag chips shown above a table, the most common first. */
const TAG_CHIPS = 12;

/**
 * Songs as a table (M46.8), Spotify's: sort by clicking a column or from Sort by, filter, list
 * or compact, columns to show, drag and resize, and several songs at once with Ctrl and Shift.
 * Playing plays the table in the order it shows.
 */
export const TrackTable: FC<TrackTableProps> = ({
  kind,
  tracks: givenTracks,
  onPlay,
  customLabel,
  onRemove,
  onReorder,
  editing: givenEditing,
  onEditingChange,
  actions,
  toolbar,
}) => {
  const { t } = useTranslation();
  // Songs whose source sent no length get the one Raygun learned (M46.26).
  const tracks = useTrackLengths(givenTracks);
  const wide = useWideScreen();
  const touch = useCoarsePointer();
  const menu = useTrackMenu();
  const playingMatch = usePlayingMatch();
  const { view, set } = useTableView(kind);
  const [filter, setFilter] = useState('');
  const [filtering, setFiltering] = useState(false);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  // Rows have grips and move (by pointer or keyboard) only in edit mode (M46.49).
  const [ownEditing, setOwnEditing] = useState(false);
  const editing = Boolean(onReorder) && (givenEditing ?? ownEditing);
  const setEditing = onEditingChange ?? setOwnEditing;
  const reorderable = editing && Boolean(onReorder);
  const lastPicked = useRef<number | null>(null);
  const [sortAnchor, setSortAnchor] = useState<HTMLElement | null>(null);
  const [columnsAnchor, setColumnsAnchor] = useState<HTMLElement | null>(null);
  const sensors = useSensors(useSensor(PointerSensor, { activationConstraint: { distance: 6 } }));
  // Rows move only by their grip, so a short drag is enough; the keyboard works too.
  const rowSensors = useSensors(
    useSensor(PointerSensor, { activationConstraint: { distance: 3 } }),
    useSensor(KeyboardSensor, { coordinateGetter: sortableKeyboardCoordinates }),
  );

  // Songs saved into the app are marked (M46.10); a selection's Download skips them (M46.46).
  const downloadedKeys = useDownloadedKeys();
  const downloadingKeys = useDownloadingKeys();
  // Genres and moods found for these songs (M46.9): chips filter the table by one.
  const trackKeys = useMemo(() => [...new Set(tracks.map((track) => track.trackKey))], [tracks]);
  const tagsFor = useTagsFor(trackKeys);
  // Songs no source had when imported (M46.31).
  const unavailable = useUnavailable(trackKeys);
  const [tag, setTag] = useState<string | null>(null);
  const tagChips = useMemo(() => {
    const counts = new Map<string, number>();
    for (const track of tracks) {
      for (const found of tagsFor.data?.get(track.trackKey) ?? []) {
        counts.set(found.name, (counts.get(found.name) ?? 0) + 1);
      }
    }
    return [...counts.entries()]
      .sort((a, b) => b[1] - a[1] || a[0].localeCompare(b[0]))
      .slice(0, TAG_CHIPS)
      .map(([name]) => name);
  }, [tracks, tagsFor.data]);
  const shown = useMemo(
    () =>
      filterTracks(sortTracks(tracks, view), filter).filter(
        (track) =>
          !tag || (tagsFor.data?.get(track.trackKey) ?? []).some((found) => found.name === tag),
      ),
    [tracks, view, filter, tag, tagsFor.data],
  );
  const columns = view.columns.filter((column) => column.visible);
  const sortLabel = (key: TrackSortKey) =>
    key === 'custom' ? (customLabel ?? t('tracks.sort.custom')) : t(`tracks.sort.${key}`);
  const sortBy = (key: TrackSortKey) =>
    set({
      sort: key,
      descending: view.sort === key ? !view.descending : key === 'added' || key === 'plays',
    });
  // Widths while a divider is dragged; saved when it's let go.
  const headerRow = useRef<HTMLDivElement>(null);
  const [liveWidths, setLiveWidths] = useState<Partial<Record<TrackColumnId, number>> | null>(null);
  // The first column holds the grip too when rows move.
  const template = `${reorderable ? '3.25rem' : '2.5rem'} minmax(12rem, 1.6fr) ${columns
    .map((column) => {
      const width = liveWidths?.[column.id] ?? column.width;
      return width > 0 ? `${width}px` : COLUMN_TRACKS[column.id];
    })
    .join(' ')} 5.5rem`;
  /** Every shown column's width now, so only the title column gives or takes space. */
  const measured = () => {
    const widths: Partial<Record<TrackColumnId, number>> = {};
    for (const cell of headerRow.current?.querySelectorAll<HTMLElement>('[data-column-id]') ?? []) {
      const id = cell.dataset.columnId as TrackColumnId;
      widths[id] = Math.round(cell.getBoundingClientRect().width) || COLUMN_FALLBACK;
    }
    return widths;
  };
  const saveWidths = (widths: Partial<Record<TrackColumnId, number>>) =>
    set({
      columns: view.columns.map((column) => {
        const width = widths[column.id];
        return width === undefined ? column : { ...column, width };
      }),
    });
  const startResize = (id: TrackColumnId, event: ReactPointerEvent) => {
    const start = event.clientX;
    const widths = measured();
    const from = widths[id] ?? COLUMN_FALLBACK;
    const at = (x: number) => ({ ...widths, [id]: clampColumn(from - (x - start)) });
    const move = (moved: PointerEvent) => setLiveWidths(at(moved.clientX));
    const up = (ended: PointerEvent) => {
      window.removeEventListener('pointermove', move);
      window.removeEventListener('pointerup', up);
      setLiveWidths(null);
      saveWidths(at(ended.clientX));
    };
    setLiveWidths(widths);
    window.addEventListener('pointermove', move);
    window.addEventListener('pointerup', up);
  };
  const resizeBy = (id: TrackColumnId, by: number) => {
    const widths = measured();
    saveWidths({ ...widths, [id]: clampColumn((widths[id] ?? COLUMN_FALLBACK) + by) });
  };
  const chosen = useMemo(
    () => shown.filter((track) => selected.has(track.rowKey)),
    [shown, selected],
  );
  // A selection all in Favorites offers to take them out, else to add them (M46.49).
  const savedSongs = useSavedSongs().data;
  const allFavorite = useMemo(
    () =>
      chosen.length > 0 &&
      (kind === 'favorites' ||
        (savedSongs !== undefined &&
          chosen.every((track) => savesIn(savedSongs, track, track.trackKey).favorite))),
    [chosen, kind, savedSongs],
  );
  const client = useQueryClient();
  const favorite = useMutation({
    mutationFn: ({ rows, on }: { rows: TableTrack[]; on: boolean }) =>
      unwrapResult(
        commands.favoritesSetMany(
          rows.map((row) => row.input),
          on,
        ),
      ),
    onSettled: () => client.invalidateQueries({ queryKey: [COLLECTIONS] }),
  });
  // Saved or on their way already: left out of the selection's Download (M46.46).
  const toDownload = chosen.filter(
    (track) => !downloadedKeys.has(track.trackKey) && !downloadingKeys.has(track.trackKey),
  );
  const allSelected = shown.length > 0 && shown.every((track) => selected.has(track.rowKey));
  const transcribe = useMutation({
    mutationFn: (rows: TableTrack[]) =>
      unwrapResult(
        commands.lyricsTranscribeMany(
          rows.map((row) => ({
            title: row.title,
            artist: row.artist,
            album: row.album,
            durationMs: row.durationMs,
          })),
        ),
      ),
  });

  const pick = (event: MouseEvent, index: number) => {
    const key = shown[index]?.rowKey;
    if (!key) return;
    // Read now: the update runs later, after `lastPicked` has moved to this row.
    const anchor = lastPicked.current;
    setSelected((current) => {
      const next = new Set(event.ctrlKey || event.metaKey || event.shiftKey ? current : []);
      if (event.shiftKey && anchor !== null) {
        const [from, to] = [Math.min(anchor, index), Math.max(anchor, index)];
        for (const track of shown.slice(from, to + 1)) next.add(track.rowKey);
      } else if (next.has(key) && (event.ctrlKey || event.metaKey)) {
        next.delete(key);
      } else {
        next.add(key);
      }
      return next;
    });
    lastPicked.current = index;
  };
  const queue = (next: boolean) =>
    void unwrapResult(
      commands.playbackQueueTracks(
        chosen.map((track) => track.input),
        next,
      ),
    );
  const onDragEnd = ({ active, over }: DragEndEvent) => {
    if (!over || active.id === over.id) return;
    const from = view.columns.findIndex((column) => column.id === active.id);
    const to = view.columns.findIndex((column) => column.id === over.id);
    set({ columns: arrayMove(view.columns, from, to) });
  };
  // A dropped row (M46.44): what's shown becomes the order, whatever the sort was.
  const onRowDragEnd = ({ active, over }: DragEndEvent) => {
    if (!onReorder || !reorderable || !over || active.id === over.id) return;
    const from = shown.findIndex((track) => track.rowKey === active.id);
    const to = shown.findIndex((track) => track.rowKey === over.id);
    const moved = shown[from];
    if (!moved || to < 0) return;
    const order = dragOrder(sortTracks(tracks, view), shown, from, to);
    const at = order.findIndex((track) => track.rowKey === moved.rowKey);
    const whole = view.sort !== 'custom' || view.descending;
    onReorder({ order, moved, before: order[at + 1] ?? null, whole });
    if (whole) set({ sort: 'custom', descending: false });
  };
  const selectAll = () => setSelected(new Set(shown.map((track) => track.rowKey)));
  // The toolbar's Select all, pressed again, clears the selection and closes its bar (M46.46).
  const toggleAll = () => (allSelected ? setSelected(new Set()) : selectAll());
  const sortOf = (key: TrackSortKey) =>
    view.sort === key ? (view.descending ? 'desc' : 'asc') : null;

  const sortSheet = sortAnchor && (
    <Sheet
      title={t('tracks.sortBy')}
      anchor={sortAnchor}
      popoverWidth={240}
      onClose={() => setSortAnchor(null)}
    >
      <div role="radiogroup" aria-label={t('tracks.sortBy')} className="-mx-2 flex flex-col">
        {SORT_KEYS.map((key) => (
          <button
            key={key}
            type="button"
            role="radio"
            aria-checked={view.sort === key}
            onClick={() => sortBy(key)}
            className={`hover:bg-background flex items-center justify-between rounded-lg px-3 py-2 text-sm ${view.sort === key ? 'text-primary' : ''}`}
          >
            {sortLabel(key)}
            {view.sort === key &&
              (view.descending ? (
                <ArrowDown aria-hidden size={14} />
              ) : (
                <ArrowUp aria-hidden size={14} />
              ))}
          </button>
        ))}
        <h3 className="text-muted px-3 pt-3 pb-1 text-xs font-semibold">{t('tracks.viewAs')}</h3>
        {(['compact', 'list'] as const).map((option) => (
          <button
            key={option}
            type="button"
            role="radio"
            aria-checked={view.view === option}
            onClick={() => set({ view: option })}
            className={`hover:bg-background flex items-center gap-3 rounded-lg px-3 py-2 text-sm ${view.view === option ? 'text-primary' : ''}`}
          >
            {option === 'compact' ? (
              <Rows3 aria-hidden size={16} />
            ) : (
              <List aria-hidden size={16} />
            )}
            <span className="flex-1 text-left">{t(`tracks.view.${option}`)}</span>
            {view.view === option && <Check aria-hidden size={14} />}
          </button>
        ))}
      </div>
    </Sheet>
  );
  const columnsSheet = columnsAnchor && (
    <Sheet
      title={t('tracks.columns')}
      anchor={columnsAnchor}
      popoverWidth={220}
      onClose={() => setColumnsAnchor(null)}
    >
      <div className="-mx-2 flex flex-col">
        {view.columns.map((column) => (
          <button
            key={column.id}
            type="button"
            role="checkbox"
            aria-checked={column.visible}
            onClick={() =>
              set({
                columns: view.columns.map((other) =>
                  other.id === column.id ? { ...other, visible: !other.visible } : other,
                ),
              })
            }
            className={`hover:bg-background flex items-center justify-between rounded-lg px-3 py-2 text-sm ${column.visible ? 'text-primary' : ''}`}
          >
            {t(`tracks.column.${column.id}`)}
            {column.visible && <Check aria-hidden size={14} />}
          </button>
        ))}
        <button
          type="button"
          onClick={() => set({ columns: view.columns.map((column) => ({ ...column, width: 0 })) })}
          className="text-muted hover:bg-background rounded-lg px-3 py-2 text-left text-xs"
        >
          {t('tracks.resetWidths')}
        </button>
      </div>
    </Sheet>
  );

  return (
    <div data-testid="track-table" data-kind={kind} className="flex flex-col gap-2">
      {tagChips.length > 0 && (
        <div
          role="group"
          aria-label={t('discover.filterByTag')}
          className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
        >
          {[null, ...tagChips].map((name) => (
            <button
              key={name ?? ''}
              type="button"
              aria-pressed={tag === name}
              onClick={() => setTag(tag === name ? null : name)}
              className={`shrink-0 rounded-full px-3 py-1 text-sm ${
                tag === name ? 'bg-foreground text-background font-semibold' : 'bg-surface'
              }`}
            >
              {name ?? t('discover.allTags')}
            </button>
          ))}
        </div>
      )}
      <div className="flex flex-wrap items-center gap-2">
        {toolbar}
        <div className="ml-auto flex items-center gap-1">
          {onReorder && (
            <button
              type="button"
              aria-pressed={editing}
              aria-label={editing ? t('tracks.doneEditing') : t('tracks.editOrderLabel')}
              onClick={() => setEditing(!editing)}
              className={`flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm ${
                editing
                  ? 'bg-foreground text-background font-semibold'
                  : 'text-muted hover:text-foreground'
              }`}
            >
              {editing ? <Check aria-hidden size={16} /> : <Pencil aria-hidden size={16} />}
              {editing ? t('tracks.doneEditing') : t('tracks.editOrder')}
            </button>
          )}
          <button
            type="button"
            aria-label={t('tracks.selectAll')}
            aria-pressed={allSelected}
            title={t(allSelected ? 'tracks.clearSelection' : 'tracks.selectAll')}
            disabled={shown.length === 0}
            onClick={toggleAll}
            className={`grid size-9 place-items-center rounded-full disabled:opacity-30 ${allSelected ? 'text-primary' : 'text-muted hover:text-foreground'}`}
          >
            <ListChecks aria-hidden size={18} />
          </button>
          {filtering || filter ? (
            <TextField
              autoFocus
              type="search"
              value={filter}
              onChange={setFilter}
              onBlur={() => setFiltering(false)}
              icon={Search}
              iconSize={14}
              autoComplete="off"
              aria-label={t('tracks.filter')}
              placeholder={t('tracks.filter')}
              className="bg-surface rounded-full py-0.5 pr-0.5 pl-3"
              inputClassName="w-40 py-1 text-sm"
            />
          ) : (
            <button
              type="button"
              aria-label={t('tracks.filter')}
              onClick={() => setFiltering(true)}
              className="text-muted hover:text-foreground grid size-9 place-items-center rounded-full"
            >
              <Search aria-hidden size={18} />
            </button>
          )}
          <button
            type="button"
            aria-haspopup="dialog"
            aria-expanded={sortAnchor !== null}
            aria-label={t('tracks.sortButton', { sort: sortLabel(view.sort) })}
            onClick={(event) => setSortAnchor(sortAnchor ? null : event.currentTarget)}
            className="text-muted hover:text-foreground flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
          >
            {sortLabel(view.sort)}
            {view.view === 'compact' ? (
              <Rows3 aria-hidden size={16} />
            ) : (
              <List aria-hidden size={16} />
            )}
          </button>
        </div>
      </div>
      {chosen.length > 0 && (
        <div
          role="toolbar"
          aria-label={t('tracks.selection')}
          className="bg-surface motion-pop sticky top-0 z-10 flex flex-wrap items-center gap-1 rounded-2xl px-3 py-2 shadow-lg shadow-black/20"
        >
          <span className="flex-1 text-sm font-medium">
            {t('tracks.selected', { count: chosen.length })}
          </span>
          {chosen.length < shown.length && (
            <button
              type="button"
              onClick={selectAll}
              className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
            >
              <ListChecks aria-hidden size={14} /> {t('tracks.selectAll')}
            </button>
          )}
          <button
            type="button"
            onClick={() => onPlay(chosen, 0)}
            className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
          >
            <Play aria-hidden size={14} /> {t('menu.play')}
          </button>
          <button
            type="button"
            onClick={() => queue(true)}
            className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
          >
            <ListStart aria-hidden size={14} /> {t('menu.playNext')}
          </button>
          <button
            type="button"
            onClick={() => queue(false)}
            className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
          >
            <ListEnd aria-hidden size={14} /> {t('menu.addToQueue')}
          </button>
          {kind !== 'favorites' && (
            <button
              type="button"
              disabled={favorite.isPending}
              onClick={() => favorite.mutate({ rows: chosen, on: !allFavorite })}
              className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
            >
              {allFavorite ? <HeartOff aria-hidden size={14} /> : <Heart aria-hidden size={14} />}{' '}
              {allFavorite ? t('tracks.unfavorite') : t('tracks.favorite')}
            </button>
          )}
          <button
            type="button"
            disabled={toDownload.length === 0}
            onClick={() => void downloadSongs(toDownload.map((track) => track.input))}
            className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm disabled:opacity-40"
          >
            <Download aria-hidden size={14} />{' '}
            {toDownload.length === 0
              ? t('songs.allDownloaded')
              : toDownload.length === chosen.length
                ? t('songs.downloadSelected')
                : t('songs.downloadSome', { count: toDownload.length })}
          </button>
          <button
            type="button"
            disabled={transcribe.isPending}
            onClick={() => transcribe.mutate(chosen)}
            className="hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
          >
            <AudioLines aria-hidden size={14} /> {t('lyricsEdit.transcribeSelected')}
          </button>
          {onRemove && (
            <button
              type="button"
              onClick={() => {
                onRemove(chosen);
                setSelected(new Set());
              }}
              className="text-primary hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm"
            >
              <Trash2 aria-hidden size={14} /> {t('tracks.remove')}
            </button>
          )}
          {transcribe.isSuccess && (
            <span role="status" className="text-muted w-full px-3 text-xs">
              {t('lyricsEdit.transcribeQueued', { count: transcribe.variables.length })}
            </span>
          )}
          {favorite.error && (
            <span role="alert" className="text-primary w-full px-3 text-xs">
              {favorite.error.message}
            </span>
          )}
          {transcribe.error && (
            <span role="alert" className="text-primary w-full px-3 text-xs">
              {transcribe.error.message}
            </span>
          )}
          <button
            type="button"
            aria-label={t('tracks.clearSelection')}
            onClick={() => setSelected(new Set())}
            className="text-muted hover:text-foreground grid size-8 place-items-center rounded-full"
          >
            <X aria-hidden size={16} />
          </button>
        </div>
      )}
      <div
        role="table"
        aria-label={t('tracks.table')}
        aria-rowcount={shown.length}
        className="flex flex-col"
        onKeyDown={(event) => {
          // Ctrl+A selects every shown song, as in a file list.
          if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === 'a') {
            event.preventDefault();
            selectAll();
          }
        }}
      >
        {wide && (
          <div
            ref={headerRow}
            role="row"
            style={{ gridTemplateColumns: template }}
            className="text-muted border-foreground/10 grid items-center gap-4 border-b px-2 pb-2 text-xs"
          >
            <span role="columnheader" className={`text-center ${reorderable ? 'pl-4' : ''}`}>
              #
            </span>
            <div
              role="columnheader"
              aria-sort={
                view.sort === 'title' ? (view.descending ? 'descending' : 'ascending') : undefined
              }
            >
              <button
                type="button"
                onClick={() => sortBy('title')}
                className={`flex items-center gap-1 ${view.sort === 'title' ? 'text-primary' : 'hover:text-foreground'}`}
              >
                {t('tracks.column.title')}
                {sortOf('title') === 'asc' && <ArrowUp aria-hidden size={12} />}
                {sortOf('title') === 'desc' && <ArrowDown aria-hidden size={12} />}
              </button>
            </div>
            <DndContext sensors={sensors} collisionDetection={closestCenter} onDragEnd={onDragEnd}>
              <SortableContext
                items={columns.map((column) => column.id)}
                strategy={horizontalListSortingStrategy}
              >
                {columns.map((column) => (
                  <HeaderCell
                    key={column.id}
                    id={column.id}
                    label={t(`tracks.column.${column.id}`)}
                    sorted={column.id === 'tags' ? null : sortOf(column.id)}
                    onSort={() => column.id !== 'tags' && sortBy(column.id)}
                    onResizeStart={(event) => startResize(column.id, event)}
                    onResizeBy={(by) => resizeBy(column.id, by)}
                  />
                ))}
              </SortableContext>
            </DndContext>
            <div className="flex justify-end">
              <button
                type="button"
                aria-label={t('tracks.columns')}
                aria-haspopup="dialog"
                onClick={(event) => setColumnsAnchor(columnsAnchor ? null : event.currentTarget)}
                className="hover:text-foreground grid size-7 place-items-center rounded-full"
              >
                <ChevronDown aria-hidden size={16} />
              </button>
            </div>
          </div>
        )}
        {tracks.length > 0 && shown.length === 0 && (
          <p className="text-muted px-2 py-4 text-sm">{t('tracks.noMatch', { filter })}</p>
        )}
        <DndContext
          sensors={rowSensors}
          collisionDetection={closestCenter}
          onDragEnd={onRowDragEnd}
        >
          <SortableContext
            items={shown.map((track) => track.rowKey)}
            strategy={verticalListSortingStrategy}
          >
            {shown.map((track, index) => {
              const playing = playingMatch(track);
              const isSelected = selected.has(track.rowKey);
              const compact = view.view === 'compact';
              return (
                <SortableRow key={track.rowKey} id={track.rowKey} enabled={reorderable}>
                  {(sortable) => {
                    // The grip (M46.44), shown on every row in edit mode (M46.49).
                    const grip = sortable && (
                      <button
                        type="button"
                        ref={sortable.setActivatorNodeRef}
                        {...sortable.attributes}
                        {...sortable.listeners}
                        aria-label={t('tracks.dragRow', { title: track.title })}
                        onClick={(event) => event.stopPropagation()}
                        className={`text-muted hover:text-foreground grid shrink-0 cursor-grab touch-none place-items-center outline-none active:cursor-grabbing ${wide ? 'h-8 w-4' : 'h-10 w-6'}`}
                      >
                        <GripVertical aria-hidden size={16} />
                      </button>
                    );
                    return (
                      <div
                        ref={sortable?.setNodeRef}
                        role="row"
                        aria-selected={isSelected}
                        data-testid="track-table-row"
                        data-reveal={songReveal(track.title, track.artist)}
                        onClick={(event) => {
                          // A finger's tap anywhere plays the song; once some are picked, it picks.
                          if (touch && !wide && selected.size === 0) onPlay(shown, index);
                          else pick(event, index);
                        }}
                        onDoubleClick={() => onPlay(shown, index)}
                        onKeyDown={(event) => {
                          if (event.key === 'Enter') onPlay(shown, index);
                        }}
                        onContextMenu={menu(track.input, {
                          onPlay: () => onPlay(shown, index),
                          playlistItemId: track.playlistItemId,
                        })}
                        tabIndex={0}
                        style={{
                          ...(wide ? { gridTemplateColumns: template } : {}),
                          ...(sortable
                            ? {
                                transform: CSS.Translate.toString(sortable.transform),
                                transition: sortable.transition,
                              }
                            : {}),
                        }}
                        className={`group grid items-center gap-4 rounded-lg px-2 outline-none ${compact ? 'py-1' : wide ? 'py-1.5' : 'py-2'} ${
                          wide ? '' : 'grid-cols-[minmax(0,1fr)_auto]'
                        } ${
                          sortable?.isDragging
                            ? 'bg-surface relative z-10 shadow-lg shadow-black/30'
                            : isSelected
                              ? 'bg-primary/15'
                              : 'hover:bg-surface focus-visible:bg-surface'
                        }`}
                      >
                        {wide && (
                          <span className="flex h-8 items-center text-sm tabular-nums">
                            {grip}
                            <span className="relative grid h-8 flex-1 place-items-center">
                              <span className="text-muted grid place-items-center group-hover:invisible">
                                {playing ? (
                                  <NowPlayingBars playing={playing === 'playing'} />
                                ) : (
                                  index + 1
                                )}
                              </span>
                              <button
                                type="button"
                                aria-label={t('tracks.playRow', { title: track.title })}
                                onClick={(event) => {
                                  event.stopPropagation();
                                  onPlay(shown, index);
                                }}
                                className="absolute inset-0 hidden place-items-center group-hover:grid"
                              >
                                <Play aria-hidden size={14} fill="currentColor" />
                              </button>
                            </span>
                          </span>
                        )}
                        <div className="flex min-w-0 items-center gap-3">
                          {!wide && grip}
                          {!compact && (
                            <Artwork
                              url={track.artworkUrl}
                              fallback={songCover(track)}
                              // Phones get larger rows, sized from the screen (M46.49).
                              className={`shrink-0 rounded ${wide ? 'size-10' : 'size-[clamp(3rem,14vw,4.25rem)]'}`}
                              iconSize={wide ? 18 : 22}
                            />
                          )}
                          <div
                            className={`flex min-w-0 flex-1 ${compact ? 'items-baseline gap-2' : 'flex-col gap-1.5'}`}
                          >
                            <button
                              type="button"
                              onClick={(event) => {
                                event.stopPropagation();
                                if (!wide) onPlay(shown, index);
                                else pick(event, index);
                              }}
                              className={`flex min-w-0 items-center gap-1.5 truncate text-left font-medium ${wide ? 'text-sm' : 'text-[clamp(0.9375rem,4vw,1.0625rem)]'} ${playing ? 'text-primary' : ''}`}
                            >
                              {playing && !wide && (
                                <NowPlayingBars playing={playing === 'playing'} />
                              )}
                              <span className="truncate">{track.title}</span>
                              {track.explicit && <ExplicitMark />}
                              <SongMarks
                                title={track.title}
                                artist={track.artist}
                                trackKey={track.trackKey}
                              />
                            </button>
                            {/* No source had it when imported (M46.31): choose one. */}
                            {unavailable.has(track.trackKey) && (
                              <button
                                type="button"
                                onClick={(event) => {
                                  event.stopPropagation();
                                  useSourcePickerStore.getState().open(track.input);
                                }}
                                className="text-muted hover:text-foreground flex items-center gap-1 self-start text-xs"
                              >
                                <CircleSlash aria-hidden size={12} />
                                {t('sourcePicker.unavailable')} · {t('sourcePicker.menu')}
                              </button>
                            )}
                            <ArtistLinks
                              credit={track.artist}
                              row
                              className={`text-muted block truncate ${wide ? 'text-xs' : 'text-[clamp(0.8125rem,3.4vw,0.9375rem)]'}`}
                            />
                          </div>
                        </div>
                        {wide &&
                          columns.map((column) => (
                            <span
                              key={column.id}
                              role="cell"
                              className="text-muted min-w-0 truncate text-sm tabular-nums"
                            >
                              {column.id === 'album' && (track.album ?? '')}
                              {column.id === 'added' && ago(track.addedAt)}
                              {column.id === 'plays' &&
                                (track.playCount > 0 ? track.playCount : '')}
                              {column.id === 'duration' &&
                                (track.durationMs ? formatTime(track.durationMs) : '')}
                              {column.id === 'tags' &&
                                (tagsFor.data?.get(track.trackKey) ?? [])
                                  .slice(0, 3)
                                  .map((found) => found.name)
                                  .join(' · ')}
                            </span>
                          ))}
                        <span
                          className="flex items-center justify-end gap-0.5"
                          onClick={(event) => event.stopPropagation()}
                        >
                          {actions ? (
                            actions(track, index)
                          ) : (
                            <>
                              {/* Shown on hover: a touch screen has none, so the row's menu
                                  saves there instead of a button no one can see. */}
                              {!touch && (
                                <SaveButton
                                  track={track.input}
                                  compact
                                  className="opacity-0 group-hover:opacity-100 focus:opacity-100"
                                />
                              )}
                              {!wide && track.durationMs ? (
                                <span className="text-muted pr-1 text-xs tabular-nums">
                                  {formatTime(track.durationMs)}
                                </span>
                              ) : null}
                              <TrackMoreButton
                                track={track.input}
                                playlistItemId={track.playlistItemId}
                              />
                            </>
                          )}
                        </span>
                      </div>
                    );
                  }}
                </SortableRow>
              );
            })}
          </SortableContext>
        </DndContext>
      </div>
      {sortSheet}
      {columnsSheet}
    </div>
  );
};

export type { TableTrack, TableView };
