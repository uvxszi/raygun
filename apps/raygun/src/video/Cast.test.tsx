import type { CastDevice, CastStatus } from '@raygun/bindings';
import { act, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

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
  titleState: {},
  chapters: [],
};

const TV: CastDevice = {
  id: 'googlecast:1',
  name: 'Living Room',
  kind: 'googleCast',
  model: 'Chromecast Ultra',
  address: '192.168.1.30:8009',
  added: false,
};

const status = (change: Partial<CastStatus> = {}): CastStatus => ({
  device: TV,
  key: MOVIE.id,
  title: MOVIE.title,
  subtitle: null,
  state: 'playing',
  positionMs: 61_000,
  durationMs: 600_000,
  volume: 0.4,
  muted: false,
  atMs: Date.now(),
  error: null,
  ...change,
});

const openPlayer = async (extra: Record<string, unknown> = {}) => {
  await AppShellWrapper.mount('/');
  emitEvent('videoPlayerEvent', { kind: 'open', json: JSON.stringify({ ...MOVIE, ...extra }) });
  return screen.findByRole('dialog', { name: 'Video player' });
};

const openCastPanel = async (user: ReturnType<typeof userEvent.setup>) => {
  const player = await screen.findByRole('dialog', { name: 'Video player' });
  await user.click(within(player).getByRole('button', { name: 'More' }));
  await user.click(screen.getByRole('menuitemradio', { name: 'Play on TV' }));
  return screen.findByRole('region', { name: 'Play on TV' });
};

describe('Play on TV', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('lists the TVs found and plays on one from where the video is', async () => {
    const user = userEvent.setup();
    commands.castDevices.mockResolvedValue({
      devices: [TV, { ...TV, id: 'dlna:uuid:2', name: 'Bedroom TV', kind: 'dlna', model: null }],
      searching: true,
    });
    await openPlayer();
    const panel = await openCastPanel(user);

    expect(
      await within(panel).findByRole('menuitemradio', { name: /Living Room/ }),
    ).toHaveTextContent('Google Cast · Chromecast Ultra');
    expect(within(panel).getByRole('menuitemradio', { name: /Bedroom TV/ })).toHaveTextContent(
      'DLNA',
    );
    expect(within(panel).getByRole('status')).toHaveTextContent('Looking for TVs…');

    await user.click(within(panel).getByRole('menuitemradio', { name: /Living Room/ }));
    expect(commands.castStart).toHaveBeenCalledWith('googlecast:1', 61_000, null);
  });

  it('says when nothing answered, and adds a TV by its address', async () => {
    const user = userEvent.setup();
    const added: CastDevice = {
      ...TV,
      id: 'dlna:uuid:9',
      name: 'Kitchen TV',
      kind: 'dlna',
      added: true,
    };
    commands.castAddDevice.mockResolvedValue({ status: 'ok', data: added });
    await openPlayer();
    const panel = await openCastPanel(user);
    expect(await within(panel).findByText('No TVs found on this network')).toBeInTheDocument();

    await user.click(within(panel).getByRole('menuitemradio', { name: 'Add by address' }));
    await user.type(within(panel).getByRole('textbox', { name: "TV's address" }), '192.168.1.20');
    await user.click(within(panel).getByRole('button', { name: 'Add' }));
    expect(commands.castAddDevice).toHaveBeenCalledWith('192.168.1.20');
    await vi.waitFor(() =>
      expect(commands.castStart).toHaveBeenCalledWith('dlna:uuid:9', 61_000, null),
    );
  });

  it('shows why a TV could not be added', async () => {
    const user = userEvent.setup();
    commands.castAddDevice.mockResolvedValue({
      status: 'error',
      error: 'No TV answered at 10.0.0.9. Check the address and that the TV is on',
    });
    await openPlayer();
    const panel = await openCastPanel(user);
    await user.click(within(panel).getByRole('menuitemradio', { name: 'Add by address' }));
    await user.type(
      within(panel).getByRole('textbox', { name: "TV's address" }),
      '10.0.0.9{Enter}',
    );
    expect(await within(panel).findByRole('alert')).toHaveTextContent('No TV answered at 10.0.0.9');
    expect(commands.castStart).not.toHaveBeenCalled();
  });

  it('is the TV’s remote while it plays, and disconnecting plays here again', async () => {
    const user = userEvent.setup();
    await openPlayer();
    act(() => emitEvent('castStatusEvent', status()));

    const remote = await screen.findByRole('dialog', { name: 'Video player' });
    expect(within(remote).getByText('Playing on Living Room')).toBeInTheDocument();
    expect(within(remote).getByRole('status')).toHaveTextContent('Playing');
    expect(within(remote).queryByTestId('desktop-video')).not.toBeInTheDocument();

    await user.click(within(remote).getByRole('button', { name: 'Pause' }));
    expect(commands.castControl).toHaveBeenCalledWith({ type: 'pause' });
    await user.click(within(remote).getByRole('button', { name: 'Stop' }));
    expect(commands.castControl).toHaveBeenCalledWith({ type: 'stop' });
    await user.click(within(remote).getByRole('button', { name: 'Mute the TV' }));
    expect(commands.castControl).toHaveBeenCalledWith({ type: 'mute', muted: true });

    act(() => emitEvent('castStatusEvent', status({ state: 'paused' })));
    expect(within(remote).getByRole('status')).toHaveTextContent('Paused');
    await user.click(within(remote).getByRole('button', { name: 'Play' }));
    expect(commands.castControl).toHaveBeenCalledWith({ type: 'play' });

    await user.click(within(remote).getByRole('button', { name: 'Disconnect' }));
    expect(commands.castDisconnect).toHaveBeenCalledWith(true);
    act(() => emitEvent('castStatusEvent', null));
    expect(await screen.findByTestId('desktop-video')).toBeInTheDocument();
  });

  it('shows what went wrong on the TV', async () => {
    await openPlayer();
    act(() =>
      emitEvent(
        'castStatusEvent',
        status({ state: 'failed', error: "The TV couldn't play this video" }),
      ),
    );
    const remote = await screen.findByRole('dialog', { name: 'Video player' });
    expect(within(remote).getByRole('status')).toHaveTextContent("Couldn't play on the TV");
    expect(within(remote).getByRole('alert')).toHaveTextContent("The TV couldn't play this video");
  });

  it('moves on to the next episode when one finishes on the TV', async () => {
    await openPlayer({ session: true });
    act(() =>
      emitEvent('videoPlayerEvent', {
        kind: 'update',
        json: JSON.stringify({ type: 'episodes', items: [], hasNext: true, hasPrevious: false }),
      }),
    );
    act(() => emitEvent('castStatusEvent', status({ state: 'finished', positionMs: 600_000 })));
    await vi.waitFor(() =>
      expect(
        commands.videoPlayerReport.mock.calls.map(
          ([json]) => JSON.parse(json as string) as unknown,
        ),
      ).toContainEqual(expect.objectContaining({ state: 'request', action: 'next', id: MOVIE.id })),
    );
  });
});
