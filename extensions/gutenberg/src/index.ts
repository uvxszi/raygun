// Project Gutenberg (M31): the catalogue from gutenberg.org's OPDS feeds (popular, newest, search
// and details) and each book's HTML, split into chapters at its headings.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { MangaChapter, MangaDetails, MangaPage, MangaSummary, Passage } from '@raygun/sdk';

const BOOKS = 'https://www.gutenberg.org';
// Sections with less text than this (title pages, lists of illustrations) aren't chapters.
const MIN_SECTION_TEXT = 120;
// Entries per page of Gutenberg's OPDS feeds.
const FEED_PAGE = 25;

type Section = { name: string; html: string };

const decode = (text: string) =>
  text
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&amp;/g, '&');

/** Page numbers and captions that would pollute names and text. */
const withoutMarginalia = (html: string) =>
  html.replace(/<span[^>]*class=["'](?:pagenum|caption)[^"']*["'][^>]*>[\s\S]*?<\/span>/gi, '');

const plainText = (html: string) =>
  decode(withoutMarginalia(html).replace(/<[^>]+>/g, ' '))
    .replace(/\{[^}]*\}|\[Pg\s*\w+\]/g, ' ')
    .replace(/\s+/g, ' ')
    .trim();

/** "Shelley, Mary Wollstonecraft" → "Mary Wollstonecraft Shelley". */
const personName = (name: string) => {
  const [last, first] = name.split(/,\s*/, 2);
  return first ? `${first} ${last}` : name;
};

const cover = (id: string) => `${BOOKS}/cache/epub/${id}/pg${id}.cover.medium.jpg`;

const fetchText = async (url: string) => {
  const response = await raygun.http.fetch(url);
  if (!response.ok) throw new Error(`Project Gutenberg answered HTTP ${response.status}`);
  return { text: await response.text(), url: response.url };
};

const entries = (xml: string) =>
  [...xml.matchAll(/<entry>([\s\S]*?)<\/entry>/g)].map((match) => match[1] ?? '');

/**
 * The tail of the icon Gutenberg's feeds give audio recordings. They have no text to read (their
 * HTML edition is missing), so book lists leave them out; LibriVox has them as audiobooks.
 */
const AUDIO_ICON = 'ZHlxyWU8AAAAAElFTkSuQmCC';

/** A page of books from an OPDS feed (author and subject entries, and recordings, are skipped). */
const feed = async (params: string, page: number): Promise<MangaPage> => {
  const start = (page - 1) * FEED_PAGE + 1;
  const { text } = await fetchText(`${BOOKS}/ebooks/search.opds/?${params}&start_index=${start}`);
  const all = entries(text);
  const items: MangaSummary[] = all.flatMap((entry) => {
    if (entry.includes(AUDIO_ICON)) return [];
    const id = /<id>[^<]*\/ebooks\/(\d+)\.opds<\/id>/.exec(entry)?.[1];
    const title = /<title>([\s\S]*?)<\/title>/.exec(entry)?.[1];
    return id && title ? [{ id, title: decode(title.trim()), coverUrl: cover(id) }] : [];
  });
  return { items, hasNextPage: all.length >= FEED_PAGE };
};

/** The book's body, between Gutenberg's start and end markers. */
const body = (html: string) => {
  const start = /\*\*\*\s*START OF (?:THE|THIS) PROJECT GUTENBERG[^*]*\*\*\*/i.exec(html);
  const end = /\*\*\*\s*END OF (?:THE|THIS) PROJECT GUTENBERG/i.exec(html);
  const from = start ? start.index + start[0].length : html.search(/<body[\s>]/i);
  const to = end ? end.index : html.length;
  return html.slice(Math.max(from, 0), to > from ? to : html.length);
};

/** Chapters: the text from each heading to the next, at the first level with two or more. */
const splitBook = (html: string): Section[] => {
  const text = body(html);
  for (const level of ['h2', 'h3']) {
    const starts = [...text.matchAll(new RegExp(`<${level}[\\s>]`, 'gi'))].map(
      (match) => match.index ?? 0,
    );
    if (starts.length < 2) continue;
    const sections: Section[] = [];
    starts.forEach((start, index) => {
      const sectionHtml = text.slice(start, starts[index + 1] ?? text.length);
      const close = sectionHtml.search(new RegExp(`</${level}>`, 'i'));
      const heading = close > 0 ? sectionHtml.slice(0, close) : '';
      // Illustrated books set some headings as pictures; their alt text names them.
      const name = plainText(heading) || decode(/alt="([^"]+)"/.exec(heading)?.[1] ?? '').trim();
      // Measured roughly (markup included) to keep long books quick.
      const contentLength = sectionHtml.length - Math.max(close, 0);
      if (/^(table of )?contents\.?$/i.test(name) || contentLength < MIN_SECTION_TEXT) return;
      sections.push({ name: name || `Section ${sections.length + 1}`, html: sectionHtml });
    });
    if (sections.length > 0) return sections;
  }
  return [{ name: 'Full text', html: text }];
};

// The last book read stays in memory, so turning chapters doesn't download it again.
let lastBook: { id: string; url: string; sections: Section[] } | null = null;

const book = async (id: string) => {
  if (!/^\d+$/.test(id)) throw new Error('Unknown book');
  if (lastBook?.id === id) return lastBook;
  const { text, url } = await fetchText(`${BOOKS}/ebooks/${id}.html.images`);
  lastBook = { id, url, sections: splitBook(text) };
  return lastBook;
};

registerProvider(
  defineProvider({
    kind: 'novel.source',
    id: 'gutenberg',
    name: 'Project Gutenberg',
    popular: ({ page }) => feed('sort_order=downloads', page),
    latest: ({ page }) => feed('sort_order=release_date', page),
    search: ({ query, page }) => feed(`query=${encodeURIComponent(query)}`, page),
    details: async ({ id }): Promise<MangaDetails> => {
      if (!/^\d+$/.test(id)) throw new Error('Unknown book');
      const { text } = await fetchText(`${BOOKS}/ebooks/${id}.opds`);
      const entry = entries(text)[0] ?? '';
      const title = /<title>([\s\S]*?)<\/title>/.exec(entry)?.[1];
      if (!title) throw new Error('This book has no catalogue entry');
      const authors = [...entry.matchAll(/<author>\s*<name>([\s\S]*?)<\/name>/g)].map((match) =>
        personName(decode((match[1] ?? '').trim())),
      );
      const summary = /Summary:\s*([\s\S]*?)<\/p>/.exec(entry)?.[1];
      // Library of Congress subject headings (the LCC class codes beside them aren't genres).
      const subjects = [...entry.matchAll(/<category scheme="[^"]*LCSH" term="([^"]*)"/g)]
        .map((match) => (decode(match[1] ?? '').split(' -- ')[0] ?? '').trim())
        .filter((subject, index, all) => subject && all.indexOf(subject) === index);
      return {
        id,
        title: decode(title.trim()),
        coverUrl: cover(id),
        authors,
        description: summary
          ? plainText(summary).replace(/\s*\(This is an automatically generated summary\.\)$/, '')
          : null,
        genres: subjects.slice(0, 8),
        status: 'completed',
        url: `${BOOKS}/ebooks/${id}`,
      };
    },
    chapters: async ({ id }) => {
      const { sections } = await book(id);
      const chapters: MangaChapter[] = sections.map((section, index) => ({
        id: `${id}/${index}`,
        name: section.name,
        number: index + 1,
      }));
      return { chapters };
    },
    passage: async ({ chapterId }): Promise<Passage> => {
      const [id = '', index = ''] = chapterId.split('/');
      const { sections, url } = await book(id);
      const section = sections[Number(index)];
      if (!section) throw new Error('This chapter is no longer in the book');
      return { html: withoutMarginalia(section.html), baseUrl: url };
    },
  }),
);
