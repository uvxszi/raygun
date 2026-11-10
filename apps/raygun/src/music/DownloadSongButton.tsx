import type { TrackInput } from '@raygun/bindings';
import { useMutation } from '@tanstack/react-query';
import { ArrowDown, CircleAlert, CircleArrowDown } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Popover } from '../shell/Popover';
import { isBusy, isFailed, shareOf, SPIN, useProblemLine, useStateLine } from './songDownloadState';
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
 * fills in; pressing it then opens a small popover beside it, where the download can be checked
 * and downloaded again from the song's source now (M46.49), or removed. A download in progress
 * can be cancelled the same way; a failed one says why, and can be tried again or given another
 * source, which shows at once.
 */
export const DownloadSongButton: FC<{
  track: TrackInput;
  compact?: boolean;
  className?: string;
}> = ({ track, compact = false, className = '' }) => {
  const { t } = useTranslation();
  const line = useStateLine();
  const problemLine = useProblemLine();
  const download = useSongDownloadFor(track);
  const paused = useSongDownloadsPaused().data ?? false;
  const { remove, retry, check, redownload } = useSongDownloadActions();
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
    check.reset();
    setAnchor(button);
  };
  // This song's check said it's fine.
  const fine = check.isSuccess && check.data.problems === 0 && !found?.problem;

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
                ? t('songs.button.downloaded')
                : found.replacing
                  ? t('songs.button.againTitle')
                  : busy
                    ? t('downloads.stop.title')
                    : t('songs.button.failedTitle')}
            </p>
            {state === 'done' && (
              <div className="flex flex-col gap-1">
                <p className="text-muted text-sm wrap-anywhere">{line(found)}</p>
                {found.problem && (
                  <Why summary={problemLine(found.problem)} detail={found.problem.detail} />
                )}
                {found.error && (
                  <Why
                    summary={t('songs.againFailed', { why: found.error })}
                    detail={found.errorDetail}
                  />
                )}
                {fine && (
                  <p role="status" className="text-sm">
                    {t('songs.button.fine')}
                  </p>
                )}
                <p className="text-muted text-sm wrap-anywhere">
                  {t('songs.button.removeHint', { title: found.title })}
                </p>
              </div>
            )}
            {busy && (
              <div className="flex flex-col gap-1">
                <p className="text-muted text-sm wrap-anywhere">{line(found, paused)}</p>
                <p className="text-muted text-sm">
                  {found.replacing ? t('songs.button.againHint') : t('songs.button.cancelHint')}
                </p>
              </div>
            )}
            {failed && (
              <Why summary={found.error ?? t('songs.state.failed')} detail={found.errorDetail} />
            )}
            {(remove.error ?? check.error ?? redownload.error) && (
              <p role="alert" className="text-primary text-sm wrap-anywhere">
                {(remove.error ?? check.error ?? redownload.error)?.message}
              </p>
            )}
            <div className="flex flex-wrap justify-end gap-2">
              <button type="button" onClick={close} className={`${BUTTON} bg-background`}>
                {t(
                  busy
                    ? 'downloads.stop.keep'
                    : state === 'done'
                      ? 'songs.button.keep'
                      : 'collections.cancel',
                )}
              </button>
              {state === 'done' && !found.problem && (
                <button
                  type="button"
                  disabled={check.isPending}
                  onClick={() => check.mutate([found.id])}
                  className={`${BUTTON} bg-background`}
                >
                  {check.isPending ? t('songs.checking') : t('songs.button.check')}
                </button>
              )}
              {state === 'done' && found.problem && (
                <button
                  type="button"
                  disabled={redownload.isPending}
                  onClick={() => {
                    close();
                    redownload.mutate([found.id]);
                  }}
                  className={`${BUTTON} bg-background`}
                >
                  {t('songs.button.again')}
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
              <button
                type="button"
                disabled={remove.isPending}
                onClick={() => {
                  close();
                  remove.mutate(found.id);
                }}
                className={`${BUTTON} bg-primary text-background`}
              >
                {found.replacing
                  ? t('songs.button.stopAgain')
                  : busy
                    ? t('downloads.stop.stop')
                    : t('songs.button.remove')}
              </button>
            </div>
          </div>
        </Popover>
      )}
    </>
  );
};
