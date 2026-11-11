import {
  byLanguages,
  languageCode,
  languagePrefs,
  listedLanguages,
  matchChoice,
  ORIGINAL_AUDIO,
  pickAudio,
  pickSubtitles,
  pickVariant,
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

describe('pickVariant', () => {
  const ladder = [
    { width: 640, height: 360, bitrate: 800_000 },
    { width: 1280, height: 720, bitrate: 2_500_000 },
    { width: 1920, height: 800, bitrate: 5_000_000 },
    { width: 3840, height: 2160, bitrate: 15_000_000 },
  ];

  it('starts at the preferred resolution and caps automatic switching there', () => {
    expect(pickVariant(ladder, '1080p')).toEqual({ start: 2, cap: 2, maxKbps: 5000 });
    expect(pickVariant(ladder, '720p')).toEqual({ start: 1, cap: 1, maxKbps: 2500 });
    expect(pickVariant(ladder, '1440p')).toEqual({ start: 2, cap: 2, maxKbps: 5000 });
  });

  it('leaves the highest uncapped', () => {
    expect(pickVariant(ladder, 'highest')).toEqual({ start: 3, cap: -1, maxKbps: null });
  });

  it('takes the lowest above when none is small enough', () => {
    expect(pickVariant(ladder.slice(1), '360p')).toEqual({ start: 0, cap: 0, maxKbps: 2500 });
  });

  it('reads sizes from bit rates when variants have none', () => {
    const unsized = [{ bitrate: 6_000_000 }, { bitrate: 2_400_000 }, { bitrate: 14_000_000 }];
    expect(pickVariant(unsized, '1080p')?.start).toBe(0);
    expect(pickVariant(ladder.slice(0, 1), '1080p')).toBeNull();
  });
});

describe('subtitle lists', () => {
  const input = {
    audioLanguage: '',
    audioFallback: '',
    subtitleFallback: '',
    appLanguage: 'de',
    subtitleLanguage: '',
    subtitleMode: 'foreign' as const,
    spoken: null,
    device: ['en-GB'],
  };

  it('list the device’s languages, then the app’s, then the rest', () => {
    expect(listedLanguages(input)).toEqual(['en', 'de']);
    // A subtitle language set in Settings comes first.
    expect(listedLanguages({ ...input, subtitleLanguage: 'fr' })).toEqual(['fr', 'en', 'de']);
  });

  it('keep every track of a language together, in the video’s order', () => {
    const options = [
      option('es', 'Español', 'spa'),
      option('en', 'English', 'en'),
      option('fr', 'Français', 'fre'),
      option('sdh', 'English SDH', 'eng'),
      option('ai', 'AI subtitles', null),
      option('de', 'Deutsch (translated)', 'de'),
    ];
    expect(byLanguages(options, listedLanguages(input)).map((track) => track.key)).toEqual([
      'en',
      'sdh',
      'de',
      'es',
      'fr',
      'ai',
    ]);
    expect(byLanguages(options, [])).toBe(options);
  });
});
