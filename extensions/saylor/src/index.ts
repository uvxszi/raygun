// Saylor Academy Open Textbooks: the open textbooks Saylor Academy adapted and publishes on
// saylordotorg.github.io, one site per book. The books are listed in `books.ts` and searched here.
// A book's details read its licensing page (s00-license.html), and only a book whose page states a
// Creative Commons licence is served. Its chapters are its table of contents' top level: each
// chapter's page holds the whole chapter. In the text, glossary definitions set in the margin are
// left out (the text defines its terms too), and footnotes become notes at the chapter's end.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type {
  FilterValues,
  MangaChapter,
  MangaDetails,
  MangaPage,
  MangaSummary,
  Passage,
  SourceFilter,
} from '@raygun/sdk';
import { BOOKS } from './books';
import type { Book, Subject } from './books';

const SITE = 'https://saylordotorg.github.io';
const HEADERS = { 'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )' };
/** The books' pages: `s05-the-foundations-of-business.html`, `s05-02-getting-down-to-business.html`. */
const PAGE = /^s\d+[\w.-]*\.html$/;
const LICENCE_PAGE = 's00-license.html';
const PAGE_SIZE = 30;

const SUBJECT_NAMES: Record<Subject, string> = {
  business: 'Business and management',
  economics: 'Economics and finance',
  law: 'Law',
  social: 'Psychology and sociology',
  science: 'Science and mathematics',
  writing: 'Writing and communication',
  history: 'History, government and geography',
  arts: 'Arts',
};
const SUBJECTS = [
  { value: 'all', label: 'All' },
  ...Object.entries(SUBJECT_NAMES).map(([value, label]) => ({ value, label })),
];
const SORTS = [
  { value: 'title', label: 'Title (A–Z)' },
  { value: 'relevance', label: 'Best match' },
];

const decode = (text: string) =>
  text
    .replace(/&nbsp;|&#160;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;|&apos;/g, "'")
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(parseInt(code, 16)),
    )
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&amp;/g, '&');

const plainText = (html: string) =>
  decode(html.replace(/<[^>]+>/g, ' '))
    .replace(/\s+/g, ' ')
    .trim();

const words = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

const VOID_ELEMENTS = new Set([
  'area',
  'base',
  'br',
  'col',
  'embed',
  'hr',
  'img',
  'input',
  'link',
  'meta',
  'source',
  'track',
  'wbr',
]);
/** A start or end tag; quoted attribute values may hold `>`. */
const TAG = /<(\/?)([a-zA-Z][\w:-]*)((?:[^>"']|"[^"]*"|'[^']*')*)>/g;

/** The HTML without the elements `drop` picks, each with everything inside it. */
const dropElements = (html: string, drop: (tag: string, attributes: string) => boolean) => {
  let kept = '';
  let from = 0;
  let open: { tag: string; depth: number } | null = null;
  for (const match of html.matchAll(TAG)) {
    const [whole, closing, name = '', attributes = ''] = match;
    const tag = name.toLowerCase();
    const at = match.index ?? 0;
    const lone = VOID_ELEMENTS.has(tag) || attributes.trimEnd().endsWith('/');
    if (open) {
      if (tag === open.tag && !lone) open.depth += closing ? -1 : 1;
      if (open.depth === 0) {
        open = null;
        from = at + whole.length;
      }
      continue;
    }
    if (closing || !drop(tag, attributes)) continue;
    kept += html.slice(from, at);
    from = at + whole.length;
    if (!lone) open = { tag, depth: 1 };
  }
  return open ? kept : kept + html.slice(from);
};

/** The element whose start tag is the first match of `start`, with everything inside it. */
const element = (html: string, start: RegExp) => {
  const at = html.search(start);
  if (at < 0) return null;
  let depth = 0;
  let tag = '';
  for (const match of html.slice(at).matchAll(TAG)) {
    const [whole, closing, name = ''] = match;
    const current = name.toLowerCase();
    if (!tag) tag = current;
    if (current !== tag || VOID_ELEMENTS.has(current)) continue;
    depth += closing ? -1 : 1;
    if (depth === 0) return html.slice(at, at + (match.index ?? 0) + whole.length);
  }
  return html.slice(at);
};

const classes = (attributes: string) =>
  /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/i
    .exec(attributes)
    ?.slice(1)
    .join(' ')
    .split(/\s+/)
    .filter(Boolean) ?? [];

const bookOf = (id: string): Book => {
  const book = BOOKS.find((known) => known.id === id);
  if (!book) throw new Error('Saylor Academy has no book by this name here');
  return book;
};

const siteOf = (book: Book) => `${SITE}/text_${book.id}/`;

const fetchPage = async (url: string) => {
  let response;
  try {
    response = await raygun.http.fetch(url, { headers: HEADERS });
  } catch {
    throw new Error("Saylor Academy's book site didn't answer; try again in a minute");
  }
  if (response.status === 404) throw new Error('Saylor Academy no longer has this page');
  if (!response.ok) {
    throw new Error(`Saylor Academy's book site answered with an error (HTTP ${response.status})`);
  }
  return { text: await response.text(), url: response.url || url };
};

/** "CC BY-NC-SA 3.0" from the licensing page's Creative Commons link; null without one. */
const licenceOf = (html: string) => {
  const found = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d\.\d)/i.exec(html);
  return found ? `CC ${(found[1] ?? '').toUpperCase()} ${found[2] ?? ''}` : null;
};

const summary = (book: Book): MangaSummary => ({ id: book.id, title: book.title });

const pageOf = (books: Book[], page: number): MangaPage => {
  const start = (Math.max(1, page) - 1) * PAGE_SIZE;
  return {
    items: books.slice(start, start + PAGE_SIZE).map(summary),
    hasNextPage: start + PAGE_SIZE < books.length,
  };
};

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

const search = (query: string, page: number, values: FilterValues | undefined): MangaPage => {
  const subject = pick(values, 'subject', SUBJECTS);
  const terms = words(query).split(' ').filter(Boolean);
  const found = BOOKS.filter(
    (book) =>
      (!subject || subject === 'all' || book.subject === subject) &&
      terms.every((term) => words(`${book.title} ${SUBJECT_NAMES[book.subject]}`).includes(term)),
  );
  const inTitle = (book: Book) => terms.filter((term) => words(book.title).includes(term)).length;
  const ordered =
    terms.length > 0 && pick(values, 'sort', SORTS) !== 'title'
      ? [...found].sort((first, second) => inTitle(second) - inTitle(first))
      : found;
  return pageOf(ordered, page);
};

const details = async (book: Book): Promise<MangaDetails> => {
  const page = await fetchPage(`${siteOf(book)}${LICENCE_PAGE}`);
  const licence = licenceOf(page.text);
  if (!licence) throw new Error("This book's licensing page doesn't name an open licence");
  const year = /Year Published:\s*(\d{4})/.exec(page.text)?.[1];
  return {
    id: book.id,
    title: book.title,
    coverUrl: null,
    authors: [],
    description: [
      `${SUBJECT_NAMES[book.subject]} textbook, version ${book.edition}, adapted and published by Saylor Academy${year ? ` in ${year}` : ''}. As its original authors asked, it doesn't name them.`,
      `Licence: ${licence}, as the book's licensing page states.`,
    ].join('\n\n'),
    genres: [SUBJECT_NAMES[book.subject]],
    status: 'completed',
    url: siteOf(book),
  };
};

const chapterList = async (book: Book): Promise<MangaChapter[]> => {
  const index = await fetchPage(`${siteOf(book)}index.html`);
  const toc = element(index.text, /<ul id="toc-top-ul"/) ?? '';
  const chapters: MangaChapter[] = [];
  // The top level: a chapter's page holds its sections, which the contents list under it.
  for (const match of toc.matchAll(/<h3><a href="([^"]+)">([\s\S]*?)<\/a><\/h3>/g)) {
    const page = decode(match[1] ?? '');
    if (!PAGE.test(page) || page === LICENCE_PAGE) continue;
    chapters.push({
      id: `${book.id}/${page}`,
      name: plainText(match[2] ?? '') || page,
      number: chapters.length + 1,
    });
  }
  if (chapters.length === 0) throw new Error("This book's table of contents lists no chapters");
  return chapters;
};

/** Footnotes are set inside the sentence; they become numbered notes after the text. */
const withNotes = (html: string) => {
  const notes: string[] = [];
  let text = '';
  let from = 0;
  for (;;) {
    const at = html.slice(from).search(/<span class="footnote"/);
    if (at < 0) break;
    const start = from + at;
    const note = element(html.slice(start), /<span/) ?? '';
    notes.push(note.replace(/^<span[^>]*>|<\/span>$/g, ''));
    text += `${html.slice(from, start)}<sup>[${notes.length}]</sup>`;
    from = start + note.length;
  }
  text += html.slice(from);
  if (notes.length === 0) return text;
  return `${text}<h3>Notes</h3>${notes.map((note, index) => `<p>[${index + 1}] ${note}</p>`).join('')}`;
};

const passage = async (chapterId: string): Promise<Passage> => {
  const slash = chapterId.indexOf('/');
  const book = bookOf(chapterId.slice(0, slash));
  const page = chapterId.slice(slash + 1);
  if (slash < 0 || !PAGE.test(page)) throw new Error('Not a chapter of a Saylor Academy book');
  const found = await fetchPage(`${siteOf(book)}${page}`);
  const content = element(found.text, /<div id="book-content"/) ?? '';
  if (/this content is permanently unavailable/i.test(content)) {
    throw new Error('Saylor Academy no longer has this chapter online');
  }
  const html = withNotes(
    dropElements(
      content,
      (tag, attributes) => tag === 'script' || classes(attributes).includes('glossdef'),
    ),
  );
  if (!plainText(html)) throw new Error('This chapter has no text');
  return { html, baseUrl: found.url };
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'subject', label: 'Subject', options: SUBJECTS },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'textbooks',
    name: 'Saylor Academy',
    popular: async ({ page }) => pageOf(BOOKS, page),
    search: async ({ query, page, filters: values }) => search(query, page, values),
    filters: async () => filters,
    details: ({ id }) => details(bookOf(id)),
    chapters: async ({ id }) => ({ chapters: await chapterList(bookOf(id)) }),
    passage: ({ chapterId }) => passage(chapterId),
  }),
);
