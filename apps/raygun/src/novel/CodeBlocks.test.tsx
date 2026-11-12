import type { MangaChapterInfo, NovelChapter } from '@raygun/bindings';
import { screen, within } from '@testing-library/react';
import userEvent from '@testing-library/user-event';

import { AppShellWrapper } from '../shell/AppShell.test-wrapper';
import { DEFAULT_SETTINGS, bindingsMock } from '../test/bindingsMock';
import BOOK_TEXT_CSS from './bookText.css?raw';
import { codeReadsAsWords, inSideScroller, languageLabel } from './code';
import { spokenText } from './spoken';

vi.mock('@raygun/bindings', async () => (await import('../test/bindingsMock')).bindingsMock);

const ok = <T,>(data: T) => ({ status: 'ok' as const, data });
const { commands } = bindingsMock;

const info: MangaChapterInfo = {
  id: 'c1',
  name: 'Document structure',
  number: null,
  volume: null,
  scanlator: null,
  dateUpload: null,
  read: false,
  lastPage: null,
  totalPages: null,
  download: null,
};

/** Wikibooks' LaTeX book: an example document between two paragraphs, one with a long address. */
const LATEX =
  '\\documentclass{report}\n\\usepackage{color}\n\n\\begin{document}\n  ...\n\\end{document}';
const ADDRESS =
  'https://en.wikibooks.org/wiki/LaTeX/Document_Structure#Document_classes_and_their_options_for_reports';
const CHAPTER: NovelChapter = {
  titleId: 't1',
  title: 'LaTeX',
  chapter: info,
  blocks: [
    { kind: 'paragraph', text: 'To use the color package, you would type:' },
    { kind: 'code', text: LATEX, language: 'latex' },
    { kind: 'paragraph', text: `More at ${ADDRESS} for reports.` },
  ],
  previousChapterId: null,
  nextChapterId: null,
};

/** The declarations of `selector`'s rule in a style sheet. */
const ruleOf = (css: string, selector: string) => {
  const at = css.indexOf(`${selector} {`);
  expect(
    at,
    `a rule for ${selector} in ${JSON.stringify(css.slice(0, 300))}`,
  ).toBeGreaterThanOrEqual(0);
  return css.slice(at, css.indexOf('}', at));
};

describe('Code and long words in book text', () => {
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

  it('shows code as written, under its language, and reads aloud around it', async () => {
    const user = userEvent.setup();
    commands.ttsAvailable.mockResolvedValue(ok(true));
    commands.ttsSpeak.mockResolvedValue(ok(null));
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const reader = await screen.findByTestId('novel-reader');

    const example = await within(reader).findByRole('figure', { name: 'LaTeX code' });
    expect(example).toHaveClass('book-code');
    expect(within(example).getByText('LaTeX')).toBeInTheDocument();
    // Its lines and spacing as written, not run together into a paragraph.
    expect(example.querySelector('pre > code')?.textContent).toBe(LATEX);
    expect(
      within(reader).queryByText(/\\documentclass\{report\} \\usepackage/, { selector: 'p' }),
    ).toBeNull();

    // Long words wrap in the column, code scrolls in its box, and nothing makes the page scroll
    // sideways.
    const article = example.closest('article')!;
    expect(article).toHaveClass('book-text');
    expect(getComputedStyle(article).overflowWrap).toBe('anywhere');
    const lines = getComputedStyle(example.querySelector('pre')!);
    expect([lines.whiteSpace, lines.overflowX]).toEqual(['pre', 'auto']);
    expect(within(reader).getByTestId('novel-scroll')).toHaveClass('overflow-x-clip');

    await user.click(within(reader).getByRole('button', { name: 'Read aloud' }));
    expect(commands.ttsSpeak).toHaveBeenCalledWith(
      expect.objectContaining({
        paragraphs: [
          { id: 0, text: 'To use the color package, you would type:' },
          { id: 2, text: `More at ${ADDRESS} for reports.` },
        ],
      }),
    );
  });

  it('keeps code in its own box in pages too', async () => {
    commands.settingsGet.mockResolvedValue(
      ok({ ...DEFAULT_SETTINGS, novelReader: { ...DEFAULT_SETTINGS.novelReader, paged: true } }),
    );
    await AppShellWrapper.mount('/novel/read?title=t1&chapter=c1');
    const pages = await screen.findByTestId('novel-pages');
    const example = await within(pages).findByRole('figure', { name: 'LaTeX code' });
    expect(example.parentElement).toHaveClass('book-text');
  });

  it('wraps long words, and scrolls code and wide formulas in their own boxes', () => {
    expect(ruleOf(BOOK_TEXT_CSS, '.book-text')).toContain('overflow-wrap: anywhere');
    // Normal words never split: no `word-break` that would break them anywhere.
    expect(BOOK_TEXT_CSS).not.toMatch(/word-break:\s*break-all/);
    const code = ruleOf(BOOK_TEXT_CSS, '.book-code pre');
    expect(code).toContain('white-space: pre;');
    expect(code).toContain('overflow-x: auto');
    expect(code).toContain('overflow-wrap: normal');
    const formula = ruleOf(BOOK_TEXT_CSS, '.book-text .math-formula:not(.math-display)');
    expect(formula).toContain('max-width: 100%');
    expect(formula).toContain('overflow-x: auto');
    expect(formula).toContain('baseline-source: last');
  });

  it('tells code from verse set out line by line', () => {
    expect(languageLabel('latex')).toBe('LaTeX');
    expect(languageLabel('fortran')).toBe('fortran');
    expect(codeReadsAsWords({ text: LATEX, language: null })).toBe(false);
    expect(codeReadsAsWords({ text: 'print("hi")', language: 'python' })).toBe(false);
    expect(codeReadsAsWords({ text: 'x = {a: [1, 2]};\nreturn x;' })).toBe(false);
    const verse = "'Twas brillig, and the slithy toves\n  Did gyre and gimble in the wabe:";
    expect(codeReadsAsWords({ text: verse })).toBe(true);
    expect(spokenText({ kind: 'code', text: verse })).toBe(verse);
    expect(spokenText({ kind: 'code', text: LATEX, language: 'latex' })).toBeNull();
  });

  it('lets boxes that scroll sideways keep their swipes', () => {
    const box = document.createElement('div');
    box.className = 'book-code';
    const pre = document.createElement('pre');
    const code = document.createElement('code');
    pre.append(code);
    box.append(pre);
    document.body.append(box);
    Object.defineProperty(pre, 'scrollWidth', { configurable: true, value: 600 });
    Object.defineProperty(pre, 'clientWidth', { configurable: true, value: 300 });
    expect(inSideScroller(code)).toBe(true);
    Object.defineProperty(pre, 'scrollWidth', { configurable: true, value: 300 });
    expect(inSideScroller(code)).toBe(false);
    expect(inSideScroller(document.body)).toBe(false);
    box.remove();
  });
});
