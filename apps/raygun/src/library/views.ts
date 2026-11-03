export type LibraryView =
  | 'all'
  | 'podcasts'
  | 'audiobooks'
  | 'playlists'
  | 'favorites'
  | 'downloaded'
  | 'albums'
  | 'artists'
  | 'tracks';

/** Library → Audio's chips: music views, podcasts and audiobooks. */
export const LIBRARY_VIEWS: LibraryView[] = [
  'all',
  'podcasts',
  'audiobooks',
  'playlists',
  'favorites',
  'downloaded',
  'albums',
  'artists',
  'tracks',
];

/** Views of the device's own files; the others work without any library folder. */
export const isLocalView = (view: LibraryView) =>
  view === 'albums' || view === 'artists' || view === 'tracks';

export const isLibraryView = (value: unknown): value is LibraryView =>
  typeof value === 'string' && (LIBRARY_VIEWS as string[]).includes(value);
