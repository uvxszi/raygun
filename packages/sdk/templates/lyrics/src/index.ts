// __NAME__: lyrics (`lyrics`) from Wikisource (https://en.wikisource.org), the library of
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
const HEADERS = { 'User-Agent': `__ID__/0.1.0 (Raygun extension; ${CONTACT})` };
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
    name: '__NAME__',

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
