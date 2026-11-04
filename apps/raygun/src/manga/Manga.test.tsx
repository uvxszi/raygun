import type { MangaChapterInfo, MangaChapterPages, MangaTitleView } from '@raygun/bindings';
import { fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.example.comics/comics';

const card = (id: string, title: string, inLibrary = false) => ({
  sourceId: SOURCE,
  id,
  title,
  coverUrl: `http://127.0.0.1:1/image/${id}`,
  inLibrary,
});

const chapter = (
  id: string,
  name: string,
  extra: Partial<MangaChapterInfo> = {},
): MangaChapterInfo => ({
  id,
  name,
  number: null,
  volume: null,
  scanlator: 'Group',
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
  ...extra,
});

const TITLE: MangaTitleView = {
  id: 't1',
  hiddenNsfw: false,
  sourceId: SOURCE,
  sourceName: 'Comic Shelf',
  remoteId: 'm1',
  title: 'Frieren',
  coverUrl: null,
  authors: ['Yamada'],
  artists: ['Abe'],
  description: 'An elf mage outlives her party.',
  genres: ['Fantasy'],
  status: 'ongoing',
  url: null,
  inLibrary: false,
  chapters: [chapter('c2', 'Chapter 2'), chapter('c1', 'Chapter 1', { read: true })],
  continueChapterId: 'c2',
};

describe('Manga', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    commands.mangaSources.mockResolvedValue(
      ok([{ id: SOURCE, name: 'Comic Shelf', extensionId: 'dev.example.comics' }]),
    );
  });

  it('switches the library to manga and lists titles with unread counts', async () => {
    const user = userEvent.setup();
    commands.mangaLibrary.mockResolvedValue(
      ok([
        libraryEntry({
          titleId: 't1',
          sourceId: SOURCE,
          remoteId: 'm1',
          title: 'Frieren',
          unread: 3,
        }),
      ]),
    );
    await AppShellWrapper.mount('/library');
    await user.click(await screen.findByTestId('library-media-manga'));

    const library = await screen.findByTestId('manga-library');
    expect(await within(library).findByTestId('library-item')).toHaveTextContent('Frieren');
    expect(within(library).getByLabelText('3 unread chapters')).toBeInTheDocument();
    expect(screen.getByTestId('nav-browse-tab')).toHaveAttribute('href', '/browse');
  });

  it('browses a source, loads more and searches', async () => {
    const user = userEvent.setup();
    commands.mangaBrowse.mockImplementation(
      (_source: string, mode: string, query: string | null, page: number) =>
        Promise.resolve(
          ok(
            mode === 'search'
              ? { items: [card('s1', `Found ${query}`)], hasNextPage: false }
              : { items: [card(`p${page}`, `Popular ${page}`, page === 1)], hasNextPage: page < 2 },
          ),
        ),
    );
    await AppShellWrapper.mount(`/manga/source?id=${encodeURIComponent(SOURCE)}`);
    const page = await screen.findByTestId('manga-source-page');
    expect(await within(page).findByRole('heading', { name: 'Comic Shelf' })).toBeInTheDocument();
    const first = await within(page).findByTestId('manga-card');
    expect(first).toHaveTextContent('In library');
    expect(first).toHaveTextContent('Popular 1');

    await user.click(within(page).getByRole('button', { name: 'Load more' }));
    expect(await within(page).findByText('Popular 2')).toBeInTheDocument();
    expect(within(page).queryByRole('button', { name: 'Load more' })).not.toBeInTheDocument();

    await user.type(within(page).getByLabelText('Search this source'), 'frieren{Enter}');
    expect(await within(page).findByText('Found frieren')).toBeInTheDocument();
    expect(commands.mangaBrowse).toHaveBeenLastCalledWith(SOURCE, 'search', 'frieren', 1);
  });

  it('shows a title, adds it to the library and marks chapters read', async () => {
    const user = userEvent.setup();
    commands.mangaTitle.mockResolvedValue(ok(TITLE));
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');

    expect(await within(page).findByRole('heading', { name: 'Frieren' })).toBeInTheDocument();
    expect(within(page).getByText('Yamada, Abe · Ongoing')).toBeInTheDocument();
    expect(within(page).getByTestId('source-tag')).toHaveTextContent('Comic Shelf');
    // The floating button picks up where reading stopped.
    expect(within(page).getByRole('button', { name: /Continue: Chapter 2/ })).toBeInTheDocument();
    expect(within(page).getAllByTestId('chapter-row')).toHaveLength(2);

    await user.click(within(page).getByRole('button', { name: 'Add to library' }));
    expect(commands.mangaSetInLibrary).toHaveBeenCalledWith('t1', true);
    await user.click(within(page).getByRole('button', { name: 'Mark Chapter 1 unread' }));
    expect(commands.mangaSetRead).toHaveBeenCalledWith('c1', false);
    await user.click(within(page).getByRole('button', { name: 'Refresh from the source' }));
    expect(commands.mangaTitle).toHaveBeenLastCalledWith(SOURCE, 'm1', true);
  });

  it('tags an adult title 18+ beside its name', async () => {
    commands.mangaTitle.mockResolvedValue(ok({ ...TITLE, adult: true }));
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');
    const heading = await within(page).findByRole('heading', { name: /Frieren/ });
    expect(within(heading).getByRole('img', { name: 'For adults (18+)' })).toBeInTheDocument();
  });

  it('shows a notice instead of an adult title while NSFW content is off', async () => {
    commands.mangaTitle.mockResolvedValue(ok({ ...TITLE, hiddenNsfw: true }));
    await AppShellWrapper.mount(`/manga/title?source=${encodeURIComponent(SOURCE)}&id=m1`);
    const page = await screen.findByTestId('manga-title-page');

    expect(
      await within(page).findByRole('heading', { name: 'This title is for adults' }),
    ).toBeInTheDocument();
    expect(within(page).queryByText('Frieren')).not.toBeInTheDocument();
    expect(within(page).getByRole('link', { name: 'Open Settings' })).toHaveAttribute(
      'href',
      '/settings',
    );
  });

  it('reads a chapter, saves progress and moves to the next one', async () => {
    const user = userEvent.setup();
    type Entries = { isIntersecting: boolean; target: Element }[];
    const observer: { notify?: (entries: Entries) => void } = {};
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        constructor(callback: (entries: Entries) => void) {
          observer.notify = callback;
        }
        observe() {}
        disconnect() {}
      },
    );
    const pages = (chapterId: string): MangaChapterPages => ({
      titleId: 't1',
      title: 'Frieren',
      readerMode: 'webtoon',
      ownReaderMode: null,
      chapter: chapter(chapterId, chapterId === 'c1' ? 'Chapter 1' : 'Chapter 2'),
      pages: ['http://img/1', 'http://img/2', 'http://img/3'].map((url) => `${url}-${chapterId}`),
      previousChapterId: chapterId === 'c2' ? 'c1' : null,
      nextChapterId: chapterId === 'c1' ? 'c2' : null,
    });
    commands.mangaChapter.mockImplementation((_title: string, chapterId: string) =>
      Promise.resolve(ok(pages(chapterId))),
    );
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');

    const reader = await screen.findByTestId('manga-reader');
    const images = await within(reader).findAllByTestId('manga-page');
    expect(images).toHaveLength(3);
    expect(screen.queryByTestId('bottom-tabs')).not.toBeInTheDocument();
    expect(within(reader).getByTestId('page-indicator')).toHaveTextContent('Page 1 of 3');

    // The page crossing the reading line is the current one.
    const slots = within(reader).getAllByTestId('webtoon-page');
    observer.notify?.([{ isIntersecting: true, target: slots[2] as Element }]);
    expect(await within(reader).findByText('Page 3 of 3')).toBeInTheDocument();
    await vi.waitFor(() => expect(commands.mangaSetProgress).toHaveBeenCalledWith('c1', 2, 3));

    // Chapters replace each other in history: Back leaves the reader.
    await user.click(within(reader).getByRole('button', { name: 'Next chapter' }));
    expect(await within(reader).findByText('Chapter 2')).toBeInTheDocument();
    expect(commands.mangaChapter).toHaveBeenLastCalledWith('t1', 'c2');
    vi.unstubAllGlobals();
  });

  it('reads right to left with tap zones, the slider and an end card', async () => {
    const user = userEvent.setup();
    const pages = (chapterId: string): MangaChapterPages => ({
      titleId: 't1',
      title: 'Frieren',
      readerMode: 'pagedRtl',
      ownReaderMode: 'pagedRtl',
      chapter: chapter(chapterId, chapterId === 'c1' ? 'Chapter 1' : 'Chapter 2'),
      pages: [1, 2, 3, 4, 5].map((n) => `http://img/${chapterId}-${n}`),
      previousChapterId: null,
      nextChapterId: chapterId === 'c1' ? 'c2' : null,
    });
    commands.mangaChapter.mockImplementation((_title: string, chapterId: string) =>
      Promise.resolve(ok(pages(chapterId))),
    );
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const surface = await within(reader).findByTestId('paged-reader');
    vi.spyOn(surface, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      top: 0,
      right: 300,
      bottom: 600,
      width: 300,
      height: 600,
      x: 0,
      y: 0,
      toJSON: () => ({}),
    });
    const tapAt = (clientX: number) =>
      user.pointer({ keys: '[MouseLeft]', target: surface, coords: { clientX, clientY: 300 } });
    const indicator = () => within(reader).getByTestId('page-indicator');

    expect(indicator()).toHaveTextContent('Page 1 of 5');
    // Right to left: the left third turns forward, the right third back.
    await tapAt(30);
    await vi.waitFor(() => expect(indicator()).toHaveTextContent('Page 2 of 5'));
    await tapAt(280);
    await vi.waitFor(() => expect(indicator()).toHaveTextContent('Page 1 of 5'));

    // The slider jumps, and is mirrored for right to left.
    const slider = within(reader).getByRole('slider', { name: 'Jump to page' });
    expect(slider).toHaveAttribute('dir', 'rtl');
    fireEvent.change(slider, { target: { value: '4' } });
    expect(indicator()).toHaveTextContent('Page 5 of 5');
    await vi.waitFor(() => expect(commands.mangaSetProgress).toHaveBeenCalledWith('c1', 4, 5));
    // Near the end, the next chapter is fetched ahead.
    await vi.waitFor(() => expect(commands.mangaChapter).toHaveBeenCalledWith('t1', 'c2'));

    await tapAt(30);
    expect(await within(reader).findByText('End of Chapter 1')).toBeInTheDocument();
    await user.click(within(reader).getByRole('button', { name: 'Next chapter' }));
    expect(await within(reader).findByText('Chapter 2')).toBeInTheDocument();
  });

  it('remembers the reading mode picked in the reader', async () => {
    const user = userEvent.setup();
    commands.mangaChapter.mockResolvedValue(
      ok({
        titleId: 't1',
        title: 'Frieren',
        readerMode: 'webtoon',
        ownReaderMode: null,
        chapter: chapter('c1', 'Chapter 1'),
        pages: ['http://img/1', 'http://img/2'],
        previousChapterId: null,
        nextChapterId: null,
      }),
    );
    await AppShellWrapper.mount('/manga/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await within(reader).findAllByTestId('manga-page');
    expect(within(reader).queryByTestId('paged-reader')).not.toBeInTheDocument();

    // The top bar's (the page bar has one too, M46.7). A title without its own mode follows
    // the default.
    await user.click(within(reader).getAllByRole('button', { name: 'Reading mode' })[0]!);
    const modes = await screen.findByRole('dialog', { name: 'Reading mode' });
    expect(within(modes).getByRole('radio', { name: 'Default (Vertical strip)' })).toBeChecked();
    await user.click(within(modes).getByRole('radio', { name: 'Paged, left to right' }));
    expect(commands.mangaSetReaderMode).toHaveBeenCalledWith('t1', 'pagedLtr');
    expect(within(reader).getByTestId('paged-reader')).toBeInTheDocument();

    // Back to the default.
    await user.click(within(reader).getAllByRole('button', { name: 'Reading mode' })[0]!);
    await user.click(
      within(await screen.findByRole('dialog', { name: 'Reading mode' })).getByRole('radio', {
        name: 'Default (Vertical strip)',
      }),
    );
    expect(commands.mangaSetReaderMode).toHaveBeenLastCalledWith('t1', null);
    expect(within(reader).queryByTestId('paged-reader')).not.toBeInTheDocument();
  });
});
