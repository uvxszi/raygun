import type { MangaTitleView, NovelChapter } from '@raygun/bindings';
import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.raygun.gutenberg/gutenberg';

/** Stands in for IntersectionObserver; tests decide what is on screen. */
class FakeObserver {
  static instances: FakeObserver[] = [];
  observed: Element[] = [];
  constructor(private readonly callback: IntersectionObserverCallback) {
    FakeObserver.instances.push(this);
  }
  observe(element: Element) {
    this.observed.push(element);
  }
  disconnect() {
    this.observed = [];
  }
  unobserve() {}
  takeRecords() {
    return [];
  }
  show(elements: Element[]) {
    act(() => {
      this.callback(
        elements.map((target) => ({ target, isIntersecting: true }) as IntersectionObserverEntry),
        this as unknown as IntersectionObserver,
      );
    });
  }
}

const chapterInfo = (id: string, name: string, extra = {}) => ({
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

// Texts of 10, 20 and 30 characters: blocks start at 0, 10 (heading has 0+), 30.
const CHAPTER: NovelChapter = {
  titleId: 't1',
  title: 'Frankenstein',
  chapter: chapterInfo('c1', 'Letter 1', { lastPage: 35, totalPages: 60 }),
  blocks: [
    { kind: 'heading', text: 'Letter 1!!' },
    { kind: 'paragraph', text: 'It was a dreary night of Nov' },
    { kind: 'image', url: 'http://127.0.0.1:1/image/abc' },
    { kind: 'paragraph', text: 'ember. Rain fell on the panes.' },
    { kind: 'separator' },
  ],
  previousChapterId: null,
  nextChapterId: 'c2',
};

describe('Novels', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    FakeObserver.instances = [];
    vi.stubGlobal('IntersectionObserver', FakeObserver);
    Element.prototype.scrollIntoView = vi.fn();
    commands.novelSources.mockResolvedValue(
      ok([{ id: SOURCE, name: 'Project Gutenberg', extensionId: 'dev.raygun.gutenberg' }]),
    );
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('shows the novel library from the library switch', async () => {
    const user = userEvent.setup();
    commands.novelLibrary.mockResolvedValue(
      ok([
        {
          titleId: 't1',
          sourceId: SOURCE,
          remoteId: '84',
          title: 'Frankenstein',
          coverUrl: null,
          chapters: 24,
          unread: 20,
        },
      ]),
    );
    await AppShellWrapper.mount('/library');
    await user.click(await screen.findByTestId('library-media-novel'));

    const library = await screen.findByTestId('novel-library');
    const card = await within(library).findByTestId('manga-card');
    expect(card.getAttribute('href')).toMatch(/^\/novel\/title\?source=dev\.raygun\.gutenberg/);
    expect(within(library).getByRole('link', { name: 'Browse sources' })).toHaveAttribute(
      'href',
      '/novel/browse',
    );
    expect(within(library).queryByRole('link', { name: 'Downloads' })).not.toBeInTheDocument();
  });

  it('shows a novel without download controls and with reading percentages', async () => {
    const title: MangaTitleView = {
      id: 't1',
      sourceId: SOURCE,
      sourceName: 'Project Gutenberg',
      remoteId: '84',
      title: 'Frankenstein',
      coverUrl: null,
      authors: ['Mary Wollstonecraft Shelley'],
      artists: [],
      description: null,
      genres: [],
      status: 'completed',
      url: null,
      inLibrary: true,
      chapters: [
        chapterInfo('c1', 'Letter 1', { read: true }),
        chapterInfo('c2', 'Letter 2', { lastPage: 450, totalPages: 1000 }),
      ],
      continueChapterId: 'c2',
    };
    commands.novelTitle.mockResolvedValue(ok(title));
    await AppShellWrapper.mount(`/novel/title?source=${encodeURIComponent(SOURCE)}&id=84`);
    const page = await screen.findByTestId('manga-title-page');

    expect(await within(page).findByRole('link', { name: /Continue: Letter 2/ })).toHaveAttribute(
      'href',
      '/novel/read?title=t1&chapter=c2',
    );
    expect(within(page).getByText('45% read')).toBeInTheDocument();
    expect(within(page).queryByRole('button', { name: /^Download/ })).not.toBeInTheDocument();
    expect(commands.novelTitle).toHaveBeenCalledWith(SOURCE, '84', false);
  });

  it('reads a chapter: resumes, saves progress, finishes and moves on', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockImplementation((_title: string, chapter: string) =>
      Promise.resolve(
        ok(
          chapter === 'c1'
            ? CHAPTER
            : { ...CHAPTER, chapter: chapterInfo('c2', 'Letter 2'), nextChapterId: null },
        ),
      ),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');

    expect(await within(reader).findByRole('heading', { name: 'Letter 1!!' })).toBeInTheDocument();
    expect(within(reader).getByText('It was a dreary night of Nov')).toBeInTheDocument();
    expect(reader.querySelector('img')).toHaveAttribute('src', 'http://127.0.0.1:1/image/abc');
    // Blocks start at 0, 10, 38, 38 and 68: the saved offset 35 is in the first paragraph.
    const firstParagraph = within(reader).getByText('It was a dreary night of Nov');
    expect(firstParagraph.scrollIntoView).toHaveBeenCalledWith({ block: 'start' });

    vi.useFakeTimers({ shouldAdvanceTime: true });
    const observer = FakeObserver.instances.at(-1)!;
    observer.show([within(reader).getByText('ember. Rain fell on the panes.')]);
    expect(within(reader).getByTestId('novel-progress')).toHaveTextContent('56%');
    await act(() => vi.advanceTimersByTimeAsync(1000));
    expect(commands.novelSetProgress).toHaveBeenLastCalledWith('c1', 38, 68);
    vi.useRealTimers();

    observer.show([within(reader).getByTestId('novel-end')]);
    expect(commands.novelSetProgress).toHaveBeenLastCalledWith('c1', 68, 68);

    await user.click(within(reader).getByRole('button', { name: 'Next chapter' }));
    expect(await within(reader).findByText('Letter 2')).toBeInTheDocument();
    expect(commands.novelChapter).toHaveBeenLastCalledWith('t1', 'c2');
  });

  it('changes the text settings', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
    commands.settingsUpdate.mockImplementation((change: { value: unknown }) =>
      Promise.resolve(ok({ ...DEFAULT_SETTINGS, novelReader: change.value })),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('It was a dreary night of Nov');

    await user.click(within(reader).getByRole('button', { name: 'Text settings' }));
    const panel = within(reader).getByRole('dialog', { name: 'Text settings' });
    await user.click(within(panel).getByRole('button', { name: 'More: Font size' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'novelReader',
      value: { ...DEFAULT_SETTINGS.novelReader, fontSize: 20 },
    });
    await vi.waitFor(() =>
      expect(within(reader).getByRole('article')).toHaveStyle({ fontSize: '20px' }),
    );
    await user.click(within(panel).getByRole('button', { name: 'Sans' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'novelReader',
      value: { ...DEFAULT_SETTINGS.novelReader, fontSize: 20, font: 'sans' },
    });
  });

  it('turns pages with taps in paged mode', async () => {
    commands.settingsGet.mockResolvedValue(
      ok({ ...DEFAULT_SETTINGS, novelReader: { ...DEFAULT_SETTINGS.novelReader, paged: true } }),
    );
    commands.novelChapter.mockImplementation((_title: string, chapter: string) =>
      Promise.resolve(
        ok(chapter === 'c1' ? CHAPTER : { ...CHAPTER, chapter: chapterInfo('c2', 'Letter 2') }),
      ),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const pages = await screen.findByTestId('novel-pages');
    expect(screen.getByTestId('novel-progress')).toHaveTextContent('Page 1 of 1');

    // The middle third hides the controls; the right third on the last page moves on.
    vi.spyOn(pages, 'getBoundingClientRect').mockReturnValue({
      left: 0,
      width: 300,
      top: 0,
      height: 600,
    } as DOMRect);
    fireEvent.click(pages, { clientX: 150 });
    expect(screen.queryByTestId('novel-progress')).not.toBeInTheDocument();
    fireEvent.click(pages, { clientX: 290 });
    await vi.waitFor(() => expect(commands.novelChapter).toHaveBeenLastCalledWith('t1', 'c2'));
    expect(commands.novelSetProgress).toHaveBeenCalledWith('c1', 68, 68);
  });
});
