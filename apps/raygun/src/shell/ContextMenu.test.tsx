import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { ANDROID_INFO, bindingsMock } from '../test/bindingsMock';
import { AppShellWrapper } from './AppShell.test-wrapper';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

describe('Right-click menu', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it("offers a song's options, then the page's, and works from the keyboard", async () => {
    const user = userEvent.setup();
    commands.playbackQueueTracks.mockResolvedValue(ok(null));
    commands.onlineSearch.mockResolvedValue(
      ok({
        library: [],
        top: [],
        providers: [
          {
            provider: { id: 'x/y', extensionId: 'x', kind: 'music.metadata', name: 'Source' },
            tracks: [
              {
                externalId: null,
                title: 'Get Lucky',
                artist: 'Daft Punk, Pharrell Williams',
                album: null,
                durationMs: 200_000,
                artworkUrl: null,
                source: null,
              },
            ],
            error: null,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=get+lucky');
    const row = await screen.findByTestId('online-track-row');

    fireEvent.contextMenu(row, { clientX: 40, clientY: 40 });
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByText('Get Lucky')).toBeInTheDocument();
    expect(within(menu).getByRole('menuitem', { name: 'Go to Pharrell Williams' })).toBeEnabled();
    // The page's own options follow the song's.
    expect(within(menu).getByRole('menuitem', { name: /^Back/ })).toBeInTheDocument();

    await user.click(within(menu).getByRole('menuitem', { name: 'Play next' }));
    expect(commands.playbackQueueTracks).toHaveBeenCalledWith(
      [expect.objectContaining({ title: 'Get Lucky' })],
      true,
    );
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    // Arrow keys and Enter choose; Escape closes.
    fireEvent.contextMenu(row, { clientX: 40, clientY: 40 });
    const again = await screen.findByRole('menu');
    fireEvent.keyDown(again, { key: 'ArrowDown' });
    fireEvent.keyDown(again, { key: 'ArrowDown' });
    fireEvent.keyDown(again, { key: 'Enter' });
    expect(commands.playbackQueueTracks).toHaveBeenLastCalledWith(
      [expect.objectContaining({ title: 'Get Lucky' })],
      true,
    );
    fireEvent.contextMenu(row, { clientX: 40, clientY: 40 });
    fireEvent.keyDown(await screen.findByRole('menu'), { key: 'Escape' });
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();
  });

  it('offers removing a downloaded song, going to its album, and one set of copy items (M46.36)', async () => {
    const user = userEvent.setup();
    commands.songDownloads.mockResolvedValue(
      ok([{ id: 'dl-1', trackKey: 'daft punk|get lucky', state: 'done' }]) as never,
    );
    commands.songDownloadFor.mockResolvedValue(ok({ id: 'dl-1' }) as never);
    commands.songDownloadRemove.mockResolvedValue(ok(null));
    commands.musicFindAlbum.mockResolvedValue(
      ok({
        provider: { id: 'x/y', extensionId: 'x', kind: 'music.metadata', name: 'Source' },
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
            provider: { id: 'x/y', extensionId: 'x', kind: 'music.metadata', name: 'Source' },
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
    const row = await screen.findByTestId('online-track-row');
    // The downloaded marks come from the one list of downloads, not a request per row.
    await vi.waitFor(() => expect(commands.songDownloads).toHaveBeenCalledTimes(1));

    fireEvent.contextMenu(row, { clientX: 40, clientY: 40 });
    const menu = await screen.findByRole('menu');
    expect(within(menu).queryByRole('menuitem', { name: 'Download' })).not.toBeInTheDocument();
    // Copy title and Copy artist, without "Copy “Get Lucky…”" for the words under the pointer.
    expect(within(menu).getByRole('menuitem', { name: 'Copy title' })).toBeInTheDocument();
    expect(within(menu).queryByRole('menuitem', { name: /^Copy “/ })).not.toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Remove from downloads' }));
    await vi.waitFor(() => expect(commands.songDownloadRemove).toHaveBeenCalledWith('dl-1'));

    fireEvent.contextMenu(row, { clientX: 40, clientY: 40 });
    await user.click(
      within(await screen.findByRole('menu')).getByRole('menuitem', { name: 'Go to album' }),
    );
    expect(commands.musicFindAlbum).toHaveBeenCalledWith('Random Access Memories', 'Daft Punk');
    await vi.waitFor(() => expect(commands.musicAlbum).toHaveBeenCalledWith('x/y', 'ram'));
  });

  it('replaces the webview menu everywhere, with the page options', async () => {
    // Full screen is a desktop window's.
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    await AppShellWrapper.mount('/');
    await screen.findByTestId('app-bar');
    const page = await screen.findByTestId('page-content');
    const shown = !fireEvent.contextMenu(page, { clientX: 10, clientY: 10 });
    expect(shown).toBe(true);
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: /^Full screen/ })).toBeInTheDocument();
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

    const openSongSheet = async () => {
      commands.playbackQueueTracks.mockResolvedValue(ok(null));
      commands.onlineSearch.mockResolvedValue(
        ok({
          library: [],
          top: [],
          providers: [
            {
              provider: { id: 'x/y', extensionId: 'x', kind: 'music.metadata', name: 'Source' },
              tracks: [
                {
                  externalId: null,
                  title: 'Get Lucky',
                  artist: 'Daft Punk',
                  album: null,
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
      fireEvent.contextMenu(await screen.findByTestId('online-track-row'), {
        clientX: 40,
        clientY: 40,
      });
      return screen.findByRole('menu');
    };

    it('keeps the cover and title above rows that scroll inside the sheet', async () => {
      const menu = await openSongSheet();
      expect(menu).toHaveAttribute('data-sheet');
      expect(menu.className).toContain('max-h-[85dvh]');
      const rows = within(menu).getByTestId('context-menu-rows');
      expect(rows.className).toContain('overflow-y-auto');
      expect(rows.className).toContain('env(safe-area-inset-bottom)');
      // The heading sits outside the scrolling rows; every option is inside them.
      expect(within(rows).queryByText('Get Lucky')).not.toBeInTheDocument();
      expect(within(menu).getByText('Get Lucky')).toBeInTheDocument();
      const play = within(rows).getByRole('menuitem', { name: 'Play next' });
      expect(play.className).toContain('min-h-[max(3rem,7dvh)]');
    });

    it('treats a scroll as a scroll: no item runs and the sheet stays open', async () => {
      const menu = await openSongSheet();
      const rows = within(menu).getByTestId('context-menu-rows');
      const play = within(rows).getByRole('menuitem', { name: 'Play next' });

      // A finger that moves over the list before lifting.
      fireEvent.pointerDown(play, { clientX: 100, clientY: 300 });
      fireEvent.pointerMove(play, { clientX: 100, clientY: 240 });
      fireEvent.click(play, { detail: 1 });
      expect(commands.playbackQueueTracks).not.toHaveBeenCalled();

      // The list scrolled under a still finger.
      fireEvent.pointerDown(play, { clientX: 100, clientY: 300 });
      fireEvent.scroll(rows);
      fireEvent.click(play, { detail: 1 });
      expect(commands.playbackQueueTracks).not.toHaveBeenCalled();

      // A drag that ends beside the sheet doesn't close it.
      fireEvent.pointerDown(play, { clientX: 100, clientY: 300 });
      fireEvent.click(menu.parentElement as HTMLElement, { detail: 1 });
      expect(screen.getByRole('menu')).toBeInTheDocument();

      // A plain tap chooses.
      fireEvent.pointerDown(play, { clientX: 100, clientY: 300 });
      fireEvent.click(play, { detail: 1 });
      expect(commands.playbackQueueTracks).toHaveBeenCalledWith(
        [expect.objectContaining({ title: 'Get Lucky' })],
        true,
      );
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
