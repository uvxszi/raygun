export type LibraryView = 'albums' | 'artists' | 'tracks';

export const LIBRARY_VIEWS: LibraryView[] = ['albums', 'artists', 'tracks'];

export const isLibraryView = (value: unknown): value is LibraryView =>
  typeof value === 'string' && (LIBRARY_VIEWS as string[]).includes(value);
