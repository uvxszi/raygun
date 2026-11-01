import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const NAME = 'raygun-20261031-120000.raygunbk';

describe('Backup card', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('creates a backup, restores it after confirming and deletes it', async () => {
    const user = userEvent.setup();
    const file = { name: NAME, sizeBytes: 4096, createdAt: Date.UTC(2026, 9, 31, 12) };
    commands.backupList.mockResolvedValue(ok([file]));
    commands.backupRestore.mockResolvedValue(ok({ kind: 'raygun', received: 40, changed: 3 }));
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('backup-card');

    await user.click(within(card).getByRole('button', { name: 'Create backup' }));
    expect(commands.backupCreate).toHaveBeenCalled();
    expect(await within(card).findByText(`Saved ${NAME}`)).toBeInTheDocument();

    const row = await within(card).findByTestId('backup-row');
    expect(row).toHaveTextContent('4 KB');
    await user.click(within(row).getByRole('button', { name: /Restore the backup from/ }));
    expect(commands.backupRestore).not.toHaveBeenCalled();
    await user.click(within(row).getByRole('button', { name: 'Restore' }));
    expect(commands.backupRestore).toHaveBeenCalledWith(NAME);
    expect(await within(card).findByText('Restored. 3 items changed.')).toBeInTheDocument();

    await user.click(within(row).getByRole('button', { name: /Delete the backup from/ }));
    expect(commands.backupDelete).toHaveBeenCalledWith(NAME);
  });

  it('imports a picked Mihon backup and reports skipped sources', async () => {
    const user = userEvent.setup();
    commands.backupImport.mockResolvedValue(
      ok({
        kind: 'mihon',
        summary: {
          titles: 2,
          libraryTitles: 2,
          chapters: 30,
          readChapters: 12,
          skipped: [{ name: 'Some Scanlator', titles: 4 }],
        },
      }),
    );
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('backup-card');

    const picked = new File([new Uint8Array([0x1f, 0x8b, 1, 2])], 'mihon.tachibk');
    await user.upload(within(card).getByLabelText('Backup file (Raygun or Mihon)'), picked);

    expect(
      await within(card).findByText('Imported 2 titles with 30 chapters (12 read) from Mihon.'),
    ).toBeInTheDocument();
    expect(
      within(card).getByText('Skipped, no matching source: Some Scanlator (4)'),
    ).toBeInTheDocument();
    expect(commands.backupImport).toHaveBeenCalledWith('H4sBAg==');
  });
});
