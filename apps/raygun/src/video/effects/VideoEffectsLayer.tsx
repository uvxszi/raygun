import { useEffect, useEffectEvent, useRef } from 'react';
import type { FC, RefObject } from 'react';

import { EffectsRenderer } from './glRenderer';
import type { MotionSources } from './glRenderer';
import { heavyKinds, isNeutral, shaderParams } from './effectParams';
import type { Effects, HeavyKind } from './effectParams';
import { missesRefreshes, progressAt } from './motionMath';
import type { Watermark, WatermarkMask } from './watermarkMath';
import { WatermarkSampler } from './watermarkSampler';

/** The watermark remover's part: Find it asked for, and what the frames told. */
export type WatermarkHooks = {
  /** Find it was asked for: the whole picture is sampled until the answer comes. */
  finding: boolean;
  /** A mask was found for the box (true), or it's learning again (false). */
  onMask: (found: boolean) => void;
  /** Finding's answer: a box, or none (unsure: too little moved to tell). */
  onFound: (box: Pick<Watermark, 'x' | 'y' | 'width' | 'height'> | null, unsure: boolean) => void;
  /** Scene changes seen while finding (an answer needs two). */
  onProgress?: (cuts: number) => void;
};

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
  watermark?: WatermarkHooks;
  /** Smooth motion's shaders, from its chip (none: `effects` has it off). */
  motion?: MotionSources | null;
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
 * same fit drawn with each new frame (`requestVideoFrameCallback`), and again at once while paused
 * when the values change. Only there while an effect is on; clicks go through to the video. Frames
 * the drawing couldn't keep up with (presented frames skipped between callbacks) drop the heaviest
 * effect.
 *
 * With smooth motion, each frame is kept and every screen refresh draws the picture between the
 * last two (`present`), trailing the video by one frame; refreshes missed too often while it draws
 * turn it off.
 */
export const VideoEffectsLayer: FC<Props> = ({
  videoRef,
  fit,
  effects,
  onTooSlow,
  onUnavailable,
  watermark,
  motion = null,
}) => {
  const finding = watermark?.finding ?? false;
  // Find it samples the picture even before there's a box.
  const active = !isNeutral(effects) || finding;
  const canvasRef = useRef<HTMLCanvasElement>(null);
  const params = useRef(shaderParams(effects));
  const smooth = useRef(effects.smoothMotion);
  const model = useRef(effects.motionModel);
  const heavy = useRef(heavyKinds(effects));
  const lastTime = useRef(Number.NaN);
  const redraw = useRef<() => void>(() => {});
  const tooSlow = useEffectEvent((kind: HeavyKind) => onTooSlow(kind));
  const unavailable = useEffectEvent(() => onUnavailable());
  const maskFound = useEffectEvent((found: boolean) => watermark?.onMask(found));
  const overlayFound = useEffectEvent(
    (box: Pick<Watermark, 'x' | 'y' | 'width' | 'height'> | null, unsure: boolean) =>
      watermark?.onFound(box, unsure),
  );
  const findProgress = useEffectEvent((cuts: number) => watermark?.onProgress?.(cuts));
  const box = useRef(effects.watermark);
  const findingNow = useRef(finding);
  const renderer = useRef<EffectsRenderer | null>(null);
  const sampler = useRef<WatermarkSampler | null>(null);

  // New values apply from the next frame; paused, the frame shows them at once.
  useEffect(() => {
    params.current = shaderParams(effects);
    smooth.current = effects.smoothMotion;
    model.current = effects.motionModel;
    renderer.current?.setMotionModel(effects.motionModel);
    heavy.current = heavyKinds(effects);
    box.current = effects.watermark;
    renderer.current?.setWatermark(effects.watermark);
    sampler.current?.setWatermark(effects.watermark);
    if (videoRef.current?.paused) redraw.current();
  }, [effects, videoRef]);

  // Find it: the sampler looks until it can tell (the layer is on meanwhile).
  useEffect(() => {
    findingNow.current = finding;
    if (finding && sampler.current && !sampler.current.finding) sampler.current.find();
  }, [finding]);

  useEffect(() => {
    const video = videoRef.current as FrameVideo | null;
    const canvas = canvasRef.current;
    if (!active || !video || !canvas) return;
    let drawing: EffectsRenderer;
    try {
      drawing = new EffectsRenderer(canvas, motion);
    } catch {
      unavailable();
      return;
    }
    drawing.setWatermark(box.current);
    drawing.setMotionModel(model.current);
    renderer.current = drawing;
    const boxFrames = new WatermarkSampler(
      (mask: WatermarkMask | null) => {
        drawing.setWatermarkMask(mask);
        maskFound(mask !== null);
        if (video.paused) redraw.current();
      },
      (found, unsure) => overlayFound(found, unsure),
      (cuts) => findProgress(cuts),
    );
    boxFrames.setWatermark(box.current);
    if (findingNow.current) boxFrames.find();
    sampler.current = boxFrames;
    let stopped = false;
    let handle = 0;
    let refresh = 0;
    const counts = { presented: -1, shown: 0, missed: 0, since: performance.now() };
    // Smooth motion: when the latest frame came, the media time between it and the one before,
    // and the screen's refresh intervals while it draws.
    const pair = { at: 0, gap: 0, time: Number.NaN };
    const refreshes: number[] = [];
    let lastRefresh = 0;

    const draw = (time: number, at: number) => {
      try {
        if (smooth.current) {
          drawing.keep(video, params.current, time);
          if (time !== lastTime.current) {
            pair.gap = Number.isFinite(pair.time) ? time - pair.time : 0;
            pair.time = time;
            pair.at = at;
          }
          if (video.paused || !drawing.hasPair) drawing.present(1);
        } else {
          drawing.draw(video, params.current, time);
        }
      } catch {
        // A frame that can't be read yet; the next one draws.
      }
      if (time !== lastTime.current) boxFrames.frame(video);
      lastTime.current = time;
    };
    // Paused: the frame shown is drawn again with the new values at once.
    redraw.current = () => {
      if (stopped || !video.paused || video.readyState < 2) return;
      const time = Number.isFinite(lastTime.current) ? lastTime.current : video.currentTime;
      draw(time, performance.now());
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
      const playing = !video.paused && video.playbackRate <= 1.5;
      if (
        heaviest &&
        playing &&
        counts.missed >= MISSED_LEAST &&
        counts.missed > total * MISSED_SHARE
      ) {
        tooSlow(heaviest);
      } else if (smooth.current && playing && missesRefreshes(refreshes)) {
        tooSlow('smoothMotion');
      }
      counts.shown = 0;
      counts.missed = 0;
      counts.since = now;
      refreshes.length = 0;
    };
    const onFrame: FrameCallback = (now, metadata) => {
      if (stopped) return;
      draw(metadata.mediaTime, now);
      count(metadata.presentedFrames);
      handle = video.requestVideoFrameCallback?.(onFrame) ?? 0;
    };
    // Without frame callbacks: every screen refresh, drawing when the time moved.
    const onRefresh = (now: number) => {
      if (stopped) return;
      if (video.currentTime !== lastTime.current) draw(video.currentTime, now);
      handle = window.requestAnimationFrame(onRefresh);
    };
    // Smooth motion: every screen refresh draws the picture between the last two frames.
    const onMotion = (now: number) => {
      if (stopped) return;
      if (smooth.current && !video.paused && drawing.hasPair) {
        if (lastRefresh) refreshes.push(now - lastRefresh);
        lastRefresh = now;
        const t = progressAt(now - pair.at, pair.gap, video.playbackRate);
        try {
          drawing.present(t ?? 1);
        } catch {
          // Drawn again at the next refresh.
        }
      } else {
        lastRefresh = 0;
      }
      refresh = window.requestAnimationFrame(onMotion);
    };
    const frames = typeof video.requestVideoFrameCallback === 'function';
    if (frames) handle = video.requestVideoFrameCallback?.(onFrame) ?? 0;
    else handle = window.requestAnimationFrame(onRefresh);
    refresh = window.requestAnimationFrame(onMotion);
    const onSeeking = () => {
      drawing.restart();
      pair.time = Number.NaN;
    };
    video.addEventListener('seeking', onSeeking);
    if (video.readyState >= 2) draw(video.currentTime, performance.now());
    return () => {
      stopped = true;
      if (frames) video.cancelVideoFrameCallback?.(handle);
      else window.cancelAnimationFrame(handle);
      window.cancelAnimationFrame(refresh);
      video.removeEventListener('seeking', onSeeking);
      drawing.dispose();
      renderer.current = null;
      sampler.current = null;
      redraw.current = () => {};
      lastTime.current = Number.NaN;
    };
  }, [active, videoRef, motion]);

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
