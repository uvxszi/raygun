import type { AppPermission } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { PermissionPromptHost } from '../permissions/PermissionPrompt';
import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';
import { renderWithProviders } from '../test/renderWithProviders';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const { commands } = bindingsMock;
const ok = <T,>(data: T) => ({ status: 'ok' as const, data });

const ANDROID: AppPermission[] = [
  { kind: 'notifications', state: 'denied', count: null },
  { kind: 'battery', state: 'restricted', count: null },
  { kind: 'media', state: 'granted', count: null },
  { kind: 'folders', state: 'granted', count: 2 },
  { kind: 'installApps', state: 'blocked', count: null },
];

describe('Settings → Permissions', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.permissionsList.mockResolvedValue(ok(ANDROID));
  });

  it('lists each permission with its state and the way to change it', async () => {
    await AppShellWrapper.mount('/settings/permissions');
    const notifications = await screen.findByTestId('permission-notifications');
    expect(within(notifications).getByText('Not allowed')).toBeInTheDocument();
    expect(
      within(notifications).getByRole('button', { name: 'Allow: Notifications' }),
    ).toBeInTheDocument();
    const battery = screen.getByTestId('permission-battery');
    expect(within(battery).getByText(/music and downloads stop/)).toBeInTheDocument();
    expect(within(battery).getByRole('button', { name: /Open settings/ })).toBeInTheDocument();
    // Allowed: nothing to press.
    expect(within(screen.getByTestId('permission-media')).queryByRole('button')).toBeNull();
    expect(
      within(screen.getByTestId('permission-folders')).getByText('2 folders'),
    ).toBeInTheDocument();
  });

  it('explains the optional access to all files and opens its page (M46.51)', async () => {
    const user = userEvent.setup();
    commands.permissionsList.mockResolvedValue(
      ok([...ANDROID, { kind: 'allFiles', state: 'denied', count: null }]),
    );
    await AppShellWrapper.mount('/settings/permissions');
    const allFiles = await screen.findByTestId('permission-allFiles');
    expect(allFiles).toHaveTextContent('All files (optional)');
    expect(allFiles).toHaveTextContent('EPUB books and CBZ or ZIP comics in any folder');
    await user.click(within(allFiles).getByRole('button', { name: 'Allow: All files (optional)' }));
    expect(commands.permissionRequest).toHaveBeenCalledWith('allFiles');
  });

  it('asks Android and shows the answer', async () => {
    const user = userEvent.setup();
    commands.permissionRequest.mockResolvedValue(
      ok(
        ANDROID.map((permission) =>
          permission.kind === 'notifications'
            ? { ...permission, state: 'granted' as const }
            : permission,
        ),
      ),
    );
    await AppShellWrapper.mount('/settings/permissions');
    await user.click(await screen.findByRole('button', { name: 'Allow: Notifications' }));
    expect(commands.permissionRequest).toHaveBeenCalledWith('notifications');
    const notifications = screen.getByTestId('permission-notifications');
    expect(await within(notifications).findByText('Allowed')).toBeInTheDocument();
  });
});

describe('a feature asking for a permission', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('says why when Rust asks, and remembers Not now', async () => {
    const user = userEvent.setup();
    renderWithProviders(<PermissionPromptHost />);
    emitEvent('permissionPromptEvent', { kind: 'notifications', reason: 'download' });
    expect(
      await screen.findByRole('dialog', { name: 'Show downloads in notifications' }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Not now' }));
    expect(commands.permissionPromptAnswer).toHaveBeenCalledWith(
      { kind: 'notifications', reason: 'download' },
      false,
    );
    expect(screen.queryByRole('dialog')).toBeNull();
  });

  it('shows a question raised before it listened', async () => {
    const user = userEvent.setup();
    commands.permissionPromptPending.mockResolvedValue({ kind: 'battery', reason: 'playback' });
    renderWithProviders(<PermissionPromptHost />);
    expect(
      await screen.findByRole('dialog', { name: 'Keep playing in the background' }),
    ).toBeInTheDocument();
    await user.click(screen.getByRole('button', { name: 'Allow' }));
    expect(commands.permissionPromptAnswer).toHaveBeenCalledWith(
      { kind: 'battery', reason: 'playback' },
      true,
    );
  });
});
