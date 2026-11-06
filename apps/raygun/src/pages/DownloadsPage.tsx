import { commands } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { Link, useSearch } from '@tanstack/react-router';
import { ChevronRight, Download, HardDrive } from 'lucide-react';
import type { FC, ReactNode } from 'react';
import { useTranslation } from 'react-i18next';

import { ExportsList } from '../export/ExportsList';
import { useExportJobs } from '../export/useExports';
import { unwrapResult } from '../lib/commandResult';
import { formatBytes } from '../lib/formatBytes';
import { ChapterDownloads } from '../manga/ChapterDownloads';
import { DownloadedTitles } from '../downloads/DownloadedTitles';
import { useDownloadedTitles } from '../downloads/useDownloadedTitles';
import { SongDownloadsList } from '../music/SongDownloadsList';
import { useSongDownloads } from '../music/useSongDownloads';
import { useMangaDownloads, useMangaLiveEvents } from '../manga/useManga';
import { EmptyState, Loading } from '../shell/EmptyState';
import { Tabs } from '../shell/Tabs';
import { VideoDownloadsList } from '../watch/VideoDownloadsList';
import { useVideoDownloads } from '../watch/useVideoLibrary';
import { PageHeader } from './PageHeader';
import { FileDownloadsList } from '../websites/FileDownloads';
import { useFileDownloads } from '../websites/useFileDownloads';
import { YtDlpJobs } from '../youtube/YtDlpJobs';
import { useYtDlpJobs } from '../youtube/useYtDlp';

export type DownloadsTab = 'all' | 'music' | 'comics' | 'books' | 'videos' | 'files' | 'exports';

/** Titles each kind shows on the All tab; its own tab has the rest. */
const ALL_TAB_TITLES = 5;

/** How much room downloads take on this device, with the way to Storage. */
const StorageLine: FC = () => {
  const { t } = useTranslation();
  // Shares Settings → Data → Storage's answer.
  const usage = useQuery({
    queryKey: ['storage'],
    queryFn: () => unwrapResult(commands.storageUsage()),
    staleTime: 60_000,
  });
  const bytes = usage.data?.find((area) => area.area === 'downloads')?.bytes ?? 0;
  if (bytes <= 0) return null;
  return (
    <Link
      to="/settings/data"
      data-testid="downloads-storage"
      className="bg-surface hover:bg-surface/70 flex items-center gap-3 rounded-xl px-4 py-3 text-sm"
    >
      <HardDrive aria-hidden size={18} className="text-primary shrink-0" />
      <span className="flex-1">{t('downloads.storage', { size: formatBytes(bytes) })}</span>
      <ChevronRight aria-hidden size={16} className="text-muted" />
    </Link>
  );
};

/** One kind on the All tab: its name leads to its own tab. */
const Section: FC<{ tab: DownloadsTab; title: string; children: ReactNode }> = ({
  tab,
  title,
  children,
}) => {
  const { t } = useTranslation();
  return (
    <section aria-label={title} className="flex flex-col gap-2">
      <Link
        to="/downloads"
        search={{ tab } as never}
        replace
        aria-label={t('downloads.seeAll', { name: title })}
        className="flex items-center justify-between gap-2"
      >
        <h2 className="text-lg font-semibold">{title}</h2>
        <ChevronRight aria-hidden size={20} className="text-muted" />
      </Link>
      {children}
    </section>
  );
};

/**
 * Downloads → Comics or Books: that medium's chapters in the queue, then every title with
 * chapters downloaded; one empty state when there are neither.
 */
const TitleDownloads: FC<{ medium: 'manga' | 'novel'; limit?: number }> = ({ medium, limit }) => {
  const { t } = useTranslation();
  const queue = useMangaDownloads();
  const titles = useDownloadedTitles(medium);
  const queued = (queue.data?.items ?? []).some((item) => item.medium === medium);
  const empty = queue.isSuccess && titles.isSuccess && !queued && titles.data.length === 0;
  return (
    <div className="flex flex-col gap-4">
      <ChapterDownloads medium={medium} />
      <DownloadedTitles medium={medium} limit={limit} />
      {empty && <EmptyState icon={Download} title={t('downloads.noneTitle')} />}
    </div>
  );
};

/** Downloads → Videos: the episodes saved or saving, after the titles they belong to. */
const VideoDownloads: FC<{ limit?: number }> = ({ limit }) => (
  <div className="flex flex-col gap-4">
    <DownloadedTitles medium="video" limit={limit} />
    <VideoDownloadsList />
  </div>
);

/**
 * Downloads → All (M46.37): every kind that has something, each with its own list; one empty
 * state when nothing is downloaded at all.
 */
const AllDownloads: FC = () => {
  const { t } = useTranslation();
  const songs = useSongDownloads();
  const chapters = useMangaDownloads();
  const comics = useDownloadedTitles('manga');
  const books = useDownloadedTitles('novel');
  const videos = useVideoDownloads();
  const exports = useExportJobs();
  const youtube = useYtDlpJobs();
  const files = useFileDownloads();
  const answered = [songs, chapters, comics, books, videos, exports, youtube, files].every(
    (query) => query.data !== undefined || query.isError,
  );
  const queued = (medium: string) =>
    (chapters.data?.items ?? []).some((item) => item.medium === medium);
  const shown = {
    music: (songs.data?.length ?? 0) > 0,
    comics: queued('manga') || (comics.data?.length ?? 0) > 0,
    books: queued('novel') || (books.data?.length ?? 0) > 0,
    videos: (videos.data?.items.length ?? 0) > 0,
    files: (files.data ?? []).some((file) => file.state !== 'cancelled'),
    exports: (exports.data?.length ?? 0) + (youtube.data?.length ?? 0) > 0,
  };
  const nothing = Object.values(shown).every((has) => !has);
  if (!answered) return <Loading label={t('downloads.loading')} />;
  return (
    <div data-testid="downloads-all" className="flex flex-col gap-6">
      <StorageLine />
      {nothing && <EmptyState icon={Download} title={t('downloads.noneTitle')} />}
      {shown.music && (
        <Section tab="music" title={t('songs.tab')}>
          {/* Finished songs are a line here; the Music tab lists them. */}
          <SongDownloadsList compact />
        </Section>
      )}
      {shown.comics && (
        <Section tab="comics" title={t('library.tabs.manga')}>
          <TitleDownloads medium="manga" limit={ALL_TAB_TITLES} />
        </Section>
      )}
      {shown.books && (
        <Section tab="books" title={t('library.tabs.novel')}>
          <TitleDownloads medium="novel" limit={ALL_TAB_TITLES} />
        </Section>
      )}
      {shown.videos && (
        <Section tab="videos" title={t('downloads.videos')}>
          <VideoDownloads limit={ALL_TAB_TITLES} />
        </Section>
      )}
      {shown.files && (
        <Section tab="files" title={t('downloads.files.tab')}>
          <FileDownloadsList compact />
        </Section>
      )}
      {shown.exports && (
        <Section tab="exports" title={t('export.tab')}>
          <YtDlpJobs />
          {(exports.data?.length ?? 0) > 0 && <ExportsList />}
        </Section>
      )}
    </div>
  );
};

/**
 * Downloads (More → Downloads): songs, comics, books, videos, files from websites and exports,
 * in tabs. Each downloaded title or song opens and has its menu as in the Library.
 */
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
            id: 'comics',
            label: t('library.tabs.manga'),
            to: '/downloads',
            search: { tab: 'comics' },
          },
          {
            id: 'books',
            label: t('library.tabs.novel'),
            to: '/downloads',
            search: { tab: 'books' },
          },
          {
            id: 'videos',
            label: t('downloads.videos'),
            to: '/downloads',
            search: { tab: 'videos' },
          },
          {
            id: 'files',
            label: t('downloads.files.tab'),
            to: '/downloads',
            search: { tab: 'files' },
          },
          {
            id: 'exports',
            label: t('export.tab'),
            to: '/downloads',
            search: { tab: 'exports' },
          },
        ]}
      />
      {tab === 'all' && <AllDownloads />}
      {tab === 'music' && <SongDownloadsList />}
      {tab === 'comics' && <TitleDownloads medium="manga" />}
      {tab === 'books' && <TitleDownloads medium="novel" />}
      {tab === 'videos' && <VideoDownloads />}
      {tab === 'files' && <FileDownloadsList />}
      {tab === 'exports' && (
        <>
          <YtDlpJobs />
          <ExportsList />
        </>
      )}
    </section>
  );
};
