import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { useCoverSheetStore } from './coverSheetStore';
import { COVER_HOLD_MS } from './useCoverHold';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ART = 'https://img.example/600x600bb.jpg';
const SONG = { ...makeItem('Intro', 'Raygun'), artworkUrl: ART };

/** Real time passing, for a press held down. */
const wait = (ms: number) => new Promise((resolve) => setTimeout(resolve, ms));

/** The phone's full player with a song that has a cover. */
const openPlayer = async () => {
  await AppShellWrapper.mount();
  AppShellWrapper.playing([SONG, makeItem('Second')], 0);
  await AppShellWrapper.miniPlayer.open();
  const sheet = await AppShellWrapper.playerSheet.element;
  return { sheet, cover: within(sheet).getByTestId('player-artwork') };
};

describe('Holding a cover', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    // The sheet's store outlives each test.
    useCoverSheetStore.getState().close();
  });

  it('opens the save options once held for half a second, sinking while the hold fills', async () => {
    const user = userEvent.setup();
    const { cover } = await openPlayer();

    await user.pointer({ keys: '[TouchA>]', target: cover });
    // The cover shows the hold coming; nothing opens before it's long enough.
    expect(cover).toHaveAttribute('data-holding');
    await wait(COVER_HOLD_MS / 2);
    expect(screen.queryByTestId('cover-sheet')).not.toBeInTheDocument();
    const sheet = await screen.findByTestId('cover-sheet', undefined, {
      timeout: COVER_HOLD_MS * 3,
    });
    expect(cover).not.toHaveAttribute('data-holding');
    expect(bindingsMock.commands.coverVersions).toHaveBeenCalledWith(ART);
    expect(await within(sheet).findByRole('img', { name: 'Raygun - Intro' })).toBeInTheDocument();

    // Letting go after a hold is no tap: the lyrics don't open behind the sheet.
    await user.pointer({ keys: '[/TouchA]', target: cover });
    expect(screen.queryByTestId('lyrics-view')).not.toBeInTheDocument();
  });

  it('opens nothing on a short tap or on a press that scrolls', async () => {
    const user = userEvent.setup();
    const { cover } = await openPlayer();

    // A press that moves is a scroll: the hold is off.
    await user.pointer([
      { keys: '[TouchA>]', target: cover, coords: { clientX: 40, clientY: 40 } },
      { pointerName: 'TouchA', target: cover, coords: { clientX: 40, clientY: 90 } },
    ]);
    expect(cover).not.toHaveAttribute('data-holding');
    await wait(COVER_HOLD_MS + 200);
    expect(screen.queryByTestId('cover-sheet')).not.toBeInTheDocument();
    await user.pointer({ keys: '[/TouchA]', target: cover, coords: { clientX: 40, clientY: 90 } });

    // A tap does what the cover's tap does (here: the lyrics), never the save options.
    await user.click(cover);
    expect(await screen.findByTestId('lyrics-view')).toBeInTheDocument();
    await wait(COVER_HOLD_MS + 200);
    expect(screen.queryByTestId('cover-sheet')).not.toBeInTheDocument();
  });

  it('opens at once on a right-click', async () => {
    const user = userEvent.setup();
    const { cover } = await openPlayer();

    await user.pointer({ keys: '[MouseRight]', target: cover });

    expect(await screen.findByTestId('cover-sheet')).toBeInTheDocument();
    expect(screen.queryByTestId('lyrics-view')).not.toBeInTheDocument();
    // The next click is a click again.
    await user.click(screen.getByRole('button', { name: 'Close' }));
    await waitFor(() => expect(screen.queryByTestId('cover-sheet')).not.toBeInTheDocument());
    await user.click(cover);
    expect(await screen.findByTestId('lyrics-view')).toBeInTheDocument();
  });
});
