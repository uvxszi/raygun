import type {
  Lyrics,
  LyricsTranslation,
  LyricsTranslationPrefs,
  TrackQuery,
} from '@raygun/bindings';
import { useMemo } from 'react';
import { useTranslation } from 'react-i18next';
import { create } from 'zustand';

import { FALLBACK_LANGUAGE } from '../i18n/languages';
import { trackKey } from '../music/trackKey';
import { useSettings } from '../settings/useSettings';

/** The lyrics translation settings with every field. */
export type TranslationPrefs = Required<LyricsTranslationPrefs>;

/** Rust's `LyricsTranslationPrefs` defaults, until settings load. */
export const TRANSLATION_DEFAULTS: TranslationPrefs = {
  language: '',
  automatic: true,
  romanized: false,
  localModel: '',
};

/** Another source's translation shows by itself only when it lines up with this many lines. */
const AUTO_COVERAGE = 0.8;

/** The lines translations line up with, as Rust's `Lyrics::line_texts` splits them. */
export const lineTexts = (lyrics: Lyrics): string[] => {
  if (lyrics.synced.length > 0) return lyrics.synced.map((line) => line.text);
  const plain = lyrics.plain ?? '';
  return plain ? plain.replace(/\r?\n$/, '').split(/\r?\n/) : [];
};

/** A song's translations; lyrics kept by an older version have none. */
export const translationsOf = (lyrics: Lyrics | null | undefined): LyricsTranslation[] =>
  lyrics?.translations ?? [];

const baseAndScript = (tag: string): [string, string | null] => {
  const [base = '', ...rest] = tag.split(/[-_]/);
  let script: string | null = null;
  let region: string | null = null;
  for (const part of rest) {
    if (/^[a-z]{4}$/i.test(part)) script = part[0]!.toUpperCase() + part.slice(1).toLowerCase();
    else region ??= part.toUpperCase();
  }
  const lower = base.toLowerCase();
  if (lower === 'zh' && !script)
    script = ['TW', 'HK', 'MO'].includes(region ?? '') ? 'Hant' : 'Hans';
  return [lower, script];
};

/**
 * Whether text written in `written` reads as `wanted`: the same base language, and for Chinese
 * the same characters (as Rust's `marian::same_language`).
 */
export const sameLanguage = (written: string, wanted: string) => {
  const [writtenBase, writtenScript] = baseAndScript(written);
  const [wantedBase, wantedScript] = baseAndScript(wanted);
  return writtenBase === wantedBase && (writtenBase !== 'zh' || writtenScript === wantedScript);
};

const hasWords = (line: string) => /[\p{L}\p{N}]/u.test(line);

/** The share of the lyrics' lines with words that a translation has a line for. */
export const coverage = (lyrics: Lyrics, translation: LyricsTranslation) => {
  const lines = lineTexts(lyrics);
  const worded = lines.filter(hasWords).length;
  if (worded === 0) return 0;
  const covered = lines.filter(
    (line, index) => hasWords(line) && Boolean(translation.lines[index]),
  ).length;
  return covered / worded;
};

/** A song's translation picked in the Translate menu: who made it, in which language. */
export type TranslationChoice = { translator: string; language: string };

/** What shows under the lines. */
export type ShownTranslation = {
  translation: LyricsTranslation | null;
  romanization: LyricsTranslation | null;
};

export const NOTHING_SHOWN: ShownTranslation = { translation: null, romanization: null };

/**
 * The translation under the lines: the one picked for the song, else (when translations show
 * by themselves) the first in the language: the newest made or picked, the source's own, or
 * another source's when it lines up with most lines. Romanized lines show when they're switched on.
 */
export const pickShown = (
  lyrics: Lyrics | null | undefined,
  options: { language: string; prefs: TranslationPrefs; choice: TranslationChoice | null },
): ShownTranslation => {
  if (!lyrics) return NOTHING_SHOWN;
  const all = translationsOf(lyrics);
  const translations = all.filter((translation) => translation.kind === 'translation');
  const { choice, prefs, language } = options;
  const chosen = choice
    ? translations.find(
        (translation) =>
          translation.translator === choice.translator && translation.language === choice.language,
      )
    : undefined;
  const automatic = prefs.automatic
    ? translations.find(
        (translation) =>
          sameLanguage(translation.language, language) &&
          (translation.origin !== 'otherSource' || coverage(lyrics, translation) >= AUTO_COVERAGE),
      )
    : undefined;
  const romanization = prefs.romanized
    ? all.find((translation) => translation.kind === 'romanization')
    : undefined;
  return { translation: chosen ?? automatic ?? null, romanization: romanization ?? null };
};

type ChoiceState = {
  /** The translation picked for each song this session, by track key. */
  choices: Record<string, TranslationChoice>;
  choose: (key: string, choice: TranslationChoice | null) => void;
};

/** Translations picked in the Translate menu, for this session. */
export const useTranslationChoices = create<ChoiceState>((set) => ({
  choices: {},
  choose: (key, choice) =>
    set((state) => {
      const choices = { ...state.choices };
      if (choice) choices[key] = choice;
      else delete choices[key];
      return { choices };
    }),
}));

/** The key a song's translation choice is kept under. */
export const choiceKey = (query: TrackQuery) => trackKey(query.title, query.artist);

/** The lyrics translation settings, and the language translations show in. */
export const useTranslationPrefs = () => {
  const settings = useSettings();
  const { i18n } = useTranslation();
  const saved = settings.data?.lyricsTranslation;
  const prefs = useMemo<TranslationPrefs>(() => ({ ...TRANSLATION_DEFAULTS, ...saved }), [saved]);
  // An empty setting follows the app's language.
  const language = prefs.language || i18n.language || FALLBACK_LANGUAGE;
  return { prefs, language, loaded: settings.data !== undefined };
};

/** What shows under a song's lyrics lines. */
export const useShownTranslation = (
  query: TrackQuery,
  lyrics: Lyrics | null | undefined,
): ShownTranslation => {
  const { prefs, language } = useTranslationPrefs();
  const key = choiceKey(query);
  const choice = useTranslationChoices((state) => state.choices[key] ?? null);
  return useMemo(
    () => pickShown(lyrics, { language, prefs, choice }),
    [lyrics, language, prefs, choice],
  );
};
