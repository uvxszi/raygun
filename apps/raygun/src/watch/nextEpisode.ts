/** What deciding where watching continues needs to know of an episode. */
export type Watchable = {
  watched: boolean;
  season: number | null;
  /** When it comes out, Unix ms; null when unknown (taken as out). */
  releasedAt: number | null;
};

/**
 * Where watching a title continues, as the player's session decides it (`continue_index` in
 * Rust): the first episode out after the last one watched, else the first one not watched (a
 * gap), else the first; specials (season 0, or no season among seasons) only when the title has
 * nothing else. `episodes` are in watching order.
 */
export const nextEpisode = <T>(
  episodes: T[],
  view: (episode: T) => Watchable,
  now = Date.now(),
): T | undefined => {
  const seen = episodes.map(view);
  const seasons = seen.some((episode) => episode.season !== null && episode.season >= 1);
  const special = (episode: Watchable) => (episode.season === null ? seasons : episode.season < 1);
  const regular = episodes.map((_, index) => index).filter((index) => !special(seen[index]!));
  const pool = regular.length > 0 ? regular : episodes.map((_, index) => index);
  const open = (index: number) =>
    !seen[index]!.watched && (seen[index]!.releasedAt === null || seen[index]!.releasedAt! <= now);
  let lastWatched = -1;
  pool.forEach((index, position) => {
    if (seen[index]!.watched) lastWatched = position;
  });
  const index =
    pool.slice(lastWatched + 1).find(open) ?? pool.find(open) ?? (pool[0] as number | undefined);
  return index === undefined ? undefined : episodes[index];
};

/** A release date as Unix ms ("2008-01-20T12:00:00.000Z" or "2008-01-20"); null when unclear. */
export const releasedAt = (released: string | null | undefined) => {
  if (!released) return null;
  const at = Date.parse(released);
  return Number.isNaN(at) ? null : at;
};
