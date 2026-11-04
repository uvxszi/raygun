import { useNavigate } from '@tanstack/react-router';
import { Globe, Plus } from 'lucide-react';
import { useEffect, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { LibraryTop } from '../library/LibraryTop';
import { EmptyState } from '../shell/EmptyState';
import { SourceIssue } from '../shell/SourceIssue';
import { matchesSearch } from '../shell/useListSearch';
import { useSearchMode } from '../shell/useSearchMode';
import { WebsiteList } from './WebsiteList';
import { WebsiteSheet } from './WebsiteSheets';
import { useOpenWebsite, useWebsites } from './useWebsites';

/**
 * Library → Websites: the sites added by address, to open, edit, pin to quick access or remove.
 * `site` in the address (a quick access pin) opens that site once.
 */
export const WebsitesLibrary: FC<{ site?: string }> = ({ site }) => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const websites = useWebsites();
  const open = useOpenWebsite();
  const find = useSearchMode(t('websites.find'));
  const [adding, setAdding] = useState(false);
  const shown = websites.data?.filter((entry) =>
    matchesSearch(find.query, [entry.name, entry.url]),
  );

  useEffect(() => {
    if (!site) return;
    open.mutate(site);
    void navigate({ to: '/library', search: { media: 'websites' }, replace: true });
    // Once per address: the site leaves the address right away.
    // eslint-disable-next-line react-hooks/exhaustive-deps
  }, [site]);

  return (
    <>
      <LibraryTop
        media="websites"
        search={find.search}
        actions={
          <>
            {find.action}
            <button
              type="button"
              onClick={() => setAdding(true)}
              className="bg-surface hover:bg-background flex items-center gap-1.5 rounded-full px-3 py-1.5 text-sm font-medium"
            >
              <Plus aria-hidden size={16} />
              {t('websites.add')}
            </button>
          </>
        }
      />
      <div data-testid="library-websites" className="flex flex-col">
        {websites.error && (
          <SourceIssue compact title={t('websites.loadFailed')}>
            {websites.error.message}
          </SourceIssue>
        )}
        {open.error && (
          <SourceIssue compact title={t('websites.openFailed', { name: t('websites.title') })}>
            {open.error.message}
          </SourceIssue>
        )}
        {websites.data?.length === 0 && (
          <EmptyState
            icon={Globe}
            title={t('websites.empty')}
            action={{ label: t('websites.add'), icon: Plus, onClick: () => setAdding(true) }}
          />
        )}
        {shown && <WebsiteList sites={shown} large />}
      </div>
      {adding && <WebsiteSheet onClose={() => setAdding(false)} />}
    </>
  );
};
