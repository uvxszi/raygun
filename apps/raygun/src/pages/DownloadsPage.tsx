import { useSearch } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ChapterDownloads } from '../manga/ChapterDownloads';
import { useMangaLiveEvents } from '../manga/useManga';
import { Tabs } from '../shell/Tabs';
import { VideoDownloadsList } from '../watch/VideoDownloadsList';
import { PageHeader } from './PageHeader';

export type DownloadsTab = 'chapters' | 'videos';

/** Downloads (More → Downloads): the chapter queue and saved videos, in their own tabs. */
export const DownloadsPage: FC = () => {
  const { t } = useTranslation();
  const { tab = 'chapters' } = useSearch({ from: '/downloads' });
  useMangaLiveEvents();

  return (
    <section data-testid="downloads-page" className="flex flex-col gap-4">
      <PageHeader back fallback="/more" title={t('manga.downloads')} />
      <Tabs
        label={t('manga.downloads')}
        active={tab}
        items={[
          {
            id: 'chapters',
            label: t('updates.chapters'),
            to: '/downloads',
            search: { tab: 'chapters' },
          },
          {
            id: 'videos',
            label: t('downloads.videos'),
            to: '/downloads',
            search: { tab: 'videos' },
          },
        ]}
      />
      {tab === 'chapters' ? <ChapterDownloads /> : <VideoDownloadsList />}
    </section>
  );
};
