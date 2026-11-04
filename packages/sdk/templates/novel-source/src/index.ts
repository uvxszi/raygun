// __NAME__: a book source (`novel.source`) for Project Gutenberg's public-domain books
// (https://www.gutenberg.org). Lists and details come from Gutenberg's OPDS catalogue; a book's
// HTML edition is split into chapters at its headings, and Raygun turns each chapter's HTML into
// paragraphs, headings and pictures.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test gutenberg chapters id=84`.
import { defineProvider, fetchText, query, registerProvider } from '@raygun/sdk';
import type { MangaChapter, MangaDetails, MangaPage } from '@raygun/sdk';

const SITE = 'https://www.gutenberg.org';
/** Entries in one page of Gutenberg's OPDS feeds. */
const FEED_PAGE = 25;
/** Sections with less than this (title pages, lists of plates) aren't chapters. */
const MIN_CHAPTER = 200;

const decode = (text: string) =>
  text
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&amp;/g, '&')
    .trim();

const plainText = (html: string) => decode(html.replace(/<[^>]+>/g, ' ')).replace(/\s+/g, ' ');

const entries = (xml: string) =>
  [...xml.matchAll(/<entry>([\s\S]*?)<\/entry>/g)].map((match) => match[1] ?? '');

const cover = (id: string) => `${SITE}/cache/epub/${id}/pg${id}.cover.medium.jpg`;

/** A page of books from an OPDS search; author and subject entries are left out. */
const feed = async (params: Record<string, string>, page: number): Promise<MangaPage> => {
  const start = (Math.max(page, 1) - 1) * FEED_PAGE + 1;
  const xml = await fetchText(
    `${SITE}/ebooks/search.opds/?${query({ ...params, start_index: start })}`,
  );
  const found = entries(xml);
  const items = found.flatMap((entry) => {
    const id = /<id>[^<]*\/ebooks\/(\d+)\.opds<\/id>/.exec(entry)?.[1];
    const title = /<title>([\s\S]*?)<\/title>/.exec(entry)?.[1];
    return id && title ? [{ id, title: decode(title), coverUrl: cover(id) }] : [];
  });
  return { items, hasNextPage: found.length >= FEED_PAGE };
};

type Chapter = { name: string; html: string };

/** The chapters of a book's HTML: from each `<h2>` to the next, between Gutenberg's markers. */
const split = (html: string): Chapter[] => {
  const start = /\*\*\*\s*START OF (?:THE|THIS) PROJECT GUTENBERG[^*]*\*\*\*/i.exec(html);
  const end = /\*\*\*\s*END OF (?:THE|THIS) PROJECT GUTENBERG/i.exec(html);
  const body = html.slice(start ? start.index + start[0].length : 0, end?.index ?? html.length);
  const heads = [...body.matchAll(/<h2[\s>]/gi)].map((match) => match.index ?? 0);
  const chapters = heads
    .map((at, index) => {
      const part = body.slice(at, heads[index + 1] ?? body.length);
      const heading = /<h2[^>]*>([\s\S]*?)<\/h2>/i.exec(part)?.[1] ?? '';
      return { name: plainText(heading), html: part };
    })
    // A table of contents isn't a chapter; Raygun lists the chapters itself.
    .filter((chapter) => !/^(table of )?contents\.?$/i.test(chapter.name))
    .filter((chapter) => plainText(chapter.html).length >= MIN_CHAPTER)
    .map((chapter, index) => ({ ...chapter, name: chapter.name || `Chapter ${index + 1}` }));
  return chapters.length > 0 ? chapters : [{ name: 'Full text', html: body }];
};

// The book read last stays in memory, so turning a page doesn't download it again.
let lastBook: { id: string; url: string; chapters: Chapter[] } | null = null;

const book = async (id: string) => {
  if (!/^\d+$/.test(id)) throw new Error('Project Gutenberg has no such book');
  if (lastBook?.id === id) return lastBook;
  const address = `${SITE}/ebooks/${id}.html.images`;
  const response = await raygun.http.fetch(address);
  if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
  // Pictures are relative to where the address leads after its redirect.
  lastBook = { id, url: response.url, chapters: split(await response.text()) };
  return lastBook;
};

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'gutenberg',
    name: '__NAME__',

    popular: ({ page }) => feed({ sort_order: 'downloads' }, page),
    latest: ({ page }) => feed({ sort_order: 'release_date' }, page),
    search: ({ query: words, page }) =>
      words.trim() ? feed({ query: words }, page) : feed({ sort_order: 'downloads' }, page),

    details: async ({ id }): Promise<MangaDetails> => {
      if (!/^\d+$/.test(id)) throw new Error('Project Gutenberg has no such book');
      const entry = entries(await fetchText(`${SITE}/ebooks/${id}.opds`))[0] ?? '';
      const title = /<title>([\s\S]*?)<\/title>/.exec(entry)?.[1];
      if (!title) throw new Error('This book has no catalogue entry');
      // "Shelley, Mary Wollstonecraft" → "Mary Wollstonecraft Shelley".
      const authors = [...entry.matchAll(/<author>\s*<name>([\s\S]*?)<\/name>/g)].map((match) => {
        const [last, first] = decode(match[1] ?? '').split(/,\s*/, 2);
        return first ? `${first} ${last}` : (last ?? '');
      });
      const subjects = [...entry.matchAll(/<category scheme="[^"]*LCSH" term="([^"]*)"/g)].map(
        (match) => decode(match[1] ?? '').split(' -- ')[0] ?? '',
      );
      const summary = /Summary:\s*([\s\S]*?)<\/p>/.exec(entry)?.[1];
      return {
        id,
        title: decode(title),
        coverUrl: cover(id),
        authors,
        description: summary ? plainText(summary) : null,
        genres: [...new Set(subjects.filter(Boolean))].slice(0, 8),
        status: 'completed',
        url: `${SITE}/ebooks/${id}`,
      };
    },

    chapters: async ({ id }) => {
      const { chapters } = await book(id);
      const list: MangaChapter[] = chapters.map((chapter, index) => ({
        id: `${id}/${index}`,
        name: chapter.name,
        number: index + 1,
      }));
      return { chapters: list };
    },

    passage: async ({ chapterId }) => {
      const [id = '', index = ''] = chapterId.split('/');
      const { chapters, url } = await book(id);
      const chapter = chapters[Number(index)];
      if (!chapter) throw new Error('This chapter is no longer in the book');
      // Relative pictures resolve against the book's address.
      return { html: chapter.html, baseUrl: url };
    },
  }),
);
