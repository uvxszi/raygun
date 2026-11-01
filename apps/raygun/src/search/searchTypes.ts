/** What a search looks through: everything, one kind of media, or what you already have. */
export type SearchType = 'all' | 'music' | 'podcast' | 'video' | 'manga' | 'novel' | 'library';

export const SEARCH_TYPES: SearchType[] = [
  'all',
  'music',
  'podcast',
  'video',
  'manga',
  'novel',
  'library',
];

export const isSearchType = (value: unknown): value is SearchType =>
  typeof value === 'string' && (SEARCH_TYPES as string[]).includes(value);
