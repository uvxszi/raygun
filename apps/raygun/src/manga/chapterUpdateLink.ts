import type { MangaUpdateItem } from '@raygun/bindings';

import { ROUTES } from './medium';

/** Where a new chapter opens: the comic or the book reader. */
export const chapterUpdateLink = (item: MangaUpdateItem) => ({
  to: ROUTES[item.medium === 'novel' ? 'novel' : 'manga'].read,
  search: { title: item.titleId, chapter: item.chapter.id },
});
