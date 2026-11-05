import { cleanup, screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const onDesktop = () =>
  commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });

const searchSettings = async (user: ReturnType<typeof userEvent.setup>, text: string) => {
  await AppShellWrapper.mount('/settings');
  await user.click(await screen.findByRole('button', { name: 'Search settings' }));
  await user.type(screen.getByRole('searchbox', { name: 'Search settings' }), text);
};

describe('Settings shown per platform', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.settingsGet.mockResolvedValue(ok(DEFAULT_SETTINGS));
  });

  it("lists Android's permissions on a phone only", async () => {
    await AppShellWrapper.mount('/settings');
    expect(await screen.findByRole('link', { name: /^Permissions/ })).toBeInTheDocument();
  });

  it('leaves Permissions out of the list and search on desktop', async () => {
    const user = userEvent.setup();
    onDesktop();
    await searchSettings(user, 'Permissions');
    expect(await screen.findByTestId('settings-search-empty')).toBeInTheDocument();
    await user.clear(screen.getByRole('searchbox', { name: 'Search settings' }));
    expect(await screen.findByRole('link', { name: /^Appearance/ })).toBeInTheDocument();
    expect(screen.queryByRole('link', { name: /^Permissions/ })).not.toBeInTheDocument();
  });

  it("shows the phone player's gestures, picture-in-picture and rotation on a phone", async () => {
    await AppShellWrapper.mount('/settings/video');
    expect(await screen.findByRole('link', { name: /^Gestures/ })).toBeInTheDocument();
    expect(screen.getByRole('link', { name: /^Picture-in-picture/ })).toBeInTheDocument();
    cleanup();
    await AppShellWrapper.mount('/settings/video/controls');
    expect(await screen.findByRole('button', { name: /^Orientation/ })).toBeInTheDocument();
    cleanup();
    await AppShellWrapper.mount('/settings/video/advanced');
    expect(await screen.findByRole('button', { name: /^Decoder/ })).toBeInTheDocument();
  });

  it("hides the phone player's settings on desktop, in the screens and in search", async () => {
    const user = userEvent.setup();
    onDesktop();
    await AppShellWrapper.mount('/settings/video');
    expect(await screen.findByRole('link', { name: /^Subtitles/ })).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(screen.queryByRole('link', { name: /^Gestures/ })).not.toBeInTheDocument(),
    );
    expect(screen.queryByRole('link', { name: /^Picture-in-picture/ })).not.toBeInTheDocument();

    cleanup();
    await AppShellWrapper.mount('/settings/video/controls');
    expect(await screen.findByRole('button', { name: /^Aspect/ })).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(screen.queryByRole('button', { name: /^Orientation/ })).not.toBeInTheDocument(),
    );
    expect(screen.queryByRole('switch', { name: /brightness/i })).not.toBeInTheDocument();

    cleanup();
    await AppShellWrapper.mount('/settings/video/advanced');
    expect(await screen.findByRole('switch', { name: /trailer/i })).toBeInTheDocument();
    expect(screen.queryByRole('button', { name: /^Decoder/ })).not.toBeInTheDocument();

    cleanup();
    await searchSettings(user, 'Brightness swipe');
    expect(await screen.findByTestId('settings-search-empty')).toBeInTheDocument();
  });

  it('finds the brightness swipe in search on a phone', async () => {
    const user = userEvent.setup();
    await searchSettings(user, 'Brightness swipe');
    expect(await screen.findByRole('link', { name: /Brightness swipe/ })).toBeInTheDocument();
  });

  it('offers downloads in battery saver on a phone only', async () => {
    await AppShellWrapper.mount('/settings/downloads');
    expect(await screen.findByRole('switch', { name: /battery saver/ })).toBeInTheDocument();

    onDesktop();
    cleanup();
    await AppShellWrapper.mount('/settings/downloads');
    expect(await screen.findByRole('switch', { name: /pages/i })).toBeInTheDocument();
    await vi.waitFor(() =>
      expect(screen.queryByRole('switch', { name: /battery saver/ })).not.toBeInTheDocument(),
    );
  });

  it("offers the reader's full screen button on desktop only", async () => {
    await AppShellWrapper.mount('/settings/reader');
    expect(await screen.findByRole('switch', { name: /Show progress/ })).toBeInTheDocument();
    expect(screen.queryByRole('switch', { name: /Full screen button/ })).not.toBeInTheDocument();

    onDesktop();
    cleanup();
    await AppShellWrapper.mount('/settings/reader');
    expect(await screen.findByRole('switch', { name: /Full screen button/ })).toBeInTheDocument();
  });
});
