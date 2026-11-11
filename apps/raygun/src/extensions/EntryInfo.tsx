import type { CatalogEntry } from '@raygun/bindings';
import { Globe } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { AdultMark } from '../shell/AdultMark';
import { hostLabel } from './hostLabel';
import { RunsOn } from './RunsOn';

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
    </div>
  );
};
