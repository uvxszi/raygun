import type { LocalNetworkAccess, RemoteStatus } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ASK: LocalNetworkAccess = { needed: true, granted: false, blocked: false };
const GRANTED: LocalNetworkAccess = { needed: true, granted: true, blocked: false };
const DENIED: LocalNetworkAccess = { needed: true, granted: false, blocked: false };
const BLOCKED: LocalNetworkAccess = { needed: true, granted: false, blocked: true };
const LAN_ADDON = 'http://192.168.1.20:7000/manifest.json';

const REMOTE_ON: RemoteStatus = {
  enabled: true,
  url: 'http://192.168.1.20:8820',
  pairUrl: 'http://192.168.1.20:8820/#pair=abcd234567',
  pairQrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
  devices: [],
  error: null,
};

const addAddonByAddress = async (user: ReturnType<typeof userEvent.setup>, address: string) => {
  await AppShellWrapper.mount('/watch/addons');
  const page = await screen.findByTestId('watch-addons-page');
  await user.type(
    within(page).getByRole('textbox', { name: 'Add an addon by its address' }),
    address,
  );
  await user.click(within(page).getByRole('button', { name: 'Add' }));
  return page;
};

describe('Local network access (Android 17)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('adds an addon on the network once access is allowed', async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(ok(ASK));
    commands.localNetworkRequest.mockResolvedValue(ok(GRANTED));
    await addAddonByAddress(user, LAN_ADDON);

    expect(commands.localNetworkAccess).toHaveBeenCalledWith(LAN_ADDON);
    const sheet = await screen.findByRole('dialog', { name: 'Local network access' });
    expect(within(sheet).getByText(/This addon is on your network/)).toBeInTheDocument();
    expect(commands.stremioAdd).not.toHaveBeenCalled();

    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(commands.localNetworkRequest).toHaveBeenCalled();
    expect(commands.stremioAdd).toHaveBeenCalledWith(LAN_ADDON);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('says what to allow when access is denied, and adds nothing', async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(ok(ASK));
    commands.localNetworkRequest.mockResolvedValue(ok(DENIED));
    await addAddonByAddress(user, LAN_ADDON);

    const sheet = await screen.findByRole('dialog', { name: 'Local network access' });
    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(await within(sheet).findByRole('alert')).toHaveTextContent(
      'Allow local network access to reach addons on your network.',
    );
    expect(commands.stremioAdd).not.toHaveBeenCalled();

    await user.click(within(sheet).getByRole('button', { name: 'Not now' }));
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(commands.stremioAdd).not.toHaveBeenCalled();
  });

  it("sends the user to Raygun's settings once Android stops asking", async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(ok(BLOCKED));
    commands.localNetworkRequest.mockResolvedValue(ok(BLOCKED));
    await addAddonByAddress(user, LAN_ADDON);

    // Android is asked first even when it last said no for good: that may have changed since.
    const sheet = await screen.findByRole('dialog', { name: 'Local network access' });
    expect(within(sheet).queryByText(/Android won't ask again/)).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(await within(sheet).findByText(/Android won't ask again/)).toBeInTheDocument();
    expect(within(sheet).getByRole('alert')).toHaveTextContent(
      'Allow local network access to reach addons on your network.',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Open settings' }));
    expect(commands.localNetworkOpenSettings).toHaveBeenCalled();
    expect(commands.localNetworkRequest).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
    expect(commands.stremioAdd).not.toHaveBeenCalled();
  });

  it('adds a public addon without asking', async () => {
    const user = userEvent.setup();
    await addAddonByAddress(user, 'addon.example');

    expect(commands.localNetworkAccess).toHaveBeenCalledWith('addon.example');
    expect(commands.stremioAdd).toHaveBeenCalledWith('addon.example');
    expect(screen.queryByRole('dialog')).not.toBeInTheDocument();
  });

  it('turns the LAN remote on only with access, and says why not', async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(ok(ASK));
    commands.localNetworkRequest.mockResolvedValueOnce(ok(DENIED));
    commands.remoteSetEnabled.mockResolvedValue(ok(REMOTE_ON));
    await AppShellWrapper.mount('/settings');
    const card = await screen.findByTestId('remote-card');

    await user.click(await within(card).findByRole('switch', { name: 'Remote control' }));
    expect(commands.localNetworkAccess).toHaveBeenCalledWith(null);
    const sheet = await screen.findByRole('dialog', { name: 'Local network access' });
    expect(within(sheet).getByText(/Phones and computers on your Wi-Fi/)).toBeInTheDocument();

    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(await within(sheet).findByRole('alert')).toHaveTextContent(
      'Allow local network access so devices on your network can reach the remote.',
    );
    expect(commands.remoteSetEnabled).not.toHaveBeenCalled();

    commands.localNetworkRequest.mockResolvedValueOnce(ok(GRANTED));
    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(commands.remoteSetEnabled).toHaveBeenCalledWith(true);
    expect(await within(card).findByTestId('remote-qr')).toBeInTheDocument();
  });

  it('turns the remote off without asking', async () => {
    const user = userEvent.setup();
    commands.remoteStatus.mockResolvedValue(ok(REMOTE_ON));
    await AppShellWrapper.mount('/settings');
    const card = await screen.findByTestId('remote-card');

    await user.click(await within(card).findByRole('switch', { name: 'Remote control' }));
    expect(commands.remoteSetEnabled).toHaveBeenCalledWith(false);
    expect(commands.localNetworkAccess).not.toHaveBeenCalled();
  });
});
