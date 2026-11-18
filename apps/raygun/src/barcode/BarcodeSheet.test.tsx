import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const openSheet = async () => {
  const user = userEvent.setup();
  await AppShellWrapper.mount('/browse?tab=search');
  await user.click(await screen.findByRole('button', { name: 'Scan a barcode' }));
  const sheet = await screen.findByRole('dialog', { name: 'Scan a barcode' });
  return { user, sheet };
};

describe('Scanning barcodes', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('looks up a typed CD barcode and offers its album', async () => {
    commands.barcodeLookup.mockResolvedValue(
      ok({
        kind: 'album',
        code: '0720642442517',
        title: 'Nevermind',
        artist: 'Nirvana',
        year: 1991,
        matches: [],
      }),
    );
    const { user, sheet } = await openSheet();
    const field = within(sheet).getByRole('textbox', { name: 'Or type the number' });
    await user.type(field, '1234');
    expect(within(sheet).getByRole('button', { name: 'Look up' })).toBeDisabled();
    await user.clear(field);
    await user.type(field, '720642442517');
    await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
    expect(commands.barcodeLookup).toHaveBeenCalledWith('720642442517');
    expect(await within(sheet).findByText('Nevermind by Nirvana (1991)')).toBeVisible();
    // No source has the album: Search looks for it.
    expect(within(sheet).queryByRole('button', { name: 'Open the album' })).toBeNull();
    await user.click(within(sheet).getByRole('button', { name: 'Search for the album' }));
    expect(screen.queryByRole('dialog', { name: 'Scan a barcode' })).toBeNull();
  });

  it('finds a book by its ISBN in the book sources', async () => {
    commands.barcodeLookup.mockResolvedValue(
      ok({
        kind: 'book',
        code: '9780306406157',
        title: 'Programming',
        authors: ['Ada Lovelace'],
        coverUrl: null,
      }),
    );
    const { user, sheet } = await openSheet();
    await user.type(
      within(sheet).getByRole('textbox', { name: 'Or type the number' }),
      '978-0-306-40615-7',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Look up' }));
    expect(commands.barcodeLookup).toHaveBeenCalledWith('9780306406157');
    expect(await within(sheet).findByText('Ada Lovelace')).toBeVisible();
    await user.click(within(sheet).getByRole('button', { name: 'Find the book in your sources' }));
    expect(screen.queryByRole('dialog', { name: 'Scan a barcode' })).toBeNull();
  });
});
