import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../../test/bindingsMock';
import { fakeGl } from './fakeGl';

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

  it('turns smooth motion on and smart audio from the Effects panel', async () => {
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
    await user.click(within(panel).getByRole('menuitemcheckbox', { name: /Smooth motion/ }));
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

    // Smart audio is under Sound in the same panel.
    expect(within(panel).getByText('Sound')).toBeInTheDocument();
    await user.click(within(panel).getByRole('menuitemcheckbox', { name: /Night mode/ }));
    expect(requests().at(-1)).toMatchObject({
      action: 'state',
      change: { smartAudio: expect.objectContaining({ night: true }) },
    });
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
    await user.click(within(audio).getByRole('menuitemcheckbox', { name: /Clearer dialogue/ }));
    expect(requests().at(-1)).toMatchObject({
      action: 'state',
      change: { smartAudio: expect.objectContaining({ dialogue: true }) },
    });
  });
});
