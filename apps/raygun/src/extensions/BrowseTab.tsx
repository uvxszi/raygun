import type { CatalogEntry, RepoCatalog } from '@raygun/bindings';
import { RefreshCw, Search } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { nativeName } from '../i18n/languages';
import { NO_FILTER, catalogFacets, filterCatalog } from './catalogFilters';
import type { CatalogFilter, ExtensionKind } from './catalogFilters';
import { InstallSheet } from './InstallSheet';
import { useCatalog, useRefreshCatalog } from './useExtensions';

const SELECT = 'bg-surface min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none';

const languageName = (lang: string, multi: string) => (lang === 'multi' ? multi : nativeName(lang));

/** Search, kind chips, and language and repository pickers over the whole catalog. */
const Filters: FC<{
  catalog: RepoCatalog[];
  filter: CatalogFilter;
  onChange: (filter: CatalogFilter) => void;
}> = ({ catalog, filter, onChange }) => {
  const { t } = useTranslation();
  const facets = useMemo(() => catalogFacets(catalog), [catalog]);
  const chip = (kind: ExtensionKind | null, label: string) => {
    const selected = filter.kind === kind;
    return (
      <button
        key={kind ?? 'all'}
        type="button"
        aria-pressed={selected}
        onClick={() => onChange({ ...filter, kind })}
        className={`shrink-0 rounded-full px-3 py-1.5 text-sm ${selected ? 'bg-primary text-background font-semibold' : 'bg-surface'}`}
      >
        {label}
      </button>
    );
  };

  return (
    <div className="flex flex-col gap-2">
      <label className="bg-surface flex items-center gap-2 rounded-lg px-3 py-2">
        <Search aria-hidden size={16} className="text-muted" />
        <input
          type="search"
          value={filter.query}
          onChange={(event) => onChange({ ...filter, query: event.target.value })}
          placeholder={t('extensions.searchPlaceholder')}
          aria-label={t('extensions.search')}
          className="placeholder:text-muted min-w-0 flex-1 bg-transparent text-sm outline-none"
        />
      </label>
      <div
        role="group"
        aria-label={t('extensions.kindFilter')}
        className="flex gap-2 overflow-x-auto"
      >
        {chip(null, t('extensions.kind.all'))}
        {facets.kinds.map((kind) => chip(kind, t(`extensions.kind.${kind}`)))}
      </div>
      <div className="flex gap-2">
        {facets.langs.length > 0 && (
          <select
            aria-label={t('extensions.languageFilter')}
            value={filter.lang ?? ''}
            onChange={(event) => onChange({ ...filter, lang: event.target.value || null })}
            className={SELECT}
          >
            <option value="">{t('extensions.anyLanguage')}</option>
            {facets.langs.map((lang) => (
              <option key={lang} value={lang}>
                {languageName(lang, t('extensions.multiLanguage'))}
              </option>
            ))}
          </select>
        )}
        {catalog.length > 1 && (
          <select
            aria-label={t('extensions.repoFilter')}
            value={filter.repoId ?? ''}
            onChange={(event) => onChange({ ...filter, repoId: event.target.value || null })}
            className={SELECT}
          >
            <option value="">{t('extensions.allRepos')}</option>
            {catalog.map((listing) => (
              <option key={listing.repo.id} value={listing.repo.id}>
                {listing.repo.name}
              </option>
            ))}
          </select>
        )}
      </div>
    </div>
  );
};

const EntryAction: FC<{ entry: CatalogEntry; onInstall: () => void }> = ({ entry, onInstall }) => {
  const { t } = useTranslation();
  switch (entry.state) {
    case 'available':
      return (
        <button
          type="button"
          onClick={onInstall}
          className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold"
        >
          {t('extensions.install')}
        </button>
      );
    case 'updateAvailable':
      return (
        <button
          type="button"
          onClick={onInstall}
          className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold"
        >
          {t('extensions.update')}
        </button>
      );
    case 'installed':
      return <span className="text-muted text-sm">{t('extensions.upToDate')}</span>;
    case 'newerInstalled':
      return <span className="text-muted text-sm">{t('extensions.newerInstalled')}</span>;
  }
};

export const BrowseTab: FC = () => {
  const { t } = useTranslation();
  const catalog = useCatalog();
  const refresh = useRefreshCatalog();
  const [installing, setInstalling] = useState<CatalogEntry | null>(null);
  const [filter, setFilter] = useState<CatalogFilter>(NO_FILTER);
  const error = catalog.error ?? refresh.error;
  const listings = useMemo(
    () => (catalog.data ? filterCatalog(catalog.data, filter) : undefined),
    [catalog.data, filter],
  );

  return (
    <div data-testid="browse-tab" className="flex flex-col gap-4">
      <div className="flex justify-end">
        <button
          type="button"
          onClick={() => refresh.mutate()}
          disabled={refresh.isPending}
          className="text-muted flex items-center gap-1.5 text-sm disabled:opacity-50"
        >
          <RefreshCw
            aria-hidden
            size={14}
            className={refresh.isPending || catalog.isFetching ? 'animate-spin' : ''}
          />
          {t('extensions.refresh')}
        </button>
      </div>
      {error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {t('common.coreUnavailable', { message: error.message })}
        </p>
      )}
      {catalog.data?.length === 0 && (
        <p className="text-muted text-sm">{t('extensions.noRepos')}</p>
      )}
      {catalog.data && catalog.data.length > 0 && (
        <Filters catalog={catalog.data} filter={filter} onChange={setFilter} />
      )}
      {listings?.length === 0 && catalog.data && catalog.data.length > 0 && (
        <p className="text-muted text-sm">{t('extensions.noMatches')}</p>
      )}
      {listings?.map((listing) => (
        <section key={listing.repo.id} data-testid="repo-catalog" className="flex flex-col gap-2">
          <h2 className="text-muted text-sm font-medium">{listing.repo.name}</h2>
          {listing.error && (
            <p role="alert" className="text-primary text-sm wrap-anywhere">
              {t('extensions.repoFailed', { message: listing.error })}
            </p>
          )}
          {!listing.error && listing.entries.length === 0 && (
            <p className="text-muted text-sm">{t('extensions.repoEmpty')}</p>
          )}
          <ul className="bg-surface flex flex-col rounded-xl">
            {listing.entries.map((entry) => (
              <li
                key={entry.id}
                data-testid="catalog-entry"
                className="flex items-center gap-3 px-4 py-3"
              >
                <span className="flex min-w-0 flex-1 flex-col">
                  <span className="flex flex-wrap items-baseline gap-x-2">
                    <span className="font-medium">{entry.name}</span>
                    {entry.nuclear && (
                      <span className="bg-background text-muted rounded px-1.5 py-0.5 text-[10px] font-semibold tracking-wide uppercase">
                        {t('extensions.nuclearBadge')}
                      </span>
                    )}
                    <span className="text-muted text-xs">
                      {entry.state === 'updateAvailable' && entry.installedVersion
                        ? `${entry.installedVersion} → ${entry.version}`
                        : entry.version}
                    </span>
                    {entry.lang && (
                      <span className="text-muted text-xs">
                        {languageName(entry.lang, t('extensions.multiLanguage'))}
                      </span>
                    )}
                  </span>
                  {entry.description && (
                    <span className="text-muted text-xs">{entry.description}</span>
                  )}
                </span>
                <EntryAction entry={entry} onInstall={() => setInstalling(entry)} />
              </li>
            ))}
          </ul>
        </section>
      ))}
      {installing && <InstallSheet entry={installing} onClose={() => setInstalling(null)} />}
    </div>
  );
};
