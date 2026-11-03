import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

/** The JSON reports the player sent, parsed. */
const reports = () =>
  commands.videoPlayerReport.mock.calls.map(
    ([json]) => JSON.parse(json as string) as Record<string, unknown>,
  );

const open = (json: Record<string, unknown>) =>
  emitEvent('videoPlayerEvent', { kind: 'open', json: JSON.stringify(json) });
const update = (json: Record<string, unknown>) =>
  emitEvent('videoPlayerEvent', { kind: 'update', json: JSON.stringify(json) });

const MOVIE = {
  id: 'stremio:movie:tt1',
  url: 'http://127.0.0.1:1/media/t/https/cdn.example/movie.mp4',
  format: 'mp4',
  title: 'The Movie',
  subtitle: null,
  subtitles: [],
  startMs: 61_000,
  seekStepMs: 10_000,
  group: 'stremio:movie:tt1',
  titleState: { speed: 150 },
  chapters: [],
};

describe('desktop video player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('plays a video from where it was left and reports back', async () => {
    const user = userEvent.setup();
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play');
    await AppShellWrapper.mount('/');
    open(MOVIE);

    const player = await screen.findByRole('dialog', { name: 'Video player' });
    expect(within(player).getByRole('heading', { name: 'The Movie' })).toBeInTheDocument();
    const video = await within(player).findByTestId('desktop-video');
    await vi.waitFor(() => expect(video).toHaveAttribute('src', MOVIE.url));

    Object.defineProperty(video, 'duration', { value: 600, configurable: true });
    fireEvent(video, new Event('loadedmetadata'));
    expect((video as HTMLVideoElement).currentTime).toBe(61);
    // The title's remembered speed.
    expect((video as HTMLVideoElement).playbackRate).toBe(1.5);
    expect(play).toHaveBeenCalled();
    expect(within(player).getByRole('button', { name: 'Speed' })).toHaveTextContent('1.5×');

    // Pausing saves the position.
    fireEvent(video, new Event('pause'));
    expect(reports()).toContainEqual(
      expect.objectContaining({
        id: MOVIE.id,
        state: 'progress',
        positionMs: 61_000,
        durationMs: 600_000,
      }),
    );

    // Speed changes are remembered for the title.
    await user.click(within(player).getByRole('button', { name: 'Speed' }));
    await user.click(screen.getByRole('menuitemradio', { name: '2×' }));
    expect(reports()).toContainEqual(
      expect.objectContaining({
        state: 'request',
        action: 'state',
        group: MOVIE.group,
        change: { speed: 200 },
      }),
    );

    await user.click(within(player).getByRole('button', { name: 'Close the player' }));
    expect(screen.queryByRole('dialog', { name: 'Video player' })).not.toBeInTheDocument();
    expect(reports().at(-1)).toMatchObject({ id: MOVIE.id, state: 'closed' });
  });

  it('runs a session: episodes, the next one, and sources when nothing worked', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open({
      session: true,
      id: '',
      title: 'The Show',
      subtitle: 'S1E1',
      autoplay: true,
      nextPromptMs: 15_000,
    });

    const waiting = await screen.findByRole('dialog', { name: 'Video player' });
    expect(within(waiting).getByText('Finding a source…')).toBeInTheDocument();
    expect(reports()).toContainEqual(
      expect.objectContaining({ state: 'request', action: 'ready' }),
    );

    update({
      type: 'episodes',
      items: [
        { label: 'S1E1 · Pilot', watched: false, downloaded: false, current: true },
        { label: 'S1E2 · Two', watched: false, downloaded: true, current: false },
      ],
      hasNext: true,
      hasPrevious: false,
      autoplay: true,
    });
    update({
      type: 'load',
      request: {
        ...MOVIE,
        id: 'stremio:series:tt2:1:1',
        title: 'The Show',
        subtitle: 'S1E1 · Pilot',
        titleState: null,
        startMs: 0,
      },
    });
    // The first link replaces the spinner with the picture.
    const video = await screen.findByTestId('desktop-video');
    const player = screen.getByRole('dialog', { name: 'Video player' });

    await user.click(within(player).getByRole('button', { name: 'Episodes' }));
    const episodes = screen.getByRole('region', { name: 'Episodes' });
    expect(within(episodes).getByRole('menuitemradio', { name: /S1E1/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.click(within(episodes).getByRole('menuitemradio', { name: /S1E2/ }));
    expect(reports()).toContainEqual(
      expect.objectContaining({ id: 'stremio:series:tt2:1:1', action: 'episode', index: 1 }),
    );

    // The end of an episode moves on when autoplay is on.
    fireEvent(video, new Event('ended'));
    expect(reports()).toContainEqual(expect.objectContaining({ state: 'ended' }));
    expect(reports()).toContainEqual(expect.objectContaining({ action: 'next' }));

    update({
      type: 'error',
      text: 'No source could play this. Pick another in Sources.',
      showSources: true,
    });
    expect(await within(player).findByRole('alert')).toHaveTextContent('No source could play this');
    expect(screen.getByRole('region', { name: 'Sources' })).toBeInTheDocument();
  });

  it('types a subtitle delay, finds subtitles, makes AI ones and lists its keys', async () => {
    const user = userEvent.setup();
    commands.aiModels.mockResolvedValue([
      {
        id: 'whisper-tiny',
        name: 'Speech to text (Whisper tiny)',
        kind: 'speech',
        description: '',
        license: 'Apache-2.0',
        licenseUrl: '',
        sourceUrl: '',
        sizeBytes: 1,
        installed: true,
        downloading: null,
      },
    ]);
    commands.videoFindSubtitles.mockResolvedValue({
      status: 'ok',
      data: {
        tracks: [],
        suggestion: {
          url: 'https://subs.example/manifest.json',
          name: 'OpenSubtitles v3',
          description: '',
          added: false,
        },
        note: null,
      },
    });
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    await within(player).findByTestId('desktop-video');

    await user.click(within(player).getByRole('button', { name: 'Subtitles' }));
    const panel = await within(player).findByRole('region', { name: 'Subtitles' });
    // The delay is typed (M46.33).
    const delay = within(panel).getByRole('textbox', { name: 'Subtitle delay in seconds' });
    await user.clear(delay);
    await user.type(delay, '-1.25{Enter}');
    expect(reports()).toContainEqual(
      expect.objectContaining({ action: 'state', change: { subtitleDelayMs: -1250 } }),
    );

    // No addon has subtitles: OpenSubtitles is offered.
    await user.click(within(panel).getByRole('menuitemradio', { name: 'Look in your addons' }));
    expect(commands.videoFindSubtitles).toHaveBeenCalledWith(MOVIE.id);
    expect(
      await within(panel).findByRole('button', { name: 'Add OpenSubtitles v3' }),
    ).toBeInTheDocument();

    // AI subtitles from where the viewer is, with the chosen model.
    await user.click(within(panel).getByRole('button', { name: 'Make subtitles from here' }));
    expect(commands.captionsVideoTranscribe).toHaveBeenCalledWith(
      MOVIE.id,
      MOVIE.url,
      'mp4',
      expect.any(Number),
    );

    // ? lists the shortcuts.
    fireEvent.keyDown(window, { key: '?' });
    const keys = await within(player).findByRole('region', { name: 'Keyboard shortcuts' });
    expect(keys).toHaveTextContent('Subtitles on or off');
  });

  it('offers another player when the webview can’t play the format', async () => {
    const user = userEvent.setup();
    commands.videoExternalPlayers.mockResolvedValue([
      { id: 'mpv', name: 'mpv' },
      { id: 'vlc', name: 'VLC' },
    ]);
    await AppShellWrapper.mount('/');
    open({ ...MOVIE, titleState: null });
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = await within(player).findByTestId('desktop-video');
    await vi.waitFor(() => expect(video).toHaveAttribute('src', MOVIE.url));

    Object.defineProperty(video, 'error', {
      // MEDIA_ERR_SRC_NOT_SUPPORTED
      value: { code: 4, message: 'DEMUXER_ERROR' },
      configurable: true,
    });
    act(() => {
      video.dispatchEvent(new Event('error'));
    });
    const notice = await within(player).findByRole('alert');
    expect(notice).toHaveTextContent("This video's format can't play here");
    expect(reports()).toContainEqual(
      expect.objectContaining({ state: 'error', error: 'DEMUXER_ERROR' }),
    );

    // It asks which player (M46.33).
    await user.click(within(notice).getByRole('button', { name: 'Play in another player' }));
    const players = await within(player).findByRole('region', { name: 'Play in' });
    await user.click(await within(players).findByRole('menuitemradio', { name: 'VLC' }));
    expect(commands.videoPlayerExternal).toHaveBeenCalledWith(
      {
        url: MOVIE.url,
        title: 'The Movie',
        subtitles: [],
        startMs: 0,
      },
      'vlc',
    );
    expect(await within(player).findByRole('alert')).toHaveTextContent('Playing in mpv');
  });

  it('opens trailers in their own full-window player', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    emitEvent('youTubeVideoEvent', {
      videoId: 'abc123',
      title: 'Hazbin Hotel',
      subtitle: 'Trailer',
      embedUrl: 'http://localhost:1/embed/youtube?controls=1',
    });
    const trailer = await screen.findByRole('dialog', { name: 'Trailer · Hazbin Hotel' });
    expect(within(trailer).getByTitle('Trailer player')).toHaveAttribute(
      'src',
      'http://localhost:1/embed/youtube?controls=1',
    );
    await user.keyboard('{Escape}');
    expect(
      screen.queryByRole('dialog', { name: 'Trailer · Hazbin Hotel' }),
    ).not.toBeInTheDocument();
  });
});
