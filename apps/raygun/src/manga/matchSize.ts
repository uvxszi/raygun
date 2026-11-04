import type { MangaTitleView, VideoTitleView } from '@raygun/bindings';

/** How much of a title a source has, to compare sources ("Find in other sources"). */
export type MatchSize = {
  /** Chapters or episodes; none for a movie. */
  units: number | null;
  /** The furthest chapter or episode number. */
  latest: number | null;
  /** The season of the latest episode, for series with seasons. */
  latestSeason: number | null;
  /** The newest release date, Unix ms. */
  latestDate: number | null;
  /** Seasons (specials not counted), when there are several. */
  seasons: number | null;
  /** The total length, only when every episode has one. */
  durationMs: number | null;
};

const maxOf = (values: (number | null)[]) => {
  const known = values.filter((value): value is number => value !== null && Number.isFinite(value));
  return known.length > 0 ? Math.max(...known) : null;
};

export const mangaSize = (view: MangaTitleView): MatchSize => ({
  units: view.chapters.length,
  latest: maxOf(view.chapters.map((chapter) => chapter.number)),
  latestSeason: null,
  latestDate: maxOf(view.chapters.map((chapter) => chapter.dateUpload)),
  seasons: null,
  durationMs: null,
});

export const videoSize = (view: VideoTitleView): MatchSize => {
  const episodes = view.episodes;
  const movie =
    view.mediaType === 'movie' || (episodes.length === 1 && view.mediaType !== 'series');
  // Specials (season 0) neither count as a season nor as the latest episode.
  const regular = episodes.filter((episode) => episode.season !== 0);
  const seasonNumbers = new Set(
    regular.map((episode) => episode.season).filter((season): season is number => season !== null),
  );
  const latestSeason = maxOf([...seasonNumbers]);
  const inLatest =
    latestSeason === null ? regular : regular.filter((episode) => episode.season === latestSeason);
  const lengths = episodes.map((episode) => episode.durationMs);
  const durationMs =
    episodes.length > 0 && lengths.every((length) => length !== null && length > 0)
      ? lengths.reduce<number>((sum, length) => sum + (length ?? 0), 0)
      : null;
  return {
    units: movie ? null : episodes.length,
    latest: movie ? null : maxOf(inLatest.map((episode) => episode.number)),
    latestSeason: movie || seasonNumbers.size < 2 ? null : latestSeason,
    latestDate: maxOf(episodes.map((episode) => episode.date)),
    seasons: seasonNumbers.size > 1 ? seasonNumbers.size : null,
    durationMs,
  };
};

/** A number as a chapter shows it: 12, 12.5. */
export const formatNumber = (value: number) =>
  Number.isInteger(value) ? String(value) : String(Math.round(value * 10) / 10);

/** Runs at most `limit` tasks at once; the rest wait their turn in order. */
export const createLimiter = (limit: number) => {
  let running = 0;
  const waiting: (() => void)[] = [];
  const next = () => {
    running -= 1;
    waiting.shift()?.();
  };
  return <T>(task: () => Promise<T>): Promise<T> =>
    new Promise<T>((resolve, reject) => {
      const start = () => {
        running += 1;
        task().then(resolve, reject).finally(next);
      };
      if (running < limit) {
        start();
      } else {
        waiting.push(start);
      }
    });
};
