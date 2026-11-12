import type {
  MangaChapterInfo,
  NovelChapter,
  PdfChapterText,
  PdfContentsEntry,
} from '@raygun/bindings';
import { screen, waitFor, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const info = (id: string): MangaChapterInfo => ({
  id,
  name: `Part ${id}`,
  number: null,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
});

/** A book of two chapters, two pages each: its whole outline is the contents of both. */
const CONTENTS: PdfContentsEntry[] = [
  { title: 'Lighthouses', chapterId: 'c1', page: 0, pageNumber: 1, level: 0 },
  { title: 'The keeper', chapterId: 'c1', page: 1, pageNumber: 2, level: 1 },
  { title: 'Storms', chapterId: 'c2', page: 0, pageNumber: 3, level: 0 },
  { title: 'The wreck', chapterId: 'c2', page: 1, pageNumber: 4, level: 1 },
];

const chapter = (id: 'c1' | 'c2', contents = CONTENTS): NovelChapter => ({
  titleId: 't1',
  title: 'Lighthouses of the North',
  chapter: info(id),
  blocks: [],
  previousChapterId: id === 'c2' ? 'c1' : null,
  nextChapterId: id === 'c1' ? 'c2' : null,
  pdf: {
    pages: [0, 1].map((page) => `http://127.0.0.1:9/pdf/book/${id}-${page}`),
    firstPage: id === 'c1' ? 1 : 3,
    readerMode: 'pagedLtr',
    ownReaderMode: 'pagedLtr',
    contents,
  },
});

/** Each chapter's text: a bookmark's heading partway down its page, after text flowing on. */
const TEXTS: Record<string, PdfChapterText> = {
  c1: {
    blocks: [
      { kind: 'heading', text: 'Lighthouses' },
      { kind: 'paragraph', text: 'The coast had many lights.' },
      { kind: 'paragraph', text: 'Their keepers wrote everything down.' },
      { kind: 'heading', text: '1.1 The keeper' },
      { kind: 'paragraph', text: 'The keeper climbed the stairs.' },
    ],
    pageStarts: [0, 2],
  },
  c2: {
    blocks: [
      { kind: 'heading', text: 'Storms' },
      { kind: 'paragraph', text: 'The wind rose at dusk.' },
      { kind: 'heading', text: '2.1 The wreck' },
      { kind: 'paragraph', text: 'A hull lay on the rocks.' },
    ],
    pageStarts: [0, 2],
  },
};

describe('A PDF book’s contents', () => {
  /** The elements brought into view, in order. */
  let scrolled: Element[];

  beforeEach(() => {
    AppShellWrapper.seedCore();
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        observe() {}
        disconnect() {}
      },
    );
    scrolled = [];
    Element.prototype.scrollIntoView = vi.fn(function (this: Element) {
      scrolled.push(this);
    });
    localStorage.clear();
    commands.novelChapter.mockImplementation(async (_title: string, id: string) =>
      ok(chapter(id === 'c2' ? 'c2' : 'c1')),
    );
    commands.novelPdfText.mockImplementation(async (_title: string, id: string) =>
      ok(TEXTS[id] ?? TEXTS.c1!),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  const openContents = async (reader: HTMLElement) => {
    await userEvent.click(within(reader).getByRole('button', { name: 'Contents' }));
    return screen.findByRole('dialog', { name: 'Contents' });
  };

  /** The book's one list: its contents, and no chapter list beside them. */
  const expectOneList = (reader: HTMLElement) => {
    expect(within(reader).getAllByRole('button', { name: 'Contents' })).toHaveLength(1);
    expect(within(reader).queryByRole('button', { name: 'Chapters' })).not.toBeInTheDocument();
  };

  it('lists the chapters with the outline in the page view, in place of a chapter list', async () => {
    const user = userEvent.setup();
    // Front matter before the first bookmark: a chapter of its own, already read.
    commands.mangaChapters.mockResolvedValue(
      ok([{ ...info('c0'), name: 'Front matter', read: true }, info('c1'), info('c2')]),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await within(reader).findAllByTestId('manga-page');
    expectOneList(reader);

    const dialog = await openContents(reader);
    const front = await within(dialog).findByTestId('pdf-contents-chapter');
    expect(front).toHaveTextContent('Front matter');
    expect(within(front).getByLabelText('Read')).toBeInTheDocument();
    // Chapters the bookmarks lead into are their bookmarks: no rows of their own.
    expect(within(dialog).getAllByTestId('pdf-contents-chapter')).toHaveLength(1);
    expect(within(dialog).getByRole('button', { name: 'Lighthouses, page 1' })).toHaveAttribute(
      'aria-current',
      'true',
    );
    await user.click(front);
    await waitFor(() => expect(commands.novelChapter).toHaveBeenLastCalledWith('t1', 'c0'));
    expect(screen.queryByRole('dialog', { name: 'Contents' })).not.toBeInTheDocument();
  });

  it('keeps the one list for a document without bookmarks, in the text view too', async () => {
    const user = userEvent.setup();
    localStorage.setItem('raygun.pdfView.t1', 'text');
    commands.novelChapter.mockImplementation(async (_title: string, id: string) => {
      const opened = chapter(id === 'c2' ? 'c2' : 'c1', []);
      // The next chapter was left on its second page.
      return ok(id === 'c2' ? { ...opened, chapter: { ...opened.chapter, lastPage: 1 } } : opened);
    });
    commands.mangaChapters.mockResolvedValue(ok([info('c1'), info('c2')]));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('The keeper climbed the stairs.');
    expectOneList(reader);

    const dialog = await openContents(reader);
    await waitFor(() =>
      expect(
        within(dialog)
          .getAllByTestId('pdf-contents-chapter')
          .map((row) => row.textContent),
      ).toEqual(['Part c1', 'Part c2']),
    );
    expect(within(dialog).getByRole('button', { name: 'Part c1' })).toHaveAttribute(
      'aria-current',
      'true',
    );
    // Another chapter opens at its start, not where it was left.
    scrolled = [];
    await user.click(within(dialog).getByRole('button', { name: 'Part c2' }));
    await waitFor(() => expect(commands.novelPdfText).toHaveBeenCalledWith('t1', 'c2'));
    const next = await screen.findByTestId('novel-reader');
    expect(await within(next).findByText('Storms')).toBeInTheDocument();
    expect(scrolled).not.toContain(within(next).getByText('2.1 The wreck').closest('h2'));
  });

  it('lists the whole outline as a tree in the page view and opens bookmarks anywhere', async () => {
    const user = userEvent.setup();
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await within(reader).findAllByTestId('manga-page');
    expectOneList(reader);

    let dialog = await openContents(reader);
    // The part being read is marked and its branch open; other branches are closed.
    expect(within(dialog).getByRole('button', { name: 'Lighthouses, page 1' })).toHaveAttribute(
      'aria-current',
      'true',
    );
    expect(within(dialog).getByRole('button', { name: 'The keeper, page 2' })).toBeInTheDocument();
    const storms = within(dialog).getByRole('button', { name: 'Sections of Storms' });
    expect(storms).toHaveAttribute('aria-expanded', 'false');
    expect(within(dialog).queryByRole('button', { name: 'The wreck, page 4' })).toBeNull();
    await user.click(storms);
    expect(storms).toHaveAttribute('aria-expanded', 'true');

    // A bookmark in this chapter turns to its page.
    await user.click(within(dialog).getByRole('button', { name: 'The keeper, page 2' }));
    await waitFor(() =>
      expect(screen.queryByRole('dialog', { name: 'Contents' })).not.toBeInTheDocument(),
    );
    await waitFor(() =>
      expect(within(reader).getByTestId('page-indicator')).toHaveTextContent('Page 2 of 2'),
    );

    // The marked part follows the page; a bookmark in the next chapter opens it at its page.
    dialog = await openContents(reader);
    expect(within(dialog).getByRole('button', { name: 'The keeper, page 2' })).toHaveAttribute(
      'aria-current',
      'true',
    );
    await user.click(within(dialog).getByRole('button', { name: 'Sections of Storms' }));
    await user.click(within(dialog).getByRole('button', { name: 'The wreck, page 4' }));
    await waitFor(() => expect(commands.novelChapter).toHaveBeenCalledWith('t1', 'c2'));
    const next = await screen.findByTestId('manga-reader');
    await waitFor(() => expect(within(next).getByText('Part c2')).toBeInTheDocument());
    await waitFor(() =>
      expect(within(next).getByTestId('page-indicator')).toHaveTextContent('Page 2 of 2'),
    );
  });

  it('lists the same contents in the text view and opens a bookmark at its heading', async () => {
    const user = userEvent.setup();
    localStorage.setItem('raygun.pdfView.t1', 'text');
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('The keeper climbed the stairs.');
    expectOneList(reader);

    let dialog = await openContents(reader);
    const rows = within(within(dialog).getByTestId('pdf-contents'))
      .getAllByRole('button')
      .map((button) => button.getAttribute('aria-label'));
    expect(rows).toEqual([
      'Sections of Lighthouses',
      'Lighthouses, page 1',
      'The keeper, page 2',
      'Sections of Storms',
      'Storms, page 3',
    ]);

    // In this chapter: the heading the bookmark names, not the top of its page.
    scrolled = [];
    await user.click(within(dialog).getByRole('button', { name: 'The keeper, page 2' }));
    await waitFor(() =>
      expect(scrolled).toContain(within(reader).getByText('1.1 The keeper').closest('h2')),
    );

    // In another chapter: it opens there, at the bookmark's heading.
    dialog = await openContents(reader);
    await user.click(within(dialog).getByRole('button', { name: 'Sections of Storms' }));
    scrolled = [];
    await user.click(within(dialog).getByRole('button', { name: 'The wreck, page 4' }));
    await waitFor(() => expect(commands.novelPdfText).toHaveBeenCalledWith('t1', 'c2'));
    const wreck = await within(await screen.findByTestId('novel-reader')).findByText(
      '2.1 The wreck',
    );
    await waitFor(() => expect(scrolled).toContain(wreck.closest('h2')));
  });
});
