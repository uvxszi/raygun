import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import type * as ArtworkModule from '../library/Artwork';
import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { IDLE_STATE, bindingsMock, emitEvent } from '../test/bindingsMock';
import { isHandleTap, sheetDismissed } from './swipe';
import { markReceived, positionAt } from './useLivePosition';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

// Every cover drawn: the bar, the full player and the rows all draw one, so counting them shows
// whether a report re-rendered what holds them.
const covers = vi.hoisted(() => ({ drawn: 0 }));
vi.mock('../library/Artwork', async (original) => {
  const actual = await original<typeof ArtworkModule>();
  return {
    ...actual,
    Artwork: (props: Parameters<typeof actual.Artwork>[0]) => {
      covers.drawn += 1;
      return actual.Artwork(props);
    },
  };
});

const { commands } = bindingsMock;
const QUEUE = [makeItem('Intro', 'Raygun'), makeItem('Second', 'Other'), makeItem('Third')];

const at = (positionMs: number) =>
  emitEvent('playbackStateEvent', {
    ...IDLE_STATE,
    status: 'playing',
    currentItemId: 'id-Intro',
    positionMs,
    durationMs: 180_000,
  });

describe('The player as a song plays', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('re-renders only the seek bar for position reports, not the bar, sheet or queue', async () => {
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;
    await AppShellWrapper.playerSheet.press('Up next');
    await AppShellWrapper.playerSheet.upNext;
    await waitFor(() => expect(within(sheet).getByTestId('position')).toHaveTextContent('1:05'));
    const before = covers.drawn;

    for (const second of [66, 67, 68, 69, 70]) at(second * 1000);

    await waitFor(() => expect(within(sheet).getByTestId('position')).toHaveTextContent('1:10'));
    expect(covers.drawn).toBe(before);
  });

  it('shows the bar artist as text, with Go to the artist in the menus', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;
    expect(within(bar).getByTestId('bar-artist')).toHaveTextContent('Raygun');
    expect(within(bar).queryByRole('link')).not.toBeInTheDocument();

    // On a phone the full player's artist is text too; More options leads to them.
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;
    expect(within(sheet).getByTestId('sheet-artist')).toHaveTextContent('Raygun');
    expect(within(sheet).queryByRole('link', { name: 'Raygun' })).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'More options' }));
    const more = await screen.findByTestId('track-more-actions');
    expect(await within(more).findByRole('button', { name: 'Go to Raygun' })).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Cancel' }));
  });

  it('closes from its handle with a tap', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;
    await user.click(screen.getByRole('button', { name: 'Close player' }));
    await AppShellWrapper.playerSheet.waitUntilClosed();
  });

  it('closes from its handle with Enter, and tells a tap from a drag', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    await AppShellWrapper.playerSheet.element;
    screen.getByRole('button', { name: 'Close player' }).focus();
    await user.keyboard('{Enter}');
    await AppShellWrapper.playerSheet.waitUntilClosed();

    const start = { x: 100, y: 10, at: 0 };
    expect(isHandleTap(start, { x: 102, y: 13, at: 150 })).toBe(true);
    // A short drag that snaps back, or a long press, isn't a tap.
    expect(isHandleTap(start, { x: 100, y: 60, at: 300 })).toBe(false);
    expect(isHandleTap(start, { x: 100, y: 10, at: 900 })).toBe(false);
    expect(sheetDismissed({ y: 200 }, { y: 0 })).toBe(true);
    expect(sheetDismissed({ y: 40 }, { y: 100 })).toBe(false);
  });

  it('lays a phone on its side out with the cover beside the controls', async () => {
    // A short, wide window: only the landscape query matches.
    window.matchMedia = vi.fn().mockImplementation((query: string) => ({
      matches: query.includes('orientation: landscape'),
      media: query,
      addEventListener: vi.fn(),
      removeEventListener: vi.fn(),
    }));
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.open();
    const sheet = await AppShellWrapper.playerSheet.element;
    expect(sheet).toHaveAttribute('data-layout', 'landscape');
    // The lyrics and the queue are a tap away in its header.
    await userEvent.click(within(sheet).getByRole('button', { name: 'Up next' }));
    expect(await within(sheet).findByTestId('up-next')).toHaveTextContent('Second');
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('moves the playhead at the playback speed, and only for a moment past a report', () => {
    const state = { ...IDLE_STATE, status: 'playing' as const, positionMs: 10_000, rate: 2 };
    markReceived(state);
    const now = performance.now();
    expect(positionAt(state, now - 10)).toBe(10_000);
    expect(positionAt(state, now + 1000)).toBeCloseTo(12_000, -1);
    // A report this late means it isn't moving: the playhead waits.
    expect(positionAt(state, now + 60_000)).toBe(15_000);
    expect(positionAt({ ...state, status: 'paused' }, now + 1000)).toBe(10_000);
    expect(commands.playbackControl).not.toHaveBeenCalled();
  });
});
