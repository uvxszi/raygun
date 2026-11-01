import { Link } from '@tanstack/react-router';
import { Bell, Compass, Download, FolderCog } from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ImportEpubButton } from '../novel/ImportEpubButton';
import { LibraryControls } from './LibraryControls';
import {
  ALL_CATEGORIES,
  UNCATEGORIZED,
  applyLibraryFilter,
  libraryTags,
  loadFilter,
  saveFilter,
} from './libraryFilters';
import type { LibraryFilter } from './libraryFilters';
import { MangaGrid } from './MangaGrid';
import { ROUTES } from './medium';
import type { ReadingMedium } from './medium';
import { UpdateLibraryButton } from './UpdateLibraryButton';
import { useCategories } from './useCategories';
import { useMangaLibrary, useMangaLiveEvents } from './useManga';

const CHIP = 'bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm';

/**
 * Library → Manga or Novels: the titles you added, by category (Mihon-style tabs), with search,
 * filters, sort and genre tags.
 */
export const MangaLibraryView: FC<{ medium?: ReadingMedium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  useMangaLiveEvents();
  const library = useMangaLibrary(medium);
  const categories = useCategories(medium);
  const [category, setCategory] = useState(ALL_CATEGORIES);
  const [filter, setFilter] = useState<LibraryFilter>(() => loadFilter(medium));
  useEffect(() => saveFilter(medium, filter), [medium, filter]);

  const entries = library.data;
  const shown = useMemo(
    () => (entries ? applyLibraryFilter(entries, filter, category) : []),
    [entries, filter, category],
  );
  const tags = useMemo(() => (entries ? libraryTags(entries) : []), [entries]);
  const hasCategories = (categories.data?.length ?? 0) > 0;
  const hasUncategorized = entries?.some((entry) => entry.categoryIds.length === 0) ?? false;
  const tab = (id: string, label: string, count: number) => (
    <button
      key={id}
      type="button"
      role="tab"
      aria-selected={category === id}
      onClick={() => setCategory(id)}
      className={`shrink-0 border-b-2 px-3 py-2 text-sm ${category === id ? 'border-primary font-semibold' : 'text-muted border-transparent'}`}
    >
      {label} <span className="text-muted text-xs">{count}</span>
    </button>
  );

  return (
    <div data-testid={`${medium}-library`} className="flex flex-col gap-4">
      <nav aria-label={t('manga.libraryActions')} className="flex flex-wrap gap-2">
        <Link to={ROUTES[medium].browse} className={CHIP}>
          <Compass aria-hidden size={16} className="text-primary" />
          {t('manga.browseSources')}
        </Link>
        {medium === 'manga' && (
          <Link to="/manga/updates" className={CHIP}>
            <Bell aria-hidden size={16} className="text-primary" />
            {t('manga.updates')}
          </Link>
        )}
        <Link to="/manga/downloads" className={CHIP}>
          <Download aria-hidden size={16} className="text-primary" />
          {t('manga.downloads')}
        </Link>
        {medium === 'manga' ? <UpdateLibraryButton /> : <ImportEpubButton className={CHIP} />}
        <Link to="/library/categories" search={{ medium }} className={CHIP}>
          <FolderCog aria-hidden size={16} className="text-primary" />
          {t('library.categories.edit')}
        </Link>
      </nav>
      {library.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {library.error.message}
        </p>
      )}
      {entries?.length === 0 && (
        <p className="text-muted text-sm">
          {t(medium === 'novel' ? 'novel.emptyLibrary' : 'manga.emptyLibrary')}
        </p>
      )}
      {entries && entries.length > 0 && (
        <>
          {hasCategories && (
            <div
              role="tablist"
              aria-label={t('library.categories.title')}
              className="-mx-4 flex overflow-x-auto px-4 lg:mx-0 lg:px-0"
            >
              {tab(ALL_CATEGORIES, t('library.categories.all'), entries.length)}
              {categories.data?.map((item) =>
                tab(
                  item.id,
                  item.name,
                  entries.filter((entry) => entry.categoryIds.includes(item.id)).length,
                ),
              )}
              {hasUncategorized &&
                tab(
                  UNCATEGORIZED,
                  t('library.categories.uncategorized'),
                  entries.filter((entry) => entry.categoryIds.length === 0).length,
                )}
            </div>
          )}
          <LibraryControls filter={filter} tags={tags} onChange={setFilter} />
          {shown.length === 0 ? (
            <p className="text-muted text-sm">{t('library.filter.noMatches')}</p>
          ) : (
            <MangaGrid
              medium={medium}
              items={shown.map((entry) => ({
                sourceId: entry.sourceId,
                id: entry.remoteId,
                title: entry.title,
                coverUrl: entry.coverUrl,
                unread: entry.unread,
              }))}
            />
          )}
        </>
      )}
    </div>
  );
};
