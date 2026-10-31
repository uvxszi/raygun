import { createRootRoute, createRoute, createRouter } from '@tanstack/react-router';
import type { RouterHistory } from '@tanstack/react-router';

import { isLibraryView } from './library/views';
import type { LibraryView } from './library/views';
import { AlbumPage } from './pages/AlbumPage';
import { ArtistPage } from './pages/ArtistPage';
import { HomePage } from './pages/HomePage';
import { LibraryPage } from './pages/LibraryPage';
import { SearchPage } from './pages/SearchPage';
import { SettingsPage } from './pages/SettingsPage';
import { AppShell } from './shell/AppShell';

const rootRoute = createRootRoute({ component: AppShell });

const text = (value: unknown) => (typeof value === 'string' ? value : '');

const routeTree = rootRoute.addChildren([
  createRoute({ getParentRoute: () => rootRoute, path: '/', component: HomePage }),
  createRoute({
    getParentRoute: () => rootRoute,
    path: '/library',
    component: LibraryPage,
    validateSearch: (search: Record<string, unknown>): { view?: LibraryView } =>
      isLibraryView(search.view) ? { view: search.view } : {},
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
  createRoute({ getParentRoute: () => rootRoute, path: '/search', component: SearchPage }),
  createRoute({ getParentRoute: () => rootRoute, path: '/settings', component: SettingsPage }),
]);

export const createAppRouter = (history?: RouterHistory) => createRouter({ routeTree, history });

declare module '@tanstack/react-router' {
  interface Register {
    router: ReturnType<typeof createAppRouter>;
  }
}
