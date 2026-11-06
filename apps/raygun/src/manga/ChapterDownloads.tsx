import type { MangaDownloadItem } from '@raygun/bindings';
import { Pause, Play, RotateCw, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import {
  useDownloadChapters,
  useMangaDownloads,
  useRemoveDownloads,
  useSetDownloadsPaused,
} from './useManga';
import { SourceIssue } from '../shell/SourceIssue';

const PILL_SHAPE = 'flex items-center gap-2 rounded-full px-4 py-2 text-sm disabled:opacity-60';
const PILL = `bg-surface ${PILL_SHAPE}`;

const DownloadRow: FC<{ item: MangaDownloadItem; paused: boolean }> = ({ item, paused }) => {
  const { t } = useTranslation();
  const retry = useDownloadChapters();
  const remove = useRemoveDownloads();
  const { state, pagesDone, pagesTotal, error } = item.download;
  const label = `${item.title} · ${item.chapter}`;
  const share = pagesTotal ? Math.min(1, pagesDone / pagesTotal) : 0;
  return (
    <li data-testid="download-row" className="bg-surface flex flex-col gap-2 rounded-xl px-4 py-3">
      <div className="flex items-center gap-2">
        <span className="flex min-w-0 flex-1 flex-col">
          <span className="truncate text-sm font-medium">{item.title}</span>
          <span className="text-muted truncate text-xs">
            {item.chapter} ·{' '}
            {state === 'downloading' && pagesTotal
              ? t('manga.pagesProgress', { done: pagesDone, count: pagesTotal })
              : state === 'queued' && paused
                ? t('watch.paused')
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
            className="bg-primary h-full origin-left transition-transform"
            style={{ transform: `scaleX(${share})` }}
          />
        </div>
      )}
      {state === 'failed' && error && (
        <SourceIssue compact title={t('manga.downloadFailed')}>
          {error}
        </SourceIssue>
      )}
    </li>
  );
};

/**
 * `medium`'s chapters waiting, downloading or failed (comics and books share the queue), and
 * nothing while it has none: Downloads → Comics and Books list what's downloaded below it.
 */
export const ChapterDownloads: FC<{ medium: 'manga' | 'novel' }> = ({ medium }) => {
  const { t } = useTranslation();
  const downloads = useMangaDownloads();
  const setPaused = useSetDownloadsPaused();
  const retry = useDownloadChapters();
  const remove = useRemoveDownloads();
  // Cancelling everything takes a second tap.
  const [clearing, setClearing] = useState(false);
  const all = downloads.data;
  const queue = all && { ...all, items: all.items.filter((item) => item.medium === medium) };
  const failed = queue?.items.filter((item) => item.download.state === 'failed') ?? [];

  return (
    <div data-testid="chapter-downloads" className="flex flex-col gap-4">
      {downloads.error && (
        <SourceIssue
          compact
          title={t('manga.downloadsUnreadable')}
          onRetry={() => void downloads.refetch()}
        >
          {downloads.error.message}
        </SourceIssue>
      )}
      {queue && queue.items.length > 0 && (
        <>
          <div className="flex flex-wrap items-center gap-2">
            <button
              type="button"
              disabled={setPaused.isPending}
              onClick={() => setPaused.mutate(!queue.paused)}
              className={PILL}
            >
              {queue.paused ? (
                <Play aria-hidden size={16} className="text-primary" />
              ) : (
                <Pause aria-hidden size={16} className="text-primary" />
              )}
              {t(queue.paused ? 'manga.resumeDownloads' : 'manga.pauseDownloads')}
            </button>
            {failed.length > 1 && (
              <button
                type="button"
                disabled={retry.isPending}
                onClick={() => retry.mutate(failed.map((item) => item.chapterId))}
                className={PILL}
              >
                <RotateCw aria-hidden size={16} className="text-primary" />
                {t('manga.retryFailed', { count: failed.length })}
              </button>
            )}
            {queue.items.length > 1 && (
              <button
                type="button"
                disabled={remove.isPending}
                onBlur={() => setClearing(false)}
                onClick={() => {
                  if (!clearing) {
                    setClearing(true);
                    return;
                  }
                  setClearing(false);
                  remove.mutate(queue.items.map((item) => item.chapterId));
                }}
                className={
                  clearing ? `bg-primary text-background font-semibold ${PILL_SHAPE}` : PILL
                }
              >
                <X aria-hidden size={16} className={clearing ? '' : 'text-primary'} />
                {clearing
                  ? t('manga.cancelAllConfirm', { count: queue.items.length })
                  : t('manga.cancelAll')}
              </button>
            )}
          </div>
          <ul className="flex flex-col gap-2">
            {queue.items.map((item) => (
              <DownloadRow key={item.chapterId} item={item} paused={queue.paused} />
            ))}
          </ul>
        </>
      )}
    </div>
  );
};
