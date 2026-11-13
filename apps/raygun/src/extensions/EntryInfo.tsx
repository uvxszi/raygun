import type { CatalogEntry } from '@raygun/bindings';
import { Globe, RefreshCw } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AdultMark } from '../shell/AdultMark';
import { SourceIssue } from '../shell/SourceIssue';
import { isNotForRaygun } from './catalogFilters';
import { hostLabel } from './hostLabel';
import { useInstallQueue } from './installQueue';
import { RunsOn } from './RunsOn';
import { useRefreshRepo } from './useExtensions';

/**
 * What a repository says about an extension (M46.7): version, author, what it does and the
 * network access it asks for. The Browse tab's info button shows it beside the Install button,
 * which installs at once.
 */
export const EntryInfo: FC<{ entry: CatalogEntry; repoName: string }> = ({ entry, repoName }) => {
  const { t } = useTranslation();
  return (
    <div data-testid="entry-info" className="flex flex-col gap-3 px-3 pb-2 text-sm">
      <div className="flex flex-col gap-0.5">
        <span className="flex items-center gap-2">
          <span className="text-base font-semibold">{entry.name}</span>
          {entry.adult && <AdultMark />}
        </span>
        <span className="text-muted">
          {[entry.version, entry.author, repoName].filter(Boolean).join(' · ')}
        </span>
      </div>
      {entry.description && <p>{entry.description}</p>}
      {isNotForRaygun(entry) && entry.unsupported && (
        <p className="text-muted">
          {t('extensions.notForRaygunWhy', { reason: entry.unsupported })}
        </p>
      )}
      <div className="flex flex-col gap-1.5">
        <h3 className="font-medium">{t('extensions.permissionsTitle')}</h3>
        {entry.nuclear ? (
          <p className="text-muted">{t('extensions.nuclearNetwork')}</p>
        ) : entry.net.length > 0 ? (
          <ul data-testid="install-permissions" className="flex flex-col gap-1">
            {entry.net.map((host) => (
              <li key={host} className="text-muted flex items-center gap-2 wrap-anywhere">
                <Globe aria-hidden size={14} className="shrink-0" />
                {hostLabel(host, t)}
              </li>
            ))}
          </ul>
        ) : (
          <p className="text-muted">{t('extensions.noNetwork')}</p>
        )}
      </div>
      <RunsOn sites={entry.sites ?? []} />
      <CheckForUpdate entry={entry} />
    </div>
  );
};

/**
 * Reads the extension's entry again (its repository's index fetched anew) and updates it at once
 * when a newer version is listed; else says it's up to date, or what went wrong.
 */
const CheckForUpdate: FC<{ entry: CatalogEntry }> = ({ entry }) => {
  const { t } = useTranslation();
  const refresh = useRefreshRepo();
  const { install } = useInstallQueue();
  const [fresh, setFresh] = useState<CatalogEntry | null | undefined>(undefined);
  const check = () =>
    refresh.mutate(entry.repoId, {
      onSuccess: (catalog) => {
        const listing = catalog.find((item) => item.repo.id === entry.repoId);
        const found = listing?.entries.find((item) => item.id === entry.id) ?? null;
        setFresh(found);
        if (found?.state === 'updateAvailable' && !found.unsupported) install([found]);
      },
    });
  const listingError = refresh.data?.find((item) => item.repo.id === entry.repoId)?.error;
  return (
    <div className="flex flex-col gap-2">
      <button
        type="button"
        disabled={refresh.isPending}
        onClick={check}
        className="bg-surface flex items-center gap-2 self-start rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60"
      >
        <RefreshCw aria-hidden size={14} className={refresh.isPending ? 'animate-spin' : ''} />
        {t(refresh.isPending ? 'extensions.checkingUpdate' : 'extensions.checkUpdate')}
      </button>
      {refresh.isError && (
        <SourceIssue compact title={t('extensions.checkFailed')}>
          {refresh.error.message}
        </SourceIssue>
      )}
      {refresh.isSuccess && listingError && (
        <SourceIssue compact title={t('extensions.checkFailed')}>
          {listingError}
        </SourceIssue>
      )}
      {refresh.isSuccess && !listingError && fresh !== undefined && (
        <p role="status" className="text-muted">
          {fresh === null
            ? t('extensions.entryGone')
            : fresh.state === 'updateAvailable' && !fresh.unsupported
              ? t('extensions.updatingTo', { version: fresh.version })
              : fresh.state === 'available'
                ? t('extensions.listedVersion', { version: fresh.version })
                : t('extensions.stillUpToDate')}
        </p>
      )}
    </div>
  );
};
