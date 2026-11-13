import { useEffect, useEffectEvent, useRef } from 'react';
import type { FC, RefObject } from 'react';

import { EffectsRenderer } from './glRenderer';
import { heavyKinds, isNeutral, shaderParams } from './effectParams';
import type { Effects, HeavyKind } from './effectParams';

type Props = {
  videoRef: RefObject<HTMLVideoElement | null>;
  /** How the picture fits, as the video's own (`object-contain`…), so the canvas covers it. */
  fit: string;
  /** The effects that play (any dropped for speed already left out). */
  effects: Effects;
  /** Frames went missing with heavy effects on: the heaviest should go. */
  onTooSlow: (kind: HeavyKind) => void;
  /** No WebGL 2 here: the picture plays as it is. */
  onUnavailable: () => void;
};

/** How often missed frames are counted up, and the share of them that's too many. */
const WATCH_MS = 3_000;
const MISSED_SHARE = 0.15;
const MISSED_LEAST = 10;

type FrameCallback = (
  now: number,
  metadata: { mediaTime: number; presentedFrames: number },
) => void;
type FrameVideo = HTMLVideoElement & {
  requestVideoFrameCallback?: (callback: FrameCallback) => number;
  cancelVideoFrameCallback?: (handle: number) => void;
};

/**
 * The picture's effects over the desktop player's `<video>` (`glRenderer.ts`): a canvas of the
 * same fit drawn with each new frame (`requestVideoFrameCallback`), at once while paused. Only
 * there while an effect is on; clicks go through to the video. Frames the drawing couldn't keep up
 * with (presented frames skipped between callbacks) drop the heaviest effect.
 */
export const VideoEffectsLayer: FC<Props> = ({
  videoRef,
  fit,
  effects,
  onTooSlow,
  onUnavailable,
}) => {
  const active = !isNeutral(effects);
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const renderer = useRef<EffectsRenderer | null>(null);
  const params = useRef(shaderParams(effects));
  const heavy = useRef(heavyKinds(effects));
  const lastTime = useRef(Number.NaN);
  const tooSlow = useEffectEvent((kind: HeavyKind) => onTooSlow(kind));
  const unavailable = useEffectEvent(() => onUnavailable());

  // New values apply from the next frame; paused, the frame shows them at once.
  useEffect(() => {
    params.current = shaderParams(effects);
    heavy.current = heavyKinds(effects);
    const video = videoRef.current;
    if (video?.paused && renderer.current && Number.isFinite(lastTime.current)) {
      try {
        renderer.current.draw(video, params.current, lastTime.current);
      } catch {
        // A frame that can't be read yet; the next one draws.
      }
    }
  }, [effects, videoRef]);

  useEffect(() => {
    const video = videoRef.current as FrameVideo | null;
    const canvas = canvasRef.current;
    if (!active || !video || !canvas) return;
    let drawing: EffectsRenderer;
    try {
      drawing = new EffectsRenderer(canvas);
    } catch {
      unavailable();
      return;
    }
    renderer.current = drawing;
    let stopped = false;
    let handle = 0;
    const counts = { presented: -1, shown: 0, missed: 0, since: performance.now() };

    const draw = (time: number) => {
      lastTime.current = time;
      try {
        drawing.draw(video, params.current, time);
      } catch {
        // A frame that can't be read yet; the next one draws.
      }
    };
    const count = (presented: number) => {
      if (counts.presented >= 0 && presented > counts.presented) {
        counts.shown += 1;
        counts.missed += presented - counts.presented - 1;
      }
      counts.presented = presented;
      const now = performance.now();
      if (now - counts.since < WATCH_MS) return;
      const total = counts.shown + counts.missed;
      const [heaviest] = heavy.current;
      if (
        heaviest &&
        !video.paused &&
        video.playbackRate <= 1.5 &&
        counts.missed >= MISSED_LEAST &&
        counts.missed > total * MISSED_SHARE
      ) {
        tooSlow(heaviest);
      }
      counts.shown = 0;
      counts.missed = 0;
      counts.since = now;
    };
    const onFrame: FrameCallback = (_now, metadata) => {
      if (stopped) return;
      draw(metadata.mediaTime);
      count(metadata.presentedFrames);
      handle = video.requestVideoFrameCallback?.(onFrame) ?? 0;
    };
    // Without frame callbacks: every screen refresh, drawing when the time moved.
    const onRefresh = () => {
      if (stopped) return;
      if (video.currentTime !== lastTime.current) draw(video.currentTime);
      handle = window.requestAnimationFrame(onRefresh);
    };
    const frames = typeof video.requestVideoFrameCallback === 'function';
    if (frames) handle = video.requestVideoFrameCallback?.(onFrame) ?? 0;
    else handle = window.requestAnimationFrame(onRefresh);
    const onSeeking = () => drawing.restart();
    video.addEventListener('seeking', onSeeking);
    if (video.readyState >= 2) draw(video.currentTime);
    return () => {
      stopped = true;
      if (frames) video.cancelVideoFrameCallback?.(handle);
      else window.cancelAnimationFrame(handle);
      video.removeEventListener('seeking', onSeeking);
      drawing.dispose();
      renderer.current = null;
      lastTime.current = Number.NaN;
    };
  }, [active, videoRef]);

  if (!active) return null;
  return (
    <canvas
      ref={canvasRef}
      aria-hidden
      data-testid="video-effects"
      className={`pointer-events-none absolute inset-0 size-full ${fit}`}
    />
  );
};
