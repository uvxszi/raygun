import { act, renderHook } from '@testing-library/react';
import { afterEach, describe, expect, it, vi } from 'vitest';

import { smartOrder } from './smartOrder';
import { steadySpan, subtitleTimeMs, useSubtitleClock } from './subtitleClock';
import { cuesAt } from './subtitles';
import type { Cue } from './subtitles';

const cues: Cue[] = [
  { startMs: 1_000, endMs: 3_000, lines: ['one'] },
  { startMs: 5_000, endMs: 6_000, lines: ['two'] },
  { startMs: 5_500, endMs: 8_000, lines: ['three'] },
];

describe('subtitle delay', () => {
  it('moves the lines either way, also past the ends of the video', () => {
    // Two seconds later: the line at 1 s shows at 3 s.
    expect(cuesAt(cues, subtitleTimeMs(3_000, 2_000))).toEqual(['one']);
    expect(cuesAt(cues, subtitleTimeMs(1_500, 2_000))).toEqual([]);
    // Half a second earlier: the line at 5 s shows at 4.5 s.
    expect(cuesAt(cues, subtitleTimeMs(4_500, -500))).toEqual(['two']);
    // Longer than the video either way: nothing shows, nothing breaks.
    expect(cuesAt(cues, subtitleTimeMs(4_000, 600_000))).toEqual([]);
    expect(cuesAt(cues, subtitleTimeMs(4_000, -600_000))).toEqual([]);
  });

  it('knows how long the lines on screen stay the same', () => {
    expect(steadySpan([cues], 0)).toEqual({ from: Number.NEGATIVE_INFINITY, until: 1_000 });
    expect(steadySpan([cues], 1_000)).toEqual({ from: 1_000, until: 3_000 });
    expect(steadySpan([cues], 5_700)).toEqual({ from: 5_500, until: 6_000 });
    expect(steadySpan([cues], 9_000)).toEqual({ from: 8_000, until: Number.POSITIVE_INFINITY });
    // The second subtitles' lines count too.
    expect(steadySpan([cues, [{ startMs: 2_000, endMs: 2_500, lines: ['b'] }]], 1_500)).toEqual({
      from: 1_000,
      until: 2_000,
    });
  });
});

describe('the subtitle clock', () => {
  /** A paused video whose playhead and speed the test moves. */
  const fakeVideo = () => {
    const video = document.createElement('video');
    let time = 0;
    Object.defineProperty(video, 'currentTime', {
      get: () => time,
      set: (next: number) => (time = next),
    });
    Object.defineProperty(video, 'paused', { get: () => true });
    const move = (seconds: number, type: string) =>
      act(() => {
        time = seconds;
        video.dispatchEvent(new Event(type));
      });
    return { video, move };
  };

  it('reads the playhead less the delay after seeks, speed changes and delay changes', () => {
    const { video, move } = fakeVideo();
    const ref = { current: video };
    const { result, rerender } = renderHook(
      ({ delay }: { delay: number }) => useSubtitleClock(ref, cues, [], delay),
      { initialProps: { delay: 10_000 } },
    );
    // Ten seconds later on a video shorter than that: before every line.
    move(4, 'seeked');
    expect(result.current).toBe(-6_000);
    expect(cuesAt(cues, result.current)).toEqual([]);
    // Played on at 2× (the delay is in the video's time): the line at 1 s shows at 11 s.
    video.playbackRate = 2;
    move(11, 'ratechange');
    expect(cuesAt(cues, result.current)).toEqual(['one']);
    // Half speed: same.
    video.playbackRate = 0.5;
    move(12.5, 'ratechange');
    expect(cuesAt(cues, result.current)).toEqual(['one']);
    // Five seconds earlier, from the very start: the line at 5 s shows at 0.
    rerender({ delay: -5_000 });
    move(0, 'seeked');
    expect(result.current).toBe(5_000);
    expect(cuesAt(cues, result.current)).toEqual(['two']);
  });
});

describe('the subtitle clock over a long play', () => {
  /** A video that plays: its time moves each frame at its speed unless paused or waiting. */
  const playingVideo = () => {
    const video = document.createElement('video');
    let time = 0;
    let paused = false;
    Object.defineProperty(video, 'currentTime', {
      get: () => time,
      set: (next: number) => (time = next),
    });
    Object.defineProperty(video, 'paused', { get: () => paused });
    const frames = new Map<number, FrameRequestCallback>();
    let next = 1;
    vi.stubGlobal('requestAnimationFrame', (callback: FrameRequestCallback) => {
      frames.set(next, callback);
      return next++;
    });
    vi.stubGlobal('cancelAnimationFrame', (id: number) => frames.delete(id));
    const state = { stalled: false };
    /** One frame of `ms`: time moves on, then the frame's callbacks run. */
    const frame = (ms: number) =>
      act(() => {
        if (!paused && !state.stalled) time += (ms / 1000) * video.playbackRate;
        const due = [...frames.entries()];
        frames.clear();
        for (const [, callback] of due) callback(0);
      });
    const fire = (type: string, change: () => void) =>
      act(() => {
        change();
        video.dispatchEvent(new Event(type));
      });
    return {
      video,
      frame,
      stall: (on: boolean) => fire(on ? 'waiting' : 'playing', () => (state.stalled = on)),
      seek: (seconds: number) => fire('seeked', () => (time = seconds)),
      speed: (rate: number) => fire('ratechange', () => (video.playbackRate = rate)),
      pause: (on: boolean) => fire(on ? 'pause' : 'play', () => (paused = on)),
      now: () => time * 1000,
    };
  };

  afterEach(() => vi.unstubAllGlobals());

  it('shows every line on time through stalls, seeks, speed and delay changes', () => {
    // Two hundred lines of varied lengths and gaps, some overlapping.
    const many: Cue[] = Array.from({ length: 200 }, (_, index) => {
      const startMs = 1_000 + index * 1_517 + (index % 7) * 113;
      return { startMs, endMs: startMs + 400 + (index % 5) * 531, lines: [`line ${index}`] };
    });
    const player = playingVideo();
    const ref = { current: player.video };
    const { result, rerender } = renderHook(
      ({ delay, list }: { delay: number; list: Cue[] }) => useSubtitleClock(ref, list, [], delay),
      { initialProps: { delay: 0, list: many } },
    );
    let delay = 0;
    const steps: Record<number, () => void> = {
      400: () => player.stall(true),
      460: () => player.stall(false),
      900: () => player.seek(150),
      1200: () => player.speed(1.5),
      1500: () => {
        delay = 1_500;
        rerender({ delay, list: many });
      },
      1800: () => player.speed(0.75),
      2000: () => player.pause(true),
      2030: () => player.pause(false),
      2200: () => player.seek(40),
      // A new list each render (lines edited, joined): the clock keeps reading.
      2400: () => rerender({ delay, list: [...many] }),
      2600: () => {
        delay = -800;
        rerender({ delay, list: many });
        player.speed(2);
      },
      2800: () => player.stall(true),
      2810: () => player.stall(false),
    };
    for (let step = 0; step < 3_000; step += 1) {
      player.frame(50);
      steps[step]?.();
      // What shows is what the playhead less the delay shows, at every frame.
      expect(cuesAt(many, result.current)).toEqual(
        cuesAt(many, subtitleTimeMs(player.now(), delay)),
      );
    }
  });
});

describe('Smart Subtitles order', () => {
  it('puts each language’s most alike first and keeps languages in place', () => {
    const listed = [
      { key: 'en-a', language: 'en', rank: 2 },
      { key: 'en-b', language: 'en', rank: 0 },
      { key: 'de', language: 'de' },
      { key: 'en-c', language: 'en' },
      { key: 'en-d', language: 'en', rank: 1 },
    ];
    expect(smartOrder(listed).map((option) => option.key)).toEqual([
      'en-b',
      'en-d',
      'en-a',
      'en-c',
      'de',
    ]);
    const plain = listed.map(({ key, language }) => ({ key, language }));
    expect(smartOrder(plain)).toBe(plain);
  });
});
