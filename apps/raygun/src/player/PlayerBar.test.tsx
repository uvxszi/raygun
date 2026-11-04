import { fireEvent, screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { swipeAction } from './swipe';
import { ANDROID_INFO, bindingsMock, emitEvent, IDLE_STATE } from '../test/bindingsMock';

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

  it('has Connect and the favourite button on phones; swipes change the song', async () => {
    setWide(false);
    const user = userEvent.setup();
    commands.syncStatus.mockResolvedValue({
      status: 'ok',
      data: {
        enabled: true,
        deviceName: 'Pixel',
        fingerprint: 'ab12',
        pairingText: null,
        pairingQrSvg: null,
        kinds: ['library'],
        peers: [
          {
            deviceId: 'pc-1',
            name: 'Desktop PC',
            fingerprint: 'cd34',
            address: null,
            pairedAt: 0,
            lastSyncAt: Date.now() - 5 * 60_000,
          },
        ],
        log: [],
        error: null,
      },
    });
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 1);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    expect(
      within(bar).getByRole('button', { name: 'Save to Favorites or a playlist' }),
    ).toBeInTheDocument();
    expect(within(bar).queryByRole('button', { name: 'Shuffle' })).not.toBeInTheDocument();
    // Swiping replaces the Next button (M46.13).
    expect(within(bar).queryByRole('button', { name: 'Next' })).not.toBeInTheDocument();
    expect(swipeAction({ x: -90, y: 8 }, { x: 0, y: 0 })).toBe('next');
    expect(swipeAction({ x: 20, y: 2 }, { x: 600, y: 0 })).toBe('previous');
    expect(swipeAction({ x: 4, y: -60 }, { x: 0, y: 0 })).toBe('open');
    expect(swipeAction({ x: 4, y: 60 }, { x: 0, y: 0 })).toBe('close');
    expect(swipeAction({ x: 2, y: 12 }, { x: 0, y: 500 })).toBe('close');
    expect(swipeAction({ x: 30, y: 5 }, { x: 50, y: 0 })).toBeNull();

    await user.click(within(bar).getByRole('button', { name: 'Connect to a device' }));
    const sheet = await screen.findByTestId('connect-sheet');
    expect(within(sheet).getByText('Pixel')).toBeInTheDocument();
    // Pairing, syncing and the remote live in one place now (M46.43), a link away.
    expect(within(sheet).getByRole('link', { name: 'Manage devices' })).toHaveAttribute(
      'href',
      '/settings/devices',
    );
  });

  it('opens the miniplayer from the desktop bar', async () => {
    setWide(true);
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    await user.click(await within(bar).findByRole('button', { name: 'Open the miniplayer' }));
    expect(commands.miniplayerOpen).toHaveBeenCalled();
    expect(within(bar).getByRole('button', { name: 'Connect to a device' })).toBeInTheDocument();
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

  it('shows the volume slider beside the mute button while the bar has no room for it', async () => {
    setWide(true);
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;
    const mute = within(bar).getByRole('button', { name: 'Mute' });
    expect(screen.queryByTestId('volume-popover')).not.toBeInTheDocument();

    // jsdom lays nothing out, so the bar's own slider counts as hidden (a narrow bar).
    fireEvent.pointerOver(mute, { pointerType: 'mouse' });
    const popover = await screen.findByTestId('volume-popover');
    const slider = within(popover).getByRole('slider', { name: 'Volume' });
    // From the button up to the slider, the pointer stays inside: still there.
    fireEvent.pointerOut(mute, { pointerType: 'mouse', relatedTarget: slider });
    fireEvent.pointerOver(slider, { pointerType: 'mouse', relatedTarget: mute });
    fireEvent.change(slider, { target: { value: '0.3' } });
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'setVolume', volume: 0.3 });
    expect(screen.getByTestId('volume-popover')).toBeInTheDocument();
    // Leaving both closes it.
    fireEvent.pointerOut(slider, { pointerType: 'mouse', relatedTarget: document.body });
    await waitFor(() => expect(screen.queryByTestId('volume-popover')).not.toBeInTheDocument());

    // A finger never opens it (a tap would only flash it).
    fireEvent.pointerOver(mute, { pointerType: 'touch' });
    expect(screen.queryByTestId('volume-popover')).not.toBeInTheDocument();
  });

  it('shows what plays beside the page: artwork, lyrics and what comes next', async () => {
    setWide(true);
    const user = userEvent.setup();
    commands.lyricsGet.mockResolvedValue({
      status: 'ok',
      data: {
        providerName: 'LRCLIB',
        plain: 'First line' + String.fromCharCode(10) + 'Second line',
        synced: [],
      },
    });
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    await user.click(within(bar).getByRole('button', { name: 'Now playing view' }));
    const panel = screen.getByRole('complementary', { name: 'Now playing' });
    expect(within(panel).getByTestId('side-panel-title')).toHaveTextContent('Intro');
    expect(await within(panel).findByText(/First line/)).toBeInTheDocument();
    const nextUp = within(panel).getByRole('region', { name: 'Next in queue' });
    expect(nextUp).toHaveTextContent('Second');

    // Its tabs switch to the queue; the bar's button closes it again.
    await user.click(within(panel).getByRole('button', { name: 'Open the queue' }));
    expect(screen.getByRole('complementary', { name: 'Queue' })).toBeInTheDocument();
    await user.click(within(bar).getByRole('button', { name: 'Queue' }));
    expect(screen.queryByRole('complementary')).not.toBeInTheDocument();
  });

  it('grows Next in queue by rows, shrinks an empty lyrics card and closes the player', async () => {
    setWide(true);
    const user = userEvent.setup();
    commands.lyricsGet.mockResolvedValue({ status: 'ok', data: null });
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    await user.click(within(bar).getByRole('button', { name: 'Now playing view' }));
    const panel = screen.getByRole('complementary', { name: 'Now playing' });
    // No lyrics: the card is a line, not 26 rem tall (M46.24).
    expect(await within(panel).findByText('No lyrics found for this track.')).toBeInTheDocument();
    expect(within(panel).getByTestId('lyrics-view').className).not.toContain('h-[26rem]');

    // One song by default; the handle's arrow keys show more, saved in the panels setting.
    const nextUp = within(panel).getByRole('region', { name: 'Next in queue' });
    expect(within(nextUp).getAllByRole('listitem')).toHaveLength(1);
    const handle = within(nextUp).getByRole('separator', {
      name: 'Drag to show more or fewer songs',
    });
    handle.focus();
    await user.keyboard('{ArrowDown}');
    // Settings save one after another, so this one may wait for an earlier save.
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
        key: 'panels',
        value: expect.objectContaining({ nextUpRows: 2 }),
      }),
    );

    // The × after the seek bar closes the player: the queue empties.
    await user.click(
      within(bar).getByRole('button', { name: 'Close the player and stop playing' }),
    );
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'clear' });
  });

  it('shows the cover large from the artwork and saves it as an image (M46.26)', async () => {
    setWide(true);
    const user = userEvent.setup();
    const withArt = {
      ...makeItem('Intro', 'Raygun'),
      artworkUrl: 'https://img.example/600x600bb.jpg',
    };
    await AppShellWrapper.mount();
    AppShellWrapper.playing([withArt, ...QUEUE.slice(1)], 0);
    await AppShellWrapper.miniPlayer.title;
    await user.click(
      within(AppShellWrapper.miniPlayer.element).getByRole('button', { name: 'Now playing view' }),
    );
    const panel = screen.getByRole('complementary', { name: 'Now playing' });
    await user.click(within(panel).getByRole('button', { name: 'Show the cover large' }));
    const sheet = await screen.findByTestId('cover-sheet');
    await user.click(within(sheet).getByRole('button', { name: 'PNG' }));
    await user.click(within(sheet).getByRole('button', { name: 'Save image' }));
    expect(commands.coverSave).toHaveBeenCalledWith(
      'https://img.example/600x600bb.jpg',
      'Raygun - Intro',
      'png',
      'pictures',
    );
    expect(await within(sheet).findByRole('status')).toHaveTextContent(
      'Saved to Pictures/Raygun/Cover.png',
    );
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
    // Clear asks first, then takes everything after the playing song out in one go.
    await user.click(within(queue).getByRole('button', { name: 'Clear' }));
    const ask = within(queue).getByRole('alertdialog', { name: 'Take 2 songs out of Up next?' });
    await user.click(within(ask).getByRole('button', { name: 'Clear' }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'clearUpcoming' });

    // The grip moves a song with the arrow keys.
    within(next).getByRole('button', { name: 'Move Third' }).focus();
    await user.keyboard('{ArrowUp}');
    expect(commands.playbackControl).toHaveBeenLastCalledWith({
      type: 'move',
      itemId: 'id-Third',
      beforeItemId: 'id-Second',
    });

    await user.click(within(queue).getByRole('button', { name: 'Close the side panel' }));
    expect(screen.queryByRole('complementary', { name: 'Queue' })).not.toBeInTheDocument();
  });
});
