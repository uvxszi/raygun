import type { NetworkStatus, SettingChange, Settings } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { useSourceHealth } from '../search/sourceHealth';
import { DEFAULT_SETTINGS, ONLINE, bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const OFFLINE: NetworkStatus = { ...ONLINE, offline: true };

/** Offline Mode as Rust keeps it: what the switch sets is what's read back. */
const withNetwork = (initial: NetworkStatus = ONLINE) => {
  let current = initial;
  commands.networkStatus.mockImplementation(async () => current);
  commands.networkSetOffline.mockImplementation(async (offline: boolean) => {
    current = { ...current, offline };
    return ok(current);
  });
};

/** Settings that Rust saves and sends back, as the real command does. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
  return () => current;
};

const turnOff = 'Turn off offline mode';

describe('Offline mode', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withNetwork();
  });

  it('turns on at the top of More beside incognito, shows in the top bar, and a tap ends it', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/more');
    const more = await AppShellWrapper.page('more');
    expect(within(more).getByRole('switch', { name: 'Incognito' })).toBeInTheDocument();
    const toggle = within(more).getByRole('switch', { name: 'Offline mode' });
    expect(toggle).toHaveAttribute('aria-checked', 'false');

    await user.click(toggle);
    expect(commands.networkSetOffline).toHaveBeenCalledWith(true);
    await vi.waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'));
    expect(document.documentElement).toHaveAttribute('data-offline');
    const badge = within(screen.getByTestId('top-bar')).getByRole('button', { name: turnOff });

    await user.click(badge);
    expect(commands.networkSetOffline).toHaveBeenLastCalledWith(false);
    await vi.waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'false'));
    expect(screen.queryByRole('button', { name: turnOff })).not.toBeInTheDocument();
    expect(document.documentElement).not.toHaveAttribute('data-offline');
  });

  it('follows offline mode turned on or off elsewhere', async () => {
    await AppShellWrapper.mount('/library');
    await AppShellWrapper.page('library');
    emitEvent('networkStatusEvent', OFFLINE);
    expect(await screen.findByRole('button', { name: turnOff })).toBeInTheDocument();
    emitEvent('networkStatusEvent', ONLINE);
    await vi.waitFor(() =>
      expect(screen.queryByRole('button', { name: turnOff })).not.toBeInTheDocument(),
    );
  });

  it("turns on and off from Home's ⋮ menu", async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    const home = await AppShellWrapper.page('home');
    await user.click(within(home).getByRole('button', { name: 'More options' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Turn on offline mode' }));
    expect(commands.networkSetOffline).toHaveBeenCalledWith(true);
    await screen.findByRole('button', { name: turnOff });

    await user.click(within(home).getByRole('button', { name: 'More options' }));
    await user.click(await screen.findByRole('menuitem', { name: turnOff }));
    expect(commands.networkSetOffline).toHaveBeenLastCalledWith(false);
  });

  it('keeps Data Saver and its parts in Settings → Offline mode & data saver', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/settings/network');
    const page = await screen.findByTestId('settings-network-page');

    const saver = within(page).getByRole('switch', { name: 'Data saver' });
    const covers = within(page).getByRole('switch', { name: 'Smaller covers' });
    expect(covers).toBeDisabled();
    await user.click(saver);
    await vi.waitFor(() => expect(saved().network?.dataSaver?.enabled).toBe(true));
    await vi.waitFor(() => expect(covers).toBeEnabled());
    await user.click(covers);
    await vi.waitFor(() => expect(saved().network?.dataSaver?.smallerCovers).toBe(false));
    // The rest keep their defaults, and Offline Mode stays off.
    expect(saved().network?.dataSaver?.noPrefetch).toBe(true);
    expect(saved().network?.dataSaver?.videoResolution).toBe('480p');
    expect(saved().network?.offline).toBe(false);

    await user.click(within(page).getByRole('switch', { name: 'Offline mode' }));
    expect(commands.networkSetOffline).toHaveBeenCalledWith(true);
  });

  it('says calmly that offline mode is on where a source would fail, and blames no source', async () => {
    const user = userEvent.setup();
    withNetwork(OFFLINE);
    commands.musicChart.mockResolvedValue({ status: 'error', error: 'Offline mode is on' });
    await AppShellWrapper.mount('/music/chart?provider=x&id=top');

    const words = await screen.findByText('Offline mode is on');
    const note = words.closest<HTMLElement>('[role="status"]');
    if (!note) throw new Error('no note');
    expect(screen.queryByTestId('source-issue')).not.toBeInTheDocument();
    expect(within(note).queryByRole('button', { name: 'Try again' })).not.toBeInTheDocument();

    useSourceHealth.getState().failed('dev.example/source');
    expect(useSourceHealth.getState().failures).not.toHaveProperty('dev.example/source');

    await user.click(within(note).getByRole('button', { name: turnOff }));
    expect(commands.networkSetOffline).toHaveBeenCalledWith(false);
  });
});
