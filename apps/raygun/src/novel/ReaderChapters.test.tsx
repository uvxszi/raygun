import type { MangaChapterInfo, NovelChapter } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const chapterInfo = (
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

/** The reader answers each chapter of `chapters` with a line of text. */
const readsFrom = (titleId: string, title: string, chapters: MangaChapterInfo[]) =>
  commands.novelChapter.mockImplementation(async (_title: string, id: string) => {
    const chapter = chapters.find((candidate) => candidate.id === id) ?? chapters[0]!;
    const text: NovelChapter = {
      titleId,
      title,
      chapter,
      blocks: [{ kind: 'paragraph', text: `The text of ${chapter.name}.` }],
      previousChapterId: null,
      nextChapterId: null,
    };
    return ok(text);
  });

/** The rows of the open chapter list, and each row's chapter name in the order shown. */
const chapterRows = (list: HTMLElement) => within(list).getAllByTestId('reader-chapter');
const chapterNames = (list: HTMLElement) =>
  chapterRows(list).map((row) => within(row).getByText(/^(Letter|Cover|Chapter)/).textContent);
const rowOf = (list: HTMLElement, name: string) =>
  chapterRows(list).find((row) => within(row).queryByText(name) !== null)!;

describe('The book reader’s chapter list', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    Element.prototype.scrollIntoView = vi.fn();
  });

  it('lists the book’s chapters with how far each was read, and reads the one picked there', async () => {
    const user = userEvent.setup();
    const chapters = [
      chapterInfo('c1', 'Letter 1', { number: 1, read: true }),
      // Progress in a book is a character offset into the chapter's text.
      chapterInfo('c2', 'Letter 2', { number: 2, lastPage: 450, totalPages: 1000 }),
      chapterInfo('c3', 'Letter 3', { number: 3 }),
      chapterInfo('c4', 'Letter 4', {
        number: 4,
        download: { state: 'done', pagesDone: 1, pagesTotal: 1, error: null },
      }),
    ];
    commands.mangaChapters.mockResolvedValue(ok(chapters));
    readsFrom('t1', 'Frankenstein', chapters);
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c3');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('The text of Letter 3.');

    // The text settings make way for the list.
    await user.click(within(reader).getByRole('button', { name: 'Text settings' }));
    expect(within(reader).getByRole('dialog', { name: 'Text settings' })).toBeInTheDocument();
    const open = within(reader).getByRole('button', { name: 'Chapters' });
    await user.click(open);
    expect(within(reader).queryByRole('dialog', { name: 'Text settings' })).not.toBeInTheDocument();
    expect(open).toHaveAttribute('aria-expanded', 'true');

    const list = await screen.findByTestId('reader-chapters');
    expect(commands.mangaChapters).toHaveBeenCalledWith('t1');
    await vi.waitFor(() =>
      expect(chapterNames(list)).toEqual(['Letter 1', 'Letter 2', 'Letter 3', 'Letter 4']),
    );
    expect(within(list).getByText('4 chapters')).toBeInTheDocument();
    // The chapter being read is marked and scrolled to.
    const current = rowOf(list, 'Letter 3');
    expect(current).toHaveAttribute('aria-current', 'true');
    const scrolled = vi.mocked(Element.prototype.scrollIntoView);
    expect(scrolled.mock.calls[scrolled.mock.contexts.indexOf(current)]).toEqual([
      { block: 'center' },
    ]);
    for (const name of ['Letter 1', 'Letter 2', 'Letter 4']) {
      expect(rowOf(list, name)).not.toHaveAttribute('aria-current');
    }
    // Read, partly read (a share of the text, not a page) and downloaded, as in the comic reader.
    expect(within(rowOf(list, 'Letter 1')).getByLabelText('Read')).not.toHaveAttribute(
      'aria-hidden',
    );
    expect(within(rowOf(list, 'Letter 2')).getByText('45% read')).toBeInTheDocument();
    expect(within(rowOf(list, 'Letter 2')).queryByLabelText('Read')).not.toBeInTheDocument();
    expect(within(list).queryByText(/Stopped on page/)).not.toBeInTheDocument();
    expect(within(rowOf(list, 'Letter 4')).getByLabelText('Downloaded')).toBeInTheDocument();

    await user.click(rowOf(list, 'Letter 4'));
    await vi.waitFor(() => expect(commands.novelChapter).toHaveBeenLastCalledWith('t1', 'c4'));
    expect(screen.queryByTestId('reader-chapters')).not.toBeInTheDocument();
    expect(await within(reader).findByText('The text of Letter 4.')).toBeInTheDocument();
    // The top bar names the chapter now read.
    expect(within(reader).getByText('Letter 4')).toBeInTheDocument();
  });

  it('only closes when the chapter being read is picked', async () => {
    const user = userEvent.setup();
    const chapters = [chapterInfo('c1', 'Letter 1'), chapterInfo('c2', 'Letter 2')];
    commands.mangaChapters.mockResolvedValue(ok(chapters));
    readsFrom('t1', 'Frankenstein', chapters);
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c2');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('The text of Letter 2.');

    await user.click(within(reader).getByRole('button', { name: 'Chapters' }));
    const list = await screen.findByTestId('reader-chapters');
    await vi.waitFor(() => expect(chapterNames(list)).toEqual(['Letter 1', 'Letter 2']));
    commands.novelChapter.mockClear();
    await user.click(rowOf(list, 'Letter 2'));
    expect(screen.queryByTestId('reader-chapters')).not.toBeInTheDocument();
    expect(commands.novelChapter).not.toHaveBeenCalled();
    expect(within(reader).getByText('The text of Letter 2.')).toBeInTheDocument();
  });

  it('lists an imported book’s table of contents, sorted as asked, and opens any entry', async () => {
    const user = userEvent.setup();
    // An EPUB's chapters are its table of contents, numbered in the book's order.
    const contents = [
      chapterInfo('k1', 'Cover', { number: 1, read: true }),
      chapterInfo('k2', 'Chapter I. The Beginning', { number: 2 }),
      chapterInfo('k3', 'Chapter II. The Middle', { number: 3 }),
      chapterInfo('k4', 'Chapter III. The End', { number: 4 }),
    ];
    commands.mangaChapters.mockResolvedValue(ok(contents));
    readsFrom('b1', 'My Book', contents);
    await AppShellWrapper.mount('/novel/read?title=b1&chapter=k2');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('The text of Chapter I. The Beginning.');

    await user.click(within(reader).getByRole('button', { name: 'Chapters' }));
    let list = await screen.findByTestId('reader-chapters');
    expect(commands.mangaChapters).toHaveBeenCalledWith('b1');
    await vi.waitFor(() =>
      expect(chapterNames(list)).toEqual([
        'Cover',
        'Chapter I. The Beginning',
        'Chapter II. The Middle',
        'Chapter III. The End',
      ]),
    );
    expect(rowOf(list, 'Chapter I. The Beginning')).toHaveAttribute('aria-current', 'true');
    expect(within(rowOf(list, 'Cover')).getByLabelText('Read')).toBeInTheDocument();

    // The same sort and filter as on the book's page.
    await user.click(within(list).getByRole('button', { name: 'Sort and filter chapters' }));
    await user.click(await screen.findByRole('button', { name: /Chapter number/ }));
    await vi.waitFor(() =>
      expect(chapterNames(list)).toEqual([
        'Chapter III. The End',
        'Chapter II. The Middle',
        'Chapter I. The Beginning',
        'Cover',
      ]),
    );
    await user.click(screen.getByRole('checkbox', { name: 'Unread' }));
    await vi.waitFor(() =>
      expect(chapterNames(list)).toEqual([
        'Chapter III. The End',
        'Chapter II. The Middle',
        'Chapter I. The Beginning',
      ]),
    );
    await user.keyboard('{Escape}');
    expect(screen.queryByRole('checkbox', { name: 'Unread' })).not.toBeInTheDocument();

    await user.click(rowOf(list, 'Chapter III. The End'));
    await vi.waitFor(() => expect(commands.novelChapter).toHaveBeenLastCalledWith('b1', 'k4'));
    expect(
      await within(reader).findByText('The text of Chapter III. The End.'),
    ).toBeInTheDocument();
    expect(screen.queryByTestId('reader-chapters')).not.toBeInTheDocument();

    // Opened again, the list marks the new chapter and keeps the order picked.
    await user.click(within(reader).getByRole('button', { name: 'Chapters' }));
    list = await screen.findByTestId('reader-chapters');
    await vi.waitFor(() =>
      expect(rowOf(list, 'Chapter III. The End')).toHaveAttribute('aria-current', 'true'),
    );
    expect(chapterNames(list)).toEqual([
      'Chapter III. The End',
      'Chapter II. The Middle',
      'Chapter I. The Beginning',
    ]);
  });
});
