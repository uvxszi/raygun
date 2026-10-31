import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from './AppShell.test-wrapper';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

describe('AppShell', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('opens on the home page with the home tab active', async () => {
    await AppShellWrapper.mount();

    expect(await AppShellWrapper.page('home')).toHaveTextContent('Welcome to Raygun');
    expect(AppShellWrapper.activeTab).toHaveTextContent('Home');
  });

  it('navigates between pages with the bottom tabs', async () => {
    await AppShellWrapper.mount();

    await AppShellWrapper.tapTab('library');
    expect(await AppShellWrapper.page('library')).toHaveTextContent('Your library is empty');
    expect(AppShellWrapper.activeTab).toHaveTextContent('Library');

    await AppShellWrapper.tapTab('search');
    expect(await AppShellWrapper.page('search')).toBeInTheDocument();
  });

  it('shows appearance and about sections in settings', async () => {
    await AppShellWrapper.mount('/settings');

    const settings = await AppShellWrapper.page('settings');
    expect(settings).toHaveTextContent('Appearance');
    expect(await screen.findByTestId('platform')).toHaveTextContent('android');
    expect(await screen.findByTestId('storage-mode')).toHaveTextContent('App sandbox');
  });

  it('opens the player sheet from the mini player and closes it again', async () => {
    await AppShellWrapper.mount();

    await AppShellWrapper.miniPlayer.open();
    expect(await AppShellWrapper.playerSheet.element).toHaveTextContent('Nothing playing');

    await AppShellWrapper.playerSheet.close();
    await AppShellWrapper.playerSheet.waitUntilClosed();
  });

  it('closes the player sheet with the system Back button and keeps the page', async () => {
    await AppShellWrapper.mount('/library');
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;

    window.history.back();

    await AppShellWrapper.playerSheet.waitUntilClosed();
    expect(await AppShellWrapper.page('library')).toBeInTheDocument();
  });

  it('removes its history entry when closed with the button', async () => {
    await AppShellWrapper.mount();
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;
    expect(window.history.state).toEqual({ raygunSheet: true });

    await AppShellWrapper.playerSheet.close();
    await AppShellWrapper.playerSheet.waitUntilClosed();

    await waitFor(() => expect(window.history.state).not.toEqual({ raygunSheet: true }));
  });

  it('closes the player sheet with Escape', async () => {
    await AppShellWrapper.mount();
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;

    await userEvent.keyboard('{Escape}');

    await AppShellWrapper.playerSheet.waitUntilClosed();
  });
});
