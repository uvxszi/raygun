import type { SongDownload } from '@raygun/bindings';
import { useNavigate } from '@tanstack/react-router';
import { useCallback, useEffect } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { dockSummary, IDLE_DOCK, useDownloadDockStore } from './downloadDockStore';
import { RecordingHost } from './RecordingHost';
import { useSongDownloadsSelect } from './useSongDownloads';

/** The ring's arc glides between reports where things may move. */
const RING_GLIDE =
  '[:root:is([data-motion=lively],[data-motion=calm])_&]:transition-[stroke-dashoffset] [:root:is([data-motion=lively],[data-motion=calm])_&]:duration-300 ease-linear';

/**
 * Above the player bar (above the tabs while nothing plays), in the layout, so it covers nothing:
 * the recording bar while songs are recorded, or, minimized, a round button on the right with a
 * ring for how far the songs on their way are and the number left inside, downloads included.
 * Tapping it brings the bar back, or with nothing to record opens Downloads → Music. Minimized
 * stays so for the session.
 */
export const DownloadDock: FC = () => {
  const { t } = useTranslation();
  const navigate = useNavigate();
  const minimized = useDownloadDockStore((state) => state.minimized);
  const since = useDownloadDockStore((state) => state.since);
  const setMinimized = useDownloadDockStore((state) => state.setMinimized);
  const follow = useDownloadDockStore((state) => state.follow);
  const select = useCallback((list: SongDownload[]) => dockSummary(list, since), [since]);
  const summary = useSongDownloadsSelect(select).data ?? IDLE_DOCK;
  const busy = summary.left > 0;
  useEffect(() => {
    follow(busy, Date.now());
  }, [busy, follow]);

  if (summary.recordings > 0 && !minimized) {
    return (
      <div data-testid="download-dock" className="shrink-0 px-2 pb-2 lg:px-4">
        <div className="mx-auto w-full max-w-md">
          <RecordingHost onMinimize={() => setMinimized(true)} />
        </div>
      </div>
    );
  }
  if (!busy) return null;

  const percent = Math.round(summary.progress * 100);
  const recording = summary.recordings > 0;
  const label = recording
    ? t('songs.dock.showRecording', { count: summary.left, percent })
    : t('songs.dock.openDownloads', { count: summary.left, percent });
  return (
    <div data-testid="download-dock" className="flex shrink-0 justify-end pr-3 pb-1.5 lg:pr-5">
      <button
        type="button"
        aria-label={label}
        title={label}
        onClick={() => {
          if (recording) setMinimized(false);
          else void navigate({ to: '/downloads', search: { tab: 'music' } });
        }}
        className="bg-surface motion-pop relative grid size-12 shrink-0 place-items-center rounded-full shadow-lg shadow-black/25"
      >
        <svg aria-hidden viewBox="0 0 48 48" className="absolute inset-0 size-full -rotate-90">
          <circle cx="24" cy="24" r="21" fill="none" strokeWidth="3" className="stroke-muted/25" />
          <circle
            data-testid="download-dock-ring"
            cx="24"
            cy="24"
            r="21"
            fill="none"
            strokeWidth="3"
            strokeLinecap={percent > 0 ? 'round' : 'butt'}
            pathLength={100}
            strokeDasharray="100"
            strokeDashoffset={100 - percent}
            className={`stroke-primary ${RING_GLIDE}`}
          />
        </svg>
        <span aria-hidden className="text-sm font-semibold tabular-nums">
          {summary.left > 99 ? '99+' : summary.left}
        </span>
      </button>
    </div>
  );
};
