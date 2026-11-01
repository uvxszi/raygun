import { createRootRoute, createRoute, createRouter, redirect } from '@tanstack/react-router';
import type { MangaBrowseMode } from '@raygun/bindings';
import type { RouterHistory } from '@tanstack/react-router';

import { isLibraryView } from './library/views';
import type { LibraryView } from './library/views';
import { AlbumPage } from './pages/AlbumPage';
import { AiPage } from './pages/AiPage';
import { CategoriesPage } from './pages/CategoriesPage';
import { PodcastPage } from './pages/PodcastPage';
import { WatchAddonsPage } from './pages/WatchAddonsPage';
import { WatchCatalogPage } from './pages/WatchCatalogPage';
import { AboutPage } from './pages/AboutPage';
import { BrowsePage } from './pages/BrowsePage';
import type { BrowseSection, ExtensionList } from './pages/BrowsePage';
import { MorePage } from './pages/MorePage';
import { ReposPage } from './pages/ReposPage';
import { StremioAddonPage } from './pages/StremioAddonPage';
import { WatchTitlePage } from './pages/WatchTitlePage';
import { VideoTitlePage } from './pages/VideoTitlePage';
import { isSearchType } from './search/searchTypes';
import type { SearchType } from './search/searchTypes';
import { DevicesPage } from './pages/DevicesPage';
import { ExtensionDetailPage } from './pages/ExtensionDetailPage';
import { ArtistPage } from './pages/ArtistPage';
import { HomePage } from './pages/HomePage';
import { LibraryPage } from './pages/LibraryPage';
import { MangaDownloadsPage } from './pages/MangaDownloadsPage';
import { MangaReaderPage } from './pages/MangaReaderPage';
import { MangaSourcePage, NovelSourcePage, VideoSourcePage } from './pages/MangaSourcePage';
import { MangaTitlePage, NovelTitlePage } from './pages/MangaTitlePage';
import { MangaUpdatesPage } from './pages/MangaUpdatesPage';
import { NovelReaderPage } from './pages/NovelReaderPage';
import { PlaylistPage } from './pages/PlaylistPage';
import { SearchPage } from './pages/SearchPage';
import { SettingsPage } from './pages/SettingsPage';
import { SETTINGS_SECTIONS } from './settings/sections';
import { SettingsSectionPage } from './pages/SettingsSectionPage';
import { AppShell } from './shell/AppShell';

const rootRoute = createRootRoute({ component: AppShell });

// Search values that look like numbers (a book id such as 84) arrive parsed; ids stay text.
const text = (value: unknown) =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '';

/** Library shows music unless `media` is `manga` or `novel`. */
export type LibraryMedia = 'podcast' | 'manga' | 'novel' | 'video';
const isLibraryMedia = (value: unknown): value is LibraryMedia =>
  value === 'podcast' || value === 'manga' || value === 'novel' || value === 'video';

const MANGA_MODES: MangaBrowseMode[] = ['popular', 'latest', 'search'];
const isMangaMode = (value: unknown): value is MangaBrowseMode =>
  typeof value === 'string' && (MANGA_MODES as string[]).includes(value);

const routeTree = rootRoute.addChildren([
  createRoute({ getParentRoute: () => rootRoute, path: '/', component: HomePage }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/library',
    component: LibraryPage,
    validateSearch: (
      search: Record<string, unknown>,
    ): { view?: LibraryView; media?: LibraryMedia } => ({
      ...(isLibraryView(search.view) ? { view: search.view } : {}),
      ...(isLibraryMedia(search.media) ? { media: search.media } : {}),
      // Podcasts used to be a music view.
      ...(search.view === 'podcasts' ? { media: 'podcast' as const } : {}),
    }),
  }),
  // The source lists moved into Browse (M42.1).
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/browse',
    beforeLoad: () => {
      throw redirect({ to: '/browse', replace: true });
    },
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/updates',
    component: MangaUpdatesPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/downloads',
    component: MangaDownloadsPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/source',
    component: MangaSourcePage,
    validateSearch: (
      search: Record<string, unknown>,
    ): { id: string; mode?: MangaBrowseMode; q?: string } => ({
      id: text(search.id),
      ...(isMangaMode(search.mode) ? { mode: search.mode } : {}),
      ...(text(search.q) ? { q: text(search.q) } : {}),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/title',
    component: MangaTitlePage,
    validateSearch: (search: Record<string, unknown>) => ({
      source: text(search.source),
      id: text(search.id),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/read',
    component: MangaReaderPage,
    validateSearch: (search: Record<string, unknown>) => ({
      title: text(search.title),
      chapter: text(search.chapter),
    }),
  }),
  // The source lists moved into Browse (M42.1).
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/novel/browse',
    beforeLoad: () => {
      throw redirect({ to: '/browse', replace: true });
    },
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/novel/source',
    component: NovelSourcePage,
    validateSearch: (
      search: Record<string, unknown>,
    ): { id: string; mode?: MangaBrowseMode; q?: string } => ({
      id: text(search.id),
      ...(isMangaMode(search.mode) ? { mode: search.mode } : {}),
      ...(text(search.q) ? { q: text(search.q) } : {}),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/novel/title',
    component: NovelTitlePage,
    validateSearch: (search: Record<string, unknown>) => ({
      source: text(search.source),
      id: text(search.id),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/novel/read',
    component: NovelReaderPage,
    validateSearch: (search: Record<string, unknown>) => ({
      title: text(search.title),
      chapter: text(search.chapter),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/library/album',
    component: AlbumPage,
    validateSearch: (search: Record<string, unknown>) => ({
      artist: text(search.artist),
      album: text(search.album),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/library/artist',
    component: ArtistPage,
    validateSearch: (search: Record<string, unknown>) => ({ name: text(search.name) }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/library/playlist',
    component: PlaylistPage,
    validateSearch: (search: Record<string, unknown>) => ({ id: text(search.id) }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/search',
    component: SearchPage,
    validateSearch: (search: Record<string, unknown>): { q?: string; type?: SearchType } => ({
      ...(text(search.q) ? { q: text(search.q) } : {}),
      ...(isSearchType(search.type) && search.type !== 'all' ? { type: search.type } : {}),
    }),
  }),
  createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: SettingsPage }),
  ...SETTINGS_SECTIONS.map((section) =>
    createRoute({
      getParentRoute: () => rootRoute,
      path: `/settings/${section.id}`,
      component: SettingsSectionPage,
    }),
  ),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/devices',
    component: DevicesPage,
  }),
  createRoute({ getParentRoute: () => rootRoute, path: '/settings/ai', component: AiPage }),
  // Watch was a tab until M42.1; what it held now lives in Library → Videos and Browse.
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch',
    beforeLoad: () => {
      throw redirect({ to: '/library', search: { media: 'video' }, replace: true });
    },
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/addon',
    component: StremioAddonPage,
    validateSearch: (search: Record<string, unknown>) => ({ id: text(search.id) }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/browse',
    component: BrowsePage,
    validateSearch: (
      search: Record<string, unknown>,
    ): { tab?: BrowseSection; list?: ExtensionList } => ({
      ...(search.tab === 'extensions' ? { tab: 'extensions' as const } : {}),
      ...(search.list === 'available' ? { list: 'available' as const } : {}),
    }),
  }),
  createRoute({ getParentRoute: () => rootRoute, path: '/more', component: MorePage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/about', component: AboutPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/extensions/repos', component: ReposPage }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/title',
    component: WatchTitlePage,
    validateSearch: (search: Record<string, unknown>) => ({
      type: text(search.type),
      id: text(search.id),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/catalog',
    component: WatchCatalogPage,
    validateSearch: (
      search: Record<string, unknown>,
    ): { addon: string; type: string; catalog: string; genre?: string } => ({
      addon: text(search.addon),
      type: text(search.type),
      catalog: text(search.catalog),
      ...(text(search.genre) ? { genre: text(search.genre) } : {}),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/addons',
    component: WatchAddonsPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/video',
    component: VideoTitlePage,
    validateSearch: (search: Record<string, unknown>) => ({
      source: text(search.source),
      id: text(search.id),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/source',
    component: VideoSourcePage,
    validateSearch: (
      search: Record<string, unknown>,
    ): { id: string; mode?: MangaBrowseMode; q?: string } => ({
      id: text(search.id),
      ...(isMangaMode(search.mode) ? { mode: search.mode } : {}),
      ...(text(search.q) ? { q: text(search.q) } : {}),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/library/categories',
    component: CategoriesPage,
    validateSearch: (search: Record<string, unknown>): { medium: 'manga' | 'novel' } => ({
      medium: search.medium === 'novel' ? 'novel' : 'manga',
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/podcast',
    component: PodcastPage,
    validateSearch: (search: Record<string, unknown>) => ({ id: text(search.id) }),
  }),
  // Extensions moved into Browse (installed, available) and their own screen (repositories).
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions',
    beforeLoad: ({ search }: { search: Record<string, unknown> }) => {
      throw search.tab === 'repos'
        ? redirect({ to: '/extensions/repos', replace: true })
        : redirect({
            to: '/browse',
            search: {
              tab: 'extensions',
              ...(search.tab === 'browse' ? { list: 'available' } : {}),
            },
            replace: true,
          });
    },
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/extensions/detail',
    component: ExtensionDetailPage,
    validateSearch: (search: Record<string, unknown>) => ({ id: text(search.id) }),
  }),
]);

export const createAppRouter = (history?: RouterHistory) => createRouter({ routeTree, history });

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
