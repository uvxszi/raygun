import type { TrackInput } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { ArrowDown, CircleAlert, CircleArrowDown } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Popover } from '../shell/Popover';
import { isBusy, isFailed, shareOf, SPIN, useStateLine } from './songDownloadState';
import { chooseDownloadSource } from './sourcePickerStore';
import {
  downloadSongs,
  useSongDownloadActions,
  useSongDownloadFor,
  useSongDownloadsPaused,
} from './useSongDownloads';

const BUTTON = 'rounded-full px-4 py-1.5 text-sm font-semibold disabled:opacity-60';

/**
 * A ring that fills as the song downloads, around a small arrow; it turns while how far isn't
 * known yet (and stands still while downloads are paused). It moves only when the download
 * reports (a few times a second at most).
 */
const ProgressRing: FC<{ value: number | null; size: number; still: boolean }> = ({
  value,
  size,
  still,
}) => {
  const radius = size / 2 - 2;
  const length = 2 * Math.PI * radius;
  return (
    <span className="relative grid place-items-center" style={{ width: size, height: size }}>
      <svg
        aria-hidden
        width={size}
        height={size}
        className={`absolute inset-0 ${value === null && !still ? SPIN : '-rotate-90'}`}
      >
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
          className="stroke-primary"
        />
      </svg>
      <ArrowDown aria-hidden size={size * 0.5} className="text-primary" />
    </span>
  );
};

/** What went wrong, then the sources' own errors under More info. */
const Why: FC<{ summary: string; detail: string | null }> = ({ summary, detail }) => {
  const { t } = useTranslation();
  const [open, setOpen] = useState(false);
  return (
    <div className="flex flex-col items-start gap-1.5">
      <p className="text-muted text-sm wrap-anywhere">{summary}</p>
      {detail && (
        <button
          type="button"
          aria-expanded={open}
          onClick={() => setOpen(!open)}
          className="text-muted hover:text-foreground text-xs font-semibold"
        >
          {open ? t('issue.less') : t('issue.more')}
        </button>
      )}
      {detail && open && (
        <p className="selectable bg-background w-full rounded-lg px-2 py-1.5 font-mono text-xs whitespace-pre-wrap wrap-anywhere">
          {detail}
        </p>
      )}
    </div>
  );
};

/**
 * Spotify's download button beside + (M46.18): downloads the song into the app. Downloaded, it
 * fills in; pressing it then asks in a small popover beside it before the download is removed.
 * A download in progress can be cancelled the same way; a failed one says why, and can be tried
 * again or given another source, which shows at once.
 */
export const DownloadSongButton: FC<{
  track: TrackInput;
  compact?: boolean;
  className?: string;
}> = ({ track, compact = false, className = '' }) => {
  const { t } = useTranslation();
  const line = useStateLine();
  const download = useSongDownloadFor(track);
  const paused = useSongDownloadsPaused().data ?? false;
  const { remove, retry, record } = useSongDownloadActions();
  const start = useMutation({ mutationFn: () => downloadSongs([track]) });
  const [anchor, setAnchor] = useState<HTMLElement | null>(null);
  // A file from the device's library is already here.
  if (track.source?.kind === 'localFile') return null;

  const size = compact ? 18 : 22;
  const found = download.data ?? null;
  const state = found?.state ?? null;
  // Asked for, and Rust hasn't said it's queued yet.
  const starting = !found && start.isPending;
  const busy = starting || (found !== null && isBusy(found));
  const failed = found !== null && isFailed(found);
  const label =
    state === 'done'
      ? t('songs.button.downloaded')
      : busy
        ? t('songs.button.downloading')
        : failed
          ? t('songs.button.failed')
          : t('songs.button.download');
  const close = () => setAnchor(null);
  const press = (button: HTMLElement) => {
    if (anchor) {
      close();
      return;
    }
    if (!found) {
      if (!start.isPending) start.mutate();
      return;
    }
    setAnchor(button);
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
        disabled={download.isPending}
        onClick={(event) => press(event.currentTarget)}
        className={`grid shrink-0 place-items-center rounded-full ${compact ? 'size-9' : 'size-11'} ${found || starting ? 'text-primary' : 'text-muted hover:text-foreground'} disabled:opacity-60 ${className}`}
      >
        {state === 'done' ? (
          <span
            className="bg-primary text-background grid place-items-center rounded-full"
            style={{ width: size, height: size }}
          >
            <ArrowDown aria-hidden size={size * 0.65} strokeWidth={3} />
          </span>
        ) : busy ? (
          <ProgressRing
            value={found ? shareOf(found) : null}
            size={size}
            still={paused && state === 'queued'}
          />
        ) : failed ? (
          <CircleAlert aria-hidden size={size} />
        ) : (
          <CircleArrowDown aria-hidden size={size} />
        )}
      </button>
      {anchor && found && (
        <Popover anchor={anchor} label={label} onClose={close} width={300}>
          <div data-testid="download-confirm" className="flex flex-col gap-3 p-4">
            <p className="font-semibold">
              {state === 'done'
                ? t('songs.button.removeTitle')
                : busy
                  ? t('songs.button.cancelTitle')
                  : t('songs.button.failedTitle')}
            </p>
            {state === 'done' && (
              <p className="text-muted text-sm wrap-anywhere">
                {t('songs.button.removeHint', { title: found.title })}
              </p>
            )}
            {busy && (
              <div className="flex flex-col gap-1">
                <p className="text-muted text-sm wrap-anywhere">{line(found, paused)}</p>
                <p className="text-muted text-sm">{t('songs.button.cancelHint')}</p>
              </div>
            )}
            {failed && (
              <Why summary={found.error ?? t('songs.state.failed')} detail={found.errorDetail} />
            )}
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
                  onClick={() => {
                    close();
                    record.mutate(found.id);
                  }}
                  className={`${BUTTON} bg-background`}
                >
                  {t('songs.record')}
                </button>
              )}
              {failed && (
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
              {failed && (
                <button
                  type="button"
                  onClick={() => {
                    // It shows as queued at once; the popover's question is answered.
                    close();
                    retry.mutate(found.id);
                  }}
                  className={`${BUTTON} bg-background`}
                >
                  {t('songs.button.retry')}
                </button>
              )}
              {state !== 'recording' && (
                <button
                  type="button"
                  disabled={remove.isPending}
                  onClick={() => {
                    close();
                    remove.mutate(found.id);
                  }}
                  className={`${BUTTON} bg-primary text-background`}
                >
                  {busy ? t('songs.button.cancel') : t('songs.button.remove')}
                </button>
              )}
            </div>
          </div>
        </Popover>
      )}
    </>
  );
};
