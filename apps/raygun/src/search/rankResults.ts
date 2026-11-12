import type { MangaCard, SourceSearchResult } from '@raygun/bindings';

/** A result with the source it came from, for the ranked view (M46.9). */
export type RankedCard = { card: MangaCard; sourceName: string };

const simple = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/** The query last scored against, made simple once for every title it's compared with. */
let lastQuery = '';
let lastWanted = '';

const simpleQuery = (query: string) => {
  if (query !== lastQuery) {
    lastQuery = query;
    lastWanted = simple(query);
  }
  return lastWanted;
};

/** How well `title` answers `query`: exact, then starts with it, then shares its words. */
export const titleScore = (title: string, query: string) => {
  const wanted = simpleQuery(query);
  const found = simple(title);
  if (!wanted || !found) return 0;
  if (found === wanted) return 100;
  if (found.startsWith(`${wanted} `) || found.startsWith(wanted)) return 80;
  const words = wanted.split(' ');
  const have = new Set(found.split(' '));
  const shared = words.filter((word) => have.has(word)).length;
  if (shared === words.length) return found.includes(wanted) ? 70 : 60;
  return Math.round((40 * shared) / words.length);
};

/** An entry of one of several lists ranked as one: the entry and which list it came from. */
export type RankedEntry<Item> = { item: Item; list: number };

/**
 * Several lists' entries in one order, the best answers to `query` first. Among equals an entry's
 * place in its own list wins, then the lists' order; with nothing typed the lists take turns, so
 * no one list fills the top. `titleOf` names an entry, `score` rates that name against the query.
 */
export const rankLists = <Item>(
  lists: readonly (readonly Item[])[],
  query: string,
  titleOf: (item: Item) => string,
  score: (title: string, query: string) => number = titleScore,
): RankedEntry<Item>[] => {
  const asked = query.trim() !== '';
  const scored = lists.flatMap((items, list) =>
    items.map((item, position) => ({
      item,
      list,
      score: asked ? score(titleOf(item), query) : 0,
      position,
    })),
  );
  scored.sort((a, b) => b.score - a.score || a.position - b.position || a.list - b.list);
  return scored.map(({ item, list }) => ({ item, list }));
};

/**
 * Every source's results in one list, the best answers to `query` first, each title once per
 * source. Among equals a source's own order wins, then the sources' order. With nothing typed
 * (each source's popular titles) the sources take turns, so no one source fills the top.
 * `score` rates a title against the query (videos weigh seasons: `videoTitleScore`).
 */
export const rankResults = (
  results: SourceSearchResult[],
  query: string,
  score: (title: string, query: string) => number = titleScore,
): RankedCard[] =>
  rankLists(
    results.map((result) => result.items),
    query,
    (card) => card.title,
    score,
  ).map(({ item, list }) => ({ card: item, sourceName: results[list]?.source.name ?? '' }));
