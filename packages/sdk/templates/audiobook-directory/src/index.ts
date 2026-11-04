// __NAME__: an audiobook directory (`audiobook.directory`) of LibriVox's public-domain recordings
// (https://librivox.org), found through the Internet Archive, which keeps them in its
// `librivoxaudio` collection. Each book's LibriVox project number gives its chapter feed
// (`librivox.org/rss/<number>`), which Raygun reads itself like a podcast's, so the feed's host
// needs no permission.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test librivox search query="jane austen"`.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { Audiobook, AudiobookPage } from '@raygun/sdk';

const SEARCH = 'https://archive.org/advancedsearch.php';
const PAGE_SIZE = 30;
const FIELDS = ['identifier', 'title', 'creator', 'call_number', 'runtime', 'language'];

type Doc = {
  identifier: string;
  title?: string | string[];
  creator?: string | string[];
  call_number?: string | string[];
  runtime?: string;
  language?: string | string[];
};
type Answer = { response?: { numFound?: number; docs?: Doc[] } };

const first = (value: string | string[] | undefined) =>
  (Array.isArray(value) ? value[0] : value)?.trim() || null;

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const toBook = (doc: Doc): Audiobook[] => {
  const project = first(doc.call_number);
  const title = first(doc.title);
  // Only LibriVox projects have a chapter feed.
  if (!project || !/^\d+$/.test(project) || !title) return [];
  return [
    {
      id: doc.identifier,
      title,
      author: first(doc.creator),
      feedUrl: `https://librivox.org/rss/${project}`,
      artworkUrl: `https://archive.org/services/img/${encodeURIComponent(doc.identifier)}`,
      runtime: doc.runtime ?? null,
      language: first(doc.language),
    },
  ];
};

/** A page of LibriVox books matching `search`, in `sort` order. Pages count from 1. */
const find = async (search: string, sort: string, page: number): Promise<AudiobookPage> => {
  const at = Math.max(page, 1);
  const params = query({
    q: `collection:librivoxaudio AND ${search}`,
    'fl[]': FIELDS,
    'sort[]': sort,
    rows: PAGE_SIZE,
    page: at,
    output: 'json',
  });
  const answer = await fetchJson<Answer>(`${SEARCH}?${params}`);
  return {
    books: (answer.response?.docs ?? []).flatMap(toBook),
    hasMore: at * PAGE_SIZE < (answer.response?.numFound ?? 0),
  };
};

registerProvider(
  defineProvider({
    kind: 'audiobook.directory',
    id: 'librivox',
    name: '__NAME__',

    search: ({ query: words, page }) => {
      const cleaned = escape(words);
      const search = cleaned ? `(title:(${cleaned}) OR creator:(${cleaned}))` : 'mediatype:audio';
      return find(search, 'downloads desc', page);
    },
    popular: ({ page }) => find('mediatype:audio', 'downloads desc', page),
    latest: ({ page }) => find('mediatype:audio', 'publicdate desc', page),
    // Genres by their English names ("Mystery", "Poetry"), as the archive files LibriVox's books.
    subject: ({ subject, page }) => {
      const cleaned = escape(subject);
      return cleaned
        ? find(`subject:(${cleaned})`, 'downloads desc', page)
        : Promise.resolve({ books: [], hasMore: false });
    },
  }),
);
