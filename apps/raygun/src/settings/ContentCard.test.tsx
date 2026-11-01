import { screen } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;

describe('Content card', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('keeps NSFW content off by default and turns it on', async () => {
    const user = userEvent.setup();
    const settings = (await commands.settingsGet()) as { data: Record<string, unknown> };
    commands.settingsUpdate.mockResolvedValue({
      status: 'ok',
      data: { ...settings.data, showNsfw: true },
    });
    await AppShellWrapper.mount('/settings');

    const toggle = await screen.findByRole('switch', { name: 'Show NSFW (18+) content' });
    expect(toggle).toHaveAttribute('aria-checked', 'false');
    // Turning it on refreshes every query; the stored value comes back on.
    commands.settingsGet.mockResolvedValue({
      status: 'ok',
      data: { ...settings.data, showNsfw: true },
    });
    await user.click(toggle);

    expect(commands.settingsUpdate).toHaveBeenCalledWith({ key: 'showNsfw', value: true });
    expect(toggle).toHaveAttribute('aria-checked', 'true');
  });
});
