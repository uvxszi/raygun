import { useEffect, useRef, useState } from 'react';
import type { RefObject } from 'react';

import type { Cue } from './subtitles';

/**
 * Subtitle timing (desktop): lines show at the playhead minus the subtitle delay, any delay
 * (negative shows them earlier; one longer than the video moves every line past its end), and
 * follow the playhead exactly: the clock is read every frame while the video plays, so lines
 * change on their own times at any speed, after a seek and when the delay changes, not at the
 * element's few `timeupdate`s a second.
 */

/** The time subtitles are read at: the playhead moved back by the delay. */
export const subtitleTimeMs = (positionMs: number, delayMs: number) => positionMs - delayMs;

/**
 * The stretch around `atMs` in which the lines of `lists` showing don't change: from the last
 * start or end at or before it, until the first after it.
 */
export const steadySpan = (lists: readonly (readonly Cue[])[], atMs: number) => {
  let from = Number.NEGATIVE_INFINITY;
  let until = Number.POSITIVE_INFINITY;
  for (const cues of lists) {
    for (const cue of cues) {
      for (const edge of [cue.startMs, cue.endMs]) {
        if (edge <= atMs) from = Math.max(from, edge);
        else until = Math.min(until, edge);
      }
    }
  }
  return { from, until };
};

/** A text track cue's lines, without markup. */
const cueLines = (text: string) =>
  text
    .replace(/<[^>]*>/g, '')
    .split('\n')
    .map((line) => line.trim())
    .filter(Boolean);

/** The lines of the stream's own (hidden) text tracks showing at `atMs` of the video's time. */
export const embeddedLinesAt = (video: HTMLVideoElement, atMs: number): string[] => {
  const lines: string[] = [];
  const at = atMs / 1000;
  for (const track of Array.from(video.textTracks ?? [])) {
    if (track.mode !== 'hidden' || !track.cues) continue;
    for (const cue of Array.from(track.cues)) {
      if (cue.startTime <= at && at < cue.endTime) lines.push(...cueLines((cue as VTTCue).text));
    }
  }
  return lines;
};

/**
 * The subtitle clock: the video's time minus `delayMs`, kept in state only when it leaves the
 * stretch where the lines of `main` and `second` stay the same (so the player draws again only when a line
 * changes). Read every animation frame while the video plays, and at once on seeks, pauses,
 * speed changes and delay changes.
 */
export const useSubtitleClock = (
  videoRef: RefObject<HTMLVideoElement | null>,
  main: readonly Cue[],
  second: readonly Cue[],
  delayMs: number,
): number => {
  // Set from the video once its effect runs.
  const [clockMs, setClockMs] = useState(() => subtitleTimeMs(0, delayMs));
  const span = useRef({ from: 0, until: 0 });
  const usedDelay = useRef(delayMs);

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    let frame = 0;
    // New lines (a track chosen, edits) only move the stretch; a new delay moves the clock.
    const delayChanged = usedDelay.current !== delayMs;
    usedDelay.current = delayMs;
    const check = (force: boolean) => {
      const now = subtitleTimeMs(video.currentTime * 1000, delayMs);
      const { from, until } = span.current;
      if (force || now < from || now >= until) {
        span.current = steadySpan([main, second], now);
        setClockMs(now);
      }
    };
    const loop = () => {
      check(false);
      if (!video.paused) frame = requestAnimationFrame(loop);
    };
    const restart = () => {
      cancelAnimationFrame(frame);
      check(true);
      if (!video.paused) frame = requestAnimationFrame(loop);
    };
    const passing = () => check(false);
    const types = ['play', 'playing', 'pause', 'seeking', 'seeked', 'ratechange'];
    for (const type of types) video.addEventListener(type, restart);
    // A paused video moved by hand (frame steps) is read as it goes.
    video.addEventListener('timeupdate', passing);
    // The delay or the lines changed: what shows is read again now (the clock itself moves only
    // when the delay did, or when it left the lines' new stretch).
    span.current = steadySpan([main, second], clockMs);
    check(delayChanged);
    if (!video.paused) frame = requestAnimationFrame(loop);
    return () => {
      cancelAnimationFrame(frame);
      for (const type of types) video.removeEventListener(type, restart);
      video.removeEventListener('timeupdate', passing);
    };
  }, [videoRef, delayMs, main, second, clockMs]);

  return clockMs;
};
