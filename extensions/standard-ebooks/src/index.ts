// Standard Ebooks: public-domain books in carefully produced editions; Standard Ebooks dedicates
// its own work on them to the public domain (CC0). Books are found through the catalogue's search
// pages, the XHTML endpoint its OpenSearch description lists (the OPDS and full Atom feeds are
// for its patrons), and read through each book's "Read online" pages: the table of contents,
// then one page per chapter.
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

const SITE = 'https://standardebooks.org';
const PER_PAGE = 48;
const HEADERS = {
  Accept: 'application/xhtml+xml',
  'User-Agent': 'Raygun/1.2 (https://github.com/uvxszi/raygun)',
};
/** Pages of a book's text with nothing to read: title pages, the imprint and the licence. */
const FRAME_PAGES = new Set(['titlepage', 'imprint', 'halftitlepage', 'colophon', 'uncopyright']);
/** `author/title`, with a translator or illustrator part for some editions. */
const BOOK_ID = /^[a-z0-9-]+(\/[a-z0-9_-]+){1,3}$/;
const PAGE_NAME = /^[a-z0-9_.-]+$/;

const SUBJECTS = [
  { value: 'all', label: 'All' },
  { value: 'adventure', label: 'Adventure' },
  { value: 'autobiography', label: 'Autobiography' },
  { value: 'biography', label: 'Biography' },
  { value: 'childrens', label: 'Children’s' },
  { value: 'comedy', label: 'Comedy' },
  { value: 'drama', label: 'Drama' },
  { value: 'fantasy', label: 'Fantasy' },
  { value: 'fiction', label: 'Fiction' },
  { value: 'horror', label: 'Horror' },
  { value: 'memoir', label: 'Memoir' },
  { value: 'mystery', label: 'Mystery' },
  { value: 'nonfiction', label: 'Nonfiction' },
  { value: 'philosophy', label: 'Philosophy' },
  { value: 'poetry', label: 'Poetry' },
  { value: 'satire', label: 'Satire' },
  { value: 'science-fiction', label: 'Science fiction' },
  { value: 'shorts', label: 'Short stories' },
  { value: 'spirituality', label: 'Spirituality' },
  { value: 'travel', label: 'Travel' },
];
const SORTS = [
  { value: 'relevance', label: 'Best match' },
  { value: 'popularity', label: 'Most popular' },
  { value: 'newest', label: 'Newest editions' },
  { value: 'author-alpha', label: 'Author (A–Z)' },
  { value: 'reading-ease', label: 'Easiest to read' },
  { value: 'length', label: 'Shortest first' },
];

const decode = (text: string) =>
  text
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&apos;/g, "'")
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(parseInt(code, 16)),
    )
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&amp;/g, '&');

/** U+2060, which Standard Ebooks sets around dashes. */
const WORD_JOINER = new RegExp(String.fromCharCode(0x2060), 'g');

/** Text without markup or word joiners; inline tags ("Part <span>I</span>: 1805") join up. */
const plainText = (html: string) =>
  decode(html.replace(/<br\s*\/?>|<\/(?:p|div|li|h\d)>/gi, ' ').replace(/<[^>]+>/g, ''))
    .replace(WORD_JOINER, '')
    .replace(/\s+/g, ' ')
    .trim();

/** Paragraphs of HTML as plain text, separated by blank lines. */
const paragraphs = (html: string) =>
  [...html.matchAll(/<p[^>]*>([\s\S]*?)<\/p>/g)]
    .map((match) => plainText(match[1] ?? ''))
    .filter(Boolean)
    .join('\n\n');

const absolute = (path: string) => (path.startsWith('http') ? path : `${SITE}${path}`);

/**
 * Standard Ebooks is run by volunteers, and its catalogue feeds are for members, so this reads
 * the public pages gently: one request at a time, at least 1.5 s apart (with some jitter), and a
 * longer wait when the site asks for one (429, 503). Only `/ebooks/…` pages built from book ids
 * are fetched; links found in pages are never followed (the site bans clients that do).
 */
const GAP_MS = 1500;
const JITTER_MS = 700;
const RETRIES = 2;
let lastRequest = 0;
let queue: Promise<unknown> = Promise.resolve();

const pace = async () => {
  const wait = lastRequest + GAP_MS + Math.random() * JITTER_MS - Date.now();
  if (wait > 0) await raygun.time.sleep(wait);
  lastRequest = Date.now();
};

const request = async (path: string) => {
  for (let attempt = 0; ; attempt += 1) {
    await pace();
    const response = await raygun.http.fetch(`${SITE}${path}`, { headers: HEADERS });
    if ((response.status === 429 || response.status === 503) && attempt < RETRIES) {
      const asked = Number(response.headers?.['retry-after']);
      // The host caps one sleep at 10 s.
      await raygun.time.sleep(
        Number.isFinite(asked) && asked > 0 ? Math.min(asked * 1000, 10_000) : 5000 * (attempt + 1),
      );
      continue;
    }
    return response;
  }
};

const fetchPage = (path: string): Promise<{ text: string; url: string } | null> => {
  const next = queue.then(async () => {
    const response = await request(path);
    if (response.status === 404) return null;
    if (response.status === 429)
      throw new Error('Standard Ebooks asked to slow down; try again in a minute');
    if (!response.ok) throw new Error(`Standard Ebooks answered HTTP ${response.status}`);
    return { text: await response.text(), url: response.url };
  });
  queue = next.catch(() => undefined);
  return next;
};

const bookId = (id: string) => {
  if (!BOOK_ID.test(id)) throw new Error('Not a Standard Ebooks book');
  return id;
};

/** The books of a catalogue page: each `schema:Book` item, its title and its cover. */
const books = (xhtml: string): MangaSummary[] =>
  [...xhtml.matchAll(/<li typeof="schema:Book" about="\/ebooks\/([^"]+)">([\s\S]*?)<\/li>/g)]
    .map((match) => {
      const id = decode(match[1] ?? '');
      const item = match[2] ?? '';
      const title = /<span property="schema:name">([\s\S]*?)<\/span>/.exec(item)?.[1];
      const cover = /<img src="([^"]+)"/.exec(item)?.[1];
      return {
        id,
        title: title ? plainText(title) : '',
        coverUrl: cover ? absolute(decode(cover)) : null,
      };
    })
    .filter((book) => BOOK_ID.test(book.id) && book.title !== '');

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

/** A page of the catalogue: words, a subject and an order, as the site's own search takes them. */
const catalogue = async (
  { query = '', subject = null, sort }: { query?: string; subject?: string | null; sort: string },
  page: number,
): Promise<MangaPage> => {
  const params = [
    query && `query=${encodeURIComponent(query)}`,
    subject && subject !== 'all' && `tags%5B%5D=${encodeURIComponent(subject)}`,
    `sort=${encodeURIComponent(sort)}`,
    `per-page=${PER_PAGE}`,
    `page=${Math.max(1, page)}`,
  ].filter(Boolean);
  const found = await fetchPage(`/ebooks?${params.join('&')}`);
  if (!found) return { items: [], hasNextPage: false };
  const items = books(found.text);
  return { items, hasNextPage: items.length > 0 && /rel="next"/.test(found.text) };
};

type TocEntry = { file: string; label: string; depth: number };

/** The links of a book's table of contents, in reading order, with how deeply each is nested. */
const tocEntries = (xhtml: string): TocEntry[] => {
  const nav = /<nav[^>]*id="toc"[^>]*>([\s\S]*?)<\/nav>/.exec(xhtml)?.[1] ?? '';
  const entries: TocEntry[] = [];
  let depth = 0;
  for (const match of nav.matchAll(
    /<(\/?)ol\b[^>]*>|<a href="text\/([^"#]+)[^"]*"[^>]*>([\s\S]*?)<\/a>/g,
  )) {
    if (match[2] === undefined) {
      depth += match[1] ? -1 : 1;
      continue;
    }
    entries.push({ file: match[2], label: plainText(match[3] ?? ''), depth });
  }
  return entries;
};

/**
 * Chapters from the table of contents: every page to read, named with the parts it sits in
 * ("Book I · Part I: 1805 · I"). Pages that only head the entries under them (a part's title
 * page) and the frame of the book (title pages, imprint, colophon, licence) aren't chapters.
 */
const chapterList = (id: string, entries: TocEntry[]): MangaChapter[] => {
  const chapters: MangaChapter[] = [];
  const open: TocEntry[] = [];
  const seen = new Set<string>();
  entries.forEach((entry, index) => {
    while ((open[open.length - 1]?.depth ?? -1) >= entry.depth) open.pop();
    const within = open
      .filter((parent) => !FRAME_PAGES.has(parent.file))
      .map((parent) => parent.label);
    open.push(entry);
    const heads = (entries[index + 1]?.depth ?? 0) > entry.depth;
    if (
      FRAME_PAGES.has(entry.file) ||
      heads ||
      seen.has(entry.file) ||
      !PAGE_NAME.test(entry.file)
    ) {
      return;
    }
    seen.add(entry.file);
    chapters.push({
      id: `${id}/text/${entry.file}`,
      name: [...within, entry.label || entry.file].join(' · '),
      number: chapters.length + 1,
    });
  });
  return chapters;
};

const details = async (id: string): Promise<MangaDetails> => {
  const found = await fetchPage(`/ebooks/${bookId(id)}`);
  if (!found) throw new Error('This book is no longer in the Standard Ebooks catalogue');
  const page = found.text;
  const title = /<h1 property="schema:name">([\s\S]*?)<\/h1>/.exec(page)?.[1];
  if (!title) throw new Error('Standard Ebooks sent a page without a book');
  const heading = /<hgroup>([\s\S]*?)<\/hgroup>/.exec(page)?.[1] ?? '';
  const authors = [
    ...heading.matchAll(
      /property="schema:author"[^>]*>\s*<span property="schema:name">([\s\S]*?)<\/span>/g,
    ),
  ].map((match) => plainText(match[1] ?? ''));
  const facts = /<aside id="reading-ease">([\s\S]*?)<\/aside>/.exec(page)?.[1] ?? '';
  const translators = /<p>(Translated by [\s\S]*?)<\/p>/.exec(facts)?.[1];
  const genres = [...facts.matchAll(/<a href="\/subjects\/[^"]+">([\s\S]*?)<\/a>/g)].map((match) =>
    plainText(match[1] ?? ''),
  );
  const about = /<div property="schema:description">([\s\S]*?)<\/div>/.exec(page)?.[1];
  const summary = /<meta property="schema:abstract" content="([^"]*)"/.exec(page)?.[1];
  const description = [
    about ? paragraphs(about) : summary ? decode(summary) : '',
    translators && plainText(translators),
  ]
    .filter(Boolean)
    .join('\n\n');
  const cover = /<meta property="schema:thumbnailUrl" content="([^"]+)"/.exec(page)?.[1];
  return {
    id,
    title: plainText(title),
    coverUrl: cover ? absolute(decode(cover)) : null,
    authors,
    description: description || null,
    genres,
    status: 'completed',
    url: `${SITE}/ebooks/${id}`,
  };
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'subject', label: 'Subject', options: SUBJECTS },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'books',
    name: 'Standard Ebooks',
    popular: ({ page }) => catalogue({ sort: 'popularity' }, page),
    latest: ({ page }) => catalogue({ sort: 'newest' }, page),
    search: ({ query, page, filters: values }) => {
      const words = query.trim();
      return catalogue(
        {
          query: words,
          subject: pick(values, 'subject', SUBJECTS),
          sort: pick(values, 'sort', SORTS) ?? (words ? 'relevance' : 'popularity'),
        },
        page,
      );
    },
    filters: async () => filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => {
      const found = await fetchPage(`/ebooks/${bookId(id)}/text`);
      if (!found) throw new Error('This book is no longer in the Standard Ebooks catalogue');
      const chapters = chapterList(id, tocEntries(found.text));
      if (chapters.length === 0) throw new Error('Standard Ebooks lists no chapters for this book');
      return { chapters };
    },
    passage: async ({ chapterId }): Promise<Passage> => {
      const at = chapterId.lastIndexOf('/text/');
      const book = chapterId.slice(0, Math.max(at, 0));
      const name = chapterId.slice(at + '/text/'.length);
      if (at < 0 || !PAGE_NAME.test(name)) throw new Error('Not a Standard Ebooks chapter');
      const found = await fetchPage(`/ebooks/${bookId(book)}/text/${name}`);
      if (!found) throw new Error('This chapter is no longer in the book');
      const main = /<main\b[^>]*>([\s\S]*?)<\/main>/.exec(found.text)?.[1];
      if (!main) throw new Error('Standard Ebooks sent a chapter page without its text');
      // Note markers would run into the words; the endnotes are a chapter of their own.
      const html = main.replace(/<a\b[^>]*epub:type="noteref"[^>]*>[\s\S]*?<\/a>/g, '');
      return { html, baseUrl: found.url };
    },
  }),
);
