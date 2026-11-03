import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import { bindingsMock, CONNECT_IDLE, emitEvent, IDLE_STATE } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const QUEUE = [makeItem('Intro', 'Raygun'), makeItem('Second')];

const PC = {
  id: 'pc',
  name: 'Living room PC',
  address: '10.0.2.2:8820',
  kind: 'computer',
  paired: true,
  found: true,
  syncId: null,
  lastSyncAt: null,
};

/** Playing on the PC, which plays "Over There". */
const SESSION = {
  ...CONNECT_IDLE,
  session: {
    deviceId: 'pc',
    deviceName: 'Living room PC',
    deviceKind: 'computer',
    state: { ...IDLE_STATE, status: 'playing' as const, currentItemId: 'remote-1' },
    queue: {
      items: [{ ...makeItem('Over There', 'Far Away'), id: 'remote-1' }],
      order: [0],
    },
  },
};

describe('Connect', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('plays on another device, then shows and controls it from here', async () => {
    const user = userEvent.setup();
    commands.connectDevices.mockResolvedValue(
      ok([PC, { ...PC, id: 'tablet', name: 'Tablet', kind: 'phone', paired: false, found: false }]),
    );
    commands.connectTo.mockResolvedValue(ok(SESSION));
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;
    const bar = AppShellWrapper.miniPlayer.element;

    await user.click(await within(bar).findByRole('button', { name: 'Connect to a device' }));
    const sheet = await screen.findByTestId('connect-sheet');
    expect(within(sheet).getByText('Pixel')).toBeInTheDocument();
    expect(await within(sheet).findByText('Ready')).toBeInTheDocument();
    expect(within(sheet).getByText('Not seen on the network')).toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Play on Living room PC' }));
    expect(commands.connectTo).toHaveBeenCalledWith('pc');

    // The bar now shows the PC's player, and says where it plays.
    expect(await screen.findByTestId('playing-on')).toHaveTextContent('Playing on Living room PC');
    expect(await AppShellWrapper.miniPlayer.title).toHaveTextContent('Over There');
    await AppShellWrapper.miniPlayer.togglePlayback('Pause');
    expect(commands.playbackControl).toHaveBeenCalledWith({ type: 'toggle' });

    // Its updates arrive as events.
    emitEvent('connectEvent', {
      ...SESSION,
      session: { ...SESSION.session, state: { ...SESSION.session.state, status: 'paused' } },
    });
    expect(await within(bar).findByRole('button', { name: 'Play' })).toBeInTheDocument();
  });

  it('brings playback back here, or leaves it playing there', async () => {
    const user = userEvent.setup();
    commands.connectStatus.mockResolvedValue(SESSION);
    commands.connectDevices.mockResolvedValue(ok([PC]));
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;

    await user.click(
      await within(AppShellWrapper.miniPlayer.element).findByRole('button', {
        name: 'Connect to a device',
      }),
    );
    const sheet = await screen.findByTestId('connect-sheet');
    expect(await within(sheet).findByText('Playing')).toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: 'Play on Living room PC' })).toBeDisabled();
    await user.click(within(sheet).getByRole('button', { name: /Play here/ }));
    expect(commands.connectDisconnect).toHaveBeenCalledWith(true);
  });

  it('adds a device by its address', async () => {
    const user = userEvent.setup();
    commands.connectAdd.mockResolvedValue(ok(PC));
    await AppShellWrapper.mount();
    AppShellWrapper.playing(QUEUE, 0);
    await AppShellWrapper.miniPlayer.title;

    await user.click(
      await within(AppShellWrapper.miniPlayer.element).findByRole('button', {
        name: 'Connect to a device',
      }),
    );
    const sheet = await screen.findByTestId('connect-sheet');
    expect(await within(sheet).findByText(/No other devices yet/)).toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Add a device by address' }));
    await user.type(within(sheet).getByRole('textbox'), '10.0.2.2:8820');
    await user.click(within(sheet).getByRole('button', { name: 'Add' }));
    expect(commands.connectAdd).toHaveBeenCalledWith('10.0.2.2:8820');
  });

  it('asks before another device first plays here', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount();

    emitEvent('connectRequestEvent', { id: 'q1', name: 'Pixel 4', open: true });
    const prompt = await screen.findByTestId('connect-prompt');
    expect(prompt).toHaveTextContent('Link with Pixel 4?');
    await user.click(within(prompt).getByRole('button', { name: 'Link' }));
    expect(commands.connectAnswer).toHaveBeenCalledWith('q1', true);
    expect(screen.queryByTestId('connect-prompt')).not.toBeInTheDocument();

    // A question that times out closes by itself.
    emitEvent('connectRequestEvent', { id: 'q2', name: 'Pixel 4', open: true });
    expect(await screen.findByTestId('connect-prompt')).toBeInTheDocument();
    emitEvent('connectRequestEvent', { id: 'q2', name: 'Pixel 4', open: false });
    expect(screen.queryByTestId('connect-prompt')).not.toBeInTheDocument();
  });
});
