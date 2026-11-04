import type { SettingChange, Settings } from '@raygun/bindings';
import { act, fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from './AppShell.test-wrapper';
import { resetBackLayers } from './backLayers';
import { useQuickAccessStore } from './quickAccessStore';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
  return () => current;
};

/** The screen's size in CSS px, as Android's WebView reports it. */
const setScreen = (width: number, height: number) => {
  Object.defineProperty(window.screen, 'width', { value: width, configurable: true });
  Object.defineProperty(window.screen, 'height', { value: height, configurable: true });
};

/** A window as wide as a phone on its side: `md` and taller, never `lg`. */
const setLandscape = () => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: !query.includes('1024px'),
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

const drawer = () => screen.getByRole('dialog', { name: 'Quick access' });
const pinNames = () =>
  within(within(drawer()).getByTestId('rail-pins'))
    .getAllByRole('link')
    .map((link) => link.getAttribute('aria-label') ?? link.textContent);

describe('Phones (no side panel, quick access)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    useQuickAccessStore.setState({ open: false });
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] }]),
    );
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
    setScreen(0, 0);
  });

  it('keeps the phone bar and no side panel on its side', async () => {
    setLandscape();
    setScreen(393, 851);
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro', 'Raygun')], 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;
    expect(within(bar).queryByRole('button', { name: 'Shuffle' })).not.toBeInTheDocument();
    expect(screen.queryByRole('complementary', { name: 'Now playing' })).not.toBeInTheDocument();
    expect(screen.getByTestId('quick-access-button')).toBeInTheDocument();
  });

  it('keeps the wide bar on a tablet', async () => {
    setLandscape();
    setScreen(800, 1280);
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro', 'Raygun')], 0);
    await AppShellWrapper.miniPlayer.title;
    expect(
      within(AppShellWrapper.miniPlayer.element).getByRole('button', { name: 'Shuffle' }),
    ).toBeInTheDocument();
  });

  it('opens the pins from Raygun’s icon, toggles compact and closes from outside', async () => {
    setScreen(393, 851);
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/');
    await user.click(await screen.findByTestId('quick-access-button'));
    await waitFor(() =>
      expect(pinNames()).toEqual(['Favorites', 'Downloaded', 'Local files', 'Road trip']),
    );
    expect(drawer()).toHaveAttribute('data-expanded');

    await user.click(within(drawer()).getByRole('button', { name: 'Show icons only' }));
    await waitFor(() => expect(saved().rail.drawerExpanded).toBe(false));
    await waitFor(() => expect(drawer()).not.toHaveAttribute('data-expanded'));
    // Compact: names only as labels, and the sort button stays.
    expect(within(drawer()).queryByText('Road trip')).not.toBeInTheDocument();
    await user.click(within(drawer()).getByRole('button', { name: 'Sort the sidebar' }));
    await user.click(screen.getByRole('radio', { name: 'Recently opened' }));
    await waitFor(() => expect(saved().rail.order).toBe('opened'));

    await user.click(screen.getByTestId('quick-access-backdrop'));
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Quick access' })).not.toBeInTheDocument(),
    );
  });

  it('opens from the right edge, and a pin opens its page and closes it', async () => {
    setScreen(393, 851);
    const user = userEvent.setup();
    withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/library');
    await screen.findByTestId('quick-access-button');
    const edge = window.innerWidth - 4;
    fireEvent.touchStart(window, { touches: [{ clientX: edge, clientY: 300 }] });
    fireEvent.touchMove(window, { touches: [{ clientX: edge - 60, clientY: 305 }] });
    await waitFor(() => expect(pinNames()).toContain('Road trip'));

    await user.click(within(drawer()).getByRole('link', { name: /Road trip/ }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Quick access' })).not.toBeInTheDocument(),
    );
  });

  it('holds a fixed pin for its menu, and pins it again from its tile', async () => {
    setScreen(393, 851);
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/library?view=all');
    await user.click(await screen.findByTestId('quick-access-button'));
    await waitFor(() => expect(pinNames()).toContain('Downloaded'));

    fireEvent.contextMenu(within(drawer()).getByRole('link', { name: /Downloaded/ }));
    expect(await screen.findByRole('menuitem', { name: 'Check downloads' })).toBeInTheDocument();
    await user.click(screen.getByRole('menuitem', { name: 'Unpin from Quick access' }));
    await waitFor(() =>
      expect(saved().rail.pins.map((pin) => pin.kind)).not.toContain('downloaded'),
    );

    await user.click(screen.getByTestId('quick-access-backdrop'));
    fireEvent.contextMenu(await screen.findByRole('link', { name: /^Downloaded.*tracks?$/ }));
    await user.click(await screen.findByRole('menuitem', { name: 'Pin to Quick access' }));
    await waitFor(() => expect(saved().rail.pins.map((pin) => pin.kind)).toContain('downloaded'));
  });

  it('closes on Back', async () => {
    // A phone: the drawer has a history entry of its own (shell/backLayers).
    resetBackLayers(true);
    setScreen(393, 851);
    withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/browse');
    await userEvent.click(await screen.findByTestId('quick-access-button'));
    expect(drawer()).toBeInTheDocument();
    await waitFor(() => expect(window.history.state).toHaveProperty('raygunLayer'));
    act(() => window.history.back());
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Quick access' })).not.toBeInTheDocument(),
    );
  });

  it('isn’t there on desktop, which has the sidebar', async () => {
    commands.appInfo.mockResolvedValue({ ...(await commands.appInfo()), platform: 'windows' });
    await AppShellWrapper.mount('/');
    await AppShellWrapper.page('home');
    expect(screen.queryByTestId('quick-access-button')).not.toBeInTheDocument();
  });
});
