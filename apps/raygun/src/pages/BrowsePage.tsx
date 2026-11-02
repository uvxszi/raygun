import type { MangaSource } from '@raygun/bindings';
import { useIsFetching } from '@tanstack/react-query';
import { Link, useNavigate, useSearch } from '@tanstack/react-router';
import { LayoutList, LoaderCircle, Package, Puzzle, RefreshCw, Search } from 'lucide-react';
import { Fragment, useEffect, useState } from 'react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { BrowseTab as AvailableExtensions } from '../extensions/BrowseTab';
import { InstalledTab } from '../extensions/InstalledTab';
import { useExtensions, useRefreshCatalog, useReloadExtensions } from '../extensions/useExtensions';
import { useLayout } from '../layout/useLayout';
import { useMangaSources } from '../manga/useManga';
import { LetterAvatar } from '../shell/LetterAvatar';
import { OverflowMenu } from '../shell/OverflowMenu';
import { Tabs } from '../shell/Tabs';
import { SearchPanel } from '../search/SearchPanel';
import { SourceMenu } from '../search/SourceMenu';
import { WebsitesSection } from '../websites/WebsitesSection';
import type { SearchType } from '../search/searchTypes';
import { useStremioAddons } from '../watch/useStremio';
import { useVideoSources } from '../watch/useVideoSources';

export type BrowseSection = 'search' | 'sources' | 'extensions';
export type ExtensionList = 'installed' | 'available';

type SourceRow = {
  key: string;
  name: string;
  detail: string;
  logo?: string | null;
  to: string;
  search: Record<string, unknown>;
  /** Where "Latest" leads, for sources that list new titles. */
  latest?: { to: string; search: Record<string, unknown> };
  /** An extension's source: its ⋯ menu, and whether the user turned it off (M46.8). */
  source?: { id: string; extensionId: string; enabled: boolean };
};

const SourceSection: FC<{ title: string; rows: SourceRow[] }> = ({ title, rows }) => {
  const { t } = useTranslation();
  if (rows.length === 0) return null;
  return (
    <section aria-label={title} className="flex flex-col">
      <h2 className="text-muted px-1 pt-2 pb-1 text-sm font-medium">{title}</h2>
      <ul className="flex flex-col">
        {rows.map((row) => (
          <li key={row.key} className="flex items-center gap-1">
            <Link
              to={row.to}
              search={row.search}
              aria-label={row.name}
              className={`hover:bg-surface flex min-w-0 flex-1 items-center gap-4 rounded-xl px-1 py-2.5 ${row.source?.enabled === false ? 'opacity-50' : ''}`}
            >
              <LetterAvatar name={row.name} logo={row.logo} />
              <span className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{row.name}</span>
                <span className="text-muted truncate text-xs">
                  {row.source?.enabled === false
                    ? t('sources.disabledNote', { detail: row.detail })
                    : row.detail}
                </span>
              </span>
            </Link>
            {row.latest && (
              <Link
                to={row.latest.to}
                search={row.latest.search}
                aria-label={t('browse.latestOf', { name: row.name })}
                className="text-primary shrink-0 rounded-full px-3 py-2 text-sm font-medium"
              >
                {t('manga.list.latest')}
              </Link>
            )}
            {row.source && (
              <SourceMenu
                sourceId={row.source.id}
                extensionId={row.source.extensionId}
                name={row.name}
              />
            )}
          </li>
        ))}
      </ul>
    </section>
  );
};

/** Browse → Sources: every catalogue to browse, by kind (Aniyomi's Browse). */
const SourcesList: FC = () => {
  const { t } = useTranslation();
  const layout = useLayout();
  const extensions = useExtensions();
  const addons = useStremioAddons();
  const video = useVideoSources();
  const manga = useMangaSources('manga');
  const novel = useMangaSources('novel');
  const extensionOf = (source: MangaSource) =>
    extensions.data?.find((extension) => extension.id === source.extensionId);
  const catalogue = (sources: MangaSource[] | undefined, route: string): SourceRow[] =>
    (sources ?? []).map((source) => ({
      key: source.id,
      name: source.name,
      detail: extensionOf(source)?.name ?? source.extensionId,
      // A source shows its extension's icon (M46.2).
      logo: extensionOf(source)?.icon,
      to: route,
      search: { id: source.id },
      latest: { to: route, search: { id: source.id, mode: 'latest' } },
      source: { id: source.id, extensionId: source.extensionId, enabled: source.enabled },
    }));

  const videoRows: SourceRow[] = [
    ...(addons.data ?? [])
      .filter((addon) => addon.catalogs > 0 && !addon.error)
      .map((addon) => ({
        key: addon.id,
        name: addon.name,
        detail: t('browse.stremioAddon'),
        logo: addon.logo,
        to: '/watch/addon',
        search: { id: addon.id },
      })),
    ...catalogue(video.data, '/watch/source'),
  ];
  const loading = addons.isPending || video.isPending || manga.isPending || novel.isPending;
  const empty =
    !loading &&
    videoRows.length === 0 &&
    (manga.data?.length ?? 0) === 0 &&
    (novel.data?.length ?? 0) === 0;

  // The sections in the user's order (M46.5).
  const sections: Record<string, ReactNode> = {
    video: <SourceSection title={t('browse.kind.video')} rows={videoRows} />,
    manga: (
      <SourceSection
        title={t('library.media.manga')}
        rows={catalogue(manga.data, '/manga/source')}
      />
    ),
    novel: (
      <SourceSection
        title={t('library.media.novel')}
        rows={catalogue(novel.data, '/novel/source')}
      />
    ),
    websites: <WebsitesSection />,
  };
  return (
    <div data-testid="browse-sources" className="flex flex-col gap-2">
      {empty && <p className="text-muted px-1 text-sm">{t('browse.noSources')}</p>}
      {layout.browseSections
        .filter((item) => item.visible)
        .map((item) => (
          <Fragment key={item.id}>{sections[item.id]}</Fragment>
        ))}
    </div>
  );
};

const BROWSE_TAB_LABEL: Record<BrowseSection, string> = {
  search: 'search.title',
  sources: 'browse.sources',
  extensions: 'extensions.title',
};

/** Wait for a pause in typing before searching, so each keystroke doesn't hit the network. */
export const SEARCH_DEBOUNCE_MS = 400;

/**
 * Browse: where content is found (Mihon's Browse with CloudStream's search). The search bar is the
 * header; under it Search (every kind of media, or one with its sources' filters), Sources (the
 * catalogues by kind) and Extensions. Typing switches to Search. The library never browses.
 */
export const BrowsePage: FC = () => {
  const { t } = useTranslation();
  const current = useSearch({ from: '/browse' });
  const layout = useLayout();
  // Without a tab in the address, Sources, or the first tab the user shows (M46.5).
  const shownTabs = layout.browseTabs
    .filter((item) => item.visible)
    .map((item) => item.id)
    .filter(
      (id): id is BrowseSection => id === 'search' || id === 'sources' || id === 'extensions',
    );
  const firstTab: BrowseSection = shownTabs.includes('sources')
    ? 'sources'
    : (shownTabs[0] ?? 'search');
  const { tab = firstTab, list = 'installed', q = '', type = 'all' } = current;
  const navigate = useNavigate({ from: '/browse' });
  const reload = useReloadExtensions();
  const refresh = useRefreshCatalog();
  const fetching = useIsFetching() > 0;
  const [input, setInput] = useState(q);

  // The query lives in the URL, so Back to this page shows the same results.
  useEffect(() => {
    if (input.trim() === q) return;
    const timer = setTimeout(() => {
      const next = input.trim();
      void navigate({ search: { ...current, q: next || undefined }, replace: true });
    }, SEARCH_DEBOUNCE_MS);
    return () => clearTimeout(timer);
  }, [input, q, current, navigate]);

  const onInput = (value: string) => {
    setInput(value);
    // Typing searches: the Search tab shows the results.
    if (value.trim() && tab !== 'search') {
      void navigate({ search: { ...current, tab: 'search' }, replace: true });
    }
  };
  const setType = (next: SearchType) =>
    void navigate({
      search: { ...current, tab: 'search', type: next === 'all' ? undefined : next },
      replace: true,
    });
  const searchParams = {
    tab: 'search',
    ...(q ? { q } : {}),
    ...(type === 'all' ? {} : { type }),
  };

  return (
    <section data-testid="browse-page" className="flex flex-col gap-3">
      <header className="bg-background sticky top-0 z-30 -mx-4 flex min-h-16 items-center gap-1 px-3 py-2 lg:-mx-10 lg:px-8">
        <h1 className="sr-only">{t('browse.title')}</h1>
        <label className="bg-surface flex min-w-0 flex-1 items-center gap-3 rounded-full px-5 py-3">
          <Search aria-hidden size={20} className="text-muted shrink-0" />
          <input
            type="search"
            enterKeyHint="search"
            value={input}
            onChange={(event) => onInput(event.target.value)}
            placeholder={t(`search.placeholderFor.${type}`)}
            aria-label={t('search.title')}
            className="placeholder:text-muted min-w-0 flex-1 bg-transparent outline-none"
          />
          {fetching && q && tab === 'search' && (
            <LoaderCircle
              aria-label={t('search.searching')}
              role="status"
              size={20}
              className="text-muted shrink-0 animate-spin"
            />
          )}
        </label>
        <OverflowMenu
          items={[
            ...(tab === 'extensions' && list === 'installed'
              ? [
                  {
                    label: t('extensions.reload'),
                    icon: RefreshCw,
                    onSelect: () => reload.mutate(),
                  },
                ]
              : []),
            ...(tab === 'extensions' && list === 'available'
              ? [
                  {
                    label: t('extensions.refresh'),
                    icon: RefreshCw,
                    onSelect: () => refresh.mutate(),
                  },
                ]
              : []),
            { label: t('watch.addons'), icon: Puzzle, to: '/watch/addons' },
            { label: t('browse.repos'), icon: Package, to: '/extensions/repos' },
            {
              label: t('layout.editBrowse'),
              icon: LayoutList,
              to: '/settings/layout/browseSections',
            },
          ]}
        />
      </header>
      <Tabs
        label={t('browse.title')}
        active={tab}
        items={shownTabs.concat(shownTabs.includes(tab) ? [] : [tab]).map((id) => ({
          id,
          label: t(BROWSE_TAB_LABEL[id]),
          to: '/browse',
          search: id === 'search' ? searchParams : id === firstTab ? {} : { tab: id },
          testId: `browse-tab-${id}`,
        }))}
      />
      {(reload.error ?? refresh.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(reload.error ?? refresh.error)?.message}
        </p>
      )}
      {(reload.isPending || refresh.isPending) && (
        <p role="status" className="text-muted flex items-center gap-2 text-sm">
          <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
          {t(reload.isPending ? 'extensions.reload' : 'extensions.refresh')}
        </p>
      )}
      {tab === 'search' && <SearchPanel query={q} type={type} onType={setType} />}
      {tab === 'sources' && <SourcesList />}
      {tab === 'extensions' && (
        <div className="flex flex-col gap-4">
          <nav className="flex gap-2" aria-label={t('extensions.tabs')}>
            {(['installed', 'available'] as const).map((option) => (
              <Link
                key={option}
                to="/browse"
                search={{ tab: 'extensions', ...(option === 'installed' ? {} : { list: option }) }}
                replace
                data-testid={`extensions-tab-${option === 'installed' ? 'installed' : 'browse'}`}
                aria-current={option === list ? 'page' : undefined}
                className={`rounded-full px-4 py-1.5 text-sm ${option === list ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
              >
                {t(`extensions.tab.${option === 'installed' ? 'installed' : 'browse'}`)}
              </Link>
            ))}
          </nav>
          {list === 'installed' ? <InstalledTab /> : <AvailableExtensions />}
        </div>
      )}
    </section>
  );
};
