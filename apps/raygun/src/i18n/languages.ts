/**
 * The UI languages: one JSON file per locale in `./locales` (see docs/TRANSLATING.md). English is
 * bundled; the others load when chosen. Missing keys fall back to English.
 */
export const FALLBACK_LANGUAGE = 'en';
/** The `language` setting value that follows the device. */
export const SYSTEM_LANGUAGE = 'system';

const loaders = import.meta.glob<{ default: Record<string, unknown> }>([
  './locales/*.json',
  '!./locales/en.json',
]);

const tagOf = (path: string) => path.replace(/^.*\/(.+)\.json$/, '$1');

/** Every available locale tag, English first. */
export const LANGUAGES: string[] = [
  FALLBACK_LANGUAGE,
  ...Object.keys(loaders)
    .map(tagOf)
    .sort((a, b) => a.localeCompare(b)),
];

/** Loads a locale's strings; `null` for English (bundled) or an unknown tag. */
export const loadLanguage = async (tag: string): Promise<Record<string, unknown> | null> => {
  const loader = loaders[`./locales/${tag}.json`];
  return loader ? (await loader()).default : null;
};

/** The best available locale for a list of preferred tags (exact, then by base language). */
export const matchLanguage = (preferred: readonly string[], available = LANGUAGES): string => {
  for (const wanted of preferred) {
    const exact = available.find((tag) => tag.toLowerCase() === wanted.toLowerCase());
    if (exact) return exact;
    const base = wanted.split('-')[0]!.toLowerCase();
    const sameBase = available.find((tag) => tag.split('-')[0]!.toLowerCase() === base);
    if (sameBase) return sameBase;
  }
  return FALLBACK_LANGUAGE;
};

/** The locale a setting value means here: a chosen tag, or the device's best match. */
export const resolveLanguage = (
  setting: string | undefined,
  device: readonly string[] = typeof navigator === 'undefined' ? [] : navigator.languages,
): string =>
  !setting || setting === SYSTEM_LANGUAGE ? matchLanguage(device) : matchLanguage([setting]);

const displayName = (tag: string, inLanguage: string): string | null => {
  try {
    const name = new Intl.DisplayNames([inLanguage], { type: 'language' }).of(tag);
    // Without data for a language, engines echo the tag back.
    return name && name.toLowerCase() !== tag.toLowerCase() ? name : null;
  } catch {
    return null;
  }
};

/**
 * A language's own name ("Deutsch", "日本語"). Android's WebView lacks names for some languages,
 * so the English name comes next, then the tag.
 */
export const nativeName = (tag: string): string => {
  const name = displayName(tag, tag) ?? displayName(tag, 'en');
  return name ? name.charAt(0).toLocaleUpperCase(tag) + name.slice(1) : tag;
};
