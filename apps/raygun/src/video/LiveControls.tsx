import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { formatTime } from '../player/formatTime';

/** "● LIVE": red at the live edge, grey behind it; a click goes back to the edge. */
export const LiveBadge: FC<{ atLive: boolean; behindMs: number; onGoLive: () => void }> = ({
  atLive,
  behindMs,
  onGoLive,
}) => {
  const { t } = useTranslation();
  return (
    <span className="ml-2 flex items-center gap-2 text-sm tabular-nums">
      <button
        type="button"
        aria-label={t('videoPlayer.goLive')}
        title={t('videoPlayer.goLive')}
        onClick={onGoLive}
        className="flex items-center gap-1.5 rounded-full bg-black/40 px-2.5 py-1 text-xs font-bold hover:bg-white/15"
      >
        <span
          aria-hidden
          className={`size-2 rounded-full ${atLive ? 'bg-red-600' : 'bg-neutral-400'}`}
        />
        {t('videoPlayer.live')}
      </button>
      {!atLive && behindMs > 0 && <span className="text-white/80">−{formatTime(behindMs)}</span>}
    </span>
  );
};

/** The offer to hold a live stream behind its edge for AI subtitles. */
export const LiveDelayPrompt: FC<{
  neededMs: number;
  onDelay: () => void;
  onDecline: () => void;
}> = ({ neededMs, onDelay, onDecline }) => {
  const { t } = useTranslation();
  return (
    <div
      role="status"
      className="absolute right-6 bottom-28 z-10 flex max-w-md items-center gap-2 rounded-xl bg-neutral-900/95 p-2 pl-4 shadow-xl ring-1 ring-white/10"
    >
      <span className="text-sm">
        {t('videoPlayer.liveDelayAsk', {
          n: t('videoPlayer.seconds', { n: Math.round(neededMs / 1000) }),
        })}
      </span>
      <button
        type="button"
        onClick={onDecline}
        className="rounded-lg px-3 py-1.5 text-sm hover:bg-white/10"
      >
        {t('videoPlayer.notNow')}
      </button>
      <button
        type="button"
        onClick={onDelay}
        className="bg-primary text-background rounded-lg px-3 py-1.5 text-sm font-semibold"
      >
        {t('videoPlayer.liveDelay')}
      </button>
    </div>
  );
};
