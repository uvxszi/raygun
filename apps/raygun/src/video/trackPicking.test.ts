import {
  languageCode,
  languagePrefs,
  matchChoice,
  ORIGINAL_AUDIO,
  pickAudio,
  pickSubtitles,
} from './trackPicking';
import type { LanguagePrefs, TrackOption } from './trackPicking';

const option = (key: string, label: string, language: string | null, extra = {}): TrackOption => ({
  key,
  label,
  language,
  ...extra,
});

const english: LanguagePrefs = {
  audio: ['en'],
  subtitles: ['en'],
  subtitleMode: 'foreign',
  spoken: null,
};

describe('track languages', () => {
  it('reads codes, tags and names as one code', () => {
    expect(languageCode('en')).toBe('en');
    expect(languageCode('eng')).toBe('en');
    expect(languageCode('en-US')).toBe('en');
    expect(languageCode('pt_BR')).toBe('pt');
    expect(languageCode('zh-Hant')).toBe('zh');
    expect(languageCode('ger')).toBe('de');
    expect(languageCode('deu')).toBe('de');
    expect(languageCode('iw')).toBe('he');
    expect(languageCode('nb-NO')).toBe('no');
    // OpenSubtitles' own codes.
    expect(languageCode('pob')).toBe('pt');
    expect(languageCode('English')).toBe('en');
    expect(languageCode('Japanese - Original')).toBe('ja');
    expect(languageCode('English (US)')).toBe('en');
    expect(languageCode('Español (Latinoamérica)')).toBe('es');
    expect(languageCode('ENG SDH')).toBe('en');
    expect(languageCode('日本語')).toBe('ja');
    for (const nothing of ['und', 'Signs & Songs', 'Track 1', 'SDH', '', null]) {
      expect(languageCode(nothing)).toBeNull();
    }
  });

  it("puts the device's languages first, then the app's, unless a language is chosen", () => {
    const base = {
      audioLanguage: '',
      audioFallback: '',
      subtitleFallback: '',
      appLanguage: 'fr',
      subtitleLanguage: '',
      subtitleMode: 'foreign' as const,
      spoken: null,
      device: ['en-GB', 'de-DE'],
    };
    expect(languagePrefs(base)).toEqual({
      audio: ['en', 'de', 'fr'],
      subtitles: ['en', 'de', 'fr'],
      subtitleMode: 'foreign',
      spoken: null,
    });
    const chosen = languagePrefs({
      ...base,
      audioLanguage: 'ja',
      audioFallback: 'en',
      subtitleLanguage: 'es',
      subtitleFallback: 'en',
    });
    expect(chosen.audio).toEqual(['ja', 'en']);
    expect(chosen.subtitles).toEqual(['es', 'en']);
    expect(languagePrefs({ ...base, audioLanguage: ORIGINAL_AUDIO }).audio).toEqual([]);
  });
});

describe('choosing audio', () => {
  const tracks = [
    option('0', 'Japanese', 'jpn', { original: true }),
    option('1', 'English', 'eng'),
    option('2', 'English commentary', 'eng', { extra: true }),
  ];

  it('plays the first of your languages, not the track marked default', () => {
    expect(pickAudio(tracks, undefined, english, '0')?.key).toBe('1');
  });

  it('keeps the engine’s pick among tracks in the language', () => {
    const twice = [...tracks, option('3', 'English 5.1', 'en')];
    expect(pickAudio(twice, undefined, english, '3')?.key).toBe('3');
  });

  it('falls back to the track marked default, then the first', () => {
    const german = { ...english, audio: ['de'] };
    expect(pickAudio(tracks, undefined, german, '1')?.key).toBe('0');
    expect(pickAudio(tracks.slice(1), undefined, german)?.key).toBe('1');
    // The video's own: no languages at all.
    expect(pickAudio(tracks, undefined, { ...english, audio: [] })?.key).toBe('0');
  });

  it('keeps what was chosen for the title', () => {
    expect(pickAudio(tracks, { language: 'ja', label: 'Japanese' }, english)?.key).toBe('0');
    expect(pickAudio(tracks.slice(0, 1), undefined, english)).toBeNull();
  });
});

describe('choosing subtitles', () => {
  const subtitles = [
    option('signs', 'English Signs', 'en', { forced: true }),
    option('full', 'English', 'en'),
    option('es', 'Spanish', 'es'),
  ];

  it('turns on your language when the audio is in another one', () => {
    expect(pickSubtitles(subtitles, undefined, english, 'ja')?.key).toBe('full');
  });

  it('shows only forced subtitles when the audio is in your language', () => {
    expect(pickSubtitles(subtitles, undefined, english, 'en')?.key).toBe('signs');
    expect(pickSubtitles(subtitles.slice(1), undefined, english, 'en')).toBeNull();
    // Unknown audio is taken as yours.
    expect(pickSubtitles(subtitles, undefined, english, null)).toBeNull();
  });

  it('follows the setting: always, or never', () => {
    expect(
      pickSubtitles(subtitles, undefined, { ...english, subtitleMode: 'always' }, 'en')?.key,
    ).toBe('full');
    expect(
      pickSubtitles(subtitles, undefined, { ...english, subtitleMode: 'never' }, 'ja'),
    ).toBeNull();
  });

  it('keeps the title’s own choice, off included', () => {
    expect(pickSubtitles(subtitles, { off: true }, english, 'ja')).toBeNull();
    expect(pickSubtitles(subtitles, { language: 'es', label: 'Spanish' }, english, 'en')?.key).toBe(
      'es',
    );
  });

  it('prefers and skips tracks by their names', () => {
    const named = [option('a', 'English Songs', 'en'), option('b', 'English Dialogue', 'en')];
    expect(pickSubtitles(named, undefined, english, 'ja', 'dialogue', 'songs')?.key).toBe('b');
  });

  it('finds a remembered track by name, then by language', () => {
    const choice = { language: 'English', label: 'Other name' };
    expect(matchChoice(subtitles, choice)?.key).toBe('full');
  });
});
