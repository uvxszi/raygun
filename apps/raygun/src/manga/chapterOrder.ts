import type { MangaChapterInfo } from '@raygun/bindings';
import { useState } from 'react';

/** How a title's chapters are listed (M46.38), as Mihon's chapter sort and filter. */
export type ChapterSort = 'source' | 'number' | 'uploaded';
export type ChapterFilter = 'unread' | 'downloaded' | 'started';

export type ChapterOrder = {
  sort: ChapterSort;
  /** Newest (or highest) first. */
  descending: boolean;
  filters: ChapterFilter[];
};

export const DEFAULT_ORDER: ChapterOrder = { sort: 'source', descending: true, filters: [] };

const key = (titleId: string) => `raygun.chapterOrder.${titleId}`;

const read = (titleId: string): ChapterOrder => {
  try {
    const saved = localStorage.getItem(key(titleId));
    if (!saved) return DEFAULT_ORDER;
    const parsed = JSON.parse(saved) as Partial<ChapterOrder>;
    return {
      sort:
        parsed.sort === 'number' || parsed.sort === 'uploaded' || parsed.sort === 'source'
          ? parsed.sort
          : DEFAULT_ORDER.sort,
      descending: parsed.descending ?? DEFAULT_ORDER.descending,
      filters: (parsed.filters ?? []).filter(
        (filter): filter is ChapterFilter =>
          filter === 'unread' || filter === 'downloaded' || filter === 'started',
      ),
    };
  } catch {
    return DEFAULT_ORDER;
  }
};

/** A title's chapter order, kept on this device for that title. */
export const useChapterOrder = (titleId: string) => {
  const [order, setOrder] = useState<{ id: string; value: ChapterOrder }>(() => ({
    id: titleId,
    value: read(titleId),
  }));
  const current = order.id === titleId ? order.value : read(titleId);
  const set = (value: ChapterOrder) => {
    setOrder({ id: titleId, value });
    try {
      localStorage.setItem(key(titleId), JSON.stringify(value));
    } catch {
      // A per-device convenience.
    }
  };
  return [current, set] as const;
};

/**
 * The chapters as `order` lists them. Sources list newest first, so "source order" ascending is
 * the reverse; chapters without a number or date go last whichever way.
 */
export const orderChapters = (
  chapters: MangaChapterInfo[],
  order: ChapterOrder,
): MangaChapterInfo[] => {
  const kept = chapters.filter((chapter) =>
    order.filters.every((filter) =>
      filter === 'unread'
        ? !chapter.read
        : filter === 'downloaded'
          ? chapter.download?.state === 'done'
          : chapter.lastPage !== null && !chapter.read,
    ),
  );
  if (order.sort === 'source') return order.descending ? kept : [...kept].reverse();
  const value = (chapter: MangaChapterInfo) =>
    order.sort === 'number' ? chapter.number : chapter.dateUpload;
  return kept
    .map((chapter, index) => ({ chapter, index }))
    .sort((a, b) => {
      const left = value(a.chapter);
      const right = value(b.chapter);
      if (left === null && right === null) return a.index - b.index;
      if (left === null) return 1;
      if (right === null) return -1;
      return (order.descending ? right - left : left - right) || a.index - b.index;
    })
    .map(({ chapter }) => chapter);
};
