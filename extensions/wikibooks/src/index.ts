// Wikibooks: open textbooks and manuals written by Wikibooks' volunteers, through the MediaWiki
// API. All of Wikibooks' text is under CC BY-SA 4.0; a book's details name the licence the wiki
// itself states for its content (its `rightsinfo`). Popular is last month's most-read books
// (Wikimedia's page view counts, each page counted for the book it belongs to), latest the books
// most recently marked completed, and search finds pages and lists the books they belong to. Only
// books Wikibooks catalogues are listed (main pages in its book categories). A book's chapters are
// its saved collection (`Wikibooks:Collections/<book>`) when it has one, else the subpages its
// main page links to, in that order, else all of its subpages.
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

const WIKI = 'https://en.wikibooks.org';
const API = `${WIKI}/w/api.php`;
const PAGEVIEWS = 'https://wikimedia.org/api/rest_v1/metrics/pageviews/top/en.wikibooks/all-access';
const PAGE_SIZE = 30;
/** Search hits read for one page of books: several hits are usually pages of one book. */
const HITS = 50;
const AGENT = 'Raygun/1.0 ( https://github.com/uvxszi/raygun )';
// Wikimedia asks scripts to name themselves; `Api-User-Agent` is the header for clients whose
// User-Agent is set for them.
const HEADERS = { 'User-Agent': AGENT, 'Api-User-Agent': AGENT };
/** The main namespace and Wikijunior's, where books live. */
const BOOK_NAMESPACES = '0|110';
/** The hidden categories Wikibooks files its books' main pages in. */
const BOOK_CATEGORIES = [
  'Book:Wikibooks Stacks/Books',
  'Books by completion status/all books',
  'Featured books',
  'Completed books',
].map((name) => `Category:${name}`);
/** Catalogued like books, but they are the catalogue and the front page. */
const NOT_BOOKS = new Set(['Wikibooks Stacks', 'Main Page']);
/** Page view counts cover every namespace; these hold no books. */
const OTHER_PAGES =
  /^(?:Special|Talk|User|Wikibooks|File|MediaWiki|Template|Help|Category|Cookbook|Transwiki|Subject|Module|TimedText|Event|Media|Department|[^:/]+ talk):/i;
/** One page with the whole book, or a PDF's: the chapters already hold it. */
const WHOLE_BOOK_PAGES =
  /\/(?:print(?:able)?[ _-]?version|pdf(?:[ _-]?version)?|all[ _]chapters|single[ _]page)$/i;
/** A main page with this much text besides its links is a chapter too, not only contents. */
const MAIN_TEXT = 3000;
/** Navigation, edit links, notices and boxes for the screen only, as Wikibooks' print view drops. */
const DROPPED_CLASSES = new Set([
  'noprint',
  'navbox',
  'navigation-not-searchable',
  'hatnote',
  'mw-editsection',
  'catlinks',
  'printfooter',
  'messagebox',
  'ambox',
  'metadata',
  'mw-empty-elt',
  // Boards drawn with CSS, which the reader can't show.
  'chessboard-container',
]);

const SUBJECTS = [
  { value: 'all', label: 'All' },
  { value: 'Computing', label: 'Computing' },
  { value: 'Engineering', label: 'Engineering' },
  { value: 'Humanities', label: 'Humanities' },
  { value: 'Languages', label: 'Languages' },
  { value: 'Mathematics', label: 'Mathematics' },
  { value: 'Science', label: 'Science' },
  { value: 'Social sciences', label: 'Social sciences' },
  { value: 'Standard curricula', label: 'School curricula' },
  { value: 'Recreational activities', label: 'Recreation' },
  { value: 'Miscellaneous', label: 'Miscellaneous' },
  { value: 'Wikijunior', label: 'Wikijunior (for children)' },
];
const STATUSES = [
  { value: 'all', label: 'All books' },
  { value: 'Featured books', label: 'Featured' },
  { value: 'Completed books', label: 'Completed' },
];
const SORTS = [
  { value: 'relevance', label: 'Best match' },
  { value: 'incoming_links_desc', label: 'Most linked' },
  { value: 'last_edit_desc', label: 'Recently edited' },
  { value: 'create_timestamp_desc', label: 'Newest' },
];

type Category = { title: string; hidden?: boolean };
type WikiPage = {
  title: string;
  ns: number;
  missing?: boolean;
  invalid?: boolean;
  categories?: Category[];
  thumbnail?: { source?: string };
  extract?: string;
  revisions?: { slots?: { main?: { content?: string } } }[];
};
type Rename = { from: string; to: string };
type QueryAnswer = {
  query?: {
    normalized?: Rename[];
    redirects?: Rename[];
    pages?: WikiPage[];
    search?: { title: string }[];
    searchinfo?: { totalhits?: number };
    categorymembers?: { title: string }[];
    allpages?: { title: string }[];
    rightsinfo?: { url?: string; text?: string };
  };
};
type ParseAnswer = { parse?: { title: string; text: string } };
type TopAnswer = { items?: { articles?: { article: string; views: number }[] }[] };

// Wikimedia asks API clients to send one request at a time.
let turn: Promise<unknown> = Promise.resolve();
const inTurn = <T>(task: () => Promise<T>): Promise<T> => {
  const next = turn.then(task);
  turn = next.catch(() => undefined);
  return next;
};

const request = (url: string) =>
  inTurn(() =>
    raygun.http.fetch(url, { headers: HEADERS }).catch(() => {
      throw new Error("Wikibooks didn't answer; try again in a minute");
    }),
  );

const api = async <T>(params: Record<string, string | number>): Promise<T> => {
  const query = Object.entries({ format: 'json', formatversion: 2, ...params })
    .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
    .join('&');
  const response = await request(`${API}?${query}`);
  if (!response.ok) throw new Error(`Wikibooks answered with an error (HTTP ${response.status})`);
  const answer = await response.json<T & { error?: { info?: string } }>();
  if (answer.error) {
    throw new Error(`Wikibooks couldn't answer: ${answer.error.info ?? 'the request failed'}`);
  }
  return answer;
};

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

/** Text without markup; inline tags join up with the words around them, blocks don't. */
const plainText = (html: string) =>
  decode(
    html
      .replace(/<style[\s\S]*?<\/style>/g, '')
      .replace(/<br\s*\/?>|<\/(?:p|div|li|dd|dt|td|th|h\d)>/gi, ' ')
      .replace(/<[^>]+>/g, ''),
  )
    .replace(/\s+/g, ' ')
    .trim();

/** Wikitext of a heading or label as plain text: links' labels, no templates, quotes or tags. */
const wikiText = (text: string) =>
  decode(
    text
      .replace(/\{\{[^{}]*\}\}/g, '')
      .replace(/\[\[(?:[^\]|]*\|)?([^\]]*)\]\]/g, '$1')
      .replace(/'{2,}/g, '')
      .replace(/<[^>]+>/g, ''),
  )
    .replace(/\s+/g, ' ')
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

const classes = (attributes: string) =>
  /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/i
    .exec(attributes)
    ?.slice(1)
    .join(' ')
    .split(/\s+/)
    .filter(Boolean) ?? [];

/** "Python_Programming/Lists" as the API spells titles: spaces, first letter in capitals. */
const normalTitle = (title: string) => {
  const spaced = decode(title).replace(/_/g, ' ').replace(/\s+/g, ' ').trim();
  return spaced.charAt(0).toUpperCase() + spaced.slice(1);
};

/** A subpage ("Book/Chapter 1") belongs to the book before the first slash. */
const bookOf = (title: string) => title.split('/')[0] ?? title;

/** Wikijunior's books carry its namespace in their titles. */
const bookName = (title: string) => title.replace(/^Wikijunior:\s*/, 'Wikijunior: ');

const pageUrl = (title: string) =>
  `${WIKI}/wiki/${title.replace(/ /g, '_').split('/').map(encodeURIComponent).join('/')}`;

const cleanImage = (url: string | undefined) =>
  url ? (url.startsWith('//') ? `https:${url}` : url).replace(/\?utm_[^#]*$/, '') : null;

// Lists stay in memory for a while: the counts change monthly, the categories seldom.
const kept = new Map<string, { at: number; value: Promise<unknown> }>();
const remembered = <T>(key: string, hours: number, load: () => Promise<T>): Promise<T> => {
  const known = kept.get(key);
  if (known && Date.now() - known.at < hours * 3_600_000) return known.value as Promise<T>;
  const value = load();
  kept.set(key, { at: Date.now(), value });
  value.catch(() => kept.delete(key));
  return value;
};

// The books each list's pages answered, so a book an earlier page listed isn't listed again (the
// pages of one book can be spread over several pages of hits).
const shown = new Map<string, string[][]>();
const unseen = (list: string, page: number, items: MangaSummary[]) => {
  let pages = shown.get(list);
  if (!pages) {
    pages = [];
    shown.set(list, pages);
    if (shown.size > 30) shown.delete(shown.keys().next().value ?? list);
  }
  const before = new Set(pages.slice(0, page - 1).flat());
  const fresh = items.filter((item) => !before.has(item.id));
  pages[page - 1] = fresh.map((item) => item.id);
  return fresh;
};

/** The books among these titles in the same order, with covers: catalogued main pages only. */
const books = async (titles: string[]): Promise<MangaSummary[]> => {
  const asked = [...new Set(titles)].slice(0, 50);
  if (asked.length === 0) return [];
  const answer = await api<QueryAnswer>({
    action: 'query',
    titles: asked.join('|'),
    prop: 'categories|pageimages',
    clcategories: BOOK_CATEGORIES.join('|'),
    cllimit: 'max',
    piprop: 'thumbnail',
    pithumbsize: 400,
    redirects: 1,
  });
  const renamed = new Map<string, string>();
  for (const rename of [...(answer.query?.normalized ?? []), ...(answer.query?.redirects ?? [])]) {
    renamed.set(rename.from, rename.to);
  }
  const pages = new Map((answer.query?.pages ?? []).map((page) => [page.title, page]));
  const seen = new Set<string>();
  return asked.flatMap((title) => {
    let name = title;
    for (let hop = 0; hop < 3; hop += 1) name = renamed.get(name) ?? name;
    const page = pages.get(name);
    const isBook =
      page !== undefined &&
      !page.missing &&
      !page.invalid &&
      (page.ns === 0 || page.ns === 110) &&
      !page.title.includes('/') &&
      !NOT_BOOKS.has(page.title) &&
      (page.categories?.length ?? 0) > 0;
    if (!isBook || seen.has(page.title)) return [];
    seen.add(page.title);
    return [
      { id: page.title, title: bookName(page.title), coverUrl: cleanImage(page.thumbnail?.source) },
    ];
  });
};

const monthBack = (back: number) => {
  const now = new Date();
  const month = new Date(Date.UTC(now.getUTCFullYear(), now.getUTCMonth() - back, 1));
  return `${month.getUTCFullYear()}/${String(month.getUTCMonth() + 1).padStart(2, '0')}`;
};

/** Books by last month's page views, each page counted for its book, most read first. */
const mostRead = () =>
  remembered('popular', 12, async () => {
    // A month's counts arrive a day or so into the next; the month before stands in until then.
    for (const back of [1, 2]) {
      const response = await request(`${PAGEVIEWS}/${monthBack(back)}/all-days`);
      if (response.status === 404) continue;
      if (!response.ok) {
        throw new Error(
          `Wikimedia's reading counts answered with an error (HTTP ${response.status})`,
        );
      }
      const answer = await response.json<TopAnswer>();
      const views = new Map<string, number>();
      for (const { article, views: count } of answer.items?.[0]?.articles ?? []) {
        const title = normalTitle(article);
        if (OTHER_PAGES.test(title)) continue;
        const book = bookOf(title);
        views.set(book, (views.get(book) ?? 0) + count);
      }
      return [...views].sort((a, b) => b[1] - a[1]).map(([book]) => book);
    }
    throw new Error("Wikimedia hasn't counted last month's reading yet; try again tomorrow");
  });

/** Books in the order Wikibooks last marked them completed. */
const recentlyCompleted = () =>
  remembered('completed', 12, async () => {
    const answer = await api<QueryAnswer>({
      action: 'query',
      list: 'categorymembers',
      cmtitle: 'Category:Completed books',
      cmnamespace: BOOK_NAMESPACES,
      cmsort: 'timestamp',
      cmdir: 'desc',
      cmprop: 'title',
      cmlimit: 'max',
    });
    return (answer.query?.categorymembers ?? []).map((member) => member.title);
  });

const listPage = async (list: string, titles: string[], page: number): Promise<MangaPage> => {
  const start = (Math.max(1, page) - 1) * PAGE_SIZE;
  const items = await books(titles.slice(start, start + PAGE_SIZE));
  return { items: unseen(list, page, items), hasNextPage: start + PAGE_SIZE < titles.length };
};

/** A page of books from a Wikibooks search: the books of a page of hits, in their order. */
const searchBooks = async (search: string, sort: string, page: number): Promise<MangaPage> => {
  const offset = (Math.max(1, page) - 1) * HITS;
  const answer = await api<QueryAnswer>({
    action: 'query',
    list: 'search',
    srsearch: search,
    srnamespace: BOOK_NAMESPACES,
    srlimit: HITS,
    sroffset: offset,
    srsort: sort,
    srprop: '',
    srinfo: 'totalhits',
  });
  const hits = answer.query?.search ?? [];
  const items = await books(hits.map((hit) => bookOf(hit.title)));
  return {
    items: unseen(`search ${sort} ${search}`, page, items),
    hasNextPage:
      hits.length > 0 && offset + hits.length < (answer.query?.searchinfo?.totalhits ?? 0),
  };
};

// The pages read last stay in memory: a book's chapters and its first chapter can share one.
const parsedPages = new Map<string, Promise<{ title: string; html: string }>>();
const parsed = (title: string) => {
  let page = parsedPages.get(title);
  if (!page) {
    page = api<ParseAnswer>({
      action: 'parse',
      page: title,
      prop: 'text',
      redirects: 1,
      disableeditsection: 1,
      disablelimitreport: 1,
      disabletoc: 1,
    }).then((answer) => {
      if (!answer.parse) throw new Error('Wikibooks has no page by this name');
      return { title: answer.parse.title, html: answer.parse.text };
    });
    page.catch(() => parsedPages.delete(title));
    parsedPages.set(title, page);
    if (parsedPages.size > 8) parsedPages.delete(parsedPages.keys().next().value ?? title);
  }
  return page;
};

/** "CC BY-SA 4.0" from the licence's address, else its name as the wiki gives it. */
const licenceName = (rights: { url?: string; text?: string } | undefined) => {
  const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d+(?:\.\d+)?)/i.exec(
    rights?.url ?? '',
  );
  if (creativeCommons) {
    return `CC ${(creativeCommons[1] ?? '').toUpperCase()} ${creativeCommons[2] ?? ''}`;
  }
  return rights?.text || 'CC BY-SA 4.0';
};

const details = async (id: string): Promise<MangaDetails> => {
  const answer = await api<QueryAnswer>({
    action: 'query',
    titles: id,
    prop: 'extracts|pageimages|categories',
    exintro: 1,
    explaintext: 1,
    piprop: 'thumbnail',
    pithumbsize: 600,
    cllimit: 'max',
    clprop: 'hidden',
    meta: 'siteinfo',
    siprop: 'rightsinfo',
    redirects: 1,
  });
  const page = answer.query?.pages?.[0];
  if (!page || page.missing || page.invalid) throw new Error('Wikibooks has no book by this name');
  const categories = page.categories ?? [];
  const names = categories.map((category) => category.title.replace(/^Category:/, ''));
  const shelves = categories
    .filter((category) => !category.hidden && category.title.startsWith('Category:Shelf:'))
    .map((category) => category.title.slice('Category:Shelf:'.length));
  const about = (page.extract ?? '').replace(/\n+/g, '\n\n').trim();
  const licence = licenceName(answer.query?.rightsinfo);
  return {
    id: page.title,
    title: bookName(page.title),
    coverUrl: cleanImage(page.thumbnail?.source),
    authors: [],
    description: [
      about,
      `Written by Wikibooks' contributors. Licence: ${licence}, as Wikibooks states for all of its text.`,
    ]
      .filter(Boolean)
      .join('\n\n'),
    genres: [...new Set(shelves)].slice(0, 8),
    status: names.includes('Completed books') ? 'completed' : 'ongoing',
    url: pageUrl(page.title),
  };
};

type Link = { title: string; label: string };

/** A subpage's own name: "Understanding monads/Maybe" for "Haskell/Understanding monads/Maybe". */
const leaf = (title: string, book: string) =>
  title === book ? bookName(book) : title.slice(book.length + 1).replace(/\//g, ' · ');

/** The book's pages in its saved collection, named with the headings above them. */
const collection = async (book: string): Promise<Link[]> => {
  const name = `Wikibooks:Collections/${book}`;
  const answer = await api<QueryAnswer>({
    action: 'query',
    titles: name,
    prop: 'revisions',
    rvprop: 'content',
    rvslots: 'main',
    redirects: 1,
  });
  const page = answer.query?.pages?.[0];
  // A collection under another name (a book's "beginner's track") holds only part of the book.
  if (!page || page.missing || page.title !== name) return [];
  const links: Link[] = [];
  let part = '';
  for (const line of (page.revisions?.[0]?.slots?.main?.content ?? '').split('\n')) {
    const heading = /^;\s*(.+)$/.exec(line);
    if (heading) {
      part = wikiText(heading[1] ?? '');
      continue;
    }
    const link = /^:\s*\[\[\s*([^\]|#]+)(?:#[^\]|]*)?(?:\|([^\]]*))?\]\]/.exec(line);
    if (!link) continue;
    const title = normalTitle(link[1] ?? '');
    if ((title !== book && !title.startsWith(`${book}/`)) || WHOLE_BOOK_PAGES.test(title)) continue;
    const label = wikiText(link[2] ?? '') || leaf(title, book);
    links.push({ title, label: part ? `${part} · ${label}` : label });
  }
  return links;
};

/** Links to the book's subpages, in the order the page gives them. */
const subpageLinks = (html: string, book: string): Link[] => {
  const links: Link[] = [];
  for (const match of html.matchAll(
    /<a href="\/wiki\/[^"]*"[^>]*?\btitle="([^"]+)"[^>]*>([\s\S]*?)<\/a>/g,
  )) {
    const title = decode(match[1] ?? '');
    if (!title.startsWith(`${book}/`) || WHOLE_BOOK_PAGES.test(title)) continue;
    const text = plainText(match[2] ?? '');
    // "Lists II (map)" says more than the page's name; "the overview" in a sentence doesn't.
    links.push({ title, label: /^[\p{Lu}\p{N}]/u.test(text) ? text : leaf(title, book) });
  }
  return links;
};

/** Every subpage of the book, in alphabetical order: the last resort. */
const allSubpages = async (book: string): Promise<Link[]> => {
  const junior = book.startsWith('Wikijunior:');
  const answer = await api<QueryAnswer>({
    action: 'query',
    list: 'allpages',
    apnamespace: junior ? 110 : 0,
    apprefix: `${junior ? book.slice('Wikijunior:'.length) : book}/`,
    apfilterredir: 'nonredirects',
    aplimit: 'max',
  });
  return (answer.query?.allpages ?? [])
    .map((page) => page.title)
    .filter((title) => !WHOLE_BOOK_PAGES.test(title))
    .map((title) => ({ title, label: leaf(title, book) }));
};

const chapterList = async (id: string): Promise<MangaChapter[]> => {
  const main = await parsed(id);
  const book = main.title;
  let links = await collection(book);
  if (links.length === 0) links = subpageLinks(main.html, book);
  if (links.length === 0) links = await allSubpages(book);
  // The main page leads when it has text of its own besides the contents.
  const ownText = plainText(cleanPage(main.html).replace(/<a\b[\s\S]*?<\/a>/g, '')).length;
  if (links.length === 0 || (ownText > MAIN_TEXT && !links.some((link) => link.title === book))) {
    links.unshift({ title: book, label: bookName(book) });
  }
  const seen = new Set<string>();
  return links
    .filter((link) => !seen.has(link.title) && seen.add(link.title))
    .map((link, index) => ({ id: link.title, name: link.label, number: index + 1 }));
};

/** A placeholder in MediaWiki's italics, `''name''`, as it stays inside highlighted code. */
const PLACEHOLDER = /''([^'<>\n]{1,60})''/g;
/** Code highlighted as LaTeX in the line (`{{LaTeX/LaTeX|code=…}}`). */
const LATEX_CODE = /<code\b[^>]*\bmw-highlight-lang-(?:latex|tex)\b[^>]*>[\s\S]*?<\/code>/g;

/**
 * MediaWiki leaves wikitext inside highlighted code as written, so the LaTeX book's placeholders
 * (`\chapter{''title''}`, meant in italics) keep their quotes on the site: they're italics again.
 * In LaTeX, `''` only closes a quotation, so a word between two pairs is never meant as written.
 */
const placeholders = (html: string) =>
  html.replace(LATEX_CODE, (code) => code.replace(PLACEHOLDER, '<i>$1</i>'));

/** The page's own text: no navigation, notices or edit links; maths and code as they are. */
const cleanPage = (html: string) =>
  placeholders(
    dropElements(html.replace(/<!--[\s\S]*?-->/g, ''), (_tag, attributes) =>
      classes(attributes).some(
        (name) =>
          DROPPED_CLASSES.has(name) ||
          name.startsWith('mbox-') ||
          // Pictures of formulas, shown only where MathML can't be: the MathML stays.
          name.startsWith('mwe-math-fallback'),
      ),
    ),
  );

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'subject', label: 'Subject', options: SUBJECTS },
  { kind: 'select', key: 'status', label: 'Books', options: STATUSES },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'books',
    name: 'Wikibooks',
    popular: async ({ page }) => listPage('popular', await mostRead(), page),
    latest: async ({ page }) => listPage('completed', await recentlyCompleted(), page),
    search: ({ query, page, filters: values }) => {
      const words = query.trim();
      const subject = pick(values, 'subject', SUBJECTS);
      const status = pick(values, 'status', STATUSES);
      const categories = [
        subject && subject !== 'all' && `Department:${subject}/all books`,
        status && status !== 'all' && status,
      ].filter((category): category is string => Boolean(category));
      // Without words, every catalogued book (the filters narrow it).
      if (!words && categories.length === 0) categories.push('Book:Wikibooks Stacks/Books');
      const search = [words, ...categories.map((category) => `incategory:"${category}"`)]
        .filter(Boolean)
        .join(' ');
      const sort = pick(values, 'sort', SORTS) ?? (words ? 'relevance' : 'incoming_links_desc');
      return searchBooks(search, sort, page);
    },
    filters: async () => filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
    passage: async ({ chapterId }): Promise<Passage> => {
      const page = await parsed(chapterId);
      return { html: cleanPage(page.html), baseUrl: pageUrl(page.title) };
    },
  }),
);
