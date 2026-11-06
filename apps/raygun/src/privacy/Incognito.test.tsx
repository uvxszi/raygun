import type { IncognitoStatus, SettingChange, Settings } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper, makeItem } from '../shell/AppShell.test-wrapper';
import {
  ANDROID_INFO,
  DEFAULT_SETTINGS,
  INCOGNITO_OFF,
  bindingsMock,
  emitEvent,
} from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const ON: IncognitoStatus = { on: true, sinceMs: 1_000 };

/** Incognito as Rust keeps it: what the switch sets is what's read back. */
const withIncognito = (initial: IncognitoStatus = INCOGNITO_OFF) => {
  let current = initial;
  commands.incognitoStatus.mockImplementation(async () => current);
  commands.incognitoSetEnabled.mockImplementation(async (on: boolean) => {
    current = on ? ON : INCOGNITO_OFF;
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

const turnOff = 'Turn off incognito';

describe('Incognito (M47)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    withIncognito();
  });

  it('turns on at the top of More, shows in the top bar and the player bar, and a tap ends it', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/more');
    const more = await AppShellWrapper.page('more');
    const toggle = within(more).getByRole('switch', { name: 'Incognito' });
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    expect(screen.queryByRole('button', { name: turnOff })).not.toBeInTheDocument();

    await user.click(toggle);
    expect(commands.incognitoSetEnabled).toHaveBeenCalledWith(true);
    await vi.waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'true'));
    const topBar = screen.getByTestId('top-bar');
    expect(within(topBar).getByRole('button', { name: turnOff })).toBeInTheDocument();

    AppShellWrapper.playing([makeItem('Song', 'Artist')], 0);
    const bar = await screen.findByTestId('mini-player');
    await user.click(within(bar).getByRole('button', { name: turnOff }));
    expect(commands.incognitoSetEnabled).toHaveBeenLastCalledWith(false);
    await vi.waitFor(() => expect(toggle).toHaveAttribute('aria-checked', 'false'));
    expect(screen.queryByRole('button', { name: turnOff })).not.toBeInTheDocument();
  });

  it('follows incognito turned on or off elsewhere', async () => {
    await AppShellWrapper.mount('/library');
    await AppShellWrapper.page('library');
    expect(screen.queryByRole('button', { name: turnOff })).not.toBeInTheDocument();
    emitEvent('incognitoEvent', ON);
    expect(await screen.findByRole('button', { name: turnOff })).toBeInTheDocument();
    emitEvent('incognitoEvent', INCOGNITO_OFF);
    await vi.waitFor(() =>
      expect(screen.queryByRole('button', { name: turnOff })).not.toBeInTheDocument(),
    );
  });

  it("turns on and off from Home's ⋮ menu", async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    const home = await AppShellWrapper.page('home');
    await user.click(within(home).getByRole('button', { name: 'More options' }));
    await user.click(await screen.findByRole('menuitem', { name: 'Turn on incognito' }));
    expect(commands.incognitoSetEnabled).toHaveBeenCalledWith(true);
    await screen.findByRole('button', { name: turnOff });

    await user.click(within(home).getByRole('button', { name: 'More options' }));
    await user.click(await screen.findByRole('menuitem', { name: turnOff }));
    expect(commands.incognitoSetEnabled).toHaveBeenLastCalledWith(false);
  });

  it('is in Settings → Privacy, with staying on after a restart', async () => {
    const user = userEvent.setup();
    const saved = withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/settings/privacy');
    const privacy = await screen.findByTestId('settings-privacy-page');
    await user.click(within(privacy).getByRole('switch', { name: 'Incognito' }));
    expect(commands.incognitoSetEnabled).toHaveBeenCalledWith(true);
    await vi.waitFor(() =>
      expect(within(privacy).getByRole('switch', { name: 'Incognito' })).toHaveAttribute(
        'aria-checked',
        'true',
      ),
    );

    const keep = within(privacy).getByRole('switch', { name: 'Stay incognito after a restart' });
    expect(keep).toHaveAttribute('aria-checked', 'false');
    await user.click(keep);
    await vi.waitFor(() => expect(saved().privacy.keepIncognito).toBe(true));
    expect(saved().privacy.history).toBe(true);
  });

  describe('on desktop', () => {
    beforeEach(() => {
      commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    });

    it('turns on from the app menu and shows in the app bar, not twice', async () => {
      const user = userEvent.setup();
      await AppShellWrapper.mount('/');
      const bar = await screen.findByTestId('app-bar');
      await user.click(within(bar).getByRole('button', { name: 'Main menu' }));
      await user.click(screen.getByRole('menuitem', { name: 'File' }));
      await user.click(screen.getByRole('menuitem', { name: 'Turn on incognito' }));
      expect(commands.incognitoSetEnabled).toHaveBeenCalledWith(true);

      const badge = await within(bar).findByRole('button', { name: turnOff });
      expect(badge).toHaveTextContent('Incognito');
      // The screen's own top bar leaves it to the app bar.
      expect(
        within(screen.getByTestId('top-bar')).queryByRole('button', { name: turnOff }),
      ).not.toBeInTheDocument();
    });

    it('leaves when pages are played from to Rust, which keeps nothing while it is on', async () => {
      withIncognito(ON);
      await AppShellWrapper.mount('/library?view=favorites');
      await screen.findByRole('button', { name: turnOff });
      await new Promise((resolve) => setTimeout(resolve, 50));
      expect(commands.settingsUpdate).not.toHaveBeenCalledWith(
        expect.objectContaining({ key: 'rail' }),
      );
    });
  });
});
