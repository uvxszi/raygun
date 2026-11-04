import { Plus } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { SourceIssue } from '../shell/SourceIssue';
import { matchesSearch } from '../shell/useListSearch';
import { WebsiteList } from './WebsiteList';
import { WebsiteSheet } from './WebsiteSheets';
import { useWebsites } from './useWebsites';

/**
 * Browse → Sources → Websites (M46.3): sites the user added, opened in Raygun's built-in
 * browser with its ad and popup blocker.
 */
export const WebsitesSection: FC<{ query?: string }> = ({ query = '' }) => {
  const { t } = useTranslation();
  const websites = useWebsites();
  const shown = websites.data?.filter((site) => matchesSearch(query, [site.name, site.url]));
  const [adding, setAdding] = useState(false);
  const title = t('websites.title');

  return (
    <section aria-label={title} className="flex flex-col">
      <div className="flex items-center justify-between gap-2 px-1 pt-2 pb-1">
        <h2 className="text-muted text-sm font-medium">{title}</h2>
        <button
          type="button"
          onClick={() => setAdding(true)}
          className="text-primary flex items-center gap-1 rounded-full px-3 py-1.5 text-sm font-medium"
        >
          <Plus aria-hidden size={16} />
          {t('websites.add')}
        </button>
      </div>
      {websites.error && (
        <SourceIssue compact title={t('websites.loadFailed')}>
          {websites.error.message}
        </SourceIssue>
      )}
      {websites.data?.length === 0 && (
        <p className="text-muted px-1 pb-2 text-sm">{t('websites.empty')}</p>
      )}
      {shown && <WebsiteList sites={shown} />}
      {adding && <WebsiteSheet onClose={() => setAdding(false)} />}
    </section>
  );
};
