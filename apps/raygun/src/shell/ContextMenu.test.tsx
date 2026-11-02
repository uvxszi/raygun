import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { bindingsMock } from '../test/bindingsMock';
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

  it('replaces the webview menu everywhere, with the page options', async () => {
    await AppShellWrapper.mount('/');
    const page = await screen.findByTestId('page-content');
    const shown = !fireEvent.contextMenu(page, { clientX: 10, clientY: 10 });
    expect(shown).toBe(true);
    const menu = await screen.findByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: /^Full screen/ })).toBeInTheDocument();
  });
});
