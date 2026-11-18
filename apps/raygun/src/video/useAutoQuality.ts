import { useEffect, useEffectEvent, useRef } from 'react';
import type { RefObject } from 'react';
import { useTranslation } from 'react-i18next';

import { capOf, lowerStep, raiseStep, StallWatch } from './autoQuality';
import type { MediaEngine } from './mediaEngine';
import { sendRequest } from './protocol';
import { stepName } from './qualityText';
import { useVideoPlayerStore } from './videoPlayerStore';

/** How often stalls are looked at, ms. */
const LOOK_MS = 250;
/** A wait this soon after a seek is the seek's, not a stall. */
const SEEK_GRACE_MS = 600;

type Options = {
  videoRef: RefObject<HTMLVideoElement | null>;
  engine: MediaEngine | null;
  videoKey: string;
  /** Says a step down ("Lowered to 720p to keep playing"). */
  onLowered: (text: string) => void;
};

/**
 * Automatic quality that keeps the video playing ([`StallWatch`]): while nothing was picked by
 * hand (a step in Quality, a variant held, a link chosen in Sources), stalls step an HLS stream's
 * cap down a variant, or ask Rust for a lower step's link from where the video is (`quality` with
 * `auto`); a long smooth stretch lifts an HLS cap a step at a time. Never below the lowest.
 */
export const useAutoQuality = ({ videoRef, engine, videoKey, onLowered }: Options): void => {
  const { t } = useTranslation();
  const watch = useRef(new StallWatch());
  /** The cap in effect on the stream's variants (a tier), while stepped down. */
  const cap = useRef<number | null>(null);
  const stalled = useRef(false);

  const look = useEffectEvent((now: number) => {
    const video = videoRef.current;
    if (!video || !engine) return;
    const state = useVideoPlayerStore.getState();
    const locked =
      state.qualityWanted !== null || state.qualityHandPicked || engine.quality() !== -1;
    if (locked) return;
    const adaptive = engine.variants().length > 1;
    const action = watch.current.tick(
      now,
      !video.paused,
      stalled.current,
      adaptive && cap.current !== null,
    );
    if (action === 'lower') {
      const row = lowerStep(state.qualities, adaptive);
      if (!row) return;
      if (adaptive) {
        const height = capOf(row);
        if (!engine.capHeight(height)) return;
        cap.current = height;
      } else if (state.session) {
        sendRequest(videoKey, 'quality', video.currentTime * 1000, { key: row.key, auto: true });
      } else {
        return;
      }
      watch.current.stepped(now);
      onLowered(t('videoPlayer.qualityLowered', { n: stepName(row.height, row.range, t) }));
    } else if (action === 'raise' && cap.current !== null) {
      const row = raiseStep(state.qualities, cap.current);
      const holds = row ? engine.capHeight(capOf(row)) : false;
      if (!holds) engine.capHeight(null);
      cap.current = holds && row ? capOf(row) : null;
      watch.current.stepped(now);
    }
  });

  // Each video starts afresh: its stalls are its own, and a cap goes with the engine.
  useEffect(() => {
    const video = videoRef.current;
    if (!video || !engine) return;
    watch.current.reset(performance.now());
    cap.current = null;
    stalled.current = false;
    let begun = false;
    let seekedAt = 0;
    const onPlaying = () => {
      begun = true;
      stalled.current = false;
    };
    const onReady = () => {
      stalled.current = false;
    };
    const onWaiting = () => {
      // Running dry while playing: not before the first frame, not a seek's wait.
      if (begun && performance.now() - seekedAt > SEEK_GRACE_MS && !video.seeking) {
        stalled.current = true;
      }
    };
    const onSeeking = () => {
      seekedAt = performance.now();
    };
    video.addEventListener('playing', onPlaying);
    video.addEventListener('canplay', onReady);
    video.addEventListener('waiting', onWaiting);
    video.addEventListener('seeking', onSeeking);
    const timer = window.setInterval(() => look(performance.now()), LOOK_MS);
    return () => {
      video.removeEventListener('playing', onPlaying);
      video.removeEventListener('canplay', onReady);
      video.removeEventListener('waiting', onWaiting);
      video.removeEventListener('seeking', onSeeking);
      window.clearInterval(timer);
    };
  }, [engine, videoRef]);
};
