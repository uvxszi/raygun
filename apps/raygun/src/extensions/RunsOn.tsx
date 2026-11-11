import { AppWindow } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { siteLabels } from './siteLabels';

/**
 * Where a site-scripts extension's scripts run in the websites browser, shown before it's
 * installed and on its page: its repository's patterns are checked against its package at
 * install, so this is what runs.
 */
export const RunsOn: FC<{ sites: readonly string[]; large?: boolean }> = ({ sites, large }) => {
  const { t } = useTranslation();
  if (sites.length === 0) return null;
  return (
    <div data-testid="runs-on" className="flex flex-col gap-1.5">
      <h3 className="font-medium">{t('extensions.runsOnTitle')}</h3>
      <p className="text-muted text-xs">{t('extensions.runsOnHint')}</p>
      <ul className="flex flex-col gap-1">
        {siteLabels(sites, t).map((site) => (
          <li
            key={site}
            className={`flex items-center gap-2 wrap-anywhere ${large ? 'px-1 py-2' : 'text-muted'}`}
          >
            <AppWindow aria-hidden size={large ? 18 : 14} className="text-muted shrink-0" />
            {site}
          </li>
        ))}
      </ul>
    </div>
  );
};
