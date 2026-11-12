/** Languages' own spellings, by the names highlighters give them (Rust's `language_name`). */
const LANGUAGES: Record<string, string> = {
  bibtex: 'BibTeX',
  c: 'C',
  cpp: 'C++',
  csharp: 'C#',
  css: 'CSS',
  go: 'Go',
  haskell: 'Haskell',
  html: 'HTML',
  java: 'Java',
  javascript: 'JavaScript',
  json: 'JSON',
  kotlin: 'Kotlin',
  latex: 'LaTeX',
  lisp: 'Lisp',
  lua: 'Lua',
  markdown: 'Markdown',
  matlab: 'MATLAB',
  perl: 'Perl',
  php: 'PHP',
  python: 'Python',
  r: 'R',
  ruby: 'Ruby',
  rust: 'Rust',
  scheme: 'Scheme',
  shell: 'Shell',
  sql: 'SQL',
  swift: 'Swift',
  typescript: 'TypeScript',
  xml: 'XML',
  yaml: 'YAML',
};

/** A code block's language as people write it: `latex` → LaTeX; others as given. */
export const languageLabel = (language: string) => LANGUAGES[language] ?? language;

/** Signs code is full of and words hardly use. */
const CODE_SIGNS = /[{}[\]<>;=\\|$#_~^`]/g;

/**
 * Whether a code block reads as words (verse or a letter set out line by line), which reading
 * aloud says; code (a block in a named language, or full of code's signs) is passed over.
 */
export const codeReadsAsWords = (block: { text: string; language?: string | null }) => {
  if (block.language) return false;
  const shown = block.text.replace(/\s+/g, '');
  if (shown === '') return false;
  const signs = shown.match(CODE_SIGNS)?.length ?? 0;
  const letters = shown.match(/\p{L}/gu)?.length ?? 0;
  return signs * 100 <= shown.length && letters >= shown.length * 0.7;
};

/** The boxes in the text that scroll sideways when wider than the column. */
const SIDE_SCROLLERS = '.book-code pre, .math-formula';

/**
 * Whether a touch starts in a box that scrolls sideways (code, a wide formula): its swipes scroll
 * it, they don't turn the page.
 */
export const inSideScroller = (target: EventTarget | null) => {
  const box = target instanceof Element ? target.closest(SIDE_SCROLLERS) : null;
  return box instanceof HTMLElement && box.scrollWidth > box.clientWidth;
};
