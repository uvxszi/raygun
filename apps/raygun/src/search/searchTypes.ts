/** What a search looks through: everything, one kind of media, what you already have, or answers
 * to a question or calculation. Podcasts are a pill of Music since M46.25. */
export type SearchType = 'all' | 'music' | 'video' | 'manga' | 'novel' | 'library' | 'answers';

export const SEARCH_TYPES: SearchType[] = [
  'all',
  'music',
  'video',
  'manga',
  'novel',
  'library',
  'answers',
];

export const isSearchType = (value: unknown): value is SearchType =>
  typeof value === 'string' && (SEARCH_TYPES as string[]).includes(value);

/** Music's pills (M46.25): a bit of everything first, then one kind each. */
export type MusicView =
  'relevant' | 'songs' | 'artists' | 'albums' | 'playlists' | 'podcasts' | 'audiobooks';
export const MUSIC_VIEWS: MusicView[] = [
  'relevant',
  'songs',
  'artists',
  'albums',
  'playlists',
  'podcasts',
  'audiobooks',
];

/** Movies & series' pills (M46.25). */
export type VideoView = 'relevant' | 'movies' | 'series' | 'sources';
export const VIDEO_VIEWS: VideoView[] = ['relevant', 'movies', 'series', 'sources'];

export type SearchView = MusicView | VideoView;

export const isSearchView = (value: unknown): value is SearchView =>
  typeof value === 'string' &&
  ((MUSIC_VIEWS as string[]).includes(value) || (VIDEO_VIEWS as string[]).includes(value));

/** The pills a kind of search has, Relevant first; kinds without pills have none. */
export const viewsOf = (type: SearchType): readonly SearchView[] =>
  type === 'music' ? MUSIC_VIEWS : type === 'video' ? VIDEO_VIEWS : [];

/** The pill shown for `view` within `type`: Relevant unless `view` is one of its pills. */
export const viewIn = <View extends SearchView>(type: SearchType, view: unknown): View =>
  ((viewsOf(type) as unknown[]).includes(view) ? view : 'relevant') as View;

/** Where Browse → Search shows `type`'s `view` pill (Relevant when none), searching for `q`. */
export type SearchLocation = {
  to: '/browse';
  search: { tab: 'search'; type?: SearchType; view?: SearchView; q?: string; pill?: string };
};

/**
 * The one way to a search (every pill, every link): Browse → Search on a kind's pill, with what
 * to search for. Links elsewhere (Library's "Find…", Browse all's tiles) use it, so each pill
 * they open is the pill Search shows.
 */
export const searchLink = (
  type: SearchType = 'all',
  view?: SearchView,
  q?: string,
): SearchLocation => {
  const shown = view ? viewIn(type, view) : 'relevant';
  return {
    to: '/browse',
    search: {
      tab: 'search',
      ...(type === 'all' ? {} : { type }),
      ...(shown === 'relevant' ? {} : { view: shown }),
      ...(q?.trim() ? { q: q.trim() } : {}),
    },
  };
};

export const pill = (on: boolean) =>
  on
    ? 'bg-primary/20 text-primary shrink-0 rounded-full px-4 py-1.5 text-sm font-semibold'
    : 'bg-surface shrink-0 rounded-full px-4 py-1.5 text-sm';
