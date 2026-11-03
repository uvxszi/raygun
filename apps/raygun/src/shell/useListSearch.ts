import { useMemo, useState } from 'react';

/** Lower case without accents, so "Pokémon" matches "pokemon". */
export const searchable = (text: string) =>
  text
    .normalize('NFD')
    .replace(/\p{Diacritic}/gu, '')
    .toLowerCase();

/** Whether every word of `query` appears in one of `fields` (M46.35: searchable lists). */
export const matchesSearch = (query: string, fields: (string | null | undefined)[]) => {
  const words = searchable(query).split(/\s+/).filter(Boolean);
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
