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

describe('Barcodes read from a picture', () => {
  const book = '9780306406157';
  const other = '4006381333931';
  // What the browser's reader answers for each size of the picture it is given.
  let frames: { rawValue: string; format?: string }[][] = [];

  beforeEach(() => {
    AppShellWrapper.seedCore();
    vi.stubGlobal(
      'createImageBitmap',
      vi.fn(async () => ({ width: 800, height: 600, close: () => undefined })),
    );
    vi.spyOn(HTMLCanvasElement.prototype, 'getContext').mockReturnValue({
      drawImage: () => undefined,
    } as unknown as CanvasRenderingContext2D);
    vi.stubGlobal(
      'BarcodeDetector',
      class {
        static getSupportedFormats = async () => ['ean_13', 'upc_a', 'ean_8', 'upc_e'];
        detect = async () => frames.shift() ?? [];
      },
    );
    commands.barcodeLookup.mockImplementation(async (code: string) =>
      ok(
        code === book
          ? { kind: 'book', code, title: 'Programming', authors: ['Ada Lovelace'], coverUrl: null }
          : { kind: 'unknown', code },
      ),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
    vi.restoreAllMocks();
  });

  const upload = async () => {
    const { user, sheet } = await openSheet();
    await user.upload(
      within(sheet).getByLabelText('Choose a picture', { selector: 'input' }),
      new File(['x'], 'back.jpg', { type: 'image/jpeg' }),
    );
    return { user, sheet };
  };

  it('offers the codes read to pick from, leaving out misreads', async () => {
    frames = [
      [
        { rawValue: book, format: 'ean_13' },
        { rawValue: other, format: 'ean_13' },
      ],
      // A digit misread: its check digit fails, so it isn't offered.
      [{ rawValue: book }, { rawValue: '9780306406158' }],
    ];
    const { user, sheet } = await upload();
    expect(await within(sheet).findByText('Which of these codes is it?')).toBeVisible();
    expect(await within(sheet).findByText(`${book} · read 2 times`)).toBeVisible();
    expect(within(sheet).getByText(`${other} · read once`)).toBeVisible();
    expect(within(sheet).queryByText(/9780306406158/)).toBeNull();
    expect(await within(sheet).findByText('Nothing known about it')).toBeVisible();
    await user.click(await within(sheet).findByRole('button', { name: /Programming/ }));
    expect(
      within(sheet).getByRole('button', { name: 'Find the book in your sources' }),
    ).toBeVisible();
    // The other code stays one tap away.
    expect(within(sheet).getByText('Other codes read')).toBeVisible();
  });

  it('shows a code that clearly won at once, the others under it', async () => {
    frames = [
      [
        { rawValue: book, format: 'ean_13' },
        { rawValue: book, format: 'ean_13' },
        { rawValue: other, format: 'ean_13' },
      ],
      [
        { rawValue: book, format: 'ean_13' },
        { rawValue: book, format: 'ean_13' },
      ],
    ];
    const { sheet } = await upload();
    expect(
      await within(sheet).findByRole('button', { name: 'Find the book in your sources' }),
    ).toBeVisible();
    expect(within(sheet).queryByText('Which of these codes is it?')).toBeNull();
    expect(within(sheet).getByText('Other codes read')).toBeVisible();
    expect(within(sheet).getByText(`${other} · read once`)).toBeVisible();
  });

  it('says so when the picture has no valid code', async () => {
    frames = [[{ rawValue: '9780306406158', format: 'ean_13' }], []];
    const { sheet } = await upload();
    expect(await within(sheet).findByText('No barcode was found in that picture')).toBeVisible();
    expect(commands.barcodeLookup).not.toHaveBeenCalled();
  });
});
