import type { SyncStatus } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ON: SyncStatus = {
  enabled: true,
  deviceName: 'Android phone',
  fingerprint: 'ab12 cd34 ef56 7890',
  pairingText: null,
  pairingQrSvg: null,
  kinds: ['settings', 'library', 'history'],
  peers: [],
  log: [],
  error: null,
};

const LAPTOP = {
  deviceId: 'dev-laptop',
  name: 'Laptop',
  fingerprint: '1111 2222 3333 4444',
  address: '192.168.1.9:8830',
  pairedAt: 1,
  lastSyncAt: Date.UTC(2026, 9, 31, 12),
};

describe('Devices', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('turns sync on from Settings and shows this device', async () => {
    const user = userEvent.setup();
    commands.syncSetEnabled.mockResolvedValue(ok(ON));
    await AppShellWrapper.mount('/settings/connections');
    expect(await screen.findByTestId('devices-card')).toHaveTextContent('Off');
    await user.click(screen.getByTestId('devices-card'));

    const page = await screen.findByTestId('devices-page');
    await user.click(await within(page).findByRole('switch', { name: 'Device sync' }));
    expect(commands.syncSetEnabled).toHaveBeenCalledWith(true);
    expect(
      await within(page).findByText('No devices yet. Pair one with a code from either device.'),
    ).toBeInTheDocument();
    expect(within(page).getByTestId('device-name')).toHaveTextContent('Android phone');
    expect(within(page).getByText('Key ab12 cd34 ef56 7890')).toBeInTheDocument();
  });

  it('shows a pairing code and joins with pasted text', async () => {
    const user = userEvent.setup();
    const text = 'raygun-pair:192.168.1.20:8830:abcd2345:dev-phone:' + '07'.repeat(32);
    commands.syncStatus.mockResolvedValue(ok(ON));
    commands.syncOpenPairing.mockResolvedValue(
      ok({
        ...ON,
        pairingText: text,
        pairingQrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
      }),
    );
    commands.syncJoin.mockResolvedValue(ok({ ...ON, peers: [LAPTOP] }));
    await AppShellWrapper.mount('/settings/devices');
    const page = await screen.findByTestId('devices-page');

    await user.click(await within(page).findByRole('button', { name: 'Show pairing code' }));
    expect(await within(page).findByTestId('pairing-text')).toHaveTextContent(text);
    expect(
      within(page).getByRole('img', { name: 'QR code with the pairing text' }),
    ).toBeInTheDocument();
    await user.click(within(page).getByRole('button', { name: 'Copy text' }));
    expect(await navigator.clipboard.readText()).toBe(text);

    await user.type(within(page).getByLabelText('Pairing text'), 'raygun-pair:10.0.2.2:8835:x:y:z');
    await user.click(within(page).getByRole('button', { name: 'Pair and sync' }));
    expect(commands.syncJoin).toHaveBeenCalledWith('raygun-pair:10.0.2.2:8835:x:y:z');
    expect(await within(page).findByTestId('peer')).toHaveTextContent('Laptop');
  });

  it('syncs, removes a device and changes what syncs', async () => {
    const user = userEvent.setup();
    commands.syncStatus.mockResolvedValue(ok({ ...ON, peers: [LAPTOP] }));
    commands.syncNow.mockResolvedValue(ok({ ...ON, peers: [LAPTOP] }));
    commands.syncSetKinds.mockResolvedValue(
      ok({ ...ON, kinds: ['settings', 'library'], peers: [LAPTOP] }),
    );
    await AppShellWrapper.mount('/settings/devices');
    const page = await screen.findByTestId('devices-page');

    const peer = await within(page).findByTestId('peer');
    expect(peer).toHaveTextContent('1111 2222 3333 4444');
    expect(peer).toHaveTextContent(/Synced Oct 31, 2026/);
    await user.click(within(peer).getByRole('button', { name: 'Sync with Laptop now' }));
    expect(commands.syncNow).toHaveBeenCalledWith('dev-laptop');

    await user.click(within(page).getByRole('checkbox', { name: 'History' }));
    expect(commands.syncSetKinds).toHaveBeenCalledWith(['settings', 'library']);
    expect(await within(page).findByRole('checkbox', { name: 'History' })).not.toBeChecked();

    await user.click(within(peer).getByRole('button', { name: 'Remove Laptop' }));
    expect(commands.syncRemovePeer).toHaveBeenCalledWith('dev-laptop');
  });

  it('lists sessions and undoes the last one', async () => {
    const user = userEvent.setup();
    const session = {
      id: 7,
      peerName: 'Laptop',
      startedAt: Date.UTC(2026, 9, 31, 12),
      finished: true,
      sent: 2,
      received: 5,
      changed: 3,
      error: null,
      undone: false,
    };
    const failed = {
      ...session,
      id: 6,
      error: 'the other device didn’t answer in time',
      changed: 0,
    };
    commands.syncStatus.mockResolvedValue(ok({ ...ON, peers: [LAPTOP], log: [session, failed] }));
    commands.syncUndoLast.mockResolvedValue(
      ok({ ...ON, peers: [LAPTOP], log: [{ ...session, undone: true }, failed] }),
    );
    await AppShellWrapper.mount('/settings/devices');
    const page = await screen.findByTestId('devices-page');

    const sessions = await within(page).findAllByTestId('sync-session');
    expect(sessions[0]).toHaveTextContent('2 sent · 5 received · 3 changed here');
    expect(sessions[1]).toHaveTextContent('didn’t answer in time');
    await user.click(within(page).getByRole('button', { name: 'Undo last sync' }));
    expect(commands.syncUndoLast).toHaveBeenCalled();
    expect(await within(page).findByText(/undone/)).toBeInTheDocument();
    expect(within(page).queryByRole('button', { name: 'Undo last sync' })).not.toBeInTheDocument();
  });

  it('refreshes favorites after another device synced changes here', async () => {
    commands.favoritesList.mockResolvedValue(ok([]));
    await AppShellWrapper.mount('/library?view=favorites');
    await screen.findByText(/Tap the heart/);
    const before = commands.favoritesList.mock.calls.length;

    emitEvent('syncChangedEvent', 2);
    await vi.waitFor(() =>
      expect(commands.favoritesList.mock.calls.length).toBeGreaterThan(before),
    );
  });
});
