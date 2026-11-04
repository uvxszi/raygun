import { createRootRoute, createRoute, createRouter, redirect } from '@tanstack/react-router';
import type { MangaBrowseMode } from '@raygun/bindings';
import type { RouterHistory } from '@tanstack/react-router';

import { isLibraryView } from './library/views';
import type { Medium } from './manga/medium';
import type { LibraryView } from './library/views';
import { AlbumPage } from './pages/AlbumPage';
import { AiPage } from './pages/AiPage';
import { CategoriesPage } from './pages/CategoriesPage';
import { PodcastPage } from './pages/PodcastPage';
import { WatchAddonsPage } from './pages/WatchAddonsPage';
import { WatchCatalogPage } from './pages/WatchCatalogPage';
import { AboutPage } from './pages/AboutPage';
import { MusicAlbumPage } from './pages/MusicAlbumPage';
import { MusicPlaylistPage } from './pages/MusicPlaylistPage';
import { MusicChartPage } from './pages/MusicChartPage';
import { MusicChartsPage } from './pages/MusicChartsPage';
import { MusicTagPage } from './pages/MusicTagPage';
import { MadeForYouPage } from './pages/MadeForYouPage';
import { AudiobookPage } from './pages/AudiobookPage';
import { MixPage } from './pages/MixPage';
import { BrowsePage } from './pages/BrowsePage';
import { isExtensionList } from './extensions/tabs';
import type { BrowseSection, ExtensionList } from './pages/BrowsePage';
import { MorePage } from './pages/MorePage';
import { ReposPage } from './pages/ReposPage';
import { LocalFilesPage } from './pages/LocalFilesPage';
import { StremioAddonPage } from './pages/StremioAddonPage';
import { WatchHistoryPage } from './pages/WatchHistoryPage';
import { WatchTitlePage } from './pages/WatchTitlePage';
import { VideoTitlePage } from './pages/VideoTitlePage';
import { isSearchType, isSearchView, searchLink } from './search/searchTypes';
import type { SearchType, SearchView } from './search/searchTypes';
import { DevicesPage } from './pages/DevicesPage';
import { HiddenPage } from './pages/HiddenPage';
import { ExtensionDetailPage } from './pages/ExtensionDetailPage';
import { MusicArtistPage } from './pages/MusicArtistPage';
import { HomePage } from './pages/HomePage';
import { LibraryPage } from './pages/LibraryPage';
import { DownloadsPage } from './pages/DownloadsPage';
import type { DownloadsTab } from './pages/DownloadsPage';
import { MangaReaderPage } from './pages/MangaReaderPage';
import { MangaSourcePage, NovelSourcePage, VideoSourcePage } from './pages/MangaSourcePage';
import { MangaTitlePage, NovelTitlePage } from './pages/MangaTitlePage';
import { UpdatesPage } from './pages/UpdatesPage';
import type { UpdatesTab } from './pages/UpdatesPage';
import { NovelReaderPage } from './pages/NovelReaderPage';
import { PlaylistPage } from './pages/PlaylistPage';
import { SettingsPage } from './pages/SettingsPage';
import { SETTINGS_SECTIONS } from './settings/sections';
import { PlayerSettingsPage } from './pages/PlayerSettingsPage';
import { LayoutSettingsPage, LayoutSurfacePage } from './pages/LayoutSettingsPage';
import { SettingsSectionPage } from './pages/SettingsSectionPage';
import { AppShell } from './shell/AppShell';

const rootRoute = createRootRoute({ component: AppShell });

// Search values that look like numbers (a book id such as 84) arrive parsed; ids stay text.
const text = (value: unknown) =>
  typeof value === 'string' ? value : typeof value === 'number' ? String(value) : '';

/** Library shows Audio unless `media` is `manga` (Comics), `novel` (Books) or `video`. */
export type LibraryMedia = 'manga' | 'novel' | 'video';
const isLibraryMedia = (value: unknown): value is LibraryMedia =>
  value === 'manga' || value === 'novel' || value === 'video';

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
    ): {
      view?: LibraryView;
      media?: LibraryMedia | 'music' | 'all' | 'websites';
      site?: string;
    } => ({
      ...(isLibraryView(search.view) ? { view: search.view } : {}),
      ...(isLibraryMedia(search.media) ||
      search.media === 'music' ||
      search.media === 'all' ||
      search.media === 'websites'
        ? { media: search.media }
        : {}),
      // A website's quick access pin: Library → Websites opens it.
      ...(search.media === 'websites' && text(search.site) ? { site: text(search.site) } : {}),
      // Podcasts were a media tab of their own before they joined Audio.
      ...(search.media === 'podcast' ? { view: 'podcasts' as const } : {}),
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
  // Updates and downloads cover chapters and videos (M43).
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/updates',
    component: UpdatesPage,
    validateSearch: (search: Record<string, unknown>): { tab?: UpdatesTab } =>
      search.tab === 'episodes' || search.tab === 'chapters' || search.tab === 'releases'
        ? { tab: search.tab }
        : {},
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/downloads',
    component: DownloadsPage,
    validateSearch: (search: Record<string, unknown>): { tab?: DownloadsTab } =>
      search.tab === 'videos' ||
      search.tab === 'exports' ||
      search.tab === 'music' ||
      search.tab === 'chapters'
        ? { tab: search.tab }
        : {},
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/updates',
    beforeLoad: () => {
      throw redirect({ to: '/updates', replace: true });
    },
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/manga/downloads',
    beforeLoad: () => {
      throw redirect({ to: '/downloads', search: { tab: 'chapters' }, replace: true });
    },
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
    // Every artist name leads to one page now (M46.8).
    beforeLoad: ({ search }) => {
      throw redirect({ to: '/artist', search: { name: search.name }, replace: true });
    },
    validateSearch: (search: Record<string, unknown>) => ({ name: text(search.name) }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/music/charts',
    component: MusicChartsPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/music/chart',
    component: MusicChartPage,
    validateSearch: (search: Record<string, unknown>) => ({
      provider: text(search.provider),
      id: text(search.id),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/music/album',
    component: MusicAlbumPage,
    validateSearch: (search: Record<string, unknown>) => ({
      provider: text(search.provider),
      id: text(search.id),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/music/playlist',
    component: MusicPlaylistPage,
    validateSearch: (search: Record<string, unknown>) => ({
      provider: text(search.provider),
      id: text(search.id),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/music/for-you',
    component: MadeForYouPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/audiobooks',
    // Finding audiobooks is Search → Audio's Audiobooks pill.
    beforeLoad: () => {
      throw redirect({ ...searchLink('music', 'audiobooks'), replace: true });
    },
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/audiobook',
    component: AudiobookPage,
    validateSearch: (search: Record<string, unknown>) => ({ id: text(search.id) }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/music/mix',
    component: MixPage,
    validateSearch: (search: Record<string, unknown>) => ({ id: text(search.id) }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/music/tag',
    component: MusicTagPage,
    validateSearch: (search: Record<string, unknown>) => ({ name: text(search.name) }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/artist',
    component: MusicArtistPage,
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
    // Search lives in Browse now: old links land on its Search tab.
    beforeLoad: ({ search }: { search: Record<string, unknown> }) => {
      throw redirect({
        to: '/browse',
        search: {
          tab: 'search',
          ...(text(search.q) ? { q: text(search.q) } : {}),
          ...(isSearchType(search.type) && search.type !== 'all' ? { type: search.type } : {}),
          ...(isSearchView(search.view) && search.view !== 'relevant' ? { view: search.view } : {}),
          ...(search.type === 'podcast'
            ? { type: 'music' as const, view: 'podcasts' as const }
            : {}),
        },
        replace: true,
      });
    },
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
    path: '/settings/player/$page',
    component: PlayerSettingsPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/layout',
    component: LayoutSettingsPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/layout/$surface',
    component: LayoutSurfacePage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/devices',
    component: DevicesPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/settings/hidden',
    component: HiddenPage,
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
    ): {
      tab?: BrowseSection;
      list?: ExtensionList;
      q?: string;
      type?: SearchType;
      view?: SearchView;
      /** A video source's own Search pill (M46.35). */
      pill?: string;
    } => ({
      ...(search.tab === 'extensions' ||
      search.tab === 'search' ||
      search.tab === 'sources' ||
      search.tab === 'discover'
        ? { tab: search.tab }
        : {}),
      // Installed is the default; "available" was the whole catalogue's name before.
      ...(search.list === 'available'
        ? { list: 'all' as const }
        : isExtensionList(search.list) && search.list !== 'installed'
          ? { list: search.list }
          : {}),
      ...(text(search.q) ? { q: text(search.q) } : {}),
      ...(isSearchType(search.type) && search.type !== 'all' ? { type: search.type } : {}),
      ...(isSearchView(search.view) && search.view !== 'relevant' ? { view: search.view } : {}),
      ...(text(search.pill) ? { pill: text(search.pill) } : {}),
      // Podcasts were a search type of their own before they joined Music (M46.25).
      ...(search.type === 'podcast' ? { type: 'music' as const, view: 'podcasts' as const } : {}),
    }),
  }),
  createRoute({ getParentRoute: () => rootRoute, path: '/more', component: MorePage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/about', component: AboutPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/extensions/repos', component: ReposPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/local-files', component: LocalFilesPage }),
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
    ): { addon?: string; type?: string; catalog?: string; genre?: string; extra?: string } => ({
      ...(text(search.addon) ? { addon: text(search.addon) } : {}),
      ...(text(search.type) ? { type: text(search.type) } : {}),
      ...(text(search.catalog) ? { catalog: text(search.catalog) } : {}),
      ...(text(search.genre) ? { genre: text(search.genre) } : {}),
      // Discover's other choices (a year...), as a query string.
      ...(text(search.extra) ? { extra: text(search.extra) } : {}),
    }),
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/addons',
    component: WatchAddonsPage,
  }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/watch/history',
    component: WatchHistoryPage,
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
    validateSearch: (search: Record<string, unknown>): { medium: Medium } => ({
      medium: search.medium === 'novel' || search.medium === 'video' ? search.medium : 'manga',
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
              ...(search.tab === 'browse' ? { list: 'all' } : {}),
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
