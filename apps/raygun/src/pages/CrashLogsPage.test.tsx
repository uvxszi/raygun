import type { CrashLogs } from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { NORMAL_START, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const LOGS: CrashLogs = {
  records: [
    {
      id: '1700000000000-renderer.txt',
      kind: 'renderer',
      at: 1_700_000_000_000,
      summary: 'The page’s renderer crashed',
    },
    {
      id: '1600000000000-panic.txt',
      kind: 'panic',
      at: 1_600_000_000_000,
      summary: 'index out of bounds (crates/x.rs:1:2)',
    },
  ],
  heldBack: [{ id: 'dev.broken', name: 'Broken Source' }],
  folder: '/data/logs',
  safeStart: NORMAL_START,
};

describe('About → Crash logs', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.crashLogs.mockResolvedValue(ok(LOGS));
    commands.crashLog.mockResolvedValue(ok('Raygun crash record\nKind: renderer\n\nThe details'));
  });

  it('lists what went wrong, newest first, and opens one to share or save', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/about/crashes');
    const rows = await screen.findAllByTestId('crash-record');
    expect(rows.map((row) => row.textContent)).toEqual([
      expect.stringContaining('The screen stopped'),
      expect.stringContaining('Part of Raygun stopped'),
    ]);
    await user.click(rows[0]!);
    const text = await screen.findByTestId('crash-log-text');
    await waitFor(() => expect(text).toHaveTextContent('The details'));
    const sheet = within(screen.getByRole('dialog'));
    await user.click(sheet.getByRole('button', { name: 'Share' }));
    expect(commands.crashLogShare).toHaveBeenCalledWith('1700000000000-renderer.txt');
    await user.click(sheet.getByRole('button', { name: 'Save' }));
    expect(await sheet.findByText('Saved in Download/Raygun/logs/x.txt')).toBeVisible();
  });

  it('turns a held-back source on again, and clears the logs only on a second tap', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/about/crashes');
    await user.click(await screen.findByRole('button', { name: /Broken Source/ }));
    expect(commands.extensionHoldBack).toHaveBeenCalledWith('dev.broken', false);

    await user.click(screen.getByRole('button', { name: 'Clear crash logs' }));
    expect(commands.crashLogsClear).not.toHaveBeenCalled();
    await user.click(screen.getByRole('button', { name: 'Tap again to clear every crash log' }));
    await waitFor(() => expect(commands.crashLogsClear).toHaveBeenCalled());
  });

  it('opens the record a crash screen points to', async () => {
    await AppShellWrapper.mount('/about/crashes?id=1600000000000-panic.txt');
    await waitFor(() => expect(commands.crashLog).toHaveBeenCalledWith('1600000000000-panic.txt'));
    expect(await screen.findByTestId('crash-log-text')).toBeVisible();
  });

  it('says when this start is a safe one after a crash', async () => {
    const safe = {
      active: true,
      reason: 'stopped' as const,
      crashes: 1,
      suspects: [{ id: 'dev.suspect', name: 'Suspect Source' }],
      heldNow: [],
    };
    commands.safeStartStatus.mockResolvedValue(safe);
    commands.crashLogs.mockResolvedValue(ok({ ...LOGS, safeStart: safe }));
    const user = userEvent.setup();
    await AppShellWrapper.mount('/about/crashes');
    const notice = await screen.findByTestId('safe-start-notice');
    expect(notice).toHaveTextContent('Raygun stopped while it last started');
    await user.click(within(notice).getByRole('button', { name: 'Turn off' }));
    expect(commands.extensionHoldBack).toHaveBeenCalledWith('dev.suspect', true);
    await user.click(within(notice).getByRole('button', { name: 'Close' }));
    expect(screen.queryByTestId('safe-start-notice')).not.toBeInTheDocument();
    expect(screen.getByText('This is a safe start')).toBeVisible();
  });
});
