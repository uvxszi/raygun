import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent, IDLE_STATE } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const QUEUE = [makeItem('Intro', 'Raygun'), makeItem('Second'), makeItem('Third')];

/** A window at least `md` wide, or a phone. */
const setWide = (wide: boolean) => {
  window.matchMedia = vi.fn().mockImplementation((query: string) => ({
    matches: wide,
    media: query,
    addEventListener: vi.fn(),
    removeEventListener: vi.fn(),
  }));
};

describe('Player bar', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  afterEach(() => {
    // jsdom has no matchMedia of its own.
    delete (window as { matchMedia?: unknown }).matchMedia;
  });

  it('has the favourite and next buttons on phones', async () => {
    setWide(false);
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    expect(within(bar).getByRole('button', { name: 'Add to favorites' })).toBeInTheDocument();
    expect(within(bar).queryByRole('button', { name: 'Shuffle' })).not.toBeInTheDocument();
    await user.click(within(bar).getByRole('button', { name: 'Next' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'next' });
  });

  it('has Spotify’s controls on wide windows', async () => {
    setWide(true);
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    expect(within(bar).getByTestId('bar-position')).toHaveTextContent('1:05');
    expect(within(bar).getByTestId('bar-duration')).toHaveTextContent('3:00');
    await user.click(within(bar).getByRole('button', { name: 'Shuffle' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'setShuffle', enabled: true });
    await user.click(within(bar).getByRole('button', { name: 'Previous' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'previous' });
    await user.click(within(bar).getByRole('button', { name: /^Repeat/ }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'setRepeat', mode: 'all' });

    // Seeking commits on release.
    const seek = within(bar).getByRole('slider', { name: 'Seek' });
    fireEvent.change(seek, { target: { value: '90000' } });
    fireEvent.pointerUp(seek);
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'seek', positionMs: 90_000 });

    // Volume: the slider, then mute and back to where it was.
    fireEvent.change(within(bar).getByRole('slider', { name: 'Volume' }), {
      target: { value: '0.4' },
    });
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'setVolume', volume: 0.4 });
    emitEvent('playbackStateEvent', {
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: 'id-Intro',
      positionMs: 65_000,
      durationMs: 180_000,
      volume: 0.4,
    });
    // Rust's state reaches the bar a moment later.
    await vi.waitFor(() =>
      expect(within(bar).getByRole('slider', { name: 'Volume' })).toHaveValue('0.4'),
    );
    await user.click(within(bar).getByRole('button', { name: 'Mute' }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'setVolume', volume: 0 });
    emitEvent('playbackStateEvent', {
      ...IDLE_STATE,
      status: 'playing',
      currentItemId: 'id-Intro',
      volume: 0,
    });
    await user.click(await within(bar).findByRole('button', { name: 'Unmute' }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'setVolume', volume: 0.4 });

    // Lyrics open the full player on its lyrics.
    await user.click(within(bar).getByRole('button', { name: 'Lyrics' }));
    expect(await screen.findByTestId('lyrics-view')).toBeInTheDocument();
  });

  it('shows the queue beside the bar, to play, remove and clear', async () => {
    setWide(true);
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;

    await user.click(
      within(AppShellWrapper.miniPlayer.element).getByRole('button', { name: 'Queue' }),
    );
    const queue = screen.getByRole('complementary', { name: 'Queue' });
    expect(queue).toHaveTextContent('Intro');
    const next = within(queue).getByTestId('queue-panel-list');
    expect(next).toHaveTextContent('Second');
    expect(next).toHaveTextContent('Third');

    await user.click(within(next).getByRole('button', { name: 'Third' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'playItem', itemId: 'id-Third' });
    await user.click(within(next).getByRole('button', { name: 'Remove Second from the queue' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'remove', itemId: 'id-Second' });
    await user.click(within(queue).getByRole('button', { name: 'Clear' }));
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'remove', itemId: 'id-Third' });

    await user.click(within(queue).getByRole('button', { name: 'Close the queue' }));
    expect(screen.queryByRole('complementary', { name: 'Queue' })).not.toBeInTheDocument();
  });
});
