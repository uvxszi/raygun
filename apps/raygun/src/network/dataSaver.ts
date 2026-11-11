import type { DataSaverPrefs } from '@raygun/bindings';

/** Data Saver as a new install has it (`DataSaverPrefs::default` in raygun-model). */
export const SAVER_DEFAULTS: Required<DataSaverPrefs> = {
  enabled: false,
  mobileDataOnly: false,
  limitVideo: true,
  videoResolution: '480p',
  limitMusic: true,
  musicQuality: 'low',
  fewerSources: true,
  noPrefetch: true,
  noBackgroundLookups: true,
  smallerCovers: true,
  noTrailers: true,
  imagesWhenVisible: true,
};

/** `given`, with what it leaves out (a row saved by an older version) as in `defaults`. */
export const withDefaults = <T extends object>(defaults: Required<T>, given?: T): Required<T> => {
  const set = (given ?? {}) as Record<string, unknown>;
  return Object.fromEntries(
    Object.entries(defaults).map(([key, value]) => [key, set[key] ?? value]),
  ) as Required<T>;
};
