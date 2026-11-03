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

  it('has Home, Library, Browse and More, with search inside Browse', async () => {
    await AppShellWrapper.mount();
    const tabs = within(screen.getByTestId('bottom-tabs')).getAllByRole('link');
    expect(tabs.map((tab) => tab.textContent)).toEqual(['Home', 'Library', 'Browse', 'More']);

    await AppShellWrapper.tapTab('browse');
    expect(await AppShellWrapper.page('browse')).toBeInTheDocument();
    await AppShellWrapper.tapTab('more');
    expect(await AppShellWrapper.page('more')).toHaveTextContent('Settings');
  });

  it('sends the old Watch address to Library → Videos', async () => {
    await AppShellWrapper.mount('/watch');
    expect(await screen.findByTestId('video-library')).toBeInTheDocument();
    expect(screen.getByTestId('library-media-video')).toHaveAttribute('aria-current', 'page');
  });

  it('navigates between pages with the bottom tabs', async () => {
    await AppShellWrapper.mount();

    await AppShellWrapper.tapTab('library');
    expect(await screen.findByText('Your library is empty')).toBeInTheDocument();
    expect(AppShellWrapper.activeTab).toHaveTextContent('Library');

    // Browse is where content is found: its header is the search bar.
    await AppShellWrapper.tapTab('browse');
    const browse = await AppShellWrapper.page('browse');
    await userEvent.type(within(browse).getByRole('searchbox', { name: 'Search' }), 'blues');
    expect(await within(browse).findByTestId('search-page')).toBeInTheDocument();
    expect(within(browse).getByRole('link', { name: 'Search' })).toHaveAttribute(
      'aria-current',
      'page',
    );
  });

  it('sends old Search addresses to Browse → Search', async () => {
    await AppShellWrapper.mount('/search?type=manga&q=frieren');
    const browse = await AppShellWrapper.page('browse');
    expect(within(browse).getByRole('searchbox', { name: 'Search' })).toHaveValue('frieren');
    expect(within(browse).getByRole('tab', { name: 'Comics' })).toHaveAttribute(
      'aria-selected',
      'true',
    );
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
