import type { CachePrefs } from '@raygun/bindings';

export type Cache = Required<CachePrefs>;

/** Matches `CachePrefs::default()` in raygun-model. */
export const CACHE_DEFAULTS: Cache = {
  coverCacheMb: 512,
  pageCacheMb: 512,
  coverRefreshDays: 7,
  clearPagesOnStart: false,
  coverSourceSongs: 25_000,
  songFactsDays: 365,
};

/**
 * Matches `CACHE_SIZES_MB`, `COVER_REFRESH_DAYS`, `COVER_SOURCE_SONGS` and `SONG_FACTS_DAYS` in
 * raygun-model.
 */
export const CACHE_SIZES_MB = [128, 256, 512, 1024, 2048, 4096];
export const COVER_REFRESH_DAYS = [0, 1, 3, 7, 30];
export const COVER_SOURCE_SONGS = [5_000, 10_000, 25_000, 50_000, 100_000];
export const SONG_FACTS_DAYS = [90, 180, 365, 730, 0];

/** The cache settings with every field filled (the stored row may predate some). */
export const withCache = (prefs: CachePrefs | undefined): Cache => ({
  ...CACHE_DEFAULTS,
  ...prefs,
});
