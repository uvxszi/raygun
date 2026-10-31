import type { CatalogEntry } from '@raygun/bindings';
import { RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { InstallSheet } from './InstallSheet';
import { useCatalog, useRefreshCatalog } from './useExtensions';

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
  const error = catalog.error ?? refresh.error;

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
      {catalog.data?.map((listing) => (
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
                    <span className="text-muted text-xs">
                      {entry.state === 'updateAvailable' && entry.installedVersion
                        ? `${entry.installedVersion} → ${entry.version}`
                        : entry.version}
                    </span>
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
