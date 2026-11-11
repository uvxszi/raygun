import type { MangaChapterInfo, NovelChapter, PdfChapter } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { bindingsMock } from '../test/bindingsMock';
import { safeMath } from './mathml';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const info = (id: string, extra: Partial<MangaChapterInfo> = {}): MangaChapterInfo => ({
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
  ...extra,
});

const FORMULAS: NovelChapter = {
  titleId: 't1',
  title: 'Calculus',
  chapter: info('c1'),
  blocks: [
    {
      kind: 'paragraph',
      text: 'Let x^2 grow, with H₂O.',
      parts: [
        { kind: 'text', text: 'Let ' },
        {
          kind: 'math',
          mathml: '<math><msup><mi>x</mi><mn>2</mn></msup></math>',
          text: 'x^2',
          speech: 'x squared',
        },
        { kind: 'text', text: ' grow, with H' },
        { kind: 'sub', text: '2' },
        { kind: 'text', text: 'O.' },
      ],
    },
    {
      kind: 'math',
      mathml:
        '<math display="block"><mi onclick="alert(1)">y</mi><script>alert(2)</script><mo>=</mo><mn>1</mn></math>',
      text: 'y = 1',
      speech: 'y equals 1',
    },
  ],
  previousChapterId: null,
  nextChapterId: null,
};

const PDF: PdfChapter = {
  pages: ['http://127.0.0.1:9/pdf/lab/0', 'http://127.0.0.1:9/pdf/lab/1'],
  firstPage: 1,
  readerMode: 'pagedLtr',
  ownReaderMode: 'pagedLtr',
  contents: [{ title: 'Section 2', page: 1, level: 0 }],
};

const PDF_CHAPTER: NovelChapter = {
  titleId: 't1',
  title: 'Lighthouses',
  chapter: info('c1'),
  blocks: [],
  previousChapterId: null,
  nextChapterId: null,
  pdf: PDF,
};

describe('Formulas and PDFs in the book reader', () => {
  beforeEach(() => {
    AppShellWrapper.seedCore();
    vi.stubGlobal(
      'IntersectionObserver',
      class {
        observe() {}
        disconnect() {}
      },
    );
    Element.prototype.scrollIntoView = vi.fn();
    localStorage.clear();
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('keeps only MathML elements and safe attributes', () => {
    const math = safeMath(
      '<math><mi onclick="alert(1)" style="background:url(https://x)">a</mi><a href="https://x"><mn>1</mn></a><script>alert(2)</script></math>',
    )!;
    expect(math.localName).toBe('math');
    expect(math.querySelector('script, a')).toBeNull();
    const letter = math.querySelector('mi')!;
    expect(letter.getAttribute('onclick')).toBeNull();
    expect(letter.getAttribute('style')).toBeNull();
    expect(safeMath('<p>no formula</p>')).toBeNull();
  });

  it('draws formulas in the text and reads them aloud in words', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.ttsSpeak.mockResolvedValue(ok(null));
    commands.novelChapter.mockResolvedValue(ok(FORMULAS));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');

    const formulas = await within(reader).findAllByRole('math');
    expect(formulas).toHaveLength(2);
    expect(formulas[0]!.getAttribute('aria-label')).toBe('x squared');
    expect(formulas[0]!.querySelector('msup')).not.toBeNull();
    // On a line of its own, without what isn't MathML.
    expect(formulas[1]!.closest('[data-display]')).not.toBeNull();
    expect(formulas[1]!.querySelector('mo')?.textContent).toBe('=');
    expect(formulas[1]!.querySelector('script')).toBeNull();
    expect(formulas[1]!.querySelector('mi')!.getAttribute('onclick')).toBeNull();
    expect(within(reader).getByText('2', { selector: 'sub' })).toBeInTheDocument();

    await user.click(await within(reader).findByRole('button', { name: 'Read aloud' }));
    expect(commands.ttsSpeak).toHaveBeenCalledWith(
      [
        { id: 0, text: 'Let x squared grow, with H2O.' },
        { id: 1, text: 'y equals 1' },
      ],
      1,
    );
  });

  it('reads a PDF chapter as pages drawn at the screen width, and as its text', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(ok(PDF_CHAPTER));
    commands.novelPdfText.mockResolvedValue(
      ok({
        blocks: [
          { kind: 'heading', text: 'Chapter Two' },
          { kind: 'paragraph', text: 'The keeper climbed the stairs.' },
        ],
        pageStarts: [0, 0],
      }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    const page = await within(reader).findByTestId('manga-page');
    expect(page.getAttribute('src')).toMatch(/^http:\/\/127\.0\.0\.1:9\/pdf\/lab\/0\?w=\d+$/);
    expect(within(reader).getByTestId('page-indicator')).toHaveTextContent('Page 1 of 2');

    await user.click(within(reader).getByRole('button', { name: 'Read as text' }));
    const text = await screen.findByTestId('novel-reader');
    expect(await within(text).findByText('The keeper climbed the stairs.')).toBeInTheDocument();
    expect(commands.novelPdfText).toHaveBeenCalledWith('t1', 'c1');
    expect(localStorage.getItem('raygun.pdfView.t1')).toBe('text');

    await user.click(within(text).getByRole('button', { name: 'Read as pages' }));
    expect(await screen.findByTestId('manga-reader')).toBeInTheDocument();
  });

  it('finds words in a PDF and opens the page they are on', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(ok(PDF_CHAPTER));
    commands.novelPdfSearch.mockResolvedValue(
      ok([
        {
          chapterId: 'c1',
          page: 1,
          pageNumber: 2,
          before: 'the old ',
          matched: 'lighthouse',
          after: ' every night',
        },
      ]),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await within(reader).findByTestId('manga-page');

    await user.click(within(reader).getByRole('button', { name: 'Search in the PDF' }));
    const dialog = await screen.findByRole('dialog', { name: 'Search in the PDF' });
    await user.type(
      within(dialog).getByRole('searchbox', { name: 'Words to find' }),
      'lighthouse{Enter}',
    );
    expect(commands.novelPdfSearch).toHaveBeenCalledWith('t1', 'c1', 'lighthouse');
    const hit = await within(dialog).findByRole('button', { name: /Page 2/ });
    expect(within(hit).getByText('lighthouse', { selector: 'mark' })).toBeInTheDocument();
    await user.click(hit);

    await vi.waitFor(() =>
      expect(within(reader).getByTestId('page-indicator')).toHaveTextContent('Page 2 of 2'),
    );
  });

  it('lists a PDF chapter’s bookmarks and opens one', async () => {
    const user = userEvent.setup();
    commands.novelChapter.mockResolvedValue(ok(PDF_CHAPTER));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('manga-reader');
    await within(reader).findByTestId('manga-page');

    await user.click(within(reader).getByRole('button', { name: 'Contents' }));
    const dialog = await screen.findByRole('dialog', { name: 'Contents' });
    await user.click(within(dialog).getByRole('button', { name: /Section 2/ }));
    await vi.waitFor(() =>
      expect(within(reader).getByTestId('page-indicator')).toHaveTextContent('Page 2 of 2'),
    );
  });
});
