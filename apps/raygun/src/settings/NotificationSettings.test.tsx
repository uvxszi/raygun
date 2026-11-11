import type { SettingChange, Settings } from '@raygun/bindings';
import { screen, waitFor } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

describe('Notification settings', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    let current: Settings = DEFAULT_SETTINGS;
    commands.settingsGet.mockImplementation(async () => ok(current));
    commands.settingsUpdate.mockImplementation(async (change: SettingChange) => {
      current = { ...current, [change.key]: change.value };
      return ok(current);
    });
  });

  it('turns each kind of news on or off on its own', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/notifications');
    const books = await screen.findByRole('switch', { name: /New book chapters/ });
    expect(books).toBeChecked();
    expect(screen.getByRole('switch', { name: /New versions of Raygun/ })).toBeChecked();
    await user.click(books);
    await waitFor(() =>
      expect(commands.settingsUpdate).toHaveBeenCalledWith({
        key: 'notifications',
        value: expect.objectContaining({ books: false, comics: true, appUpdates: true }),
      }),
    );
  });
});
