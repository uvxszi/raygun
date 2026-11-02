import type { MangaLibraryEntry } from '@raygun/bindings';

import { libraryEntry } from '../test/libraryEntry';
import {
  ALL_CATEGORIES,
  DEFAULT_FILTER,
  FAVORITES_TAB,
  UNCATEGORIZED,
  applyLibraryFilter,
  libraryPeople,
  libraryTags,
  loadFilter,
  nextTriState,
} from './libraryFilters';

const entry = (overrides: Partial<MangaLibraryEntry>): MangaLibraryEntry =>
  libraryEntry({ sourceId: 's', ...overrides });

const LIBRARY = [
  entry({
    titleId: 'a',
    title: 'Frieren',
    unread: 3,
    chapters: 120,
    genres: ['Fantasy', 'Drama'],
    authors: ['Yamada'],
    categoryIds: ['c1'],
    lastReadAt: 30,
    addedAt: 1,
    list: 'reading',
    favorite: true,
    refreshedAt: 5,
    latestFetchedAt: 50,
  }),
  entry({
    titleId: 'b',
    title: 'Berserk',
    unread: 0,
    chapters: 370,
    genres: ['Fantasy', 'Horror'],
    authors: ['Miura'],
    status: 'completed',
    local: true,
    started: true,
    lastReadAt: 10,
    addedAt: 3,
    list: 'completed',
    refreshedAt: 9,
  }),
  entry({
    titleId: 'c',
    title: 'Chainsaw Man',
    unread: 12,
    chapters: 180,
    genres: ['Action', 'Horror'],
    authors: ['Fujimoto'],
    categoryIds: ['c2'],
    started: true,
    downloaded: 2,
    latestChapterAt: 99,
    latestFetchedAt: 70,
    addedAt: 2,
  }),
];
const ids = (list: MangaLibraryEntry[]) => list.map((item) => item.titleId);
const sorted = (overrides: Partial<typeof DEFAULT_FILTER>) =>
  ids(applyLibraryFilter(LIBRARY, { ...DEFAULT_FILTER, ...overrides }));

describe('library filters', () => {
  it('sorts by every key, ties A→Z, nothing unread last', () => {
    expect(sorted({})).toEqual(['b', 'c', 'a']);
    expect(sorted({ descending: true })).toEqual(['a', 'c', 'b']);
    expect(sorted({ sort: 'totalChapters' })).toEqual(['a', 'c', 'b']);
    expect(sorted({ sort: 'lastRead', descending: true })).toEqual(['a', 'b', 'c']);
    expect(sorted({ sort: 'lastRead' }), 'never read comes first').toEqual(['c', 'b', 'a']);
    expect(sorted({ sort: 'lastChecked', descending: true })).toEqual(['b', 'a', 'c']);
    expect(sorted({ sort: 'unread' })).toEqual(['a', 'c', 'b']);
    expect(sorted({ sort: 'unread', descending: true })).toEqual(['c', 'a', 'b']);
    expect(sorted({ sort: 'latestChapter', descending: true })[0]).toBe('c');
    expect(sorted({ sort: 'fetchedAt', descending: true })).toEqual(['c', 'a', 'b']);
    expect(sorted({ sort: 'added' })).toEqual(['a', 'c', 'b']);
  });

  it('shuffles the same way for a seed, and another way for another', () => {
    const first = sorted({ sort: 'random', seed: 1 });
    expect(sorted({ sort: 'random', seed: 1 })).toEqual(first);
    expect(first.slice().sort()).toEqual(['a', 'b', 'c']);
    const seeds = [2, 3, 4, 5, 6, 7].map((seed) => sorted({ sort: 'random', seed }).join(''));
    expect(new Set([first.join(''), ...seeds]).size).toBeGreaterThan(1);
  });

  it('keeps pinned titles on top unless asked not to', () => {
    const pinned = LIBRARY.map((item) => (item.titleId === 'a' ? { ...item, pinned: true } : item));
    expect(ids(applyLibraryFilter(pinned, DEFAULT_FILTER))).toEqual(['a', 'b', 'c']);
    expect(ids(applyLibraryFilter(pinned, { ...DEFAULT_FILTER, descending: true }))[0]).toBe('a');
    expect(ids(applyLibraryFilter(pinned, DEFAULT_FILTER, ALL_CATEGORIES, false))).toEqual([
      'b',
      'c',
      'a',
    ]);
  });

  it('filters with tri-state flags, lists, genres and people', () => {
    expect(sorted({ flags: { unread: 'include' } })).toEqual(['c', 'a']);
    expect(sorted({ flags: { unread: 'exclude' } })).toEqual(['b']);
    expect(sorted({ flags: { downloaded: 'include' } }), 'local counts').toEqual(['b', 'c']);
    expect(sorted({ flags: { completed: 'include', started: 'include' } })).toEqual(['b']);
    expect(sorted({ flags: { favorite: 'exclude' } })).toEqual(['b', 'c']);
    expect(sorted({ lists: { reading: 'include', completed: 'include' } })).toEqual(['b', 'a']);
    expect(sorted({ lists: { none: 'include' } })).toEqual(['c']);
    expect(sorted({ lists: { completed: 'exclude' } })).toEqual(['c', 'a']);
    expect(sorted({ tags: { horror: 'include' } })).toEqual(['b', 'c']);
    expect(sorted({ tags: { horror: 'include', action: 'exclude' } })).toEqual(['b']);
    expect(sorted({ people: { miura: 'include', yamada: 'include' } })).toEqual(['b', 'a']);
    expect(sorted({ query: 'saw' })).toEqual(['c']);
    expect(sorted({ query: 'author:miura || -horror' })).toEqual(['b', 'a']);
  });

  it('shows tabs by category or list', () => {
    expect(ids(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, 'c1'))).toEqual(['a']);
    expect(ids(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, UNCATEGORIZED))).toEqual(['b']);
    expect(ids(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, 'list:completed'))).toEqual(['b']);
    expect(ids(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, FAVORITES_TAB))).toEqual(['a']);
    expect(applyLibraryFilter(LIBRARY, DEFAULT_FILTER, ALL_CATEGORIES)).toHaveLength(3);
  });

  it('lists names by how common they are', () => {
    expect(libraryTags(LIBRARY)).toEqual(['Fantasy', 'Horror', 'Action', 'Drama']);
    expect(libraryTags(LIBRARY, 1)).toEqual(['Fantasy']);
    expect(libraryPeople(LIBRARY)).toEqual(['Fujimoto', 'Miura', 'Yamada']);
  });

  it('cycles tri-states and reads filters saved before M46', () => {
    expect(nextTriState('off')).toBe('include');
    expect(nextTriState('include')).toBe('exclude');
    expect(nextTriState('exclude')).toBe('off');
    localStorage.setItem(
      'raygun.libraryFilter.manga',
      JSON.stringify({ unread: true, tags: ['Horror'], sort: 'unread', descending: true }),
    );
    expect(loadFilter('manga')).toMatchObject({
      flags: { unread: 'include' },
      tags: { horror: 'include' },
      sort: 'unread',
      descending: true,
    });
    localStorage.removeItem('raygun.libraryFilter.manga');
  });
});
