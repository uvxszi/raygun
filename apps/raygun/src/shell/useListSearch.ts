import { useMemo, useState } from 'react';

/** Texts made searchable lately: long lists ask again for the same names on every keystroke. */
const SEEN = new Map<string, string>();
const SEEN_MAX = 20_000;

/** Lower case without accents, so "Pokémon" matches "pokemon". */
export const searchable = (text: string) => {
  let found = SEEN.get(text);
  if (found === undefined) {
    found = text
      .normalize('NFD')
      .replace(/\p{Diacritic}/gu, '')
      .toLowerCase();
    if (SEEN.size >= SEEN_MAX) SEEN.clear();
    SEEN.set(text, found);
  }
  return found;
};

/** The last query's words: a list asks with the same query for each of its rows. */
let lastQuery: { query: string; words: string[] } = { query: '', words: [] };

const wordsOf = (query: string) => {
  if (lastQuery.query !== query) {
    lastQuery = { query, words: searchable(query).split(/\s+/).filter(Boolean) };
  }
  return lastQuery.words;
};

/** Whether every word of `query` appears in one of `fields` (M46.35: searchable lists). */
export const matchesSearch = (query: string, fields: (string | null | undefined)[]) => {
  const words = wordsOf(query);
  if (words.length === 0) return true;
  const haystack = searchable(fields.filter(Boolean).join(' '));
  return words.every((word) => haystack.includes(word));
};

/**
 * A list narrowed by what's typed (M46.35: every list can be searched): `fields` gives each
 * item's searchable text, such as its name and subtitle.
 */
export const useListSearch = <T>(
  items: T[] | undefined,
  fields: (item: T) => (string | null | undefined)[],
) => {
  const [query, setQuery] = useState('');
  const filtered = useMemo(
    () => (items ?? []).filter((item) => matchesSearch(query, fields(item))),
    // `fields` is a new function each render; the query and items decide the result.
    // eslint-disable-next-line react-hooks/exhaustive-deps
    [items, query],
  );
  return { query, setQuery, filtered };
};
