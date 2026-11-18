import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../../test/bindingsMock';
import { fakeGl } from './fakeGl';
import { IN_APP_MOTION } from './motion/inApp';
import { MOTION_FILES } from './motionSources';

vi.mock('@raygun/bindings', async () => (await import('../../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const open = (json: Record<string, unknown>) =>
  emitEvent('videoPlayerEvent', { kind: 'open', json: JSON.stringify(json) });

const MOVIE = {
  id: 'stremio:movie:tt2',
  url: 'http://127.0.0.1:1/media/t/https/cdn.example/old-film.mp4',
  format: 'mp4',
  title: 'The Old Film',
  subtitle: null,
  subtitles: [],
  startMs: 0,
  group: 'stremio:movie:tt2',
  chapters: [],
};

const requests = () =>
  commands.videoPlayerReport.mock.calls
    .map(([json]) => JSON.parse(json as string) as Record<string, unknown>)
    .filter((report) => report.state === 'request');

describe('effects in the desktop player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it('names the effects button Fx', async () => {
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const button = within(player).getByRole('button', { name: 'Effects' });
    expect(button).toHaveTextContent('Fx');
    expect(button.querySelector('svg')).toBeNull();
  });

  it("draws a title's own effects over the picture, and changes them from More", async () => {
    const { gl, draws } = fakeGl();
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
      () => gl as unknown as RenderingContext,
    );
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open({ ...MOVIE, titleState: { pictureEffects: { deflicker: 40 } } });
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = (await within(player).findByTestId('desktop-video')) as HTMLVideoElement;
    // The canvas covers the picture with the same fit, and lets clicks through to the video.
    const canvas = within(player).getByTestId('video-effects');
    expect(canvas).toHaveClass('object-contain', 'pointer-events-none');

    Object.defineProperty(video, 'videoWidth', { value: 640, configurable: true });
    Object.defineProperty(video, 'videoHeight', { value: 360, configurable: true });
    Object.defineProperty(video, 'readyState', { value: 4, configurable: true });
    Object.defineProperty(video, 'paused', { value: true, configurable: true });

    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemradio', { name: /^Effects/ }));
    const panel = screen.getByRole('region', { name: 'Effects' });
    expect(within(panel).getByRole('slider', { name: 'Deflicker' })).toHaveValue('40');

    // Paused, the frame is drawn again with the new value at once.
    fireEvent.change(within(panel).getByRole('slider', { name: 'Sharpen' }), {
      target: { value: '50' },
    });
    await vi.waitFor(() =>
      expect(draws.some((draw) => draw.pass === 'final' && draw.uniforms.uSharpen === 0.8)).toBe(
        true,
      ),
    );
    await vi.waitFor(() =>
      expect(requests()).toContainEqual(
        expect.objectContaining({
          action: 'state',
          group: MOVIE.group,
          change: { pictureEffects: expect.objectContaining({ deflicker: 40, sharpen: 50 }) },
        }),
      ),
    );

    // All off: the canvas goes, the video shows as it is.
    await user.click(within(panel).getByRole('button', { name: 'Off' }));
    expect(within(player).queryByTestId('video-effects')).not.toBeInTheDocument();
  });

  it('draws the paused frame at once when the first effect is turned on', async () => {
    const { gl, draws } = fakeGl();
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
      () => gl as unknown as RenderingContext,
    );
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = (await within(player).findByTestId('desktop-video')) as HTMLVideoElement;
    Object.defineProperty(video, 'videoWidth', { value: 640, configurable: true });
    Object.defineProperty(video, 'videoHeight', { value: 360, configurable: true });
    Object.defineProperty(video, 'readyState', { value: 4, configurable: true });
    Object.defineProperty(video, 'paused', { value: true, configurable: true });
    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemradio', { name: /^Effects/ }));
    const panel = screen.getByRole('region', { name: 'Effects' });
    expect(within(player).queryByTestId('video-effects')).not.toBeInTheDocument();
    // Nothing was on: the effects start paused and the frame shows them without playing.
    await user.click(within(panel).getByRole('button', { name: 'Warm' }));
    expect(within(player).getByTestId('video-effects')).toBeInTheDocument();
    await vi.waitFor(() => expect(draws.some((draw) => draw.pass === 'final')).toBe(true));
    // Each further change draws it again, still paused.
    const before = draws.filter((draw) => draw.pass === 'final').length;
    fireEvent.change(within(panel).getByRole('slider', { name: 'Brightness' }), {
      target: { value: '20' },
    });
    await vi.waitFor(() =>
      expect(draws.filter((draw) => draw.pass === 'final').length).toBeGreaterThan(before),
    );
  });

  it('hides the player’s controls while a watermark box is drawn, until Done or Cancel', async () => {
    const { gl } = fakeGl();
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
      () => gl as unknown as RenderingContext,
    );
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemradio', { name: /^Effects/ }));
    const panel = screen.getByRole('region', { name: 'Effects' });
    await user.click(within(panel).getByRole('menuitemcheckbox', { name: /Remove a watermark/ }));
    await user.click(within(panel).getByRole('button', { name: 'Draw a box' }));
    const drawing = within(player).getByTestId('watermark-draw');
    expect(drawing).toBeInTheDocument();
    // Only the drawing and its Done and Cancel: the bars are hidden.
    const bars = within(player).getByRole('button', { name: 'Effects' }).closest('.duration-200');
    expect(bars).toHaveClass('opacity-0', 'pointer-events-none');
    await user.click(within(drawing).getByRole('button', { name: 'Cancel' }));
    expect(within(player).queryByTestId('watermark-draw')).not.toBeInTheDocument();
  });

  it('offers smooth motion only with its Smart Chip', async () => {
    const { gl } = fakeGl();
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
      () => gl as unknown as RenderingContext,
    );
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemradio', { name: /^Effects/ }));
    const panel = screen.getByRole('region', { name: 'Effects' });
    expect(within(panel).getByRole('button', { name: 'Cinema' })).toBeInTheDocument();
    expect(
      within(panel).queryByRole('menuitemcheckbox', { name: /Smooth motion/ }),
    ).not.toBeInTheDocument();
  });

  it('turns smooth motion on and smart audio from the Effects panel', async () => {
    // The Smooth Motion chip is installed and on: its shaders are read from it.
    commands.chipsList.mockResolvedValue([
      {
        id: 'smoothMotion',
        name: 'Smooth Motion',
        description: '',
        kind: 'feature',
        version: '1.0.0',
        size: 1,
        installed: true,
        outdated: false,
        on: true,
        progress: { kind: 'idle' },
      },
    ]);
    const byFile = Object.fromEntries(
      Object.entries(MOTION_FILES).map(([part, file]) => [
        file,
        IN_APP_MOTION[part as keyof typeof IN_APP_MOTION],
      ]),
    );
    commands.chipsReadText.mockImplementation(async (_id: string, name: string) => ({
      status: 'ok',
      data: byFile[name],
    }));
    const { gl, draws } = fakeGl();
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(
      () => gl as unknown as RenderingContext,
    );
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });
    const video = (await within(player).findByTestId('desktop-video')) as HTMLVideoElement;
    Object.defineProperty(video, 'videoWidth', { value: 640, configurable: true });
    Object.defineProperty(video, 'videoHeight', { value: 360, configurable: true });
    Object.defineProperty(video, 'readyState', { value: 4, configurable: true });
    Object.defineProperty(video, 'paused', { value: true, configurable: true });

    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemradio', { name: /^Effects/ }));
    const panel = screen.getByRole('region', { name: 'Effects' });
    await user.click(await within(panel).findByRole('menuitemcheckbox', { name: /Smooth motion/ }));
    // Paused, the frame is kept and shown as it is.
    await vi.waitFor(() => expect(draws.some((draw) => draw.pass === 'motionCopy')).toBe(true));
    await vi.waitFor(() =>
      expect(requests()).toContainEqual(
        expect.objectContaining({
          action: 'state',
          change: { pictureEffects: expect.objectContaining({ smoothMotion: true }) },
        }),
      ),
    );

    // A preset keeps smooth motion on.
    await user.click(within(panel).getByRole('button', { name: 'Cinema' }));
    expect(within(panel).getByRole('button', { name: 'Cinema' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(within(panel).getByRole('menuitemcheckbox', { name: /Smooth motion/ })).toHaveAttribute(
      'aria-checked',
      'true',
    );

    // Its models: Balanced unless changed; Sharp is kept for the title and draws again at once.
    expect(within(panel).getByRole('radio', { name: 'Balanced' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    const before = draws.length;
    await user.click(within(panel).getByRole('radio', { name: 'Sharp' }));
    await vi.waitFor(() =>
      expect(requests()).toContainEqual(
        expect.objectContaining({
          action: 'state',
          change: { pictureEffects: expect.objectContaining({ motionModel: 'sharp' }) },
        }),
      ),
    );
    expect(draws.length).toBeGreaterThan(before);

    // Smart audio is under Sound in the same panel.
    expect(within(panel).getByText('Sound')).toBeInTheDocument();
    await user.click(within(panel).getByRole('menuitemcheckbox', { name: /Night mode/ }));
    // (The preset is kept a moment after it's picked, so it may come after.)
    expect(requests()).toContainEqual(
      expect.objectContaining({
        action: 'state',
        change: { smartAudio: expect.objectContaining({ night: true }) },
      }),
    );
  });

  it('says when there is no WebGL 2, and keeps smart audio in the Audio panel', async () => {
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockImplementation(() => null);
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    open(MOVIE);
    const player = await screen.findByRole('dialog', { name: 'Video player' });

    await user.click(within(player).getByRole('button', { name: 'More' }));
    await user.click(screen.getByRole('menuitemradio', { name: /^Effects/ }));
    const panel = screen.getByRole('region', { name: 'Effects' });
    await user.click(within(panel).getByRole('button', { name: 'Warm' }));
    expect(await within(panel).findByText(/need WebGL 2/)).toBeInTheDocument();
    expect(within(player).queryByTestId('video-effects')).not.toBeInTheDocument();

    // A video with one track still has the Audio panel, for smart audio.
    await user.click(within(player).getByRole('button', { name: 'Audio' }));
    const audio = screen.getByRole('region', { name: 'Audio' });
    // Smart audio's switches sit at the top as small buttons; what each does is their tip.
    const dialogue = within(audio).getByRole('button', { name: 'Clearer dialogue' });
    expect(dialogue).toHaveTextContent('Dialogue');
    expect(dialogue).toHaveAttribute('title', 'Clearer dialogue: Voices brought forward');
    expect(dialogue).toHaveAttribute('aria-pressed', 'false');
    await user.click(dialogue);
    expect(requests().at(-1)).toMatchObject({
      action: 'state',
      change: { smartAudio: expect.objectContaining({ dialogue: true }) },
    });
    expect(within(audio).getByRole('button', { name: 'Clearer dialogue' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    // Add an audio file is a small button by the title.
    expect(within(audio).getByRole('button', { name: 'Add an audio file' })).toBeInTheDocument();
  });
});
