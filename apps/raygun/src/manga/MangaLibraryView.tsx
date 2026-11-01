import {
  Bell,
  BookImage,
  BookPlus,
  BookText,
  Clapperboard,
  Download,
  FolderCog,
  History,
  ListFilter,
  RefreshCw,
  Search,
} from 'lucide-react';
import { useEffect, useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LibraryTop } from '../library/LibraryTop';
import { EmptyState } from '../shell/EmptyState';
import { useImportEpub } from '../novel/useImportEpub';
import { TopBarAction } from '../shell/TopBar';
import type { OverflowItem } from '../shell/OverflowMenu';
import { ContinueWatchingRow } from '../watch/ContinueWatchingRow';
import { videoTitleLink } from '../watch/useVideoLibrary';
import { LibraryFilterSheet } from './LibraryFilterSheet';
import {
  ALL_CATEGORIES,
  UNCATEGORIZED,
  applyLibraryFilter,
  isFiltering,
  libraryTags,
  loadFilter,
  saveFilter,
} from './libraryFilters';
import type { LibraryFilter } from './libraryFilters';
import { MangaGrid } from './MangaGrid';
import type { Medium } from './medium';
import { useCategories } from './useCategories';
import {
  useMangaLibrary,
  useMangaLiveEvents,
  useMangaUpdateStatus,
  useUpdateLibrary,
} from './useManga';

const EMPTY = {
  manga: { icon: BookImage, title: 'manga.emptyTitle', body: 'manga.emptyLibrary' },
  novel: { icon: BookText, title: 'novel.emptyTitle', body: 'novel.emptyLibrary' },
  video: { icon: Clapperboard, title: 'library.videos.emptyTitle', body: 'library.videos.empty' },
} as const;

/**
 * Library → Manga, Novels or Videos: the titles you added, by category (Mihon-style tabs), with
 * search, filters, sort and genre tags. Videos start with Continue watching (Aniyomi's anime
 * library with CloudStream's resume row).
 */
export const MangaLibraryView: FC<{ medium?: Medium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  useMangaLiveEvents();
  const library = useMangaLibrary(medium);
  const categories = useCategories(medium);
  const [category, setCategory] = useState(ALL_CATEGORIES);
  const [filter, setFilter] = useState<LibraryFilter>(() => loadFilter(medium));
  useEffect(() => saveFilter(medium, filter), [medium, filter]);
  const [searching, setSearching] = useState(filter.query !== '');
  const [filtering, setFiltering] = useState(false);
  const updateStatus = useMangaUpdateStatus();
  const updateLibrary = useUpdateLibrary();
  const updating = updateStatus.data?.running ?? false;
  const epub = useImportEpub();

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
      className={`shrink-0 border-b-2 px-3 py-2 text-sm ${category === id ? 'border-primary text-primary font-semibold' : 'text-muted border-transparent'}`}
    >
      {label} <span className="text-muted text-xs">{count}</span>
    </button>
  );

  const update: OverflowItem = {
    label: updating
      ? t('manga.updating', {
          checked: updateStatus.data?.checked ?? 0,
          total: updateStatus.data?.total ?? 0,
        })
      : t('manga.updateLibrary'),
    icon: RefreshCw,
    onSelect: () => {
      if (!updating) updateLibrary.mutate();
    },
  };
  const video = medium === 'video';
  const menu: OverflowItem[] = [
    ...(medium === 'novel'
      ? [{ label: t('novel.importEpub'), icon: BookPlus, onSelect: epub.pick }]
      : [
          update,
          {
            label: t('manga.updates'),
            icon: Bell,
            to: '/updates',
            ...(video ? { search: { tab: 'episodes' } } : {}),
          },
        ]),
    {
      label: t('manga.downloads'),
      icon: Download,
      to: '/downloads',
      ...(video ? { search: { tab: 'videos' } } : {}),
    },
    ...(video ? [{ label: t('library.videos.history'), icon: History, to: '/watch/history' }] : []),
    {
      label: t('library.categories.edit'),
      icon: FolderCog,
      to: '/library/categories',
      search: { medium },
    },
  ];
  const empty = EMPTY[medium];

  return (
    <div data-testid={`${medium}-library`} className="flex flex-col gap-4">
      <LibraryTop
        media={medium}
        search={
          searching
            ? {
                value: filter.query,
                onChange: (query) => setFilter({ ...filter, query }),
                placeholder: t('library.filter.searchPlaceholder'),
                onClose: () => {
                  setSearching(false);
                  setFilter({ ...filter, query: '' });
                },
              }
            : null
        }
        actions={
          <>
            {!searching && (
              <TopBarAction
                label={t('library.filter.search')}
                icon={Search}
                onClick={() => setSearching(true)}
              />
            )}
            <TopBarAction
              label={t('library.filter.open')}
              icon={ListFilter}
              pressed={isFiltering(filter)}
              onClick={() => setFiltering(true)}
            />
          </>
        }
        menu={menu}
      />
      {epub.input}
      {updating && updateStatus.data && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
          {t('manga.updating', {
            checked: updateStatus.data.checked,
            total: updateStatus.data.total,
          })}
        </p>
      )}
      {epub.importBook.isPending && (
        <p role="status" className="text-muted text-sm">
          {t('novel.importing')}
        </p>
      )}
      {epub.importBook.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {epub.importBook.error.message}
        </p>
      )}
      {library.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {library.error.message}
        </p>
      )}
      {video && <ContinueWatchingRow />}
      {entries?.length === 0 && (
        <EmptyState
          icon={empty.icon}
          title={t(empty.title)}
          body={t(empty.body)}
          action={{
            label: t('library.findInSearch'),
            icon: Search,
            to: '/browse',
            search: { tab: 'search', type: medium },
          }}
        />
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
                downloaded: entry.downloaded,
                ...(video ? { link: videoTitleLink(entry.sourceId, entry.remoteId) } : {}),
              }))}
            />
          )}
        </>
      )}
      {filtering && (
        <LibraryFilterSheet
          filter={filter}
          tags={tags}
          medium={medium}
          onChange={setFilter}
          onClose={() => setFiltering(false)}
        />
      )}
    </div>
  );
};
