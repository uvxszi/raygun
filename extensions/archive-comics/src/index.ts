// Internet Archive comics: the part of the Archive's comics collection that is in the public domain
// by age, published more than 95 years ago (US law), so the scans are free to read wherever they
// came from. Uploaders date and mark their own items, so an item must also carry a public-domain
// mark or sit in one of the collection's curated sets rather than its open upload inbox; that
// keeps out recent comics given an old date. Only items with page images are listed. Each book in
// an item is a chapter, read through the Archive's BookReader at a size that suits a phone.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type {
  FilterValues,
  MangaChapter,
  MangaDetails,
  MangaPage,
  SourceFilter,
} from '@raygun/sdk';

const API = 'https://archive.org';
const ROWS = 30;
/** Works published before this year are in the US public domain: 95 years have passed. */
const PUBLIC_DOMAIN_BEFORE = new Date().getUTCFullYear() - 95;
const SCAN_FORMAT = 'Single Page Processed JP2 ZIP';
const BASE_QUERY = [
  'mediatype:texts',
  'collection:comics',
  `date:[1000-01-01 TO ${PUBLIC_DOMAIN_BEFORE - 1}-12-31]`,
  `format:"${SCAN_FORMAT}"`,
  '(licenseurl:*publicdomain* OR licenseurl:*zero* OR (collection:comics AND -collection:comics_inbox))',
].join(' AND ');
/** Page pictures are asked for about this wide (the scans are often 5,000 pixels). */
const PAGE_WIDTH = 1200;
const LARGE_PAGE_WIDTH = 2400;

type SearchDoc = { identifier: string; title?: string | string[] };
type SearchResponse = { response?: { numFound?: number; docs?: SearchDoc[] } };
type ItemFile = { name: string; format?: string; title?: string };
type ItemMetadata = {
  server?: string;
  dir?: string;
  files?: ItemFile[];
  is_dark?: boolean;
  metadata?: {
    title?: string | string[];
    creator?: string | string[];
    description?: string | string[];
    subject?: string | string[];
    date?: string;
    language?: string | string[];
  };
};

const SORTS = [
  { value: 'downloads desc', label: 'Most read' },
  { value: 'addeddate desc', label: 'Recently added' },
  { value: 'date asc', label: 'Oldest first' },
  { value: 'date desc', label: 'Newest first' },
  { value: 'titleSorter asc', label: 'Title' },
];
const LANGUAGES = [
  { value: 'eng', label: 'English' },
  { value: 'fre', label: 'French' },
  { value: 'ita', label: 'Italian' },
  { value: 'ger', label: 'German' },
  { value: 'spa', label: 'Spanish' },
  { value: 'dut', label: 'Dutch' },
  { value: 'por', label: 'Portuguese' },
];
/** The Archive writes languages as codes or names. */
const LANGUAGE_NAMES: Record<string, string> = {
  eng: 'English',
  fre: 'French',
  ita: 'Italian',
  ger: 'German',
  spa: 'Spanish',
  dut: 'Dutch',
  por: 'Portuguese',
};

// Lucene syntax characters would turn a query into a malformed or different search.
const escapeQuery = (query: string) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const strings = (value: string | string[] | undefined): string[] =>
  (Array.isArray(value) ? value : value ? [value] : []).map((text) => text.trim()).filter(Boolean);

const cover = (identifier: string) => `${API}/services/img/${encodeURIComponent(identifier)}`;

/** Descriptions are HTML; the app shows plain text. */
const plainText = (html: string) =>
  html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/p>/gi, '\n\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

/** A page of items; without a `sort`, the best matches come first. */
const searchPage = async (query: string, sort: string | null, page: number): Promise<MangaPage> => {
  const at = Math.max(page, 1);
  const params = [
    `q=${encodeURIComponent(query)}`,
    'fl[]=identifier',
    'fl[]=title',
    `rows=${ROWS}`,
    `page=${at}`,
    ...(sort ? [`sort[]=${encodeURIComponent(sort)}`] : []),
    'output=json',
  ];
  const result = await fetchJson<SearchResponse>(`${API}/advancedsearch.php?${params.join('&')}`);
  const docs = result.response?.docs ?? [];
  return {
    items: docs
      .filter((doc) => doc.identifier)
      .map((doc) => ({
        id: doc.identifier,
        title: strings(doc.title)[0] ?? doc.identifier,
        coverUrl: cover(doc.identifier),
      })),
    hasNextPage: at * ROWS < (result.response?.numFound ?? 0),
  };
};

const filters = async (): Promise<SourceFilter[]> => [
  { kind: 'select', key: 'sort', label: 'Sort by', default: SORTS[0]!.value, options: SORTS },
  {
    kind: 'select',
    key: 'language',
    label: 'Language',
    default: 'any',
    options: [{ value: 'any', label: 'Any' }, ...LANGUAGES],
  },
];

const pick = (chosen: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = chosen?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

const search = (query: string, page: number, chosen?: FilterValues) => {
  const words = escapeQuery(query);
  const parts = [`(${BASE_QUERY})`];
  if (words) parts.push(`(title:(${words}) OR creator:(${words}) OR subject:(${words}))`);
  const language = pick(chosen, 'language', LANGUAGES);
  if (language) parts.push(`language:(${language} OR "${LANGUAGE_NAMES[language]}")`);
  // Words without a chosen order go by relevance; a bare browse by what's read most.
  const sort = pick(chosen, 'sort', SORTS) ?? (words ? null : SORTS[0]!.value);
  return searchPage(parts.join(' AND '), sort, page);
};

// Item metadata is read for details, chapters and pages; keep the last few.
const items = new Map<string, Promise<ItemMetadata>>();

const metadata = (identifier: string): Promise<ItemMetadata> => {
  let item = items.get(identifier);
  if (!item) {
    item = fetchJson<ItemMetadata>(`${API}/metadata/${encodeURIComponent(identifier)}`);
    item.catch(() => items.delete(identifier));
    items.set(identifier, item);
    if (items.size > 20) items.delete(items.keys().next().value!);
  }
  return item;
};

const details = async (id: string): Promise<MangaDetails> => {
  const item = await metadata(id);
  const meta = item.metadata;
  if (!meta) throw new Error('The Internet Archive has no such item');
  const date = meta.date?.slice(0, 10);
  const description = strings(meta.description).map(plainText).join('\n\n');
  const languages = strings(meta.language).map((code) => LANGUAGE_NAMES[code] ?? code);
  return {
    id,
    title: strings(meta.title)[0] ?? id,
    coverUrl: cover(id),
    authors: strings(meta.creator),
    artists: [],
    description:
      [date ? `Published ${date}.` : '', description].filter(Boolean).join('\n\n') || null,
    genres: [...new Set([...strings(meta.subject), ...languages])].slice(0, 12),
    status: 'completed',
    url: `${API}/details/${encodeURIComponent(id)}`,
  };
};

/** Each book of an item: its page scans, `<book>_jp2.zip`. */
const books = (item: ItemMetadata) =>
  (item.files ?? [])
    .filter((file) => file.format === SCAN_FORMAT && file.name.endsWith('_jp2.zip'))
    .map((file) => ({ prefix: file.name.slice(0, -'_jp2.zip'.length), title: file.title }))
    .sort((a, b) => a.prefix.localeCompare(b.prefix, 'en', { numeric: true }));

/** A chapter id: the item and the book in it (identifiers never hold `|`). */
const chapterId = (identifier: string, prefix: string) => `${identifier}|${prefix}`;

const chapters = async (id: string): Promise<MangaChapter[]> => {
  const item = await metadata(id);
  const title = strings(item.metadata?.title)[0] ?? id;
  const published = item.metadata?.date ? Date.parse(item.metadata.date.slice(0, 10)) : NaN;
  const found = books(item);
  return found
    .map((book, index) => ({
      id: chapterId(id, book.prefix),
      name:
        found.length === 1
          ? title
          : book.title?.trim() || book.prefix.split('/').pop()!.replace(/[_]+/g, ' ').trim(),
      number: index + 1,
      dateUpload: Number.isFinite(published) ? published : null,
    }))
    .reverse();
};

/** `BookReaderJSIA.php`: the book's access and its pages, as spreads of one or two. */
type BookReader = {
  data?: {
    data?: { isRestricted?: boolean };
    brOptions?: { data?: { uri?: string; width?: number }[][] };
  };
};

/** The BookReader's scale is a power of two that divides the scan's size. */
const scaleFor = (width: number | undefined, target: number) => {
  if (!width || width <= target) return 1;
  return 2 ** Math.min(5, Math.floor(Math.log2(width / target)));
};

const pages = async (id: string) => {
  const cut = id.indexOf('|');
  const identifier = cut > 0 ? id.slice(0, cut) : id;
  const item = await metadata(identifier);
  const prefix = cut > 0 ? id.slice(cut + 1) : books(item)[0]?.prefix;
  if (!item.server || !item.dir || !prefix) {
    throw new Error('The Internet Archive has no pages for this item');
  }
  const params = [
    `id=${encodeURIComponent(identifier)}`,
    `itemPath=${encodeURIComponent(item.dir)}`,
    `server=${encodeURIComponent(item.server)}`,
    'format=jsonp',
    `subPrefix=${encodeURIComponent(prefix)}`,
  ];
  const reader = await fetchJson<BookReader>(
    `https://${item.server}/BookReader/BookReaderJSIA.php?${params.join('&')}`,
  );
  if (reader.data?.data?.isRestricted) {
    throw new Error('The Internet Archive lends this item only to signed-in readers');
  }
  const target =
    raygun.settings.get<boolean>('largePages') === true ? LARGE_PAGE_WIDTH : PAGE_WIDTH;
  const urls = (reader.data?.brOptions?.data ?? [])
    .flat()
    .filter((page) => page.uri?.startsWith('https://'))
    .map((page) => `${page.uri}&scale=${scaleFor(page.width, target)}&rotate=0`);
  if (urls.length === 0) throw new Error('The Internet Archive has no pages for this book');
  return urls;
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'comics',
    name: 'Internet Archive',
    popular: ({ page }) => searchPage(BASE_QUERY, 'downloads desc', page),
    latest: ({ page }) => searchPage(BASE_QUERY, 'addeddate desc', page),
    search: ({ query, page, filters: chosen }) => search(query, page, chosen),
    filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    pages: async ({ chapterId: id }) => ({ pages: await pages(id) }),
  }),
);
