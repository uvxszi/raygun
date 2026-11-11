// Green Tea Press: Allen B. Downey's free textbooks (with Chris Mayfield for Think Java), read from
// their web editions. The books are listed in `books.ts` with the licence each one states and where
// it states it. Web editions come in three kinds: Jupyter Books (the chapters in the sidebar, the
// text in the article), a Quarto book (Think Java) and HeVeA's HTML of the LaTeX editions (the
// chapters in the index page's contents). A chapter's text keeps its code and its output, its
// figures and its formulas (MathJax's `\(…\)` and `\[…\]` as they come); notices about printed
// copies, links to run notebooks elsewhere and the licence lines repeated under each chapter are
// left out.
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
import type { Book } from './books';

const HEADERS = { 'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )' };
const PAGE = /^[\w.-]+\.html$/;
const PAGE_SIZE = 30;

const SUBJECTS = [
  { value: 'all', label: 'All' },
  { value: 'programming', label: 'Programming' },
  { value: 'data', label: 'Statistics and data science' },
  { value: 'science', label: 'Science and engineering' },
  { value: 'systems', label: 'Computer systems' },
];
const SORTS = [
  { value: 'featured', label: 'Featured' },
  { value: 'newest', label: 'Newest' },
  { value: 'title', label: 'Title (A–Z)' },
];

const decode = (text: string) =>
  text
    .replace(/&nbsp;|&#160;|&#XA0;/gi, ' ')
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

const fetchPage = async (url: string) => {
  let response;
  try {
    response = await raygun.http.fetch(url, { headers: HEADERS });
  } catch {
    throw new Error("The book's website didn't answer; try again in a minute");
  }
  if (response.status === 404) throw new Error('The book no longer has this page');
  if (!response.ok)
    throw new Error(`The book's website answered with an error (HTTP ${response.status})`);
  return { text: await response.text(), url: response.url || url };
};

const bookOf = (id: string): Book => {
  const book = BOOKS.find((known) => known.id === id);
  if (!book) throw new Error('Green Tea Press has no book by this name here');
  return book;
};

type Link = { page: string; title: string };

/** The chapters a book's contents list, in order, each page once. */
const contents = (book: Book, html: string): Link[] => {
  const links: Link[] = [];
  const add = (href: string, title: string) => {
    const page = decode(href).replace(/^\.\//, '');
    if (!PAGE.test(page) || links.some((link) => link.page === page)) return;
    if (book.skip?.includes(page) || /^(?:contents|index)$/i.test(title)) return;
    links.push({ page, title: title || page.replace(/\.html$/, '') });
  };
  const anchors = (part: string) => [...part.matchAll(/<a\b([^>]*)>([\s\S]*?)<\/a>/g)];
  if (book.format === 'jupyter') {
    // The sidebar: `class="bd-links bd-docs-nav"`, or `id="bd-docs-nav"` in older themes.
    const nav = element(html, /<nav\b[^>]*\bbd-docs-nav\b/) ?? '';
    for (const match of anchors(nav)) {
      const href = /\bhref="([^"#]+)"/.exec(match[1] ?? '')?.[1];
      if (href && classes(match[1] ?? '').includes('internal'))
        add(href, plainText(match[2] ?? ''));
    }
  } else if (book.format === 'quarto') {
    const sidebar = element(html, /<nav id="quarto-sidebar"/) ?? html;
    for (const match of anchors(sidebar)) {
      const href = /\bhref="([^"#]+)"/.exec(match[1] ?? '')?.[1];
      if (href && classes(match[1] ?? '').includes('sidebar-link')) {
        add(href, plainText(match[2] ?? '').replace(/^(\d+)\s+/, '$1 '));
      }
    }
  } else {
    // HeVeA's index: the chapters' pages, then their sections as `page#sec…`.
    const content = element(html, /<div id="content"/) ?? html;
    for (const match of anchors(content)) {
      const href = /\bhref="([^"#]+)"/.exec(match[1] ?? '')?.[1];
      if (href && !/^[a-z]+:/i.test(href)) add(href, plainText(match[2] ?? ''));
    }
  }
  return links;
};

/** Notices about printed copies and links to run the notebook elsewhere, not the book's text. */
const isNotice = (paragraph: string) =>
  /colab\.research\.google\.com|mybinder\.org/.test(paragraph) ||
  /^(?:you can order print|printed (?:and electronic )?copies|print and electronic versions|the (?:third|second) edition of .* is available now|if you find this book useful|and you might like this new book)/i.test(
    plainText(paragraph),
  );

/** The licence lines under each chapter: the book's title, copyright and licences. */
const LICENCE_LINE = /^(?:Copyright\s+(?:©\s*)?\d{4}|Code license:|Text license:|License:)/i;

const dropLicenceLines = (html: string) => {
  const paragraphs = [...html.matchAll(/<p\b[^>]*>([\s\S]*?)<\/p>/g)];
  const dropped = new Set<number>();
  paragraphs.forEach((match, index) => {
    if (!LICENCE_LINE.test(plainText(match[1] ?? ''))) return;
    dropped.add(index);
    // The book's name, set on its own line just above the copyright.
    const before = paragraphs[index - 1];
    const title = plainText(before?.[1] ?? '');
    if (
      before &&
      /^Copyright/i.test(plainText(match[1] ?? '')) &&
      title.length < 90 &&
      !/[.:?!]$/.test(title)
    ) {
      dropped.add(index - 1);
    }
  });
  let kept = html;
  for (const index of [...dropped].sort((first, second) => second - first)) {
    const match = paragraphs[index];
    if (!match || match.index === undefined) continue;
    kept = kept.slice(0, match.index) + kept.slice(match.index + match[0].length);
  }
  return kept;
};

const jupyterText = (html: string) => {
  const article =
    element(html, /<article class="bd-article/) ?? element(html, /<main id="main-content"/) ?? '';
  // Notices sit before the chapter's first section.
  const first = article.search(/<section\b/);
  const body = first >= 0 ? article.slice(first) : article;
  const cleaned = dropElements(body, (tag, attributes) => {
    const names = classes(attributes);
    return (
      tag === 'summary' ||
      tag === 'footer' ||
      tag === 'script' ||
      names.includes('headerlink') ||
      names.some((name) => name.startsWith('prev-next'))
    );
  });
  return dropLicenceLines(
    cleaned.replace(/<p\b[^>]*>[\s\S]*?<\/p>/g, (paragraph) =>
      isNotice(paragraph) ? '' : paragraph,
    ),
  );
};

const quartoText = (html: string) => {
  const main = element(html, /<main class="content"/) ?? '';
  return dropElements(main, (tag, attributes) => {
    const names = classes(attributes);
    return (
      tag === 'nav' ||
      tag === 'script' ||
      names.includes('notice') ||
      names.includes('anchorjs-link') ||
      names.includes('quarto-title-meta')
    );
  });
};

/** HeVeA's links to the previous and next chapters, with the rule between them and the text. */
const HEVEA_NAVIGATION = new RegExp(
  String.raw`(?:<hr\b[^>]*>\s*)?(?:<a\b[^>]*>\s*<img\b[^>]*\balt="(?:Previous|Up|Next)"[^>]*>\s*<\/a>\s*)+(?:<hr\b[^>]*>)?`,
  'gi',
);

/**
 * HeVeA's chapter: `div#content` in newer pages; older ones set the text in a table cell beside
 * the sidebar, between links to the previous and next chapters.
 */
const heveaText = (html: string) => {
  const part = element(html, /<div id="content"/) ?? element(html, /<body\b/) ?? html;
  return dropElements(part, (tag, attributes) => {
    const names = classes(attributes);
    return (
      tag === 'script' ||
      names.includes('notice') ||
      names.includes('gtp-sidebar') ||
      /\bid="(?:sidebar|ea-slot)"/.test(attributes)
    );
  })
    .replace(HEVEA_NAVIGATION, '')
    .replace(/<a\b[^>]*>\s*Buy this book[\s\S]*?<\/a>/gi, '');
};

const summary = (book: Book): MangaSummary => ({
  id: book.id,
  title: book.title,
  coverUrl: book.cover,
  authors: book.authors,
});

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

const newest = (books: Book[]) => [...books].sort((first, second) => second.year - first.year);

const search = (query: string, page: number, values: FilterValues | undefined): MangaPage => {
  const subject = pick(values, 'subject', SUBJECTS);
  const sort = pick(values, 'sort', SORTS);
  const terms = words(query).split(' ').filter(Boolean);
  const found = BOOKS.filter((book) => {
    if (subject && subject !== 'all' && book.subject !== subject) return false;
    const text = words(`${book.title} ${book.authors.join(' ')} ${book.description}`);
    return terms.every((term) => text.includes(term));
  });
  const titled = (book: Book) => terms.filter((term) => words(book.title).includes(term)).length;
  const ordered =
    sort === 'newest'
      ? newest(found)
      : sort === 'title'
        ? [...found].sort((first, second) => first.title.localeCompare(second.title))
        : [...found].sort((first, second) => titled(second) - titled(first));
  return pageOf(ordered, page);
};

const SUBJECT_NAMES: Record<Book['subject'], string> = {
  programming: 'Programming',
  data: 'Statistics and data science',
  science: 'Science and engineering',
  systems: 'Computer systems',
};

const details = (book: Book): MangaDetails => ({
  id: book.id,
  title: book.title,
  coverUrl: book.cover,
  authors: book.authors,
  description: [
    book.description,
    `Published by Green Tea Press, ${book.year}. Licence: ${book.licence}, as stated at ${book.licenceAt}`,
  ].join('\n\n'),
  genres: [SUBJECT_NAMES[book.subject]],
  status: 'completed',
  url: book.site,
});

const filters: SourceFilter[] = [
  { kind: 'select', key: 'subject', label: 'Subject', options: SUBJECTS },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'books',
    name: 'Green Tea Press',
    popular: async ({ page }) => pageOf(BOOKS, page),
    latest: async ({ page }) => pageOf(newest(BOOKS), page),
    search: async ({ query, page, filters: values }) => search(query, page, values),
    filters: async () => filters,
    details: async ({ id }) => details(bookOf(id)),
    chapters: async ({ id }): Promise<{ chapters: MangaChapter[] }> => {
      const book = bookOf(id);
      const index = await fetchPage(`${book.site}index.html`);
      const chapters = contents(book, index.text).map((link, number) => ({
        id: `${book.id}/${link.page}`,
        name: link.title,
        number: number + 1,
      }));
      if (chapters.length === 0) throw new Error("This book's contents list no chapters");
      return { chapters };
    },
    passage: async ({ chapterId }): Promise<Passage> => {
      const slash = chapterId.indexOf('/');
      const book = bookOf(chapterId.slice(0, slash));
      const page = chapterId.slice(slash + 1);
      if (slash < 0 || !PAGE.test(page)) throw new Error('Not a chapter of a Green Tea Press book');
      const found = await fetchPage(`${book.site}${page}`);
      const html =
        book.format === 'jupyter'
          ? jupyterText(found.text)
          : book.format === 'quarto'
            ? quartoText(found.text)
            : heveaText(found.text);
      if (!plainText(html)) throw new Error('This chapter has no text');
      return { html, baseUrl: found.url };
    },
  }),
);
