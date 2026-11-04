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

/** How well `title` answers `query`: exact, then starts with it, then shares its words. */
export const titleScore = (title: string, query: string) => {
  const wanted = simple(query);
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
): RankedCard[] => {
  const scored = results.flatMap((result, sourceIndex) =>
    result.items.map((card, position) => ({
      card,
      sourceName: result.source.name,
      score: query.trim() ? score(card.title, query) : 0,
      position,
      sourceIndex,
    })),
  );
  scored.sort(
    (a, b) => b.score - a.score || a.position - b.position || a.sourceIndex - b.sourceIndex,
  );
  return scored.map(({ card, sourceName }) => ({ card, sourceName }));
};
