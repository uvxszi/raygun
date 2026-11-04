import type { SongDownload, TrackInput } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { ArrowDown, CircleAlert, CircleArrowDown } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { issueSummary } from '../lib/issue';
import { Popover } from '../shell/Popover';
import { chooseDownloadSource } from './sourcePickerStore';
import { downloadSongs, useSongDownloadActions, useSongDownloadFor } from './useSongDownloads';

const BUTTON = 'rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60';

/** How far along a download is, 0–1; `null` when it isn't known yet. */
const share = (download: SongDownload) =>
  download.state === 'downloading' && download.totalBytes
    ? Math.min(1, (download.doneBytes ?? 0) / download.totalBytes)
    : null;

/** A ring that fills as the song downloads, around a small arrow. */
const ProgressRing: FC<{ value: number | null; size: number }> = ({ value, size }) => {
  const radius = size / 2 - 2;
  const length = 2 * Math.PI * radius;
  return (
    <span className="relative grid place-items-center" style={{ width: size, height: size }}>
      <svg aria-hidden width={size} height={size} className="absolute inset-0 -rotate-90">
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          strokeWidth={2}
          className="stroke-foreground/20"
        />
        <circle
          cx={size / 2}
          cy={size / 2}
          r={radius}
          fill="none"
          strokeWidth={2}
          strokeLinecap="round"
          strokeDasharray={
            value === null ? `${length / 4} ${length}` : `${length * value} ${length}`
          }
          className={`stroke-primary ${value === null ? 'origin-center animate-spin' : ''}`}
        />
      </svg>
      <ArrowDown aria-hidden size={size * 0.5} className="text-primary" />
    </span>
  );
};

/**
 * Spotify's download button beside + (M46.18): downloads the song into the app. Downloaded, it
 * fills in; pressing it then asks in a small popover beside it before the download is removed.
 * A download in progress can be cancelled the same way; a failed one can be tried again.
 */
export const DownloadSongButton: FC<{
  track: TrackInput;
  compact?: boolean;
  className?: string;
}> = ({ track, compact = false, className = '' }) => {
  const { t } = useTranslation();
  const client = useQueryClient();
  const download = useSongDownloadFor(track);
  const { remove, retry, record } = useSongDownloadActions();
  const start = useMutation({
    mutationFn: () => downloadSongs([track]),
    onSettled: () => void client.invalidateQueries({ queryKey: ['songs'] }),
  });
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  // A file from the device's library is already here.
  if (track.source?.kind === 'localFile') return null;

  const size = compact ? 18 : 22;
  const found = download.data ?? null;
  const state = found?.state ?? null;
  const busy = state === 'queued' || state === 'downloading' || state === 'recording';
  const label =
    state === 'done'
      ? t('songs.button.downloaded')
      : busy
        ? t('songs.button.downloading')
        : state === 'failed' || state === 'needsRecording'
          ? t('songs.button.failed')
          : t('songs.button.download');
  const close = () => setAnchor(null);
  const press = (button: HTMLElement) => {
    if (anchor) {
      close();
      return;
    }
    if (!found) {
      start.mutate();
      return;
    }
    setAnchor(button);
  };
  const removeIt = () => {
    if (found) remove.mutate(found.id, { onSuccess: close });
  };

  return (
    <>
      <button
        type="button"
        data-testid="download-song"
        aria-label={label}
        data-hint={label}
        aria-pressed={state === 'done'}
        aria-haspopup={found ? 'dialog' : undefined}
        disabled={download.isPending || start.isPending}
        onClick={(event) => press(event.currentTarget)}
        className={`grid shrink-0 place-items-center rounded-full ${compact ? 'size-9' : 'size-11'} ${found ? 'text-primary' : 'text-muted hover:text-foreground'} disabled:opacity-60 ${className}`}
      >
        {state === 'done' ? (
          <span
            className="bg-primary text-background grid place-items-center rounded-full"
            style={{ width: size, height: size }}
          >
            <ArrowDown aria-hidden size={size * 0.65} strokeWidth={3} />
          </span>
        ) : busy && found ? (
          <ProgressRing value={share(found)} size={size} />
        ) : state === 'failed' || state === 'needsRecording' ? (
          <CircleAlert aria-hidden size={size} />
        ) : (
          <CircleArrowDown aria-hidden size={size} />
        )}
      </button>
      {anchor && found && (
        <Popover anchor={anchor} label={label} onClose={close} width={280}>
          <div data-testid="download-confirm" className="flex flex-col gap-3 p-4">
            <p className="font-semibold">
              {state === 'done'
                ? t('songs.button.removeTitle')
                : busy
                  ? t('songs.button.cancelTitle')
                  : t('songs.button.failedTitle')}
            </p>
            <p className="text-muted text-sm wrap-anywhere">
              {state === 'done'
                ? t('songs.button.removeHint', { title: found.title })
                : busy
                  ? t('songs.button.cancelHint')
                  : found.error
                    ? issueSummary(found.error, t)
                    : t('songs.state.failed')}
            </p>
            {remove.isError && (
              <p role="alert" className="text-primary text-sm wrap-anywhere">
                {remove.error.message}
              </p>
            )}
            <div className="flex flex-wrap justify-end gap-2">
              <button type="button" onClick={close} className={`${BUTTON} bg-background`}>
                {t(state === 'done' || busy ? 'songs.button.keep' : 'collections.cancel')}
              </button>
              {state === 'needsRecording' && found.canRecord && (
                <button
                  type="button"
                  disabled={record.isPending}
                  onClick={() => record.mutate(found.id, { onSuccess: close })}
                  className={`${BUTTON} bg-background`}
                >
                  {t('songs.record')}
                </button>
              )}
              {(state === 'failed' || state === 'needsRecording') && (
                <button
                  type="button"
                  onClick={() => {
                    close();
                    chooseDownloadSource(found);
                  }}
                  className={`${BUTTON} bg-background`}
                >
                  {t('songs.button.chooseSource')}
                </button>
              )}
              {(state === 'failed' || state === 'needsRecording') && (
                <button
                  type="button"
                  disabled={retry.isPending}
                  onClick={() => retry.mutate(found.id, { onSuccess: close })}
                  className={`${BUTTON} bg-background`}
                >
                  {t('songs.button.retry')}
                </button>
              )}
              <button
                type="button"
                disabled={remove.isPending}
                onClick={removeIt}
                className={`${BUTTON} bg-primary text-background`}
              >
                {state === 'done'
                  ? t('songs.button.remove')
                  : busy
                    ? t('songs.button.cancel')
                    : t('songs.button.remove')}
              </button>
            </div>
          </div>
        </Popover>
      )}
    </>
  );
};
