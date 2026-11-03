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

export type DownloadsTab = 'all' | 'music' | 'chapters' | 'videos' | 'exports';

/** Downloads (More → Downloads): songs, the chapter queue, saved videos and exports, in tabs. */
export const DownloadsPage: FC = () => {
  const { t } = useTranslation();
  // All first (M46.37): every kind of download on one page, each with its own list.
  const { tab = 'all' } = useSearch({ from: '/downloads' });
  useMangaLiveEvents();

  return (
    <section data-testid="downloads-page" className="flex flex-col gap-4">
      <PageHeader back fallback="/more" title={t('manga.downloads')} />
      <Tabs
        label={t('manga.downloads')}
        active={tab}
        items={[
          { id: 'all', label: t('library.tabs.all'), to: '/downloads', search: {} },
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
      {tab === 'all' && (
        <div className="flex flex-col gap-6">
          <section aria-label={t('songs.tab')} className="flex flex-col gap-2">
            <h2 className="text-lg font-semibold">{t('songs.tab')}</h2>
            <SongDownloadsList />
          </section>
          <section aria-label={t('updates.chapters')} className="flex flex-col gap-2">
            <h2 className="text-lg font-semibold">{t('updates.chapters')}</h2>
            <ChapterDownloads />
          </section>
          <section aria-label={t('downloads.videos')} className="flex flex-col gap-2">
            <h2 className="text-lg font-semibold">{t('downloads.videos')}</h2>
            <VideoDownloadsList />
          </section>
          <section aria-label={t('export.tab')} className="flex flex-col gap-2">
            <h2 className="text-lg font-semibold">{t('export.tab')}</h2>
            <ExportsList />
          </section>
        </div>
      )}
      {tab === 'music' && <SongDownloadsList />}
      {tab === 'chapters' && <ChapterDownloads />}
      {tab === 'videos' && <VideoDownloadsList />}
      {tab === 'exports' && <ExportsList />}
    </section>
  );
};
