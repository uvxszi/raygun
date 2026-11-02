import type { CachePrefs } from '@raygun/bindings';

export type Cache = Required<CachePrefs>;

/** Matches `CachePrefs::default()` in raygun-model. */
export const CACHE_DEFAULTS: Cache = {
  coverCacheMb: 512,
  pageCacheMb: 512,
  coverRefreshDays: 7,
  clearPagesOnStart: false,
};

/** Matches `CACHE_SIZES_MB` and `COVER_REFRESH_DAYS` in raygun-model. */
export const CACHE_SIZES_MB = [128, 256, 512, 1024, 2048, 4096];
export const COVER_REFRESH_DAYS = [0, 1, 3, 7, 30];

/** The cache settings with every field filled (the stored row may predate some). */
export const withCache = (prefs: CachePrefs | undefined): Cache => ({
  ...CACHE_DEFAULTS,
  ...prefs,
});
