// __NAME__: a comics source (`manga.source`) for Pepper&Carrot, David Revoy's webcomic, free
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
  items: [{ id: SERIES, title: 'Pepper&Carrot', coverUrl: COVER, authors: ['David Revoy'] }],
  hasNextPage: false,
};
const NOTHING: MangaPage = { items: [], hasNextPage: false };

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'peppercarrot',
    name: '__NAME__',

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
