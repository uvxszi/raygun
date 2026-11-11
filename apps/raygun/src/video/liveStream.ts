import { useState } from 'react';
import type { RefObject } from 'react';

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

/** What the player's tick reads of a live stream: its DVR window and where the engine plays it
 * when at live (ms on the video's clock). */
export type LiveView = { dvr: LiveWindow | null; livePoint: number | null };

/** A live stream as the tick sees it; null for anything else. */
export const readLive = (
  video: HTMLVideoElement | null,
  engine: MediaEngine | null,
  hinted: boolean,
): LiveView | null =>
  isLiveVideo(video, engine, hinted)
    ? { dvr: liveWindow(video), livePoint: engine?.livePoint() ?? null }
    : null;

/**
 * A live stream in the webview's player (as `PlayerLive.kt` on Android): where it is against its
 * live edge, going back to it, and holding the picture behind it so AI subtitles (which hear a
 * window once all its sound is out) are ready when their lines show. `view` is what the tick
 * read; `neededMs` is how far behind AI subtitles need the picture, as Rust measures it
 * (`Captions::live_delay_ms`) while they're made.
 */
export const useLiveStream = (
  view: LiveView | null,
  timeMs: number,
  neededMs: number | null,
  videoRef: RefObject<HTMLVideoElement | null>,
) => {
  const [heldMs, setHeldMs] = useState(0);
  const [declinedMs, setDeclinedMs] = useState(0);
  const live = view !== null;
  const dvr = view?.dvr ?? null;
  const edgeMs = dvr?.edgeMs ?? null;
  const livePoint = view?.livePoint ?? (edgeMs === null ? null : edgeMs - DEFAULT_BEHIND_MS);
  const behindMs = edgeMs === null ? 0 : Math.max(0, edgeMs - timeMs);
  const atLive = heldMs === 0 && (livePoint === null || timeMs >= livePoint - AT_LIVE_SLACK_MS);

  /** Holds the picture `ms` behind the newest segment (as far back as the stream keeps). */
  const hold = (ms: number) => {
    const video = videoRef.current;
    if (!video || !dvr) return;
    if (ms <= 0) {
      goLive();
      return;
    }
    const target = Math.max(dvr.startMs + 2_000, dvr.edgeMs - ms);
    video.currentTime = target / 1000;
    setHeldMs(ms);
  };
  const goLive = () => {
    const video = videoRef.current;
    if (!video) return;
    setHeldMs(0);
    // At the edge on purpose: AI subtitles don't ask again until they need more.
    setDeclinedMs((declined) => Math.max(declined, neededMs ?? 0));
    if (livePoint !== null) video.currentTime = livePoint / 1000;
    if (video.paused) void video.play().catch(() => {});
  };
  /** Held for AI subtitles before: keeps up as they need more, without asking (the player's
   * tick calls it). */
  const keepUp = () => {
    if (heldMs > 0 && neededMs && neededMs > heldMs + HOLD_SLACK_MS) hold(neededMs);
  };

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
    keepUp,
    decline: () => setDeclinedMs(neededMs ?? 0),
  };
};
