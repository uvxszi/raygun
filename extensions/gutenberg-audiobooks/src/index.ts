// The Project Gutenberg Open Audiobook Collection: books from Project Gutenberg (public domain in
// the US) read by neural text-to-speech voices, made by Project Gutenberg, Microsoft and MIT and
// kept by the Internet Archive (collection `gutenberg-audiobooks`). Each book is one recording of
// the whole text, so a book's "chapters" are its MP3 files, read from the item's file list. Lists
// and searches come from the Archive's advanced-search API. No key needed.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { Audiobook, AudiobookChapter, AudiobookPage } from '@raygun/sdk';

const API = 'https://archive.org';
const COLLECTION = 'collection:gutenberg-audiobooks AND mediatype:audio';
const PAGE_SIZE = 30;

type Doc = {
  identifier: string;
  title?: string | string[];
  creator?: string | string[];
  language?: string | string[];
};
type Answer = { response?: { numFound?: number; docs?: Doc[] } };

type ItemFile = { name: string; format?: string; length?: string; title?: string };
type Item = {
  files?: ItemFile[];
  metadata?: {
    title?: string | string[];
    creator?: string | string[];
    language?: string | string[];
    source?: string;
    collection?: string | string[];
  };
};

const first = (value: string | string[] | undefined): string | null =>
  (Array.isArray(value) ? value[0] : value)?.trim() || null;

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:/\\&|]/g, ' ').trim();

/** The title's first line: subtitles follow on the next ones. */
const titleOf = (value: string | string[] | undefined) =>
  first(value)?.split('\n')[0]?.trim() || null;

/**
 * Project Gutenberg's catalogue names people "Surname, Given names, years"; Raygun shows
 * "Given names Surname". Other names ("Various", "Anonymous") stay as they are.
 */
export const authorOf = (value: string | string[] | undefined) => {
  const names = (Array.isArray(value) ? value : value ? [value] : [])
    .map((name) => {
      const parts = name.split(',').map((part) => part.trim());
      const [surname, given] = parts;
      if (!surname) return '';
      if (!given || /^[\d?\s-]+(BCE?|AD)?$/i.test(given)) return surname;
      return `${given.replace(/\s*\([^)]*\)/g, '')} ${surname}`.trim();
    })
    .filter(Boolean);
  return names.length > 0 ? names.join(', ') : null;
};

const parseLength = (length: string | undefined) => {
  if (!length) return null;
  const seconds = length
    .split(':')
    .reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

const artworkOf = (identifier: string) => `${API}/services/img/${encodeURIComponent(identifier)}`;

const toBook = (doc: Doc): Audiobook | null => {
  const title = titleOf(doc.title);
  if (!title) return null;
  return {
    id: doc.identifier,
    title,
    author: authorOf(doc.creator),
    artworkUrl: artworkOf(doc.identifier),
    language: first(doc.language),
  };
};

const find = async (
  q: string,
  sort: string,
  page: number,
): Promise<AudiobookPage & { found: number }> => {
  const params = query({
    q: `${COLLECTION} AND ${q}`,
    'fl[]': ['identifier', 'title', 'creator', 'language'],
    'sort[]': sort,
    rows: PAGE_SIZE,
    page: Math.max(1, page),
    output: 'json',
  });
  const answer = await fetchJson<Answer>(`${API}/advancedsearch.php?${params}`);
  const found = answer.response?.numFound ?? 0;
  return {
    books: (answer.response?.docs ?? [])
      .map(toBook)
      .filter((book): book is Audiobook => book !== null),
    hasMore: Math.max(1, page) * PAGE_SIZE < found,
    found,
  };
};

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

registerProvider(
  defineProvider({
    kind: 'audiobook.directory',
    id: 'books',
    name: 'Gutenberg open audiobooks',

    async search({ query: text, page }) {
      const words = escape(text);
      if (!words) return find('identifier:*', 'downloads desc', page);
      // Titles and authors first; any word only when neither matches.
      const found = await find(`(title:(${words}) OR creator:(${words}))`, 'downloads desc', page);
      return found.found === 0 && page <= 1 ? find(`(${words})`, 'downloads desc', page) : found;
    },

    async popular({ page }) {
      return find('identifier:*', 'downloads desc', page);
    },

    async chapters({ id }) {
      const item = await fetchJson<Item>(`${API}/metadata/${encodeURIComponent(id)}`);
      const collections = [item.metadata?.collection ?? []].flat();
      if (!collections.includes('gutenberg-audiobooks')) {
        throw new Error('This book is no longer in the Project Gutenberg audiobook collection');
      }
      const title = titleOf(item.metadata?.title) ?? id;
      const files = (item.files ?? [])
        .filter((file) => file.format === 'VBR MP3' || file.format === 'MP3')
        .sort((a, b) => a.name.localeCompare(b.name));
      const chapters: AudiobookChapter[] = files.map((file, index) => ({
        id: file.name,
        title: files.length === 1 ? title : file.title?.trim() || `Part ${index + 1}`,
        audioUrl: `${API}/download/${encodeURIComponent(id)}/${encodePath(file.name)}`,
        audioType: 'audio/mpeg',
        durationMs: parseLength(file.length),
      }));
      const source = item.metadata?.source;
      return {
        title,
        author: authorOf(item.metadata?.creator),
        artworkUrl: artworkOf(id),
        description: source
          ? `Read by a neural voice from Project Gutenberg's text (${source}).`
          : "Read by a neural voice from Project Gutenberg's text.",
        link: `${API}/details/${encodeURIComponent(id)}`,
        chapters,
      };
    },
  }),
);
