import type { TrackChoice } from './protocol';

/**
 * Which subtitle and audio tracks the webview's player turns on by itself (M46.4), by the same
 * rules as the Android player (M45): the title's remembered choice, else the languages from
 * Settings → Player, preferring tracks named with a prefer word and leaving out skip words.
 */

export type TrackOption = { key: string; label: string; language: string | null };

const THREE_LETTER: Record<string, string> = {
  eng: 'en',
  spa: 'es',
  fre: 'fr',
  fra: 'fr',
  ger: 'de',
  deu: 'de',
  ita: 'it',
  por: 'pt',
  jpn: 'ja',
  kor: 'ko',
  chi: 'zh',
  zho: 'zh',
  rus: 'ru',
  ara: 'ar',
  hin: 'hi',
  tur: 'tr',
  pol: 'pl',
  dut: 'nl',
  nld: 'nl',
  swe: 'sv',
  nor: 'no',
  nob: 'no',
  dan: 'da',
  fin: 'fi',
  ind: 'id',
  vie: 'vi',
  tha: 'th',
  heb: 'he',
  gre: 'el',
  ell: 'el',
  cze: 'cs',
  ces: 'cs',
  hun: 'hu',
  rum: 'ro',
  ron: 'ro',
  ukr: 'uk',
  may: 'ms',
  msa: 'ms',
  fil: 'tl',
  tgl: 'tl',
  per: 'fa',
  fas: 'fa',
  ben: 'bn',
  tam: 'ta',
  tel: 'te',
  bul: 'bg',
  hrv: 'hr',
  srp: 'sr',
  slo: 'sk',
  slk: 'sk',
  slv: 'sl',
  cat: 'ca',
  est: 'et',
  lav: 'lv',
  lit: 'lt',
};

const NAMES: Record<string, string> = {
  english: 'en',
  spanish: 'es',
  español: 'es',
  french: 'fr',
  français: 'fr',
  german: 'de',
  deutsch: 'de',
  italian: 'it',
  portuguese: 'pt',
  japanese: 'ja',
  korean: 'ko',
  chinese: 'zh',
  russian: 'ru',
  arabic: 'ar',
  hindi: 'hi',
  turkish: 'tr',
  polish: 'pl',
  dutch: 'nl',
  swedish: 'sv',
  norwegian: 'no',
  danish: 'da',
  finnish: 'fi',
  indonesian: 'id',
  vietnamese: 'vi',
  thai: 'th',
  hebrew: 'he',
  greek: 'el',
  czech: 'cs',
  hungarian: 'hu',
  romanian: 'ro',
  ukrainian: 'uk',
};

/** `en`, `eng`, `en-US`, `English` → `en`; unknown codes stay as they are, lowercased. */
export const languageCode = (value: string | null | undefined): string | null => {
  const text = value?.trim().toLowerCase();
  if (!text) return null;
  const base = text.split(/[-_ (]/)[0] ?? text;
  if (base.length === 2) return base;
  return THREE_LETTER[base] ?? NAMES[base] ?? base;
};

const words = (list: string) =>
  list
    .split(',')
    .map((word) => word.trim().toLowerCase())
    .filter(Boolean);

/** The remembered track among `options`: the same label, else the same language. */
export const matchChoice = <T extends TrackOption>(options: T[], choice: TrackChoice): T | null =>
  options.find((option) => choice.label && option.label === choice.label) ??
  options.find(
    (option) => choice.language && languageCode(option.language) === languageCode(choice.language),
  ) ??
  null;

/** The best track in the first of `languages` that has any. */
export const pickByLanguage = <T extends TrackOption>(
  options: T[],
  languages: (string | null | undefined)[],
  preferWords = '',
  skipWords = '',
): T | null => {
  const prefer = words(preferWords);
  const skip = words(skipWords);
  for (const language of languages) {
    const code = languageCode(language);
    if (!code) continue;
    const matching = options
      .filter((option) => languageCode(option.language) === code)
      .filter((option) => !skip.some((word) => option.label.toLowerCase().includes(word)));
    const preferred = matching.find((option) =>
      prefer.some((word) => option.label.toLowerCase().includes(word)),
    );
    if (preferred ?? matching[0]) return preferred ?? matching[0]!;
  }
  return null;
};

/** The track a remembered choice describes. */
export const choiceOf = (option: TrackOption): TrackChoice => ({
  language: option.language,
  label: option.label,
});
