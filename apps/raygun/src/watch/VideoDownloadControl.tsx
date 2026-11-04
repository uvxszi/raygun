import type { VideoDownloadItem } from '@raygun/bindings';
import { CircleAlert, CircleCheck, Clock, Download } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { ProgressRing } from '../manga/DownloadButton';
import { ConfirmButton } from '../shell/ConfirmButton';
import { downloadShare, useRemoveVideoDownload, useRetryVideoDownload } from './useVideoLibrary';

const BASE = 'grid size-9 shrink-0 place-items-center rounded-full disabled:opacity-60';

/**
 * An episode's or film's download control (Aniyomi's): download (which first asks for a link),
 * then queued or a progress ring (tap to cancel), then a check (tap twice to delete); a failed one
 * retries on tap.
 */
export const VideoDownloadControl: FC<{
  item: VideoDownloadItem | undefined;
  /** What the title page knows, before the download list has loaded. */
  downloaded: boolean;
  name: string;
  onDownload: () => void;
}> = ({ item, downloaded, name, onDownload }) => {
  const { t } = useTranslation();
  const remove = useRemoveVideoDownload();
  const retry = useRetryVideoDownload();
  const busy = remove.isPending || retry.isPending;
  const state = item?.state ?? (downloaded ? 'done' : undefined);

  if (state === 'done') {
    return (
      <ConfirmButton
        label={t('watch.deleteDownload', { name })}
        confirm={t('watch.deleteConfirm')}
        icon={CircleCheck}
        disabled={busy || !item}
        onConfirm={() => item && remove.mutate(item.id)}
        idleClassName="text-primary size-9"
      />
    );
  }
  if (item && (state === 'queued' || state === 'downloading')) {
    const share = downloadShare(item) ?? 0;
    return (
      <button
        type="button"
        aria-label={
          state === 'queued'
            ? t('watch.cancelQueued', { name })
            : t('watch.cancelDownloading', { name, percent: Math.round(share * 100) })
        }
        disabled={busy}
        onClick={() => remove.mutate(item.id)}
        className={`${BASE} text-muted`}
      >
        {state === 'queued' ? <Clock aria-hidden size={18} /> : <ProgressRing fraction={share} />}
      </button>
    );
  }
  if (item && state === 'failed') {
    return (
      <button
        type="button"
        aria-label={t('watch.retryDownload', { name })}
        title={item.error ?? undefined}
        disabled={busy}
        onClick={() => retry.mutate(item.id)}
        className={`${BASE} text-primary`}
      >
        <CircleAlert aria-hidden size={18} />
      </button>
    );
  }
  return (
    <button
      type="button"
      aria-label={t('watch.downloadEpisode', { name })}
      onClick={onDownload}
      className={`${BASE} text-muted`}
    >
      <Download aria-hidden size={18} />
    </button>
  );
};
