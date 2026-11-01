import { commands } from '@raygun/bindings';

/** What a catalogue page shows: manga (images), novels (text) or videos (M42). They share
 * sources, browsing and search; manga and novels also share titles and reading. */
export type Medium = 'manga' | 'novel' | 'video';

/** The media read page by page, with a library of their own. */
export type ReadingMedium = 'manga' | 'novel';

/** The sources and browsing commands of each medium. */
export const browseCommands = (medium: Medium) =>
  medium === 'video'
    ? { sources: commands.videoSources, browse: commands.videoBrowse }
    : catalogCommands(medium);

/** The catalogue commands of a reading medium; both take and return the same shapes. */
export const catalogCommands = (medium: ReadingMedium) =>
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

/** The library list of a medium (videos: movies and series from sources and Stremio, M43). */
export const libraryCommand = (medium: Medium) =>
  medium === 'video' ? commands.videoLibrary : catalogCommands(medium).library;

/** The source id of imported EPUB books. */
export const LOCAL_EPUB = 'local.epub';

export const ROUTES = {
  manga: {
    browse: '/browse',
    source: '/manga/source',
    title: '/manga/title',
    read: '/manga/read',
  },
  novel: {
    browse: '/browse',
    source: '/novel/source',
    title: '/novel/title',
    read: '/novel/read',
  },
  video: {
    browse: '/browse',
    source: '/watch/source',
    title: '/watch/video',
    read: '/watch/video',
  },
} as const;
