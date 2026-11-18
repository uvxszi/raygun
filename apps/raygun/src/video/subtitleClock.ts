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

/** The player's events after which the clock reads the playhead at once. */
const CLOCK_EVENTS = [
  'play',
  'playing',
  'pause',
  'waiting',
  'seeking',
  'seeked',
  'ratechange',
  'loadedmetadata',
] as const;

/**
 * The subtitle clock: the video's time minus `delayMs`, kept in state only when it leaves the
 * stretch where the lines of `main` and `second` stay the same (so the player draws again only
 * when a line changes). Read every animation frame while the video plays (also while it waits
 * for data: its time then stands still, and so do the lines), on every `timeupdate` (frames don't
 * come in a hidden window), and at once on seeks, pauses, stalls, speed changes and delay
 * changes. Nothing is counted between reads: each one is the playhead itself, so no stall, seek
 * or speed change can leave the lines behind.
 *
 * The frame loop lives as long as the video element; the lines and the delay reach it through
 * refs, so new lists (a track chosen, edits, lines joined) never restart it.
 */
export const useSubtitleClock = (
  videoRef: RefObject<HTMLVideoElement | null>,
  main: readonly Cue[],
  second: readonly Cue[],
  delayMs: number,
): number => {
  // Set from the video once its effect runs.
  const [clockMs, setClockMs] = useState(() => subtitleTimeMs(0, delayMs));
  const clock = useRef(clockMs);
  const span = useRef({ from: 0, until: 0 });
  const lists = useRef<readonly (readonly Cue[])[]>([main, second]);
  const delay = useRef(delayMs);
  /** Reads the playhead now; `force` sets the clock even inside the stretch. */
  const read = useRef<(force: boolean) => void>(() => {});

  useEffect(() => {
    const video = videoRef.current;
    if (!video) return;
    let frame = 0;
    const check = (force: boolean) => {
      const now = subtitleTimeMs(video.currentTime * 1000, delay.current);
      const { from, until } = span.current;
      if (force || now < from || now >= until) {
        span.current = steadySpan(lists.current, now);
        clock.current = now;
        setClockMs(now);
      }
    };
    const loop = () => {
      check(false);
      frame = video.paused ? 0 : requestAnimationFrame(loop);
    };
    const restart = () => {
      cancelAnimationFrame(frame);
      check(true);
      frame = video.paused ? 0 : requestAnimationFrame(loop);
    };
    read.current = check;
    const passing = () => check(false);
    for (const type of CLOCK_EVENTS) video.addEventListener(type, restart);
    // A paused video moved by hand (frame steps), and a window whose frames stopped.
    video.addEventListener('timeupdate', passing);
    restart();
    return () => {
      cancelAnimationFrame(frame);
      read.current = () => {};
      for (const type of CLOCK_EVENTS) video.removeEventListener(type, restart);
      video.removeEventListener('timeupdate', passing);
    };
  }, [videoRef]);

  // New lines (a track chosen, edits) only move the stretch around the clock; a new delay moves
  // the clock itself at once.
  useEffect(() => {
    const delayChanged = delay.current !== delayMs;
    delay.current = delayMs;
    lists.current = [main, second];
    span.current = steadySpan(lists.current, clock.current);
    read.current(delayChanged);
  }, [delayMs, main, second]);

  return clockMs;
};
