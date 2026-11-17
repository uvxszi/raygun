import { describe, expect, it } from 'vitest';

import { smartOrder } from './smartOrder';
import { steadySpan, subtitleTimeMs } from './subtitleClock';
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
