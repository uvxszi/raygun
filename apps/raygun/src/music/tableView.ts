import type {
  SavedTrack,
  TrackColumn,
  TrackColumnId,
  TrackInput,
  TrackListView,
  TrackSortKey,
  TrackTableView,
} from '@raygun/bindings';

import { useSettings, useUpdateSetting } from '../settings/useSettings';

/** Where a song table is (each kind keeps its own sort, view and columns, M46.8). */
export type TrackTableKind =
  'favorites' | 'playlist' | 'library' | 'album' | 'artist' | 'history' | 'search';

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
  const tables = settings?.trackTables?.tables ?? {};
  const view = merged(tables[kind]);
  const set = (patch: Partial<TableView>) =>
    update.mutate({
      key: 'trackTables',
      value: { tables: { ...tables, [kind]: { ...view, ...patch } } },
    });
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
  },
  ...(playlistItemId ? { playlistItemId } : {}),
});
