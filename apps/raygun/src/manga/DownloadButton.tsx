import type { MangaChapterInfo } from '@raygun/bindings';
import { CircleAlert, CircleCheck, Clock, Download } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { useDownloadChapters, useRemoveDownloads } from './useManga';

/** Progress as a ring around the button's middle. */
const ProgressRing: FC<{ fraction: number }> = ({ fraction }) => {
  const radius = 9;
  const circumference = 2 * Math.PI * radius;
  return (
    <svg aria-hidden viewBox="0 0 24 24" className="size-5 -rotate-90">
      <circle
        cx="12"
        cy="12"
        r={radius}
        fill="none"
        strokeWidth="2.5"
        className="stroke-muted/30"
      />
      <circle
        cx="12"
        cy="12"
        r={radius}
        fill="none"
        strokeWidth="2.5"
        strokeLinecap="round"
        strokeDasharray={circumference}
        strokeDashoffset={circumference * (1 - fraction)}
        className="stroke-primary transition-[stroke-dashoffset]"
      />
    </svg>
  );
};

/**
 * A chapter's download control: download, then queued or a progress ring (tap to cancel), then a
 * check (tap to delete the files); a failed download retries on tap.
 */
export const DownloadButton: FC<{ chapter: MangaChapterInfo }> = ({ chapter }) => {
  const { t } = useTranslation();
  const download = useDownloadChapters();
  const remove = useRemoveDownloads();
  const state = chapter.download?.state;
  const busy = download.isPending || remove.isPending;
  const base = 'grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60';

  if (state === 'done') {
    return (
      <button
        type="button"
        aria-label={t('manga.deleteDownload', { name: chapter.name })}
        disabled={busy}
        onClick={() => remove.mutate([chapter.id])}
        className={`${base} text-primary`}
      >
        <CircleCheck aria-hidden size={18} />
      </button>
    );
  }
  if (state === 'queued' || state === 'downloading') {
    const { pagesDone, pagesTotal } = chapter.download ?? { pagesDone: 0, pagesTotal: null };
    const percent = pagesTotal ? Math.round((pagesDone / pagesTotal) * 100) : 0;
    return (
      <button
        type="button"
        aria-label={
          state === 'queued'
            ? t('manga.cancelQueued', { name: chapter.name })
            : t('manga.cancelDownloading', { name: chapter.name, percent })
        }
        disabled={busy}
        onClick={() => remove.mutate([chapter.id])}
        className={`${base} text-muted`}
      >
        {state === 'queued' ? (
          <Clock aria-hidden size={18} />
        ) : (
          <ProgressRing fraction={pagesTotal ? pagesDone / pagesTotal : 0} />
        )}
      </button>
    );
  }
  return (
    <button
      type="button"
      aria-label={
        state === 'failed'
          ? t('manga.retryDownload', { name: chapter.name })
          : t('manga.download', { name: chapter.name })
      }
      title={chapter.download?.error ?? undefined}
      disabled={busy}
      onClick={() => download.mutate([chapter.id])}
      className={`${base} ${state === 'failed' ? 'text-primary' : 'text-muted'}`}
    >
      {state === 'failed' ? (
        <CircleAlert aria-hidden size={18} />
      ) : (
        <Download aria-hidden size={18} />
      )}
    </button>
  );
};
