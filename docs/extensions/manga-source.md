# Comics: `manga.source`

A comics source lists titles, finds them, and gives each title's chapters and each chapter's
pages. Raygun shows it in Browse → Sources and Search → Comics, keeps the titles people save in
the library, checks them for new chapters, downloads chapters, and reads the pages in its reader
(paged, long strip or webtoon). The calls and their shapes follow Mihon's sources.

## Calls

| Call                         | Receives                       | Answers                        | Required |
| ---------------------------- | ------------------------------ | ------------------------------ | -------- |
| `popular`                    | `{ page }`                     | `MangaPage`                    | yes      |
| `latest`                     | `{ page }`                     | `MangaPage`                    | no       |
| `search`                     | `{ query, page, filters? }`    | `MangaPage`                    | yes      |
| `filters`                    | nothing                        | `SourceFilter[]`               | no       |
| `details`                    | `{ id }`                       | `MangaDetails`                 | yes      |
| `chapters`                   | `{ id }`                       | `{ chapters: MangaChapter[] }` | yes      |
| `pages`                      | `{ chapterId }`                | `{ pages: PageImage[] }`       | yes      |

Each call has 30 s. Search asks every comics source for its first page at once and shows a row
per source; a row waits 15 s. Before anything is typed, Search shows each source's `popular`.

### Lists

`popular`, `latest` and `search` answer a page at a time, counting from 1:

```ts
import type { MangaPage } from '@raygun/sdk';

const page: MangaPage = {
  items: [
    {
      id: 'peppercarrot',
      title: 'Pepper&Carrot',
      coverUrl: 'https://www.peppercarrot.com/0_sources/0ther/artworks/low-res/cover.jpg',
    },
  ],
  // Raygun asks for page 2 when the list is scrolled to its end while this is true.
  hasNextPage: false,
};
```

- `id` and `title` are required; an item without them is dropped.
- `coverUrl` must be `http(s)`; `coverHeaders` (a Referer, a User-Agent) are sent when Raygun
  fetches it.
- `nsfw: true` marks an adult title: Raygun hides it while NSFW content is off.

### Details, chapters and pages

- `details` answers a `MangaDetails`: only `title` is required. `status` is one of `ongoing`,
  `completed`, `hiatus`, `cancelled`, `publishingFinished` or `unknown` (anything else reads as
  `unknown`); `url` is the title's page, which the source's menu opens.
- `chapters` answers every chapter at once, usually newest first. Each needs an `id` and a `name`
  or a `number` ("Chapter 12" is made from a number). `dateUpload` is Unix milliseconds;
  `volume` and `scanlator` are optional. Raygun reads chapters in the order of their `number`.
- `pages` answers the pages in reading order, each an `http(s)` address, or `{ url, headers }`
  when the image host wants headers. An answer with no pages fails ("has no pages for this
  chapter").
- A chapter that is a PDF answers `{ pdf: 'https://…/chapter.pdf' }` instead, or
  `{ pdf: { url, headers, firstPage, lastPage } }` for some of a document's pages (counted from
  1, both included). Raygun fetches the document once, draws its pages on the device as wide as
  the screen shows them, and a download keeps the document itself.

### Filters

A source with filters for its search lists them in `filters`; Raygun draws them and passes the
choices back in `search`'s `filters`, only for the ones the user changed. Read them with the
SDK's helpers, which fall back to each filter's default:

```ts
import { defineProvider, selectValue, tagsValue, toggleValue } from '@raygun/sdk';
import type { SelectFilter, TagsFilter, ToggleFilter } from '@raygun/sdk';

const SORT: SelectFilter = {
  kind: 'select',
  key: 'sort',
  label: 'Sort by',
  options: [
    { value: 'popular', label: 'Most read' },
    { value: 'new', label: 'Newest' },
  ],
  default: 'popular',
};
const GENRES: TagsFilter = {
  kind: 'tags',
  key: 'genres',
  label: 'Genres',
  options: [{ value: 'fantasy', label: 'Fantasy' }, { value: 'comedy', label: 'Comedy' }],
};
const FINISHED: ToggleFilter = { kind: 'toggle', key: 'finished', label: 'Finished only' };

export const source = defineProvider({
  kind: 'manga.source',
  id: 'comics',
  filters: async () => [SORT, GENRES, FINISHED],
  search: async ({ query, page, filters }) => {
    const sort = selectValue(filters, SORT);
    const { include, exclude } = tagsValue(filters, GENRES);
    const finished = toggleValue(filters, FINISHED);
    raygun.log.info('search', { query, page, sort, include, exclude, finished });
    return { items: [], hasNextPage: false };
  },
  popular: async () => ({ items: [], hasNextPage: false }),
  details: async ({ id }) => ({ id, title: 'Untitled' }),
  chapters: async () => ({ chapters: [] }),
  pages: async () => ({ pages: [] }),
});
```

A search's `query` may be empty when filters are set. Up to 20 filters of 500 options are kept.

### Errors

Throw an `Error` with a plain sentence for a title or chapter that's gone ("Pepper&Carrot has no
episode ep99"), and let `fetchJson`/`fetchText` throw for HTTP failures. A list with nothing in
it is `{ items: [], hasNextPage: false }`, not an error.

## Example: Pepper&Carrot

One series whose episodes are its chapters, from the site's own episode list, with their names in
the language picked in the extension's settings.

```sh
pnpm raygun-ext init ../my-extensions/peppercarrot --kind manga.source --id dev.example.peppercarrot --name "Pepper&Carrot"
```

<!-- example: manga.source dev.example.peppercarrot Pepper&Carrot -->

```json
{
  "id": "dev.example.peppercarrot",
  "name": "Pepper&Carrot",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Pepper&Carrot, David Revoy's webcomic, free under CC BY 4.0.",
  "website": "https://www.peppercarrot.com",
  "provides": ["manga.source"],
  "permissions": { "net": ["www.peppercarrot.com"] },
  "contentRating": "safe",
  "settings": [
    {
      "key": "language",
      "type": "select",
      "label": "Language",
      "options": ["English", "French", "Spanish", "German", "Italian", "Japanese"],
      "default": "English"
    }
  ]
}
```

```ts
// Pepper&Carrot: a comics source (`manga.source`) for Pepper&Carrot, David Revoy's webcomic, free
// under CC BY 4.0 (https://www.peppercarrot.com). One series; its episodes are the chapters,
// read in the language picked in the extension's settings.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test peppercarrot chapters id=peppercarrot`.
import { defineProvider, fetchJson, fetchText, registerProvider } from '@raygun/sdk';
import type { MangaChapter, MangaDetails, MangaPage } from '@raygun/sdk';

const SITE = 'https://www.peppercarrot.com';
const SOURCES = `${SITE}/0_sources`;
const SERIES = 'peppercarrot';
const COVER = `${SOURCES}/0ther/artworks/low-res/2016-02-24_vertical-cover_remake_by-David-Revoy.jpg`;

/** The site's codes for the languages the manifest's `language` setting offers. */
const LANGUAGES: Record<string, string> = {
  English: 'en',
  French: 'fr',
  Spanish: 'es',
  German: 'de',
  Italian: 'it',
  Japanese: 'ja',
};
const language = () => LANGUAGES[raygun.settings.get<string>('language') ?? ''] ?? 'en';

/** An episode as `episodes.json` lists it: its folder, its page count and its translations. */
type Episode = { name: string; total_pages: number; translated_languages: string[] };

const episodes = async () => {
  const list = await fetchJson<Episode[]>(`${SOURCES}/episodes.json`);
  return list.filter((episode) => /^ep\d+_/.test(episode.name) && episode.total_pages > 0);
};

/** "ep01_Potion-of-Flight" → 1. */
const numberOf = (folder: string) => Number(/^ep(\d+)_/.exec(folder)?.[1]);

/** The episodes' names in `lang`, by folder, from the site's page that lists them. */
const names = async (lang: string) => {
  const html = await fetchText(`${SITE}/${lang}/webcomics/peppercarrot.html`);
  const found = new Map<string, string>();
  for (const link of raygun.html.select(html, 'figure figcaption a[href]')) {
    const folder = /\/webcomic\/(ep\d+_[^/.]+)\.html/.exec(link.attr('href') ?? '')?.[1];
    if (folder) found.set(folder, link.text.replace(/\s+/g, ' ').trim());
  }
  return found;
};

const SERIES_PAGE: MangaPage = {
  items: [{ id: SERIES, title: 'Pepper&Carrot', coverUrl: COVER }],
  hasNextPage: false,
};
const NOTHING: MangaPage = { items: [], hasNextPage: false };

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'peppercarrot',
    name: 'Pepper&Carrot',

    // Every list holds the one series.
    popular: async () => SERIES_PAGE,
    latest: async () => SERIES_PAGE,
    search: async ({ query, page }) => {
      const words = query.toLowerCase().split(/\s+/).filter(Boolean);
      const about = 'pepper carrot david revoy hereva witch';
      return page === 1 && words.every((word) => about.includes(word)) ? SERIES_PAGE : NOTHING;
    },

    details: async (): Promise<MangaDetails> => ({
      id: SERIES,
      title: 'Pepper&Carrot',
      coverUrl: COVER,
      authors: ['David Revoy'],
      artists: ['David Revoy'],
      description:
        'Pepper, a young witch, and her cat Carrot in the magical world of Hereva. Free and open, under CC BY 4.0.',
      genres: ['Fantasy', 'Comedy', 'Adventure'],
      status: 'ongoing',
      url: `${SITE}/${language()}/webcomics/peppercarrot.html`,
    }),

    chapters: async () => {
      // The names are a nicety: without them, chapters are named after their folders.
      const [list, named] = await Promise.all([
        episodes(),
        names(language()).catch(() => new Map<string, string>()),
      ]);
      const chapters: MangaChapter[] = list.map((episode) => ({
        id: episode.name,
        name: named.get(episode.name) ?? episode.name.replace(/^ep\d+_/, '').replace(/-/g, ' '),
        number: numberOf(episode.name),
      }));
      // Newest first, as comics sources list them.
      chapters.sort((a, b) => (b.number ?? 0) - (a.number ?? 0));
      return { chapters };
    },

    pages: async ({ chapterId }) => {
      const episode = (await episodes()).find((item) => item.name === chapterId);
      if (!episode) throw new Error(`Pepper&Carrot has no episode ${chapterId}`);
      // An episode not yet translated is read in English.
      const chosen = language();
      const lang = episode.translated_languages.includes(chosen) ? chosen : 'en';
      const two = (value: number) => String(value).padStart(2, '0');
      const number = two(numberOf(episode.name));
      // Page 00 is the episode's title; the story runs from 01.
      const pages = Array.from(
        { length: episode.total_pages + 1 },
        (_, page) =>
          `${SOURCES}/${episode.name}/low-res/${lang}_Pepper-and-Carrot_by-David-Revoy_E${number}P${two(page)}.jpg`,
      );
      return { pages };
    },
  }),
);
```

## Try it

```sh
pnpm exec raygun-ext test peppercarrot popular
pnpm exec raygun-ext test peppercarrot chapters id=peppercarrot
pnpm exec raygun-ext test peppercarrot pages chapterId=ep01_Potion-of-Flight --set language=French
```
