import type { SettingChange, Settings } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

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
  return () => current;
};

const rail = () => screen.getByTestId('rail-pins');
const names = () =>
  within(rail())
    .getAllByRole('link')
    .map((link) => link.getAttribute('aria-label') ?? link.textContent);

describe('The sidebar (M46.29)', () => {
  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  beforeEach(() => {
    AppShellWrapper.seedCore();
    // A window wide enough for the sidebar's names and its sort button.
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: true,
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] }]),
    );
  });

  it('starts with Favorites, Downloaded, Local files and the playlists, and sorts', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/');
    await vi.waitFor(() =>
      expect(names()).toEqual(['Favorites', 'Downloaded', 'Local files', 'Road trip']),
    );
    expect(saved().rail.seeded).toBe(true);

    // The sort button: Recently opened.
    await user.click(screen.getByRole('button', { name: 'Sort the sidebar' }));
    await user.click(await screen.findByRole('radio', { name: 'Recently opened' }));
    await vi.waitFor(() => expect(saved().rail.order).toBe('opened'));

    // Right-click a pin: unpin it.
    fireEvent.contextMenu(within(rail()).getByRole('link', { name: /Road trip/ }));
    await user.click(await screen.findByRole('menuitem', { name: 'Unpin from sidebar' }));
    await vi.waitFor(() =>
      expect(saved().rail.pins.map((pin) => pin.kind)).toEqual([
        'favorites',
        'downloaded',
        'localFiles',
      ]),
    );
  });

  it('drops the pin of a playlist that was deleted', async () => {
    const pin = (id: string, name: string) => ({
      kind: 'playlist',
      to: '/library/playlist',
      search: { id },
      name,
      artworkUrl: null,
      pinnedAt: 1,
      playedAt: 0,
      openedAt: 0,
    });
    const saved = withSettings({
      ...DEFAULT_SETTINGS,
      rail: {
        pins: [pin('p1', 'Road trip'), pin('gone', 'Deleted list')],
        order: 'custom',
        seeded: true,
      },
    });
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(saved().rail.pins.map((kept) => kept.search.id)).toEqual(['p1']));
    expect(names()).toEqual(['Road trip']);
  });

  it("pins a page from its top bar and stamps when it's opened", async () => {
    const user = userEvent.setup();
    const saved = withSettings({
      ...DEFAULT_SETTINGS,
      rail: { pins: [], order: 'custom', seeded: true },
    });
    commands.podcastGet.mockResolvedValue(
      ok({
        id: 'show-1',
        feedUrl: 'https://example.com/feed.xml',
        title: 'The Show',
        author: 'Host',
        description: null,
        artworkUrl: null,
        link: null,
        subscribed: true,
        episodes: [],
      }),
    );
    await AppShellWrapper.mount('/podcast?id=show-1');
    await user.click(await screen.findByRole('button', { name: 'Pin to sidebar' }));
    await vi.waitFor(() =>
      expect(saved().rail.pins).toEqual([
        expect.objectContaining({ kind: 'podcast', to: '/podcast', search: { id: 'show-1' } }),
      ]),
    );
    expect(within(rail()).getByRole('link', { name: /The Show/ })).toBeInTheDocument();
  });
});
