import type { MangaLibraryEntry } from '@raygun/bindings';

import {
  ALL_CATEGORIES,
  DEFAULT_FILTER,
  UNCATEGORIZED,
  applyLibraryFilter,
  libraryTags,
} from './libraryFilters';

const entry = (overrides: Partial<MangaLibraryEntry>): MangaLibraryEntry => ({
  titleId: 't',
  sourceId: 's',
  remoteId: 'r',
  title: 'Title',
  coverUrl: null,
  chapters: 10,
  unread: 0,
  genres: [],
  status: 'ongoing',
  addedAt: 0,
  lastReadAt: null,
  latestChapterAt: null,
  started: false,
  downloaded: 0,
  categoryIds: [],
  ...overrides,
});

const LIBRARY = [
  entry({
    titleId: 'a',
    title: 'Frieren',
    unread: 3,
    genres: ['Fantasy', 'Drama'],
    categoryIds: ['c1'],
    lastReadAt: 30,
    addedAt: 1,
  }),
  entry({
    titleId: 'b',
    title: 'Berserk',
    unread: 0,
    genres: ['Fantasy', 'Horror'],
    status: 'completed',
    downloaded: 2,
    started: true,
    lastReadAt: 10,
    addedAt: 3,
  }),
  entry({
    titleId: 'c',
    title: 'Chainsaw Man',
    unread: 12,
    genres: ['Action', 'Horror'],
    categoryIds: ['c2'],
    started: true,
    latestChapterAt: 99,
    addedAt: 2,
  }),
];
const ids = (list: MangaLibraryEntry[]) => list.map((item) => item.titleId);

describe('library filters', () => {
  it('sorts by title by default and by other keys in either direction', () => {
    expect(ids(applyLibraryFilter(LIBRARY, DEFAULT_FILTER))).toEqual(['b', 'c', 'a']);
    expect(
      ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, sort: 'unread', descending: true })),
    ).toEqual(['c', 'a', 'b']);
    expect(
      ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, sort: 'lastRead', descending: true })),
    ).toEqual(['a', 'b', 'c']);
    expect(ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, sort: 'added' }))).toEqual([
      'a',
      'c',
      'b',
    ]);
    expect(
      ids(
        applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, sort: 'latestChapter', descending: true }),
      )[0],
    ).toBe('c');
  });

  it('filters by state, tags, text and category', () => {
    expect(ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, unread: true }))).toEqual([
      'c',
      'a',
    ]);
    expect(ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, started: true }))).toEqual([
      'b',
      'c',
    ]);
    expect(ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, downloaded: true }))).toEqual([
      'b',
    ]);
    expect(ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, completed: true }))).toEqual(['b']);
    expect(
      ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, tags: ['horror', 'Fantasy'] })),
    ).toEqual(['b']);
    expect(ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, query: 'saw' }))).toEqual(['c']);
    expect(ids(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, 'c1'))).toEqual(['a']);
    expect(ids(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, UNCATEGORIZED))).toEqual(['b']);
    expect(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, ALL_CATEGORIES)).toHaveLength(3);
  });

  it('lists tags by how common they are', () => {
    expect(libraryTags(LIBRARY)).toEqual(['Fantasy', 'Horror', 'Action', 'Drama']);
    expect(libraryTags(LIBRARY, 1)).toEqual(['Fantasy']);
  });
});
