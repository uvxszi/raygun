// Pepper&Carrot (peppercarrot.com): David Revoy's webcomic, CC BY 4.0, translated by its readers.
// The site publishes its episodes as JSON (name, page count, languages); pages are pictures named
// by language, episode and page, and each language's webcomic page gives the episodes' names in it
// and their dates. An episode not yet translated is read in English.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { MangaChapter, MangaDetails, MangaPage, MangaSummary } from '@raygun/sdk';

const SITE = 'https://www.peppercarrot.com';
const SOURCES = `${SITE}/0_sources`;
const SERIES_ID = 'peppercarrot';
const COVER = `${SOURCES}/0ther/artworks/low-res/2016-02-24_vertical-cover_remake_by-David-Revoy.jpg`;
/** The list changes with each new episode or translation; it's read again after this long. */
const LIST_TTL_MS = 30 * 60_000;

// The site's own language codes (`cn`, `mx`), not ISO ones.
const LANGUAGES: Record<string, string> = {
  English: 'en',
  French: 'fr',
  Spanish: 'es',
  'Mexican Spanish': 'mx',
  German: 'de',
  Italian: 'it',
  'Portuguese (Brazil)': 'pt',
  Dutch: 'nl',
  Polish: 'pl',
  Russian: 'ru',
  Japanese: 'ja',
  'Chinese (Simplified)': 'cn',
  Indonesian: 'id',
  Vietnamese: 'vi',
  Catalan: 'ca',
  Danish: 'da',
  Finnish: 'fi',
  Croatian: 'hr',
  Hungarian: 'hu',
  Romanian: 'ro',
  Esperanto: 'eo',
};
const language = () => LANGUAGES[raygun.settings.get<string>('language') ?? ''] ?? 'en';

const SERIES: MangaSummary = { id: SERIES_ID, title: 'Pepper&Carrot', coverUrl: COVER };
const ONE_SERIES: MangaPage = { items: [SERIES], hasNextPage: false };

type Episode = { name: string; total_pages: number; translated_languages: string[] };
/** An episode's name and date in one language, from its webcomic page. */
type Caption = { name: string; date: number | null };

const remembered = new Map<string, { at: number; value: Promise<unknown> }>();

const cached = <T>(key: string, load: () => Promise<T>): Promise<T> => {
  const hit = remembered.get(key);
  if (hit && Date.now() - hit.at < LIST_TTL_MS) return hit.value as Promise<T>;
  const value = load();
  remembered.set(key, { at: Date.now(), value });
  value.catch(() => remembered.delete(key));
  return value;
};

const episodes = () =>
  cached('episodes', async () => {
    const response = await raygun.http.fetch(`${SOURCES}/episodes.json`);
    if (!response.ok) throw new Error(`Pepper&Carrot answered HTTP ${response.status}`);
    const list = await response.json<Episode[]>();
    return list.filter(
      (episode) =>
        /^ep\d+_/.test(episode.name) &&
        Number.isInteger(episode.total_pages) &&
        episode.total_pages > 0 &&
        Array.isArray(episode.translated_languages),
    );
  });

/** Episode names and dates in `lang`, by episode folder; none when the page can't be read. */
const captions = (lang: string) =>
  cached(`captions:${lang}`, async () => {
    const found = new Map<string, Caption>();
    const response = await raygun.http.fetch(`${SITE}/${lang}/webcomics/peppercarrot.html`);
    if (!response.ok) return found;
    const html = await response.text();
    for (const caption of raygun.html.select(html, 'figure figcaption')) {
      const link = caption.select('a[href]')[0];
      const folder = /\/webcomic\/(ep\d+_[^/.]+)\.html/.exec(link?.attr('href') ?? '')?.[1];
      const name = link?.text.replace(/\s+/g, ' ').trim();
      if (!folder || !name) continue;
      const day = /(\d{4})-(\d{2})-(\d{2})/.exec(caption.text);
      found.set(folder, {
        name,
        date: day ? Date.UTC(Number(day[1]), Number(day[2]) - 1, Number(day[3])) : null,
      });
    }
    return found;
  });

const numberOf = (episode: Episode) => Number(/^ep(\d+)_/.exec(episode.name)?.[1] ?? NaN);

/** "ep01_Potion-of-Flight" → "Potion of Flight", when the site gave no name. */
const nameFromFolder = (folder: string) => folder.replace(/^ep\d+_/, '').replace(/-/g, ' ');

const chapters = async (): Promise<MangaChapter[]> => {
  const lang = language();
  const [list, names, english] = await Promise.all([
    episodes(),
    captions(lang),
    lang === 'en' ? Promise.resolve(new Map<string, Caption>()) : captions('en'),
  ]);
  return list
    .map((episode) => {
      const caption = names.get(episode.name) ?? english.get(episode.name);
      const number = numberOf(episode);
      return {
        id: episode.name,
        name: caption?.name ?? nameFromFolder(episode.name),
        number: Number.isFinite(number) ? number : null,
        dateUpload: caption?.date ?? english.get(episode.name)?.date ?? null,
      };
    })
    .sort((a, b) => (b.number ?? 0) - (a.number ?? 0));
};

const two = (value: number) => String(value).padStart(2, '0');

const pages = async (chapterId: string) => {
  const episode = (await episodes()).find((item) => item.name === chapterId);
  if (!episode) throw new Error(`Pepper&Carrot has no episode ${chapterId}`);
  const chosen = language();
  const lang = episode.translated_languages.includes(chosen) ? chosen : 'en';
  const size = raygun.settings.get<boolean>('fullSize') === true ? 'hi-res' : 'low-res';
  const number = two(numberOf(episode));
  // P00 is the episode's title header; the story runs from P01.
  return Array.from(
    { length: episode.total_pages + 1 },
    (_, page) =>
      `${SOURCES}/${episode.name}/${size}/${lang}_Pepper-and-Carrot_by-David-Revoy_E${number}P${two(page)}.jpg`,
  );
};

const details = async (): Promise<MangaDetails> => ({
  id: SERIES_ID,
  title: 'Pepper&Carrot',
  coverUrl: COVER,
  authors: ['David Revoy'],
  artists: ['David Revoy'],
  description:
    'Pepper, a young witch, and her cat Carrot in the magical world of Hereva. Free and open source, under the Creative Commons Attribution 4.0 license.',
  genres: ['Fantasy', 'Comedy', 'Adventure'],
  status: 'ongoing',
  url: `${SITE}/${language()}/webcomics/peppercarrot.html`,
});

const simple = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

const search = async (query: string, page: number): Promise<MangaPage> => {
  const none: MangaPage = { items: [], hasNextPage: false };
  if (page > 1) return none;
  const words = simple(query).split(' ').filter(Boolean);
  if (words.length === 0) return ONE_SERIES;
  const matches = (text: string) => {
    const haystack = simple(text);
    return words.every((word) => haystack.includes(word));
  };
  if (matches('pepper carrot david revoy hereva')) return ONE_SERIES;
  const names = await chapters().catch(() => []);
  return names.some((chapter) => matches(chapter.name ?? '')) ? ONE_SERIES : none;
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'peppercarrot',
    name: 'Pepper&Carrot',
    popular: async () => ONE_SERIES,
    latest: async () => ONE_SERIES,
    search: ({ query, page }) => search(query, page),
    details: () => details(),
    chapters: async () => ({ chapters: await chapters() }),
    pages: async ({ chapterId }) => ({ pages: await pages(chapterId) }),
  }),
);
