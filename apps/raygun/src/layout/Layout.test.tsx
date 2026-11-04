import type { LayoutItem, SettingChange, Settings } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';
import { LAYOUT_DEFAULTS } from './layout';
import type { Layout } from './layout';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** Settings that Rust saves and sends back, as the real command does. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

const withLayout = (change: Partial<Layout>) =>
  withSettings({ ...DEFAULT_SETTINGS, layout: { ...LAYOUT_DEFAULTS, ...change } });

const show = (items: LayoutItem[], ...ids: string[]) =>
  items.map((item) => (ids.includes(item.id) ? { ...item, visible: true } : item));

/** The ids of the layout last saved for `surface`, shown ones only. */
const savedShown = (surface: keyof Layout) => {
  const calls = commands.settingsUpdate.mock.calls.filter(([change]) => change.key === 'layout');
  const last = calls.at(-1)?.[0] as { value: Layout } | undefined;
  return last?.value[surface].filter((item) => item.visible).map((item) => item.id);
};

describe('Customizable layout', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withSettings(DEFAULT_SETTINGS);
  });

  it('adds, moves and hides tabs, and the bar follows', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/layout/tabs');
    const editor = await screen.findByTestId('layout-editor');

    // Watch is listed, hidden, and More can't be hidden.
    const watch = within(editor).getByRole('switch', { name: 'Show Watch' });
    expect(watch).toHaveAttribute('aria-checked', 'false');
    expect(within(editor).queryByRole('switch', { name: 'Show More' })).not.toBeInTheDocument();
    expect(
      within(screen.getByTestId('layout-row-more')).getByLabelText('Always shown'),
    ).toBeInTheDocument();

    await user.click(watch);
    expect(savedShown('tabs')).toEqual(['home', 'library', 'browse', 'more', 'watch']);
    const bar = screen.getByTestId('bottom-tabs');
    expect(await within(bar).findByTestId('nav-watch-tab')).toHaveTextContent('Watch');

    // Watch before More, then Home hidden.
    await user.click(within(editor).getByRole('button', { name: 'Move Watch up' }));
    expect(savedShown('tabs')).toEqual(['home', 'library', 'browse', 'watch', 'more']);
    await user.click(within(editor).getByRole('switch', { name: 'Show Home' }));
    expect(savedShown('tabs')).toEqual(['library', 'browse', 'watch', 'more']);
    await vi.waitFor(() =>
      expect(within(bar).queryByTestId('nav-home-tab')).not.toBeInTheDocument(),
    );
    const tabs = within(bar)
      .getAllByRole('link')
      .map((link) => link.textContent);
    expect(tabs).toEqual(['Library', 'Browse', 'Watch', 'More']);

    // Back to the default.
    await user.click(screen.getByRole('button', { name: 'Reset to default' }));
    expect(savedShown('tabs')).toEqual(['home', 'library', 'browse', 'more']);
  });

  it('lists tabs that do not fit in More, and opens the chosen start screen', async () => {
    withSettings({
      ...DEFAULT_SETTINGS,
      startScreen: 'watch',
      layout: { ...LAYOUT_DEFAULTS, tabs: show(LAYOUT_DEFAULTS.tabs, 'watch', 'comics', 'search') },
    });
    await AppShellWrapper.mount('/');

    // The app opens on Watch: Library → Videos, with the Watch tab current.
    await vi.waitFor(() =>
      expect(screen.getByTestId('library-media-video')).toHaveAttribute('aria-current', 'page'),
    );
    const bar = screen.getByTestId('bottom-tabs');
    expect(within(bar).getByTestId('nav-watch-tab')).toHaveAttribute('aria-current', 'page');
    expect(within(bar).getByTestId('nav-library-tab')).not.toHaveAttribute('aria-current');

    // Seven shown: four in the bar and More; the others are at the top of More.
    expect(
      within(bar)
        .getAllByRole('link')
        .map((link) => link.textContent),
    ).toEqual(['Home', 'Library', 'Browse', 'Watch', 'More']);
    await AppShellWrapper.tapTab('more');
    const others = await screen.findByRole('list', { name: 'Your other tabs' });
    expect(within(others).getByRole('link', { name: /Comics/ })).toBeInTheDocument();
    expect(within(others).getByRole('link', { name: /Search/ })).toBeInTheDocument();
    expect(within(others).queryByRole('link', { name: /Watch/ })).not.toBeInTheDocument();
  });

  it('lists hidden default tabs in More, but not screens that were never added', async () => {
    withLayout({
      tabs: LAYOUT_DEFAULTS.tabs.map((item) =>
        item.id === 'home' ? { ...item, visible: false } : item,
      ),
    });
    await AppShellWrapper.mount('/more');
    const others = await screen.findByRole('list', { name: 'Your other tabs' });
    expect(within(others).getByRole('link', { name: /Home/ })).toBeInTheDocument();
    expect(within(others).queryByRole('link', { name: /Music/ })).not.toBeInTheDocument();
    expect(within(others).queryByRole('link', { name: /Watch/ })).not.toBeInTheDocument();
  });

  it('opens the first library tab shown, and Audio on its first view', async () => {
    withLayout({
      libraryMedia: [
        { id: 'all', visible: false },
        { id: 'video', visible: true },
        { id: 'music', visible: true },
        { id: 'manga', visible: false },
        { id: 'novel', visible: true },
      ],
    });
    await AppShellWrapper.mount('/library');
    // Once settings arrive.
    await vi.waitFor(() =>
      expect(screen.getByTestId('library-media-video')).toHaveAttribute('aria-current', 'page'),
    );
    expect(screen.getByTestId('library-media-music')).not.toHaveAttribute('aria-current');
    const tabs = screen.getByRole('navigation', { name: 'What to show' });
    expect(
      within(tabs)
        .getAllByRole('link')
        .map((link) => link.textContent),
      // A tab added later (Websites) joins after the one it follows by default.
    ).toEqual(['Videos', 'Websites', 'Audio', 'Books']);
  });

  it('arranges Home and More', async () => {
    withLayout({
      homeSections: [
        { id: 'recentlyPlayed', visible: true },
        { id: 'yourMusic', visible: true },
        { id: 'continueWatching', visible: false },
        { id: 'continueReading', visible: true },
      ],
      moreEntries: LAYOUT_DEFAULTS.moreEntries.map((item) =>
        item.id === 'about' ? { ...item, visible: false } : item,
      ),
    });
    await AppShellWrapper.mount('/');
    const home = await screen.findByTestId('home-page');
    const headings = within(home)
      .getAllByRole('heading', { level: 2 })
      .map((heading) => heading.textContent);
    expect(headings.indexOf('Recently played')).toBeLessThan(headings.indexOf('Your music'));

    await AppShellWrapper.tapTab('more');
    const more = await screen.findByTestId('more-page');
    expect(within(more).getByRole('link', { name: /Settings/ })).toBeInTheDocument();
    expect(within(more).queryByRole('link', { name: /About/ })).not.toBeInTheDocument();
  });

  it('lists every surface with what it shows', async () => {
    await AppShellWrapper.mount('/settings/layout');
    const page = await screen.findByTestId('layout-settings');
    const tabs = within(page).getByRole('link', { name: /Navigation tabs/ });
    expect(tabs).toHaveTextContent('Home · Library · Browse · More');
    expect(within(page).getByRole('link', { name: /Library tabs/ })).toHaveTextContent(
      'Audio · Comics · Books · Videos',
    );
  });
});
