import { commands } from '@raygun/bindings';

/** What a catalogue page shows: manga (images) or novels (text). They share browsing and titles. */
export type Medium = 'manga' | 'novel';

/** The catalogue commands of each medium; both take and return the same shapes. */
export const catalogCommands = (medium: Medium) =>
  medium === 'novel'
    ? {
        sources: commands.novelSources,
        browse: commands.novelBrowse,
        title: commands.novelTitle,
        setInLibrary: commands.novelSetInLibrary,
        library: commands.novelLibrary,
        setRead: commands.novelSetRead,
      }
    : {
        sources: commands.mangaSources,
        browse: commands.mangaBrowse,
        title: commands.mangaTitle,
        setInLibrary: commands.mangaSetInLibrary,
        library: commands.mangaLibrary,
        setRead: commands.mangaSetRead,
      };

export const ROUTES = {
  manga: {
    browse: '/manga/browse',
    source: '/manga/source',
    title: '/manga/title',
    read: '/manga/read',
  },
  novel: {
    browse: '/novel/browse',
    source: '/novel/source',
    title: '/novel/title',
    read: '/novel/read',
  },
} as const;
