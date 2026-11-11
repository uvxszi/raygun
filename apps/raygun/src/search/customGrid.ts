import type { ExtensionInfo, RepoCatalog } from '@raygun/bindings';

import type { Medium } from '../manga/medium';
import { matchesSearch } from '../shell/useListSearch';
import type { SourceOption } from './useCustomSources';

/**
 * Custom sources' grid of source toggles (search/SourceGrid.tsx): which sources it shows and how
 * (found by name, grouped, the ones on first), its rows at the panel's width, All/None/Invert
 * over what's shown, and moving between tiles with the keys.
 */

/** A source as its tile shows it, with what groups it. */
export type GridSource = {
  id: string;
  name: string;
  adult: boolean;
  /** Its extension's icon, through the cover cache; its first letter without one. */
  icon: string | null;
  /** The repository its extension came from; none for one added by itself. */
  repository: string | null;
  /** Its extension's language as its repository lists it (`en`, `multi`); none when unknown. */
  lang: string | null;
};

export type Grouping = 'none' | 'repository' | 'language';
export const GROUPINGS: readonly Grouping[] = ['none', 'repository', 'language'];

/** One repository's sources or one language's, under a header that folds them. */
export type GridGroup = { key: string; label: string; sources: GridSource[] };

/** A row of the grid: a group's header, or a row of tiles. */
export type GridRow =
  | { kind: 'group'; key: string; group: GridGroup; open: boolean }
  | { kind: 'tiles'; key: string; tiles: GridSource[] };

/** A place in the grid: its row, and the tile (or the header's button) in it. */
export type Cell = { row: number; col: number };

/** Every source's tile, with its extension's icon and repository and, when known, language. */
export const gridSources = (
  sources: readonly SourceOption[],
  extensions: readonly ExtensionInfo[] | undefined,
  catalog: readonly RepoCatalog[] | undefined,
): GridSource[] => {
  const byId = new Map((extensions ?? []).map((extension) => [extension.id, extension]));
  const langs = new Map<string, string>();
  for (const listing of catalog ?? []) {
    for (const entry of listing.entries) {
      if (entry.lang && !langs.has(entry.id)) langs.set(entry.id, entry.lang);
    }
  }
  return sources.map((source) => {
    const extension = source.extensionId ? byId.get(source.extensionId) : undefined;
    return {
      id: source.id,
      name: source.name,
      adult: Boolean(source.adult),
      icon: extension?.icon ?? null,
      repository: extension?.repository ?? null,
      lang: (source.extensionId && langs.get(source.extensionId)) || null,
    };
  });
};

/** A source's group under `grouping`: its repository's name, or its language. */
const groupOf = (source: GridSource, grouping: Grouping) =>
  grouping === 'repository'
    ? (source.repository ?? '')
    : source.lang === 'all'
      ? 'multi'
      : (source.lang ?? '');

/** Groups by name; many languages, then unknown ones or sources added by themselves, last. */
const groupRank = (grouping: Grouping, key: string) =>
  key === '' ? 2 : grouping === 'language' && key === 'multi' ? 1 : 0;

type Arrangement = {
  /** What's typed in the panel's field: sources whose names match. */
  needle: string;
  grouping: Grouping;
  /** The sources that were on when the list was last arranged, first (On first). */
  first: ReadonlySet<string> | null;
  /** A group's header. */
  labelOf: (grouping: Grouping, key: string) => string;
};

/**
 * The sources the panel shows, in groups (one group without a name when not grouped): those
 * whose names match what's typed, in the order they're listed, the ones on first when asked.
 */
export const arrangeSources = (
  sources: readonly GridSource[],
  { needle, grouping, first, labelOf }: Arrangement,
): GridGroup[] => {
  const matching = needle
    ? sources.filter((source) => matchesSearch(needle, [source.name]))
    : sources;
  const ordered = first
    ? [
        ...matching.filter((source) => first.has(source.id)),
        ...matching.filter((source) => !first.has(source.id)),
      ]
    : [...matching];
  if (grouping === 'none')
    return ordered.length > 0 ? [{ key: '', label: '', sources: ordered }] : [];
  const groups = new Map<string, GridSource[]>();
  for (const source of ordered) {
    const key = groupOf(source, grouping);
    const members = groups.get(key);
    if (members) members.push(source);
    else groups.set(key, [source]);
  }
  return [...groups]
    .map(([key, members]) => ({ key, label: labelOf(grouping, key), sources: members }))
    .sort(
      (one, other) =>
        groupRank(grouping, one.key) - groupRank(grouping, other.key) ||
        one.label.localeCompare(other.label),
    );
};

/**
 * The grid's rows at `columns` tiles a row: each group's header (when there are headers) and,
 * unless it's folded, its tiles.
 */
export const gridRows = (
  groups: readonly GridGroup[],
  columns: number,
  folded: ReadonlySet<string>,
  headers: boolean,
): GridRow[] => {
  const rows: GridRow[] = [];
  for (const group of groups) {
    const open = !headers || !folded.has(group.key);
    if (headers) rows.push({ kind: 'group', key: `group:${group.key}`, group, open });
    if (!open) continue;
    for (let at = 0; at < group.sources.length; at += columns) {
      const tiles = group.sources.slice(at, at + columns);
      rows.push({ kind: 'tiles', key: `tiles:${tiles[0]!.id}`, tiles });
    }
  }
  return rows;
};

/**
 * Tiles a row by the grid's own width (not the window's), each at least about 4.4 rem wide so a
 * name fits two short lines: 4 on a phone held upright, 6 on a wider phone, a small tablet or a
 * window beside the side panel, 8 on a wide window, a large tablet or a phone on its side.
 */
export const columnsFor = (width: number, remPx: number) =>
  width >= 38 * remPx ? 8 : width >= 28 * remPx ? 6 : 4;

export type BulkAction = 'all' | 'none' | 'invert';

/** The sources on after `action` on `shown` (what the panel shows); the others keep theirs. */
export const bulkChoice = (
  picked: readonly string[],
  shown: readonly string[],
  action: BulkAction,
): string[] => {
  const on = new Set(picked);
  for (const id of shown) {
    const want = action === 'all' ? true : action === 'none' ? false : !on.has(id);
    if (want) on.add(id);
    else on.delete(id);
  }
  return [...on];
};

/** How many cells a row has: its tiles, or a header's fold and its switch. */
export const cellsOf = (row: GridRow) => (row.kind === 'tiles' ? row.tiles.length : 2);

/**
 * Where a key moves the focus from `at`, or null when it doesn't move it: Left and Right to the
 * next cell in reading order, Up and Down to the row above or below at `column` (the column the
 * focus keeps while it moves up and down), Home and End to the row's ends (with Ctrl, the
 * grid's), Page Up and Down `page` rows.
 */
export const moveFocus = (
  rows: readonly GridRow[],
  at: Cell,
  key: string,
  ctrl: boolean,
  column: number,
  page: number,
): Cell | null => {
  const last = rows.length - 1;
  const row = rows[at.row];
  if (!row) return null;
  const toRow = (index: number): Cell => {
    const target = Math.max(0, Math.min(last, index));
    return { row: target, col: Math.min(column, cellsOf(rows[target]!) - 1) };
  };
  switch (key) {
    case 'ArrowRight':
      if (at.col + 1 < cellsOf(row)) return { row: at.row, col: at.col + 1 };
      return at.row < last ? { row: at.row + 1, col: 0 } : null;
    case 'ArrowLeft':
      if (at.col > 0) return { row: at.row, col: at.col - 1 };
      return at.row > 0 ? { row: at.row - 1, col: cellsOf(rows[at.row - 1]!) - 1 } : null;
    case 'ArrowDown':
      return at.row < last ? toRow(at.row + 1) : null;
    case 'ArrowUp':
      return at.row > 0 ? toRow(at.row - 1) : null;
    case 'PageDown':
      return at.row < last ? toRow(at.row + page) : null;
    case 'PageUp':
      return at.row > 0 ? toRow(at.row - page) : null;
    case 'Home':
      return ctrl ? { row: 0, col: 0 } : { row: at.row, col: 0 };
    case 'End':
      return ctrl
        ? { row: last, col: cellsOf(rows[last]!) - 1 }
        : { row: at.row, col: cellsOf(row) - 1 };
    default:
      return null;
  }
};

/** How the panel shows the sources, remembered per device and kind. */
export type GridView = {
  grouping: Grouping;
  /** The sources on lead, as they were when the list was last arranged. */
  onFirst: boolean;
  /** Groups folded, by key. */
  folded: string[];
};

const viewKey = (medium: Medium) => `raygun.search.customView.${medium}`;

/** Many sources come in groups (by repository) until another way is picked. */
export const defaultView = (count: number, many: number): GridView => ({
  grouping: count > many ? 'repository' : 'none',
  onFirst: false,
  folded: [],
});

export const readView = (medium: Medium): GridView | null => {
  try {
    const saved: unknown = JSON.parse(localStorage.getItem(viewKey(medium)) ?? 'null');
    if (!saved || typeof saved !== 'object') return null;
    const { grouping, onFirst, folded } = saved as Partial<Record<keyof GridView, unknown>>;
    return {
      grouping: GROUPINGS.includes(grouping as Grouping) ? (grouping as Grouping) : 'none',
      onFirst: onFirst === true,
      folded: Array.isArray(folded)
        ? folded.filter((key): key is string => typeof key === 'string')
        : [],
    };
  } catch {
    return null;
  }
};

export const saveView = (medium: Medium, view: GridView) => {
  try {
    localStorage.setItem(viewKey(medium), JSON.stringify(view));
  } catch {
    // A per-device convenience.
  }
};
