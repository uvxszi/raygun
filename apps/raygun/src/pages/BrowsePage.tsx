import type { MangaSource } from '@raygun/bindings';
import { Link, useSearch } from '@tanstack/react-router';
import { Package, Puzzle, RefreshCw, Search } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { BrowseTab as AvailableExtensions } from '../extensions/BrowseTab';
import { InstalledTab } from '../extensions/InstalledTab';
import { useExtensions, useRefreshCatalog, useReloadExtensions } from '../extensions/useExtensions';
import { useMangaSources } from '../manga/useManga';
import { LetterAvatar } from '../shell/LetterAvatar';
import { OverflowMenu } from '../shell/OverflowMenu';
import { Tabs } from '../shell/Tabs';
import { TopBar, TopBarAction } from '../shell/TopBar';
import { useStremioAddons } from '../watch/useStremio';
import { useVideoSources } from '../watch/useVideoSources';

export type BrowseSection = 'sources' | 'extensions';
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
              className="hover:bg-surface flex min-w-0 flex-1 items-center gap-4 rounded-xl px-1 py-2.5"
            >
              <LetterAvatar name={row.name} logo={row.logo} />
              <span className="flex min-w-0 flex-col">
                <span className="truncate font-medium">{row.name}</span>
                <span className="text-muted truncate text-xs">{row.detail}</span>
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
          </li>
        ))}
      </ul>
    </section>
  );
};

/** Browse → Sources: every catalogue to browse, by kind (Aniyomi's Browse). */
const SourcesList: FC = () => {
  const { t } = useTranslation();
  const extensions = useExtensions();
  const addons = useStremioAddons();
  const video = useVideoSources();
  const manga = useMangaSources('manga');
  const novel = useMangaSources('novel');
  const extensionName = (source: MangaSource) =>
    extensions.data?.find((extension) => extension.id === source.extensionId)?.name ??
    source.extensionId;
  const catalogue = (sources: MangaSource[] | undefined, route: string): SourceRow[] =>
    (sources ?? []).map((source) => ({
      key: source.id,
      name: source.name,
      detail: extensionName(source),
      to: route,
      search: { id: source.id },
      latest: { to: route, search: { id: source.id, mode: 'latest' } },
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

  return (
    <div data-testid="browse-sources" className="flex flex-col gap-2">
      <SourceSection title={t('browse.kind.video')} rows={videoRows} />
      <SourceSection
        title={t('library.media.manga')}
        rows={catalogue(manga.data, '/manga/source')}
      />
      <SourceSection
        title={t('library.media.novel')}
        rows={catalogue(novel.data, '/novel/source')}
      />
      {empty && <p className="text-muted px-1 text-sm">{t('browse.noSources')}</p>}
    </div>
  );
};

/** Browse: catalogues to explore and the extensions that provide them. */
export const BrowsePage: FC = () => {
  const { t } = useTranslation();
  const { tab = 'sources', list = 'installed' } = useSearch({ from: '/browse' });
  const reload = useReloadExtensions();
  const refresh = useRefreshCatalog();

  return (
    <section data-testid="browse-page" className="flex flex-col gap-3">
      <TopBar
        title={t('browse.title')}
        actions={
          <>
            {tab === 'extensions' && list === 'installed' && (
              <TopBarAction
                label={t('extensions.reload')}
                icon={RefreshCw}
                disabled={reload.isPending}
                onClick={() => reload.mutate()}
              />
            )}
            {tab === 'extensions' && list === 'available' && (
              <TopBarAction
                label={t('extensions.refresh')}
                icon={RefreshCw}
                disabled={refresh.isPending}
                onClick={() => refresh.mutate()}
              />
            )}
            <TopBarAction label={t('search.title')} icon={Search} to="/search" />
            <OverflowMenu
              items={[
                { label: t('watch.addons'), icon: Puzzle, to: '/watch/addons' },
                { label: t('browse.repos'), icon: Package, to: '/extensions/repos' },
              ]}
            />
          </>
        }
      />
      <Tabs
        label={t('browse.title')}
        active={tab}
        items={[
          {
            id: 'sources',
            label: t('browse.sources'),
            to: '/browse',
            search: {},
            testId: 'browse-tab-sources',
          },
          {
            id: 'extensions',
            label: t('extensions.title'),
            to: '/browse',
            search: { tab: 'extensions' },
            testId: 'browse-tab-extensions',
          },
        ]}
      />
      {(reload.error ?? refresh.error) && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {(reload.error ?? refresh.error)?.message}
        </p>
      )}
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
