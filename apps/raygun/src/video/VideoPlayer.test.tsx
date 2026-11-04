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
    // The ring says how many sources have answered.
    update({
      type: 'sources',
      groups: [
        { name: 'Alpha', state: 'ready', error: null, links: [] },
        { name: 'Slow', state: 'resolving', error: null, links: [] },
      ],
    });
    expect(await within(waiting).findByText('1 of 2 sources checked')).toBeInTheDocument();

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
    // Until it plays, the ring says which source this is, after one that failed.
    update({
      type: 'sources',
      groups: [
        {
          name: 'Alpha',
          state: 'ready',
          error: null,
          links: [
            { label: '1080p', current: false, failed: true },
            { label: '720p', current: true, failed: false },
          ],
        },
      ],
    });
    expect(
      await within(player).findByText('Trying source 2 of 2 · Alpha · 720p'),
    ).toBeInTheDocument();

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
    expect(notice).not.toHaveTextContent('DEMUXER_ERROR');
    expect(reports()).toContainEqual(
      expect.objectContaining({ state: 'error', error: 'DEMUXER_ERROR' }),
    );
    // The player's own words are under More info.
    await user.click(within(notice).getByRole('button', { name: 'More info' }));
    expect(notice).toHaveTextContent('DEMUXER_ERROR');

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

  it('turns on subtitles in your language when the audio is in another one', async () => {
    const user = userEvent.setup();
    const subtitles = [
      { url: 'http://127.0.0.1:1/subtitle/es', lang: 'spa', label: null, format: 'srt' },
      { url: 'http://127.0.0.1:1/subtitle/en', lang: 'English', label: null, format: 'srt' },
    ];
    await AppShellWrapper.mount('/');
    // An anime: its streams don't say they're Japanese.
    open({ ...MOVIE, titleState: null, subtitles, spokenLanguage: 'ja', subtitleMode: 'foreign' });
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = await within(player).findByTestId('desktop-video');
    Object.defineProperty(video, 'duration', { value: 600, configurable: true });
    fireEvent(video, new Event('loadedmetadata'));
    const button = within(player).getByRole('button', { name: 'Subtitles' });
    expect(button).toHaveAttribute('aria-pressed', 'true');
    await user.click(button);
    const panel = await within(player).findByRole('region', { name: 'Subtitles' });
    expect(within(panel).getByRole('menuitemradio', { name: /^English/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );

    // Audio in your language leaves them off.
    open({ ...MOVIE, titleState: null, subtitles, spokenLanguage: 'en', subtitleMode: 'foreign' });
    const again = await screen.findByTestId('desktop-video');
    Object.defineProperty(again, 'duration', { value: 600, configurable: true });
    fireEvent(again, new Event('loadedmetadata'));
    expect(
      within(screen.getByRole('dialog', { name: 'Video player' })).getByRole('button', {
        name: 'Subtitles',
      }),
    ).toHaveAttribute('aria-pressed', 'false');
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

  it('loops a video from the More menu without ending, moving on or finishing it', async () => {
    const user = userEvent.setup();
    const play = vi.spyOn(HTMLMediaElement.prototype, 'play');
    await AppShellWrapper.mount('/');
    open({ session: true, id: '', title: 'The Show', autoplay: true, nextPromptMs: 15_000 });
    update({
      type: 'episodes',
      items: [
        { label: 'S1E1 · Pilot', watched: false, downloaded: false, current: true },
        { label: 'S1E2 · Two', watched: false, downloaded: false, current: false },
      ],
      hasNext: true,
      hasPrevious: false,
      autoplay: true,
    });
    update({
      type: 'load',
      request: { ...MOVIE, id: 'ep1', title: 'The Show', titleState: null, startMs: 0 },
    });
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = (await within(player).findByTestId('desktop-video')) as HTMLVideoElement;
    Object.defineProperty(video, 'duration', { value: 100, configurable: true });
    fireEvent(video, new Event('loadedmetadata'));
    video.currentTime = 95;
    fireEvent(video, new Event('timeupdate'));
    expect(within(player).getByText(/Next episode/)).toBeInTheDocument();
    expect(video.loop).toBe(false);

    await user.click(within(player).getByRole('button', { name: 'More' }));
    const loop = screen.getByRole('menuitemcheckbox', { name: 'Loop this video' });
    expect(loop).toHaveAttribute('aria-checked', 'false');
    await user.click(loop);
    expect(loop).toHaveAttribute('aria-checked', 'true');
    // A file loops in the element itself: same decoder, no reload.
    expect(video.loop).toBe(true);
    expect(video).toHaveAttribute('src', MOVIE.url);
    // No next-episode prompt while it loops.
    expect(within(player).queryByText(/Next episode/)).not.toBeInTheDocument();

    // Going round: the start is saved as progress, marked as looping, never as a finish.
    commands.videoPlayerReport.mockClear();
    video.currentTime = 99.5;
    fireEvent(video, new Event('timeupdate'));
    video.currentTime = 0.1;
    fireEvent(video, new Event('timeupdate'));
    expect(reports()).toContainEqual(
      expect.objectContaining({ id: 'ep1', state: 'progress', looping: true }),
    );

    // A stream's end (Media Source) goes back to the start of the same source.
    play.mockClear();
    video.currentTime = 100;
    fireEvent(video, new Event('ended'));
    expect(video.currentTime).toBe(0);
    expect(play).toHaveBeenCalled();
    expect(video).toHaveAttribute('src', MOVIE.url);
    expect(reports()).not.toContainEqual(expect.objectContaining({ state: 'ended' }));
    expect(reports()).not.toContainEqual(expect.objectContaining({ action: 'next' }));

    // Off again: the end moves on as before.
    await user.click(loop);
    expect(loop).toHaveAttribute('aria-checked', 'false');
    expect(video.loop).toBe(false);
    fireEvent(video, new Event('ended'));
    expect(reports()).toContainEqual(expect.objectContaining({ state: 'ended', looping: false }));
    expect(reports()).toContainEqual(expect.objectContaining({ action: 'next' }));
  });

  it('forgets looping when the player closes', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemcheckbox', { name: 'Loop this video' }));
    await user.click(within(player).getByRole('button', { name: 'Close the player' }));

    open(MOVIE);
    const again = await screen.findByRole('dialog', { name: 'Video player' });
    const video = (await within(again).findByTestId('desktop-video')) as HTMLVideoElement;
    expect(video.loop).toBe(false);
    await user.click(within(again).getByRole('button', { name: 'More' }));
    expect(screen.getByRole('menuitemcheckbox', { name: 'Loop this video' })).toHaveAttribute(
      'aria-checked',
      'false',
    );
  });
});
