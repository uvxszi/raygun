import type { DeviceFilesPeer, DeviceFilesStatus } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const PEER: DeviceFilesPeer = {
  deviceId: 'sync-laptop',
  name: 'Laptop',
  prefs: { kinds: ['songEdits'], wifiOnly: true },
  lastAt: null,
  delivered: 3,
  waiting: 0,
  note: null,
};

const LAPTOP: DeviceFilesStatus = { peers: [PEER], transfer: null };

describe('Files to linked devices', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.syncStatus.mockResolvedValue(
      ok({
        enabled: true,
        deviceName: 'Phone',
        fingerprint: '',
        pairingText: null,
        pairingQrSvg: null,
        kinds: ['library'],
        peers: [],
        log: [],
        error: null,
      }),
    );
    commands.deviceFilesStatus.mockResolvedValue(ok(LAPTOP));
  });

  it('chooses what goes to a device, sends it and follows the transfer', async () => {
    const user = userEvent.setup();
    const withSongs: DeviceFilesStatus = {
      ...LAPTOP,
      peers: [{ ...PEER, prefs: { kinds: ['songEdits', 'songs'], wifiOnly: true } }],
    };
    commands.deviceFilesSetPrefs.mockResolvedValue(ok(withSongs));
    commands.deviceFilesSendNow.mockResolvedValue(ok(withSongs));
    await AppShellWrapper.mount('/settings/devices');

    expect(await screen.findByText('Send to Laptop')).toBeInTheDocument();
    expect(screen.getByRole('switch', { name: 'Song edits' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    expect(screen.queryByRole('button', { name: /Send now/ })).not.toBeInTheDocument();

    await user.click(screen.getByRole('switch', { name: 'Downloaded songs' }));
    expect(commands.deviceFilesSetPrefs).toHaveBeenCalledWith('sync-laptop', {
      kinds: ['songEdits', 'songs'],
      wifiOnly: true,
    });
    await user.click(await screen.findByRole('button', { name: /Send now/ }));
    expect(commands.deviceFilesSendNow).toHaveBeenCalledWith('sync-laptop');

    emitEvent('deviceFilesEvent', {
      ...withSongs,
      transfer: {
        deviceId: 'sync-laptop',
        deviceName: 'Laptop',
        direction: 'sending',
        itemName: 'Swim · BTS',
        itemKind: 'song',
        index: 2,
        count: 5,
        doneBytes: 1024 * 1024,
        totalBytes: 4 * 1024 * 1024,
      },
    });
    const transfer = await screen.findByTestId('device-files-transfer');
    expect(within(transfer).getByText('Sending to Laptop')).toBeInTheDocument();
    expect(within(transfer).getByText('Swim · BTS (2 of 5)')).toBeInTheDocument();
    commands.deviceFilesCancel.mockResolvedValue(ok(withSongs));
    await user.click(screen.getByRole('button', { name: 'Stop' }));
    expect(commands.deviceFilesCancel).toHaveBeenCalled();
  });
});
