import type { MangaDownloadItem } from '@raygun/bindings';
import { CircleAlert, Download, Pause, Play, RotateCw, X } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useDownloadChapters,
  useMangaDownloads,
  useMangaLiveEvents,
  useRemoveDownloads,
  useSetDownloadsPaused,
} from '../manga/useManga';
import { EmptyState } from '../shell/EmptyState';
import { PageHeader } from './PageHeader';

const DownloadRow: FC<{ item: MangaDownloadItem }> = ({ item }) => {
  const { t } = useTranslation();
  const retry = useDownloadChapters();
  const remove = useRemoveDownloads();
  const { state, pagesDone, pagesTotal, error } = item.download;
  const label = `${item.title} · ${item.chapter}`;
  return (
    <li data-testid="download-row" className="bg-surface flex flex-col gap-2 rounded-xl px-4 py-3">
      <div className="flex items-center gap-2">
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{item.title}</span>
          <span className="text-muted truncate text-xs">
            {item.chapter} ·{' '}
            {state === 'downloading' && pagesTotal
              ? t('manga.pagesProgress', { done: pagesDone, total: pagesTotal })
              : t(`manga.downloadState.${state}`)}
          </span>
        </span>
        {state === 'failed' && (
          <button
            type="button"
            aria-label={t('manga.retryDownload', { name: label })}
            disabled={retry.isPending}
            onClick={() => retry.mutate([item.chapterId])}
            className="text-primary grid size-9 place-items-center rounded-full"
          >
            <RotateCw aria-hidden size={18} />
          </button>
        )}
        <button
          type="button"
          aria-label={t('manga.cancelDownload', { name: label })}
          disabled={remove.isPending}
          onClick={() => remove.mutate([item.chapterId])}
          className="text-muted grid size-9 place-items-center rounded-full"
        >
          <X aria-hidden size={18} />
        </button>
      </div>
      {state === 'downloading' && (
        <div
          role="progressbar"
          aria-label={label}
          aria-valuemin={0}
          aria-valuemax={pagesTotal ?? 0}
          aria-valuenow={pagesDone}
          className="bg-background h-1 overflow-hidden rounded-full"
        >
          <div
            className="bg-primary h-full transition-[width]"
            style={{ width: pagesTotal ? `${(pagesDone / pagesTotal) * 100}%` : '0%' }}
          />
        </div>
      )}
      {state === 'failed' && error && (
        <p className="text-primary flex items-start gap-1.5 text-xs wrap-anywhere">
          <CircleAlert aria-hidden size={14} className="mt-px shrink-0" />
          {error}
        </p>
      )}
    </li>
  );
};

/** The download queue: chapters waiting, downloading or failed. */
export const MangaDownloadsPage: FC = () => {
  const { t } = useTranslation();
  useMangaLiveEvents();
  const downloads = useMangaDownloads();
  const setPaused = useSetDownloadsPaused();
  const queue = downloads.data;

  return (
    <section data-testid="manga-downloads-page" className="flex flex-col gap-4">
      <PageHeader back fallback="/more" title={t('manga.downloads')} />
      {downloads.error && (
        <p role="alert" className="text-primary text-sm wrap-anywhere">
          {downloads.error.message}
        </p>
      )}
      {queue && (
        <>
          <div className="flex flex-wrap items-center gap-3">
            {queue.items.length > 0 && (
              <button
                type="button"
                disabled={setPaused.isPending}
                onClick={() => setPaused.mutate(!queue.paused)}
                className="bg-surface flex items-center gap-2 rounded-full px-4 py-2 text-sm"
              >
                {queue.paused ? (
                  <Play aria-hidden size={16} className="text-primary" />
                ) : (
                  <Pause aria-hidden size={16} className="text-primary" />
                )}
                {t(queue.paused ? 'manga.resumeDownloads' : 'manga.pauseDownloads')}
              </button>
            )}
            {(queue.downloaded > 0 || queue.items.length > 0) && (
              <p className="text-muted text-xs">
                {t('manga.downloadedChapters', { count: queue.downloaded })}
              </p>
            )}
          </div>
          {queue.items.length === 0 ? (
            <EmptyState
              icon={Download}
              title={t('manga.queueEmpty')}
              body={t('manga.queueEmptyHint')}
            />
          ) : (
            <ul className="flex flex-col gap-2">
              {queue.items.map((item) => (
                <DownloadRow key={item.chapterId} item={item} />
              ))}
            </ul>
          )}
        </>
      )}
    </section>
  );
};
