import type { SettingChange, Settings } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from './AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, DEFAULT_SETTINGS } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

/** Settings that Rust saves and sends back, as the real command does. */
const withSettings = (settings: Settings) => {
  let current = settings;
  commands.settingsGet.mockImplementation(async () => ok(current));
  commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
    current = { ...current, [change.key]: change.value };
    return ok(current);
  });
};

const onDesktop = () =>
  commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });

describe('The desktop app bar', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    onDesktop();
  });

  it("isn't on phones", async () => {
    commands.appInfo.mockResolvedValue(ANDROID_INFO);
    await AppShellWrapper.mount('/');
    await screen.findByTestId('home-page');
    expect(screen.queryByTestId('app-bar')).not.toBeInTheDocument();
  });

  it('goes back and forward, home, and draws the window buttons', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    const bar = await screen.findByTestId('app-bar');
    const back = within(bar).getByRole('button', { name: 'Back' });
    const forward = within(bar).getByRole('button', { name: 'Forward' });
    expect(back).toBeDisabled();
    expect(forward).toBeDisabled();
    expect(within(bar).getByRole('button', { name: 'Close' })).toBeInTheDocument();
    expect(within(bar).getByRole('button', { name: 'Minimize' })).toBeInTheDocument();

    // Downloads opens a popover first (M46.37), with a way to the page.
    await user.click(within(bar).getByRole('button', { name: 'Downloads' }));
    await user.click(await screen.findByRole('link', { name: 'Open Downloads' }));
    await screen.findByTestId('downloads-page');
    expect(back).toBeEnabled();
    await user.click(back);
    await screen.findByTestId('home-page');
    expect(forward).toBeEnabled();
    await user.click(forward);
    await screen.findByTestId('downloads-page');
    await user.click(within(bar).getByRole('link', { name: 'Home' }));
    await screen.findByTestId('home-page');
  });

  it('searches from anywhere and opens Browse all', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    const bar = await screen.findByTestId('app-bar');
    const search = within(bar).getByRole('searchbox', { name: 'Search' });
    expect(search).toHaveAttribute('placeholder', 'What do you want to play?');
    await user.type(search, 'daft punk{Enter}');
    await screen.findByTestId('browse-page');
    await vi.waitFor(() => expect(commands.onlineSearch).toHaveBeenCalledWith('daft punk'));
    // Browse's own field is the bar's on desktop.
    expect(screen.getAllByRole('searchbox', { name: 'Search' })).toHaveLength(1);

    await user.click(within(bar).getByRole('link', { name: 'Browse' }));
    expect(await screen.findByTestId('browse-all')).toBeInTheDocument();
  });

  it('searches again from Browse without the old query coming back', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/browse?tab=search&type=music&q=old%20song');
    const bar = await screen.findByTestId('app-bar');
    const search = within(bar).getByRole('searchbox', { name: 'Search' });
    await vi.waitFor(() => expect(search).toHaveValue('old song'));
    await user.clear(search);
    await user.type(search, 'new song');
    await vi.waitFor(() => expect(commands.onlineSearch).toHaveBeenLastCalledWith('new song'));
    await new Promise((resolve) => setTimeout(resolve, 900));
    expect(search).toHaveValue('new song');
  });

  it('runs File, View, Playback and Help from the ⋯ menu', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    const bar = await screen.findByTestId('app-bar');

    await user.click(within(bar).getByRole('button', { name: 'Main menu' }));
    await user.click(screen.getByRole('menuitem', { name: 'View' }));
    await user.click(
      within(screen.getByRole('menu', { name: 'View' })).getByRole('menuitem', {
        name: 'Library',
      }),
    );
    expect(await screen.findByTestId('library-page')).toBeInTheDocument();
    expect(screen.queryByRole('menu')).not.toBeInTheDocument();

    await user.click(within(bar).getByRole('button', { name: 'Main menu' }));
    await user.hover(screen.getByRole('menuitem', { name: 'Playback' }));
    await user.click(screen.getByRole('menuitem', { name: /^Next song/ }));
    expect(commands.playbackControl).toHaveBeenLastCalledWith({ type: 'next' });

    // From the keyboard: the arrow opens a group, Escape steps back out.
    await user.click(within(bar).getByRole('button', { name: 'Main menu' }));
    const help = screen.getByRole('menuitem', { name: 'Help' });
    help.focus();
    await user.keyboard('{ArrowRight}');
    await vi.waitFor(() =>
      expect(screen.getByRole('menuitem', { name: /^Keyboard shortcuts/ })).toHaveFocus(),
    );
    await user.keyboard('{Enter}');
    expect(await screen.findByRole('dialog', { name: 'Keyboard shortcuts' })).toBeInTheDocument();

    await user.keyboard('{Escape}');
    await user.click(within(bar).getByRole('button', { name: 'Main menu' }));
    await user.click(screen.getByRole('menuitem', { name: 'File' }));
    await user.click(screen.getByRole('menuitem', { name: /^New playlist/ }));
    expect(await screen.findByRole('dialog', { name: 'New playlist' })).toBeInTheDocument();
  });

  it('zooms with the keys and focuses search with Ctrl+K', async () => {
    withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/');
    const bar = await screen.findByTestId('app-bar');
    fireEvent.keyDown(window, { key: '=', ctrlKey: true });
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenLastCalledWith({ key: 'zoom', value: 110 }),
    );
    fireEvent.keyDown(window, { key: '0', ctrlKey: true });
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenLastCalledWith({ key: 'zoom', value: 100 }),
    );
    fireEvent.keyDown(window, { key: 'k', ctrlKey: true });
    expect(within(bar).getByRole('searchbox', { name: 'Search' })).toHaveFocus();
    // Ctrl and the wheel zoom too (M46.24); plain scrolling doesn't.
    fireEvent.wheel(window, { deltaY: -100, ctrlKey: true });
    await vi.waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenLastCalledWith({ key: 'zoom', value: 110 }),
    );
    const calls = commands.settingsUpdate.mock.calls.length;
    fireEvent.wheel(window, { deltaY: 100 });
    expect(commands.settingsUpdate).toHaveBeenCalledTimes(calls);
  });

  it("has the mascot's button, which says mascots are coming", async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/');
    const bar = await screen.findByTestId('app-bar');
    await user.click(within(bar).getByRole('button', { name: 'Mascot' }));
    expect(await screen.findByTestId('mascot-soon')).toHaveTextContent('Mascots are coming soon');
  });

  it('offers the frame and zoom in Settings → Appearance', async () => {
    const user = userEvent.setup();
    withSettings(DEFAULT_SETTINGS);
    await AppShellWrapper.mount('/settings/appearance');
    await user.click(await screen.findByRole('button', { name: /Top of the window/ }));
    await user.click(await screen.findByRole('radio', { name: "The system's title bar" }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'windowFrame',
      value: 'system',
    });
    expect(screen.getByRole('slider', { name: 'Interface size' })).toHaveValue('100');
  });

  it("leaves the window's buttons to the system's title bar when asked", async () => {
    withSettings({ ...DEFAULT_SETTINGS, windowFrame: 'system' });
    await AppShellWrapper.mount('/');
    const bar = await screen.findByTestId('app-bar');
    await vi.waitFor(() =>
      expect(within(bar).queryByRole('button', { name: 'Close' })).not.toBeInTheDocument(),
    );
  });
});
