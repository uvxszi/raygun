import type { OnlineTrack, ProviderSearchResult } from '@raygun/bindings';

import { titleScore } from '../search/rankResults';

/** Answers each source shows at most, so one source with many can't hide the others. */
const PER_SOURCE = 5;

export type SourceAnswer = { track: OnlineTrack; source: string; score: number };

const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/**
 * Every source's answers for a song, best match first: the same song once per source, not once
 * in all (Choose a source is about which source, so Audius's copy must show beside YouTube's).
 */
export const sourceAnswers = (
  query: string,
  providers: readonly ProviderSearchResult[],
  keep: (track: OnlineTrack) => boolean = () => true,
): SourceAnswer[] => {
  const words = new Set(simple(query).split(' ').filter(Boolean));
  const entries: (SourceAnswer & { at: number })[] = [];
  providers.forEach((result, which) => {
    if (result.error) return;
    const seen = new Set<string>();
    const answers = result.tracks
      .filter(keep)
      .filter((track) => {
        const key = `${simple(track.artist)}|${simple(track.title)}`;
        if (seen.has(key)) return false;
        seen.add(key);
        return true;
      })
      .map((track, index) => {
        // A query naming the artist ("daft punk get lucky") counts the artist's words too.
        const artistBonus = simple(track.artist)
          .split(' ')
          .some((word) => word && words.has(word))
          ? 10
          : 0;
        const score = Math.max(
          titleScore(track.title, query),
          titleScore(`${track.artist ?? ''} ${track.title}`, query),
        );
        return { track, source: result.provider.name, score: score + artistBonus, index };
      })
      .sort((a, b) => b.score - a.score || a.index - b.index)
      .slice(0, PER_SOURCE);
    for (const answer of answers) {
      entries.push({ ...answer, at: which * 1000 + answer.index });
    }
  });
  entries.sort((a, b) => b.score - a.score || a.at - b.at);
  return entries.map(({ track, source, score }) => ({ track, source, score }));
};
