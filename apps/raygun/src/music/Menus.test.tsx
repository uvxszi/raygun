import type { MusicAlbum, OnlineTrack, PlayContext } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';
import { Play } from 'lucide-react';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import type { MenuItem } from '../shell/contextMenuStore';
import { bindingsMock } from '../test/bindingsMock';
import { buildMenu } from './mediaMenu';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const TUNES = {
  id: 'dev.example.tunes/catalogue',
  extensionId: 'dev.example.tunes',
  kind: 'music.metadata',
  name: 'Tunes',
};

const song = (title: string, artist: string, album: string | null = null): OnlineTrack => ({
  externalId: null,
  title,
  artist,
  album,
  durationMs: null,
  artworkUrl: null,
  source: null,
  explicit: false,
});

const ALBUM: MusicAlbum = {
  provider: TUNES,
  id: '6575789',
  title: 'Random Access Memories',
  artist: 'Daft Punk',
  year: 2013,
  artworkUrl: null,
  trackCount: 13,
};

/** The menu's lines as text: items by label, `|` for a separator, `#` before the heading. */
const lines = (menu: HTMLElement) =>
  Array.from(menu.querySelectorAll('[role="menuitem"], [role="separator"], div.text-muted')).map(
    (line) =>
      line.getAttribute('role') === 'separator'
        ? '|'
        : line.getAttribute('role') === 'menuitem'
          ? (line.textContent ?? '')
          : `# ${line.textContent ?? ''}`,
  );

/** Everything before the page's own options (Back, Forward…). */
const ownLines = (menu: HTMLElement) => {
  const all = lines(menu);
  const back = all.indexOf('BackAlt+←');
  return back < 0 ? all : all.slice(0, back - 1);
};

const touchScreen = (coarse: boolean) => {
  window.matchMedia = ((query: string) => ({
    matches: coarse && query === '(pointer: coarse)',
    media: query,
    onchange: null,
    addEventListener: () => undefined,
    removeEventListener: () => undefined,
    addListener: () => undefined,
    removeListener: () => undefined,
    dispatchEvent: () => false,
  })) as unknown as typeof window.matchMedia;
};

describe('Menus', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });
  afterEach(() => {
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('keeps one order, with a line between groups and none for an empty group', () => {
    const item = (label: string): MenuItem => ({ label, icon: Play, onSelect: () => undefined });
    const menu = buildMenu({
      heading: { label: 'Thing' },
      hide: [item('Not interested')],
      play: [item('Play'), false, item('Add to queue')],
      share: [],
      save: [null, item('Add to playlist')],
      keep: [item('Download')],
    });
    expect(menu.map((line) => ('label' in line ? line.label : '|'))).toEqual([
      'Thing',
      'Play',
      'Add to queue',
      '|',
      'Add to playlist',
      '|',
      'Download',
      '|',
      'Not interested',
    ]);
  });

  it("gives a song the basic items in order, and its artist's Don't show", async () => {
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: TUNES,
            tracks: [song('Get Lucky', 'Daft Punk, Pharrell Williams', 'Random Access Memories')],
            error: null,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=get+lucky');
    fireEvent.contextMenu(await screen.findByTestId('online-track-row'), {
      clientX: 30,
      clientY: 30,
    });
    expect(ownLines(await screen.findByRole('menu'))).toEqual([
      '# Get Lucky',
      'Play',
      'Play next',
      'Add to queue',
      '|',
      'Add to playlist',
      'Add to favorites',
      '|',
      'Go to album',
      'Go to Daft Punk',
      'Go to Pharrell Williams',
      'Find this song elsewhere',
      '|',
      'Download',
      '|',
      'Copy title',
      'Copy artist',
      '|',
      'Edit details',
      'Edit sound and lyrics',
      'Find a source…',
      '|',
      "Don't show Daft Punk",
    ]);
  });

  it('gives an album card the same order, and plays, queues and adds all its songs', async () => {
    const user = userEvent.setup();
    commands.onlineSearch.mockResolvedValue(ok({ library: [], top: [], providers: [] }));
    commands.musicSearchAlbums.mockResolvedValue(ok([ALBUM]));
    commands.musicAlbum.mockResolvedValue(
      ok({ album: ALBUM, tracks: [song('One', 'Daft Punk'), song('Two', 'Daft Punk')] }),
    );
    commands.playbackQueueTracks.mockResolvedValue(ok(null));
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] }]),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=albums&q=random');
    const tile = await screen.findByRole('link', { name: /Random Access Memories/ });

    fireEvent.contextMenu(tile, { clientX: 30, clientY: 30 });
    const menu = await screen.findByRole('menu');
    expect(ownLines(menu)).toEqual([
      '# Random Access Memories',
      'Play',
      'Shuffle',
      'Play next',
      'Add to queue',
      '|',
      'Add to playlist',
      '|',
      'Go to Daft Punk',
      '|',
      'Download',
      // Quick access holds pins on phones too (the drawer).
      'Pin to Quick access',
      '|',
      'Copy name',
      '|',
      'Edit album',
      '|',
      "Don't show Daft Punk",
    ]);
    await user.click(within(menu).getByRole('menuitem', { name: 'Add to queue' }));
    await vi.waitFor(() =>
      expect(commands.playbackQueueTracks).toHaveBeenCalledWith(
        [expect.objectContaining({ title: 'One' }), expect.objectContaining({ title: 'Two' })],
        false,
      ),
    );
    expect(await screen.findByTestId('toast')).toHaveTextContent('Added 2 songs to the queue');

    fireEvent.contextMenu(tile, { clientX: 30, clientY: 30 });
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Add to playlist' }),
    );
    const sheet = await screen.findByRole('dialog', { name: 'Add to playlist' });
    await user.click(within(sheet).getByRole('button', { name: /Road trip/ }));
    await vi.waitFor(() =>
      expect(commands.playlistAdd).toHaveBeenCalledWith(
        'p1',
        [expect.objectContaining({ title: 'One' }), expect.objectContaining({ title: 'Two' })],
        false,
      ),
    );
    expect(await screen.findByTestId('toast')).toHaveTextContent('Added 2 songs to Road trip');
  });

  it("opens an artist row's menu from ⋮, to follow, edit or hide them", async () => {
    const user = userEvent.setup();
    commands.onlineSearch.mockResolvedValue(ok({ library: [], top: [], providers: [] }));
    commands.musicSearchArtists.mockResolvedValue(
      ok([{ provider: TUNES, id: '27', name: 'Daft Punk', imageUrl: null }]),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=artists&q=daft');
    await user.click(await screen.findByRole('button', { name: 'More for Daft Punk' }));
    const menu = await screen.findByRole('menu');
    expect(lines(menu)).toEqual([
      '# Daft Punk',
      'Play',
      'Shuffle',
      'Play next',
      'Add to queue',
      '|',
      'Follow',
      '|',
      'Copy name',
      '|',
      'Edit artist',
      '|',
      "Don't show Daft Punk",
    ]);
    await user.click(within(menu).getByRole('menuitem', { name: 'Follow' }));
    expect(commands.artistFollow).toHaveBeenCalledWith('Daft Punk', null);

    await user.click(screen.getByRole('button', { name: 'More for Daft Punk' }));
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Edit artist' }),
    );
    expect(await screen.findByRole('dialog', { name: 'Edit artist' })).toBeInTheDocument();
  });

  it('comes up as a sheet on a touch screen, with only its own options', async () => {
    touchScreen(true);
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [{ provider: TUNES, tracks: [song('Get Lucky', 'Daft Punk')], error: null }],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&view=songs&q=get+lucky');
    fireEvent.contextMenu(await screen.findByTestId('online-track-row'), {
      clientX: 30,
      clientY: 30,
    });
    const menu = await screen.findByRole('menu');
    expect(menu).toHaveAttribute('data-sheet');
    expect(within(menu).getByText('Get Lucky')).toBeInTheDocument();
    expect(within(menu).getByText('Daft Punk')).toBeInTheDocument();
    expect(within(menu).queryByRole('menuitem', { name: /^Back/ })).not.toBeInTheDocument();
    // A tap beside it closes it.
    fireEvent.pointerDown(menu.parentElement as HTMLElement);
    fireEvent.click(menu.parentElement as HTMLElement);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it("gives Your music's tiles their kind's menu, and Remove from Your music", async () => {
    const user = userEvent.setup();
    const album: PlayContext = {
      kind: 'album',
      name: 'Random Access Memories',
      subtitle: 'Daft Punk',
      artworkUrl: null,
      to: '/music/album',
      search: { provider: TUNES.id, id: ALBUM.id },
      playedAt: 1,
    };
    commands.recentContexts.mockResolvedValue(ok([album]));
    await AppShellWrapper.mount('/');
    const tile = await screen.findByRole('link', { name: /Random Access Memories/ });
    fireEvent.contextMenu(tile, { clientX: 30, clientY: 30 });
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: 'Shuffle' })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Remove from Your music' }));
    expect(commands.hiddenItemSet).toHaveBeenCalledWith(
      expect.objectContaining({
        kind: 'album',
        key: `/music/album?id=${ALBUM.id}&provider=${TUNES.id}`,
      }),
      true,
    );
    await vi.waitFor(() =>
      expect(screen.queryByRole('link', { name: /Random Access Memories/ })).toBeNull(),
    );
  });

  it("orders a playlist's menu the same way, with Delete last", async () => {
    commands.playlistsList.mockResolvedValue(
      ok([{ id: 'p1', name: 'Road trip', itemCount: 3, artworkUrls: [] }]),
    );
    await AppShellWrapper.mount('/library?view=playlists');
    fireEvent.contextMenu(await screen.findByRole('link', { name: /Road trip/ }), {
      clientX: 30,
      clientY: 30,
    });
    const own = ownLines(await screen.findByRole('menu'));
    expect(own.slice(0, 5)).toEqual([
      '# Road trip',
      'Play',
      'Shuffle',
      'Play next',
      'Add to queue',
    ]);
    expect(own).toContain('Download');
    expect(own.at(-1)).toBe('Delete playlist');
  });
});
