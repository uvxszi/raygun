import { useSearch } from '@tanstack/react-router';
import { RefreshCw } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ChapterUpdates } from '../manga/ChapterUpdates';
import { useMangaLiveEvents, useMangaUpdateStatus, useUpdateLibrary } from '../manga/useManga';
import { Tabs } from '../shell/Tabs';
import { TopBarAction } from '../shell/TopBar';
import { EpisodeUpdates } from '../watch/EpisodeUpdates';
import { PageHeader } from './PageHeader';

export type UpdatesTab = 'chapters' | 'episodes';

/**
 * Updates (More → Updates): what library updates found, chapters and episodes in their own tabs.
 * One update checks both, so its progress and the refresh action sit above the tabs.
 */
export const UpdatesPage: FC = () => {
  const { t, i18n } = useTranslation();
  const { tab = 'chapters' } = useSearch({ from: '/updates' });
  useMangaLiveEvents();
  const status = useMangaUpdateStatus();
  const update = useUpdateLibrary();
  const running = status.data?.running ?? false;
  const lastRun = status.data?.lastRunAt;
  const finished = status.data && !status.data.running && lastRun ? status.data : null;

  return (
    <section data-testid="updates-page" className="flex flex-col gap-4">
      <PageHeader
        back
        fallback="/more"
        title={t('manga.updates')}
        actions={
          <TopBarAction
            label={t('manga.updateLibrary')}
            icon={RefreshCw}
            disabled={running || update.isPending}
            onClick={() => update.mutate()}
          />
        }
      />
      <Tabs
        label={t('manga.updates')}
        active={tab}
        items={[
          {
            id: 'chapters',
            label: t('updates.chapters'),
            to: '/updates',
            search: { tab: 'chapters' },
          },
          {
            id: 'episodes',
            label: t('updates.episodes'),
            to: '/updates',
            search: { tab: 'episodes' },
          },
        ]}
      />
      {(running || finished?.lastRunAt) && (
        <div className="flex flex-wrap items-center gap-3">
          {running && status.data && (
            <p role="status" className="text-muted flex items-center gap-2 text-sm">
              <RefreshCw aria-hidden size={14} className="text-primary animate-spin" />
              {t('manga.updating', { checked: status.data.checked, total: status.data.total })}
            </p>
          )}
          {finished?.lastRunAt && (
            <p className="text-muted text-sm italic">
              {t('manga.lastUpdate', {
                when: new Intl.DateTimeFormat(i18n.language, {
                  dateStyle: 'medium',
                  timeStyle: 'short',
                }).format(finished.lastRunAt),
              })}
              {finished.failed > 0 && ` · ${t('manga.updateFailures', { count: finished.failed })}`}
            </p>
          )}
        </div>
      )}
      {tab === 'chapters' ? <ChapterUpdates /> : <EpisodeUpdates />}
    </section>
  );
};
