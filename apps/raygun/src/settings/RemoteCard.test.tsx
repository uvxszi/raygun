import type { RemoteStatus } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ON: RemoteStatus = {
  enabled: true,
  url: 'http://192.168.1.20:8820',
  pairUrl: 'http://192.168.1.20:8820/#pair=abcd234567',
  pairQrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
  devices: [],
  error: null,
};

describe('Remote control card', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('shows the pairing QR code and address once devices may find this one', async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(
      ok({ needed: false, granted: true, blocked: false }),
    );
    await AppShellWrapper.mount('/settings/devices');
    const card = await screen.findByTestId('remote-card');
    expect(within(card).queryByRole('switch')).not.toBeInTheDocument();
    expect(within(card).queryByTestId('remote-qr')).not.toBeInTheDocument();

    commands.remoteStatus.mockResolvedValue(ok(ON));
    const devices = screen.getByTestId('linked-devices');
    await user.click(
      await within(devices).findByRole('switch', { name: 'Let your devices find this one' }),
    );

    expect(commands.devicesSetSharing).toHaveBeenCalledWith(true);
    const qr = await within(card).findByRole('img', { name: 'QR code to pair a device' });
    expect(qr.getAttribute('src')).toMatch(/^data:image\/svg\+xml;utf8,%3Csvg/);
    expect(within(card).getByTestId('remote-pair-url')).toHaveTextContent(ON.pairUrl as string);
    expect(within(card).getByText('No devices yet.')).toBeInTheDocument();
  });

  it('makes a new pairing code once the last one was used', async () => {
    const user = userEvent.setup();
    commands.remoteStatus.mockResolvedValue(ok({ ...ON, pairUrl: null, pairQrSvg: null }));
    commands.remoteNewCode.mockResolvedValue(ok(ON));
    await AppShellWrapper.mount('/settings/devices');
    const card = await screen.findByTestId('remote-card');

    expect(await within(card).findByText(/The pairing code was used/)).toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'New pairing code' }));
    expect(commands.remoteNewCode).toHaveBeenCalled();
    expect(await within(card).findByTestId('remote-qr')).toBeInTheDocument();
  });

  it('picks up a device that paired from its browser', async () => {
    commands.remoteStatus.mockResolvedValue(ok(ON));
    await AppShellWrapper.mount('/settings/devices');
    const card = await screen.findByTestId('remote-card');
    expect(await within(card).findByText('No devices yet.')).toBeInTheDocument();

    commands.remoteStatus.mockResolvedValue(
      ok({
        ...ON,
        pairUrl: null,
        pairQrSvg: null,
        devices: [{ id: 'd1', name: 'Phone', pairedAt: 1 }],
      }),
    );
    expect(await within(card).findByText('Phone', {}, { timeout: 5000 })).toBeInTheDocument();
    expect(within(card).queryByTestId('remote-qr')).not.toBeInTheDocument();
  });

  it('lists paired devices and removes one', async () => {
    const user = userEvent.setup();
    const laptop = { id: 'd1', name: 'Laptop', pairedAt: Date.UTC(2026, 9, 31) };
    commands.remoteStatus.mockResolvedValue(ok({ ...ON, devices: [laptop] }));
    commands.remoteForgetDevice.mockResolvedValue(ok(ON));
    await AppShellWrapper.mount('/settings/devices');
    const card = await screen.findByTestId('remote-card');

    expect(await within(card).findByText('Laptop')).toBeInTheDocument();
    expect(within(card).getByText(/Paired Oct 3[01], 2026/)).toBeInTheDocument();
    await user.click(within(card).getByRole('button', { name: 'Remove Laptop' }));
    expect(commands.remoteForgetDevice).toHaveBeenCalledWith('d1');
    expect(await within(card).findByText('No devices yet.')).toBeInTheDocument();
  });
});
