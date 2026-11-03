import type { MusicAlbum } from '@raygun/bindings';

/** How a discography tab is sorted (M46.45). */
export type ReleaseSort = 'newest' | 'oldest' | 'title' | 'tracks';
export const RELEASE_SORTS: ReleaseSort[] = ['newest', 'oldest', 'title', 'tracks'];

const STORE = 'raygun.discography.sort';

/** Each tab's sort, remembered on this device. */
export const readReleaseSorts = (): Partial<Record<string, ReleaseSort>> => {
  try {
    const saved = JSON.parse(localStorage.getItem(STORE) ?? '{}') as unknown;
    return saved && typeof saved === 'object'
      ? (saved as Partial<Record<string, ReleaseSort>>)
      : {};
  } catch {
    return {};
  }
};

export const saveReleaseSorts = (sorts: Partial<Record<string, ReleaseSort>>) => {
  try {
    localStorage.setItem(STORE, JSON.stringify(sorts));
  } catch {
    // Not remembered; the sort still applies now.
  }
};

/** Releases in `sort`'s order; ones without a year or a track count go last. */
export const sortReleases = (releases: MusicAlbum[], sort: ReleaseSort): MusicAlbum[] => {
  const last = (value: number | null | undefined, descending: boolean) =>
    value === null || value === undefined ? Infinity : descending ? -value : value;
  const by: Record<ReleaseSort, (a: MusicAlbum, b: MusicAlbum) => number> = {
    newest: (a, b) => last(a.year, true) - last(b.year, true),
    oldest: (a, b) => last(a.year, false) - last(b.year, false),
    title: (a, b) => a.title.localeCompare(b.title, undefined, { numeric: true }),
    tracks: (a, b) => last(a.trackCount, true) - last(b.trackCount, true),
  };
  // Stable: releases that tie keep the sources' order.
  return [...releases].sort((a, b) => by[sort](a, b) || 0);
};
