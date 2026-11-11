import { useEffect, useEffectEvent, useState } from 'react';
import type { FC } from 'react';
import { useTranslation } from 'react-i18next';

import { formatTime } from '../player/formatTime';
import type { MediaEngine } from './mediaEngine';

/** Less than this to seek in, and a live stream's time bar hides. */
export const DVR_MIN_MS = 60_000;
/** Within this of the engine's live point, the picture counts as live. */
const AT_LIVE_SLACK_MS = 3_000;
/** Where a live point is taken to be without the engine's word: this far behind the edge. */
const DEFAULT_BEHIND_MS = 10_000;
/** A hold this close to what AI subtitles need is enough. */
const HOLD_SLACK_MS = 3_000;
/** Steps of the live delay buttons. */
export const HOLD_STEP_MS = 5_000;

/** A live stream's seekable window on the video's clock, ms. */
export type LiveWindow = { startMs: number; edgeMs: number };

/** The stream's seekable window (its DVR window when live); null when not known. */
export const liveWindow = (video: HTMLVideoElement | null): LiveWindow | null => {
  const ranges = video?.seekable;
  if (!ranges || ranges.length === 0) return null;
  const startMs = ranges.start(0) * 1000;
  const edgeMs = ranges.end(ranges.length - 1) * 1000;
  return Number.isFinite(edgeMs) && edgeMs > startMs ? { startMs, edgeMs } : null;
};

/** Whether what plays is live: the request says so, the engine knows by its playlist, or the
 * element's length is endless. */
export const isLiveVideo = (
  video: HTMLVideoElement | null,
  engine: MediaEngine | null,
  hinted: boolean,
): boolean => hinted || Boolean(engine?.isLive()) || video?.duration === Infinity;

/**
 * A live stream in the webview's player (as `PlayerLive.kt` on Android): where it is against its
 * live edge, going back to it, and holding the picture behind it so AI subtitles (which hear a
 * window once all its sound is out) are ready when their lines show. `neededMs` is how far
 * behind that takes, as Rust measures it (`Captions::live_delay_ms`) while they're made.
 */
export const useLiveStream = (
  video: HTMLVideoElement | null,
  engine: MediaEngine | null,
  hinted: boolean,
  neededMs: number | null,
) => {
  const [heldMs, setHeldMs] = useState(0);
  const [declinedMs, setDeclinedMs] = useState(0);
  const live = isLiveVideo(video, engine, hinted);
  const dvr = live ? liveWindow(video) : null;
  const timeMs = (video?.currentTime ?? 0) * 1000;
  const edgeMs = dvr?.edgeMs ?? null;
  const livePoint = engine?.livePoint() ?? (edgeMs === null ? null : edgeMs - DEFAULT_BEHIND_MS);
  const behindMs = edgeMs === null ? 0 : Math.max(0, edgeMs - timeMs);
  const atLive = heldMs === 0 && (livePoint === null || timeMs >= livePoint - AT_LIVE_SLACK_MS);

  /** Holds the picture `ms` behind the newest segment (as far back as the stream keeps). */
  const hold = (ms: number) => {
    if (!video || !dvr) return;
    const target = Math.max(dvr.startMs + 2_000, dvr.edgeMs - ms);
    video.currentTime = target / 1000;
    setHeldMs(ms);
  };
  const goLive = () => {
    if (!video) return;
    setHeldMs(0);
    // At the edge on purpose: AI subtitles don't ask again until they need more.
    setDeclinedMs((declined) => Math.max(declined, neededMs ?? 0));
    if (livePoint !== null) video.currentTime = livePoint / 1000;
    if (video.paused) void video.play().catch(() => {});
  };
  // Held for AI subtitles before: keep up as they need more, without asking.
  const keepUp = useEffectEvent(() => {
    if (heldMs > 0 && neededMs && neededMs > heldMs + HOLD_SLACK_MS) hold(neededMs);
  });
  useEffect(() => keepUp(), [neededMs]);

  const offer =
    live &&
    neededMs !== null &&
    heldMs === 0 &&
    behindMs < neededMs - HOLD_SLACK_MS &&
    neededMs > declinedMs + HOLD_SLACK_MS;
  return {
    live,
    dvr,
    behindMs,
    atLive,
    heldMs,
    offer,
    neededMs,
    hold,
    goLive,
    decline: () => setDeclinedMs(neededMs ?? 0),
  };
};

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
      {!atLive && behindMs > 0 && (
        <span className="text-white/80">−{formatTime(behindMs)}</span>
      )}
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
        {t('videoPlayer.liveDelayAsk', { n: t('videoPlayer.seconds', { n: Math.round(neededMs / 1000) }) })}
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
