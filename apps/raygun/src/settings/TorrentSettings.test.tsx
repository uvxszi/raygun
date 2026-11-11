import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const OFF = { enabled: false, wifiOnly: true, cacheGb: 4, seed: false, port: 0, dht: true };

describe('Torrents in Settings → Video player', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('turn on only after the notes about sharing are read', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/video');

    const toggle = await screen.findByRole('switch', { name: 'Play torrents' });
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    // The rest waits for them to be on.
    expect(screen.getByRole('switch', { name: 'Only on Wi-Fi' })).toBeDisabled();

    await user.click(toggle);
    const notes = await screen.findByTestId('torrent-notes');
    expect(notes).toHaveTextContent('They see your IP address');
    expect(notes).toHaveTextContent('Raygun suggests no torrent sources');
    expect(commands.settingsUpdate).not.toHaveBeenCalledWith(
      expect.objectContaining({ key: 'torrents' }),
    );

    await user.click(within(notes).getByRole('button', { name: 'Turn on' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'torrents',
      value: { ...OFF, enabled: true },
    });
  });

  it('turn off at once, and keep a port of their own when one is typed', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(
      ok({ ...DEFAULT_SETTINGS, torrents: { ...OFF, enabled: true } }),
    );
    await AppShellWrapper.mount('/settings/video');

    await user.click(await screen.findByRole('button', { name: /^Listening port/ }));
    const field = screen.getByRole('textbox', { name: 'Listening port' });
    await user.type(field, '80');
    expect(await screen.findByRole('alert')).toHaveTextContent('A port is 1024 to 65535');
    await user.clear(field);
    await user.type(field, '6881');
    await user.click(screen.getByRole('button', { name: 'Save' }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'torrents',
      value: { ...OFF, enabled: true, port: 6881 },
    });

    // Turning them off asks nothing.
    await user.click(screen.getByRole('switch', { name: 'Play torrents' }));
    expect(screen.queryByTestId('torrent-notes')).not.toBeInTheDocument();
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'torrents',
      value: expect.objectContaining({ enabled: false }),
    });
  });
});
