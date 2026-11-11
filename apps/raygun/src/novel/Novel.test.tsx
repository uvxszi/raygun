import type { AiModel, MangaTitleView, NovelChapter, TtsState, TtsUpdate } from '@raygun/bindings';
import { act, fireEvent, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { ANDROID_INFO, DEFAULT_SETTINGS, bindingsMock, emitEvent } from '../test/bindingsMock';
import { libraryEntry } from '../test/libraryEntry';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;
const SOURCE = 'dev.raygun.gutenberg/gutenberg';

const AI_MODEL: AiModel = {
  id: 'realcugan-up2x-conservative',
  name: 'Real-CUGAN 2× (conservative)',
  kind: 'upscale',
  description: 'Doubles images.',
  license: 'MIT',
  licenseUrl: 'https://example.com/license',
  sourceUrl: 'https://example.com',
  sizeBytes: 2_578_409,
  installed: false,
  downloading: null,
};

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

/** Letter 1's paragraphs as reading aloud gets them. */
const SPOKEN = [
  { id: 0, text: 'Letter 1!!' },
  { id: 1, text: 'It was a dreary night of Nov' },
  { id: 3, text: 'ember. Rain fell on the panes.' },
];

/** What the reader asks Rust to read aloud. */
const speech = (from: number, resume: boolean, chapter = 'c1', name = 'Letter 1') => ({
  chapterId: chapter,
  title: 'Frankenstein',
  chapter: name,
  paragraphs: chapter === 'c1' ? SPOKEN : expect.any(Array),
  from,
  resume,
  rate: 1,
});

/** Where reading aloud is, as Rust reports it. */
const update = (state: TtsState, paragraph: number, extra: Partial<TtsUpdate> = {}): TtsUpdate => ({
  state,
  chapterId: 'c1',
  paragraph,
  start: 0,
  end: 0,
  notice: null,
  ...extra,
});

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
        libraryEntry({
          titleId: 't1',
          sourceId: SOURCE,
          remoteId: '84',
          title: 'Frankenstein',
          chapters: 24,
          unread: 20,
        }),
      ]),
    );
    await AppShellWrapper.mount('/library');
    await user.click(await screen.findByTestId('library-media-novel'));

    const library = await screen.findByTestId('novel-library');
    const card = await within(library).findByTestId('library-item');
    expect(card.getAttribute('href')).toMatch(/^\/novel\/title\?source=dev\.raygun\.gutenberg/);
    // Library actions are in the top bar's menu: books from sources get updates too.
    await user.click(within(library).getByRole('button', { name: 'More options' }));
    const menu = within(library).getByRole('menu');
    expect(within(menu).getByRole('menuitem', { name: 'Downloads' })).toBeInTheDocument();
    expect(within(menu).getByRole('menuitem', { name: 'Import EPUB or PDF' })).toBeInTheDocument();
    expect(within(menu).getByRole('menuitem', { name: 'Updates' })).toBeInTheDocument();
    await user.click(within(menu).getByRole('menuitem', { name: 'Update library' }));
    expect(commands.mangaUpdateLibrary).toHaveBeenCalled();
  });

  it('shows a novel with reading percentages and download controls', async () => {
    const title: MangaTitleView = {
      id: 't1',
      hiddenNsfw: false,
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

    expect(
      await within(page).findByRole('button', { name: /Continue: Letter 2/ }),
    ).toBeInTheDocument();
    expect(within(page).getByText('45% read')).toBeInTheDocument();
    expect(within(page).getByRole('button', { name: 'Download' })).toBeEnabled();
    await userEvent.click(within(page).getByRole('button', { name: 'Download Letter 2' }));
    expect(commands.novelTitle).toHaveBeenCalledWith(SOURCE, '84', false);
    expect(commands.mangaDownload).toHaveBeenCalledWith(['c2']);
  });

  it('offers no downloads for an imported book', async () => {
    commands.novelTitle.mockResolvedValue(
      ok({
        id: 'b1',
        sourceId: 'local.epub',
        sourceName: 'local.epub',
        remoteId: 'abc',
        title: 'My Book',
        coverUrl: null,
        authors: [],
        artists: [],
        description: null,
        genres: [],
        status: 'completed',
        url: null,
        inLibrary: true,
        chapters: [chapterInfo('k1', 'One')],
        continueChapterId: 'k1',
      }),
    );
    await AppShellWrapper.mount('/novel/title?source=local.epub&id=abc');
    const page = await screen.findByTestId('manga-title-page');
    expect(await within(page).findByText('One')).toBeInTheDocument();
    expect(within(page).queryByRole('button', { name: /^Download/ })).not.toBeInTheDocument();
  });

  it('imports an EPUB from the novel library and opens it', async () => {
    const user = userEvent.setup();
    commands.uploadBegin.mockResolvedValue(ok('u1'));
    commands.uploadAppend.mockResolvedValue(ok(4));
    commands.novelImportEpubUpload.mockResolvedValue(
      ok({ sourceId: 'local.epub', novelId: 'abc' }),
    );
    commands.novelTitle.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/library?media=novel');
    const library = await screen.findByTestId('novel-library');

    // The book goes over in slices, not as one message.
    const book = new File([new Uint8Array([0x50, 0x4b, 3, 4])], 'book.epub');
    await user.upload(within(library).getByLabelText('EPUB or PDF book file'), book);
    await vi.waitFor(() =>
      expect(commands.novelImportEpubUpload).toHaveBeenCalledWith('u1', 'book.epub'),
    );
    expect(commands.uploadAppend).toHaveBeenCalledWith('u1', 'UEsDBA==');
    await vi.waitFor(() =>
      expect(commands.novelTitle).toHaveBeenCalledWith('local.epub', 'abc', false),
    );
  });

  it('reads a chapter aloud from the paragraph on screen and highlights it', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('It was a dreary night of Nov');
    FakeObserver.instances.at(-1)!.show([within(reader).getByText('It was a dreary night of Nov')]);

    await user.click(await within(reader).findByRole('button', { name: 'Read aloud' }));
    // From the paragraph on screen, or where the chapter was left aloud (Rust knows).
    expect(commands.ttsSpeak).toHaveBeenCalledWith(speech(1, true));
    const paragraph = within(reader).getByText('ember. Rain fell on the panes.');
    emitEvent('ttsEvent', update('speaking', 3, { start: 7, end: 30 }));
    expect(paragraph).toHaveAttribute('aria-current', 'true');
    // The sentence being read is marked in it.
    expect(within(reader).getByTestId('spoken-sentence')).toHaveTextContent(
      'Rain fell on the panes.',
    );
    // While reading, a tapped paragraph is read next; the toolbar steps and replays.
    commands.ttsSpeak.mockClear();
    await user.click(within(reader).getByText('It was a dreary night of Nov'));
    expect(commands.ttsSpeak).toHaveBeenLastCalledWith(speech(1, false));
    await user.click(within(reader).getByRole('button', { name: 'Skip to the next paragraph' }));
    expect(commands.ttsStep).toHaveBeenLastCalledWith(1);
    await user.click(within(reader).getByRole('button', { name: 'Read this paragraph again' }));
    expect(commands.ttsStep).toHaveBeenLastCalledWith(0);
    await user.click(
      within(reader).getByRole('button', { name: 'Read from the previous paragraph' }),
    );
    expect(commands.ttsStep).toHaveBeenLastCalledWith(-1);
    await user.click(within(reader).getByRole('button', { name: 'Stop reading aloud' }));
    expect(commands.ttsStop).toHaveBeenCalled();
    expect(paragraph).not.toHaveAttribute('aria-current');
    expect(within(reader).queryByTestId('spoken-sentence')).not.toBeInTheDocument();
  });

  it('pauses reading aloud on the sentence and resumes from it', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await user.click(await within(reader).findByRole('button', { name: 'Read aloud' }));
    emitEvent('ttsEvent', update('speaking', 1, { start: 0, end: 28 }));

    const controls = within(reader).getByRole('toolbar', { name: 'Reading aloud' });
    await user.click(within(controls).getByRole('button', { name: 'Pause reading aloud' }));
    expect(commands.ttsPause).toHaveBeenCalled();
    expect(commands.ttsStop).not.toHaveBeenCalled();
    // Paused, the sentence stays marked and the button resumes.
    expect(within(reader).getByTestId('spoken-sentence')).toHaveTextContent(
      'It was a dreary night of Nov',
    );
    await user.click(within(controls).getByRole('button', { name: 'Resume reading aloud' }));
    expect(commands.ttsResume).toHaveBeenCalled();
    expect(commands.ttsSpeak).toHaveBeenCalledTimes(1);

    // Paused from the notification or a headset: the reader follows.
    emitEvent('ttsEvent', update('paused', 1, { start: 0, end: 28 }));
    expect(
      await within(controls).findByRole('button', { name: 'Resume reading aloud' }),
    ).toHaveAttribute('aria-pressed', 'true');
    emitEvent('ttsEvent', update('speaking', 3, { start: 0, end: 6 }));
    expect(
      await within(controls).findByRole('button', { name: 'Pause reading aloud' }),
    ).toBeInTheDocument();
    // Stopped from the notification: the controls go, nothing else is asked.
    emitEvent('ttsEvent', update('stopped', 3));
    await vi.waitFor(() =>
      expect(within(reader).queryByRole('toolbar', { name: 'Reading aloud' })).toBeNull(),
    );
    expect(commands.ttsStop).not.toHaveBeenCalled();
  });

  it('says when another voice reads than the one set', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await user.click(await within(reader).findByRole('button', { name: 'Read aloud' }));
    emitEvent('ttsEvent', update('speaking', 1, { notice: { kind: 'noVoice', language: 'de' } }));
    expect(await within(reader).findByRole('status')).toHaveTextContent(
      "No downloaded voice reads German: the device's voice reads it.",
    );
  });

  it('translates a chapter through AI layers, bottom first, from the place being read', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('It was a dreary night of Nov');

    await user.click(within(reader).getByRole('button', { name: 'AI layers' }));
    const sheet = await screen.findByRole('dialog', { name: 'AI layers' });
    // Books only translate; comics also upscale and restore.
    expect(within(sheet).queryByRole('button', { name: 'Upscale' })).not.toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Translate' }));
    await user.click(within(sheet).getByRole('button', { name: 'Translate' }));
    const layers = within(sheet).getAllByRole('listitem');
    expect(layers).toHaveLength(2);
    // The new layer is on top: it translates what the one under it made.
    await user.selectOptions(
      within(layers[0]!).getByRole('combobox', { name: 'Translate into' }),
      'de',
    );

    await within(reader).findByText('[de] [en] ember. Rain fell on the panes.');
    expect(commands.readerAiTranslate).toHaveBeenCalledWith(
      expect.objectContaining({
        to: 'en',
        texts: expect.arrayContaining(['It was a dreary night of Nov']),
      }),
    );
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'readerAi',
      value: expect.objectContaining({
        novel: [expect.objectContaining({ to: 'en' }), expect.objectContaining({ to: 'de' })],
      }),
    });

    // A layer turned off is skipped.
    await user.click(within(layers[0]!).getByRole('switch'));
    await within(reader).findByText('[en] ember. Rain fell on the panes.');
  });

  it('keeps what paragraphs show while changed layers translate them again', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('It was a dreary night of Nov');
    await user.click(within(reader).getByRole('button', { name: 'AI layers' }));
    const sheet = await screen.findByRole('dialog', { name: 'AI layers' });
    await user.click(within(sheet).getByRole('button', { name: 'Translate' }));
    await within(reader).findByText('[en] ember. Rain fell on the panes.');

    // German comes when the test lets it: the English stays meanwhile, not the book's words.
    const pending: (() => void)[] = [];
    commands.readerAiTranslate.mockImplementation(
      (request: { texts: string[]; to?: string }) =>
        new Promise((resolve) => {
          pending.push(() =>
            resolve(ok(request.texts.map((text) => `[${request.to ?? ''}] ${text}`))),
          );
        }),
    );
    await user.selectOptions(within(sheet).getByRole('combobox', { name: 'Translate into' }), 'de');
    await vi.waitFor(() => expect(pending.length).toBeGreaterThan(0));
    expect(within(reader).getByText('[en] ember. Rain fell on the panes.')).toBeInTheDocument();
    expect(within(reader).queryByText('ember. Rain fell on the panes.')).not.toBeInTheDocument();
    await act(async () => {
      for (const answer of pending.splice(0)) answer();
    });
    await within(reader).findByText('[de] ember. Rain fell on the panes.');
    expect(
      within(reader).queryByText('[en] ember. Rain fell on the panes.'),
    ).not.toBeInTheDocument();

    // Off, the book's own words show at once.
    await user.click(within(sheet).getByRole('switch'));
    expect(within(reader).getByText('ember. Rain fell on the panes.')).toBeInTheDocument();
  });

  it('offers the models this device needs to translate a book', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    commands.videoSubtitlesTranslators.mockResolvedValue(
      ok([
        {
          id: 'device',
          kind: null,
          name: '',
          models: ['opus-mt-en-de'],
          downloadBytes: 3e8,
          supported: true,
        },
      ]),
    );
    commands.aiModels.mockResolvedValue([
      {
        ...AI_MODEL,
        id: 'opus-mt-en-de',
        name: 'English → German',
        installed: false,
        sizeBytes: 3e8,
      },
    ]);
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('It was a dreary night of Nov');
    await user.click(within(reader).getByRole('button', { name: 'AI layers' }));
    const sheet = await screen.findByRole('dialog', { name: 'AI layers' });
    await user.click(within(sheet).getByRole('button', { name: 'Translate' }));
    await user.selectOptions(within(sheet).getByRole('combobox', { name: 'Translate into' }), 'de');
    expect(await within(sheet).findByText(/Needs English → German/)).toBeInTheDocument();
    await user.click(within(sheet).getByRole('button', { name: 'Download' }));
    expect(commands.aiModelDownload).toHaveBeenCalledWith('opus-mt-en-de');
  });

  it('keeps reading aloud in the chapter opened, from its start', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    // The first letter loads when the test lets it.
    let release: (() => void) | undefined;
    commands.novelChapter.mockImplementation((_title: string, chapter: string) =>
      chapter === 'c1'
        ? new Promise((resolve) => {
            release = () => resolve(ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }));
          })
        : Promise.resolve(
            ok({
              ...CHAPTER,
              chapter: chapterInfo('c2', 'Letter 2'),
              blocks: [{ kind: 'paragraph' as const, text: 'A second letter.' }],
              previousChapterId: 'c1',
              nextChapterId: null,
            }),
          ),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c2');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('A second letter.');
    await user.click(await within(reader).findByRole('button', { name: 'Read aloud' }));
    expect(commands.ttsSpeak).toHaveBeenCalledWith({
      chapterId: 'c2',
      title: 'Frankenstein',
      chapter: 'Letter 2',
      paragraphs: [{ id: 0, text: 'A second letter.' }],
      from: 0,
      resume: true,
      rate: 1,
    });
    emitEvent('ttsEvent', update('speaking', 0, { chapterId: 'c2' }));
    commands.ttsSpeak.mockClear();
    commands.ttsStop.mockClear();

    await user.click(within(reader).getByRole('button', { name: 'Go to the previous chapter' }));
    // The chapter left stops at once; a late report that it ended changes nothing.
    expect(commands.ttsStop).toHaveBeenCalled();
    await vi.waitFor(() => expect(release).toBeDefined());
    emitEvent('ttsEvent', update('ended', 0, { chapterId: 'c2' }));
    act(() => release?.());
    expect(await within(reader).findByText('It was a dreary night of Nov')).toBeInTheDocument();
    await vi.waitFor(() => expect(commands.ttsSpeak).toHaveBeenCalledWith(speech(0, false)));
    expect(commands.ttsSpeak).toHaveBeenCalledTimes(1);
    expect(commands.novelChapter).toHaveBeenLastCalledWith('t1', 'c1');
    expect(within(reader).getByRole('toolbar', { name: 'Reading aloud' })).toBeInTheDocument();
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

  it('keeps its bars over the page area, beside the rail and the side panel', async () => {
    // The page area starts 72 px in and is 800 px wide in a 1024 px window.
    const page = (element: Element) => element.getAttribute('data-testid') === 'page-content';
    const rect = vi
      .spyOn(HTMLElement.prototype, 'getBoundingClientRect')
      .mockImplementation(function (this: HTMLElement) {
        return (page(this) ? { left: 72, width: 800 } : { left: 0, width: 0 }) as DOMRect;
      });
    const width = vi
      .spyOn(HTMLElement.prototype, 'clientWidth', 'get')
      .mockImplementation(function (this: HTMLElement) {
        return page(this) ? 800 : 0;
      });
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByRole('heading', { name: 'Letter 1!!' });

    const shell = screen.getByTestId('page-content').closest<HTMLElement>('[style]')!;
    expect(shell.style.getPropertyValue('--main-left')).toBe('72px');
    expect(shell.style.getPropertyValue('--main-right')).toBe(`${window.innerWidth - 872}px`);
    expect(shell.style.getPropertyValue('--main-width')).toBe('800px');
    const bottom = within(reader).getByTestId('novel-progress').closest('.fixed')!;
    const header = within(reader).getByRole('button', { name: 'Close reader' }).closest('header')!;
    for (const bar of [bottom, header]) {
      expect(bar).toHaveClass('left-[var(--main-left,0px)]', 'right-[var(--main-right,0px)]');
      expect(bar).not.toHaveClass('inset-x-0');
    }
    rect.mockRestore();
    width.mockRestore();
  });

  it('reads aloud from a paragraph held down while word lookups are off', async () => {
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.settingsGet.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        dictionary: { enabled: false, order: [], off: [], language: '' },
      }),
    );
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const paragraph = await within(reader).findByText('ember. Rain fell on the panes.');
    await within(reader).findByRole('button', { name: 'Read aloud' });

    vi.useFakeTimers({ shouldAdvanceTime: true });
    fireEvent.pointerDown(paragraph, { clientX: 10, clientY: 10 });
    await act(() => vi.advanceTimersByTimeAsync(600));
    fireEvent.pointerUp(paragraph);
    vi.useRealTimers();
    expect(commands.ttsSpeak).toHaveBeenCalledWith(speech(3, false));
    expect(paragraph).toHaveAttribute('aria-current', 'true');
    expect(within(reader).getByRole('toolbar', { name: 'Reading aloud' })).toBeInTheDocument();
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
    // Text size is what's added to the screen's own (M46.48): typed, or on the slider.
    const size = within(panel).getByRole('spinbutton', { name: 'Font size' });
    await user.clear(size);
    await user.type(size, '2{Enter}');
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'novelReader',
      value: { ...DEFAULT_SETTINGS.novelReader, textSize: 2 },
    });
    await vi.waitFor(() =>
      // The screen's size plus 2 px, times the wide-window scale.
      expect(within(reader).getByRole('article').getAttribute('style')).toContain(
        'calc((1.125rem + 2px)',
      ),
    );
    await user.click(within(panel).getByRole('button', { name: 'Sans' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'novelReader',
      value: { ...DEFAULT_SETTINGS.novelReader, textSize: 2, font: 'sans' },
    });
  });

  it('indents paragraphs by the amount set, none at 0', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
    commands.settingsUpdate.mockImplementation((change: { value: unknown }) =>
      Promise.resolve(ok({ ...DEFAULT_SETTINGS, novelReader: change.value })),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const paragraph = await within(reader).findByText('It was a dreary night of Nov');
    expect(paragraph.style.textIndent).toBe('');

    await user.click(within(reader).getByRole('button', { name: 'Text settings' }));
    const panel = within(reader).getByRole('dialog', { name: 'Text settings' });
    const indent = within(panel).getByRole('spinbutton', { name: 'Paragraph indent' });
    expect(indent).toHaveValue(0);
    expect(within(panel).getByRole('slider', { name: 'Paragraph indent' })).toBeInTheDocument();
    // Typed in em, kept in tenths.
    await user.clear(indent);
    await user.type(indent, '1.5{Enter}');
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'novelReader',
      value: { ...DEFAULT_SETTINGS.novelReader, indent: 15 },
    });
    await vi.waitFor(() =>
      expect(within(reader).getByText('It was a dreary night of Nov').style.textIndent).toBe(
        '1.5em',
      ),
    );

    await user.click(within(panel).getByRole('button', { name: 'Reset Paragraph indent' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'novelReader',
      value: { ...DEFAULT_SETTINGS.novelReader, indent: 0 },
    });
    await vi.waitFor(() =>
      expect(within(reader).getByText('It was a dreary night of Nov').style.textIndent).toBe(''),
    );
  });

  it('reads in a font of the device’s, in front of its own typeface', async () => {
    const user = userEvent.setup();
    commands.appInfo.mockResolvedValue({ ...ANDROID_INFO, platform: 'windows' });
    commands.fontsList.mockResolvedValue(ok([{ family: 'Literata', faces: [] }]));
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
    let stored = { ...DEFAULT_SETTINGS };
    commands.settingsGet.mockImplementation(() => Promise.resolve(ok(stored)));
    commands.settingsUpdate.mockImplementation((change: { key: string; value: unknown }) => {
      stored = { ...stored, [change.key]: change.value };
      return Promise.resolve(ok(stored));
    });
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByText('It was a dreary night of Nov');

    await user.click(within(reader).getByRole('button', { name: 'Text settings' }));
    const panel = within(reader).getByRole('dialog', { name: 'Text settings' });
    await user.click(within(panel).getByRole('button', { name: 'More fonts' }));
    await user.click(await screen.findByRole('radio', { name: 'Literata' }));
    expect(commands.settingsUpdate).toHaveBeenLastCalledWith({
      key: 'fonts',
      value: { interface: '', reading: 'Literata' },
    });
    await vi.waitFor(() => {
      const style = within(reader).getByRole('article').getAttribute('style') ?? '';
      expect(style).toMatch(/font-family: "?Literata"?, Georgia/);
    });
    expect(within(panel).getByRole('button', { name: 'Literata' })).toHaveAttribute(
      'aria-pressed',
      'true',
    );
    expect(within(panel).getByRole('button', { name: 'Serif' })).toHaveAttribute(
      'aria-pressed',
      'false',
    );
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

  it('turns pages with the keyboard in paged mode', async () => {
    const user = userEvent.setup();
    commands.settingsGet.mockResolvedValue(
      ok({ ...DEFAULT_SETTINGS, novelReader: { ...DEFAULT_SETTINGS.novelReader, paged: true } }),
    );
    commands.novelChapter.mockImplementation((_title: string, chapter: string) =>
      Promise.resolve(
        ok(chapter === 'c1' ? CHAPTER : { ...CHAPTER, chapter: chapterInfo('c2', 'Letter 2') }),
      ),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    await screen.findByTestId('novel-pages');
    // Nothing needs focusing first: one page, so Space moves to the next chapter.
    await user.keyboard(' ');
    await vi.waitFor(() => expect(commands.novelChapter).toHaveBeenLastCalledWith('t1', 'c2'));
  });

  it('colours the page and justifies the text as set', async () => {
    commands.settingsGet.mockResolvedValue(
      ok({
        ...DEFAULT_SETTINGS,
        novelReader: { ...DEFAULT_SETTINGS.novelReader, theme: 'sepia', justify: true },
      }),
    );
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const paragraph = await within(reader).findByText('It was a dreary night of Nov');
    expect(reader).toHaveAttribute('data-theme-page', 'sepia');
    expect(reader.style.backgroundColor).toBe('rgb(244, 236, 216)');
    expect(paragraph.style.textAlign).toBe('justify');
  });

  it('says when reading aloud stops on its own', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.ttsSpeak.mockResolvedValue(ok(null));
    commands.novelChapter.mockResolvedValue(
      ok({ ...CHAPTER, chapter: chapterInfo('c1', 'Letter 1') }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await user.click(await within(reader).findByRole('button', { name: 'Read aloud' }));
    emitEvent('ttsEvent', update('error', 1, { notice: { kind: 'engineFailed' } }));
    expect(await within(reader).findByRole('status')).toHaveTextContent('Reading aloud stopped.');
    expect(within(reader).getByRole('button', { name: 'Read aloud' })).toBeInTheDocument();
  });
});
