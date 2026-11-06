import type { BackupEntry, DataUsage } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { BACKUP_CATEGORIES } from './dataLabels';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const ID = 'content://media/external/downloads/7';
const CREATED_AT = Date.UTC(2026, 9, 31, 12);

const found: BackupEntry = {
  id: ID,
  name: 'raygun-20261031-120000.raygunbk',
  place: 'downloads',
  folder: 'Download/Raygun',
  sizeBytes: 40 * 1024,
  modifiedAt: CREATED_AT,
  problem: null,
  manifest: {
    version: 2,
    createdAt: CREATED_AT,
    deviceId: 'device-a',
    deviceName: 'Pixel 4',
    appVersion: '1.2.0',
    platform: 'android',
    secrets: true,
    sections: [
      {
        category: 'settings',
        counts: [{ item: 'settings', count: 5, bytes: null }],
      },
      {
        category: 'comics',
        counts: [
          { item: 'titles', count: 3, bytes: null },
          { item: 'chapters', count: 40, bytes: null },
        ],
      },
    ],
  },
};

const usage: DataUsage[] = [
  {
    category: 'comics',
    counts: [{ item: 'titles', count: 12, bytes: null }],
    bytes: 0,
    scope: 'device',
  },
];

describe('Backup card', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('backs up the chosen categories in a folder picked then, which is remembered', async () => {
    const user = userEvent.setup();
    commands.dataUsage.mockResolvedValue(ok(usage));
    const picked = {
      usual: 'Download/Raygun',
      usualPlace: 'downloads' as const,
      folders: [{ uri: 'content://tree/backups', name: 'Backups', usedAt: 1 }],
      last: 'content://tree/backups',
    };
    commands.backupPickFolder.mockResolvedValue(ok(picked));
    commands.backupCreate.mockResolvedValue(
      ok({ ...found, folder: 'Backups', place: 'folder' as const }),
    );
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('backup-card');

    await user.click(within(card).getByRole('button', { name: /Create backup/ }));
    const sheet = await screen.findByTestId('create-backup');
    // Everything is chosen at first, each with what it holds here.
    const comics = within(sheet).getByRole('checkbox', { name: /Comics/ });
    expect(comics).toBeChecked();
    expect(await within(sheet).findByText('12 titles')).toBeInTheDocument();
    expect(within(sheet).getByRole('radio', { name: /Save to the usual place/ })).toBeChecked();

    await user.click(within(sheet).getByRole('checkbox', { name: /Search history/ }));
    await user.click(within(sheet).getByRole('checkbox', { name: /Include passwords and keys/ }));
    await user.click(within(sheet).getByRole('button', { name: 'Choose a folder…' }));
    expect(await within(sheet).findByRole('radio', { name: 'Backups' })).toBeChecked();
    await user.click(within(sheet).getByRole('button', { name: 'Create' }));

    expect(commands.backupCreate).toHaveBeenCalledWith({
      categories: BACKUP_CATEGORIES.filter((category) => category !== 'searches'),
      secrets: true,
      folder: 'content://tree/backups',
    });
    expect(await within(card).findByText(`Saved ${found.name} in Backups`)).toBeInTheDocument();
  });

  it("shows a found backup's index before restoring the categories chosen", async () => {
    const user = userEvent.setup();
    commands.backupScan.mockResolvedValue(ok([found]));
    commands.backupRestore.mockResolvedValue(
      ok({
        kind: 'raygun',
        received: 40,
        changed: 3,
        queued: 2,
        problems: ['1 video wasn’t downloaded again'],
      }),
    );
    await AppShellWrapper.mount('/settings/data');
    const card = await screen.findByTestId('backup-card');

    const row = await within(card).findByTestId('backup-row');
    expect(row).toHaveTextContent('Pixel 4 · Raygun 1.2.0');
    expect(row).toHaveTextContent('Comics, Settings');
    expect(row).toHaveTextContent('40 KB · Download/Raygun');
    await user.click(within(row).getByRole('button', { name: /Open the backup from/ }));

    const sheet = await screen.findByTestId('restore-backup');
    expect(within(sheet).getByText('Holds passwords and keys')).toBeInTheDocument();
    const sections = within(sheet).getAllByTestId('backup-section');
    // Listed in Settings → Data's order, whatever the file's.
    expect(sections[0]).toHaveTextContent('Comics3 titles · 40 chapters');
    expect(sections[1]).toHaveTextContent('Settings5 settings changed');
    expect(commands.backupRestore).not.toHaveBeenCalled();

    await user.click(within(sheet).getByRole('checkbox', { name: /Settings/ }));
    await user.click(within(sheet).getByRole('button', { name: 'Restore' }));
    expect(commands.backupRestore).toHaveBeenCalledWith(ID, ['comics']);
    expect(await within(card).findByText('Restored. 3 items changed.')).toBeInTheDocument();
    expect(within(card).getByText('2 downloads, installs and folder scans started again.')).toBeInTheDocument();
    expect(within(card).getByText('1 video wasn’t downloaded again')).toBeInTheDocument();
  });

  it('deletes a found backup only after asking', async () => {
    const user = userEvent.setup();
    commands.backupScan.mockResolvedValue(ok([found]));
    await AppShellWrapper.mount('/settings/data');
    const row = await screen.findByTestId('backup-row');

    await user.click(within(row).getByRole('button', { name: /Delete the backup from/ }));
    expect(commands.backupDelete).not.toHaveBeenCalled();
    await user.click(within(row).getByRole('button', { name: 'Delete' }));
    expect(commands.backupDelete).toHaveBeenCalledWith(ID);
  });

  it('says why a backup it found cannot be opened', async () => {
    commands.backupScan.mockResolvedValue(
      ok([{ ...found, manifest: null, problem: 'The backup’s description is damaged' }]),
    );
    await AppShellWrapper.mount('/settings/data');
    const row = await screen.findByTestId('backup-row');
    expect(row).toHaveTextContent('The backup’s description is damaged');
    expect(within(row).getByRole('button', { name: /Open the backup from/ })).toBeDisabled();
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
