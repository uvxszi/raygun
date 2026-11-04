import type { MangaChapterInfo, MangaTitleView } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.raygun.mangadex/mangadex';

const chapter = (
  id: string,
  name: string,
  extra: Partial<MangaChapterInfo> = {},
): MangaChapterInfo => ({
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
  ...extra,
});

const title = (chapters: MangaChapterInfo[]): MangaTitleView => ({
  id: 't1',
  hiddenNsfw: false,
  sourceId: SOURCE,
  sourceName: 'MangaDex',
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
  chapters,
  continueChapterId: chapters.at(-1)?.id ?? null,
});

const open = async (view: MangaTitleView) => {
  commands.mangaTitle.mockResolvedValue(ok(view));
  await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
  return screen.findByTestId('manga-title-page');
};

describe('A title’s chapters', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.librarySetChaptersRead.mockResolvedValue(ok(null));
    commands.libraryMarkPreviousRead.mockResolvedValue(ok(2));
    commands.mangaDownload.mockResolvedValue(ok(null));
    commands.mangaRemoveDownloads.mockResolvedValue(ok(null));
  });

  it('selects chapters with a long press and marks them together', async () => {
    const user = userEvent.setup();
    const page = await open(
      title([
        chapter('c3', 'Chapter 3'),
        chapter('c2', 'Chapter 2', {
          download: { state: 'done', pagesDone: 10, pagesTotal: 10, error: null },
        }),
        chapter('c1', 'Chapter 1'),
      ]),
    );
    const rows = await within(page).findAllByTestId('chapter-row');

    // A long press starts selecting; a tap then picks rows instead of opening them.
    fireEvent.contextMenu(rows[0]!);
    expect(await screen.findByTestId('selection-bar')).toHaveTextContent('1 selected');
    await user.click(rows[1]!);
    expect(screen.getByTestId('selection-bar')).toHaveTextContent('2 selected');
    expect(rows[1]).toHaveAttribute('aria-pressed', 'true');

    const actions = screen.getByTestId('chapter-actions');
    // Only one of them is downloaded: Delete downloads takes just that one.
    await user.click(within(actions).getByRole('button', { name: 'Delete downloads' }));
    expect(commands.mangaRemoveDownloads).toHaveBeenCalledWith(['c2']);
    await vi.waitFor(() => expect(screen.queryByTestId('selection-bar')).not.toBeInTheDocument());

    fireEvent.contextMenu(rows[0]!);
    await user.click(rows[2]!);
    await user.click(
      within(screen.getByTestId('chapter-actions')).getByRole('button', { name: 'Mark as read' }),
    );
    expect(commands.librarySetChaptersRead).toHaveBeenCalledWith('manga', ['c3', 'c1'], true);
  });

  it('marks the chapters before one as read, and selects all or the rest', async () => {
    const user = userEvent.setup();
    const page = await open(
      title([chapter('c3', 'Chapter 3'), chapter('c2', 'Chapter 2'), chapter('c1', 'Chapter 1')]),
    );
    await within(page).findAllByTestId('chapter-row');
    await user.click(within(page).getByRole('button', { name: 'Select chapters' }));
    const bar = await screen.findByTestId('selection-bar');
    expect(bar).toHaveTextContent('0 selected');
    await user.click(within(bar).getByRole('button', { name: 'Select all' }));
    expect(bar).toHaveTextContent('3 selected');
    await user.click(within(bar).getByRole('button', { name: 'Invert selection' }));
    expect(bar).toHaveTextContent('0 selected');

    await user.click(within(page).getAllByTestId('chapter-row')[1]!);
    await user.click(
      within(screen.getByTestId('chapter-actions')).getByRole('button', {
        name: 'Mark the chapters before as read',
      }),
    );
    expect(commands.libraryMarkPreviousRead).toHaveBeenCalledWith('manga', 't1', 'c2');

    // Escape stops selecting.
    await user.click(within(page).getByRole('button', { name: 'Select chapters' }));
    await user.keyboard('{Escape}');
    expect(screen.queryByTestId('selection-bar')).not.toBeInTheDocument();
  });

  it('offers no resume button once every chapter is read', async () => {
    const page = await open(
      title([
        chapter('c2', 'Chapter 2', { read: true }),
        chapter('c1', 'Chapter 1', { read: true }),
      ]),
    );
    await within(page).findAllByTestId('chapter-row');
    expect(within(page).queryByRole('button', { name: /^Continue/ })).not.toBeInTheDocument();
    expect(within(page).queryByRole('button', { name: 'Start reading' })).not.toBeInTheDocument();
  });

  it('opens the title’s website from its page', async () => {
    const user = userEvent.setup();
    commands.catalogOpenPage.mockResolvedValue(ok(null));
    const page = await open(title([chapter('c1', 'Chapter 1')]));
    await user.click(await within(page).findByRole('button', { name: 'Website' }));
    expect(commands.catalogOpenPage).toHaveBeenCalledWith(SOURCE, null);
  });
});
