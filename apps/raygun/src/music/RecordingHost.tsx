import { commands } from '@raygun/bindings';
import type { SongDownload } from '@raygun/bindings';
import { ChevronDown, CircleStop, Disc3, X } from 'lucide-react';
import { useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { GLIDE, SPIN, recordingsOf, shareOf } from './songDownloadState';
import { useSongDownloadsSelect } from './useSongDownloads';

const NONE: SongDownload[] = [];

const percent = (share: number | null) => (share === null ? null : `${Math.round(share * 100)}%`);

/**
 * One card for every song being recorded or waiting to be (M46.52): songs only YouTube's player
 * plays, recorded one after another in the background from hidden players of their own, silent,
 * so whatever plays goes on. It says how many, which one records now and how far it is, opens to
 * the whole queue, and stops one or all (they keep what was recorded and wait for Retry). Put
 * away, it comes back when another song is added to record.
 */
export const RecordingHost: FC = () => {
  const { t } = useTranslation();
  const recordings = useSongDownloadsSelect(recordingsOf).data ?? NONE;
  const [open, setOpen] = useState(false);
  const [hidden, setHidden] = useState<ReadonlySet<string>>(new Set());
  if (recordings.length === 0 || recordings.every((item) => hidden.has(item.id))) return null;

  const current = recordings.find((item) => item.state === 'downloading') ?? null;
  const share = current ? shareOf(current) : null;
  const line = current
    ? [t('songs.recording.now', { title: current.title }), percent(share)]
        .filter(Boolean)
        .join(' · ')
    : (recordings[0]?.error ?? t('songs.recording.waiting'));
  const stop = (id: string | null) => void commands.songDownloadCancelRecording(id);

  return (
    <section
      aria-label={t('songs.recording.title', { count: recordings.length })}
      data-testid="recording-card"
      className="bg-surface fixed inset-x-4 bottom-[calc(env(safe-area-inset-bottom)+5.5rem)] z-[70] mx-auto flex max-w-sm flex-col gap-2 rounded-2xl p-3 shadow-xl"
    >
      <div className="flex items-center gap-3">
        <Disc3 aria-hidden size={20} className={`text-primary shrink-0 ${current ? SPIN : ''}`} />
        <div className="flex min-w-0 flex-1 flex-col">
          <p className="truncate text-sm font-semibold">
            {t('songs.recording.title', { count: recordings.length })}
          </p>
          <p className="text-muted truncate text-xs">{line}</p>
        </div>
        <button
          type="button"
          aria-expanded={open}
          aria-label={open ? t('songs.recording.hideQueue') : t('songs.recording.showQueue')}
          onClick={() => setOpen((shown) => !shown)}
          className="hover:bg-primary/15 shrink-0 rounded-full p-2"
        >
          <ChevronDown aria-hidden size={18} className={open ? 'rotate-180' : undefined} />
        </button>
        <button
          type="button"
          aria-label={t('songs.recording.stopAll')}
          onClick={() => stop(null)}
          className="hover:bg-primary/15 shrink-0 rounded-full p-2"
        >
          <CircleStop aria-hidden size={18} />
        </button>
        <button
          type="button"
          aria-label={t('songs.recording.hide')}
          onClick={() => setHidden(new Set(recordings.map((item) => item.id)))}
          className="hover:bg-primary/15 shrink-0 rounded-full p-2"
        >
          <X aria-hidden size={18} />
        </button>
      </div>
      {current && (
        <div
          role="progressbar"
          aria-label={t('songs.recording.progress')}
          aria-valuemin={0}
          aria-valuemax={100}
          aria-valuenow={Math.round((share ?? 0) * 100)}
          className="bg-background h-1 w-full overflow-hidden rounded-full"
        >
          <div
            className={`bg-primary h-full w-full origin-left ${GLIDE}`}
            style={{ transform: `scaleX(${share ?? 0})` }}
          />
        </div>
      )}
      <p className="text-muted text-xs">{t('songs.recording.background')}</p>
      {open && (
        <ol className="flex max-h-48 flex-col gap-1 overflow-y-auto" data-testid="recording-queue">
          {recordings.map((item) => (
            <li key={item.id} className="flex items-center gap-2 text-sm">
              <span className="min-w-0 flex-1 truncate">
                {item.artist ? `${item.title} – ${item.artist}` : item.title}
              </span>
              <span className="text-muted shrink-0 text-xs tabular-nums">
                {item.state === 'downloading'
                  ? (percent(shareOf(item)) ?? t('songs.state.recording'))
                  : t('songs.recording.waiting')}
              </span>
              <button
                type="button"
                aria-label={t('songs.recording.stop', { title: item.title })}
                onClick={() => stop(item.id)}
                className="hover:bg-primary/15 shrink-0 rounded-full p-1"
              >
                <X aria-hidden size={14} />
              </button>
            </li>
          ))}
        </ol>
      )}
    </section>
  );
};
