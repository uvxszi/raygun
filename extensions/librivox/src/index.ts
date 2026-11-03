// LibriVox audiobooks (M46.22): public-domain books read by volunteers. The Internet Archive's
// `librivoxaudio` collection is searched (free text, popular, newest, by subject); each book's
// LibriVox project number gives its chapter feed (`librivox.org/rss/<number>`), which Raygun reads
// like a podcast's. No key needed.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Audiobook, AudiobookPage } from '@raygun/sdk';

const SEARCH = 'https://archive.org/advancedsearch.php';
const PAGE_SIZE = 30;

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
    feedUrl: `https://librivox.org/rss/${number}`,
    runtime: doc.runtime ?? null,
    language: first(doc.language),
    description: plain(first(doc.description))?.slice(0, 600) ?? null,
  };
};

const find = async (query: string, sort: string, page: number): Promise<AudiobookPage> => {
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
  };
};

registerProvider(
  defineProvider({
    kind: 'audiobook.directory',
    id: 'books',
    name: 'LibriVox',
    async search({ query, page }) {
      const words = escape(query);
      if (!words) return find('mediatype:audio', 'downloads desc', page);
      return find(`(${words})`, 'downloads desc', page);
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
      return find(`subject:(${words})`, 'downloads desc', page);
    },
  }),
);
