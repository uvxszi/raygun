import type { LocalBrowse, LocalFolder } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, bindingsMock, emitEvent } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const MEDIA: LocalFolder = {
  id: 'f1',
  name: 'Media',
  location: '/storage/emulated/0/Media',
  access: 'path',
  counts: { songs: 12, comics: 3, books: 1, videos: 0, failed: 0, unplayable: 0 },
  scannedAt: 1,
  scanning: false,
  progress: null,
  needsAccess: false,
  missing: false,
};

const ROOT = '/storage/emulated/0';

/** The phone's storage at its top, read with the media permissions only. */
const TOP: LocalBrowse = {
  path: ROOT,
  name: '',
  trail: [],
  folders: [
    { name: 'Download', path: `${ROOT}/Download`, items: 5 },
    { name: 'Media', path: `${ROOT}/Media`, items: 16 },
  ],
  items: 0,
  view: 'media',
  canAllowAllFiles: true,
  added: false,
};

const DOWNLOAD: LocalBrowse = {
  ...TOP,
  path: `${ROOT}/Download`,
  name: 'Download',
  trail: [{ name: 'Download', path: `${ROOT}/Download` }],
  folders: [{ name: 'Shows', path: `${ROOT}/Download/Shows`, items: 2 }],
  items: 3,
};

const MEDIA_FOLDER: LocalBrowse = {
  ...TOP,
  path: `${ROOT}/Media`,
  name: 'Media',
  trail: [{ name: 'Media', path: `${ROOT}/Media` }],
  folders: [],
  items: 16,
};

/** Answers the browser like the phone would, folder by folder. */
const browseStorage = (listings: LocalBrowse[]) =>
  commands.localFilesBrowse.mockImplementation(async (path: string | null) =>
    ok(listings.find((listing) => listing.path === (path ?? ROOT)) ?? TOP),
  );

describe('Local Files (M46.27)', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('adds a folder, follows its scan, and removes it after asking', async () => {
    const user = userEvent.setup();
    browseStorage([TOP, MEDIA_FOLDER]);
    commands.localFolderAddPath.mockResolvedValue(ok([{ ...MEDIA, scanning: true }]));
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
    const browser = await screen.findByRole('dialog', { name: 'Add a folder' });
    await user.click(await within(browser).findByRole('button', { name: /Media/ }));
    await within(browser).findByText('16 items right in this folder');
    await user.click(within(browser).getByRole('button', { name: 'Use this folder' }));
    expect(commands.localFolderAddPath).toHaveBeenCalledWith(`${ROOT}/Media`);
    await vi.waitFor(() => expect(browser).not.toBeInTheDocument());
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
          // Picked in Android's folder picker: picked there again.
          access: 'tree',
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
    browseStorage([TOP]);
    commands.localFolderAddPath.mockResolvedValue({ status: 'error', error: 'database is locked' });
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const browser = await screen.findByRole('dialog', { name: 'Add a folder' });
    await within(browser).findByRole('list', { name: 'Folders' });
    await user.click(within(browser).getByRole('button', { name: 'Use this folder' }));
    const issue = await within(browser).findByTestId('source-issue');
    expect(issue).toHaveTextContent("Couldn't add the folder");
    expect(within(issue).getByText('database is locked')).not.toBeVisible();
    // The browser stays open to choose again.
    expect(browser).toBeInTheDocument();
  });

  it('browses the whole storage, Download too, and says what each folder holds (M46.51)', async () => {
    const user = userEvent.setup();
    browseStorage([TOP, DOWNLOAD]);
    commands.localFolderAddPath.mockResolvedValue(
      ok([{ ...MEDIA, id: 'd', name: 'Download', location: `${ROOT}/Download`, scanning: true }]),
    );
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const browser = await screen.findByRole('dialog', { name: 'Add a folder' });
    const trail = within(browser).getByRole('navigation', { name: 'Folder path' });
    const folders = await within(browser).findByRole('list', { name: 'Folders' });
    expect(commands.localFilesBrowse).toHaveBeenCalledWith(null);
    expect(within(folders).getByRole('button', { name: /Download/ })).toHaveTextContent(
      'Download5 items',
    );
    expect(within(trail).getByText('Internal storage')).toHaveAttribute('aria-current', 'location');
    // With the media permissions only, books and archives wait for access to all files.
    expect(browser).toHaveTextContent('EPUB books and comic archives (CBZ, ZIP) show once');

    await user.click(within(folders).getByRole('button', { name: /Download/ }));
    await within(browser).findByText('3 items right in this folder');
    expect(commands.localFilesBrowse).toHaveBeenCalledWith(`${ROOT}/Download`);
    expect(within(trail).getByText('Download')).toHaveAttribute('aria-current', 'location');
    expect(within(browser).getByRole('button', { name: /Shows/ })).toHaveTextContent('2 items');

    // The trail goes back up.
    await user.click(within(trail).getByRole('button', { name: 'Internal storage' }));
    await within(browser).findByRole('button', { name: /Media/ });
    await user.click(within(browser).getByRole('button', { name: /Download/ }));
    await within(browser).findByText('3 items right in this folder');
    await user.click(within(browser).getByRole('button', { name: 'Use this folder' }));
    expect(commands.localFolderAddPath).toHaveBeenCalledWith(`${ROOT}/Download`);
    expect(await within(page).findByRole('listitem', { name: 'Download' })).toHaveTextContent(
      'Looking for files…',
    );
  });

  it('asks for access to all files only when asked, and lists the folder again', async () => {
    const user = userEvent.setup();
    browseStorage([TOP]);
    commands.permissionRequest.mockImplementation(async () => {
      browseStorage([{ ...TOP, view: 'allFiles', canAllowAllFiles: false }]);
      return ok([{ kind: 'allFiles', state: 'granted', count: null }]);
    });
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const browser = await screen.findByRole('dialog', { name: 'Add a folder' });
    await within(browser).findByRole('list', { name: 'Folders' });
    expect(commands.permissionRequest).not.toHaveBeenCalled();

    await user.click(within(browser).getByRole('button', { name: 'Allow access to all files' }));
    expect(commands.permissionRequest).toHaveBeenCalledWith('allFiles');
    await vi.waitFor(() =>
      expect(browser).not.toHaveTextContent('EPUB books and comic archives (CBZ, ZIP) show once'),
    );
  });

  it("says how to see the phone's folders without media access", async () => {
    const user = userEvent.setup();
    commands.localFilesBrowse.mockResolvedValue(ok({ ...TOP, folders: [], view: 'none' }));
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const browser = await screen.findByRole('dialog', { name: 'Add a folder' });
    expect(
      await within(browser).findByText(
        "Allow access to the phone's songs, videos and pictures to see its folders.",
      ),
    ).toBeInTheDocument();
    await user.click(within(browser).getByRole('button', { name: 'Allow access' }));
    expect(commands.permissionRequest).toHaveBeenCalledWith('media');
  });

  it("keeps Android's own folder picker for SD cards and USB drives", async () => {
    const user = userEvent.setup();
    browseStorage([TOP]);
    commands.localFolderAdd.mockResolvedValue(ok([{ ...MEDIA, access: 'tree' }]));
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const browser = await screen.findByRole('dialog', { name: 'Add a folder' });
    await user.click(within(browser).getByRole('button', { name: 'Use system picker' }));
    expect(commands.localFolderAdd).toHaveBeenCalled();
    expect(browser).not.toBeInTheDocument();
    expect(await within(page).findByRole('listitem', { name: 'Media' })).toBeInTheDocument();
  });

  it('a folder already added says so instead of adding it twice', async () => {
    browseStorage([{ ...TOP, added: true }]);
    const user = userEvent.setup();
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    const browser = await screen.findByRole('dialog', { name: 'Add a folder' });
    expect(
      await within(browser).findByRole('button', { name: 'Already in Local files' }),
    ).toBeDisabled();
  });

  it('adds single files from anywhere and says where each kind went', async () => {
    const user = userEvent.setup();
    commands.localFilesAddFiles.mockResolvedValue(
      ok({
        songs: 2,
        videos: 0,
        comics: 0,
        books: 1,
        failed: [{ name: 'notes.txt', error: "Raygun doesn't play or read this kind of file" }],
        folders: [
          {
            ...MEDIA,
            id: 'added',
            name: 'Added files',
            location: '/data/user/0/app.raygun.player/downloads/added',
            access: 'added',
            counts: { ...MEDIA.counts, songs: 2, comics: 0, books: 0 },
            scanning: true,
          },
        ],
      }),
    );
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    await user.click(within(page).getAllByRole('button', { name: 'Add files' })[0] as HTMLElement);
    expect(commands.localFilesAddFiles).toHaveBeenCalled();

    const book = await within(page).findByRole('link', { name: '1 book' });
    expect(book).toHaveAttribute('href', '/library?media=novel');
    const added = book.closest('[role="status"]') as HTMLElement;
    expect(added).toHaveTextContent('Added');
    expect(within(added).getByRole('link', { name: '2 songs' })).toHaveAttribute(
      'href',
      '/library?view=tracks',
    );
    expect(within(page).getByTestId('source-issue')).toHaveTextContent("1 file couldn't be added");
    // Songs and videos are kept in Raygun's own folder.
    const folder = within(page).getByRole('listitem', { name: 'Added files' });
    expect(folder).toHaveTextContent('Copies of the songs and videos you added');
    await user.click(within(folder).getByRole('button', { name: 'Remove Added files' }));
    expect(
      within(folder).getByRole('button', { name: 'Remove and delete the copies' }),
    ).toBeInTheDocument();
  });

  it("keeps a desktop's folder dialog, with Add files beside it", async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.localFolderAdd.mockResolvedValue(ok([{ ...MEDIA, location: 'C:/Users/me/Media' }]));
    await AppShellWrapper.mount('/local-files');
    const page = await screen.findByTestId('local-files-page');
    // Known to be a desktop once its bar shows.
    await screen.findByTestId('app-bar');
    await user.click(
      within(page).getAllByRole('button', { name: 'Add a folder' })[0] as HTMLElement,
    );
    expect(await within(page).findByRole('listitem', { name: 'Media' })).toBeInTheDocument();
    expect(commands.localFolderAdd).toHaveBeenCalled();
    expect(commands.localFilesBrowse).not.toHaveBeenCalled();
    expect(screen.queryByRole('dialog', { name: 'Add a folder' })).not.toBeInTheDocument();

    await user.click(within(page).getByRole('button', { name: 'Add files' }));
    expect(commands.localFilesAddFiles).toHaveBeenCalled();
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
