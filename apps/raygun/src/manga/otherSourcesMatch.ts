/**
 * How Find in other sources weighs a match beyond its title: whether its byline agrees with the
 * author asked for, and whether a source (or a directory's book) is in the language picked.
 */

/** A small lead for a match whose author agrees… */
const AGREES = 5;
/** …and out of the close matches (even the very same title) for one whose author doesn't. */
const DISAGREES = -50;

/** A tag's base language, lower case ("pt-BR" is "pt"). */
export const baseOf = (tag: string) => (tag.trim().split(/[-_]/)[0] ?? '').toLowerCase();

/** A name's words of three letters or more, lower case. */
const nameWords = (name: string) =>
  name
    .toLowerCase()
    .split(/[^\p{L}\p{N}]+/u)
    .filter((word) => [...word].length >= 3);

/**
 * Whether `wanted` and one of `names` name the same writer as far as their words tell: they share
 * a word of three letters or more ("Shelley"), however each is written. Not knowing agrees.
 */
export const authorAgrees = (wanted: string, names: readonly string[] | null | undefined) => {
  const want = nameWords(wanted);
  if (want.length === 0 || !names?.length) return true;
  return names.some((name) => nameWords(name).some((word) => want.includes(word)));
};

/** How a match's byline moves it: up a little when it agrees, out when it doesn't. */
export const authorWeight = (wanted: string, names: readonly string[] | null | undefined) => {
  if (nameWords(wanted).length === 0 || !names?.length) return 0;
  return authorAgrees(wanted, names) ? AGREES : DISAGREES;
};

/** Catalogue languages a source lists for many languages at once. */
const MANY = new Set(['all', 'multi', 'mul']);

/**
 * Whether a source with catalogue language `lang` (`en`, `ja`, `all` for many) is searched for
 * `picked`: any source for no language picked; then its own language, or many. A source whose
 * language isn't known is left out.
 */
export const sourceFits = (lang: string | null | undefined, picked: string) => {
  if (!picked) return true;
  if (!lang) return false;
  const value = lang.toLowerCase();
  return MANY.has(value) || baseOf(value) === baseOf(picked);
};

/** A language's English name ("French"), when the runtime knows it. */
const englishName = (tag: string) => {
  try {
    return new Intl.DisplayNames(['en'], { type: 'language' }).of(baseOf(tag)) ?? null;
  } catch {
    return null;
  }
};

/**
 * Whether a directory's language field ("English", "en", "fr-CA") is `picked`. Nothing picked,
 * or a field that says nothing, fits.
 */
export const languageFits = (field: string | null | undefined, picked: string) => {
  if (!picked || !field?.trim()) return true;
  const value = field.trim().toLowerCase();
  if (baseOf(value) === baseOf(picked)) return true;
  const name = englishName(picked)?.toLowerCase();
  return Boolean(name) && (value === name || value.startsWith(`${name} `));
};

/** The languages sources list titles in, without "many" and unknown ones, each once. */
export const languagesOf = (langs: Iterable<string | null>) => {
  const seen = new Map<string, string>();
  for (const lang of langs) {
    if (!lang || MANY.has(lang.toLowerCase())) continue;
    const key = lang.toLowerCase();
    if (!seen.has(key)) seen.set(key, lang);
  }
  return [...seen.values()];
};

/** A feed's host ("feeds.example.com"), to tell feeds of the same show apart. */
export const hostOf = (url: string) => {
  try {
    return new URL(url).host.replace(/^www\./, '');
  } catch {
    return '';
  }
};
