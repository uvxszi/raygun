import type { SettingChange, Settings } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

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
  commands.settingsUpdateMany.mockImplementation(async (changes: SettingChange[]) => {
    for (const change of changes) current = { ...current, [change.key]: change.value };
    return ok(current);
  });
  return () => current;
};

const listed = () =>
  within(screen.getByTestId('rail-pins'))
    .getAllByRole('link')
    .map((link) => link.getAttribute('aria-label') ?? link.textContent);

describe('Settings → Quick access', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    // A desktop window wide enough for the sidebar beside the settings.
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: true,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.playlistsList.mockResolvedValue(
      ok(
        Array.from({ length: 15 }, (_, index) => ({
          id: `p${index}`,
          name: `List ${index}`,
          itemCount: 1,
          artworkUrls: [],
        })),
      ),
    );
  });

  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('lists at most the number chosen, 10 to 1000, and says how many more', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/settings/quickAccess');
    const count = await screen.findByRole('spinbutton', { name: 'Items shown' });
    expect(count).toHaveValue(100);
    await waitFor(() => expect(listed()).toHaveLength(18));

    await user.clear(count);
    await user.type(count, '10{Enter}');
    await waitFor(() => expect(saved().rail.shown).toBe(10));
    // The fixed blocks and the playlists are 18: ten show, and a link says eight more.
    await waitFor(() => expect(listed()).toHaveLength(10));
    expect(screen.getByRole('link', { name: '8 more in your library' })).toBeInTheDocument();

    // Kept within the limits, and back to 100.
    const typed = screen.getByRole('spinbutton', { name: 'Items shown' });
    await user.clear(typed);
    await user.type(typed, '5000{Enter}');
    await waitFor(() => expect(saved().rail.shown).toBe(1000));
    await user.click(screen.getByRole('button', { name: 'Reset Items shown' }));
    await waitFor(() => expect(saved().rail.shown).toBe(100));
    expect(screen.getByRole('slider', { name: 'Items shown' })).toHaveAttribute(
      'aria-valuetext',
      '100 items',
    );
  });

  it('chooses what is listed, the order and whether pins stay on top', async () => {
    const user = userEvent.setup();
    const pin = {
      kind: 'manga',
      to: '/manga/title',
      search: { source: 's', id: 'comic' },
      name: 'A comic',
      artworkUrl: null,
      pinnedAt: 1,
      playedAt: 0,
      openedAt: 0,
    };
    const saved = withSettings({
      ...DEFAULT_SETTINGS,
      rail: { ...DEFAULT_SETTINGS.rail, pins: [pin] },
    });
    await AppShellWrapper.mount('/settings/quickAccess');
    await waitFor(() => expect(listed()).toContain('Favorites'));

    // Favorites off: gone from the list; playlists off too, but a pin always shows.
    await user.click(await screen.findByRole('switch', { name: 'Favorites' }));
    await waitFor(() => expect(saved().rail.kinds?.favorites).toBe(false));
    await user.click(screen.getByRole('switch', { name: 'Playlists' }));
    await waitFor(() => expect(saved().rail.kinds?.playlists).toBe(false));
    await waitFor(() => expect(listed()).toEqual(['A comic', 'Downloaded', 'Local files']));

    await user.click(screen.getByRole('switch', { name: 'Pins stay on top' }));
    await waitFor(() => expect(saved().rail.pinnedFirst).toBe(false));

    await user.click(screen.getByRole('button', { name: /Sort by/ }));
    await user.click(await screen.findByRole('radio', { name: 'Name' }));
    await waitFor(() => expect(saved().rail.order).toBe('name'));

    // Comics, books and videos added to the library: pinned, at most so many.
    await user.click(screen.getByRole('switch', { name: 'Pin what you add to the library' }));
    await waitFor(() => expect(saved().rail.autoAdd).toBe(false));
    expect(screen.getByRole('button', { name: /Keep at most/ })).toBeDisabled();

    // Reset to defaults keeps the pins.
    await user.click(screen.getByRole('button', { name: 'More options' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Reset to defaults' }));
    await user.click(await screen.findByRole('button', { name: 'Reset' }));
    await waitFor(() => expect(saved().rail.order).toBe('played'));
    expect(saved().rail.pinnedFirst).toBe(true);
    expect(saved().rail.autoAdd).toBe(true);
    expect(saved().rail.kinds?.favorites).toBe(true);
    expect(saved().rail.pins).toEqual([pin]);
  });
});
