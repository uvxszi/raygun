import type { QuickAccessItem, SettingChange, Settings } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, DEFAULT_SETTINGS, emitEvent } from '../test/bindingsMock';

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
const menu = () => screen.getByRole('menu');
const openMenu = async (name: RegExp) => {
  fireEvent.contextMenu(within(rail()).getByRole('link', { name }));
  return screen.findByRole('menu');
};

/** A row as Rust's `quick_access_list` gives it. */
const row = (name: string, extra: Partial<QuickAccessItem> = {}): QuickAccessItem => ({
  kind: 'playlist',
  to: '/library/playlist',
  search: { id: name },
  name,
  subtitle: null,
  artworkUrl: null,
  artworkUrls: [],
  pinned: false,
  library: true,
  removable: true,
  addedAt: 0,
  playedAt: 0,
  ...extra,
});

const pinOf = (id: string) => ({
  kind: 'playlist',
  to: '/library/playlist',
  search: { id },
  name: id,
  artworkUrl: null,
  pinnedAt: 1,
  playedAt: 0,
  openedAt: 0,
});

describe('Quick access in the sidebar', () => {
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

  it('lists the audio library by itself and sorts it', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/');
    await vi.waitFor(() =>
      expect(names()).toEqual(['Favorites', 'Downloaded', 'Local files', 'Road trip']),
    );
    expect(commands.quickAccessList).toHaveBeenCalledWith({
      favorites: 'Favorites',
      downloaded: 'Downloaded',
      localFiles: 'Local files',
    });
    // Nothing was pinned to get there.
    expect(saved().rail.pins).toEqual([]);

    await user.click(screen.getByRole('button', { name: 'Sort the sidebar' }));
    const orders = await screen.findByRole('radiogroup', { name: 'Sort the sidebar' });
    expect(
      within(orders)
        .getAllByRole('radio')
        .map((radio) => radio.textContent),
    ).toEqual(['Recently played', 'Recently added', 'Name']);
    expect(within(orders).getByRole('radio', { name: 'Recently played' })).toBeChecked();
    await user.click(within(orders).getByRole('radio', { name: 'Recently added' }));
    await vi.waitFor(() => expect(saved().rail.order).toBe('added'));
  });

  it('shows the order Rust gives, and lists again when Rust says it changed', async () => {
    withSettings(DEFAULT_SETTINGS);
    commands.quickAccessList.mockResolvedValue(
      ok({
        items: [
          row('Played last', { playedAt: 900 }),
          row('Favorites', { kind: 'favorites', to: '/library', search: { view: 'favorites' } }),
          row('Never played'),
        ],
        more: 0,
      }),
    );
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(names()).toEqual(['Played last', 'Favorites', 'Never played']));

    commands.quickAccessList.mockResolvedValue(
      ok({ items: [row('Never played', { playedAt: 1000 }), row('Played last')], more: 0 }),
    );
    emitEvent('quickAccessChangedEvent', '/library/playlist');
    await vi.waitFor(() => expect(names()).toEqual(['Never played', 'Played last']));
  });

  it('pins an item on top with a right click, and unpins it: it stays listed', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(names()).toContain('Road trip'));

    await openMenu(/Road trip/);
    expect(within(menu()).queryByRole('menuitem', { name: 'Unpin' })).not.toBeInTheDocument();
    expect(within(menu()).getByRole('menuitem', { name: 'Remove from library' })).toBeVisible();
    // The playlist's own items stay, but its Delete: Remove from library is that.
    expect(within(menu()).getByRole('menuitem', { name: 'Shuffle' })).toBeVisible();
    expect(within(menu()).queryByRole('menuitem', { name: 'Delete playlist' })).toBeNull();
    await user.click(within(menu()).getByRole('menuitem', { name: 'Pin' }));
    await vi.waitFor(() =>
      expect(saved().rail.pins).toEqual([
        expect.objectContaining({
          kind: 'playlist',
          to: '/library/playlist',
          search: { id: 'p1' },
        }),
      ]),
    );
    // On top, marked pinned.
    await vi.waitFor(() =>
      expect(names()).toEqual(['Road trip', 'Favorites', 'Downloaded', 'Local files']),
    );
    expect(within(rail()).getByRole('list', { name: 'Pinned' })).toHaveTextContent('Road trip');

    await openMenu(/Road trip/);
    await user.click(within(menu()).getByRole('menuitem', { name: 'Unpin' }));
    await vi.waitFor(() => expect(saved().rail.pins).toEqual([]));
    await vi.waitFor(() => expect(names()).toContain('Road trip'));
    expect(within(rail()).queryByRole('list', { name: 'Pinned' })).not.toBeInTheDocument();
  });

  it('removes a playlist from the library after asking in the menu', async () => {
    const user = userEvent.setup();
    withSettings(DEFAULT_SETTINGS);
    commands.playlistDelete.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(names()).toContain('Road trip'));

    // From the keyboard: the first item has the focus, Up goes round to the last.
    await openMenu(/Road trip/);
    await vi.waitFor(() =>
      expect(within(menu()).getByRole('menuitem', { name: 'Pin' })).toHaveFocus(),
    );
    await user.keyboard('{ArrowUp}');
    expect(within(menu()).getByRole('menuitem', { name: 'Remove from library' })).toHaveFocus();
    await user.keyboard('{Enter}');
    const question = await screen.findByRole('alertdialog', {
      name: /Remove “Road trip” from your library\?/,
    });
    expect(commands.playlistDelete).not.toHaveBeenCalled();
    await user.click(within(question).getByRole('button', { name: 'Remove from library' }));
    await vi.waitFor(() => expect(commands.playlistDelete).toHaveBeenCalledWith('p1'));

    // The fixed blocks never leave the library.
    await openMenu(/Favorites/);
    expect(
      within(menu()).queryByRole('menuitem', { name: 'Remove from library' }),
    ).not.toBeInTheDocument();
  });

  it('unfollows a show and an artist from their menus', async () => {
    const user = userEvent.setup();
    withSettings(DEFAULT_SETTINGS);
    commands.podcastSetSubscribed.mockResolvedValue(ok(null));
    commands.quickAccessList.mockResolvedValue(
      ok({
        items: [
          row('The Show', { kind: 'podcast', to: '/podcast', search: { id: 'show-1' } }),
          row('Raygun', { kind: 'artist', to: '/artist', search: { name: 'Raygun' } }),
          row('Local album', {
            kind: 'album',
            to: '/library/album',
            search: { artist: 'A', album: 'Local album' },
            removable: false,
          }),
        ],
        more: 0,
      }),
    );
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(names()).toContain('The Show'));

    await openMenu(/The Show/);
    await user.click(within(menu()).getByRole('menuitem', { name: 'Remove from library' }));
    await user.click(
      within(await screen.findByRole('alertdialog')).getByRole('button', {
        name: 'Remove from library',
      }),
    );
    await vi.waitFor(() =>
      expect(commands.podcastSetSubscribed).toHaveBeenCalledWith('show-1', false),
    );

    await openMenu(/Raygun/);
    await user.click(within(menu()).getByRole('menuitem', { name: 'Remove from library' }));
    await user.click(
      within(await screen.findByRole('alertdialog')).getByRole('button', {
        name: 'Remove from library',
      }),
    );
    await vi.waitFor(() => expect(commands.artistUnfollow).toHaveBeenCalledWith('Raygun'));

    // The device's own album comes from its files: it can only be pinned.
    await openMenu(/Local album/);
    expect(within(menu()).getByRole('menuitem', { name: 'Pin' })).toBeVisible();
    expect(
      within(menu()).queryByRole('menuitem', { name: 'Remove from library' }),
    ).not.toBeInTheDocument();
  });

  it('keeps pins on top in their order, moved from the menu', async () => {
    const user = userEvent.setup();
    const saved = withSettings({
      ...DEFAULT_SETTINGS,
      rail: { ...DEFAULT_SETTINGS.rail, pins: [pinOf('a'), pinOf('b')] },
    });
    commands.quickAccessList.mockResolvedValue(
      ok({
        items: [row('a', { pinned: true }), row('b', { pinned: true }), row('c')],
        more: 0,
      }),
    );
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(names()).toEqual(['a', 'b', 'c']));

    await openMenu(/^a/);
    expect(within(menu()).queryByRole('menuitem', { name: 'Move up' })).not.toBeInTheDocument();
    await user.click(within(menu()).getByRole('menuitem', { name: 'Move down' }));
    await vi.waitFor(() =>
      expect(saved().rail.pins.map((pin) => pin.search.id)).toEqual(['b', 'a']),
    );
    // Shown at once, before Rust lists again.
    await vi.waitFor(() => expect(names()).toEqual(['b', 'a', 'c']));
  });

  it('draws only the rows near the screen, and says how many more there are', async () => {
    withSettings(DEFAULT_SETTINGS);
    commands.quickAccessList.mockResolvedValue(
      ok({
        items: Array.from({ length: 300 }, (_, index) =>
          row(`List ${index}`, { artworkUrl: `https://example.com/${index}.jpg` }),
        ),
        more: 42,
      }),
    );
    await AppShellWrapper.mount('/');
    await vi.waitFor(() => expect(names()[0]).toBe('List 0'));
    const drawn = names().length;
    expect(drawn).toBeGreaterThan(5);
    expect(drawn).toBeLessThan(60);
    // Covers of rows off the screen aren't there to load.
    const covers = screen.queryAllByTestId('artwork-image').length;
    expect(covers).toBeGreaterThan(0);
    expect(covers).toBeLessThan(60);
    expect(screen.getByRole('link', { name: '42 more in your library' })).toBeInTheDocument();
  });

  it('pins a page from its top bar', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
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
        order: { sort: 'date', descending: true, filters: [] },
        customOrder: false,
        episodeCount: 0,
        downloaded: 0,
        speed: null,
      }),
    );
    await AppShellWrapper.mount('/podcast?id=show-1');
    await user.click(await screen.findByRole('button', { name: 'Pin to Quick access' }));
    await vi.waitFor(() =>
      expect(saved().rail.pins).toEqual([
        expect.objectContaining({ kind: 'podcast', to: '/podcast', search: { id: 'show-1' } }),
      ]),
    );
    expect(await within(rail()).findByRole('link', { name: /The Show/ })).toBeInTheDocument();
  });

  it('resizes the split between the tabs and Your library', async () => {
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/');
    const split = await screen.findByRole('separator', {
      name: 'Drag to resize the tabs and Your library',
    });
    expect(split).toHaveAttribute('aria-orientation', 'horizontal');
    const tabs = screen.getByTestId('rail-tabs');
    expect(tabs.style.height).toBe('');

    fireEvent.keyDown(split, { key: 'ArrowDown' });
    await vi.waitFor(() => expect(saved().panels.railNavHeight).toBe(60));
    // Shorter than its tabs, the part scrolls.
    await vi.waitFor(() => expect(tabs.style.height).toBe('60px'));
    expect(tabs).toHaveClass('overflow-y-auto');
    fireEvent.keyDown(split, { key: 'Home' });
    await vi.waitFor(() => expect(saved().panels.railNavHeight).toBe(44));

    // A double click goes back to the tabs' own height.
    fireEvent.doubleClick(split);
    await vi.waitFor(() => expect(saved().panels.railNavHeight).toBe(0));
    await vi.waitFor(() => expect(tabs.style.height).toBe(''));
  });

  it('keeps room for Your library when the tabs grow', async () => {
    const saved = withSettings(DEFAULT_SETTINGS);
    const height = vi.spyOn(Element.prototype, 'clientHeight', 'get').mockImplementation(function (
      this: Element,
    ) {
      return this.getAttribute('data-testid') === 'side-rail' ? 600 : 0;
    });
    try {
      await AppShellWrapper.mount('/');
      const split = await screen.findByRole('separator', {
        name: 'Drag to resize the tabs and Your library',
      });
      await vi.waitFor(() => expect(split).toHaveAttribute('aria-valuemax', '480'));
      fireEvent.keyDown(split, { key: 'End' });
      await vi.waitFor(() => expect(saved().panels.railNavHeight).toBe(480));
    } finally {
      height.mockRestore();
    }
  });

  it('checks downloads and scans local folders from their tiles', async () => {
    const user = userEvent.setup();
    withSettings({ ...DEFAULT_SETTINGS, rail: { pins: [], order: 'played', seeded: true } });
    commands.songDownloadsCheck.mockResolvedValue(ok({ checked: 2, problems: 0 }));
    commands.localFoldersRescan.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/library?view=all');

    await user.click(
      await within(screen.getByTestId('page-content')).findByRole('button', {
        name: 'Options for Downloaded',
      }),
    );
    await user.click(await screen.findByRole('menuitem', { name: 'Check downloads' }));
    await vi.waitFor(() => expect(commands.songDownloadsCheck).toHaveBeenCalledWith(null));
    expect(await screen.findByText('All 2 downloads are fine')).toBeInTheDocument();

    await user.click(
      await within(screen.getByTestId('page-content')).findByRole('button', {
        name: 'Options for Local files',
      }),
    );
    expect(await screen.findByRole('menuitem', { name: 'Add a folder' })).toBeInTheDocument();
    expect(screen.queryByRole('menuitem', { name: 'Play' })).not.toBeInTheDocument();
    await user.click(screen.getByRole('menuitem', { name: 'Scan the folders again' }));
    await vi.waitFor(() => expect(commands.localFoldersRescan).toHaveBeenCalledWith(null));
  });

  it('opens Favorites with Sort by open from the menu', async () => {
    const user = userEvent.setup();
    withSettings({ ...DEFAULT_SETTINGS, rail: { pins: [], order: 'played', seeded: true } });
    commands.favoritesList.mockResolvedValue(
      ok([
        {
          trackKey: 'k1',
          title: 'Intro',
          artist: 'Raygun',
          album: null,
          durationMs: 1000,
          artworkUrl: null,
          source: null,
          addedAt: 1,
        },
      ] as never),
    );
    await AppShellWrapper.mount('/library?view=all');
    await user.click(
      await within(screen.getByTestId('page-content')).findByRole('button', {
        name: 'Options for Favorites',
      }),
    );
    await user.click(await screen.findByRole('menuitem', { name: 'Sort by' }));
    expect(await screen.findByTestId('favorites-view')).toBeInTheDocument();
    expect(await screen.findByRole('radiogroup', { name: 'Sort by' })).toBeInTheDocument();
  });
});
