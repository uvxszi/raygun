import type { MangaChapterInfo, NovelChapter, TextPart } from '@raygun/bindings';
import { render, screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';
import BOOK_TEXT_CSS from './bookText.css?raw';
import { inSideScroller } from './code';
import { spokenParts, spokenText, tableSpeech } from './spoken';
import type { TableBlock } from './spoken';
import { TextParts } from './TextParts';
import { textOffsets } from './useNovel';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const info: MangaChapterInfo = {
  id: 'c1',
  name: 'Document Structure',
  number: null,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
};

const text = (words: string): TextPart => ({ kind: 'text', text: words });
const code = (words: string, italic = false): TextPart => ({
  kind: 'styled',
  text: words,
  code: true,
  ...(italic ? { italic: true } : {}),
});

/** Wikibooks' LaTeX book: the sectioning commands, each with its level and a comment. */
const SECTIONING: TableBlock = {
  kind: 'table',
  text: 'Command\tLevel\tComment\n\\part{part}\t-1\tnot in letters\n\\chapter{chapter}\t0\tonly books and reports',
  rows: [
    [
      { parts: [text('Command')], header: true },
      { parts: [text('Level')], header: true },
      { parts: [text('Comment')], header: true },
    ],
    [
      { parts: [code('\\part{'), code('part', true), code('}')] },
      { parts: [text('-1')] },
      { parts: [text('not in letters')] },
    ],
    [
      { parts: [code('\\chapter{'), code('chapter', true), code('}')] },
      { parts: [text('0')] },
      { parts: [text('only books and\nreports')] },
    ],
  ],
};

const STYLED: TextPart[] = [
  text('The '),
  { kind: 'styled', text: 'preamble', italic: true },
  text(' comes first. Then '),
  code('\\begin{document}'),
  text(' starts it.'),
];

const CHAPTER: NovelChapter = {
  titleId: 't1',
  title: 'LaTeX',
  chapter: info,
  blocks: [
    {
      kind: 'paragraph',
      text: 'The preamble comes first. Then \\begin{document} starts it.',
      parts: STYLED,
    },
    { kind: 'paragraph', text: 'Seven levels of sections', list: { marker: '1.', depth: 1 } },
    { kind: 'paragraph', text: 'Each one inside the one above', list: { marker: '◦', depth: 2 } },
    SECTIONING,
    { kind: 'paragraph', text: 'Numbers are added for you.' },
  ],
  previousChapterId: null,
  nextChapterId: null,
};

describe('Tables, lists and text set apart in book text', () => {
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
    commands.novelChapter.mockResolvedValue(ok(CHAPTER));
  });

  afterEach(() => {
    vi.unstubAllGlobals();
  });

  it('shows a table of data as a table, its cells with their code and italics', async () => {
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const table = await within(reader).findByRole('table');
    expect(
      within(table)
        .getAllByRole('columnheader')
        .map((cell) => cell.textContent),
    ).toEqual(['Command', 'Level', 'Comment']);
    const rows = within(table).getAllByRole('row');
    expect(rows).toHaveLength(3);
    const command = within(rows[1]!).getAllByRole('cell')[0]!;
    expect(command.textContent).toBe('\\part{part}');
    // The placeholder in italics, inside the command's code.
    expect(command.querySelector('em > code.book-inline-code')?.textContent).toBe('part');
    expect(command.querySelectorAll('code.book-inline-code')).toHaveLength(3);
    // A cell keeps its own line break.
    const comment = within(rows[2]!).getAllByRole('cell')[2]!;
    expect(comment.textContent).toBe('only books and\nreports');
    // The table scrolls sideways in its own box when wider than the column.
    const box = table.parentElement!;
    expect(box).toHaveClass('book-table');
    expect(box.dataset.block).toBe('3');
    // Nothing of it is a paragraph of its own.
    expect(within(reader).queryByText('not in letters', { selector: 'p' })).toBeNull();
  });

  it('sets italics and code apart in paragraphs, and lists with their marks', async () => {
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    const preamble = await within(reader).findByText('preamble');
    expect(preamble.tagName).toBe('EM');
    const typed = within(reader).getByText('\\begin{document}');
    expect(typed).toHaveClass('book-inline-code');
    expect(typed.closest('p')?.textContent).toBe(
      'The preamble comes first. Then \\begin{document} starts it.',
    );
    const first = within(reader).getByText('Seven levels of sections');
    expect(first).toHaveClass('book-item');
    // The mark is drawn, not in the text.
    expect(first.dataset.marker).toBe('1.');
    expect(first.textContent).toBe('Seven levels of sections');
    expect(first.style.getPropertyValue('--item-depth')).toBe('1');
    const inner = within(reader).getByText('Each one inside the one above');
    expect(inner.style.getPropertyValue('--item-depth')).toBe('2');
  });

  it('reads a table aloud row by row, each cell after its column', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.ttsSpeak.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');
    await within(reader).findByRole('table');
    await user.click(within(reader).getByRole('button', { name: 'Read aloud' }));
    expect(commands.ttsSpeak).toHaveBeenCalledWith(
      expect.objectContaining({
        paragraphs: [
          { id: 0, text: 'The preamble comes first. Then \\begin{document} starts it.' },
          { id: 1, text: 'Seven levels of sections' },
          { id: 2, text: 'Each one inside the one above' },
          {
            id: 3,
            text: 'Command: \\part{part}, Level: -1, Comment: not in letters. Command: \\chapter{chapter}, Level: 0, Comment: only books and reports.',
          },
          { id: 4, text: 'Numbers are added for you.' },
        ],
      }),
    );
  });

  it('keeps tables in their own box in pages too', async () => {
    commands.settingsGet.mockResolvedValue(
      ok({ ...DEFAULT_SETTINGS, novelReader: { ...DEFAULT_SETTINGS.novelReader, paged: true } }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const pages = await screen.findByTestId('novel-pages');
    const table = await within(pages).findByRole('table');
    expect(table.closest('.book-pages')).not.toBeNull();
    expect(table.parentElement).toHaveClass('book-table');
  });

  it('marks the sentence read aloud inside text set apart', () => {
    // "The preamble comes first." is the first sentence of the spoken text.
    const spoken = spokenParts(STYLED);
    const end = spoken.indexOf('.') + 1;
    render(
      <p data-testid="paragraph">
        <TextParts parts={STYLED} sentence={{ start: 0, end }} />
      </p>,
    );
    const marks = screen.getAllByTestId('spoken-sentence');
    expect(marks.map((mark) => mark.textContent).join('')).toBe('The preamble comes first.');
    // The italic word is marked inside its own element.
    expect(screen.getByText('preamble').closest('em')).not.toBeNull();
    expect(screen.getByTestId('paragraph').textContent).toBe(spoken);
  });

  it('marks a later sentence that starts in code', () => {
    const spoken = spokenParts(STYLED);
    const start = spoken.indexOf('Then');
    render(
      <p>
        <TextParts parts={STYLED} sentence={{ start, end: spoken.length }} />
      </p>,
    );
    const marks = screen.getAllByTestId('spoken-sentence');
    expect(marks.map((mark) => mark.textContent).join('')).toBe(
      'Then \\begin{document} starts it.',
    );
    expect(marks[1]!.closest('code')).not.toBeNull();
  });

  it('says tables with spans, captions and no header row', () => {
    const block: TableBlock = {
      kind: 'table',
      text: '',
      caption: [text('Greek letters')],
      rows: [
        [
          { parts: [text('Letter')], header: true, colspan: 2 },
          { parts: [text('Note')], header: true },
        ],
        [
          { parts: [code('\\alpha')] },
          { parts: [{ kind: 'math', mathml: '<math/>', text: 'α', speech: 'alpha' }] },
          { parts: [text('first')], rowspan: 2 },
        ],
        [{ parts: [code('\\beta')] }, { parts: [text('β')] }],
      ],
    };
    expect(tableSpeech(block)).toBe(
      'Greek letters. Letter: \\alpha, Letter: alpha, Note: first. Letter: \\beta, Letter: β.',
    );
    const plain: TableBlock = {
      kind: 'table',
      text: '',
      rows: [
        [{ parts: [text('a')] }, { parts: [text('b?')] }],
        [{ parts: [] }, { parts: [text('d')] }],
      ],
    };
    expect(tableSpeech(plain)).toBe('a, b? d.');
    expect(spokenText(SECTIONING)).toContain('Level: -1');
    expect(spokenText({ kind: 'table', text: '', rows: [[{ parts: [] }]] })).toBeNull();
  });

  it('counts a table in the chapter text and lets its swipes scroll it', () => {
    const { starts, total } = textOffsets(CHAPTER.blocks);
    expect(starts[4]! - starts[3]!).toBe(SECTIONING.text.length);
    expect(total).toBeGreaterThan(SECTIONING.text.length);
    const box = document.createElement('div');
    box.className = 'book-table';
    const cell = document.createElement('td');
    box.append(cell);
    document.body.append(box);
    Object.defineProperty(box, 'scrollWidth', { configurable: true, value: 900 });
    Object.defineProperty(box, 'clientWidth', { configurable: true, value: 300 });
    expect(inSideScroller(cell)).toBe(true);
    box.remove();
  });

  it('styles tables to scroll in their box and cells to wrap at spaces', () => {
    const rule = (selector: string) => {
      const at = BOOK_TEXT_CSS.indexOf(`${selector} {`);
      expect(at, selector).toBeGreaterThanOrEqual(0);
      return BOOK_TEXT_CSS.slice(at, BOOK_TEXT_CSS.indexOf('}', at));
    };
    expect(rule('.book-table')).toContain('overflow-x: auto');
    expect(rule('.book-table')).toContain('max-width: 100%');
    const cells = rule('.book-table th,\n.book-table td');
    expect(cells).toContain('overflow-wrap: normal');
    expect(cells).toContain('white-space: pre-line');
    expect(rule('.book-item[data-marker]::before')).toContain('content: attr(data-marker)');
  });
});
