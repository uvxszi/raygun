/** What a search looks through: everything, one kind of media, or what you already have. Podcasts
 * are a pill of Music since M46.25. */
export type SearchType = 'all' | 'music' | 'video' | 'manga' | 'novel' | 'library';

export const SEARCH_TYPES: SearchType[] = ['all', 'music', 'video', 'manga', 'novel', 'library'];

export const isSearchType = (value: unknown): value is SearchType =>
  typeof value === 'string' && (SEARCH_TYPES as string[]).includes(value);

/** Music's pills (M46.25): a bit of everything first, then one kind each. */
export type MusicView = 'relevant' | 'songs' | 'artists' | 'albums' | 'podcasts';
export const MUSIC_VIEWS: MusicView[] = ['relevant', 'songs', 'artists', 'albums', 'podcasts'];

/** Movies & series' pills (M46.25). */
export type VideoView = 'relevant' | 'movies' | 'series';
export const VIDEO_VIEWS: VideoView[] = ['relevant', 'movies', 'series'];

export type SearchView = MusicView | VideoView;

export const isSearchView = (value: unknown): value is SearchView =>
  typeof value === 'string' &&
  ((MUSIC_VIEWS as string[]).includes(value) || (VIDEO_VIEWS as string[]).includes(value));

export const pill = (on: boolean) =>
  on
    ? 'bg-primary/20 text-primary shrink-0 rounded-full px-4 py-1.5 text-sm font-semibold'
    : 'bg-surface shrink-0 rounded-full px-4 py-1.5 text-sm';
