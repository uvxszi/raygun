// Lit2Go (M46.48): the University of South Florida's free collection of classic stories, novels
// and poems read aloud (etc.usf.edu/lit2go). Its catalogue is one page of about two hundred books,
// read once per run for lists and searches; genres are pages of their own. A book's page lists
// its chapters, whose recordings follow one naming rule
// (`audio/mp3/<book>-<NNN>-<chapter>.<id>.mp3`), so a book has no feed: the source lists its
// chapters itself.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Audiobook, AudiobookChapter, AudiobookPage } from '@raygun/sdk';

const SITE = 'https://etc.usf.edu/lit2go';
const PAGE_SIZE = 30;

/** Raygun's genre names, as Lit2Go's genre pages (`/genres/<id>/<name>/`). */
const GENRES: Record<string, string> = {
  adventure: '2/adventure',
  'fairy tales': '13/fairy-talefolk-tale',
  fantasy: '1/fantasy',
  'historical fiction': '11/historical-fiction',
  history: '14/history',
  horror: '7/horror',
  mystery: '8/mystery',
  philosophy: '5/philosophy',
  poetry: '24/poetry',
  romance: '9/romance',
  'science fiction': '6/science-fiction',
  humor: '28/satire',
  drama: '10/tragedy',
  children: '25/nursery-rhyme',
};

const decode = (html: string) =>
  html
    .replace(/<[^>]+>/g, ' ')
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(parseInt(code, 16)),
    )
    .replace(/&quot;/g, '"')
    .replace(/&rsquo;|&lsquo;/g, "'")
    .replace(/&rdquo;|&ldquo;/g, '"')
    .replace(/&hellip;/g, '…')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/\s+/g, ' ')
    .trim();

const fetchPage = async (url: string) => {
  const response = await raygun.http.fetch(url);
  if (!response.ok) throw new Error(`HTTP ${response.status} from Lit2Go`);
  return response.text();
};

/** The books on the catalogue page: each figure's link, cover, title, author and abstract. */
const readCatalogue = (html: string): Audiobook[] => {
  const books: Audiobook[] = [];
  const seen = new Set<string>();
  for (const figure of html.split('<figure>').slice(1)) {
    const link = /href="https:\/\/etc\.usf\.edu\/lit2go\/(\d+)\/([a-z0-9-]+)\/"/.exec(figure);
    const title = /<figcaption class="title">[\s\S]*?<a [^>]*>([\s\S]*?)<\/a>/.exec(figure)?.[1];
    if (!link || !title) continue;
    const id = `${link[1]}/${link[2]}`;
    if (seen.has(id)) continue;
    seen.add(id);
    const author = /<figcaption class="author">[\s\S]*?<a [^>]*>([\s\S]*?)<\/a>/.exec(figure)?.[1];
    const cover = /data-src="(https:\/\/[^"]+)"/.exec(figure)?.[1];
    const about = /<figcaption class="abstract">([\s\S]*?)<\/figcaption>/.exec(figure)?.[1];
    books.push({
      id,
      title: decode(title),
      author: author ? decode(author) || null : null,
      artworkUrl: cover ?? null,
      runtime: null,
      language: 'en',
      description: about ? decode(about) || null : null,
    });
  }
  return books;
};

/** The whole catalogue, read once per run (asked again after a failure). */
let catalogue: Promise<Audiobook[]> | null = null;

const allBooks = () => {
  catalogue ??= fetchPage(`${SITE}/books/`)
    .then(readCatalogue)
    .catch((error: unknown) => {
      catalogue = null;
      throw error;
    });
  return catalogue;
};

const pageOf = (books: Audiobook[], page: number): AudiobookPage => {
  const start = (Math.max(1, page) - 1) * PAGE_SIZE;
  return {
    books: books.slice(start, start + PAGE_SIZE),
    hasMore: start + PAGE_SIZE < books.length,
  };
};

const simple = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/** A book's address parts from its id ("32/the-adventures-of-sherlock-holmes"). */
const parts = (id: string) => {
  const match = /^(\d+)\/([a-z0-9-]+)$/.exec(id);
  if (!match) throw new Error('This isn’t a Lit2Go book');
  return { number: match[1] ?? '', slug: match[2] ?? '' };
};

registerProvider(
  defineProvider({
    kind: 'audiobook.directory',
    id: 'books',
    name: 'Lit2Go',
    async search({ query, page }) {
      const words = simple(query).split(' ').filter(Boolean);
      const books = await allBooks();
      if (words.length === 0) return pageOf(books, page);
      return pageOf(
        books.filter((book) => {
          const text = simple(`${book.title} ${book.author ?? ''}`);
          return words.every((word) => text.includes(word));
        }),
        page,
      );
    },
    async popular({ page }) {
      return pageOf(await allBooks(), page);
    },
    async subject({ subject, page }) {
      const genre = GENRES[subject.trim().toLowerCase()];
      if (!genre) return { books: [], hasMore: false };
      const [html, books] = await Promise.all([fetchPage(`${SITE}/genres/${genre}/`), allBooks()]);
      const byId = new Map(books.map((book) => [book.id, book]));
      const listed = [
        ...html.matchAll(/href="https:\/\/etc\.usf\.edu\/lit2go\/(\d+\/[a-z0-9-]+)\/"/g),
      ]
        .map((match) => byId.get(match[1] ?? ''))
        .filter((book): book is Audiobook => book !== undefined);
      return pageOf([...new Set(listed)], page);
    },
    async chapters({ id }) {
      const { number, slug } = parts(id);
      const address = `${SITE}/${number}/${slug}/`;
      const [html, books] = await Promise.all([
        fetchPage(address),
        allBooks().catch(() => [] as Audiobook[]),
      ]);
      const known = books.find((book) => book.id === id);
      const title = decode(/<h2>([\s\S]*?)<\/h2>/.exec(html)?.[1] ?? '') || known?.title;
      if (!title) throw new Error('Lit2Go has no book there');
      const author = /<h3>[\s\S]*?<a [^>]*>([\s\S]*?)<\/a>/.exec(html)?.[1];
      const cover =
        /<div id="page_thumbnail">\s*<img[^>]+src="(https:\/\/[^"]+)"/.exec(html)?.[1] ??
        /property="og:image" content="(https:\/\/[^"]+)"/.exec(html)?.[1];
      const entries = new RegExp(
        `<dt>\\s*<a href="https://etc\\.usf\\.edu/lit2go/${number}/${slug}/(\\d+)/([a-z0-9-]+)/">([\\s\\S]*?)</a>\\s*</dt>(?:\\s*<dd>([\\s\\S]*?)</dd>)?`,
        'g',
      );
      const chapters: AudiobookChapter[] = [...html.matchAll(entries)].map((match, index) => ({
        id: match[1] ?? null,
        title: decode(match[3] ?? ''),
        // The recordings are numbered by their place in the book.
        audioUrl: `${SITE}/audio/mp3/${slug}-${String(index + 1).padStart(3, '0')}-${match[2] ?? ''}.${match[1] ?? ''}.mp3`,
        audioType: 'audio/mpeg',
        description: match[4] ? decode(match[4]) || null : null,
      }));
      return {
        title,
        author: author ? decode(author) || null : (known?.author ?? null),
        description: known?.description ?? null,
        artworkUrl: cover ?? known?.artworkUrl ?? null,
        link: address,
        chapters,
      };
    },
  }),
);
