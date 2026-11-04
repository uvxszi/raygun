import { cleanup, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, DEFAULT_SETTINGS, bindingsMock, emitEvent } from '../test/bindingsMock';
import { useVideoPlayerStore } from '../video/videoPlayerStore';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const onDesktop = () =>
  commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });

const MOVIE = {
  id: 'movie',
  url: 'http://127.0.0.1:1/media/t/https/cdn.example/movie.mp4',
  format: 'mp4',
  title: 'The Movie',
  subtitle: null,
  subtitles: [],
  startMs: 0,
  seekStepMs: 10_000,
  group: 'movie',
  titleState: null,
  chapters: [],
};

describe('Music player and Video player settings', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.settingsGet.mockResolvedValue(ok(DEFAULT_SETTINGS));
  });

  it('lists the two players apart, without the old Player and Sound entries', async () => {
    await AppShellWrapper.mount('/settings');
    const settings = await AppShellWrapper.page('settings');
    expect(within(settings).getByRole('link', { name: /^Music player/ })).toBeVisible();
    expect(within(settings).getByRole('link', { name: /^Video player/ })).toBeVisible();
    expect(within(settings).getByRole('link', { name: /^Reader/ })).toBeVisible();
    expect(within(settings).queryByRole('link', { name: /^Player\b/ })).not.toBeInTheDocument();
    expect(within(settings).queryByRole('link', { name: /^Sound\b/ })).not.toBeInTheDocument();
  });

  it('keeps music settings on Music player and video ones on Video player', async () => {
    await AppShellWrapper.mount('/settings/music');
    expect(await screen.findByRole('button', { name: /Mobile data/ })).toBeInTheDocument();
    expect(screen.getByRole('region', { name: 'Equalizer' })).toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: 'Resume videos' })).not.toBeInTheDocument();

    cleanup();
    await AppShellWrapper.mount('/settings/video');
    expect(await screen.findByRole('switch', { name: 'Resume videos' })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /^Subtitles/ })).toHaveAttribute(
      'href',
      '/settings/video/subtitles',
    );
    expect(screen.queryByRole('button', { name: /Mobile data/ })).not.toBeInTheDocument();
  });

  it('sends old links to the new screens', async () => {
    await AppShellWrapper.mount('/settings/player/subtitles');
    expect(await screen.findByTestId('player-settings-subtitles')).toBeInTheDocument();

    cleanup();
    await AppShellWrapper.mount('/settings/sound');
    expect(await screen.findByTestId('settings-music-page')).toBeInTheDocument();

    cleanup();
    await AppShellWrapper.mount('/settings/player');
    expect(await screen.findByTestId('settings-music-page')).toBeInTheDocument();
  });

  it('finds settings under the player they belong to', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings');
    await user.click(await screen.findByRole('button', { name: 'Search settings' }));
    await user.type(screen.getByRole('searchbox', { name: 'Search settings' }), 'crossfade');
    expect(await screen.findByRole('link', { name: /Crossfade/ })).toHaveTextContent(
      'Music player',
    );

    await user.clear(screen.getByRole('searchbox', { name: 'Search settings' }));
    await user.type(screen.getByRole('searchbox', { name: 'Search settings' }), 'skip button');
    const found = await screen.findByRole('link', { name: /skip button/i });
    expect(found).toHaveTextContent('Video player');
    await user.click(found);
    expect(await screen.findByTestId('player-settings-skip')).toBeInTheDocument();
  });

  it('opens Music player settings as a sheet from the full player', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount();
    AppShellWrapper.playing([makeItem('Intro')], 0);
    await AppShellWrapper.miniPlayer.open();
    const player = await AppShellWrapper.playerSheet.element;

    await user.click(within(player).getByRole('button', { name: 'Player settings' }));
    const sheet = await screen.findByRole('dialog', { name: 'Music player' });
    expect(within(sheet).getByTestId('music-settings-sheet')).toBeInTheDocument();
    await user.click(within(sheet).getByRole('switch', { name: /Keep music playing/ }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'keepMusicPlaying', value: true });
    // Still in the player.
    expect(player).toBeInTheDocument();
  });

  it('opens Video player settings over the desktop player without closing the video', async () => {
    const user = userEvent.setup();
    onDesktop();
    await AppShellWrapper.mount('/');
    emitEvent('videoPlayerEvent', { kind: 'open', json: JSON.stringify(MOVIE) });
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = await within(player).findByTestId('desktop-video');

    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemradio', { name: 'Player settings' }));
    const panel = await within(player).findByTestId('video-settings-panel');
    expect(panel).toHaveAttribute('aria-label', 'Video player');

    // A screen opens in the panel, and Back returns to the list.
    await user.click(within(panel).getByRole('button', { name: /^Controls/ }));
    expect(panel).toHaveAttribute('aria-label', 'Controls');
    await user.click(within(panel).getByRole('switch', { name: /time left/i }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'videoPlayer',
      value: expect.objectContaining({ showRemaining: true }),
    });
    await user.click(within(panel).getByRole('button', { name: 'Back' }));
    expect(within(panel).getByRole('switch', { name: 'Resume videos' })).toBeInTheDocument();

    // The video stays where it was, and no settings page opened behind it.
    expect(video).toBeInTheDocument();
    expect(useVideoPlayerStore.getState().open).toBe(true);
    expect(screen.queryByTestId('settings-video-page')).not.toBeInTheDocument();
    await user.click(within(panel).getByRole('button', { name: 'Close' }));
    expect(within(player).queryByTestId('video-settings-panel')).not.toBeInTheDocument();
  });
});
