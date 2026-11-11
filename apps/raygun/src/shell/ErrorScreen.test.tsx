import {
  RouterProvider,
  createMemoryHistory,
  createRootRoute,
  createRoute,
  createRouter,
  Outlet,
} from '@tanstack/react-router';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import type { FC } from 'react';

import { bindingsMock, seedBindings } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';
import { RootErrorBoundary, RouteError } from './ErrorScreen';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const Broken: FC = () => {
  throw new Error('render failed: answers[3] is undefined');
};

describe('The error screen', () => {
  beforeEach(() => {
    seedBindings();
    vi.spyOn(console, 'error').mockImplementation(() => {});
  });

  afterEach(() => {
    vi.mocked(console.error).mockRestore();
  });

  it('shows a plain message instead of the raw error and keeps a crash log of it', async () => {
    renderWithProviders(
      <RootErrorBoundary>
        <Broken />
      </RootErrorBoundary>,
    );
    expect(await screen.findByRole('heading', { name: 'Something went wrong' })).toBeVisible();
    expect(screen.queryByText(/answers\[3\]/)).not.toBeInTheDocument();
    await waitFor(() =>
      expect(commands.crashLogWrite).toHaveBeenCalledWith(
        'Error: render failed: answers[3] is undefined',
        expect.stringContaining('render failed'),
      ),
    );
    for (const name of ['Try again', 'See logs', 'Start in safe mode', 'Clear caches']) {
      expect(screen.getByRole('button', { name })).toBeVisible();
    }
  });

  it('starts Raygun again as a safe start, clearing caches when asked', async () => {
    const user = userEvent.setup();
    renderWithProviders(
      <RootErrorBoundary>
        <Broken />
      </RootErrorBoundary>,
    );
    await user.click(await screen.findByRole('button', { name: 'Start in safe mode' }));
    expect(commands.appRestartSafe).toHaveBeenLastCalledWith(false);
    await user.click(screen.getByRole('button', { name: 'Clear caches' }));
    expect(commands.appRestartSafe).toHaveBeenLastCalledWith(true);
  });

  it("takes a page's error, tries it again, and opens the crash log it left", async () => {
    const user = userEvent.setup();
    let fails = true;
    const Page: FC = () => {
      if (fails) throw new Error('the page broke');
      return <p>The page is back</p>;
    };
    const root = createRootRoute({ component: Outlet, errorComponent: RouteError });
    const router = createRouter({
      history: createMemoryHistory({ initialEntries: ['/'] }),
      routeTree: root.addChildren([
        createRoute({ getParentRoute: () => root, path: '/', component: Page }),
        createRoute({
          getParentRoute: () => root,
          path: '/about/crashes',
          component: () => <p>Crash logs page</p>,
        }),
      ]),
    });
    renderWithProviders(<RouterProvider router={router} />);
    expect(await screen.findByTestId('error-screen')).toBeVisible();
    expect(screen.queryByText('the page broke')).not.toBeInTheDocument();
    await waitFor(() => expect(commands.crashLogWrite).toHaveBeenCalled());

    await user.click(screen.getByRole('button', { name: 'See logs' }));
    expect(await screen.findByText('Crash logs page')).toBeVisible();
    expect(router.state.location.search).toEqual({ id: '1-ui.txt' });

    await router.navigate({ to: '/' });
    expect(await screen.findByTestId('error-screen')).toBeVisible();
    fails = false;
    await user.click(screen.getByRole('button', { name: 'Try again' }));
    expect(await screen.findByText('The page is back')).toBeVisible();
  });
});
