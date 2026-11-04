import type { LocalFolder } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const MEDIA: LocalFolder = {
  id: 'f1',
  name: 'Media',
  location: 'C:/Users/me/Media',
  counts: { songs: 12, comics: 3, books: 1, videos: 0, failed: 0, unplayable: 0 },
  scannedAt: 1,
  scanning: false,
  progress: null,
  needsAccess: false,
  missing: false,
};

describe('Local Files (M46.27)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('adds a folder, follows its scan, and removes it after asking', async () => {
    const user = userEvent.setup();
    commands.localFolderAdd.mockResolvedValue(ok([{ ...MEDIA, scanning: true }]));
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    expect(await within(page).findByText('No folders yet')).toBeInTheDocument();
    // What a folder can hold, as chips rather than a paragraph.
    expect(within(page).getByRole('list', { name: 'What a folder can hold' })).toHaveTextContent(
      'EPUB books',
    );

    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const folder = await within(page).findByRole('listitem', { name: 'Media' });
    expect(folder).toHaveTextContent('Looking for files…');

    // The scan adds comics and books, one by one.
    commands.localFolders.mockResolvedValue(
      ok([{ ...MEDIA, scanning: true, progress: { done: 1, total: 4 } }]),
    );
    emitEvent('localFilesEvent', 'f1');
    await vi.waitFor(() => expect(folder).toHaveTextContent('Adding 1 of 4…'));

    // The scan finished.
    commands.localFolders.mockResolvedValue(ok([MEDIA]));
    emitEvent('localFilesEvent', 'f1');
    await vi.waitFor(() => expect(folder).toHaveTextContent('12 songs · 3 comics · 1 book'));

    // Removing takes a second tap.
    await user.click(within(folder).getByRole('button', { name: 'Remove Media' }));
    expect(commands.localFolderRemove).not.toHaveBeenCalled();
    await user.click(within(folder).getByRole('button', { name: 'Remove' }));
    expect(commands.localFolderRemove).toHaveBeenCalledWith('f1');
  });

  it('says what a folder needs: media access, a new pick, files to try again', async () => {
    const user = userEvent.setup();
    commands.localFolders.mockResolvedValue(
      ok([
        {
          ...MEDIA,
          counts: { ...MEDIA.counts, failed: 2, unplayable: 3 },
          needsAccess: true,
          missing: true,
        },
      ]),
    );
    await AppShellWrapper.mount('/local-files');
    const folder = await screen.findByRole('listitem', { name: 'Media' });

    expect(folder).toHaveTextContent("Can't open this folder");
    expect(folder).toHaveTextContent("2 files couldn't be added");
    expect(folder).toHaveTextContent("3 songs and videos can't play from this folder");

    await user.click(within(folder).getByRole('button', { name: 'Allow access' }));
    expect(commands.localFilesAllowAccess).toHaveBeenCalled();
    await user.click(within(folder).getByRole('button', { name: 'Pick it again' }));
    expect(commands.localFolderAdd).toHaveBeenCalled();
    await user.click(within(folder).getByRole('button', { name: 'Try again' }));
    expect(commands.localFoldersRescan).toHaveBeenCalledWith('f1');

    commands.localFoldersRescan.mockClear();
    await user.click(within(folder).getByRole('button', { name: 'Scan Media again' }));
    expect(commands.localFoldersRescan).toHaveBeenCalledWith('f1');
    await user.click(screen.getByRole('button', { name: 'Scan the folders again' }));
    expect(commands.localFoldersRescan).toHaveBeenCalledWith(null);
  });

  it('leads to where each kind went, with how many', async () => {
    commands.localFolders.mockResolvedValue(ok([MEDIA]));
    await AppShellWrapper.mount('/local-files');
    const kinds = await screen.findByRole('navigation', { name: 'Open in the library' });
    const comics = within(kinds).getByRole('link', { name: /Comics/ });
    expect(comics).toHaveTextContent('Comics3');
    expect(comics).toHaveAttribute('href', '/library?media=manga');
    expect(within(kinds).getByRole('link', { name: /Songs/ })).toHaveAttribute(
      'href',
      '/library?view=tracks',
    );
    // Nothing of a kind: no link to it.
    expect(within(kinds).queryByRole('link', { name: /Videos/ })).not.toBeInTheDocument();
  });

  it('says calmly when a folder could not be added', async () => {
    const user = userEvent.setup();
    commands.localFolderAdd.mockResolvedValue({ status: 'error', error: 'database is locked' });
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const issue = await within(page).findByTestId('source-issue');
    expect(issue).toHaveTextContent("Couldn't add the folder");
    expect(within(issue).getByText('database is locked')).not.toBeVisible();
  });

  it('refreshes Comics, Books and Videos once a scan ends, not at every step', async () => {
    commands.localFolders.mockResolvedValue(ok([{ ...MEDIA, scanning: true }]));
    await AppShellWrapper.mount('/library?media=all');
    await screen.findByTestId('library-all');
    await vi.waitFor(() => expect(commands.videoLibrary).toHaveBeenCalled());
    commands.videoLibrary.mockClear();
    commands.novelLibrary.mockClear();

    // A step of the scan: the folders refresh, the libraries don't.
    const asked = commands.localFolders.mock.calls.length;
    emitEvent('localFilesEvent', 'f1');
    await vi.waitFor(() => expect(commands.localFolders.mock.calls.length).toBeGreaterThan(asked));
    expect(commands.videoLibrary).not.toHaveBeenCalled();

    commands.localFolders.mockResolvedValue(ok([MEDIA]));
    emitEvent('localFilesEvent', 'f1');
    await vi.waitFor(() => expect(commands.videoLibrary).toHaveBeenCalled());
    expect(commands.novelLibrary).toHaveBeenCalled();
  });

  it('is a green block in the Library, always there (M46.29)', async () => {
    commands.localFolders.mockResolvedValue(ok([MEDIA]));
    await AppShellWrapper.mount('/library?media=music');
    const overview = await screen.findByTestId('music-overview');
    expect(await within(overview).findByRole('link', { name: /Local files/ })).toHaveAttribute(
      'href',
      expect.stringContaining('/local-files'),
    );
    expect(within(overview).getByRole('link', { name: /Downloaded/ })).toBeInTheDocument();
    expect(await within(overview).findByText('16 items')).toBeInTheDocument();

    commands.localFolders.mockResolvedValue(ok([{ ...MEDIA, scanning: true }]));
    emitEvent('localFilesEvent', 'f1');
    expect(await within(overview).findByText('Scanning…')).toBeInTheDocument();
  });
});
