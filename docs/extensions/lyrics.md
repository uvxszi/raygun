# Lyrics: `lyrics`

A lyrics provider gives the words of the playing song. Raygun asks every lyrics provider at once
and shows the best answer in the player's Lyrics view: synced lines first (highlighted as they're
sung, and tapped to seek), then plain text. The others stay a tap away in the lyrics source menu.
Lyrics are kept on the device, so a song asks again only after a while.

## Calls

| Call     | Receives                              | Answers                 | Required |
| -------- | ------------------------------------- | ----------------------- | -------- |
| `lyrics` | `{ title, artist, album, durationMs }` | `LyricsResult \| null` | yes      |

The call has 12 s. `artist`, `album` and `durationMs` may be `null`.

### The answer

```ts
import type { LyricsResult } from '@raygun/sdk';

const synced: LyricsResult = {
  // Times in milliseconds from the start; Raygun sorts the lines by them.
  synced: [
    { timeMs: 12_000, text: 'Should auld acquaintance be forgot,' },
    { timeMs: 16_500, text: 'And never brought to mind?' },
  ],
  // Line for line: `lines[i]` belongs to `synced[i]`.
  translations: [
    {
      language: 'en',
      lines: ['Should old acquaintance be forgotten,', 'And never remembered?'],
    },
  ],
};

const plain: LyricsResult = { plain: 'Should auld acquaintance be forgot,\nAnd never brought to mind?' };

export const answers = [synced, plain];
```

- Answer `null` when there are no lyrics, or the song is instrumental. An answer with neither
  `synced` lines nor `plain` text counts as none.
- A synced line needs a `timeMs` of 0 or more; lines without one are dropped.
- `translations` (at most 8) are other languages, or a romanization (`kind: 'romanization'`, the
  original's language tag with `-Latn`, like `ja-Latn`), line for line with `synced`, or with the
  lines of `plain`. Use `''` for a line without one.
- Match carefully: wrong lyrics are worse than none. Check the title (and the artist when given)
  before answering, and use `durationMs` to tell versions apart.

## Example: Wikisource

The words of public-domain songs, hymns and ballads from Wikisource. The provider looks for a page
named like the song and reads its verse, stanza by stanza, as plain text.

```sh
pnpm raygun-ext init ../my-extensions/wikisource-lyrics --kind lyrics --id dev.example.wikisource-lyrics --name "Wikisource"
```

<!-- example: lyrics dev.example.wikisource-lyrics Wikisource -->

```json
{
  "id": "dev.example.wikisource-lyrics",
  "name": "Wikisource",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Words of public-domain songs and hymns from Wikisource.",
  "website": "https://en.wikisource.org",
  "provides": ["lyrics"],
  "permissions": { "net": ["en.wikisource.org"] },
  "contentRating": "safe"
}
```

```ts
// Wikisource: lyrics (`lyrics`) from Wikisource (https://en.wikisource.org), the library of
// public-domain texts, which holds the words of many old songs, hymns and ballads. Raygun asks
// every lyrics provider for the playing song and shows the best answer: synced lines first, then
// plain text. This one finds a page named like the song and reads its verse.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test wikisource lyrics title="Auld Lang Syne"`.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';

const API = 'https://en.wikisource.org/w/api.php';
// Wikimedia asks every client to name itself and a way to reach its author: put yours here.
const CONTACT = 'https://example.org/your-extension';
const HEADERS = { 'User-Agent': `dev.example.wikisource-lyrics/0.1.0 (Raygun extension; ${CONTACT})` };
/** Pages read for one song, at most. */
const PAGES = 3;

type SearchAnswer = { query?: { search?: { title: string }[] } };
type ParseAnswer = { parse?: { text?: string } };

/** Lower case, without accents or punctuation: "Auld Lang Syne!" → "auld lang syne". */
const simple = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    // The accents NFD splits off.
    .replace(/\p{M}/gu, '')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();

/** The song a page holds: "The Book of Scottish Song/Auld Lang Syne 1" → "auld lang syne". */
const songOf = (page: string) =>
  simple((page.split('/').pop() ?? page).replace(/\([^)]*\)/g, ' ')).replace(/ \d+$/, '');

/** A page's verse: Wikisource sets poems as stanzas of lines (`.ws-poem`). */
const verse = (html: string) =>
  raygun.html
    .select(html, '.ws-poem-stanza')
    .map((stanza) =>
      stanza
        .select('.ws-poem-line')
        .map((line) => line.text.replace(/\s+/g, ' ').trim())
        .filter(Boolean)
        .join('\n'),
    )
    .filter(Boolean)
    .join('\n\n');

registerProvider(
  defineProvider({
    kind: 'lyrics',
    id: 'wikisource',
    name: 'Wikisource',

    async lyrics({ title }) {
      const wanted = simple(title);
      if (!wanted) return null;
      const found = await fetchJson<SearchAnswer>(
        `${API}?${query({
          action: 'query',
          list: 'search',
          srsearch: `intitle:"${title.replace(/"/g, ' ')}"`,
          srnamespace: 0,
          srlimit: 10,
          format: 'json',
          formatversion: 2,
        })}`,
        { headers: HEADERS },
      );
      const pages = (found.query?.search ?? [])
        .map((result) => result.title)
        .filter((page) => songOf(page) === wanted)
        .slice(0, PAGES);
      // One page after another, until one has verse.
      for (const page of pages) {
        const parsed = await fetchJson<ParseAnswer>(
          `${API}?${query({ action: 'parse', page, prop: 'text', redirects: 1, format: 'json', formatversion: 2 })}`,
          { headers: HEADERS },
        );
        const words = verse(parsed.parse?.text ?? '');
        if (words) return { plain: words };
      }
      // No lyrics: Raygun shows another provider's, or none.
      return null;
    },
  }),
);
```

## Try it

```sh
pnpm exec raygun-ext test wikisource lyrics title="Auld Lang Syne"
```
