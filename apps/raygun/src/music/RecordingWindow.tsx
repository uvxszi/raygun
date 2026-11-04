import { commands } from '@raygun/bindings';
import { CircleStop, Disc3 } from 'lucide-react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { Artwork } from '../library/Artwork';
import { formatTime } from '../player/formatTime';
import { usePlaybackState } from '../player/usePlayback';
import { useLivePosition } from '../player/useLivePosition';
import { SPIN } from './songDownloadState';
import { useSongDownloadsSelect, useSongDownloadSync } from './useSongDownloads';

/**
 * A song being recorded (M46.46), in its own window over the app: the song, how far it is, and
 * Cancel. The player can't be paused, sought or changed meanwhile (Rust refuses it); the song
 * plays from its start trim to its end trim. The shell keeps it mounted, and it is also where
 * download changes are listened for, once for every list.
 */
export const RecordingWindow: FC = () => {
  useSongDownloadSync();
  const recording = useSongDownloadsSelect(
    (list) => list.find((song) => song.state === 'recording') ?? null,
  ).data;
  if (!recording) return null;
  return (
    <RecordingCard title={recording.title} artist={recording.artist} art={recording.artworkUrl} />
  );
};

const RecordingCard: FC<{ title: string; artist: string | null; art: string | null }> = ({
  title,
  artist,
  art,
}) => {
  const { t } = useTranslation();
  const state = usePlaybackState().data;
  const positionMs = useLivePosition(state);
  const durationMs = state?.durationMs ?? 0;
  const progress = durationMs > 0 ? Math.min(1, positionMs / durationMs) : 0;
  return (
    <div
      role="dialog"
      aria-modal="true"
      aria-labelledby="recording-title"
      data-testid="recording-window"
      className="fixed inset-0 z-[95] grid place-items-center bg-black/60 p-4 backdrop-blur-sm"
    >
      <div className="bg-surface motion-pop flex w-full max-w-sm flex-col items-center gap-4 rounded-3xl p-6 text-center shadow-2xl">
        <span className="text-primary flex items-center gap-2 text-xs font-semibold tracking-wide uppercase">
          <Disc3 aria-hidden size={14} className={SPIN} />
          {t('songs.recordingNow')}
        </span>
        <Artwork url={art} className="size-40 rounded-xl shadow-lg" iconSize={40} />
        <div className="flex min-w-0 flex-col gap-0.5">
          <h2 id="recording-title" className="truncate text-lg font-bold">
            {title}
          </h2>
          {artist && <p className="text-muted truncate text-sm">{artist}</p>}
        </div>
        <div className="flex w-full flex-col gap-1">
          <div
            role="progressbar"
            aria-label={t('songs.recordingProgress')}
            aria-valuemin={0}
            aria-valuemax={100}
            aria-valuenow={Math.round(progress * 100)}
            className="bg-background h-1.5 w-full overflow-hidden rounded-full"
          >
            {/* Scaled, not resized: the bar moves without laying the window out again. */}
            <div
              className="bg-primary h-full w-full origin-left"
              style={{ transform: `scaleX(${progress})` }}
            />
          </div>
          <div className="text-muted flex justify-between text-xs tabular-nums">
            <span>{formatTime(positionMs)}</span>
            <span>{durationMs > 0 ? formatTime(durationMs) : ''}</span>
          </div>
        </div>
        <p className="text-muted text-xs">{t('songs.recordingLocked')}</p>
        <button
          type="button"
          onClick={() => void commands.songDownloadCancelRecording()}
          className="bg-background hover:bg-primary/15 flex items-center gap-2 rounded-full px-5 py-2 text-sm font-semibold"
        >
          <CircleStop aria-hidden size={16} />
          {t('songs.recordingCancel')}
        </button>
      </div>
    </div>
  );
};
