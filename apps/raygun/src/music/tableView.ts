import type {
  SavedTrack,
  TrackColumn,
  TrackColumnId,
  TrackInput,
  TrackListView,
  TrackSortKey,
  TrackTableView,
} from '@raygun/bindings';
import { useQueryClient } from '@tanstack/react-query';

import { SETTINGS_KEY, useSettings, useUpdateSetting } from '../settings/useSettings';

/** Where a song table is (each kind keeps its own sort, view and columns, M46.8). */
export type TrackTableKind =
  'favorites' | 'downloaded' | 'playlist' | 'library' | 'album' | 'artist' | 'history' | 'search';

/** One row of a song table. */
export type TableTrack = {
  /** Unique in the table (a playlist can hold a song twice). */
  rowKey: string;
  /** The song's key, for playing in the shown order. */
  trackKey: string;
  title: string;
  artist: string | null;
  album: string | null;
  durationMs: number | null;
  /** Unix ms. */
  addedAt: number | null;
  playCount: number;
  artworkUrl: string | null;
  input: TrackInput;
  playlistItemId?: string;
  /** The source marks it explicit: the row shows "E". */
  explicit?: boolean;
};

/**
 * A row dragged to a new place (M46.44). `order` is the whole list as it now shows; `before` the
 * row it now sits in front of (null: last). `whole` when the table was sorted or reversed: the
 * shown order is then saved as the custom order, else only `moved` changes.
 */
export type TableReorder = {
  order: TableTrack[];
  moved: TableTrack;
  before: TableTrack | null;
  whole: boolean;
};

export type TableView = {
  sort: TrackSortKey;
  descending: boolean;
  view: TrackListView;
  columns: TrackColumn[];
};

/** Matches `TrackTableView::default()` in raygun-model. */
export const TABLE_DEFAULTS: TableView = {
  sort: 'custom',
  descending: false,
  view: 'list',
  columns: [
    { id: 'album', visible: true, width: 0 },
    { id: 'added', visible: true, width: 0 },
    { id: 'plays', visible: false, width: 0 },
    { id: 'tags', visible: false, width: 0 },
    { id: 'duration', visible: true, width: 0 },
  ],
};

export const SORT_KEYS: TrackSortKey[] = [
  'custom',
  'title',
  'artist',
  'album',
  'added',
  'duration',
  'plays',
];

/** Widths when a column hasn't been dragged, in CSS grid terms. */
export const COLUMN_TRACKS: Record<TrackColumnId, string> = {
  album: 'minmax(120px, 0.8fr)',
  added: '128px',
  plays: '80px',
  tags: 'minmax(110px, 0.6fr)',
  duration: '72px',
};

/** A kind's view: saved columns merged with the defaults, so new columns appear. */
const merged = (saved: TrackTableView | undefined): TableView => {
  const columns = (saved?.columns ?? []).filter((column) =>
    TABLE_DEFAULTS.columns.some((known) => known.id === column.id),
  );
  for (const column of TABLE_DEFAULTS.columns) {
    if (!columns.some((have) => have.id === column.id)) columns.push(column);
  }
  return {
    sort: saved?.sort ?? TABLE_DEFAULTS.sort,
    descending: saved?.descending ?? TABLE_DEFAULTS.descending,
    view: saved?.view ?? TABLE_DEFAULTS.view,
    columns,
  };
};

/** A table kind's look, and a way to change it (saved in Settings, M46.8). */
export const useTableView = (kind: TrackTableKind) => {
  const settings = useSettings().data;
  const update = useUpdateSetting();
  const client = useQueryClient();
  const tables = settings?.trackTables?.tables ?? {};
  const view = merged(tables[kind]);
  const set = (patch: Partial<TableView>) => {
    const value = { tables: { ...tables, [kind]: { ...view, ...patch } } };
    // Shown at once (a drag that switches to Custom order mustn't jump back meanwhile).
    if (settings) client.setQueryData(SETTINGS_KEY, { ...settings, trackTables: value });
    update.mutate({ key: 'trackTables', value });
  };
  return { view, set };
};

const text = (value: string | null) => (value ?? '').toLocaleLowerCase();

/** `tracks` sorted as `view` says; custom keeps their own order (descending reverses it). */
export const sortTracks = (tracks: TableTrack[], view: TableView): TableTrack[] => {
  const sorted = [...tracks];
  const by: Record<Exclude<TrackSortKey, 'custom'>, (a: TableTrack, b: TableTrack) => number> = {
    title: (a, b) => text(a.title).localeCompare(text(b.title), undefined, { numeric: true }),
    artist: (a, b) => text(a.artist).localeCompare(text(b.artist), undefined, { numeric: true }),
    album: (a, b) => text(a.album).localeCompare(text(b.album), undefined, { numeric: true }),
    added: (a, b) => (a.addedAt ?? 0) - (b.addedAt ?? 0),
    duration: (a, b) => (a.durationMs ?? 0) - (b.durationMs ?? 0),
    plays: (a, b) => a.playCount - b.playCount,
  };
  if (view.sort !== 'custom') sorted.sort(by[view.sort]);
  return view.descending ? sorted.reverse() : sorted;
};

/**
 * The whole list after shown row `from` is dropped on shown row `to` (M46.44). `sorted` is the
 * whole list as sorted, filters left out: the row lands before the one it was dropped on when
 * moving up, after it when moving down, and rows a filter hides keep their places.
 */
export const dragOrder = (
  sorted: TableTrack[],
  shown: TableTrack[],
  from: number,
  to: number,
): TableTrack[] => {
  const moved = shown[from];
  const target = shown[to];
  if (!moved || !target || moved.rowKey === target.rowKey) return sorted;
  const rest = sorted.filter((row) => row.rowKey !== moved.rowKey);
  const at = rest.findIndex((row) => row.rowKey === target.rowKey);
  rest.splice(from < to ? at + 1 : at, 0, moved);
  return rest;
};

/** Rows whose title, artist or album hold every word of `filter`. */
export const filterTracks = (tracks: TableTrack[], filter: string): TableTrack[] => {
  const words = filter.toLocaleLowerCase().split(/\s+/).filter(Boolean);
  if (words.length === 0) return tracks;
  return tracks.filter((track) => {
    const haystack = `${text(track.title)} ${text(track.artist)} ${text(track.album)}`;
    return words.every((word) => haystack.includes(word));
  });
};

/** A saved song (Favorites, a playlist item, history) as a table row. */
export const savedRow = (
  track: SavedTrack,
  rowKey: string,
  playlistItemId?: string,
): TableTrack => ({
  rowKey,
  trackKey: track.trackKey,
  title: track.title,
  artist: track.artist,
  album: track.album,
  durationMs: track.durationMs,
  addedAt: track.addedAt,
  playCount: track.playCount,
  artworkUrl: track.artworkUrl,
  input: {
    title: track.title,
    artist: track.artist,
    album: track.album,
    durationMs: track.durationMs,
    artworkUrl: track.artworkUrl,
    source: track.source,
    ...(track.explicit ? { explicit: true } : {}),
  },
  ...(playlistItemId ? { playlistItemId } : {}),
  ...(track.explicit ? { explicit: true } : {}),
});
