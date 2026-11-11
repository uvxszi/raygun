// Internet Archive books: the public-domain books the Internet Archive scanned with libraries
// (American, Canadian and European libraries, the Biodiversity Heritage Library, the Smithsonian…),
// read as the Archive's PDFs: the page scans with their recognised text, so the reader can search
// them and read them aloud. A book is listed only when it was published more than 95 years ago (so
// it is in the US public domain), the Archive marks it as not in copyright, it sits in one of the
// libraries' collections (which only the Archive adds to) and it isn't lent to signed-in readers.
// Every book is checked again when it opens, so one reached another way (an old library entry, a
// link) is held to the same rule. Each PDF in an item is a chapter: most items are one book, some
// hold several volumes.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type {
  FilterValues,
  MangaChapter,
  MangaDetails,
  MangaPage,
  Passage,
  SourceFilter,
} from '@raygun/sdk';

const API = 'https://archive.org';
const ROWS = 30;
/** Works published before this year are in the US public domain: 95 years have passed. */
const PUBLIC_DOMAIN_BEFORE = new Date().getUTCFullYear() - 95;
/** The libraries' scanning collections; only the Archive puts items in them. */
const LIBRARIES = [
  { value: 'americana', label: 'American libraries' },
  { value: 'toronto', label: 'Canadian libraries' },
  { value: 'europeanlibraries', label: 'European libraries' },
  { value: 'iacl', label: "Children's library" },
  { value: 'biodiversity', label: 'Biodiversity Heritage Library' },
  { value: 'smithsonian', label: 'Smithsonian Libraries' },
  { value: 'library_of_congress', label: 'Library of Congress' },
  { value: 'cdl', label: 'University of California' },
  { value: 'universallibrary', label: 'Universal Library' },
  { value: 'wellcomelibrary', label: 'Wellcome Library' },
];
const BASE_QUERY = [
  'mediatype:texts',
  'possible-copyright-status:"NOT_IN_COPYRIGHT"',
  `year:[1 TO ${PUBLIC_DOMAIN_BEFORE - 1}]`,
  'NOT access-restricted-item:true',
  'format:PDF',
  `collection:(${LIBRARIES.map((library) => library.value).join(' OR ')})`,
].join(' AND ');
/** The PDFs an item may have, best first: those with the scans' text before picture-only ones. */
const PDF_FORMATS = [
  'Text PDF',
  'Additional Text PDF',
  'PDF',
  'Grayscale PDF',
  'Image Container PDF',
];

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
  { value: 'ger', label: 'German' },
  { value: 'spa', label: 'Spanish' },
  { value: 'ita', label: 'Italian' },
  { value: 'lat', label: 'Latin' },
  { value: 'dut', label: 'Dutch' },
  { value: 'por', label: 'Portuguese' },
  { value: 'swe', label: 'Swedish' },
  { value: 'rus', label: 'Russian' },
];
/** The Archive writes languages as codes (in any case) or names. */
const LANGUAGE_NAMES: Record<string, string> = {
  ...Object.fromEntries(LANGUAGES.map((language) => [language.value, language.label])),
  fra: 'French',
  deu: 'German',
  nld: 'Dutch',
  grc: 'Ancient Greek',
  gre: 'Greek',
  heb: 'Hebrew',
  hin: 'Hindi',
  san: 'Sanskrit',
  ara: 'Arabic',
  chi: 'Chinese',
  jpn: 'Japanese',
  dan: 'Danish',
  nor: 'Norwegian',
  pol: 'Polish',
  wel: 'Welsh',
  gle: 'Irish',
};

type SearchDoc = { identifier: string; title?: string | string[]; creator?: string | string[] };
type SearchResponse = { response?: { numFound?: number; docs?: SearchDoc[] } };
type ItemFile = { name: string; format?: string; title?: string; size?: string };
type ItemMetadata = {
  files?: ItemFile[];
  is_dark?: boolean;
  metadata?: {
    title?: string | string[];
    creator?: string | string[];
    description?: string | string[];
    subject?: string | string[];
    date?: string;
    year?: string;
    volume?: string;
    publisher?: string | string[];
    contributor?: string | string[];
    language?: string | string[];
    imagecount?: string;
    collection?: string | string[];
    'possible-copyright-status'?: string;
    'access-restricted-item'?: string;
  };
};

// Lucene syntax characters would turn a query into a malformed or different search.
const escapeQuery = (query: string) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const strings = (value: string | string[] | undefined): string[] =>
  (Array.isArray(value) ? value : value ? [value] : []).map((text) => text.trim()).filter(Boolean);

/**
 * A creator as people read it: "Dostoyevsky, Fyodor, 1821-1881" is "Fyodor Dostoyevsky", while
 * "Marcus Aurelius, Emperor of Rome, 121-180" keeps its order.
 */
const person = (name: string) => {
  const parts = name
    .replace(/\s*\([^)]*\)/g, '')
    .split(/\s*,\s*/)
    .map((part) => part.trim())
    .filter(Boolean);
  // Dates ("1821-1881", "b. 1843?") end many names.
  while (parts.length > 1 && /\d/.test(parts[parts.length - 1]!)) parts.pop();
  const [last, first, ...rest] = parts;
  // "Last, First" when the surname is one word and the rest is a given name.
  if (rest.length === 0 && last && first && !/\s/.test(last) && /^[\p{L}.' -]+$/u.test(first)) {
    return `${first} ${last}`;
  }
  return parts.join(', ');
};

const creators = (value: string | string[] | undefined) => [
  ...new Set(
    strings(value)
      .filter((name) => !/^(https?:\/\/|www\.)/i.test(name))
      .map(person)
      .filter(Boolean),
  ),
];

const cover = (identifier: string) => `${API}/services/img/${encodeURIComponent(identifier)}`;

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');
const download = (identifier: string, file: string) =>
  `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file)}`;

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
    .replace(/[ \t]+\n/g, '\n')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

const languageName = (code: string) => LANGUAGE_NAMES[code.toLowerCase()] ?? code;

/** A page of books; without a `sort`, the best matches come first. */
const searchPage = async (query: string, sort: string | null, page: number): Promise<MangaPage> => {
  const at = Math.max(page, 1);
  const params = [
    `q=${encodeURIComponent(query)}`,
    'fl[]=identifier',
    'fl[]=title',
    // Who wrote each book comes in the same answer, for the cards.
    'fl[]=creator',
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
        authors: creators(doc.creator).slice(0, 6),
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
  {
    kind: 'select',
    key: 'library',
    label: 'Library',
    default: 'any',
    options: [{ value: 'any', label: 'Any' }, ...LIBRARIES],
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
  const library = pick(chosen, 'library', LIBRARIES);
  if (library) parts.push(`collection:(${library})`);
  // Words without a chosen order go by relevance; a bare browse by what's read most.
  const sort = pick(chosen, 'sort', SORTS) ?? (words ? null : SORTS[0]!.value);
  return searchPage(parts.join(' AND '), sort, page);
};

// Item metadata is read for details, chapters and passages; keep the last few.
const items = new Map<string, Promise<ItemMetadata>>();

const fetchItem = (identifier: string): Promise<ItemMetadata> => {
  let item = items.get(identifier);
  if (!item) {
    item = fetchJson<ItemMetadata>(`${API}/metadata/${encodeURIComponent(identifier)}`);
    item.catch(() => items.delete(identifier));
    items.set(identifier, item);
    if (items.size > 20) items.delete(items.keys().next().value!);
  }
  return item;
};

/** The year a book came out, from its year or its date ("1918", "[c1918]", "1918-01-01"). */
const yearOf = (meta: ItemMetadata['metadata']) => {
  const found = /\d{4}/.exec(meta?.year ?? meta?.date ?? '')?.[0];
  return found ? Number(found) : null;
};

/** Why a book may not be read here, or null when it may. */
const refusal = (item: ItemMetadata): string | null => {
  const meta = item.metadata;
  if (!meta || item.is_dark) return 'The Internet Archive has no such book';
  const status = (meta['possible-copyright-status'] ?? '').toLowerCase().replace(/[^a-z]/g, '');
  const year = yearOf(meta);
  const inLibrary = strings(meta.collection).some((collection) =>
    LIBRARIES.some((library) => library.value === collection),
  );
  if (status !== 'notincopyright' || year === null || year >= PUBLIC_DOMAIN_BEFORE || !inLibrary) {
    return "The Internet Archive doesn't list this book as public domain, so Raygun doesn't open it";
  }
  if (meta['access-restricted-item'] === 'true') {
    return 'The Internet Archive lends this book only to signed-in readers';
  }
  return null;
};

/** An item's metadata, for a book that is free to read only. */
const book = async (identifier: string): Promise<ItemMetadata> => {
  const item = await fetchItem(identifier);
  const refused = refusal(item);
  if (refused) throw new Error(refused);
  return item;
};

const details = async (id: string): Promise<MangaDetails> => {
  const item = await book(id);
  const meta = item.metadata!;
  const year = yearOf(meta);
  const description = strings(meta.description).map(plainText).join('\n\n');
  const volume = /^v(?:ol)?\.?\s*\d/i.test(meta.volume ?? '') ? meta.volume : null;
  const pages = Number.parseInt(meta.imagecount ?? '', 10);
  const facts = [
    year ? `Published ${year}` : '',
    strings(meta.publisher)[0] ?? '',
    volume ? `Volume ${volume.replace(/^v(?:ol)?\.?\s*/i, '')}` : '',
    pages > 0 ? `${pages} scanned pages` : '',
  ].filter(Boolean);
  const library = strings(meta.contributor)[0];
  const rights =
    'Public domain in the United States: published more than 95 years ago, and marked not in ' +
    `copyright by the Internet Archive.${library ? ` Scanned from a copy at ${library}.` : ''}`;
  return {
    id,
    title: strings(meta.title)[0] ?? id,
    coverUrl: cover(id),
    authors: creators(meta.creator),
    artists: [],
    description: [facts.join(' · '), description, rights].filter(Boolean).join('\n\n'),
    genres: [
      ...new Set([
        ...strings(meta.subject).flatMap((subject) => subject.split(/\s*;\s*/)),
        ...strings(meta.language).map(languageName),
      ]),
    ]
      .filter(Boolean)
      .slice(0, 12),
    status: 'completed',
    url: `${API}/details/${encodeURIComponent(id)}`,
  };
};

/** Names in natural order, "part2" before "part10" (QuickJS has no `Intl` collation). */
const naturalOrder = (a: string, b: string) => {
  const parts = (text: string) =>
    text
      .toLowerCase()
      .split(/(\d+)/)
      .map((part, index) => (index % 2 === 1 ? Number(part) : part));
  const [left, right] = [parts(a), parts(b)];
  for (let index = 0; index < Math.max(left.length, right.length); index += 1) {
    const [x, y] = [left[index], right[index]];
    if (x === undefined || y === undefined) return x === undefined ? -1 : 1;
    if (x === y) continue;
    if (typeof x === 'number' && typeof y === 'number') return x - y;
    return String(x) < String(y) ? -1 : 1;
  }
  return 0;
};

/** The item's PDFs of the best format it has: each one book or volume. */
const pdfs = (item: ItemMetadata) => {
  const files = (item.files ?? []).filter((file) => /\.pdf$/i.test(file.name));
  for (const format of PDF_FORMATS) {
    const found = files.filter((file) => file.format === format);
    if (found.length > 0) return found.sort((a, b) => naturalOrder(a.name, b.name));
  }
  return [];
};

/** A chapter id: the item and its PDF (identifiers never hold `|`). */
const chapterId = (identifier: string, file: string) => `${identifier}|${file}`;

const chapters = async (id: string): Promise<MangaChapter[]> => {
  const item = await book(id);
  const title = strings(item.metadata?.title)[0] ?? id;
  const found = pdfs(item);
  if (found.length === 0) throw new Error('The Internet Archive has no PDF of this book');
  return found.map((file, index) => ({
    id: chapterId(id, file.name),
    name:
      found.length === 1
        ? title
        : file.title?.trim() ||
          file.name
            .split('/')
            .pop()!
            .replace(/\.pdf$/i, '')
            .replace(/[_]+/g, ' ')
            .trim(),
    number: index + 1,
  }));
};

const passage = async (id: string): Promise<Passage> => {
  const cut = id.indexOf('|');
  const identifier = cut > 0 ? id.slice(0, cut) : id;
  const item = await book(identifier);
  const file = cut > 0 ? id.slice(cut + 1) : pdfs(item)[0]?.name;
  if (!file || !(item.files ?? []).some((each) => each.name === file)) {
    throw new Error('This PDF is no longer in the book');
  }
  return { pdf: download(identifier, file) };
};

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'books',
    name: 'Internet Archive',
    popular: ({ page }) => searchPage(BASE_QUERY, 'downloads desc', page),
    latest: ({ page }) => searchPage(BASE_QUERY, 'addeddate desc', page),
    search: ({ query, page, filters: chosen }) => search(query, page, chosen),
    filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    passage: ({ chapterId: id }) => passage(id),
  }),
);
