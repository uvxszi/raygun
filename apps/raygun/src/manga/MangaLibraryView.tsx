import { commands } from '@raygun/bindings';
import type { LibraryDisplayMode, MangaLibraryEntry } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import {
  Bell,
  BookImage,
  BookPlus,
  BookText,
  Clapperboard,
  Dices,
  Download,
  FileArchive,
  FolderCog,
  History,
  ListFilter,
  RefreshCw,
  Search,
} from 'lucide-react';
import { useEffect, useMemo, useRef, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LibraryTop } from '../library/LibraryTop';
import { unwrapResult } from '../lib/commandResult';
import { useImportEpub } from '../novel/useImportEpub';
import { useSettings, useUpdateSetting } from '../settings/useSettings';
import { EmptyState } from '../shell/EmptyState';
import type { OverflowItem } from '../shell/OverflowMenu';
import { TopBarAction } from '../shell/TopBar';
import { ContinueWatchingRow } from '../watch/ContinueWatchingRow';
import { STREMIO_SOURCE } from '../watch/useVideoLibrary';
import { LISTS, listKey, withLibraryDisplay } from './libraryDisplay';
import type { LibraryDisplay } from './libraryDisplay';
import { LibraryFilterSheet } from './LibraryFilterSheet';
import {
  ALL_CATEGORIES,
  FAVORITES_TAB,
  UNCATEGORIZED,
  applyLibraryFilter,
  inTab,
  isFiltering,
  libraryPeople,
  libraryTags,
  loadFilter,
  loadTab,
  newShuffleSeed,
  saveFilter,
  saveTab,
} from './libraryFilters';
import type { LibraryFilter, LibrarySort } from './libraryFilters';
import { LibraryItems } from './LibraryItems';
import { SelectionActions, SelectionTopBar } from './LibrarySelection';
import { ROUTES } from './medium';
import type { Medium } from './medium';
import { useCategories } from './useCategories';
import { useImportComics } from './useImportComics';
import { fetchContinue, useSetCategoryDisplay, useUpdateCategory } from './useLibraryActions';
import {
  useMangaLibrary,
  useMangaLiveEvents,
  useMangaUpdateStatus,
  useUpdateLibrary,
} from './useManga';
import { SourceIssue } from '../shell/SourceIssue';

const EMPTY = {
  manga: { icon: BookImage, title: 'manga.emptyTitle', body: 'manga.emptyLibrary' },
  novel: { icon: BookText, title: 'novel.emptyTitle', body: 'novel.emptyLibrary' },
  video: { icon: Clapperboard, title: 'library.videos.emptyTitle', body: 'library.videos.empty' },
} as const;

/**
 * Library → Manga, Novels or Videos: the titles you added, by category (Mihon) or by list
 * (CloudStream), with search, tri-state filters, sorts and four layouts. A long press starts
 * selecting titles to act on together. Videos start with Continue watching.
 */
export const MangaLibraryView: FC<{ medium?: Medium }> = ({ medium = 'manga' }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  useMangaLiveEvents();
  const library = useMangaLibrary(medium);
  const categories = useCategories(medium);
  const settings = useSettings();
  const updateSetting = useUpdateSetting();
  const display = withLibraryDisplay(settings.data?.libraryDisplay);
  const setDisplay = (value: LibraryDisplay) =>
    updateSetting.mutate({ key: 'libraryDisplay', value });
  const setCategoryDisplay = useSetCategoryDisplay(medium);
  const [tab, setTabState] = useState(() => loadTab(medium));
  const setTab = (next: string) => {
    setTabState(next);
    saveTab(medium, next);
  };
  const [filter, setFilter] = useState<LibraryFilter>(() => loadFilter(medium));
  useEffect(() => saveFilter(medium, filter), [medium, filter]);
  const [searching, setSearching] = useState(filter.query !== '');
  const [filtering, setFiltering] = useState(false);
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const lastPicked = useRef<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const updateStatus = useMangaUpdateStatus();
  const updateLibrary = useUpdateLibrary();
  const updateCategory = useUpdateCategory(medium);
  const updating = updateStatus.data?.running ?? false;
  const epub = useImportEpub();
  const comics = useImportComics();
  const video = medium === 'video';

  const entries = library.data;
  const categoryList = useMemo(() => categories.data ?? [], [categories.data]);
  const hasUncategorized = entries?.some((entry) => entry.categoryIds.length === 0) ?? false;

  // The tabs: categories (with All and, when needed, Uncategorized), lists, or none.
  const tabs = useMemo(() => {
    if (display.tabs === 'none') return [];
    if (display.tabs === 'lists') {
      return [
        { id: ALL_CATEGORIES, label: t('library.categories.all') },
        { id: FAVORITES_TAB, label: t('library.lists.favorites') },
        ...LISTS.map((list) => ({ id: `list:${list}`, label: t(listKey(list, video)) })),
      ];
    }
    if (categoryList.length === 0) return [];
    return [
      { id: ALL_CATEGORIES, label: t('library.categories.all') },
      ...categoryList.map((category) => ({ id: category.id, label: category.name })),
      ...(hasUncategorized
        ? [{ id: UNCATEGORIZED, label: t('library.categories.uncategorized') }]
        : []),
    ];
  }, [display.tabs, categoryList, hasUncategorized, t, video]);
  const activeTab = tabs.some((item) => item.id === tab) ? tab : ALL_CATEGORIES;

  // A category may keep its own layout and sort (Mihon's per-category settings).
  const category = categoryList.find((item) => item.id === activeTab) ?? null;
  const own = display.perCategory && category ? category.display : null;
  const mode: LibraryDisplayMode = own?.mode ?? display.mode;
  const sort = own
    ? { sort: own.sort as LibrarySort, descending: own.descending }
    : { sort: filter.sort, descending: filter.descending };

  const shown = useMemo(
    () =>
      entries
        ? applyLibraryFilter(entries, { ...filter, ...sort }, activeTab, display.pinsOnTop)
        : [],
    // eslint-disable-next-line react-hooks/exhaustive-deps -- `sort` is derived from these.
    [entries, filter, activeTab, display.pinsOnTop, sort.sort, sort.descending],
  );
  const tags = useMemo(() => (entries ? libraryTags(entries) : []), [entries]);
  const people = useMemo(() => (entries ? libraryPeople(entries) : []), [entries]);
  const selectedEntries = useMemo(
    () => (entries ?? []).filter((entry) => selected.has(entry.titleId)),
    [entries, selected],
  );
  const selecting = selected.size > 0;

  const onSort = (next: LibrarySort, descending: boolean, reshuffle: boolean) => {
    const seed = reshuffle ? newShuffleSeed() : filter.seed;
    if (display.perCategory && category) {
      setCategoryDisplay.mutate({
        categoryId: category.id,
        display: { mode, sort: next, descending },
      });
      if (reshuffle) setFilter({ ...filter, seed });
    } else {
      setFilter({ ...filter, sort: next, descending, seed });
    }
  };
  const onMode = (next: LibraryDisplayMode) => {
    if (display.perCategory && category) {
      setCategoryDisplay.mutate({
        categoryId: category.id,
        display: { mode: next, sort: sort.sort, descending: sort.descending },
      });
    } else {
      setDisplay({ ...display, mode: next });
    }
  };

  const toggle = (entry: MangaLibraryEntry) =>
    setSelected((current) => {
      const next = new Set(current);
      if (next.has(entry.titleId)) next.delete(entry.titleId);
      else next.add(entry.titleId);
      lastPicked.current = entry.titleId;
      return next;
    });
  // A long press adds the title, and everything between it and the last pick on this tab.
  const longPress = (entry: MangaLibraryEntry) =>
    setSelected((current) => {
      const next = new Set(current);
      const from = lastPicked.current;
      const start =
        from && current.has(from) ? shown.findIndex((item) => item.titleId === from) : -1;
      const end = shown.findIndex((item) => item.titleId === entry.titleId);
      if (start >= 0 && end >= 0) {
        const [low, high] = start < end ? [start, end] : [end, start];
        for (const item of shown.slice(low, high + 1)) next.add(item.titleId);
      } else {
        next.add(entry.titleId);
      }
      lastPicked.current = entry.titleId;
      return next;
    });
  const clearSelection = () => {
    setSelected(new Set());
    lastPicked.current = null;
  };
  useEffect(() => {
    if (!selecting) return;
    const onKey = (event: KeyboardEvent) => {
      if (event.key === 'Escape') clearSelection();
    };
    window.addEventListener('keydown', onKey);
    return () => window.removeEventListener('keydown', onKey);
  }, [selecting]);

  const onContinue = async (entry: MangaLibraryEntry) => {
    setNotice(null);
    try {
      if (video) {
        if (entry.sourceId === STREMIO_SOURCE) {
          const split = entry.remoteId.indexOf(':');
          await unwrapResult(
            commands.stremioWatch(
              entry.remoteId.slice(0, split),
              entry.remoteId.slice(split + 1),
              null,
            ),
          );
        } else {
          await unwrapResult(commands.videoSourceWatch(entry.titleId, null));
        }
        return;
      }
      const chapter = await fetchContinue(medium, entry.titleId);
      if (!chapter) {
        setNotice(t('library.noNextChapter'));
        return;
      }
      void navigate({ to: ROUTES[medium].read, search: { title: entry.titleId, chapter } });
    } catch (error) {
      setNotice(error instanceof Error ? error.message : String(error));
    }
  };

  const openRandom = () => {
    if (shown.length === 0) {
      setNotice(t('library.noRandom'));
      return;
    }
    const pick = shown[Math.floor(Math.random() * shown.length)];
    if (!pick) return;
    const link =
      video && pick.sourceId === STREMIO_SOURCE
        ? (() => {
            const split = pick.remoteId.indexOf(':');
            return {
              to: '/watch/title',
              search: { type: pick.remoteId.slice(0, split), id: pick.remoteId.slice(split + 1) },
            };
          })()
        : { to: ROUTES[medium].title, search: { source: pick.sourceId, id: pick.remoteId } };
    void navigate(link);
  };

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
  const menu: OverflowItem[] = [
    ...(medium === 'novel'
      ? [{ label: t('novel.importEpub'), icon: BookPlus, onSelect: epub.pick }]
      : [
          update,
          ...(category
            ? [
                {
                  label: t('library.updateCategory', { name: category.name }),
                  icon: RefreshCw,
                  onSelect: () => {
                    if (!updating) updateCategory.mutate(category.id);
                  },
                },
              ]
            : []),
          {
            label: t('manga.updates'),
            icon: Bell,
            to: '/updates',
            ...(video ? { search: { tab: 'episodes' } } : {}),
          },
        ]),
    ...(medium === 'manga'
      ? [{ label: t('manga.importComics'), icon: FileArchive, onSelect: comics.pick }]
      : []),
    { label: t('library.openRandom'), icon: Dices, onSelect: openRandom },
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
  const count = (id: string) => (entries ?? []).filter((entry) => inTab(entry, id)).length;
  const importing = epub.importBook.isPending || comics.importFiles.isPending;
  const importError = epub.importBook.error ?? comics.importFiles.error;

  return (
    <div data-testid={`${medium}-library`} className="flex flex-1 flex-col gap-4">
      {selecting ? (
        <SelectionTopBar
          count={selected.size}
          onSelectAll={() =>
            setSelected((current) => new Set([...current, ...shown.map((entry) => entry.titleId)]))
          }
          onInvert={() =>
            setSelected((current) => {
              const next = new Set(current);
              for (const entry of shown) {
                if (next.has(entry.titleId)) next.delete(entry.titleId);
                else next.add(entry.titleId);
              }
              return next;
            })
          }
          onClose={clearSelection}
        />
      ) : (
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
      )}
      {epub.input}
      {comics.input}
      {updating && updateStatus.data && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
          {t('manga.updating', {
            checked: updateStatus.data.checked,
            total: updateStatus.data.total,
          })}
        </p>
      )}
      {importing && (
        <p role="status" className="text-muted text-sm">
          {comics.progress
            ? t('manga.importingComics', {
                done: comics.progress.done + 1,
                total: comics.progress.total,
              })
            : t('novel.importing')}
        </p>
      )}
      {importError && <SourceIssue>{importError.message}</SourceIssue>}
      {notice && (
        <p role="status" className="text-muted text-sm">
          {notice}
        </p>
      )}
      {library.error && <SourceIssue>{library.error.message}</SourceIssue>}
      {video && !selecting && <ContinueWatchingRow />}
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
          {tabs.length > 0 && (
            <div
              role="tablist"
              aria-label={t(
                display.tabs === 'lists' ? 'library.lists.title' : 'library.categories.title',
              )}
              className="-mx-4 flex no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
            >
              {tabs.map((item) => (
                <button
                  key={item.id}
                  type="button"
                  role="tab"
                  aria-selected={activeTab === item.id}
                  onClick={() => setTab(item.id)}
                  className={`shrink-0 border-b-2 px-3 py-2 text-sm ${activeTab === item.id ? 'border-primary text-primary font-semibold' : 'text-muted border-transparent'}`}
                >
                  {item.label}
                  {display.showCounts && (
                    <>
                      {' '}
                      <span className="text-muted text-xs">{count(item.id)}</span>
                    </>
                  )}
                </button>
              ))}
            </div>
          )}
          {shown.length === 0 ? (
            <p className="text-muted text-sm">
              {filter.query || isFiltering(filter)
                ? t('library.filter.noMatches')
                : t('library.emptyTab')}
            </p>
          ) : (
            <LibraryItems
              entries={shown}
              medium={medium}
              mode={mode}
              display={display}
              selected={selected}
              onToggle={toggle}
              onLongPress={longPress}
              onContinue={(entry) => void onContinue(entry)}
            />
          )}
        </>
      )}
      {selecting && (
        <SelectionActions
          medium={medium}
          categories={categoryList}
          selected={selectedEntries}
          onDone={clearSelection}
        />
      )}
      {filtering && (
        <LibraryFilterSheet
          filter={filter}
          tags={tags}
          people={people}
          medium={medium}
          display={display}
          mode={mode}
          sort={sort}
          perCategoryName={display.perCategory && category ? category.name : null}
          onChange={setFilter}
          onSort={onSort}
          onMode={onMode}
          onDisplay={setDisplay}
          onClose={() => setFiltering(false)}
        />
      )}
    </div>
  );
};
