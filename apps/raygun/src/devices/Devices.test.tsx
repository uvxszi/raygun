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

  it('lets devices find this one from Settings, then links one and plays there', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings');
    await user.click(await screen.findByRole('link', { name: 'Devices' }));

    const page = await screen.findByTestId('devices-page');
    expect(within(page).getByTestId('device-name')).toHaveTextContent('Android phone');
    commands.localNetworkAccess.mockResolvedValue(
      ok({ needed: false, granted: true, blocked: false }),
    );
    commands.syncStatus.mockResolvedValue(ok(ON));
    commands.remoteStatus.mockResolvedValue(
      ok({
        enabled: true,
        url: 'http://10.0.2.15:8820',
        pairUrl: null,
        pairQrSvg: null,
        devices: [],
        error: null,
      }),
    );
    const nearby = {
      id: 'pc',
      name: 'Living room PC',
      address: '10.0.2.2:8820',
      kind: 'computer',
      paired: false,
      found: true,
      syncId: null,
      lastSyncAt: null,
    };
    commands.connectDevices.mockResolvedValue(ok([nearby]));
    await user.click(
      await within(page).findByRole('switch', { name: 'Let your devices find this one' }),
    );
    expect(commands.devicesSetSharing).toHaveBeenCalledWith(true);

    const card = within(page).getByTestId('linked-devices');
    const row = await within(card).findByTestId('nearby-device');
    commands.connectDevices.mockResolvedValue(
      ok([{ ...nearby, paired: true, syncId: 'sync-pc', lastSyncAt: Date.now() - 120_000 }]),
    );
    await user.click(within(row).getByRole('button', { name: 'Link' }));
    expect(commands.connectLink).toHaveBeenCalledWith('pc');
    const linked = await within(card).findByTestId('linked-device');
    expect(linked).toHaveTextContent('Synced 2 minutes ago');
    await user.click(within(linked).getByRole('button', { name: 'Sync now' }));
    expect(commands.syncNow).toHaveBeenCalledWith('sync-pc');
    await user.click(within(linked).getByRole('button', { name: 'Play there' }));
    expect(commands.connectTo).toHaveBeenCalledWith('pc');
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

    // Where devices don't find each other: a code shown on one, pasted on the other.
    await user.click(await within(page).findByRole('button', { name: 'Link with a code' }));
    const sheet = await screen.findByTestId('link-with-code');
    await user.click(within(sheet).getByRole('button', { name: 'Show pairing code' }));
    expect(await within(sheet).findByTestId('pairing-text')).toHaveTextContent(text);
    expect(
      within(sheet).getByRole('img', { name: 'QR code with the pairing text' }),
    ).toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Copy text' }));
    expect(await navigator.clipboard.readText()).toBe(text);

    await user.type(
      within(sheet).getByLabelText('Pairing text'),
      'raygun-pair:10.0.2.2:8835:x:y:z',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Link' }));
    expect(commands.syncJoin).toHaveBeenCalledWith('raygun-pair:10.0.2.2:8835:x:y:z');
    // A device linked by code syncs only, and is listed with the others.
    const linked = await within(page).findByTestId('linked-device');
    expect(linked).toHaveTextContent('Laptop');
    expect(linked).toHaveTextContent('syncs only');
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

    const peer = await within(page).findByTestId('linked-device');
    expect(peer).toHaveTextContent('Laptop');
    expect(peer).toHaveTextContent(/Synced .* ago/);
    await user.click(within(peer).getByRole('button', { name: 'Sync now' }));
    expect(commands.syncNow).toHaveBeenCalledWith('dev-laptop');

    await user.click(within(page).getByRole('switch', { name: 'History' }));
    expect(commands.syncSetKinds).toHaveBeenCalledWith(['settings', 'library']);
    expect(await within(page).findByRole('switch', { name: 'History' })).toHaveAttribute(
      'aria-checked',
      'false',
    );

    // Unlinking asks first.
    await user.click(within(peer).getByRole('button', { name: 'Unlink Laptop' }));
    expect(commands.syncRemovePeer).not.toHaveBeenCalled();
    await user.click(
      within(await screen.findByTestId('confirm-sheet')).getByRole('button', { name: 'Unlink' }),
    );
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
