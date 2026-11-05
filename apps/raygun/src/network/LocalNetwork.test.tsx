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
  linkText: 'raygun:link?at=192.168.1.20%3A8820&code=abcd234567&name=Pixel',
  linkQrSvg: '<svg xmlns="http://www.w3.org/2000/svg"></svg>',
  linkExpiresAt: 600_000,
  error: null,
};

const addAddonByAddress = async (user: ReturnType<typeof userEvent.setup>, address: string) => {
  await AppShellWrapper.mount('/sources');
  const page = await screen.findByTestId('sources-page');
  // The top bar's "+" opens Sources' Add; the address is looked at before it's added.
  await user.click(within(page).getByRole('button', { name: 'Add a source' }));
  const sheet = await screen.findByRole('dialog', { name: 'Add a source' });
  await user.type(within(sheet).getByRole('textbox', { name: 'Address' }), address);
  await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
  return page;
};

/** What Add shows once an address is looked up. */
const preview = (url: string) => {
  const addon = {
    id: '',
    url,
    name: 'Home addon',
    description: null,
    logo: null,
    version: '1.0.0',
    resources: ['stream'],
    types: ['movie'],
    catalogs: 0,
    adult: false,
    error: null,
    enabled: true,
    provides: [{ resource: 'stream', kind: null, count: 1 }],
    p2p: false,
    configureUrl: null,
    needsSetup: false,
    failing: false,
  };
  return ok({
    kind: 'addon' as const,
    format: null,
    server: null,
    url,
    name: addon.name,
    description: null,
    added: false,
    addon,
    entries: [],
    note: null,
  });
};

const addLookedUp = async (user: ReturnType<typeof userEvent.setup>) => {
  const card = await screen.findByRole('region', {
    name: 'Looks like a Stremio addon: Home addon',
  });
  await user.click(within(card).getByRole('button', { name: 'Add' }));
};

describe('Local network access (Android 17)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('adds an addon on the network once access is allowed', async () => {
    const user = userEvent.setup();
    commands.sourcesDetect.mockResolvedValue(preview(LAN_ADDON));
    commands.sourcesAdd.mockResolvedValue(preview(LAN_ADDON));
    commands.localNetworkAccess.mockResolvedValue(ok(ASK));
    commands.localNetworkRequest.mockResolvedValue(ok(GRANTED));
    await addAddonByAddress(user, LAN_ADDON);

    expect(commands.localNetworkAccess).toHaveBeenCalledWith(LAN_ADDON);
    const sheet = await screen.findByRole('dialog', { name: 'Local network access' });
    expect(within(sheet).getByText(/This address is on your network/)).toBeInTheDocument();
    expect(commands.sourcesDetect).not.toHaveBeenCalled();

    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(commands.localNetworkRequest).toHaveBeenCalled();
    expect(commands.sourcesDetect).toHaveBeenCalledWith(LAN_ADDON);
    expect(screen.queryByRole('dialog', { name: 'Local network access' })).not.toBeInTheDocument();
    await addLookedUp(user);
    expect(commands.sourcesAdd).toHaveBeenCalledWith(LAN_ADDON);
  });

  it('says what to allow when access is denied, and adds nothing', async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(ok(ASK));
    commands.localNetworkRequest.mockResolvedValue(ok(DENIED));
    await addAddonByAddress(user, LAN_ADDON);

    const sheet = await screen.findByRole('dialog', { name: 'Local network access' });
    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(await within(sheet).findByRole('alert')).toHaveTextContent(
      'Allow local network access to reach addresses on your network.',
    );
    expect(commands.sourcesDetect).not.toHaveBeenCalled();
    expect(commands.sourcesAdd).not.toHaveBeenCalled();

    await user.click(within(sheet).getByRole('button', { name: 'Not now' }));
    expect(screen.queryByRole('dialog', { name: 'Local network access' })).not.toBeInTheDocument();
    expect(commands.sourcesAdd).not.toHaveBeenCalled();
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
      'Allow local network access to reach addresses on your network.',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Open settings' }));
    expect(commands.localNetworkOpenSettings).toHaveBeenCalled();
    expect(commands.localNetworkRequest).toHaveBeenCalledTimes(1);
    expect(screen.queryByRole('dialog', { name: 'Local network access' })).not.toBeInTheDocument();
    expect(commands.sourcesAdd).not.toHaveBeenCalled();
  });

  it('adds a public addon without asking', async () => {
    const user = userEvent.setup();
    const found = preview('https://addon.example/manifest.json');
    commands.sourcesDetect.mockResolvedValue(found);
    commands.sourcesAdd.mockResolvedValue(found);
    await addAddonByAddress(user, 'addon.example');

    expect(commands.localNetworkAccess).toHaveBeenCalledWith('addon.example');
    expect(commands.sourcesDetect).toHaveBeenCalledWith('addon.example');
    expect(screen.queryByRole('dialog', { name: 'Local network access' })).not.toBeInTheDocument();
    await addLookedUp(user);
    // Rust adds what it found for the address it looked at.
    expect(commands.sourcesAdd).toHaveBeenCalledWith('addon.example');
  });

  it('lets devices find this one only with access, and says why not', async () => {
    const user = userEvent.setup();
    commands.localNetworkAccess.mockResolvedValue(ok(ASK));
    commands.localNetworkRequest.mockResolvedValueOnce(ok(DENIED));
    await AppShellWrapper.mount('/settings/devices');
    const devices = await screen.findByTestId('linked-devices');

    await user.click(
      await within(devices).findByRole('switch', { name: 'Let your devices find this one' }),
    );
    expect(commands.localNetworkAccess).toHaveBeenCalledWith(null);
    const sheet = await screen.findByRole('dialog', { name: 'Local network access' });
    expect(within(sheet).getByText(/Your other devices on this Wi-Fi/)).toBeInTheDocument();

    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(await within(sheet).findByRole('alert')).toHaveTextContent(
      'Allow local network access so your other devices can reach this one.',
    );
    expect(commands.devicesSetSharing).not.toHaveBeenCalled();

    commands.localNetworkRequest.mockResolvedValueOnce(ok(GRANTED));
    commands.remoteStatus.mockResolvedValue(ok(REMOTE_ON));
    await user.click(within(sheet).getByRole('button', { name: 'Continue' }));
    expect(commands.devicesSetSharing).toHaveBeenCalledWith(true);
  });

  it('stops sharing without asking', async () => {
    const user = userEvent.setup();
    commands.remoteStatus.mockResolvedValue(ok(REMOTE_ON));
    commands.syncStatus.mockResolvedValue(
      ok({
        enabled: true,
        deviceName: 'Android phone',
        fingerprint: 'ab12',
        pairingText: null,
        pairingQrSvg: null,
        kinds: ['settings', 'library', 'history'],
        peers: [],
        log: [],
        error: null,
      }),
    );
    await AppShellWrapper.mount('/settings/devices');
    const devices = await screen.findByTestId('linked-devices');

    const toggle = await within(devices).findByRole('switch', {
      name: 'Let your devices find this one',
    });
    await vi.waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'));
    await user.click(toggle);
    expect(commands.devicesSetSharing).toHaveBeenCalledWith(false);
    expect(commands.localNetworkAccess).not.toHaveBeenCalled();
  });
});
