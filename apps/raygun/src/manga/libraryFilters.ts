import type { LibraryList, MangaLibraryEntry } from '@raygun/bindings';

import { matchesQuery, parseLibraryQuery } from './libraryQuery';

/** How the manga, novel and video libraries can be ordered (Mihon's sorts, M46). */
export type LibrarySort =
  | 'title'
  | 'totalChapters'
  | 'lastRead'
  | 'lastChecked'
  | 'unread'
  | 'latestChapter'
  | 'fetchedAt'
  | 'added'
  | 'random';
export const LIBRARY_SORTS: LibrarySort[] = [
  'title',
  'totalChapters',
  'lastRead',
  'lastChecked',
  'unread',
  'latestChapter',
  'fetchedAt',
  'added',
  'random',
];

/** Off, only titles that have it, or only titles that don't (Mihon's tri-state filters). */
export type TriState = 'off' | 'include' | 'exclude';
export const nextTriState = (state: TriState): TriState =>
  state === 'off' ? 'include' : state === 'include' ? 'exclude' : 'off';

/** The yes-or-no filters. */
export type FlagFilter =
  'unread' | 'started' | 'downloaded' | 'completed' | 'favorite' | 'pinned' | 'local';
export const FLAG_FILTERS: FlagFilter[] = [
  'unread',
  'started',
  'downloaded',
  'completed',
  'favorite',
  'pinned',
  'local',
];

export type LibraryFilter = {
  query: string;
  flags: Partial<Record<FlagFilter, TriState>>;
  /** By list; `none` is titles on no list. */
  lists: Partial<Record<LibraryList | 'none', TriState>>;
  /** Genres, authors and artists, by name in lowercase. */
  tags: Record<string, TriState>;
  people: Record<string, TriState>;
  sort: LibrarySort;
  descending: boolean;
  /** What random order shuffles with; picking Random again rolls a new one. */
  seed: number;
};

export const DEFAULT_FILTER: LibraryFilter = {
  query: '',
  flags: {},
  lists: {},
  tags: {},
  people: {},
  sort: 'title',
  descending: false,
  seed: 0,
};

/** The category tab showing every title. */
export const ALL_CATEGORIES = '__all';
/** The tab of titles in no category. */
export const UNCATEGORIZED = '__none';
/** List tabs (by list) are `list:<list>`, plus favorites. */
export const FAVORITES_TAB = 'list:favorite';

const COMPLETED = new Set(['completed', 'publishingFinished', 'cancelled']);

const flagHolds = (entry: MangaLibraryEntry, flag: FlagFilter): boolean => {
  switch (flag) {
    case 'unread':
      return entry.unread > 0;
    case 'started':
      return entry.started;
    // Imported titles are on the device: they count as downloaded (as in Mihon).
    case 'downloaded':
      return entry.downloaded > 0 || entry.local;
    case 'completed':
      return COMPLETED.has(entry.status);
    case 'favorite':
      return entry.favorite;
    case 'pinned':
      return entry.pinned;
    case 'local':
      return entry.local;
  }
};

/**
 * Whether a set of names passes tri-state choices: one of the included names (any of them, so
 * two authors can be picked) and none of the excluded ones.
 */
const namesPass = (names: string[], choices: Record<string, TriState>): boolean => {
  const have = new Set(names.map((name) => name.toLocaleLowerCase()));
  const included = Object.keys(choices).filter((name) => choices[name] === 'include');
  if (included.length > 0 && !included.some((name) => have.has(name))) return false;
  return !Object.keys(choices).some((name) => choices[name] === 'exclude' && have.has(name));
};

/** Whether a tab shows an entry. */
export const inTab = (entry: MangaLibraryEntry, tab: string): boolean => {
  if (tab === ALL_CATEGORIES) return true;
  if (tab === UNCATEGORIZED) return entry.categoryIds.length === 0;
  if (tab === FAVORITES_TAB) return entry.favorite;
  if (tab.startsWith('list:')) return entry.list === tab.slice(5);
  return entry.categoryIds.includes(tab);
};

/** A new seed for the random sort. */
export const newShuffleSeed = () => Math.floor(Math.random() * 2 ** 31);

/** A number from 0 to 1 for an id, the same for the same seed (a stable shuffle). */
const shuffleKey = (id: string, seed: number) => {
  let hash = (seed ^ 0x9e3779b9) >>> 0;
  for (const char of id) hash = Math.imul(hash ^ char.charCodeAt(0), 0x01000193) >>> 0;
  return hash / 0x100000000;
};

const sortValue = (entry: MangaLibraryEntry, sort: LibrarySort, seed: number) => {
  switch (sort) {
    case 'title':
      return null;
    case 'totalChapters':
      return entry.chapters;
    case 'lastRead':
      return entry.lastReadAt;
    case 'lastChecked':
      return entry.refreshedAt;
    case 'unread':
      return entry.unread;
    case 'latestChapter':
      return entry.latestChapterAt;
    case 'fetchedAt':
      return entry.latestFetchedAt;
    case 'added':
      return entry.addedAt;
    case 'random':
      return shuffleKey(entry.titleId, seed);
  }
};

const collator = new Intl.Collator(undefined, { sensitivity: 'base', numeric: true });

/**
 * Orders entries as Mihon does: the chosen key in the chosen direction (missing values first when
 * ascending), titles with nothing unread always last when sorting by unread, then A→Z whatever
 * the direction. Pinned titles can be kept on top (Shosetsu).
 */
export const sortEntries = (
  entries: MangaLibraryEntry[],
  filter: Pick<LibraryFilter, 'sort' | 'descending' | 'seed'>,
  pinsOnTop = true,
): MangaLibraryEntry[] => {
  const direction = filter.descending && filter.sort !== 'random' ? -1 : 1;
  const sorted = [...entries].sort((a, b) => {
    if (pinsOnTop && a.pinned !== b.pinned) return a.pinned ? -1 : 1;
    if (filter.sort === 'unread' && (a.unread === 0) !== (b.unread === 0)) {
      return a.unread === 0 ? 1 : -1;
    }
    const left = sortValue(a, filter.sort, filter.seed);
    const right = sortValue(b, filter.sort, filter.seed);
    let order: number;
    if (filter.sort === 'title') {
      order = collator.compare(a.title, b.title);
    } else if (left === null || right === null) {
      order = left === right ? 0 : left === null ? -1 : 1;
    } else {
      order = left - right;
    }
    return order * direction || collator.compare(a.title, b.title);
  });
  return sorted;
};

/** The entries a tab and filter show, in the chosen order. */
export const applyLibraryFilter = (
  entries: MangaLibraryEntry[],
  filter: LibraryFilter,
  tab: string = ALL_CATEGORIES,
  pinsOnTop = true,
): MangaLibraryEntry[] => {
  const query = parseLibraryQuery(filter.query);
  const shown = entries.filter((entry) => {
    if (!inTab(entry, tab)) return false;
    for (const flag of FLAG_FILTERS) {
      const state = filter.flags[flag] ?? 'off';
      if (state !== 'off' && flagHolds(entry, flag) !== (state === 'include')) return false;
    }
    const lists = filter.lists;
    const includedLists = Object.keys(lists).filter(
      (list) => lists[list as keyof typeof lists] === 'include',
    );
    const listKey = entry.list ?? 'none';
    if (includedLists.length > 0 && !includedLists.includes(listKey)) return false;
    if (lists[listKey] === 'exclude') return false;
    if (!namesPass(entry.genres, filter.tags)) return false;
    if (!namesPass([...entry.authors, ...entry.artists], filter.people)) return false;
    return matchesQuery(entry, query);
  });
  return sortEntries(shown, filter, pinsOnTop);
};

/** Names in the library (genres, or authors and artists), most common first, for chips. */
export const libraryNames = (
  entries: MangaLibraryEntry[],
  pick: (entry: MangaLibraryEntry) => string[],
  limit = 40,
): string[] => {
  const counts = new Map<string, { name: string; count: number }>();
  for (const entry of entries) {
    for (const name of new Set(pick(entry))) {
      if (!name.trim()) continue;
      const key = name.toLocaleLowerCase();
      const known = counts.get(key);
      if (known) known.count += 1;
      else counts.set(key, { name, count: 1 });
    }
  }
  return [...counts.values()]
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
    .slice(0, limit)
    .map((item) => item.name);
};

/** Genres present in the library, most common first, for tag chips. */
export const libraryTags = (entries: MangaLibraryEntry[], limit = 40): string[] =>
  libraryNames(entries, (entry) => entry.genres, limit);

/** Authors and artists present in the library, most common first. */
export const libraryPeople = (entries: MangaLibraryEntry[], limit = 40): string[] =>
  libraryNames(entries, (entry) => [...entry.authors, ...entry.artists], limit);

/** Whether anything besides the defaults narrows the library. */
export const isFiltering = (filter: LibraryFilter) =>
  [filter.flags, filter.lists, filter.tags, filter.people].some((choices) =>
    Object.values(choices).some((state) => state !== undefined && state !== 'off'),
  );

const storageKey = (medium: string) => `raygun.libraryFilter.${medium}`;
const tabKey = (medium: string) => `raygun.libraryTab.${medium}`;

type StoredFilter = Partial<LibraryFilter> & {
  // Before M46: include-only switches and a list of genres.
  unread?: boolean;
  started?: boolean;
  downloaded?: boolean;
  completed?: boolean;
  tags?: Record<string, TriState> | string[];
};

/** The filter last used for a library (a per-device convenience). */
export const loadFilter = (medium: string): LibraryFilter => {
  try {
    const stored = JSON.parse(
      localStorage.getItem(storageKey(medium)) ?? 'null',
    ) as StoredFilter | null;
    if (!stored) return DEFAULT_FILTER;
    const flags: LibraryFilter['flags'] = { ...stored.flags };
    for (const flag of ['unread', 'started', 'downloaded', 'completed'] as const) {
      if (stored[flag] === true) flags[flag] = 'include';
    }
    const tags = Array.isArray(stored.tags)
      ? Object.fromEntries(stored.tags.map((tag) => [tag.toLocaleLowerCase(), 'include' as const]))
      : (stored.tags ?? {});
    const sort = LIBRARY_SORTS.includes(stored.sort as LibrarySort)
      ? (stored.sort as LibrarySort)
      : 'title';
    return {
      ...DEFAULT_FILTER,
      lists: stored.lists ?? {},
      people: stored.people ?? {},
      descending: stored.descending ?? false,
      seed: stored.seed ?? 0,
      flags,
      tags,
      sort,
      query: '',
    };
  } catch {
    return DEFAULT_FILTER;
  }
};

export const saveFilter = (medium: string, filter: LibraryFilter) => {
  try {
    localStorage.setItem(storageKey(medium), JSON.stringify({ ...filter, query: '' }));
  } catch {
    // Storage can be unavailable (private mode); the filter just isn't remembered.
  }
};

/** The tab last open in a library (Mihon remembers it). */
export const loadTab = (medium: string): string => {
  try {
    return localStorage.getItem(tabKey(medium)) ?? ALL_CATEGORIES;
  } catch {
    return ALL_CATEGORIES;
  }
};

export const saveTab = (medium: string, tab: string) => {
  try {
    localStorage.setItem(tabKey(medium), tab);
  } catch {
    // Not remembered; nothing else depends on it.
  }
};
