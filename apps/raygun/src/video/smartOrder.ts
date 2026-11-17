import type { TrackOption } from './trackPicking';

/** What Smart Subtitles adds to a track: its place among its language's and its mark. */
type Ranked = Pick<TrackOption, 'language'> & { rank?: number };

/**
 * `listed` with Smart Subtitles' order inside each language (the most like the others first);
 * languages keep their places, and tracks without a rank keep theirs after the ranked ones.
 */
export const smartOrder = <T extends Ranked>(listed: T[]): T[] => {
  if (!listed.some((option) => option.rank !== undefined)) return listed;
  const languageAt = new Map<string, number>();
  listed.forEach((option, index) => {
    const language = option.language ?? '';
    if (!languageAt.has(language)) languageAt.set(language, index);
  });
  return listed
    .map((option, index) => ({ option, index }))
    .sort(
      (a, b) =>
        (languageAt.get(a.option.language ?? '') ?? 0) -
          (languageAt.get(b.option.language ?? '') ?? 0) ||
        (a.option.rank ?? Number.MAX_SAFE_INTEGER) - (b.option.rank ?? Number.MAX_SAFE_INTEGER) ||
        a.index - b.index,
    )
    .map(({ option }) => option);
};
