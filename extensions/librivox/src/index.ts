// LibriVox audiobooks (M46.22): public-domain books read by volunteers. The Internet Archive's
// `librivoxaudio` collection lists them (popular, newest, by subject) and finds them by title and
// author, or by any word when neither matches. LibriVox's own catalogue API (M46.48) answers
// searches by title and author and lists its genres too, so a search still finds books while the
// Archive is slow or down. Each book's LibriVox project number gives its chapter feed
// (`librivox.org/rss/<number>`), which Raygun reads like a podcast's. No key needed.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Audiobook, AudiobookPage } from '@raygun/sdk';

const SEARCH = 'https://archive.org/advancedsearch.php';
const API = 'https://librivox.org/api/feed/audiobooks/';
const PAGE_SIZE = 30;
/** Books asked of LibriVox's API for a search's first page. */
const API_LIMIT = 20;

type Doc = {
  identifier: string;
  title?: string | string[];
  creator?: string | string[];
  call_number?: string | string[];
  runtime?: string;
  language?: string | string[];
  description?: string | string[];
};
type Answer = { response?: { numFound?: number; docs?: Doc[] } };

type ApiAuthor = { first_name?: string; last_name?: string };
type ApiBook = {
  id?: string;
  title?: string;
  description?: string;
  language?: string;
  totaltime?: string;
  authors?: ApiAuthor[];
  coverart_thumbnail?: string;
  coverart_jpg?: string;
};

const first = (value: string | string[] | undefined): string | null =>
  (Array.isArray(value) ? value[0] : value)?.trim() || null;

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:/\\&|]/g, ' ').trim();

const plain = (html: string | null) =>
  html
    ? html
        .replace(/<[^>]+>/g, ' ')
        .replace(/&amp;/g, '&')
        .replace(/&quot;/g, '"')
        .replace(/&#39;|&rsquo;/g, "'")
        .replace(/\s+/g, ' ')
        .trim()
    : null;

const feedOf = (number: string) => `https://librivox.org/rss/${number}`;

const toBook = (doc: Doc): Audiobook | null => {
  const number = first(doc.call_number);
  const title = first(doc.title);
  // Only LibriVox projects have a chapter feed.
  if (!number || !/^\d+$/.test(number) || !title) return null;
  return {
    id: doc.identifier,
    title,
    author: first(doc.creator),
    artworkUrl: `https://archive.org/services/img/${encodeURIComponent(doc.identifier)}`,
    feedUrl: feedOf(number),
    runtime: doc.runtime ?? null,
    language: first(doc.language),
    description: plain(first(doc.description))?.slice(0, 600) ?? null,
  };
};

const find = async (
  query: string,
  sort: string,
  page: number,
): Promise<AudiobookPage & { found: number }> => {
  const params = [
    `q=${encodeURIComponent(`collection:librivoxaudio AND ${query}`)}`,
    ...['identifier', 'title', 'creator', 'call_number', 'runtime', 'language', 'description'].map(
      (field) => `fl[]=${field}`,
    ),
    `sort[]=${encodeURIComponent(sort)}`,
    `rows=${PAGE_SIZE}`,
    `page=${Math.max(1, page)}`,
    'output=json',
  ];
  const answer = await fetchJson<Answer>(`${SEARCH}?${params.join('&')}`);
  const docs = answer.response?.docs ?? [];
  const found = answer.response?.numFound ?? 0;
  return {
    books: docs.map(toBook).filter((book): book is Audiobook => book !== null),
    hasMore: Math.max(1, page) * PAGE_SIZE < found,
    found,
  };
};

const fromApi = (book: ApiBook): Audiobook | null => {
  if (!book.id || !/^\d+$/.test(book.id) || !book.title?.trim()) return null;
  const authors = (book.authors ?? [])
    .map((author) => `${author.first_name ?? ''} ${author.last_name ?? ''}`.trim())
    .filter(Boolean);
  return {
    id: `librivox-${book.id}`,
    title: book.title.trim(),
    author: authors.join(', ') || null,
    artworkUrl: book.coverart_thumbnail || book.coverart_jpg || null,
    feedUrl: feedOf(book.id),
    runtime: book.totaltime ?? null,
    language: book.language ?? null,
    description: plain(book.description ?? null)?.slice(0, 600) ?? null,
  };
};

/** LibriVox's catalogue API; it answers 404 when nothing matches. */
const api = async (params: Record<string, string>, limit: number, offset = 0) => {
  const query = Object.entries({
    ...params,
    format: 'json',
    coverart: '1',
    limit: String(limit),
    offset: String(offset),
    fields: '{id,title,description,language,totaltime,authors,coverart_thumbnail,coverart_jpg}',
  })
    .map(([key, value]) => `${key}=${encodeURIComponent(value)}`)
    .join('&');
  const response = await raygun.http.fetch(`${API}?${query}`);
  if (response.status === 404) return [];
  if (!response.ok) throw new Error(`HTTP ${response.status} from LibriVox`);
  const answer = await response.json<{ books?: ApiBook[] }>();
  return (answer.books ?? []).map(fromApi).filter((book): book is Audiobook => book !== null);
};

/** LibriVox files titles without their first article ("Count of Monte Cristo"). */
const withoutArticle = (text: string) => text.replace(/^(the|a|an)\s+/i, '');

/** A search in LibriVox's API: titles starting with the words, and authors by surname. */
const apiSearch = async (query: string): Promise<Audiobook[]> => {
  const words = query.trim().toLowerCase().split(/\s+/).filter(Boolean);
  const surname = words[words.length - 1] ?? '';
  const [byTitle, byAuthor] = await Promise.allSettled([
    api({ title: `^${withoutArticle(query.trim())}` }, API_LIMIT),
    surname.length > 2 ? api({ author: surname }, API_LIMIT) : Promise.resolve([]),
  ]);
  if (byTitle.status === 'rejected' && byAuthor.status === 'rejected') throw byTitle.reason;
  // An author search lists everything by that surname: keep those naming every word.
  const authored = (byAuthor.status === 'fulfilled' ? byAuthor.value : []).filter((book) => {
    const text = `${book.author ?? ''} ${book.title}`.toLowerCase();
    return words.every((word) => text.includes(word));
  });
  return [...(byTitle.status === 'fulfilled' ? byTitle.value : []), ...authored];
};

/** Pages joined without repeating a book; the first page's order leads. */
const join = (pages: Audiobook[][]) => {
  const seen = new Set<string>();
  return pages.flat().filter((book) => {
    const key = book.feedUrl ?? book.id;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

/** LibriVox's own names for the genres Raygun asks for. */
const GENRES: Record<string, string> = {
  adventure: 'Action & Adventure Fiction',
  children: "Children's Fiction",
  'fairy tales': 'Myths, Legends & Fairy Tales',
  fantasy: 'Fantasy Fiction',
  'historical fiction': 'Historical Fiction',
  history: 'History',
  humor: 'Humorous Fiction',
  mystery: 'Crime & Mystery Fiction',
  literature: 'Literary Fiction',
  philosophy: 'Philosophy',
  poetry: 'Poetry',
  religion: 'Religion',
  romance: 'Romance',
  'science fiction': 'Science Fiction',
  'short stories': 'Short Stories',
  drama: 'Dramatic Readings',
  horror: 'Horror & Supernatural Fiction',
};

registerProvider(
  defineProvider({
    kind: 'audiobook.directory',
    id: 'books',
    name: 'LibriVox',
    async search({ query, page }) {
      const words = escape(query);
      if (!words) return find('mediatype:audio', 'downloads desc', page);
      // Titles and authors first, popular first; any word only when neither matches.
      const archive = find(`(title:(${words}) OR creator:(${words}))`, 'downloads desc', page).then(
        (found) => (found.found === 0 ? find(`(${words})`, 'downloads desc', page) : found),
      );
      const catalogue = page === 1 ? apiSearch(query) : Promise.resolve([]);
      const [fromArchive, fromCatalogue] = await Promise.allSettled([archive, catalogue]);
      if (fromArchive.status === 'rejected' && fromCatalogue.status === 'rejected') {
        throw fromArchive.reason;
      }
      const books = join([
        fromArchive.status === 'fulfilled' ? fromArchive.value.books : [],
        fromCatalogue.status === 'fulfilled' ? fromCatalogue.value : [],
      ]);
      return {
        books,
        hasMore: fromArchive.status === 'fulfilled' && fromArchive.value.hasMore,
      };
    },
    async popular({ page }) {
      return find('mediatype:audio', 'downloads desc', page);
    },
    async latest({ page }) {
      return find('mediatype:audio', 'publicdate desc', page);
    },
    async subject({ subject, page }) {
      const words = escape(subject);
      if (!words) return { books: [], hasMore: false };
      const genre = GENRES[subject.trim().toLowerCase()];
      const [fromArchive, fromCatalogue] = await Promise.allSettled([
        find(`subject:(${words})`, 'downloads desc', page),
        genre
          ? api({ genre }, PAGE_SIZE, (Math.max(1, page) - 1) * PAGE_SIZE)
          : Promise.resolve([]),
      ]);
      if (fromArchive.status === 'rejected' && fromCatalogue.status === 'rejected') {
        throw fromArchive.reason;
      }
      const catalogue = fromCatalogue.status === 'fulfilled' ? fromCatalogue.value : [];
      return {
        books: join([fromArchive.status === 'fulfilled' ? fromArchive.value.books : [], catalogue]),
        hasMore:
          (fromArchive.status === 'fulfilled' && fromArchive.value.hasMore) ||
          catalogue.length === PAGE_SIZE,
      };
    },
  }),
);
