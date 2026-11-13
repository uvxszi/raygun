import { commands } from '@raygun/bindings';
import { useEffect, useEffectEvent, useRef } from 'react';
import type { RefObject } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { catchUp, soundTime } from './externalAudio';
import type { SoundSource } from './externalAudio';
import { attachMedia, engineKind } from './mediaEngine';
import type { MediaEngine } from './mediaEngine';

/** How often the sound is checked against the picture. */
const STEP_MS = 250;
/** `HTMLMediaElement.HAVE_FUTURE_DATA`: enough to play on. */
const HAVE_FUTURE_DATA = 3;

type Options = {
  videoRef: RefObject<HTMLVideoElement | null>;
  /** What plays as the sound; null leaves the video's own. */
  source: SoundSource | null;
  /** Positive plays the sound later than the picture. */
  delayMs: number;
  volume: number;
  muted: boolean;
  /** The sound couldn't play: the engine's words. */
  onFailed: (message: string) => void;
};

/**
 * Plays `source` as the video's sound in a media element of its own, kept in step with the
 * picture (`externalAudio`): it plays, pauses, jumps and changes pace with the video, `delayMs`
 * behind it; a drift is caught up by pace or by a jump. The player mutes the video meanwhile.
 * Streams go through the same engines as the video (hls.js, dash.js, the webview's own); another
 * source's is linked through the stream proxy with its headers first (`video_media_link`).
 */
export const useExternalAudio = ({
  videoRef,
  source,
  delayMs,
  volume,
  muted,
  onFailed,
}: Options) => {
  const audioRef = useRef<HTMLAudioElement | null>(null);
  const delayRef = useRef(delayMs);
  /** Puts the sound in step now (a jump when `jump`). */
  const syncRef = useRef<((jump: boolean) => void) | null>(null);
  const failed = useEffectEvent((message: string) => onFailed(message));
  /** Another source's stream is linked through the proxy, with its headers, once picked. */
  const linkOf = useEffectEvent(async (address: string) =>
    source?.offer
      ? unwrapResult(commands.videoMediaLink(address, source.offer.headers ?? {}))
      : address,
  );
  const url = source?.url ?? null;
  const format = source?.format ?? null;

  useEffect(() => {
    const video = videoRef.current;
    if (!url || !video) return;
    const audio = document.createElement('audio');
    audio.preload = 'auto';
    audioRef.current = audio;
    let engine: MediaEngine | null = null;
    let cancelled = false;

    const sync = (jump: boolean) => {
      const target = soundTime(video.currentTime, delayRef.current);
      const silent = video.paused || video.ended || target < 0;
      if (silent || (Number.isFinite(audio.duration) && target >= audio.duration)) {
        if (!audio.paused) audio.pause();
        // A later sound waits at its start until the picture reaches it.
        if (target < 0 && audio.currentTime > 0) audio.currentTime = 0;
        return;
      }
      if (jump || audio.paused) {
        audio.currentTime = target;
        audio.playbackRate = video.playbackRate;
        if (audio.paused) void audio.play().catch(() => {});
        return;
      }
      // Still loading: it's put in step once it plays.
      if (audio.readyState < HAVE_FUTURE_DATA) return;
      const step = catchUp(audio.currentTime, target, video.playbackRate);
      if ('jumpTo' in step) audio.currentTime = step.jumpTo;
      else if (audio.playbackRate !== step.rate) audio.playbackRate = step.rate;
    };
    syncRef.current = sync;

    const onVideo = (event: Event) => {
      // The picture waits for data: so does the sound.
      if (event.type === 'waiting') {
        audio.pause();
        return;
      }
      sync(event.type === 'seeked' || event.type === 'playing' || event.type === 'ratechange');
    };
    const kinds = ['play', 'playing', 'pause', 'seeked', 'ratechange', 'waiting', 'ended'];
    for (const kind of kinds) video.addEventListener(kind, onVideo);
    const timer = window.setInterval(() => sync(false), STEP_MS);

    // An `<audio>` takes what a `<video>` does: the engines only use what both have.
    linkOf(url)
      .then((address) =>
        cancelled
          ? null
          : attachMedia(audio as unknown as HTMLVideoElement, address, engineKind(url, format), {
              onTracks: () => {},
              onFatal: (message) => {
                if (!cancelled) failed(message);
              },
            }),
      )
      .then((attached) => {
        if (!attached) return;
        if (cancelled) {
          attached.destroy();
          return;
        }
        engine = attached;
        sync(true);
      })
      .catch((error: unknown) => {
        if (!cancelled) failed(String(error));
      });

    return () => {
      cancelled = true;
      window.clearInterval(timer);
      for (const kind of kinds) video.removeEventListener(kind, onVideo);
      engine?.destroy();
      audio.pause();
      audio.removeAttribute('src');
      audio.load();
      if (audioRef.current === audio) audioRef.current = null;
      if (syncRef.current === sync) syncRef.current = null;
    };
  }, [url, format, videoRef]);

  // A new delay puts the sound in step at once.
  useEffect(() => {
    delayRef.current = delayMs;
    syncRef.current?.(true);
  }, [delayMs]);

  useEffect(() => {
    const audio = audioRef.current;
    if (!audio) return;
    audio.volume = Math.max(0, Math.min(1, volume));
    audio.muted = muted;
  }, [volume, muted, url]);
};
