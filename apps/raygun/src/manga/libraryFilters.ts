import type { MangaLibraryEntry } from '@raygun/bindings';

/** How the manga and novel libraries can be ordered (Mihon's sort options). */
export type LibrarySort = 'title' | 'lastRead' | 'latestChapter' | 'unread' | 'added';
export const LIBRARY_SORTS: LibrarySort[] = [
  'title',
  'lastRead',
  'latestChapter',
  'unread',
  'added',
];

export type LibraryFilter = {
  query: string;
  unread: boolean;
  started: boolean;
  downloaded: boolean;
  completed: boolean;
  /** Genres every shown title must have. */
  tags: string[];
  sort: LibrarySort;
  descending: boolean;
};

export const DEFAULT_FILTER: LibraryFilter = {
  query: '',
  unread: false,
  started: false,
  downloaded: false,
  completed: false,
  tags: [],
  sort: 'title',
  descending: false,
};

/** The category tab showing every title. */
export const ALL_CATEGORIES = '__all';
/** The tab of titles in no category. */
export const UNCATEGORIZED = '__none';

const COMPLETED = new Set(['completed', 'publishingFinished', 'cancelled']);

const sortValue = (entry: MangaLibraryEntry, sort: LibrarySort): number | string => {
  switch (sort) {
    case 'title':
      return entry.title.toLocaleLowerCase();
    case 'lastRead':
      return entry.lastReadAt ?? 0;
    case 'latestChapter':
      return entry.latestChapterAt ?? 0;
    case 'unread':
      return entry.unread;
    case 'added':
      return entry.addedAt ?? 0;
  }
};

/** The entries a category tab and filter show, in the chosen order. */
export const applyLibraryFilter = (
  entries: MangaLibraryEntry[],
  filter: LibraryFilter,
  category: string = ALL_CATEGORIES,
): MangaLibraryEntry[] => {
  const query = filter.query.trim().toLocaleLowerCase();
  const tags = filter.tags.map((tag) => tag.toLocaleLowerCase());
  const shown = entries.filter((entry) => {
    if (category === UNCATEGORIZED && entry.categoryIds.length > 0) return false;
    if (category !== ALL_CATEGORIES && category !== UNCATEGORIZED) {
      if (!entry.categoryIds.includes(category)) return false;
    }
    if (query && !entry.title.toLocaleLowerCase().includes(query)) return false;
    if (filter.unread && entry.unread === 0) return false;
    if (filter.started && !entry.started) return false;
    if (filter.downloaded && entry.downloaded === 0) return false;
    if (filter.completed && !COMPLETED.has(entry.status)) return false;
    if (tags.length > 0) {
      const genres = new Set(entry.genres.map((genre) => genre.toLocaleLowerCase()));
      if (!tags.every((tag) => genres.has(tag))) return false;
    }
    return true;
  });
  const direction = filter.descending ? -1 : 1;
  return shown.sort((a, b) => {
    const left = sortValue(a, filter.sort);
    const right = sortValue(b, filter.sort);
    const order =
      typeof left === 'string' && typeof right === 'string'
        ? left.localeCompare(right)
        : Number(left) - Number(right);
    return order * direction || a.title.localeCompare(b.title);
  });
};

/** Genres present in the library, most common first, for tag chips. */
export const libraryTags = (entries: MangaLibraryEntry[], limit = 40): string[] => {
  const counts = new Map<string, { name: string; count: number }>();
  for (const entry of entries) {
    for (const genre of entry.genres) {
      const key = genre.toLocaleLowerCase();
      const known = counts.get(key);
      if (known) known.count += 1;
      else counts.set(key, { name: genre, count: 1 });
    }
  }
  return [...counts.values()]
    .sort((a, b) => b.count - a.count || a.name.localeCompare(b.name))
    .slice(0, limit)
    .map((tag) => tag.name);
};

/** Whether anything besides the defaults is set. */
export const isFiltering = (filter: LibraryFilter) =>
  filter.unread ||
  filter.started ||
  filter.downloaded ||
  filter.completed ||
  filter.tags.length > 0;

const storageKey = (medium: string) => `raygun.libraryFilter.${medium}`;

/** The filter last used for a library (a per-device convenience). */
export const loadFilter = (medium: string): LibraryFilter => {
  try {
    const stored = JSON.parse(
      localStorage.getItem(storageKey(medium)) ?? 'null',
    ) as Partial<LibraryFilter> | null;
    return { ...DEFAULT_FILTER, ...stored, query: '' };
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
