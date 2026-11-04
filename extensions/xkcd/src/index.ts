// xkcd (xkcd.com): Randall Munroe's webcomic as one series, every comic a chapter. The archive page
// lists them all with their dates; each comic's page gives its picture (the sharper 2x one when
// there is one, the large version when it links to one). Comics that are programs rather than
// pictures can only be seen on the site.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { MangaChapter, MangaDetails, MangaPage, MangaSummary } from '@raygun/sdk';

const SITE = 'https://xkcd.com';
const SERIES_ID = 'xkcd';
const COVER = 'https://imgs.xkcd.com/static/terrible_small_logo.png';
/** The archive changes three times a week; it's read again after this long. */
const ARCHIVE_TTL_MS = 30 * 60_000;

const SERIES: MangaSummary = { id: SERIES_ID, title: 'xkcd', coverUrl: COVER };
const ONE_SERIES: MangaPage = { items: [SERIES], hasNextPage: false };

type Comic = { number: number; title: string; date: number | null };

let archive: { at: number; comics: Promise<Comic[]> } | null = null;

const readArchive = async (): Promise<Comic[]> => {
  const response = await raygun.http.fetch(`${SITE}/archive/`);
  if (!response.ok) throw new Error(`xkcd answered HTTP ${response.status}`);
  const html = await response.text();
  const comics: Comic[] = [];
  // `<a href="/3319/" title="2026-11-2">Elevators</a>`, newest first.
  for (const link of raygun.html.select(html, '#middleContainer a[href]')) {
    const number = /^\/(\d+)\/$/.exec(link.attr('href') ?? '')?.[1];
    if (!number) continue;
    const [year, month, day] = (link.attr('title') ?? '').split('-').map(Number);
    const date = year && month && day ? Date.UTC(year, month - 1, day) : NaN;
    comics.push({
      number: Number(number),
      title: link.text.trim() || `xkcd ${number}`,
      date: Number.isFinite(date) ? date : null,
    });
  }
  if (comics.length === 0) throw new Error("xkcd's archive had no comics in it");
  return comics;
};

const comics = () => {
  if (!archive || Date.now() - archive.at > ARCHIVE_TTL_MS) {
    const current = { at: Date.now(), comics: readArchive() };
    current.comics.catch(() => {
      if (archive === current) archive = null;
    });
    archive = current;
  }
  return archive.comics;
};

const simple = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/** The series, when the words name it, its author or one of its comics. */
const search = async (query: string, page: number): Promise<MangaPage> => {
  const none: MangaPage = { items: [], hasNextPage: false };
  if (page > 1) return none;
  const words = simple(query).split(' ').filter(Boolean);
  if (words.length === 0) return ONE_SERIES;
  const matches = (text: string) => {
    const haystack = simple(text);
    return words.every((word) => haystack.includes(word));
  };
  if (matches('xkcd randall munroe')) return ONE_SERIES;
  const found = (await comics()).some(
    (comic) => matches(comic.title) || words.join(' ') === String(comic.number),
  );
  return found ? ONE_SERIES : none;
};

const details = async (): Promise<MangaDetails> => ({
  id: SERIES_ID,
  title: 'xkcd',
  coverUrl: COVER,
  authors: ['Randall Munroe'],
  artists: ['Randall Munroe'],
  description: 'A webcomic of romance, sarcasm, math, and language.',
  genres: ['Comedy', 'Science'],
  status: 'ongoing',
  url: SITE,
});

const chapters = async (): Promise<MangaChapter[]> =>
  (await comics()).map((comic) => ({
    id: String(comic.number),
    name: comic.title,
    number: comic.number,
    dateUpload: comic.date,
  }));

const absolute = (url: string) => (url.startsWith('//') ? `https:${url}` : url);

const pages = async (chapterId: string) => {
  if (!/^\d+$/.test(chapterId)) throw new Error(`xkcd has no comic ${chapterId}`);
  const response = await raygun.http.fetch(`${SITE}/${chapterId}/`);
  if (response.status === 404) throw new Error(`xkcd has no comic ${chapterId}`);
  if (!response.ok) throw new Error(`xkcd answered HTTP ${response.status}`);
  const html = await response.text();
  const image = raygun.html.select(html, '#comic img')[0];
  if (!image) throw new Error('This xkcd is interactive; it works only on xkcd.com');
  // A few comics link their picture to a larger one.
  const large = raygun.html.select(html, '#comic a[href]')[0]?.attr('href') ?? '';
  if (/^(?:https?:)?\/\/imgs\.xkcd\.com\/comics\/.+\.(?:png|jpe?g|gif)$/i.test(large)) {
    return [absolute(large)];
  }
  const sharp = /(\S+)\s+2x/.exec(image.attr('srcset') ?? '')?.[1];
  const url = absolute(sharp ?? image.attr('src') ?? '');
  if (!url.startsWith('https://')) throw new Error('This xkcd has no picture to show');
  return [url];
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'xkcd',
    name: 'xkcd',
    popular: async () => ONE_SERIES,
    latest: async () => ONE_SERIES,
    search: ({ query, page }) => search(query, page),
    details: () => details(),
    chapters: async () => ({ chapters: await chapters() }),
    pages: async ({ chapterId }) => ({ pages: await pages(chapterId) }),
  }),
);
