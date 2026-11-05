import type { CatalogEntry, RepoCatalog } from '@raygun/bindings';
import { Check, Info, LoaderCircle, X } from 'lucide-react';
import { useMemo, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { nativeName } from '../i18n/languages';
import { LetterAvatar } from '../shell/LetterAvatar';
import { ListSearchField } from '../shell/ListSearchField';
import { Sheet } from '../shell/Sheet';
import { NO_FILTER, catalogFacets, filterCatalog } from './catalogFilters';
import type { CatalogFilter, CatalogShow, ExtensionKind } from './catalogFilters';
import { AdultMark } from '../shell/AdultMark';
import { EntryInfo } from './EntryInfo';
import { entryKey, useInstallQueue } from './installQueue';
import type { InstallStatus } from './installQueue';
import { useCatalog } from './useExtensions';
import { SourceIssue } from '../shell/SourceIssue';

const SELECT = 'bg-surface min-w-0 flex-1 rounded-lg px-3 py-2 text-sm outline-none';

const languageName = (lang: string, multi: string) => (lang === 'multi' ? multi : nativeName(lang));

/**
 * Chips for the kinds present: each narrows the list to its kind, and pressed again shows every
 * kind (Installed and the catalog share them).
 */
export const KindChips: FC<{
  kinds: ExtensionKind[];
  kind: ExtensionKind | null;
  onChange: (kind: ExtensionKind | null) => void;
}> = ({ kinds, kind, onChange }) => {
  const { t } = useTranslation();
  if (kinds.length < 2) return null;
  return (
    <div
      role="group"
      aria-label={t('extensions.kindFilter')}
      className="-mx-4 flex gap-2 no-scrollbar overflow-x-auto px-4 lg:mx-0 lg:px-0"
    >
      {kinds.map((option) => {
        const selected = kind === option;
        return (
          <button
            key={option}
            type="button"
            aria-pressed={selected}
            onClick={() => onChange(selected ? null : option)}
            className={`shrink-0 rounded-full px-3 py-1.5 text-sm ${selected ? 'bg-primary/20 text-primary font-semibold' : 'bg-surface'}`}
          >
            {t(`extensions.kind.${option}`)}
          </button>
        );
      })}
    </div>
  );
};

/** Search, kind chips, and language and repository pickers over the whole catalog. */
const Filters: FC<{
  catalog: RepoCatalog[];
  filter: CatalogFilter;
  onChange: (filter: CatalogFilter) => void;
}> = ({ catalog, filter, onChange }) => {
  const { t } = useTranslation();
  const facets = useMemo(() => catalogFacets(catalog), [catalog]);

  return (
    <div className="flex flex-col gap-2">
      <ListSearchField
        value={filter.query}
        onChange={(query) => onChange({ ...filter, query })}
        label={t('extensions.search')}
        className="w-full"
      />
      <KindChips
        kinds={facets.kinds}
        kind={filter.kind}
        onChange={(kind) => onChange({ ...filter, kind })}
      />
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

/** Another app's plugin Raygun lists but can't run (Kodi's add-ons) is never installable. */
const installable = (entry: CatalogEntry) =>
  !entry.unsupported && (entry.state === 'available' || entry.state === 'updateAvailable');

/** Install or Update, at once (M46.7): what it may access is behind the info button beside it. */
const EntryAction: FC<{
  entry: CatalogEntry;
  status: InstallStatus | undefined;
  onInstall: () => void;
  onCancel: () => void;
}> = ({ entry, status, onInstall, onCancel }) => {
  const { t } = useTranslation();
  if (entry.unsupported) {
    return (
      <span className="text-muted text-sm whitespace-nowrap">{t('extensions.cantInstall')}</span>
    );
  }
  if (status?.state === 'done') {
    return (
      <span
        role="status"
        className="text-primary flex items-center gap-1.5 px-2 text-sm whitespace-nowrap"
      >
        <Check aria-hidden size={16} />
        {t('extensions.installedNow')}
      </span>
    );
  }
  if (status?.state === 'queued' || status?.state === 'installing') {
    return (
      <span
        role="status"
        className="text-muted flex items-center gap-1.5 px-2 text-sm whitespace-nowrap"
      >
        <LoaderCircle
          aria-hidden
          size={16}
          className={status.state === 'installing' ? 'animate-spin' : ''}
        />
        {t(status.state === 'installing' ? 'extensions.installing' : 'extensions.queued')}
        {status.state === 'queued' && (
          <button
            type="button"
            aria-label={t('extensions.cancelInstall', { name: entry.name })}
            onClick={onCancel}
            className="hover:text-foreground grid size-7 place-items-center rounded-full"
          >
            <X aria-hidden size={14} />
          </button>
        )}
      </span>
    );
  }
  switch (entry.state) {
    case 'available':
    case 'updateAvailable':
      return (
        <button
          type="button"
          onClick={onInstall}
          className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold whitespace-nowrap"
        >
          {status?.state === 'failed'
            ? t('extensions.retry')
            : t(entry.state === 'available' ? 'extensions.install' : 'extensions.update')}
        </button>
      );
    case 'installed':
      return <span className="text-muted text-sm">{t('extensions.upToDate')}</span>;
    case 'newerInstalled':
      return <span className="text-muted text-sm">{t('extensions.newerInstalled')}</span>;
  }
};

/**
 * Browse → Extensions' catalogue: every repository's extensions (All), the ones not installed
 * yet, or the ones with an update, with filters over them.
 */
export const BrowseTab: FC<{ show?: CatalogShow }> = ({ show = 'all' }) => {
  const { t } = useTranslation();
  const catalog = useCatalog();
  const queue = useInstallQueue();
  const [info, setInfo] = useState<{ entry: CatalogEntry; anchor: HTMLElement } | null>(null);
  const [selected, setSelected] = useState<ReadonlySet<string>>(new Set());
  const [filter, setFilter] = useState<CatalogFilter>(NO_FILTER);
  const error = catalog.error;
  const listings = useMemo(
    () => (catalog.data ? filterCatalog(catalog.data, filter, show) : undefined),
    [catalog.data, filter, show],
  );
  const shown = useMemo(() => listings?.flatMap((listing) => listing.entries) ?? [], [listings]);
  // Entries that can still be installed, among those shown; a selection keeps only these.
  const choosable = shown.filter(
    (entry) => installable(entry) && queue.status[entryKey(entry)]?.state !== 'installing',
  );
  const chosen = choosable.filter((entry) => selected.has(entryKey(entry)));
  const updates = shown.filter((entry) => entry.state === 'updateAvailable');
  const repoNames = new Map(catalog.data?.map((listing) => [listing.repo.id, listing.repo.name]));

  const toggle = (entry: CatalogEntry) =>
    setSelected((current) => {
      const next = new Set(current);
      const key = entryKey(entry);
      if (next.has(key)) next.delete(key);
      else next.add(key);
      return next;
    });
  const installChosen = () => {
    queue.install(chosen);
    setSelected(new Set());
  };

  return (
    <div data-testid="browse-tab" className="flex flex-col gap-4">
      {error && (
        <SourceIssue>{t('common.coreUnavailable', { message: error.message })}</SourceIssue>
      )}
      {catalog.data?.length === 0 && (
        <p className="text-muted text-sm">{t('extensions.noRepos')}</p>
      )}
      {catalog.data && catalog.data.length > 0 && (
        <Filters catalog={catalog.data} filter={filter} onChange={setFilter} />
      )}
      {chosen.length > 0 ? (
        <div
          role="toolbar"
          aria-label={t('extensions.selection')}
          className="bg-surface motion-pop sticky top-0 z-10 flex flex-wrap items-center gap-2 rounded-2xl px-3 py-2 shadow-lg shadow-black/20"
        >
          <span className="flex-1 text-sm font-medium">
            {t('extensions.selected', { count: chosen.length })}
          </span>
          {chosen.length < choosable.length && (
            <button
              type="button"
              onClick={() => setSelected(new Set(choosable.map(entryKey)))}
              className="text-muted hover:text-foreground rounded-full px-3 py-1.5 text-sm"
            >
              {t('extensions.selectAll', { count: choosable.length })}
            </button>
          )}
          <button
            type="button"
            onClick={() => setSelected(new Set())}
            className="text-muted hover:text-foreground rounded-full px-3 py-1.5 text-sm"
          >
            {t('extensions.clearSelection')}
          </button>
          <button
            type="button"
            onClick={installChosen}
            className="bg-primary text-background rounded-full px-4 py-1.5 text-sm font-semibold"
          >
            {t('extensions.installSelected', { count: chosen.length })}
          </button>
        </div>
      ) : (
        (updates.length > 1 || (show === 'updates' && updates.length > 0)) && (
          <button
            type="button"
            onClick={() => queue.install(updates)}
            className="bg-surface self-start rounded-full px-4 py-1.5 text-sm font-semibold"
          >
            {t('extensions.updateAll', { count: updates.length })}
          </button>
        )
      )}
      {listings?.length === 0 && catalog.data && catalog.data.length > 0 && (
        <p className="text-muted text-sm">
          {show === 'updates' && filter === NO_FILTER
            ? t('extensions.allUpToDate')
            : show === 'notInstalled' && filter === NO_FILTER
              ? t('extensions.allInstalled')
              : t('extensions.noMatches')}
        </p>
      )}
      {listings?.map((listing) => (
        <section key={listing.repo.id} data-testid="repo-catalog" className="flex flex-col gap-2">
          <h2 className="text-primary px-1 text-sm font-semibold">{listing.repo.name}</h2>
          {listing.error && <SourceIssue source={listing.repo.name}>{listing.error}</SourceIssue>}
          {!listing.error && listing.entries.length === 0 && (
            <p className="text-muted text-sm">{t('extensions.repoEmpty')}</p>
          )}
          <ul className="flex flex-col">
            {listing.entries.map((entry) => {
              const key = entryKey(entry);
              const status = queue.status[key];
              const canChoose = installable(entry) && status?.state !== 'installing';
              return (
                <li
                  key={entry.id}
                  data-testid="catalog-entry"
                  className="flex items-center gap-3 px-1 py-2.5"
                >
                  {canChoose ? (
                    <input
                      type="checkbox"
                      aria-label={t('extensions.select', { name: entry.name })}
                      checked={selected.has(key)}
                      onChange={() => toggle(entry)}
                      className="accent-primary size-4 shrink-0 cursor-pointer"
                    />
                  ) : (
                    <span aria-hidden className="size-4 shrink-0" />
                  )}
                  <LetterAvatar name={entry.name} logo={entry.icon} />
                  <span className="flex min-w-0 flex-1 flex-col gap-0.5">
                    <span className="flex flex-wrap items-baseline gap-x-2">
                      <span className="font-medium">{entry.name}</span>
                      {entry.adult && <AdultMark />}
                      {entry.nuclear && (
                        <span className="bg-background text-muted rounded px-1.5 py-0.5 text-3xs font-semibold tracking-wide uppercase">
                          {t('extensions.nuclearBadge')}
                        </span>
                      )}
                      <span className="text-muted text-xs">
                        {entry.state !== 'updateAvailable' || !entry.installedVersion
                          ? entry.version
                          : entry.installedVersion === entry.version
                            ? t('extensions.newerConversion', { version: entry.version })
                            : `${entry.installedVersion} → ${entry.version}`}
                      </span>
                      {entry.lang && (
                        <span className="text-muted text-xs">
                          {languageName(entry.lang, t('extensions.multiLanguage'))}
                        </span>
                      )}
                    </span>
                    {entry.description && (
                      <span className="text-muted line-clamp-2 text-xs">{entry.description}</span>
                    )}
                    {entry.unsupported && (
                      <span className="text-muted text-xs">{entry.unsupported}</span>
                    )}
                    {status?.state === 'failed' && (
                      <SourceIssue compact title={t('extensions.installFailed')} className="mt-1">
                        {status.message}
                      </SourceIssue>
                    )}
                  </span>
                  <button
                    type="button"
                    aria-label={t('extensions.about', { name: entry.name })}
                    aria-haspopup="dialog"
                    aria-expanded={info?.entry === entry}
                    onClick={(event) =>
                      setInfo(info?.entry === entry ? null : { entry, anchor: event.currentTarget })
                    }
                    className="text-muted hover:text-foreground grid size-9 shrink-0 place-items-center rounded-full"
                  >
                    <Info aria-hidden size={18} />
                  </button>
                  <EntryAction
                    entry={entry}
                    status={status}
                    onInstall={() => queue.install([entry])}
                    onCancel={() => queue.cancel(entry)}
                  />
                </li>
              );
            })}
          </ul>
        </section>
      ))}
      {info && (
        <Sheet
          title={t('extensions.aboutTitle')}
          anchor={info.anchor}
          onClose={() => setInfo(null)}
        >
          <EntryInfo entry={info.entry} repoName={repoNames.get(info.entry.repoId) ?? ''} />
        </Sheet>
      )}
    </div>
  );
};
