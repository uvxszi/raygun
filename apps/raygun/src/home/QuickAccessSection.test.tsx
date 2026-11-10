import type { RailPin, RailPrefs, SettingChange, Settings } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** Settings that Rust saves and sends back, as the real command does. */
const withRail = (rail: Partial<RailPrefs>) => {
  let current: Settings = { ...DEFAULT_SETTINGS, rail: { ...DEFAULT_SETTINGS.rail, ...rail } };
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
  return () => current;
};

const ALBUM: RailPin = {
  kind: 'album',
  to: '/music/album',
  search: { provider: 'dev.example.tunes/meta', id: 'discovery' },
  name: 'Discovery',
  artworkUrl: null,
  pinnedAt: 1,
  playedAt: 0,
  openedAt: 0,
  auto: false,
};

/** The row's cards by what they're called (a name, what it is, and Pinned for a pin). */
const cards = (row: HTMLElement) =>
  within(row)
    .getAllByTestId('quick-access-card')
    .map((card) => within(card).getByRole('link').getAttribute('aria-label'));

describe("Home's Quick access", () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] }]),
    );
  });

  it('lists what quick access lists, in its order: the pins first, then the library', async () => {
    withRail({ pins: [ALBUM] });
    await AppShellWrapper.mount('/');
    const row = await screen.findByRole('region', { name: 'Quick access' });
    await vi.waitFor(() =>
      expect(cards(row)).toEqual([
        'Discovery · Album · Pinned',
        'Favorites',
        'Downloaded',
        'Local files',
        'Road trip · Playlist',
      ]),
    );
    expect(within(row).getByRole('link', { name: 'Road trip · Playlist' })).toHaveAttribute(
      'href',
      '/library/playlist?id=p1',
    );
    expect(within(row).getByRole('link', { name: 'Show all of Quick access' })).toHaveAttribute(
      'href',
      '/library',
    );
    expect(screen.queryByRole('region', { name: 'Your music' })).not.toBeInTheDocument();
  });

  it('leaves out what quick access is set to leave out', async () => {
    withRail({ kinds: { ...DEFAULT_SETTINGS.rail.kinds, downloaded: false, localFiles: false } });
    await AppShellWrapper.mount('/');
    const row = await screen.findByRole('region', { name: 'Quick access' });
    await vi.waitFor(() => expect(cards(row)).toEqual(['Favorites', 'Road trip · Playlist']));
  });

  it("gives each card quick access's menu: a pin goes first, and back", async () => {
    const user = userEvent.setup();
    const saved = withRail({});
    await AppShellWrapper.mount('/');
    const row = await screen.findByRole('region', { name: 'Quick access' });
    await within(row).findByRole('link', { name: 'Road trip · Playlist' });

    await user.click(within(row).getByRole('button', { name: 'Options for Road trip' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Pin' }),
    );
    await vi.waitFor(() =>
      expect(saved().rail.pins.map((pin) => [pin.to, pin.search])).toEqual([
        ['/library/playlist', { id: 'p1' }],
      ]),
    );
    await vi.waitFor(() => expect(cards(row)[0]).toBe('Road trip · Playlist · Pinned'));

    await user.click(within(row).getByRole('button', { name: 'Options for Road trip' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Unpin' }),
    );
    await vi.waitFor(() => expect(saved().rail.pins).toEqual([]));
    await vi.waitFor(() => expect(cards(row).at(-1)).toBe('Road trip · Playlist'));
  });
});
