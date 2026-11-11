// OpenStax (openstax.org): free, peer-reviewed college textbooks from OpenStax at Rice University,
// in English and Spanish, each under the Creative Commons licence its own data names (CC BY 4.0
// or CC BY-NC-SA 4.0); a book's details say which.
//
// - The catalogue is OpenStax's CMS: its live books, kept for hours. Popular lists them by
//   subject, then title; latest by publication date. Search looks through titles, authors and
//   subjects on the device, with subject, language and sort filters.
// - A book's contents and pages come from OpenStax's archive, at the version its reader serves
//   now (`rex/release.json`, kept for hours; books it retired are left out of the lists).
//   Chapters are the contents' pages in reading order, named with the chapter they sit in, the
//   index left out. Their ids hold only the book's and the page's ids (`<book>:<page>`), so they
//   stay the same when a book gets a new version: the current one is looked up when a page is
//   read.
// - A page keeps its figures (the archive's resources) and its formulas (MathML, passed on as it
//   is, for the reader to draw).
import { defineProvider, registerProvider, selectValue } from '@raygun/sdk';
import type {
  FilterOption,
  FilterValues,
  MangaChapter,
  MangaDetails,
  MangaPage,
  MangaSummary,
  Passage,
  SelectFilter,
  SourceFilter,
} from '@raygun/sdk';

const SITE = 'https://openstax.org';
/** What the catalogue needs of each book. */
const FIELDS = [
  'title',
  'cnx_id',
  'book_state',
  'license_name',
  'license_url',
  'license_version',
  'cover_url',
  'book_subjects',
  'book_categories',
  'publish_date',
  'description',
  'authors',
  'content_warning_text',
];
/** The most books the CMS gives in one answer. */
const CATALOGUE_PAGE = 250;
const RELEASE = `${SITE}/rex/release.json`;
const HEADERS = {
  'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )',
  Accept: 'application/json',
};
const PAGE_SIZE = 30;
/** How long the catalogue and the list of versions are kept. */
const KEEP_HOURS = 6;
const UUID = /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/;
const CHAPTER_ID = /^([0-9a-f-]{36}):([0-9a-f-]{36})$/;
/** A section's own number ("1.1", "12.4"), which names it without its chapter. */
const SECTION_NUMBER = /^\d+(?:\.\d+)+$/;

/** Subjects in the order OpenStax's subject pages give them. */
const SUBJECTS = [
  'Math',
  'Science',
  'Social Sciences',
  'Humanities',
  'Business',
  'Computer Science',
  'Nursing',
  'College Success',
];
/** The Spanish books' subjects, by the names the filter and the order use. */
const SUBJECT_NAMES: Record<string, string> = {
  Matemáticas: 'Math',
  Ciencia: 'Science',
  Empresarial: 'Business',
};
const LANGUAGES: Record<string, string> = {
  en: 'English',
  es: 'Spanish',
  pl: 'Polish',
  fr: 'French',
  de: 'German',
};

type CmsAuthor = {
  value?: {
    name?: string | null;
    senior_author?: boolean | null;
    display_at_top?: boolean | null;
  } | null;
};
type CmsBook = {
  title?: string | null;
  meta?: { slug?: string | null; locale?: string | null } | null;
  cnx_id?: string | null;
  book_state?: string | null;
  license_name?: string | null;
  license_url?: string | null;
  license_version?: string | null;
  cover_url?: string | null;
  book_subjects?: { subject_name?: string | null }[] | null;
  book_categories?: { subject_category?: string | null }[] | null;
  publish_date?: string | null;
  description?: string | null;
  authors?: CmsAuthor[] | null;
  content_warning_text?: string | null;
};
type CmsAnswer = { meta?: { total_count?: number | null } | null; items?: CmsBook[] | null };
type ReleaseAnswer = {
  archiveUrl?: string | null;
  books?: Record<string, { defaultVersion?: string | null; retired?: boolean | null } | null>;
};
type TocNode = {
  id?: string | null;
  title?: string | null;
  slug?: string | null;
  toc_target_type?: string | null;
  contents?: TocNode[] | null;
};
type Contents = {
  license?: { url?: string | null; name?: string | null } | null;
  tree?: TocNode | null;
};

type Licence = { name: string; url: string | null };

type Book = {
  /** The archive's id for the book (`cnx_id`). */
  id: string;
  title: string;
  slug: string;
  language: string;
  /** Who a list names: the authors OpenStax shows first, else its senior authors. */
  lead: string[];
  senior: string[];
  contributors: string[];
  /** As OpenStax names them. */
  subjects: string[];
  /** The subjects by the names the filter and the order use. */
  topics: string[];
  categories: string[];
  about: string;
  warning: string;
  coverUrl: string | null;
  /** `YYYY-MM-DD`, or empty. */
  published: string;
  /** As the catalogue states it; the book's own data wins when it's read. */
  licence: Licence | null;
  /** The title's words, and the words search looks through, each with a space before it. */
  titleWords: string;
  words: string;
};

type Catalogue = { books: Book[]; byId: Map<string, Book> };
type Versions = { archive: string; versions: Map<string, string> };

// ---- Text ----

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

/** One line of text from HTML: no markup, spaces collapsed. */
const inline = (html: string) =>
  decode(html.replace(/<[^>]+>/g, ''))
    .replace(/\s+/g, ' ')
    .trim();

/** Text from HTML with its paragraphs kept apart by blank lines. */
const plainText = (html: string) =>
  decode(
    html
      .replace(/<(script|style)\b[\s\S]*?<\/\1>/gi, '')
      .replace(/<\/(?:p|div|li|ul|ol|h\d|blockquote)>|<br\s*\/?>/gi, '\n\n')
      .replace(/<[^>]+>/g, ''),
  )
    .split(/\n\s*\n/)
    .map((paragraph) => paragraph.replace(/\s+/g, ' ').trim())
    .filter(Boolean)
    .join('\n\n');

/** Lower-case words without accents or punctuation, as search compares them. */
const words = (text: string) =>
  text
    .normalize('NFKD')
    .replace(/[̀-ͯ]/g, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

const unique = (names: string[]) => [...new Set(names.filter(Boolean))];

/** "A, B and 3 more": at most `most` names. */
const nameList = (names: string[], most: number) =>
  names.length <= most
    ? names.join(', ')
    : `${names.slice(0, most).join(', ')} and ${names.length - most} more`;

const isWeb = (url: string | null | undefined): url is string =>
  typeof url === 'string' && /^https?:\/\/\S+$/i.test(url.trim());

// ---- Licences ----

/** "CC BY-NC-SA 4.0" from a Creative Commons address, else the name the data gives. */
const licenceOf = (
  url: string | null | undefined,
  name: string | null | undefined,
  version?: string | null,
): Licence | null => {
  const address = isWeb(url) ? url.trim() : null;
  const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d+(?:\.\d+)?)/i.exec(
    address ?? '',
  );
  if (creativeCommons) {
    const code = (creativeCommons[1] ?? '').toUpperCase();
    return { name: `CC ${code} ${creativeCommons[2] ?? ''}`.trim(), url: address };
  }
  const named = [name?.trim(), version?.trim()].filter(Boolean).join(' ');
  return named ? { name: named, url: address } : null;
};

// ---- Requests ----

const getJson = async <T>(url: string, missing: string): Promise<T> => {
  const response = await raygun.http.fetch(url, { headers: HEADERS });
  if (response.status === 404) throw new Error(missing);
  if (!response.ok) throw new Error(`OpenStax answered with an error (HTTP ${response.status})`);
  try {
    return await response.json<T>();
  } catch {
    throw new Error("OpenStax's answer couldn't be read");
  }
};

// What's read stays in memory for a while: the catalogue and the versions change a few times a
// month. A failed read is asked again next time.
const kept = new Map<string, { at: number; value: Promise<unknown> }>();
const remembered = <T>(key: string, hours: number, load: () => Promise<T>): Promise<T> => {
  const known = kept.get(key);
  if (known && Date.now() - known.at < hours * 3_600_000) return known.value as Promise<T>;
  const value = load();
  kept.set(key, { at: Date.now(), value });
  value.catch(() => kept.delete(key));
  return value;
};

// ---- The catalogue ----

const people = (book: CmsBook) => {
  const listed = (book.authors ?? []).flatMap((author) => {
    const name = author?.value?.name?.replace(/\s+/g, ' ').trim();
    return name
      ? [
          {
            name,
            senior: Boolean(author.value?.senior_author),
            top: Boolean(author.value?.display_at_top),
          },
        ]
      : [];
  });
  const top = unique(listed.filter((author) => author.top).map((author) => author.name));
  const senior = unique([
    ...top,
    ...listed.filter((author) => author.senior).map((author) => author.name),
  ]);
  const contributors = unique(listed.map((author) => author.name)).filter(
    (name) => !senior.includes(name),
  );
  const lead = top.length > 0 ? top : senior.length > 0 ? senior : contributors.slice(0, 3);
  return { lead, senior, contributors };
};

const toBook = (item: CmsBook): Book[] => {
  const id = item.cnx_id?.trim().toLowerCase() ?? '';
  const title = inline(item.title ?? '');
  const slug = item.meta?.slug?.trim() ?? '';
  if (item.book_state !== 'live' || !UUID.test(id) || !title || !slug) return [];
  const subjects = unique(
    (item.book_subjects ?? []).map((subject) => inline(subject?.subject_name ?? '')),
  );
  const topics = unique(subjects.map((subject) => SUBJECT_NAMES[subject] ?? subject));
  const categories = unique(
    (item.book_categories ?? []).map((category) => inline(category?.subject_category ?? '')),
  );
  const { lead, senior, contributors } = people(item);
  return [
    {
      id,
      title,
      slug,
      language: item.meta?.locale?.trim().toLowerCase() || 'en',
      lead,
      senior,
      contributors,
      subjects,
      topics,
      categories,
      about: plainText(item.description ?? ''),
      warning: plainText(item.content_warning_text ?? ''),
      coverUrl: isWeb(item.cover_url) ? item.cover_url.trim() : null,
      published: /^\d{4}-\d{2}-\d{2}/.exec(item.publish_date ?? '')?.[0] ?? '',
      licence: licenceOf(item.license_url, item.license_name, item.license_version),
      titleWords: ` ${words(title)} `,
      words: ` ${words([title, ...senior, ...contributors, ...subjects, ...topics, ...categories].join(' '))} `,
    },
  ];
};

const languageRank = (language: string) => {
  const at = Object.keys(LANGUAGES).indexOf(language);
  return at < 0 ? Object.keys(LANGUAGES).length : at;
};
const topicRank = (topic: string) => {
  const at = SUBJECTS.indexOf(topic);
  return at < 0 ? SUBJECTS.length : at;
};
/** A book's place among subjects: its first in OpenStax's order, then other subjects by name. */
const subjectOf = (book: Book) =>
  [...book.topics].sort((a, b) => topicRank(a) - topicRank(b) || a.localeCompare(b))[0] ?? '';
const byTitle = (a: Book, b: Book) =>
  words(a.title).localeCompare(words(b.title)) || a.title.localeCompare(b.title);
/** Popular's order: English books, then Spanish; by subject; then by title. */
const bySubject = (a: Book, b: Book) => {
  const [first, second] = [subjectOf(a), subjectOf(b)];
  return (
    languageRank(a.language) - languageRank(b.language) ||
    topicRank(first) - topicRank(second) ||
    first.localeCompare(second) ||
    byTitle(a, b)
  );
};
const byNewest = (a: Book, b: Book) =>
  (b.published || '0').localeCompare(a.published || '0') || byTitle(a, b);

/** OpenStax's live books, in popular's order. */
const catalogue = () =>
  remembered('catalogue', KEEP_HOURS, async (): Promise<Catalogue> => {
    const items: CmsBook[] = [];
    for (let offset = 0; offset < CATALOGUE_PAGE * 4; offset += CATALOGUE_PAGE) {
      const answer = await getJson<CmsAnswer>(
        `${SITE}/apps/cms/api/v2/pages/?type=books.Book&fields=${FIELDS.join(',')}&limit=${CATALOGUE_PAGE}${offset ? `&offset=${offset}` : ''}`,
        "OpenStax's catalogue isn't there",
      );
      const page = answer.items ?? [];
      items.push(...page);
      if (page.length < CATALOGUE_PAGE || items.length >= (answer.meta?.total_count ?? 0)) break;
    }
    const books = items.flatMap(toBook).sort(bySubject);
    if (books.length === 0) throw new Error("OpenStax's catalogue lists no books");
    return { books, byId: new Map(books.map((book) => [book.id, book])) };
  });

/** The version OpenStax's reader serves of each book, and the archive that holds them. */
const release = () =>
  remembered('release', KEEP_HOURS, async (): Promise<Versions> => {
    const answer = await getJson<ReleaseAnswer>(
      RELEASE,
      "OpenStax's list of book versions isn't there",
    );
    const archive = answer.archiveUrl?.trim() ?? '';
    if (!/^\/apps\/archive\/[\w.-]+$/.test(archive)) {
      throw new Error("OpenStax's list of book versions couldn't be read");
    }
    const versions = new Map<string, string>();
    for (const [id, book] of Object.entries(answer.books ?? {})) {
      const version = book?.defaultVersion?.trim() ?? '';
      if (UUID.test(id.toLowerCase()) && /^[\w.-]+$/.test(version) && !book?.retired) {
        versions.set(id.toLowerCase(), version);
      }
    }
    return { archive, versions };
  });

/** The live books OpenStax's reader serves, in popular's order. */
const shelf = async () => {
  const [{ books }, { versions }] = await Promise.all([catalogue(), release()]);
  return books.filter((book) => versions.has(book.id));
};

const summary = (book: Book): MangaSummary => ({
  id: book.id,
  title: book.title,
  coverUrl: book.coverUrl,
  authors: book.lead.slice(0, 6),
});

const pageOf = (books: Book[], page: number): MangaPage => {
  const start = (Math.max(1, Math.floor(page) || 1) - 1) * PAGE_SIZE;
  return {
    items: books.slice(start, start + PAGE_SIZE).map(summary),
    hasNextPage: start + PAGE_SIZE < books.length,
  };
};

// ---- Search ----

const ALL: FilterOption = { value: 'all', label: 'All' };
const SORT: SelectFilter = {
  kind: 'select',
  key: 'sort',
  label: 'Sort',
  options: [
    { value: 'relevance', label: 'Best match' },
    { value: 'subject', label: 'Subject' },
    { value: 'newest', label: 'Newest' },
    { value: 'title', label: 'Title' },
  ],
  default: 'relevance',
};

/** The subject and language filters for the books there are, and the sort. */
const filtersFor = (books: Book[]) => {
  const topics = unique(books.flatMap((book) => book.topics)).sort(
    (a, b) => topicRank(a) - topicRank(b) || a.localeCompare(b),
  );
  const languages = unique(books.map((book) => book.language)).sort(
    (a, b) => languageRank(a) - languageRank(b) || a.localeCompare(b),
  );
  const subject: SelectFilter = {
    kind: 'select',
    key: 'subject',
    label: 'Subject',
    options: [ALL, ...topics.map((topic) => ({ value: topic }))],
    default: 'all',
  };
  const language: SelectFilter = {
    kind: 'select',
    key: 'language',
    label: 'Language',
    options: [
      ALL,
      ...languages.map((code) => ({ value: code, label: LANGUAGES[code] ?? code.toUpperCase() })),
    ],
    default: 'all',
  };
  return { subject, language, sort: SORT };
};

const searchBooks = async (typed: string, page: number, values: FilterValues | undefined) => {
  const books = await shelf();
  const filters = filtersFor(books);
  const subject = selectValue(values, filters.subject);
  const language = selectValue(values, filters.language);
  const sort = selectValue(values, filters.sort);
  const wanted = words(typed).split(' ').filter(Boolean);
  // Every word begins a word of the title, an author, a subject or a category.
  const has = (text: string, word: string) => text.includes(` ${word}`);
  const found = books.filter(
    (book) =>
      (subject === 'all' || book.topics.includes(subject)) &&
      (language === 'all' || book.language === language) &&
      wanted.every((word) => has(book.words, word)),
  );
  if (sort === 'newest') found.sort(byNewest);
  else if (sort === 'title') found.sort(byTitle);
  else if (sort === 'relevance' && wanted.length > 0) {
    // Books whose title has every word first, each group in popular's order.
    const inTitle = (book: Book) => wanted.every((word) => has(book.titleWords, word));
    found.sort((a, b) => Number(inTitle(b)) - Number(inTitle(a)));
  }
  return pageOf(found, page);
};

// ---- Contents ----

// The contents read last: a book's details and chapters share them.
const contentsKept = new Map<string, Promise<Contents>>();

/** A book's contents at the version OpenStax's reader serves now. */
const contentsOf = async (book: string): Promise<Contents> => {
  const { archive, versions } = await release();
  const version = versions.get(book);
  if (!version) throw new Error('OpenStax no longer offers this book');
  const address = `${SITE}${archive}/contents/${book}@${version}.json`;
  let contents = contentsKept.get(address);
  if (!contents) {
    contents = getJson<Contents>(address, 'OpenStax no longer offers this book');
    contents.catch(() => contentsKept.delete(address));
    contentsKept.set(address, contents);
    if (contentsKept.size > 6) contentsKept.delete(contentsKept.keys().next().value ?? address);
  }
  return contents;
};

const CLASS = /\bclass\s*=\s*"([^"]*)"/i;

/** The text of the first `<span>` with this class, the spans inside it included. */
const spanText = (html: string, name: string) => {
  const tags = /<(\/?)span\b([^>]*)>/gi;
  let from = -1;
  let depth = 0;
  for (let tag = tags.exec(html); tag; tag = tags.exec(html)) {
    const [whole, closing, attributes = ''] = tag;
    if (from < 0) {
      if (!closing && (CLASS.exec(attributes)?.[1] ?? '').split(/\s+/).includes(name)) {
        from = tag.index + whole.length;
        depth = 1;
      }
      continue;
    }
    depth += closing ? -1 : 1;
    if (depth === 0) return inline(html.slice(from, tag.index));
  }
  return from < 0 ? '' : inline(html.slice(from));
};

/** A contents title's number ("Chapter 1", "1.1", "Appendix A") and words. */
const titleOf = (html: string) => {
  const number = spanText(html, 'os-number');
  const text = spanText(html, 'os-text');
  return number || text ? { number, text } : { number: '', text: inline(html) };
};

type Leaf = { page: string; name: string };

/**
 * The contents' pages in reading order, named with the chapter they sit in: "1.1 Water" names
 * itself, "Introduction" is "Chapter 2 · Introduction", and a page in an unnumbered part takes
 * its name too ("Answer Key · Chapter 1"). The index is left out.
 */
const leaves = (tree: TocNode | null | undefined): Leaf[] => {
  const found: Leaf[] = [];
  const walk = (node: TocNode, context: string[], depth: number) => {
    if (depth > 8) return;
    for (const child of node.contents ?? []) {
      const { number, text } = titleOf(child.title ?? '');
      if (child.contents?.length) {
        // A numbered part ("Chapter 1", "Unit 2") names what's in it.
        walk(child, number ? [number] : [...context, text].filter(Boolean), depth + 1);
        continue;
      }
      const index =
        child.toc_target_type === 'index' ||
        (!child.toc_target_type && /^(?:index|indice)$/i.test(child.slug ?? ''));
      const page = (child.id ?? '').split('@')[0]?.trim().toLowerCase() ?? '';
      if (index || !UUID.test(page)) continue;
      const own =
        number && text ? `${number}${/^[\d.]+$/.test(number) ? ' ' : ': '}${text}` : number || text;
      if (!own) continue;
      // "Unit 1: Unit Introduction" in Unit 1 says its unit already.
      const around =
        number && context[context.length - 1] === number ? context.slice(0, -1) : context;
      found.push({ page, name: SECTION_NUMBER.test(number) ? own : [...around, own].join(' · ') });
    }
  };
  if (tree) walk(tree, [], 0);
  return found;
};

const bookId = (id: string) => {
  const book = id.trim().toLowerCase();
  if (!UUID.test(book)) throw new Error('OpenStax has no book with this id');
  return book;
};

const chapterList = async (id: string): Promise<MangaChapter[]> => {
  const book = bookId(id);
  const pages = leaves((await contentsOf(book)).tree);
  if (pages.length === 0) throw new Error("This book's contents on OpenStax list no pages");
  const seen = new Set<string>();
  return pages
    .filter((leaf) => !seen.has(leaf.page) && seen.add(leaf.page))
    .map((leaf, index) => ({ id: `${book}:${leaf.page}`, name: leaf.name, number: index + 1 }));
};

// ---- Details ----

const details = async (id: string): Promise<MangaDetails> => {
  const wanted = bookId(id);
  const book = (await catalogue()).byId.get(wanted);
  if (!book) throw new Error('OpenStax no longer lists this book');
  // The licence the book's own data states, else the catalogue's.
  let licence = book.licence;
  try {
    const contents = await contentsOf(book.id);
    licence = licenceOf(contents.license?.url, contents.license?.name) ?? licence;
  } catch (error) {
    raygun.log.info(`OpenStax's contents of ${book.title} couldn't be read: ${String(error)}`);
  }
  const credits = [
    book.senior.length > 0 && `Senior contributing authors: ${nameList(book.senior, 20)}.`,
    book.contributors.length > 0 && `Contributing authors: ${nameList(book.contributors, 20)}.`,
  ]
    .filter(Boolean)
    .join('\n');
  return {
    id: book.id,
    title: book.title,
    coverUrl: book.coverUrl,
    authors: book.senior.length > 0 ? book.senior : book.contributors.slice(0, 12),
    description: [
      book.about,
      book.warning && `Content note: ${book.warning}`,
      credits,
      book.subjects.length > 0 && `Subjects: ${book.subjects.join(', ')}.`,
      licence
        ? `Licence: ${licence.name} (from OpenStax's book data).`
        : "Licence: as the book's page on OpenStax states.",
    ]
      .filter(Boolean)
      .join('\n\n'),
    genres: unique([
      ...book.subjects,
      ...book.categories.filter((category) => category !== book.title),
    ]).slice(0, 12),
    status: 'completed',
    url: `${SITE}/details/books/${encodeURIComponent(book.slug)}`,
  };
};

// ---- Pages ----

/** A page's body: no head with its styles and scripts. */
const bodyOf = (xhtml: string) => {
  const body = /<body\b[^>]*>([\s\S]*)<\/body>/i.exec(xhtml);
  return body ? (body[1] ?? '') : xhtml.replace(/<head\b[\s\S]*?<\/head>/i, '');
};

const passage = async (chapterId: string): Promise<Passage> => {
  const parts = CHAPTER_ID.exec(chapterId.trim().toLowerCase());
  const [book = '', page = ''] = parts ? parts.slice(1) : [];
  if (!UUID.test(book) || !UUID.test(page)) throw new Error('OpenStax has no page with this id');
  const { archive, versions } = await release();
  const version = versions.get(book);
  if (!version) throw new Error('OpenStax no longer offers this book');
  // Figures are `../resources/<hash>`, beside the archive's contents.
  const contents = `${SITE}${archive}/contents/`;
  const answer = await getJson<{ content?: string | null }>(
    `${contents}${book}@${version}:${page}.json`,
    "This page isn't in the book's current version on OpenStax",
  );
  const html = bodyOf(answer.content ?? '');
  if (!inline(html) && !/<img\b/i.test(html)) throw new Error('This page has no text on OpenStax');
  return { html, baseUrl: contents };
};

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'books',
    name: 'OpenStax',
    popular: async ({ page }) => pageOf(await shelf(), page),
    latest: async ({ page }) => pageOf([...(await shelf())].sort(byNewest), page),
    search: ({ query, page, filters }) => searchBooks(query, page, filters),
    filters: async (): Promise<SourceFilter[]> => {
      const { subject, language, sort } = filtersFor(await shelf());
      return [subject, language, sort];
    },
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
    passage: ({ chapterId }) => passage(chapterId),
  }),
);
