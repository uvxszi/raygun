import { useSearch } from '@tanstack/react-router';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ExportsList } from '../export/ExportsList';
import { ChapterDownloads } from '../manga/ChapterDownloads';
import { SongDownloadsList } from '../music/SongDownloadsList';
import { useMangaLiveEvents } from '../manga/useManga';
import { Tabs } from '../shell/Tabs';
import { VideoDownloadsList } from '../watch/VideoDownloadsList';
import { PageHeader } from './PageHeader';

export type DownloadsTab = 'music' | 'chapters' | 'videos' | 'exports';

/** Downloads (More → Downloads): songs, the chapter queue, saved videos and exports, in tabs. */
export const DownloadsPage: FC = () => {
  const { t } = useTranslation();
  // Songs first (M46.10): the tab most people download into.
  const { tab = 'music' } = useSearch({ from: '/downloads' });
  useMangaLiveEvents();

  return (
    <section data-testid="downloads-page" className="flex flex-col gap-4">
      <PageHeader back fallback="/more" title={t('manga.downloads')} />
      <Tabs
        label={t('manga.downloads')}
        active={tab}
        items={[
          {
            id: 'music',
            label: t('songs.tab'),
            to: '/downloads',
            search: { tab: 'music' },
          },
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
          {
            id: 'exports',
            label: t('export.tab'),
            to: '/downloads',
            search: { tab: 'exports' },
          },
        ]}
      />
      {tab === 'music' && <SongDownloadsList />}
      {tab === 'chapters' && <ChapterDownloads />}
      {tab === 'videos' && <VideoDownloadsList />}
      {tab === 'exports' && <ExportsList />}
    </section>
  );
};
