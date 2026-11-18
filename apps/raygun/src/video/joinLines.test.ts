import { joinCloseCues } from './joinLines';
import { cuesAt } from './subtitles';
import type { Cue } from './subtitles';

const cue = (startMs: number, endMs: number, ...lines: string[]): Cue => ({
  startMs,
  endMs,
  lines,
});

describe('joining lines close together', () => {
  it('leaves ordinary dialogue as it is', () => {
    const cues = [cue(0, 2000, 'Hello.'), cue(2000, 4500, 'Hi there.'), cue(5000, 7000, 'Bye.')];
    expect(joinCloseCues(cues)).toEqual(cues);
  });

  it('joins cues that overlap, each line once', () => {
    const joined = joinCloseCues([
      cue(0, 2000, '- Where?'),
      cue(1800, 3500, '- Where?', '- There.'),
      cue(6000, 8000, 'Later.'),
    ]);
    expect(joined).toEqual([cue(0, 3500, '- Where?', '- There.'), cue(6000, 8000, 'Later.')]);
    // Nothing changes in between: no flicker at 1.8 s or 2 s.
    expect(cuesAt(joined, 1900)).toEqual(['- Where?', '- There.']);
    expect(cuesAt(joined, 2500)).toEqual(['- Where?', '- There.']);
  });

  it('joins quick changes with short gaps', () => {
    const joined = joinCloseCues([
      cue(10_000, 10_600, 'One'),
      cue(10_700, 11_300, 'two'),
      cue(11_400, 12_000, 'three!'),
      // Half a second later: a line of its own.
      cue(12_500, 13_100, 'Next.'),
    ]);
    expect(joined).toEqual([
      cue(10_000, 12_000, 'One', 'two', 'three!'),
      cue(12_500, 13_100, 'Next.'),
    ]);
  });

  it('keeps joined cues short and small', () => {
    // Quick lines on and on: each joined cue stays within six seconds and four lines.
    const quick = Array.from({ length: 12 }, (_, index) =>
      cue(index * 700, index * 700 + 600, `line ${index}`),
    );
    const joined = joinCloseCues(quick);
    expect(joined.length).toBeGreaterThan(2);
    for (const one of joined) {
      expect(one.endMs - one.startMs).toBeLessThanOrEqual(6000);
      expect(one.lines.length).toBeLessThanOrEqual(4);
    }
    // Every line is still there, in order.
    expect(joined.flatMap((one) => one.lines)).toEqual(quick.flatMap((one) => one.lines));
    // A long sign over the dialogue isn't stretched over it.
    const sign = joinCloseCues([cue(0, 20_000, 'CITY HALL'), cue(1000, 3000, 'Come in.')]);
    expect(sign).toEqual([cue(0, 20_000, 'CITY HALL'), cue(1000, 3000, 'Come in.')]);
  });

  it('sorts what comes out of order', () => {
    expect(joinCloseCues([cue(5000, 6000, 'b'), cue(0, 1000, 'a')])).toEqual([
      cue(0, 1000, 'a'),
      cue(5000, 6000, 'b'),
    ]);
  });
});
