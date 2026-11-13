// Raygun on the web, start to finish without a browser: the whole app drawn by React against the
// web core (no bindings mock), with the empty answers built from the real bindings.
import { QueryClient, QueryClientProvider } from '@tanstack/react-query';
import { RouterProvider, createMemoryHistory } from '@tanstack/react-router';
import { act, cleanup, render, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { commandAnswers } from '../../web-build/commandAnswers';
import { createAppRouter } from '../router';
import { setRemoteConnected } from './remoteState';
import { openCore } from './test/webCore';
import type { TestCore } from './test/webCore';

// Tests run in apps/raygun.
const answers = commandAnswers('../../packages/bindings/src/index.ts');

let open: TestCore | null = null;

afterEach(() => {
  // The app lets go of its listeners after this turn: the stand-in stays until the next test
  // puts its own in.
  cleanup();
  open = null;
});

const renderApp = async (path: string) => {
  open = await openCore({}, { answers });
  const client = new QueryClient({
    defaultOptions: { queries: { retry: false, networkMode: 'always' } },
  });
  const router = createAppRouter(createMemoryHistory({ initialEntries: [path] }));
  render(
    <QueryClientProvider client={client}>
      <RouterProvider router={router} />
    </QueryClientProvider>,
  );
  return open;
};

describe('Raygun on the web', () => {
  it('starts on Home with the library kept in the browser', async () => {
    const { core } = await renderApp('/');
    core.collections.createPlaylist('Road trip');
    expect(await screen.findByRole('heading', { name: 'Home' }, { timeout: 10_000 })).toBeVisible();
    // Quick access lists the playlist the core holds.
    expect(
      await screen.findAllByText('Road trip', undefined, { timeout: 10_000 }),
    ).not.toHaveLength(0);
  }, 30_000);

  it('links devices by scanning or pasting a code, never by showing one', async () => {
    await renderApp('/settings/devices');
    const page = await screen.findByTestId('linked-devices', undefined, { timeout: 10_000 });
    // Once the page knows it runs on the web.
    expect(await within(page).findByText(/open Devices and show the link code/)).toBeVisible();
    expect(within(page).queryByText('Show this device’s link code')).not.toBeInTheDocument();
    await userEvent.click(within(page).getByText('Scan a link code'));
    const field = await screen.findByRole('textbox', { name: 'Paste a link code' });
    await userEvent.type(field, 'raygun:link?at=192.168.1.20%3A8820&code=abc&name=Old');
    await userEvent.click(screen.getByRole('button', { name: 'Link' }));
    // An app without the browser fields says what to do.
    expect(await screen.findByText(/can't link with a browser yet/)).toBeVisible();
  }, 30_000);

  it('says what needs the app, until a linked app answers for the page', async () => {
    await renderApp('/local-files');
    const note = await screen.findByRole('note', undefined, { timeout: 10_000 });
    expect(within(note).getByText(/needs the Raygun app/)).toBeVisible();
    act(() => setRemoteConnected(true));
    try {
      await waitFor(() => expect(screen.queryByRole('note')).not.toBeInTheDocument());
    } finally {
      act(() => setRemoteConnected(false));
    }
  }, 30_000);
});
