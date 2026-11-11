import type { PlayContext } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { resetBackLayers } from '../shell/backLayers';
import { usePlayerUiStore } from './playerUiStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const LONG = 'A Very Long Album Name That Goes On Well Past The Width Of Any Pill';

const ALBUM: PlayContext = {
  kind: 'album',
  name: LONG,
  subtitle: null,
  artworkUrl: null,
  to: '/music/album',
  search: {},
  playedAt: 0,
};

/** How many of the phone's history entries are the player's layers. */
const layerEntries = () => {
  const state: unknown = window.history.state;
  return typeof state === 'object' && state !== null && 'raygunLayer' in state;
};

describe('The full player and Back', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    // A phone: the system's Back goes through history.
    resetBackLayers(true);
  });

  it('closes the lyrics before the player, then the page stays', async () => {
    await AppShellWrapper.mount('/library');
    AppShellWrapper.playing([makeItem('Intro'), makeItem('Outro')], 0);
    await AppShellWrapper.miniPlayer.title;
    usePlayerUiStore.getState().openSheet({ view: 'lyrics' });
    await AppShellWrapper.playerSheet.element;
    expect(await screen.findByTestId('lyrics-view')).toBeInTheDocument();
    await waitFor(() => expect(layerEntries()).toBe(true));

    window.history.back();
    await waitFor(() => expect(screen.queryByTestId('lyrics-view')).not.toBeInTheDocument());
    // The cover again, in the same player.
    expect(await AppShellWrapper.playerSheet.element).toBeInTheDocument();
    expect(usePlayerUiStore.getState().sheetView).toBe('artwork');

    window.history.back();
    await AppShellWrapper.playerSheet.waitUntilClosed();
    expect(await AppShellWrapper.page('library')).toBeInTheDocument();
    await waitFor(() => expect(layerEntries()).toBe(false));
  });

  it('closes a sheet opened in the player before the player, on Escape too', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro')], 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.press('Player layout');
    expect(await screen.findByRole('dialog', { name: 'Player layout' })).toBeInTheDocument();

    await user.keyboard('{Escape}');
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Player layout' })).not.toBeInTheDocument(),
    );
    expect(await AppShellWrapper.playerSheet.element).toBeInTheDocument();

    await user.keyboard('{Escape}');
    await AppShellWrapper.playerSheet.waitUntilClosed();
  });

  it('keeps the handle and buttons below the status bar and the camera cutout', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro')], 0);
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;
    expect(sheet.className).toContain('pt-[calc(0.75rem+var(--safe-top))]');
  });

  it('slides long names in the Up next and Playing from pills instead of growing them', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro'), makeItem(LONG)], 0, 'playing', ALBUM);
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;

    const upNext = within(sheet).getByRole('button', { name: 'Up next' });
    expect(within(upNext).getByTestId('up-next-title')).toHaveTextContent(LONG);
    expect(upNext.className).toContain('max-w-[min(15rem,calc(50%-0.25rem))]');
    const from = within(sheet).getByRole('button', { name: `Playing from ${LONG}` });
    expect(within(from).getByTestId('playing-from-name')).toHaveTextContent(LONG);
    expect(from.className).toContain('max-w-[min(15rem,calc(50%-0.25rem))]');
  });
});
