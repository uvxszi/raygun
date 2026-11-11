// GNU Manuals: the GNU Project's manuals on www.gnu.org, as Texinfo writes them for the web (a page
// per node, or one page for a whole manual). The manuals listed are in `manuals.ts`, made from
// GNU's manuals index and each manual's copying notice, and searched here. A manual's chapters are
// the entries of the table of contents in its Top node: a node's page, or the part of a page
// between two entries. The copying notice is read again from the Top node, and a manual whose
// notice doesn't grant a free licence isn't served. www.gnu.org can be slow to connect, so
// requests go one at a time and fail with a plain message.
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
import { MANUALS } from './manuals';
import type { Manual } from './manuals';

const SITE = 'https://www.gnu.org';
const PAGE_SIZE = 30;
const HEADERS = { 'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )' };
/** Texinfo's file names: letters, digits, `-`, `.` and `_` (which starts its escapes). */
const PAGE = /^[\w.-]+\.html?$/;
const ANCHOR = /^[\w.:-]*$/;
/** How long details wait for the Top node before answering from the list alone. */
const DETAILS_WAIT_MS = 8000;
/** Texinfo's navigation, menus of the nodes below, contents lists and links beside headings. */
const DROPPED_CLASSES = new Set([
  'nav-panel',
  'header',
  'node',
  'menu',
  'mini-toc',
  'contents',
  'shortcontents',
  'region-contents',
  'copiable-link',
  'copiable-anchor',
]);

const CATEGORIES = [
  { value: 'all', label: 'All' },
  ...[...new Set(MANUALS.map((manual) => manual.category))]
    .sort((first, second) => first.localeCompare(second))
    .map((category) => ({ value: category, label: category })),
];
const SORTS = [
  { value: 'known', label: 'Best known' },
  { value: 'title', label: 'Title (A–Z)' },
];

const decode = (text: string) =>
  text
    .replace(/&nbsp;|&#160;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;|&apos;/g, "'")
    .replace(/&rsquo;/g, '’')
    .replace(/&lsquo;/g, '‘')
    .replace(/&ldquo;/g, '“')
    .replace(/&rdquo;/g, '”')
    .replace(/&ndash;/g, '–')
    .replace(/&mdash;/g, '—')
    .replace(/&copy;/g, '©')
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

const classes = (attributes: string) =>
  /\bclass\s*=\s*(?:"([^"]*)"|'([^']*)')/i
    .exec(attributes)
    ?.slice(1)
    .join(' ')
    .split(/\s+/)
    .filter(Boolean) ?? [];

// www.gnu.org is asked one thing at a time.
let turn: Promise<unknown> = Promise.resolve();
const inTurn = <T>(task: () => Promise<T>): Promise<T> => {
  const next = turn.then(task);
  turn = next.catch(() => undefined);
  return next;
};

type Page = { text: string; url: string };

const fetchPage = (url: string): Promise<Page> =>
  inTurn(async () => {
    let response;
    try {
      response = await raygun.http.fetch(url, { headers: HEADERS });
    } catch {
      throw new Error("www.gnu.org didn't answer in time; try again in a minute");
    }
    if (response.status === 404)
      throw new Error('www.gnu.org no longer has this page of the manual');
    if (!response.ok)
      throw new Error(`www.gnu.org answered with an error (HTTP ${response.status})`);
    return { text: await response.text(), url: response.url };
  });

// The pages read last stay in memory: a manual's Top node serves its details and chapters, and a
// manual on one page serves every chapter.
const pages = new Map<string, Promise<Page>>();
const cachedPage = (url: string) => {
  let found = pages.get(url);
  if (!found) {
    found = fetchPage(url);
    found.catch(() => pages.delete(url));
    pages.set(url, found);
    if (pages.size > 4) pages.delete(pages.keys().next().value ?? url);
  }
  return found;
};

const manualOf = (id: string): Manual => {
  const manual = MANUALS.find((known) => known.id === id);
  if (!manual) throw new Error('Raygun lists no GNU manual by this name');
  return manual;
};

/** The manual's folder and its Top node's file: "…/html_node/" is "…/html_node/index.html". */
const placeOf = (manual: Manual) => {
  const slash = manual.path.lastIndexOf('/');
  return {
    folder: manual.path.slice(0, slash + 1),
    top: manual.path.slice(slash + 1) || 'index.html',
  };
};

const pageUrl = (manual: Manual, file: string) => `${SITE}${placeOf(manual).folder}${file}`;

/** The copying notice: in a comment atop each of Texinfo's pages, or quoted in the Top node. */
const noticeOf = (html: string) => {
  for (const comment of html.matchAll(/<!--([\s\S]*?)-->/g)) {
    const text = comment[1] ?? '';
    if (/permission|licen[cs]e/i.test(text)) return plainText(text);
  }
  const quote = /<blockquote[^>]*>([\s\S]*?)<\/blockquote>/.exec(html)?.[1] ?? '';
  return /permission|licen[cs]e/i.test(quote) ? plainText(quote) : '';
};

/** The free licence a copying notice grants, by name; null when it names none known to be free. */
const licenceOf = (notice: string): string | null => {
  const later = /any later version/i.test(notice) ? ' or later' : '';
  const gpl = /GNU General Public License/i.test(notice);
  const fdl = /GNU Free Documentation License,?\s*(?:Version\s*)?(\d\.\d)?/i.exec(notice);
  if (fdl) {
    const name = `GNU Free Documentation License${fdl[1] ? ` ${fdl[1]}` : ''}${later}`;
    return gpl && /either|dual/i.test(notice) ? `${name}, or the GNU General Public License` : name;
  }
  // The permission notice GNU manuals had before the FDL; it may name the GPL of the program.
  if (
    /Permission is granted to make and distribute verbatim copies/i.test(notice) &&
    /Permission is granted to copy and distribute modified versions/i.test(notice)
  ) {
    return 'GNU permissive documentation licence';
  }
  if (gpl) {
    const version = /General Public License[^.]*?version (\d)/i.exec(notice)?.[1];
    return `GNU General Public License${version ? ` ${version}` : ''}${later}`;
  }
  const creativeCommons = /creativecommons\.org\/licenses\/([a-z-]+)\/(\d\.\d)/i.exec(notice);
  if (creativeCommons) {
    return `CC ${(creativeCommons[1] ?? '').toUpperCase()} ${creativeCommons[2] ?? ''}`;
  }
  return /Permission is hereby granted, free of charge/i.test(notice) ? 'MIT-style licence' : null;
};

const freeLicence = (top: Page) => {
  const licence = licenceOf(noticeOf(top.text));
  if (!licence) throw new Error("This manual's copying notice doesn't grant a free licence");
  return licence;
};

/** The Top node's first paragraph about the manual ("This manual is for GNU Gzip…"). */
const introOf = (html: string) => {
  const body = /<body[^>]*>([\s\S]*)/i.exec(html)?.[1] ?? html;
  for (const match of body.matchAll(/<p>([\s\S]*?)<\/p>/g)) {
    const text = plainText(match[1] ?? '');
    if (text.length > 40 && !/^(?:Copyright|Permission|Next:|Up:|Previous:)/i.test(text)) {
      return text.length > 600 ? `${text.slice(0, 597)}…` : text;
    }
  }
  return '';
};

/** The element that starts at `at`, with everything inside it. */
const elementFrom = (html: string, at: number) => {
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

type Entry = { file: string; anchor: string; title: string; index: boolean };

/**
 * The table of contents' entries in order: the links in the Top node's contents list (Texinfo 4
 * names only the top level's `toc_…` links), else its `toc-…` links.
 */
const tocEntries = (html: string, top: string): Entry[] => {
  const at = html.search(/<div class="contents"/);
  const contents = at >= 0 ? elementFrom(html, at) : html;
  const links = [...contents.matchAll(/<a\b([^>]*)>([\s\S]*?)<\/a>/g)].filter((match) =>
    at >= 0 ? true : /\b(?:id|name)="toc[-_]/.test(match[1] ?? ''),
  );
  return links.flatMap((match) => {
    const href = /\bhref="([^"]*)"/.exec(match[1] ?? '')?.[1];
    if (href === undefined) return [];
    const [file = '', anchor = ''] = decode(href).split('#');
    const title = plainText(match[2] ?? '');
    return [
      {
        file: file || top,
        anchor,
        title,
        // Index nodes list entries with links to elsewhere: nothing to read.
        index:
          /\brel="index"/.test(match[1] ?? '') ||
          (title.length < 50 && /(?:^|\s)Index$/i.test(title)),
      },
    ];
  });
};

const chapterList = async (manual: Manual): Promise<MangaChapter[]> => {
  const { top } = placeOf(manual);
  const node = await cachedPage(pageUrl(manual, top));
  freeLicence(node);
  const places = new Set<string>();
  const entries = tocEntries(node.text, top).filter(
    (entry) =>
      !entry.index &&
      PAGE.test(entry.file) &&
      ANCHOR.test(entry.anchor) &&
      entry.anchor !== 'Top' &&
      entry.anchor !== 'SEC_Contents' &&
      // A place listed twice (a chapter and its first section) is read once.
      !places.has(`${entry.file}#${entry.anchor}`) &&
      Boolean(places.add(`${entry.file}#${entry.anchor}`)),
  );
  const chapters: MangaChapter[] = [];
  const seen = new Set<string>();
  entries.forEach((entry, index) => {
    const next = entries[index + 1];
    // Within one page, a chapter ends where the page's next entry starts.
    const end = next && next.file === entry.file && next.anchor ? next.anchor : '';
    const id = `${manual.id}/${entry.file}#${entry.anchor}#${end}`;
    if (seen.has(id)) return;
    seen.add(id);
    chapters.push({ id, name: entry.title || entry.file, number: chapters.length + 1 });
  });
  if (chapters.length === 0) throw new Error('This manual has no table of contents to read by');
  return chapters;
};

/** Where the element carrying this anchor starts, or -1. */
const anchorAt = (html: string, anchor: string) => {
  const found = [` id="${anchor}"`, ` name="${anchor}"`]
    .map((attribute) => html.indexOf(attribute))
    .filter((at) => at >= 0);
  return found.length > 0 ? html.lastIndexOf('<', Math.min(...found)) : -1;
};

/** The node's text: no navigation, menus, contents lists or rules; examples stay as they are. */
const cleanNode = (html: string) =>
  dropElements(html.replace(/<!--[\s\S]*?-->/g, ''), (tag, attributes) =>
    classes(attributes).some((name) => DROPPED_CLASSES.has(name)),
  ).replace(/<hr\b[^>]*>/gi, '');

const passage = async (chapterId: string): Promise<Passage> => {
  const match = /^([^/#]+)\/([^/#]+)#([^#]*)#([^#]*)$/.exec(chapterId);
  const [, id = '', file = '', start = '', end = ''] = match ?? [];
  if (!match || !PAGE.test(file) || !ANCHOR.test(start) || !ANCHOR.test(end)) {
    throw new Error('Not a chapter of a GNU manual');
  }
  const manual = manualOf(id);
  const url = pageUrl(manual, file);
  const found = await cachedPage(url);
  const body = /<body[^>]*>([\s\S]*?)<\/body>/i.exec(found.text)?.[1] ?? found.text;
  const from = start ? Math.max(anchorAt(body, start), 0) : 0;
  const until = end ? anchorAt(body, end) : -1;
  const html = cleanNode(body.slice(from, until > from ? until : body.length));
  if (!plainText(html)) throw new Error('This part of the manual has no text');
  return { html, baseUrl: found.url || url };
};

const summary = (manual: Manual): MangaSummary => ({ id: manual.id, title: manual.title });

const pageOf = (manuals: Manual[], page: number): MangaPage => {
  const start = (Math.max(1, page) - 1) * PAGE_SIZE;
  return {
    items: manuals.slice(start, start + PAGE_SIZE).map(summary),
    hasNextPage: start + PAGE_SIZE < manuals.length,
  };
};

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

/** How well a manual matches every one of the words (0 when one is missing). */
const score = (manual: Manual, terms: string[]) => {
  const title = words(`${manual.title} ${manual.name}`);
  const about = words(`${manual.description} ${manual.category}`);
  let total = 0;
  for (const term of terms) {
    const found = (title.includes(term) ? 3 : 0) + (about.includes(term) ? 1 : 0);
    if (found === 0) return 0;
    total += found + (title.split(' ').includes(term) ? 1 : 0);
  }
  return total;
};

const search = (query: string, page: number, values: FilterValues | undefined): MangaPage => {
  const category = pick(values, 'category', CATEGORIES);
  const sort = pick(values, 'sort', SORTS);
  const terms = words(query).split(' ').filter(Boolean);
  const listed = MANUALS.filter(
    (manual) => !category || category === 'all' || manual.category === category,
  );
  const found = terms.length
    ? listed
        .map((manual, order) => ({ manual, order, score: score(manual, terms) }))
        .filter((match) => match.score > 0)
        .sort((first, second) => second.score - first.score || first.order - second.order)
        .map((match) => match.manual)
    : listed;
  const sorted =
    sort === 'title'
      ? [...found].sort((first, second) => first.title.localeCompare(second.title))
      : found;
  return pageOf(sorted, page);
};

const details = async (id: string): Promise<MangaDetails> => {
  const manual = manualOf(id);
  // The Top node says what the manual is for and how it may be shared; when www.gnu.org is slow,
  // the list's own description and licence answer.
  const top = await Promise.race([
    cachedPage(pageUrl(manual, placeOf(manual).top)).catch(() => null),
    raygun.time.sleep(DETAILS_WAIT_MS).then(() => null),
  ]);
  const licence = top ? freeLicence(top) : manual.licence;
  return {
    id: manual.id,
    title: manual.title,
    coverUrl: null,
    authors: [],
    description: [
      manual.description,
      top ? introOf(top.text) : '',
      `Licence: ${licence}, as the manual's copying notice states.`,
    ]
      .filter(Boolean)
      .join('\n\n'),
    genres: [manual.category],
    status: 'completed',
    url: `${SITE}${manual.path}`,
  };
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'category', label: 'Category', options: CATEGORIES },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'manuals',
    name: 'GNU Manuals',
    popular: async ({ page }) => pageOf(MANUALS, page),
    search: async ({ query, page, filters: values }) => search(query, page, values),
    filters: async () => filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapterList(manualOf(id)) }),
    passage: ({ chapterId }) => passage(chapterId),
  }),
);
