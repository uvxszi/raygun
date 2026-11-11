# Books: `novel.source`

A book source has the calls and shapes of a [comics source](manga-source.md), with `passage` (a
chapter's text) in place of `pages`. Raygun shows it in Browse → Sources and Search → Books,
keeps saved books in the library, shows them in its book reader, reads them aloud and downloads
chapters.

## Calls

| Call       | Receives                    | Answers                        | Required |
| ---------- | --------------------------- | ------------------------------ | -------- |
| `popular`  | `{ page }`                  | `MangaPage`                    | yes      |
| `latest`   | `{ page }`                  | `MangaPage`                    | no       |
| `search`   | `{ query, page, filters? }` | `MangaPage`                    | yes      |
| `filters`  | nothing                     | `SourceFilter[]`               | no       |
| `details`  | `{ id }`                    | `MangaDetails`                 | yes      |
| `chapters` | `{ id }`                    | `{ chapters: MangaChapter[] }` | yes      |
| `passage`  | `{ chapterId }`             | `Passage`                      | yes      |

Lists, paging, details, chapters, filters and errors work as for [comics](manga-source.md#lists).
List chapters in reading order or newest first; Raygun reads them by `number`.

### The text

`passage` answers a chapter's text as HTML (preferred) or plain text:

```ts
import type { Passage } from '@raygun/sdk';

const asHtml: Passage = {
  html: '<h2>Letter 1</h2><p>You will rejoice to hear…</p><img src="images/plate1.jpg">',
  // Relative pictures resolve against it.
  baseUrl: 'https://www.gutenberg.org/cache/epub/84/pg84-images.html',
};

const asText: Passage = {
  // Split into paragraphs at blank lines; hard-wrapped lines are joined.
  text: 'You will rejoice to hear that no disaster has accompanied\nthe commencement.\n\nI arrived here yesterday.',
};

export const examples = [asHtml, asText];
```

No source HTML reaches the reader. Raygun keeps paragraphs, headings, horizontal rules and
`http(s)` pictures and drops everything else: scripts, styles, forms, frames and the site's
layout. Send the chapter's own part of the page, not the whole page with its menus. A passage is
at most 2 MB of text; an empty one fails ("has no text for this chapter").

### Formulas

Formulas in the HTML are kept and drawn as MathML, at the reader's text size, and read aloud in
words ("x squared plus 1"):

- MathML (`<math>`, as EPUB 3, MediaWiki and LaTeXML write it). Only MathML elements and their
  plain attributes are kept; `mfenced` becomes the row it stands for.
- TeX between `$$…$$` or `\[…\]` (a formula on its own line) and `\(…\)` or `$…$` (in the line).
  A single `$` counts only when it opens before a non-space, closes after one and isn't followed
  by a digit, so prices stay words. TeX inside `<code>` and `<pre>` stays as written.
- MathJax's `<script type="math/tex">` (and `; mode=display`), `math/mml` scripts, and
  `<span class="math">` holding bare TeX.
- KaTeX's and MathJax's rendered boxes (`.katex`, `mjx-container`, `.MathJax`) and MediaWiki's
  `.mwe-math-element`: the MathML inside is used, not the drawing or the fallback picture.

`<sup>` and `<sub>` stay raised and lowered (`x²`, `H₂O`). Pictures of formulas stay pictures,
except those whose `alt` is their TeX (MediaWiki's fallback pictures, `class="latex"`), which are
drawn from it.

### PDF chapters

A chapter can be a PDF document, or some of its pages, instead of text:

```ts
import type { Passage } from '@raygun/sdk';

const wholeDocument: Passage = { pdf: 'https://openstax.org/books/example/chapter-1.pdf' };

const pagesOfOne: Passage = {
  pdf: {
    url: 'https://www.example.org/textbook.pdf',
    headers: { Referer: 'https://www.example.org/' },
    // Counted from 1, both included; the whole document when left out.
    firstPage: 12,
    lastPage: 40,
  },
};

export const pdfExamples = [wholeDocument, pagesOfOne];
```

Raygun fetches the document once (chapters that are pages of the same file share it), draws its
pages on the device and reads them like a comic's (zoom, page turns, the reader's AI layers),
with its text a tap away for reading aloud, its bookmarks as contents and a search through it. A
download keeps the document itself. `pdf` wins over `html` and `text` in the same answer.

## Example: Project Gutenberg

Lists and details from Gutenberg's OPDS catalogue; each book's HTML edition split into chapters
at its `<h2>` headings, with the last book kept in memory so turning chapters doesn't download it
again.

```sh
pnpm raygun-ext init ../my-extensions/gutenberg-books --kind novel.source --id dev.example.gutenberg-books --name "Project Gutenberg"
```

<!-- example: novel.source dev.example.gutenberg-books Project Gutenberg -->

```json
{
  "id": "dev.example.gutenberg-books",
  "name": "Project Gutenberg",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Public-domain books from Project Gutenberg, split into chapters.",
  "website": "https://www.gutenberg.org",
  "provides": ["novel.source"],
  "permissions": { "net": ["www.gutenberg.org"] },
  "contentRating": "safe"
}
```

```ts
// Project Gutenberg: a book source (`novel.source`) for Project Gutenberg's public-domain books
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

/**
 * The line under a book's title in a feed: its author, or for a book without one the feed's
 * order ("1004 downloads", a date), which isn't kept.
 */
const author = (entry: string) => {
  const line = decode(/<content[^>]*>([\s\S]*?)<\/content>/.exec(entry)?.[1] ?? '');
  return line && !/^[\d,]+ downloads?$|^\w{3} \d{1,2}, \d{4}$/.test(line) ? [line] : [];
};

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
    // The feed names each book's author, so the list carries it too.
    return id && title
      ? [{ id, title: decode(title), coverUrl: cover(id), authors: author(entry) }]
      : [];
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
    name: 'Project Gutenberg',

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
```

## Try it

```sh
pnpm exec raygun-ext test gutenberg search query=frankenstein
pnpm exec raygun-ext test gutenberg chapters id=84
pnpm exec raygun-ext test gutenberg passage chapterId=84/1
```
