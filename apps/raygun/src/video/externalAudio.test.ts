import {
  FINE_DRIFT_S,
  JUMP_DRIFT_S,
  catchUp,
  rememberOf,
  rememberedOffer,
  soundTime,
} from './externalAudio';
import type { AudioOffer } from './protocol';

const offer = (id: string, source: string, languages: string[] = []): AudioOffer => ({
  id,
  group: 0,
  link: 0,
  source,
  languages,
  several: false,
  title: '1080p',
  sound: null,
  url: `http://127.0.0.1:1/media/t/${id}`,
  format: null,
});

describe('the sound beside the picture', () => {
  it('follows the picture by the delay: later when positive, earlier when negative', () => {
    expect(soundTime(10, 1_500)).toBe(8.5);
    expect(soundTime(10, -2_000)).toBe(12);
    expect(soundTime(1, 2_000)).toBe(-1);
  });

  it('jumps when far off, changes pace when close, and keeps the picture’s pace in step', () => {
    expect(catchUp(10 + JUMP_DRIFT_S + 0.1, 10, 1)).toEqual({ jumpTo: 10 });
    expect(catchUp(3, 5, 1)).toEqual({ jumpTo: 5 });
    expect(catchUp(0, -1, 1)).toEqual({ jumpTo: 0 });
    // A little ahead: a little slower; behind: a little faster, at the picture's speed.
    const slower = catchUp(10.1, 10, 1.5);
    expect('rate' in slower && slower.rate).toBeLessThan(1.5);
    const faster = catchUp(9.9, 10, 1.5);
    expect('rate' in faster && faster.rate).toBeGreaterThan(1.5);
    expect(catchUp(10 + FINE_DRIFT_S / 2, 10, 1.25)).toEqual({ rate: 1.25 });
  });

  it('brings back the sound a title remembers from the same source in the same language', () => {
    const offers = [
      offer('0:1', 'Torrentio', ['en']),
      offer('1:0', 'Comet', ['it']),
      offer('1:2', 'Comet', ['es']),
      offer('2:0', 'MediaFusion'),
    ];
    expect(rememberedOffer(offers, { source: 'Comet', language: 'es' })?.id).toBe('1:2');
    expect(rememberedOffer(offers, { source: 'Comet', language: 'ita' })?.id).toBe('1:0');
    expect(rememberedOffer(offers, { source: 'MediaFusion', language: null })?.id).toBe('2:0');
    expect(rememberedOffer(offers, { source: 'Comet', language: 'de' })).toBeNull();
    expect(rememberedOffer(offers, null)).toBeNull();
    expect(rememberOf(offers[1]!)).toEqual({ source: 'Comet', language: 'it' });
  });
});
