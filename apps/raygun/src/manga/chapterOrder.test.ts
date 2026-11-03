import type { MangaChapterInfo } from '@raygun/bindings';
import { describe, expect, it } from 'vitest';

import { DEFAULT_ORDER, orderChapters } from './chapterOrder';

const chapter = (
  id: string,
  number: number | null,
  dateUpload: number | null,
  extra: Partial<MangaChapterInfo> = {},
): MangaChapterInfo => ({
  id,
  name: id,
  number,
  volume: null,
  scanlator: null,
  dateUpload,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
  ...extra,
});

// As sources list them: newest first.
const LIST = [
  chapter('c3', 3, 300, { read: true }),
  chapter('extra', null, 250),
  chapter('c2', 2, 200, {
    download: { state: 'done', pagesDone: 5, pagesTotal: 5, error: null },
  }),
  chapter('c1', 1, null, { lastPage: 2 }),
];

const ids = (list: MangaChapterInfo[]) => list.map((item) => item.id);

describe('orderChapters', () => {
  it('keeps the source order, or reverses it', () => {
    expect(ids(orderChapters(LIST, DEFAULT_ORDER))).toEqual(['c3', 'extra', 'c2', 'c1']);
    expect(ids(orderChapters(LIST, { ...DEFAULT_ORDER, descending: false }))).toEqual([
      'c1',
      'c2',
      'extra',
      'c3',
    ]);
  });

  it('sorts by number or upload date, unknowns last', () => {
    expect(
      ids(orderChapters(LIST, { ...DEFAULT_ORDER, sort: 'number', descending: false })),
    ).toEqual(['c1', 'c2', 'c3', 'extra']);
    expect(ids(orderChapters(LIST, { ...DEFAULT_ORDER, sort: 'uploaded' }))).toEqual([
      'c3',
      'extra',
      'c2',
      'c1',
    ]);
  });

  it('filters unread, downloaded and started chapters', () => {
    expect(ids(orderChapters(LIST, { ...DEFAULT_ORDER, filters: ['unread'] }))).toEqual([
      'extra',
      'c2',
      'c1',
    ]);
    expect(ids(orderChapters(LIST, { ...DEFAULT_ORDER, filters: ['downloaded'] }))).toEqual(['c2']);
    expect(ids(orderChapters(LIST, { ...DEFAULT_ORDER, filters: ['started'] }))).toEqual(['c1']);
  });
});
