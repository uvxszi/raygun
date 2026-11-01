import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from './AppShell.test-wrapper';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

describe('AppShell', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('opens on the home page with the home tab active, and no player bar until something plays', async () => {
    await AppShellWrapper.mount();

    const home = await AppShellWrapper.page('home');
    expect(await screen.findByText('Nothing to pick up yet')).toBeInTheDocument();
    expect(home).toHaveTextContent('Home');
    expect(AppShellWrapper.activeTab).toHaveTextContent('Home');
    expect(screen.queryByTestId('mini-player')).not.toBeInTheDocument();

    AppShellWrapper.playing([makeItem('Intro')], 0);
    expect(await AppShellWrapper.miniPlayer.title).toHaveTextContent('Intro');
  });

  it('has Home, Library, Browse, Search and More, and Watch lives in the library', async () => {
    await AppShellWrapper.mount();
    const tabs = within(screen.getByTestId('bottom-tabs')).getAllByRole('link');
    expect(tabs.map((tab) => tab.textContent)).toEqual([
      'Home',
      'Library',
      'Browse',
      'Search',
      'More',
    ]);

    await AppShellWrapper.tapTab('browse');
    expect(await AppShellWrapper.page('browse')).toBeInTheDocument();
    await AppShellWrapper.tapTab('more');
    expect(await AppShellWrapper.page('more')).toHaveTextContent('Settings');
  });

  it('sends the old Watch address to Library → Videos', async () => {
    await AppShellWrapper.mount('/watch');
    expect(await screen.findByTestId('videos-library')).toBeInTheDocument();
    expect(screen.getByTestId('library-media-video')).toHaveAttribute('aria-current', 'page');
  });

  it('navigates between pages with the bottom tabs', async () => {
    await AppShellWrapper.mount();

    await AppShellWrapper.tapTab('library');
    expect(await screen.findByText('Your library is empty')).toBeInTheDocument();
    expect(AppShellWrapper.activeTab).toHaveTextContent('Library');

    await AppShellWrapper.tapTab('search');
    expect(await AppShellWrapper.page('search')).toBeInTheDocument();
  });

  it('lists settings by category and shows the platform in About', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings');

    const settings = await AppShellWrapper.page('settings');
    for (const name of ['Appearance', 'Library', 'Reader', 'Player', 'Browse', 'Data and backup']) {
      expect(within(settings).getByRole('link', { name: new RegExp(`^${name}`) })).toBeVisible();
    }
    await user.click(within(settings).getByRole('link', { name: /^About/ }));
    expect(await screen.findByTestId('platform')).toHaveTextContent('android');
    expect(await screen.findByTestId('storage-mode')).toHaveTextContent('App sandbox');
  });

  it('opens the player sheet from the mini player and closes it again', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro')], 0);

    await AppShellWrapper.miniPlayer.open();
    expect(await AppShellWrapper.playerSheet.element).toHaveTextContent('Intro');

    await AppShellWrapper.playerSheet.close();
    await AppShellWrapper.playerSheet.waitUntilClosed();
  });

  it('closes the player sheet with the system Back button and keeps the page', async () => {
    await AppShellWrapper.mount('/library');
    AppShellWrapper.playing([makeItem('Intro')], 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;

    window.history.back();

    await AppShellWrapper.playerSheet.waitUntilClosed();
    expect(await AppShellWrapper.page('library')).toBeInTheDocument();
  });

  it('removes its history entry when closed with the button', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro')], 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;
    expect(window.history.state).toEqual({ raygunSheet: true });

    await AppShellWrapper.playerSheet.close();
    await AppShellWrapper.playerSheet.waitUntilClosed();

    await waitFor(() => expect(window.history.state).not.toEqual({ raygunSheet: true }));
  });

  it('closes the player sheet with Escape', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro')], 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;

    await userEvent.keyboard('{Escape}');

    await AppShellWrapper.playerSheet.waitUntilClosed();
  });
});
