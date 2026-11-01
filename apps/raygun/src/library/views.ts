export type LibraryView = 'playlists' | 'favorites' | 'albums' | 'artists' | 'tracks';

export const LIBRARY_VIEWS: LibraryView[] = [
  'playlists',
  'favorites',
  'albums',
  'artists',
  'tracks',
];

/** Views of the device's own files; the others work without any library folder. */
export const isLocalView = (view: LibraryView) =>
  view === 'albums' || view === 'artists' || view === 'tracks';

export const isLibraryView = (value: unknown): value is LibraryView =>
  typeof value === 'string' && (LIBRARY_VIEWS as string[]).includes(value);
