// Loyal Books (M46.47): public-domain audiobooks (mostly LibriVox readings) with a podcast feed
// per book (`/book/<slug>/feed`), which Raygun reads like LibriVox's. The site lists books by
// popularity (`/Top_100/<page>`, a hundred a page) and by genre; its own search is Google's, so a
// search here looks through the most popular few hundred books.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Audiobook, AudiobookPage } from '@raygun/sdk';

const SITE = 'https://www.loyalbooks.com';
/** Popular pages a search looks through. */
const SEARCHED_PAGES = 5;
const PAGE_SIZE = 30;

const decode = (text: string) =>
  text
    .replace(/<[^>]+>/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&rsquo;|&#8217;/g, "'")
    .replace(/&nbsp;/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

/** The books on a listing page: each cell's link, cover, bold title and the author after it. */
const readListing = (html: string): Audiobook[] => {
  const books: Audiobook[] = [];
  const seen = new Set<string>();
  const cells = html.split(/<td class="layout2-blue"/).slice(1);
  for (const cell of cells) {
    const slug = /href="\/book\/([^"/?#]+)"/.exec(cell)?.[1];
    const title = /<b>([\s\S]*?)<\/b>/.exec(cell)?.[1];
    if (!slug || !title || seen.has(slug)) continue;
    seen.add(slug);
    const author = /<\/b><\/a><br>([^<]*)/.exec(cell)?.[1];
    const image = /<img[^>]+src="([^"]+)"/.exec(cell)?.[1];
    books.push({
      id: slug,
      title: decode(title),
      author: author ? decode(author) || null : null,
      feedUrl: `${SITE}/book/${slug}/feed`,
      artworkUrl: !image
        ? null
        : image.startsWith('http')
          ? image
          : `${SITE}/${image.replace(/^\/+/, '')}`,
      runtime: null,
      language: null,
      description: null,
    });
  }
  return books;
};

const pageCount = (html: string) => Number(/Page \d+ of (\d+)/.exec(html)?.[1] ?? '1');

const listing = async (path: string, page: number): Promise<AudiobookPage> => {
  const at = Math.max(1, page);
  const response = await raygun.http.fetch(`${SITE}${path}${at > 1 ? `/${at}` : ''}`);
  if (!response.ok) throw new Error(`HTTP ${response.status} from Loyal Books`);
  const html = await response.text();
  return { books: readListing(html), hasMore: at < pageCount(html) };
};

/** The most popular books, read once per run, for searches. */
let popularBooks: Promise<Audiobook[]> | null = null;

const searchable = () => {
  popularBooks ??= Promise.all(
    Array.from({ length: SEARCHED_PAGES }, (_, index) =>
      listing('/Top_100', index + 1).then(
        (found) => found.books,
        () => [] as Audiobook[],
      ),
    ),
  ).then((pages) => pages.flat());
  return popularBooks;
};

/** The site's genre pages (`/genre/<name>`), matched however a genre is written. */
const GENRES = [
  'Adventure',
  'Children',
  'Comedy',
  'Fairy_tales',
  'Fantasy',
  'Fiction',
  'Historical_Fiction',
  'History',
  'Humor',
  'Literature',
  'Mystery',
  'Non-fiction',
  'Philosophy',
  'Poetry',
  'Religion',
  'Romance',
  'Science_fiction',
  'Short_stories',
  'Teen_Young_adult',
];

/** The site's page for a genre, or `null` when it has none (it answers those with an error). */
const genrePage = (subject: string) => {
  const name = subject.trim().replace(/\s+/g, '_');
  return GENRES.find((genre) => genre.toLowerCase() === name.toLowerCase()) ?? null;
};

const simple = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

registerProvider(
  defineProvider({
    kind: 'audiobook.directory',
    id: 'books',
    name: 'Loyal Books',
    async search({ query, page }) {
      const words = simple(query).split(' ').filter(Boolean);
      if (words.length === 0) return listing('/Top_100', page);
      const matches = (await searchable()).filter((book) => {
        const text = simple(`${book.title} ${book.author ?? ''}`);
        return words.every((word) => text.includes(word));
      });
      const start = (Math.max(1, page) - 1) * PAGE_SIZE;
      return {
        books: matches.slice(start, start + PAGE_SIZE),
        hasMore: start + PAGE_SIZE < matches.length,
      };
    },
    async popular({ page }) {
      return listing('/Top_100', page);
    },
    async subject({ subject, page }) {
      // Genres are pages of their own: Fiction, Mystery, Children, Fantasy…
      const name = genrePage(subject);
      if (!name) return { books: [], hasMore: false };
      return listing(`/genre/${encodeURIComponent(name)}`, page);
    },
  }),
);
