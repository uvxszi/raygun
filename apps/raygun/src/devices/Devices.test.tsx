import type { SyncStatus } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { ROOMY, labelOf, tapTargets } from './tapTargets.test-wrapper';

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
        linkText: null,
        linkQrSvg: null,
        linkExpiresAt: null,
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
    expect(within(row).getByRole('button', { name: 'Link' }).className).toMatch(ROOMY);
    await user.click(within(row).getByRole('button', { name: 'Link' }));
    expect(commands.connectLink).toHaveBeenCalledWith('pc');
    const linked = await within(card).findByTestId('linked-device');
    expect(linked).toHaveTextContent('Synced 2 minutes ago');
    // Play there, Sync now and Unlink: each 44 px or more, in readable type.
    expect(tapTargets(linked)).toHaveLength(3);
    for (const target of tapTargets(linked)) {
      expect(target.className, labelOf(target)).toMatch(ROOMY);
    }
    expect(linked.querySelector('.text-xs')).toBeNull();
    await user.click(within(linked).getByRole('button', { name: 'Sync now' }));
    expect(commands.syncNow).toHaveBeenCalledWith('sync-pc');
    await user.click(within(linked).getByRole('button', { name: 'Play there' }));
    expect(commands.connectTo).toHaveBeenCalledWith('pc');
  });

  it("shows this device's link QR code, made fresh each time", async () => {
    const user = userEvent.setup();
    const text = 'raygun:link?at=10.0.2.15%3A8820&code=abcd234567&name=Android+phone';
    commands.localNetworkAccess.mockResolvedValue(
      ok({ needed: false, granted: true, blocked: false }),
    );
    const shown = {
      enabled: true,
      url: 'http://10.0.2.15:8820',
      linkText: text,
      linkQrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
      linkExpiresAt: 600_000,
      error: null,
    };
    commands.devicesSetSharing.mockResolvedValue(ok(null));
    commands.remoteNewCode.mockResolvedValue(ok(shown));
    commands.remoteStatus.mockResolvedValue(ok(shown));
    await AppShellWrapper.mount('/settings/devices');
    const page = await screen.findByTestId('devices-page');

    await user.click(
      await within(page).findByRole('button', { name: 'Show this device’s link code' }),
    );
    const sheet = await screen.findByTestId('show-link-code');
    expect(
      await within(sheet).findByRole('img', { name: 'QR code to link with this device' }),
    ).toBeInTheDocument();
    // Opening it lets the other device reach this one.
    expect(commands.devicesSetSharing).toHaveBeenCalledWith(true);
    expect(commands.remoteNewCode).toHaveBeenCalledTimes(1);
    await user.click(within(sheet).getByRole('button', { name: 'Copy as text' }));
    expect(await navigator.clipboard.readText()).toBe(text);
    await user.click(within(sheet).getByRole('button', { name: 'New code' }));
    await vi.waitFor(() => expect(commands.remoteNewCode).toHaveBeenCalledTimes(2));
  });

  it('links by a pasted link code where there is no camera, and says what went wrong', async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(
      ok({ needed: false, granted: true, blocked: false }),
    );
    commands.connectLinkCode.mockResolvedValueOnce({
      status: 'error',
      error: 'That QR code expired or was used. Show a new one on the other device.',
    });
    await AppShellWrapper.mount('/settings/devices');
    const page = await screen.findByTestId('devices-page');

    await user.click(await within(page).findByRole('button', { name: 'Scan a link code' }));
    const sheet = await screen.findByTestId('scan-link-code');
    expect(await within(sheet).findByText(/No camera here/)).toBeInTheDocument();
    const field = within(sheet).getByRole('textbox', { name: 'Paste a link code' });

    await user.type(field, 'hello');
    await user.click(within(sheet).getByRole('button', { name: 'Link' }));
    expect(within(sheet).getByText('That isn’t a Raygun link code.')).toBeInTheDocument();
    expect(commands.connectLinkCode).not.toHaveBeenCalled();

    const text = 'raygun:link?at=10.0.2.2%3A8820&code=abcd234567&name=Living+room+PC';
    await user.clear(field);
    await user.type(field, text);
    await user.click(within(sheet).getByRole('button', { name: 'Link' }));
    expect(commands.connectLinkCode).toHaveBeenCalledWith(text);
    expect(await within(sheet).findByText("Couldn't link with Living room PC")).toBeInTheDocument();
    expect(within(sheet).getByText(/expired or was used/)).toBeInTheDocument();

    commands.connectLinkCode.mockResolvedValueOnce(ok([]));
    await user.click(within(sheet).getByRole('button', { name: 'Link' }));
    await vi.waitFor(() => expect(screen.queryByTestId('scan-link-code')).not.toBeInTheDocument());
  });

  it('has no way to type a device address or control from a browser', async () => {
    await AppShellWrapper.mount('/settings/devices');
    const page = await screen.findByTestId('devices-page');
    await within(page).findByTestId('linked-devices');
    expect(within(page).queryByText(/by address/)).not.toBeInTheDocument();
    expect(within(page).queryByText(/web browser/)).not.toBeInTheDocument();
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
