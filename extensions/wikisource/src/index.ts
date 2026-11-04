// Wikisource: public-domain and Creative Commons texts from English Wikisource, through the
// MediaWiki API. Lists are its featured texts and the literature most recently proofread twice
// ("validated"); search finds works, and the chapters of works stand for their work. Every work
// listed carries a public-domain or Creative Commons licence category; versions and
// disambiguation pages are left out. A work's chapters are the subpages its main page links to,
// in that order, one level further for works in volumes or books; a work without subpages is one
// chapter.
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

const WIKI = 'https://en.wikisource.org';
const API = `${WIKI}/w/api.php`;
const PAGE_SIZE = 30;
const AGENT = 'Raygun/1.0 (https://github.com/uvxszi/raygun)';
// Wikimedia asks scripts to name themselves; `Api-User-Agent` is the header for clients whose
// User-Agent is set for them.
const HEADERS = { 'User-Agent': AGENT, 'Api-User-Agent': AGENT };
/** Volumes or books whose own pages list their chapters, read one level down. */
const MAX_CONTAINERS = 24;
/** A main page with this much text of its own (not just a title page) is a chapter too. */
const MAIN_TEXT = 5000;
/** Versions pages list editions, disambiguation pages list works: neither is one to read. */
const NOT_WORKS = '-incategory:"Versions_pages" -incategory:"Disambiguation_pages"';
/** The literature the latest list draws from (legislation and reports are validated too). */
const LITERATURE = [
  'Novels',
  'Short_stories',
  'Collections_of_short_stories',
  'Poems',
  'Collections_of_poetry',
  'Plays',
  "Children's_books",
  'Fairy_tales',
  'Essays',
].join('|');

const GENRES = [
  { value: 'all', label: 'All' },
  { value: 'Novels', label: 'Novels' },
  { value: 'Short stories', label: 'Short stories' },
  { value: 'Collections of short stories', label: 'Story collections' },
  { value: 'Poems', label: 'Poems' },
  { value: 'Collections of poetry', label: 'Poetry collections' },
  { value: 'Plays', label: 'Plays' },
  { value: "Children's books", label: 'Children’s books' },
  { value: 'Fairy tales', label: 'Fairy tales' },
  { value: 'Adventure novels', label: 'Adventure' },
  { value: 'Mystery', label: 'Mystery' },
  { value: 'Ghost stories', label: 'Ghost stories' },
  { value: 'Science fiction short stories', label: 'Science fiction' },
  { value: 'Romance novels', label: 'Romance' },
  { value: 'Essays', label: 'Essays' },
  { value: 'Speeches', label: 'Speeches' },
  { value: 'Biographies', label: 'Biographies' },
  { value: 'Travel', label: 'Travel' },
  { value: 'Philosophy', label: 'Philosophy' },
];
const SORTS = [
  { value: 'relevance', label: 'Best match' },
  { value: 'incoming_links_desc', label: 'Most linked' },
  { value: 'create_timestamp_desc', label: 'Newest' },
  { value: 'last_edit_desc', label: 'Recently edited' },
];

type Category = { title?: string; category?: string; hidden?: boolean };
type WorkPage = {
  title: string;
  missing?: boolean;
  categories?: Category[];
  pageprops?: Record<string, string>;
  thumbnail?: { source?: string };
};
type QueryAnswer = {
  continue?: Record<string, string>;
  query?: {
    pages?: WorkPage[];
    redirects?: { from: string; to: string }[];
    search?: { title: string }[];
    searchinfo?: { totalhits?: number };
    allpages?: { title: string }[];
  };
};
type ParseAnswer = { parse?: { title: string; text: string; categories?: Category[] } };
type Parsed = { title: string; html: string; categories: Category[] };

const api = async <T>(params: Record<string, string | number>): Promise<T> => {
  const query = Object.entries({ format: 'json', formatversion: 2, ...params })
    .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
    .join('&');
  const response = await raygun.http.fetch(`${API}?${query}`, { headers: HEADERS });
  if (!response.ok) throw new Error(`Wikisource answered HTTP ${response.status}`);
  const answer = await response.json<T & { error?: { info?: string } }>();
  if (answer.error) throw new Error(`Wikisource: ${answer.error.info ?? 'the request failed'}`);
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

/** Zero-width spaces and word joiners, which page numbers and templates leave behind. */
const INVISIBLE = new RegExp(`[${String.fromCharCode(0x200b, 0x2060)}]`, 'g');

/** Text without markup; inline tags join up with the words around them, blocks don't. */
const plainText = (html: string) =>
  decode(
    html
      .replace(/<style[\s\S]*?<\/style>/g, '')
      .replace(/<br\s*\/?>|<\/(?:p|div|li|dd|dt|td|th|h\d)>/gi, ' ')
      .replace(/<[^>]+>/g, ''),
  )
    .replace(INVISIBLE, '')
    .replace(/\s+/g, ' ')
    .trim();

const categoryName = (category: Category) =>
  (category.category ?? category.title?.replace(/^Category:/, '') ?? '').replace(/_/g, ' ');

/** Public domain (PD-old, PD-US, PD-USGov…) or Creative Commons, as licence templates file it. */
const isOpenLicence = (name: string) => /^(PD-|CC[- ]|CC0|Public domain)/i.test(name);

/** Categories that say how a page is kept or licensed rather than what the work is. */
const isGenre = (name: string) =>
  !isOpenLicence(name) &&
  !/^(Validated|Proofread|Featured|Problematic|Not proofread|Spoken works|Versions pages|Works |Index |Pages |Texts |Wikisource|Novels adapted|Newbery|Author|Translations)/i.test(
    name,
  ) &&
  !/\bworks$|\d{3,4}s?\b/i.test(name);

/** A subpage ("Work/Chapter 1") stands for its work. */
const workOf = (title: string) => title.split('/')[0] ?? title;

const pageUrl = (title: string) =>
  `${WIKI}/wiki/${title.replace(/ /g, '_').split('/').map(encodeURIComponent).join('/')}`;

const cleanImage = (url: string | undefined) =>
  url ? (url.startsWith('//') ? `https:${url}` : url).replace(/\?utm_[^#]*$/, '') : null;

/**
 * The works among these titles in the same order, with covers: pages that exist, carry an open
 * licence and are no versions or disambiguation page.
 */
const works = async (titles: string[]): Promise<MangaSummary[]> => {
  if (titles.length === 0) return [];
  const pages = new Map<string, WorkPage>();
  const renamed = new Map<string, string>();
  let next: Record<string, string> = {};
  // Categories come in shared batches of 500; a long list can need another round.
  for (let round = 0; round < 3; round += 1) {
    const answer = await api<QueryAnswer>({
      action: 'query',
      titles: titles.join('|'),
      prop: 'categories|pageprops|pageimages',
      cllimit: 'max',
      clprop: 'hidden',
      ppprop: 'disambiguation',
      piprop: 'thumbnail',
      pithumbsize: 400,
      redirects: 1,
      ...next,
    });
    for (const redirect of answer.query?.redirects ?? []) renamed.set(redirect.from, redirect.to);
    for (const page of answer.query?.pages ?? []) {
      const known = pages.get(page.title);
      pages.set(page.title, {
        ...known,
        ...page,
        categories: [...(known?.categories ?? []), ...(page.categories ?? [])],
      });
    }
    if (!answer.continue?.clcontinue) break;
    next = { clcontinue: answer.continue.clcontinue };
  }
  const seen = new Set<string>();
  return titles.flatMap((asked) => {
    const title = renamed.get(asked) ?? asked;
    const page = pages.get(title);
    if (!page || page.missing || seen.has(title)) return [];
    seen.add(title);
    const names = (page.categories ?? []).map(categoryName);
    const listable =
      names.some(isOpenLicence) &&
      !names.includes('Versions pages') &&
      !('disambiguation' in (page.pageprops ?? {}));
    return listable ? [{ id: title, title, coverUrl: cleanImage(page.thumbnail?.source) }] : [];
  });
};

/** A page of works from a Wikisource search, offset by page. */
const searchWorks = async (search: string, sort: string, page: number): Promise<MangaPage> => {
  const offset = (Math.max(1, page) - 1) * PAGE_SIZE;
  const answer = await api<QueryAnswer>({
    action: 'query',
    list: 'search',
    srnamespace: 0,
    srlimit: PAGE_SIZE,
    sroffset: offset,
    srsearch: search,
    srsort: sort,
    srprop: '',
    srinfo: 'totalhits',
  });
  const hits = answer.query?.search ?? [];
  const titles = [...new Set(hits.map((hit) => workOf(hit.title)))];
  return {
    items: await works(titles),
    hasNextPage:
      hits.length > 0 && offset + hits.length < (answer.query?.searchinfo?.totalhits ?? 0),
  };
};

// The pages read last stay in memory: details, chapters and the first chapter share the main page.
const parsedPages = new Map<string, Promise<Parsed>>();
const parsed = (title: string): Promise<Parsed> => {
  let page = parsedPages.get(title);
  if (!page) {
    page = api<ParseAnswer>({
      action: 'parse',
      page: title,
      prop: 'text|categories',
      redirects: 1,
      disableeditsection: 1,
      disablelimitreport: 1,
      disabletoc: 1,
    }).then((answer) => {
      if (!answer.parse) throw new Error('Wikisource has no page by this name');
      return {
        title: answer.parse.title,
        html: answer.parse.text,
        categories: answer.parse.categories ?? [],
      };
    });
    page.catch(() => parsedPages.delete(title));
    parsedPages.set(title, page);
    if (parsedPages.size > 12) parsedPages.delete(parsedPages.keys().next().value!);
  }
  return page;
};

/** The page's own text: after the header template, before the licence box. */
const bodyOf = (html: string) => {
  const data = html.indexOf('id="ws-data"');
  let start = 0;
  if (data >= 0) {
    const end = html.indexOf('</div></div>', data);
    start = end >= 0 ? end + '</div></div>'.length : data;
  }
  const licence = html.indexOf('<div class="licenseContainer', start);
  return html.slice(start, licence >= 0 ? licence : html.length);
};

/** The header's machine-readable fields (`ws-title`, `ws-author`, `ws-year`…). */
const headerField = (html: string, field: string) => {
  const value = new RegExp(`<span id="ws-${field}">([\\s\\S]*?)</span>`).exec(html)?.[1];
  return value ? plainText(value) || null : null;
};

type Link = { title: string; label: string };

/** Links to the subpages of `parent`, in the order the page gives them, each once. */
const subpageLinks = (html: string, parent: string): Link[] => {
  const prefix = `${parent}/`;
  const seen = new Set<string>();
  const links: Link[] = [];
  for (const match of html.matchAll(
    /<a href="\/wiki\/[^"]*"[^>]*?\btitle="([^"]+)"[^>]*>([\s\S]*?)<\/a>/g,
  )) {
    const title = decode(match[1] ?? '');
    if (!title.startsWith(prefix) || seen.has(title)) continue;
    seen.add(title);
    links.push({ title, label: plainText(match[2] ?? '') || title.slice(prefix.length) });
  }
  return links;
};

/** Every subpage title under `root`, at any depth (up to a few thousand). */
const allSubpages = async (root: string): Promise<string[]> => {
  const titles: string[] = [];
  let next: Record<string, string> = {};
  for (let round = 0; round < 4; round += 1) {
    const answer = await api<QueryAnswer>({
      action: 'query',
      list: 'allpages',
      apnamespace: 0,
      apprefix: `${root}/`,
      aplimit: 'max',
      ...next,
    });
    titles.push(...(answer.query?.allpages ?? []).map((page) => page.title));
    if (!answer.continue?.apcontinue) break;
    next = { apcontinue: answer.continue.apcontinue };
  }
  return titles;
};

/** Runs `task` over `items`, `width` at a time, keeping their order. */
const inTurns = async <T, R>(items: T[], width: number, task: (item: T) => Promise<R>) => {
  const results: R[] = [];
  for (let start = 0; start < items.length; start += width) {
    results.push(...(await Promise.all(items.slice(start, start + width).map(task))));
  }
  return results;
};

const chapterList = async (id: string): Promise<MangaChapter[]> => {
  const main = await parsed(id);
  const body = bodyOf(main.html);
  const top = subpageLinks(body, main.title);
  if (top.length === 0) return [{ id: main.title, name: 'Full text', number: 1 }];
  const all = await allSubpages(main.title).catch(() => [] as string[]);
  const containers = top
    .filter((link) => all.some((title) => title.startsWith(`${link.title}/`)))
    .slice(0, MAX_CONTAINERS);
  const inner = new Map<string, Link[]>(
    await inTurns(containers, 4, async (link): Promise<[string, Link[]]> => {
      const page = await parsed(link.title).catch(() => null);
      return [link.title, page ? subpageLinks(bodyOf(page.html), link.title) : []];
    }),
  );
  const listed: Link[] = [];
  // Text on the main page itself (a prologue, or a work with notes as subpages) comes first.
  const ownText = plainText(body.replace(/<a\b[\s\S]*?<\/a>/g, '')).length;
  if (ownText > MAIN_TEXT)
    listed.push({ title: main.title, label: headerField(main.html, 'title') ?? main.title });
  for (const link of top) {
    const children = inner.get(link.title) ?? [];
    if (children.length === 0) listed.push(link);
    for (const child of children)
      listed.push({ title: child.title, label: `${link.label} · ${child.label}` });
  }
  const seen = new Set<string>();
  return listed
    .filter((link) => !seen.has(link.title) && seen.add(link.title))
    .map((link, index) => ({ id: link.title, name: link.label, number: index + 1 }));
};

const details = async (id: string): Promise<MangaDetails> => {
  const [page, listed] = await Promise.all([parsed(id), works([id]).catch(() => [])]);
  const html = page.html;
  const notes = /<div class="wst-header-content[^"]*"[^>]*>([\s\S]*?)<\/div>/.exec(html)?.[1];
  const about = notes
    ? plainText(notes).replace(
        /\s*one or more chapters are available in a spoken word format\.?/i,
        '',
      )
    : '';
  const year = headerField(html, 'year');
  const translator = headerField(html, 'translator');
  const description = [
    about && about.charAt(0).toUpperCase() + about.slice(1),
    [year && `Published ${year}.`, translator && `Translated by ${translator}.`]
      .filter(Boolean)
      .join(' '),
  ]
    .filter(Boolean)
    .join('\n\n');
  const author = headerField(html, 'author');
  return {
    id: page.title,
    title: headerField(html, 'title') ?? page.title,
    coverUrl: listed[0]?.coverUrl ?? null,
    authors: author ? author.split(/\s*(?:,|;| and )\s*/).filter(Boolean) : [],
    description: description || null,
    genres: page.categories
      .filter((category) => !category.hidden)
      .map(categoryName)
      .filter(isGenre)
      .slice(0, 8),
    status: 'completed',
    url: pageUrl(page.title),
  };
};

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'genre', label: 'Genre', options: GENRES },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'texts',
    name: 'Wikisource',
    popular: ({ page }) =>
      searchWorks(`incategory:"Featured_texts" ${NOT_WORKS}`, 'incoming_links_desc', page),
    latest: ({ page }) =>
      searchWorks(
        `incategory:"Validated_texts" incategory:"${LITERATURE}" ${NOT_WORKS}`,
        'create_timestamp_desc',
        page,
      ),
    search: ({ query, page, filters: values }) => {
      const words = query.trim();
      const genre = pick(values, 'genre', GENRES);
      const search = [
        words,
        genre && genre !== 'all' && `incategory:"${genre.replace(/ /g, '_')}"`,
        NOT_WORKS,
      ]
        .filter(Boolean)
        .join(' ');
      const sort = pick(values, 'sort', SORTS) ?? (words ? 'relevance' : 'incoming_links_desc');
      return searchWorks(search, sort, page);
    },
    filters: async () => filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapterList(id) }),
    passage: async ({ chapterId }): Promise<Passage> => {
      const page = await parsed(chapterId);
      // Page-scan numbers would break paragraphs apart.
      const html = bodyOf(page.html).replace(
        /<span class="pagenum[^"]*"[^>]*>(?:<span[^>]*>[^<]*<\/span>)?<\/span>/g,
        '',
      );
      return { html, baseUrl: pageUrl(page.title) };
    },
  }),
);
