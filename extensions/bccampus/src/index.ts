// BCcampus Open Textbooks: the open textbooks BCcampus publishes on Pressbooks at opentextbc.ca.
// Pressbooks' API lists every book with its licence but can't search or sort, so the catalogue is
// read once (ten books a page, a few pages at a time), kept in memory and searched here. Only books
// BCcampus lists in its catalogue under a Creative Commons licence or in the public domain are
// listed, read from each book's own licence field; a chapter whose own licence isn't an open one
// is left out. A book's chapters are its table of contents (front matter, parts and their
// chapters, back matter) as Pressbooks gives it; each chapter's text comes from the same API.
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

const SITE = 'https://opentextbc.ca';
/** The most books Pressbooks sends in one page of its list. */
const PER_REQUEST = 10;
/** Pages of the list asked for at once. */
const AT_ONCE = 5;
const PAGE_SIZE = 30;
const HEADERS = {
  Accept: 'application/json',
  'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )',
};
const SLUG = /^[a-z0-9][a-z0-9-]*$/;
const KINDS = new Set(['front-matter', 'parts', 'chapters', 'back-matter']);
/** Pages every BCcampus book opens with, about BCcampus and its formats rather than the book. */
const BOILERPLATE =
  /^(?:accessibility statement|about bccampus open education|for students: how to access and use this textbook)$/i;
/** Hosts of pictures that also answer over HTTPS, which old chapters link over HTTP. */
const SECURE_HOSTS = /^http:\/\/((?:[\w-]+\.)*(?:opentextbc\.ca|bccampus\.ca|wikimedia\.org))\//i;

// Thema's top-level subjects, as BCcampus files its books.
const SUBJECTS = [
  { value: 'all', label: 'All' },
  { value: 'A', label: 'Arts' },
  { value: 'C', label: 'Language and linguistics' },
  { value: 'D', label: 'Literature and biography' },
  { value: 'G', label: 'Reference and interdisciplinary' },
  { value: 'J', label: 'Society and social sciences' },
  { value: 'K', label: 'Economics, finance and business' },
  { value: 'L', label: 'Law' },
  { value: 'M', label: 'Medicine and nursing' },
  { value: 'N', label: 'History and archaeology' },
  { value: 'P', label: 'Mathematics and science' },
  { value: 'Q', label: 'Philosophy and religion' },
  { value: 'R', label: 'Earth sciences and geography' },
  { value: 'T', label: 'Technology, engineering and trades' },
  { value: 'U', label: 'Computing' },
  { value: 'V', label: 'Health and personal development' },
  { value: 'W', label: 'Lifestyle, cooking and leisure' },
  { value: 'Y', label: 'Education and learning' },
];
const LICENCES = [
  { value: 'all', label: 'All' },
  { value: 'CC BY', label: 'CC BY' },
  { value: 'CC BY-SA', label: 'CC BY-SA' },
  { value: 'CC BY-NC', label: 'CC BY-NC' },
  { value: 'CC BY-NC-SA', label: 'CC BY-NC-SA' },
  { value: 'Public domain', label: 'Public domain' },
];
const SORTS = [
  { value: 'relevance', label: 'Best match' },
  { value: 'catalogue', label: 'Catalogue order' },
  { value: 'title', label: 'Title (A–Z)' },
  { value: 'newest', label: 'Newest' },
  { value: 'updated', label: 'Recently updated' },
];

type Person = { name?: string };
type Licence = { url?: string; name?: string; code?: string };
type BookMetadata = {
  name?: string;
  author?: Person[];
  editor?: Person[];
  translator?: Person[];
  about?: { identifier?: string; name?: string }[];
  keywords?: string;
  disambiguatingDescription?: string;
  thumbnailUrl?: string;
  image?: string;
  datePublished?: string;
  lastUpdated?: number | string;
  license?: Licence;
  inCatalog?: boolean;
  publisher?: { name?: string };
  copyrightYear?: string;
};
type BookEntry = { link?: string; metadata?: BookMetadata };
type Book = {
  id: string;
  title: string;
  authors: string[];
  cover: string | null;
  /** "CC BY 4.0", "Public domain"… */
  licence: string;
  /** Thema's top-level subject letters. */
  subjects: Set<string>;
  /** Lower-case words to search: title, people, subjects and keywords, the summary. */
  text: { title: string; people: string; topics: string; summary: string };
  published: number;
  updated: number;
  order: number;
};
type TocEntry = {
  id?: number;
  title?: string;
  has_post_content?: boolean;
  metadata?: { license?: Licence };
};
type Toc = {
  'front-matter'?: TocEntry[];
  parts?: (TocEntry & { chapters?: TocEntry[] })[];
  'back-matter'?: TocEntry[];
};
type Post = { link?: string; content?: { rendered?: string } };

/** Text, or none for a metadata field that isn't (Pressbooks sends `false` for some). */
const textOf = (value: unknown) => (typeof value === 'string' ? value : '');

const decode = (text: unknown) =>
  textOf(text)
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

const plainText = (html: unknown) =>
  decode(textOf(html).replace(/<[^>]+>/g, ' '))
    .replace(/\s+/g, ' ')
    .trim();

/** Lower-case words without punctuation, for matching. */
const words = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

const getJson = async <T>(path: string): Promise<{ value: T; headers: Record<string, string> }> => {
  let response;
  try {
    response = await raygun.http.fetch(`${SITE}${path}`, { headers: HEADERS });
  } catch {
    throw new Error("BCcampus didn't answer; try again in a minute");
  }
  if (response.status === 404) throw new Error('BCcampus no longer has this book or chapter');
  if (!response.ok) throw new Error(`BCcampus answered with an error (HTTP ${response.status})`);
  return { value: await response.json<T>(), headers: response.headers };
};

/**
 * "CC BY-NC-SA 4.0", "CC0 1.0" or "Public domain" for an open licence, from its address (else its
 * code); null for any other, such as All Rights Reserved.
 */
const openLicence = (licence: Licence | undefined): string | null => {
  const url = licence?.url ?? '';
  const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d+(?:\.\d+)?)/i.exec(url);
  if (creativeCommons) {
    return `CC ${(creativeCommons[1] ?? '').toUpperCase()} ${creativeCommons[2] ?? ''}`;
  }
  if (/creativecommons\.org\/publicdomain\/zero\//i.test(url)) return 'CC0 1.0';
  if (/creativecommons\.org\/publicdomain\/mark\//i.test(url)) return 'Public domain';
  const named = `${licence?.code ?? ''} ${licence?.name ?? ''}`;
  if (/public domain/i.test(named)) return 'Public domain';
  if (/\bCC0\b/i.test(named)) return 'CC0';
  const code = /\bCC (BY(?:-NC)?(?:-SA|-ND)?)\b/i.exec(named);
  return code ? `CC ${(code[1] ?? '').toUpperCase()}` : null;
};

/** The licence's family, as the licence filter names it: "CC BY-SA", "Public domain". */
const licenceFamily = (licence: string) => licence.replace(/ \d.*$/, '');

const names = (people: Person[] | undefined) =>
  (people ?? []).map((person) => plainText(person.name)).filter(Boolean);

const secure = (url: string) => url.replace(SECURE_HOSTS, 'https://$1/');

const toBook = (entry: BookEntry, order: number): Book | null => {
  const meta = entry.metadata;
  const id = /^https?:\/\/opentextbc\.ca\/([a-z0-9-]+)\/?$/.exec(entry.link ?? '')?.[1];
  if (!meta || !id || meta.inCatalog !== true || !meta.name) return null;
  const licence = openLicence(meta.license);
  if (!licence) return null;
  const people = names(meta.author);
  const authors = people.length > 0 ? people : names(meta.editor);
  const about = meta.about ?? [];
  const subjects = new Set(
    about
      .filter(
        (subject) => decode(subject.name) && /^[A-Z][A-Z0-9]*$/.test(decode(subject.identifier)),
      )
      .map((subject) => decode(subject.identifier).charAt(0)),
  );
  const topics = about.map((subject) => decode(subject.name) || decode(subject.identifier));
  const cover = textOf(meta.thumbnailUrl) || textOf(meta.image);
  const updated = Number(meta.lastUpdated) * 1000;
  const published = Date.parse(textOf(meta.datePublished));
  return {
    id,
    title: plainText(meta.name),
    authors,
    cover: cover ? secure(cover) : null,
    licence,
    subjects,
    text: {
      title: words(decode(meta.name)),
      people: words([...authors, ...names(meta.translator)].join(' ')),
      topics: words([...topics, decode(meta.keywords)].join(' ')),
      summary: words(decode(meta.disambiguatingDescription)),
    },
    published: Number.isFinite(published) ? published : Number.isFinite(updated) ? updated : 0,
    updated: Number.isFinite(updated) ? updated : 0,
    order,
  };
};

/** Runs `task` over `items`, `width` at a time, keeping their order. */
const inTurns = async <T, R>(items: T[], width: number, task: (item: T) => Promise<R>) => {
  const results: R[] = [];
  for (let start = 0; start < items.length; start += width) {
    results.push(...(await Promise.all(items.slice(start, start + width).map(task))));
  }
  return results;
};

const listPage = (page: number) =>
  getJson<BookEntry[]>(
    `/wp-json/pressbooks/v2/books?per_page=${PER_REQUEST}&page=${page}&_fields=link,metadata`,
  );

// The catalogue stays in memory for half a day: BCcampus adds a book every few weeks.
let catalogueAt = 0;
let catalogueBooks: Promise<Book[]> | null = null;
const catalogue = (): Promise<Book[]> => {
  if (catalogueBooks && Date.now() - catalogueAt < 12 * 3_600_000) return catalogueBooks;
  catalogueAt = Date.now();
  const loading = (async () => {
    const first = await listPage(1);
    const pages = Math.min(Number(first.headers['x-wp-totalpages']) || 1, 100);
    const rest = await inTurns(
      Array.from({ length: pages - 1 }, (_value, index) => index + 2),
      AT_ONCE,
      async (page) => (await listPage(page)).value,
    );
    const entries = [first.value, ...rest].flat();
    const seen = new Set<string>();
    return entries.flatMap((entry, order) => {
      const book = toBook(entry, order);
      if (!book || seen.has(book.id)) return [];
      seen.add(book.id);
      return [book];
    });
  })();
  loading.catch(() => {
    if (catalogueBooks === loading) catalogueBooks = null;
  });
  catalogueBooks = loading;
  return loading;
};

const summary = (book: Book): MangaSummary => ({
  id: book.id,
  title: book.title,
  coverUrl: book.cover,
  authors: book.authors.slice(0, 6),
});

const pageOf = (books: Book[], page: number): MangaPage => {
  const start = (Math.max(1, page) - 1) * PAGE_SIZE;
  return {
    items: books.slice(start, start + PAGE_SIZE).map(summary),
    hasNextPage: start + PAGE_SIZE < books.length,
  };
};

type Order = (first: Book, second: Book) => number;
const byCatalogue: Order = (first, second) => first.order - second.order;
const byNewest: Order = (first, second) =>
  second.published - first.published || byCatalogue(first, second);
const ORDERS = new Map<string, Order>([
  ['catalogue', byCatalogue],
  ['title', (first, second) => first.title.localeCompare(second.title)],
  ['newest', byNewest],
  ['updated', (first, second) => second.updated - first.updated || byCatalogue(first, second)],
]);

/** How well a book matches every one of the words (0 when one is missing). */
const score = (book: Book, terms: string[], phrase: string) => {
  let total = book.text.title.includes(phrase) ? 4 : 0;
  for (const word of terms) {
    const found =
      (book.text.title.includes(word) ? 3 : 0) +
      (book.text.people.includes(word) ? 2 : 0) +
      (book.text.topics.includes(word) ? 1.5 : 0) +
      (book.text.summary.includes(word) ? 1 : 0);
    if (found === 0) return 0;
    total += found;
  }
  return total;
};

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

const search = async (query: string, page: number, values: FilterValues | undefined) => {
  const subject = pick(values, 'subject', SUBJECTS);
  const licence = pick(values, 'licence', LICENCES);
  const terms = words(query).split(' ').filter(Boolean);
  const phrase = terms.join(' ');
  const sort = pick(values, 'sort', SORTS) ?? (phrase ? 'relevance' : 'catalogue');
  const order = ORDERS.get(sort);
  const books = (await catalogue()).filter(
    (book) =>
      (!subject || subject === 'all' || book.subjects.has(subject)) &&
      (!licence || licence === 'all' || licenceFamily(book.licence) === licence),
  );
  if (!phrase) return pageOf([...books].sort(order ?? byCatalogue), page);
  const found = books
    .map((book) => ({ book, score: score(book, terms, phrase) }))
    .filter((match) => match.score > 0)
    .sort((first, second) =>
      order
        ? order(first.book, second.book)
        : second.score - first.score || byCatalogue(first.book, second.book),
    );
  return pageOf(
    found.map((match) => match.book),
    page,
  );
};

const bookId = (id: string) => {
  if (!SLUG.test(id)) throw new Error('Not a BCcampus book');
  return id;
};

const details = async (id: string): Promise<MangaDetails> => {
  const { value: meta } = await getJson<BookMetadata>(
    `/${bookId(id)}/wp-json/pressbooks/v2/metadata`,
  );
  if (!meta.name) throw new Error('BCcampus sent a book without a title');
  const licence = openLicence(meta.license);
  if (!licence) throw new Error('This book is not under an open licence');
  const people = names(meta.author);
  const translators = names(meta.translator);
  const year = textOf(meta.datePublished).slice(0, 4) || textOf(meta.copyrightYear);
  const publisher = plainText(meta.publisher?.name);
  const genres = (meta.about ?? []).map((subject) => decode(subject.name)).filter(Boolean);
  const cover = textOf(meta.thumbnailUrl) || textOf(meta.image);
  return {
    id,
    title: plainText(meta.name),
    coverUrl: cover ? secure(cover) : null,
    authors: people.length > 0 ? people : names(meta.editor),
    description: [
      plainText(meta.disambiguatingDescription),
      [
        translators.length > 0 && `Translated by ${translators.join(', ')}.`,
        publisher && `Published by ${publisher}${year ? ` in ${year}` : ''}.`,
      ]
        .filter(Boolean)
        .join(' '),
      `Licence: ${licence}, as stated for this book on opentextbc.ca.`,
    ]
      .filter(Boolean)
      .join('\n\n'),
    genres: [...new Set(genres)].slice(0, 8),
    status: 'completed',
    url: `${SITE}/${id}/`,
  };
};

const chapterList = async (id: string): Promise<MangaChapter[]> => {
  const { value: toc } = await getJson<Toc>(`/${bookId(id)}/wp-json/pressbooks/v2/toc`);
  const parts = toc.parts ?? [];
  const chapters: MangaChapter[] = [];
  const add = (kind: string, entry: TocEntry, within?: string) => {
    const title = plainText(entry.title ?? '');
    if (!entry.id || !entry.has_post_content || BOILERPLATE.test(title)) return;
    // A chapter's own licence, when it names one, must be open too.
    if (entry.metadata?.license && !openLicence(entry.metadata.license)) return;
    chapters.push({
      id: `${id}/${kind}/${entry.id}`,
      name: within && within !== title ? `${within} · ${title}` : title || `${kind} ${entry.id}`,
      number: chapters.length + 1,
    });
  };
  for (const entry of toc['front-matter'] ?? []) add('front-matter', entry);
  for (const part of parts) {
    const partTitle = plainText(part.title ?? '');
    add('parts', part);
    // One part ("Main Body") names nothing; several name each chapter's place.
    for (const chapter of part.chapters ?? []) {
      add('chapters', chapter, parts.length > 1 ? partTitle : undefined);
    }
  }
  for (const entry of toc['back-matter'] ?? []) add('back-matter', entry);
  if (chapters.length === 0) throw new Error('BCcampus lists no chapters for this book');
  return chapters;
};

/** A `[latex]` shortcode's LaTeX, which holds no other shortcode. */
const SHORTCODE = String.raw`\[latex\]((?:(?!\[\/?latex\])[\s\S])*?)\[\/latex\]`;
/** WP-LaTeX's own spelling, `$latex x^2$`. */
const DOLLAR = String.raw`\$latex\s([^$]*?)\$`;

/**
 * Pressbooks' LaTeX shortcodes (its site sets them with MathJax) as LaTeX in the usual delimiters,
 * `\[…\]` for a formula alone in its paragraph and `\(…\)` within text; the LaTeX is unchanged.
 */
const latex = (html: string) =>
  html
    .replace(
      new RegExp(String.raw`<p\b([^>]*)>\s*(?:${SHORTCODE}|${DOLLAR})\s*<\/p>`, 'g'),
      (_match, attributes: string, shortcode?: string, dollar?: string) =>
        `<p${attributes}>\\[${shortcode ?? dollar ?? ''}\\]</p>`,
    )
    .replace(new RegExp(SHORTCODE, 'g'), '\\($1\\)')
    .replace(new RegExp(DOLLAR, 'g'), '\\($1\\)');

/** The interactive H5P activity in a wrapper, which only works on the website, as a note. */
const activityNote = (wrapper: string) => {
  const title = /<iframe\b[^>]*\btitle="([^"]*)"/.exec(wrapper)?.[1];
  const what = title
    ? `“${plainText(title)}” is an interactive activity`
    : 'An interactive activity';
  return `<p><em>${what} in the online version of this chapter.</em></p>`;
};

const cleanChapter = (html: string) =>
  latex(html)
    .replace(/<div class="h5p-iframe-wrapper">[\s\S]*?<\/iframe>\s*<\/div>/g, activityNote)
    .replace(/\[h5p\b[^\]]*\]/g, '')
    // Glossary terms are buttons that open their definition; the word stays.
    .replace(
      /<button\b[^>]*\bclass="[^"]*glossary-term[^"]*"[^>]*>([\s\S]*?)<\/button>/g,
      '<span>$1</span>',
    )
    .replace(
      /(<img\b[^>]*?\bsrc=")([^"]+)"/g,
      (_match, start: string, src: string) => `${start}${secure(src)}"`,
    );

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'textbooks',
    name: 'BCcampus Open Textbooks',
    popular: async ({ page }) => pageOf(await catalogue(), page),
    latest: async ({ page }) => pageOf([...(await catalogue())].sort(byNewest), page),
    search: ({ query, page, filters: values }) => search(query, page, values),
    filters: async (): Promise<SourceFilter[]> => [
      { kind: 'select', key: 'subject', label: 'Subject', options: SUBJECTS },
      { kind: 'select', key: 'licence', label: 'Licence', options: LICENCES },
      { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
    ],
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
    passage: async ({ chapterId }): Promise<Passage> => {
      const [book = '', kind = '', post = ''] = chapterId.split('/');
      if (!SLUG.test(book) || !KINDS.has(kind) || !/^\d+$/.test(post)) {
        throw new Error('Not a BCcampus chapter');
      }
      const { value } = await getJson<Post>(
        `/${book}/wp-json/pressbooks/v2/${kind}/${post}?_fields=link,content`,
      );
      const html = value.content?.rendered ?? '';
      if (!plainText(html) && !/<img\b/.test(html)) {
        throw new Error('This chapter has no text');
      }
      return { html: cleanChapter(html), baseUrl: value.link ?? `${SITE}/${book}/` };
    },
  }),
);
