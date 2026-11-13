import type {
  ExportJob,
  ExportRequest,
  MangaChapterInfo,
  MangaChapterPages,
  MangaTitleView,
} from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock, emitEvent } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';
import { SAVE_DEFAULTS } from './saveDefaults';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.example.comics/comics';

const chapter = (id: string, name: string): MangaChapterInfo => ({
  id,
  name,
  number: null,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
});

const TITLE: MangaTitleView = {
  id: 't1',
  hiddenNsfw: false,
  sourceId: SOURCE,
  sourceName: 'Comic Shelf',
  remoteId: 'm1',
  title: 'Frieren',
  coverUrl: null,
  authors: [],
  artists: [],
  description: null,
  genres: [],
  status: 'ongoing',
  url: null,
  inLibrary: true,
  chapters: [chapter('c3', 'Chapter 3'), chapter('c2', 'Chapter 2'), chapter('c1', 'Chapter 1')],
  continueChapterId: 'c1',
};

const job = (overrides: Partial<ExportJob>): ExportJob => ({
  id: 'j1',
  label: 'Frieren',
  medium: 'manga',
  format: 'cbz',
  destination: 'downloads',
  state: 'writing',
  downloaded: 0,
  toDownload: 0,
  written: 1,
  toWrite: 3,
  files: [],
  failedChapters: 0,
  skippedPages: 0,
  message: null,
  startedAt: 0,
  progress: null,
  ...overrides,
});

const lastRequest = (): ExportRequest =>
  commands.exportPlan.mock.calls[commands.exportPlan.mock.calls.length - 1]?.[0] as ExportRequest;

describe('Exports', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
  });

  it('exports a title: chapters, format, place, and what downloads first', async () => {
    const user = userEvent.setup();
    commands.mangaTitle.mockResolvedValue(ok(TITLE));
    commands.exportPlan.mockImplementation((request: ExportRequest) =>
      Promise.resolve(
        ok(
          request.destination === 'share' && request.format === 'cbz'
            ? {
                chapters: 3,
                missing: 1,
                files: 3,
                problem: 'Sharing sends one file',
                conversions: [],
              }
            : {
                chapters: 3,
                missing: 1,
                files: request.format === 'pdf' ? 1 : 3,
                problem: null,
                conversions: [],
              },
        ),
      ),
    );
    commands.exportStart.mockResolvedValue(ok(job({ state: 'downloading' })));
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');
    await user.click(await within(page).findByRole('button', { name: 'Export' }));

    const sheet = await screen.findByRole('dialog', { name: 'Export' });
    expect(await within(sheet).findByRole('status')).toHaveTextContent(
      '3 chapters · 3 files. 1 chapter downloads first',
    );
    expect(lastRequest()).toEqual({
      medium: 'manga',
      titleIds: ['t1'],
      scope: { kind: 'all' },
      format: 'cbz',
      destination: 'downloads',
      folder: null,
      source: null,
    });
    // No folder is set in Settings: only Choose folder… offers one.
    expect(within(sheet).queryByRole('radio', { name: 'Your folder' })).not.toBeInTheDocument();
    expect(within(sheet).getByRole('button', { name: 'Choose folder…' })).toBeInTheDocument();

    await user.click(within(sheet).getByRole('radio', { name: 'From one chapter to another' }));
    const from = within(sheet).getByRole('combobox', { name: 'From' });
    expect(
      within(from)
        .getAllByRole('option')
        .map((option) => option.textContent),
    ).toEqual(['Chapter 1', 'Chapter 2', 'Chapter 3']);
    await user.selectOptions(within(sheet).getByRole('combobox', { name: 'To' }), 'c2');
    await vi.waitFor(() =>
      expect(lastRequest().scope).toEqual({ kind: 'range', from: 'c1', to: 'c2' }),
    );

    await user.click(within(sheet).getByRole('radio', { name: 'Share' }));
    expect(await within(sheet).findByRole('alert')).toHaveTextContent('Sharing sends one file');
    expect(within(sheet).getByRole('button', { name: 'Export' })).toBeDisabled();
    await user.click(within(sheet).getByRole('radio', { name: 'PDF' }));
    await vi.waitFor(() => expect(lastRequest().format).toBe('pdf'));
    expect(await within(sheet).findByRole('status')).toHaveTextContent('1 file.');

    await user.click(within(sheet).getByRole('button', { name: 'Export' }));
    expect(commands.exportStart).toHaveBeenCalledWith({
      medium: 'manga',
      titleIds: ['t1'],
      scope: { kind: 'range', from: 'c1', to: 'c2' },
      format: 'pdf',
      destination: 'share',
      folder: null,
      source: null,
    });
    expect(await within(sheet).findByText('Exporting Frieren.')).toBeInTheDocument();
    await user.click(within(sheet).getByRole('link', { name: 'See progress' }));
    expect(await screen.findByTestId('downloads-page')).toBeInTheDocument();
  });

  it('ticks the chapters to export: all, a range, or chosen ones', async () => {
    const user = userEvent.setup();
    commands.mangaTitle.mockResolvedValue(
      ok({
        ...TITLE,
        chapters: [
          chapter('c3', 'Chapter 3'),
          { ...chapter('c2', 'Chapter 2'), read: true },
          {
            ...chapter('c1', 'Chapter 1'),
            download: { state: 'done', pagesDone: 1, pagesTotal: 1, error: null },
          },
        ],
      }),
    );
    commands.exportPlan.mockResolvedValue(
      ok({ chapters: 2, missing: 0, files: 2, problem: null, conversions: [] }),
    );
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');
    await user.click(await within(page).findByRole('button', { name: 'Export' }));
    const sheet = await screen.findByRole('dialog', { name: 'Export' });
    const list = within(sheet).getByRole('list', { name: 'Chapters to export' });
    const box = (name: string) => within(list).getByRole('checkbox', { name: new RegExp(name) });
    // In reading order, all ticked at first.
    expect(
      within(list)
        .getAllByRole('checkbox')
        .map((item) => item.textContent),
    ).toEqual(['Chapter 1', 'Chapter 2', 'Chapter 3']);
    expect(box('Chapter 2')).toHaveAttribute('aria-checked', 'true');

    // Unread ones tick what they take.
    await user.click(within(sheet).getByRole('radio', { name: 'Unread chapters' }));
    expect(box('Chapter 2')).toHaveAttribute('aria-checked', 'false');
    await vi.waitFor(() => expect(lastRequest().scope).toEqual({ kind: 'unread' }));

    // Ticking a box makes the list one's own.
    await user.click(box('Chapter 3'));
    expect(within(sheet).getByRole('radio', { name: 'Chosen chapters' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await vi.waitFor(() => expect(lastRequest().scope).toEqual({ kind: 'chosen', ids: ['c1'] }));
    await user.click(within(sheet).getByRole('button', { name: 'Tick all' }));
    await vi.waitFor(() =>
      expect(lastRequest().scope).toEqual({ kind: 'chosen', ids: ['c1', 'c2', 'c3'] }),
    );
    await user.click(within(sheet).getByRole('button', { name: 'Clear' }));
    await vi.waitFor(() => expect(lastRequest().scope).toEqual({ kind: 'chosen', ids: [] }));

    // A range from–to ticks what's between.
    await user.click(within(sheet).getByRole('radio', { name: 'From one chapter to another' }));
    await user.selectOptions(within(sheet).getByRole('combobox', { name: 'From' }), 'c2');
    expect(box('Chapter 1')).toHaveAttribute('aria-checked', 'false');
    expect(box('Chapter 3')).toHaveAttribute('aria-checked', 'true');
    await vi.waitFor(() =>
      expect(lastRequest().scope).toEqual({ kind: 'range', from: 'c2', to: 'c3' }),
    );
  });

  it('saves to a folder chosen on the spot, offered again next time', async () => {
    const user = userEvent.setup();
    commands.mangaTitle.mockResolvedValue(ok(TITLE));
    commands.exportPlan.mockResolvedValue(
      ok({ chapters: 3, missing: 0, files: 3, problem: null, conversions: [] }),
    );
    commands.exportStart.mockResolvedValue(ok(job({})));
    commands.settingsGet.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        save: {
          ...SAVE_DEFAULTS,
          location: 'folder',
          folderUri: 'content://tree/comics',
          folderName: 'Comics',
        },
      }),
    );
    const picked = { uri: 'content://tree/usb', name: 'USB stick' };
    commands.exportPickFolder.mockResolvedValue(ok(picked));
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');
    await user.click(await within(page).findByRole('button', { name: 'Export' }));

    const sheet = await screen.findByRole('dialog', { name: 'Export' });
    // The Settings folder is chosen at first.
    expect(await within(sheet).findByRole('radio', { name: 'Comics' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await vi.waitFor(() =>
      expect(lastRequest()).toMatchObject({ destination: 'folder', folder: null }),
    );

    await user.click(within(sheet).getByRole('button', { name: 'Choose folder…' }));
    expect(commands.exportPickFolder).toHaveBeenCalled();
    expect(await within(sheet).findByRole('radio', { name: 'USB stick' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await vi.waitFor(() =>
      expect(lastRequest()).toMatchObject({ destination: 'folder', folder: picked }),
    );
    await user.click(within(sheet).getByRole('button', { name: 'Export' }));
    expect(commands.exportStart).toHaveBeenCalledWith(
      expect.objectContaining({ destination: 'folder', folder: picked }),
    );
  });

  it('keeps the last chosen folder as a choice beside the Settings default', async () => {
    const user = userEvent.setup();
    commands.mangaTitle.mockResolvedValue(ok(TITLE));
    commands.exportLastFolder.mockResolvedValue(ok({ uri: '/media/usb', name: 'usb' }));
    commands.exportPickFolder.mockResolvedValue(ok(null));
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');
    await user.click(await within(page).findByRole('button', { name: 'Export' }));

    const sheet = await screen.findByRole('dialog', { name: 'Export' });
    const last = await within(sheet).findByRole('radio', { name: 'usb' });
    expect(last).toHaveAttribute('aria-checked', 'false');
    expect(within(sheet).getByRole('radio', { name: 'Downloads folder' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    // A closed picker changes nothing.
    await user.click(within(sheet).getByRole('button', { name: 'Choose folder…' }));
    await vi.waitFor(() => expect(commands.exportPickFolder).toHaveBeenCalled());
    expect(within(sheet).getByRole('radio', { name: 'Downloads folder' })).toHaveAttribute(
      'aria-checked',
      'true',
    );
    await user.click(last);
    await vi.waitFor(() =>
      expect(lastRequest()).toMatchObject({
        destination: 'folder',
        folder: { uri: '/media/usb', name: 'usb' },
      }),
    );
  });

  it('follows jobs in Downloads → Exports', async () => {
    const user = userEvent.setup();
    commands.exportJobs.mockResolvedValue([job({})]);
    await AppShellWrapper.mount('/downloads?tab=exports');
    const card = await screen.findByRole('listitem', { name: 'Frieren' });
    expect(card).toHaveTextContent('CBZ · Writing 1 of 3');
    expect(within(card).getByRole('progressbar')).toHaveAttribute('aria-valuenow', '1');
    // Stopping asks first; the files already saved stay.
    await user.click(within(card).getByRole('button', { name: 'Cancel Frieren' }));
    const ask = await screen.findByRole('dialog', { name: 'Stop this export?' });
    expect(ask).toHaveTextContent('Files it already saved stay where they are.');
    expect(commands.exportCancel).not.toHaveBeenCalled();
    await user.click(within(ask).getByRole('button', { name: 'Stop' }));
    expect(commands.exportCancel).toHaveBeenCalledWith('j1');

    emitEvent(
      'exportEvent',
      job({
        state: 'done',
        written: 3,
        failedChapters: 1,
        files: ['Download/Raygun/Manga/Frieren/0001 Chapter 1.cbz'],
      }),
    );
    expect(await within(card).findByText(/Done/)).toBeInTheDocument();
    expect(card).toHaveTextContent('Download/Raygun/Manga/Frieren/0001 Chapter 1.cbz');
    expect(card).toHaveTextContent("1 chapter couldn't be downloaded");
    await user.click(within(card).getByRole('button', { name: 'Remove Frieren from the list' }));
    expect(commands.exportDismiss).toHaveBeenCalledWith('j1');
    expect(await screen.findByText('No exports yet')).toBeInTheDocument();
  });

  it('exports a library selection', async () => {
    const user = userEvent.setup();
    commands.mangaLibrary.mockResolvedValue(
      ok([
        libraryEntry({ titleId: 'a', remoteId: 'a', title: 'Berserk', sourceId: SOURCE }),
        libraryEntry({ titleId: 'b', remoteId: 'b', title: 'Frieren', sourceId: SOURCE }),
      ]),
    );
    await AppShellWrapper.mount('/library?media=manga');
    const library = await screen.findByTestId('manga-library');
    await user.pointer({
      keys: '[MouseRight]',
      target: await within(library).findByRole('link', { name: 'Berserk' }),
    });
    await user.click(within(library).getByRole('button', { name: 'Select all' }));
    await user.click(
      within(within(library).getByRole('toolbar')).getByRole('button', { name: 'Export' }),
    );
    expect(await screen.findByRole('dialog', { name: 'Export' })).toBeInTheDocument();
    await vi.waitFor(() => expect(lastRequest().titleIds).toEqual(['a', 'b']));
  });

  it('saves where Settings → Downloads says', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/settings/downloads');
    await user.click(await screen.findByRole('button', { name: /^Save downloads to/ }));
    await user.click(screen.getByRole('radio', { name: /^Downloads folder/ }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'save',
      value: { ...SAVE_DEFAULTS, location: 'downloads' },
    });

    // No folder picked yet: choosing one opens the system picker.
    await user.click(screen.getByRole('button', { name: /^Save downloads to/ }));
    await user.click(screen.getByRole('radio', { name: /^A folder you choose/ }));
    expect(commands.savePickFolder).toHaveBeenCalledTimes(1);

    await user.click(screen.getByRole('button', { name: /^Save chapters as/ }));
    await user.click(screen.getByRole('radio', { name: /^Folders of images/ }));
    expect(commands.settingsUpdate).toHaveBeenCalledWith({
      key: 'save',
      value: expect.objectContaining({ chapterFormat: 'images' }),
    });
  });

  it('warns when the picked folder can no longer be written', async () => {
    commands.settingsGet.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        save: {
          ...SAVE_DEFAULTS,
          location: 'folder',
          folderUri: 'content://tree/x',
          folderName: 'Comics',
        },
      }),
    );
    commands.saveFolderState.mockResolvedValue(ok({ granted: false, name: null }));
    await AppShellWrapper.mount('/settings/downloads');
    expect(await screen.findByRole('alert')).toHaveTextContent("can't write to this folder");
    expect(screen.getByRole('button', { name: /^Change folder/ })).toHaveTextContent('Comics');
  });

  it('saves, shares and copies a page from a long press in the reader', async () => {
    const user = userEvent.setup();
    const pages: MangaChapterPages = {
      titleId: 't1',
      title: 'Frieren',
      readerMode: 'pagedLtr',
      ownReaderMode: 'pagedLtr',
      chapter: chapter('c1', 'Chapter 1'),
      pages: ['http://127.0.0.1:1/image/a', 'http://127.0.0.1:1/image/b'],
      previousChapterId: null,
      nextChapterId: null,
    };
    commands.mangaChapter.mockResolvedValue(ok(pages));
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await user.pointer({
      keys: '[MouseRight]',
      target: await within(reader).findByTestId('manga-page'),
    });
    const sheet = await screen.findByRole('dialog', { name: 'Page 1' });
    const page = { titleId: 't1', chapterId: 'c1', index: 0, url: 'http://127.0.0.1:1/image/a' };
    await user.click(within(sheet).getByRole('button', { name: 'Save page' }));
    expect(commands.pageSave).toHaveBeenCalledWith(page);
    expect(await within(sheet).findByRole('status')).toHaveTextContent(
      'Saved to Pictures/Raygun/page.jpg',
    );
    await user.click(within(sheet).getByRole('button', { name: 'Copy page' }));
    expect(commands.pageCopy).toHaveBeenCalledWith(page);
    expect(await within(sheet).findByText('Copied')).toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Share page' }));
    expect(commands.pageShare).toHaveBeenCalledWith(page);
    await vi.waitFor(() => expect(screen.queryByRole('dialog')).not.toBeInTheDocument());
  });
});
