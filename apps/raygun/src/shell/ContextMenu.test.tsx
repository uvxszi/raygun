import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock } from '../test/bindingsMock';
import { AppShellWrapper } from './AppShell.test-wrapper';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const TUNES = { id: 'x/y', extensionId: 'x', kind: 'music.metadata', name: 'Source' };

const searchArtists = async () => {
  commands.onlineSearch.mockResolvedValue(ok({ library: [], top: [], providers: [] }));
  commands.musicSearchArtists.mockResolvedValue(
    ok([{ provider: TUNES, id: '27', name: 'Daft Punk', imageUrl: null }]),
  );
  await AppShellWrapper.mount('/browse?tab=search&type=music&view=artists&q=daft');
  return screen.findByRole('button', { name: 'More for Daft Punk' });
};

describe('Item menus', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it("opens no menu on a right click or a long press; text fields keep the webview's own", async () => {
    await AppShellWrapper.mount('/');
    const page = await screen.findByTestId('page-content');
    // The webview's own menu (Back, Reload, Inspect…) stays away, and Raygun shows none.
    expect(fireEvent.contextMenu(page, { clientX: 10, clientY: 10 })).toBe(false);
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    const field = document.createElement('input');
    document.body.append(field);
    expect(fireEvent.contextMenu(field)).toBe(true);
    field.remove();
  });

  it('opens from ⋮ and works from the keyboard', async () => {
    const button = await searchArtists();
    fireEvent.click(button);
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByText('Daft Punk')).toBeInTheDocument();
    expect(within(menu).queryByRole('menuitem', { name: /^Back/ })).not.toBeInTheDocument();

    // Arrow keys and Enter choose; Escape closes.
    const items = within(menu).getAllByRole('menuitem');
    const follow = items.findIndex((item) => item.textContent === 'Follow');
    for (let step = 0; step <= follow; step += 1) fireEvent.keyDown(menu, { key: 'ArrowDown' });
    fireEvent.keyDown(menu, { key: 'Enter' });
    await vi.waitFor(() => expect(commands.artistFollow).toHaveBeenCalledWith('Daft Punk', null));
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    fireEvent.click(button);
    fireEvent.keyDown(await screen.findByRole('menu'), { key: 'Escape' });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it("offers a song's options from its ⋮: removing a download, its album, and copying (M46.36)", async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([{ id: 'dl-1', trackKey: 'daft punk|get lucky', state: 'done' }]) as never,
    );
    commands.songDownloadFor.mockResolvedValue(ok({ id: 'dl-1' }) as never);
    commands.songDownloadRemove.mockResolvedValue(ok(null));
    commands.musicFindAlbum.mockResolvedValue(
      ok({
        provider: TUNES,
        id: 'ram',
        title: 'Random Access Memories',
        artist: 'Daft Punk',
        year: 2013,
        artworkUrl: null,
        trackCount: 13,
      }),
    );
    commands.musicAlbum.mockReturnValue(new Promise(() => undefined));
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: TUNES,
            tracks: [
              {
                externalId: null,
                title: 'Get Lucky (Radio Edit)',
                artist: 'Daft Punk',
                album: 'Random Access Memories',
                durationMs: 200_000,
                artworkUrl: null,
                source: null,
                explicit: false,
              },
            ],
            error: null,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=get+lucky');
    await screen.findByTestId('online-track-row');
    // The downloaded marks come from the one list of downloads, not a request per row.
    await vi.waitFor(() => expect(commands.songDownloads).toHaveBeenCalledTimes(1));

    // The top result has one too; the song's row is last.
    const more = screen.getAllByRole('button', { name: 'More for Get Lucky (Radio Edit)' }).at(-1)!;
    await user.click(more);
    const actions = await screen.findByTestId('track-more-actions');
    expect(within(actions).queryByRole('button', { name: 'Download' })).not.toBeInTheDocument();
    expect(within(actions).getByRole('button', { name: 'Copy title' })).toBeInTheDocument();
    expect(within(actions).getByRole('button', { name: 'Copy artist' })).toBeInTheDocument();
    // It asks first, in the row's place; a second tap removes it.
    await user.click(within(actions).getByRole('button', { name: 'Remove from downloads' }));
    expect(commands.songDownloadRemove).not.toHaveBeenCalled();
    await user.click(within(actions).getByRole('button', { name: /^Delete the downloaded copy/ }));
    await vi.waitFor(() => expect(commands.songDownloadRemove).toHaveBeenCalledWith('dl-1'));

    await user.click(more);
    await user.click(
      within(await screen.findByTestId('track-more-actions')).getByRole('button', {
        name: 'Go to album',
      }),
    );
    expect(commands.musicFindAlbum).toHaveBeenCalledWith('Random Access Memories', 'Daft Punk');
    await vi.waitFor(() => expect(commands.musicAlbum).toHaveBeenCalledWith('x/y', 'ram'));
  });

  describe('on a touch screen', () => {
    const touchScreen = () => {
      window.matchMedia = ((query: string) => ({
        matches: query === '(pointer: coarse)',
        media: query,
        onchange: null,
        addEventListener: () => undefined,
        removeEventListener: () => undefined,
        addListener: () => undefined,
        removeListener: () => undefined,
        dispatchEvent: () => false,
      })) as unknown as typeof window.matchMedia;
    };
    beforeEach(touchScreen);
    afterEach(() => {
      delete (window as { matchMedia?: unknown }).matchMedia;
    });

    const openArtistSheet = async () => {
      fireEvent.click(await searchArtists());
      return screen.findByRole('menu');
    };

    it('keeps the cover and name above rows that scroll inside the sheet', async () => {
      const menu = await openArtistSheet();
      expect(menu).toHaveAttribute('data-sheet');
      expect(menu.className).toContain('max-h-[85dvh]');
      const rows = within(menu).getByTestId('context-menu-rows');
      expect(rows.className).toContain('overflow-y-auto');
      expect(rows.className).toContain('env(safe-area-inset-bottom)');
      // The heading sits outside the scrolling rows; every option is inside them.
      expect(within(rows).queryByText('Daft Punk')).not.toBeInTheDocument();
      expect(within(menu).getByText('Daft Punk')).toBeInTheDocument();
      const follow = within(rows).getByRole('menuitem', { name: 'Follow' });
      expect(follow.className).toContain('min-h-[max(3rem,7dvh)]');
    });

    it('treats a scroll as a scroll: no item runs and the sheet stays open', async () => {
      const menu = await openArtistSheet();
      const rows = within(menu).getByTestId('context-menu-rows');
      const follow = within(rows).getByRole('menuitem', { name: 'Follow' });

      // A finger that moves over the list before lifting.
      fireEvent.pointerDown(follow, { clientX: 100, clientY: 300 });
      fireEvent.pointerMove(follow, { clientX: 100, clientY: 240 });
      fireEvent.click(follow, { detail: 1 });
      expect(commands.artistFollow).not.toHaveBeenCalled();

      // The list scrolled under a still finger.
      fireEvent.pointerDown(follow, { clientX: 100, clientY: 300 });
      fireEvent.scroll(rows);
      fireEvent.click(follow, { detail: 1 });
      expect(commands.artistFollow).not.toHaveBeenCalled();

      // A drag that ends beside the sheet doesn't close it.
      fireEvent.pointerDown(follow, { clientX: 100, clientY: 300 });
      fireEvent.click(menu.parentElement as HTMLElement, { detail: 1 });
      expect(screen.getByRole('menu')).toBeInTheDocument();

      // A plain tap chooses.
      fireEvent.pointerDown(follow, { clientX: 100, clientY: 300 });
      fireEvent.click(follow, { detail: 1 });
      await vi.waitFor(() => expect(commands.artistFollow).toHaveBeenCalledWith('Daft Punk', null));
      expect(screen.queryByRole('menu')).not.toBeInTheDocument();
    });

    it("opens a top bar's ⋮ as the same sheet", async () => {
      const user = userEvent.setup();
      await AppShellWrapper.mount('/more');
      await user.click(
        within(await screen.findByTestId('more-page')).getByRole('button', {
          name: 'More options',
        }),
      );
      const menu = await screen.findByRole('menu');
      expect(menu).toHaveAttribute('data-sheet');
      expect(within(menu).getByRole('menuitem', { name: 'Edit tabs' })).toBeInTheDocument();
    });
  });
});
