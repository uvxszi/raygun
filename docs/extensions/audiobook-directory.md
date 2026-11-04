# Audiobooks: `audiobook.directory`

An audiobook directory finds books, lists popular, new and genre's books, and gives each book's
chapters. Raygun shows it in Library → Audio → Audiobooks, the Audiobooks pages and Search →
Audiobooks, asks every directory at once, and plays books chapter by chapter, keeping each one's
place.

A book either has a chapter feed (RSS, like a podcast's), which Raygun reads itself, or an `id`
the provider lists the chapters of with `chapters`.

## Calls

| Call       | Receives              | Answers                | Required |
| ---------- | --------------------- | ---------------------- | -------- |
| `search`   | `{ query, page }`     | `AudiobookPage`        | yes      |
| `popular`  | `{ page }`            | `AudiobookPage`        | yes      |
| `latest`   | `{ page }`            | `AudiobookPage`        | no       |
| `subject`  | `{ subject, page }`   | `AudiobookPage`        | no       |
| `chapters` | `{ id }`              | `AudiobookDetails`     | no       |

Pages count from 1; every call has 12 s.

### Books

```ts
import type { AudiobookDetails, AudiobookPage } from '@raygun/sdk';

const page: AudiobookPage = {
  books: [
    {
      id: 'pride_and_prejudice_librivox',
      title: 'Pride and Prejudice',
      author: 'Jane Austen',
      // Raygun reads the feed itself; its host needs no permission.
      feedUrl: 'https://librivox.org/rss/253',
      runtime: '13:06:44',
      language: 'English',
    },
  ],
  // Raygun asks for the next page while this is true.
  hasMore: true,
};

// For a book without a feed, `chapters` lists its chapters in order.
const details: AudiobookDetails = {
  title: 'The Adventures of Sherlock Holmes',
  author: 'Arthur Conan Doyle',
  chapters: [
    {
      id: '1',
      title: 'A Scandal in Bohemia',
      audioUrl: 'https://example.org/audio/01.mp3',
      durationMs: 1_500_000,
    },
  ],
};

export { details, page };
```

- A book needs a `title`, and an `http(s)` `feedUrl` or an `id`; one without a feed is opened
  through `chapters`. Books repeating a feed are dropped, and a page keeps at most 100.
- `subject` receives a genre by its English name ("Mystery", "Science fiction", "Children",
  "Poetry"), which each source matches to its own.
- A chapter needs an `http(s)` `audioUrl`; a book keeps at most 2,000. A `chapters` answer
  without a title or chapters fails to open.

## Example: LibriVox

Public-domain audiobooks read by LibriVox volunteers, found through the Internet Archive, which
keeps them with each book's LibriVox project number; the number gives the book's chapter feed.

```sh
pnpm raygun-ext init ../my-extensions/librivox-books --kind audiobook.directory --id dev.example.librivox-books --name "LibriVox"
```

<!-- example: audiobook.directory dev.example.librivox-books LibriVox -->

```json
{
  "id": "dev.example.librivox-books",
  "name": "LibriVox",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Public-domain audiobooks read by LibriVox volunteers, found through the Internet Archive.",
  "website": "https://librivox.org",
  "provides": ["audiobook.directory"],
  "permissions": { "net": ["archive.org"] },
  "contentRating": "safe"
}
```

```ts
// LibriVox: an audiobook directory (`audiobook.directory`) of LibriVox's public-domain recordings
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
    name: 'LibriVox',

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
```

## Try it

```sh
pnpm exec raygun-ext test librivox popular
pnpm exec raygun-ext test librivox search query="jane austen"
pnpm exec raygun-ext test librivox subject subject=Mystery
```
