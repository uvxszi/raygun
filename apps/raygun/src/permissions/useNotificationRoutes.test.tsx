import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import {
  Outlet,
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  useSearch,
} from '@tanstack/react-router';
import { render, screen } from '@testing-library/react';
import type { FC } from 'react';

import { CollectionSheets } from '../collections/CollectionSheetsHost';
import { useCollectionSheets } from '../collections/collectionSheets';
import { bindingsMock, emitEvent, seedBindings } from '../test/bindingsMock';
import { useNotificationRoutes } from './useNotificationRoutes';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const Shell: FC = () => {
  useNotificationRoutes();
  return (
    <>
      <Outlet />
      <CollectionSheets />
    </>
  );
};

const BrowsePage: FC = () => {
  const search = useSearch({ strict: false }) as { q?: string };
  return <p>Searching {search.q}</p>;
};

const setup = () => {
  const root = createRootRoute({ component: Shell });
  const router = createRouter({
    routeTree: root.addChildren([
      createRoute({ getParentRoute: () => root, path: '/', component: () => <p>Home</p> }),
      createRoute({ getParentRoute: () => root, path: '/browse', component: BrowsePage }),
    ]),
    history: createMemoryHistory({ initialEntries: ['/'] }),
  });
  const queryClient = new QueryClient({ defaultOptions: { queries: { retry: false } } });
  render(
    <QueryClientProvider client={queryClient}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
};

describe('useNotificationRoutes', () => {
  beforeEach(() => {
    seedBindings();
    useCollectionSheets.getState().close();
  });

  it('searches for words or a link another app shared', async () => {
    setup();
    await screen.findByText('Home');

    emitEvent('openRouteEvent', '/browse?tab=search&q=lo-fi+beats');

    expect(await screen.findByText('Searching lo-fi beats')).toBeInTheDocument();
    expect(bindingsMock.commands.notificationRouteTake).toHaveBeenCalled();
  });

  it('opens Import songs with a shared Spotify link', async () => {
    setup();
    await screen.findByText('Home');
    const link = 'https://open.spotify.com/playlist/37i9dQZF1DXcBWIGoYBM5M';

    emitEvent('openRouteEvent', `import?link=${encodeURIComponent(link)}`);

    expect(await screen.findByLabelText('Playlist, album or song link')).toHaveValue(link);
  });

  it('opens a route that came before the screen listened', async () => {
    bindingsMock.commands.notificationRouteTake.mockResolvedValueOnce(
      '/browse?tab=search&q=https%3A%2F%2Fyoutu.be%2Fabc',
    );

    setup();

    expect(await screen.findByText('Searching https://youtu.be/abc')).toBeInTheDocument();
  });
});
