// Open webcomics: webcomics their authors publish under Creative Commons licences, each a series
// whose strips are its chapters. Sandra and Woo (Oliver Knörzer and Powree, CC BY-NC-ND 3.0) lists
// its strips through its site's WordPress API, each strip a post whose page holds the picture;
// turnoff.us (Daniel Stori, CC BY-NC-SA 4.0) lists every comic on one page, each comic's page
// holding its pictures. Lists are read again after half an hour; Sandra and Woo's 1,300 strips
// are read in full once and kept, then brought up to date from the newest posts.
import { defineProvider, fetchJson, registerProvider, resolveUrl } from '@raygun/sdk';
import type { MangaChapter, MangaDetails, MangaPage, MangaStatus, MangaSummary } from '@raygun/sdk';

const SANDRA_AND_WOO = 'https://www.sandraandwoo.com';
const TURNOFF = 'https://turnoff.us';
/** WordPress's category of Sandra and Woo's strips (the other is the authors' news). */
const SANDRA_AND_WOO_STRIPS = 1;
const POSTS_PER_PAGE = 100;
const LIST_TTL_MS = 30 * 60_000;

type Strip = { id: string; name: string; number: number; date: number | null };

type Series = {
  id: string;
  title: string;
  coverUrl: string;
  authors: string[];
  artists: string[];
  description: string;
  genres: string[];
  status: MangaStatus;
  url: string;
  /** Words that find the series in a search, besides its title and authors. */
  keywords: string;
  readStrips: () => Promise<Strip[]>;
  pictures: (strip: string) => Promise<string[]>;
};

const decode = (text: string) =>
  text
    .replace(/<[^>]*>/g, '')
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(Number.parseInt(code, 16)),
    )
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .trim();

/** A page's HTML and where it ended up after redirects; a missing page is a missing strip. */
const page = async (address: string) => {
  const response = await raygun.http.fetch(address);
  if (response.status === 404) throw new Error('This strip is no longer on the site');
  if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
  return { html: await response.text(), url: response.url || address };
};

/** Pictures as absolute addresses, each once; the sites' own `http:` links go over https. */
const pictureAddresses = (base: string, sources: (string | null)[]) => [
  ...new Set(
    sources
      .filter((source): source is string => Boolean(source))
      .map((source) =>
        resolveUrl(base, source).replace(
          /^http:\/\/(www\.sandraandwoo\.com|turnoff\.us)\//,
          'https://$1/',
        ),
      )
      .filter((url) => /^https?:\/\//.test(url)),
  ),
];

// ---- Sandra and Woo ----

type Post = { id: number; date_gmt?: string; title?: { rendered?: string } };
/** A strip as kept between runs: its post, number, name and date. */
type KeptStrip = [string, number, string, number | null];

/** "[1340] MH370 until proven otherwise": the strip's number, then its name. */
const STRIP_TITLE = /^\[(\d+)\]\s*(.*)$/;
const KEPT_STRIPS = 'sandraAndWoo.strips';
/** The newest posts read to bring a kept list up to date. */
const NEWEST_POSTS = 20;

const postsAddress = (order: 'asc' | 'desc', perPage: number, at: number) =>
  `${SANDRA_AND_WOO}/wp-json/wp/v2/posts?categories=${SANDRA_AND_WOO_STRIPS}` +
  `&per_page=${perPage}&page=${at}&orderby=date&order=${order}&_fields=id,date_gmt,title`;

/** A numbered strip's post; the category also holds a few notes without numbers. */
const stripOf = (post: Post): Strip[] => {
  const title = STRIP_TITLE.exec(decode(post.title?.rendered ?? ''));
  if (!title) return [];
  const number = Number(title[1]);
  const date = post.date_gmt ? Date.parse(`${post.date_gmt}Z`) : NaN;
  return [
    {
      id: String(post.id),
      name: title[2]?.trim() || `Strip ${number}`,
      number,
      date: Number.isFinite(date) ? date : null,
    },
  ];
};

/**
 * Every strip, oldest first: the first page says how many there are, and the rest come two at
 * a time, since the site turns away more requests at once.
 */
const readAllStrips = async (): Promise<Strip[]> => {
  const address = postsAddress('asc', POSTS_PER_PAGE, 1);
  const first = await raygun.http.fetch(address);
  if (!first.ok) throw new Error(`HTTP ${first.status} from ${address}`);
  const pages = Math.min(Number.parseInt(first.headers['x-wp-totalpages'] ?? '1', 10) || 1, 50);
  const posts = [...(await first.json<Post[]>())];
  for (let at = 2; at <= pages; at += 2) {
    const batch = await Promise.all(
      [at, at + 1]
        .filter((page) => page <= pages)
        .map((page) => fetchJson<Post[]>(postsAddress('asc', POSTS_PER_PAGE, page))),
    );
    posts.push(...batch.flat());
  }
  return posts.flatMap(stripOf);
};

/** Strips by number, each once (the first post of a number), newest first. */
const numbered = (strips: Strip[]) => {
  const byNumber = new Map<number, Strip>();
  for (const strip of strips) if (!byNumber.has(strip.number)) byNumber.set(strip.number, strip);
  return [...byNumber.values()].sort((a, b) => b.number - a.number);
};

/**
 * The strips: read once in full and kept, then brought up to date from the newest posts; the
 * kept list answers when the site doesn't.
 */
const sandraAndWooStrips = async (): Promise<Strip[]> => {
  const kept = (raygun.storage.get<KeptStrip[]>(KEPT_STRIPS) ?? []).map(
    ([id, number, name, date]): Strip => ({ id, number, name, date }),
  );
  let strips: Strip[];
  if (kept.length > 0) {
    try {
      const newest = await fetchJson<Post[]>(postsAddress('desc', NEWEST_POSTS, 1));
      strips = numbered([...kept, ...newest.flatMap(stripOf)]);
    } catch {
      return numbered(kept);
    }
  } else {
    strips = numbered(await readAllStrips());
  }
  if (strips.length === 0) throw new Error('Sandra and Woo listed no strips');
  try {
    raygun.storage.set(
      KEPT_STRIPS,
      strips.map((strip): KeptStrip => [strip.id, strip.number, strip.name, strip.date]),
    );
  } catch {
    // Too large to keep: it's read in full again next time.
  }
  return strips;
};

const sandraAndWooPictures = async (strip: string) => {
  if (!/^\d+$/.test(strip)) throw new Error('Sandra and Woo has no such strip');
  // WordPress sends a post's number to its page.
  const { html, url } = await page(`${SANDRA_AND_WOO}/?p=${strip}`);
  const pictures = pictureAddresses(
    url,
    raygun.html.select(html, '#comic img').map((image) => image.attr('src')),
  );
  if (pictures.length === 0) throw new Error('This post has no strip to show');
  return pictures;
};

// ---- turnoff.us ----

const turnoffStrips = async (): Promise<Strip[]> => {
  const { html } = await page(`${TURNOFF}/all/`);
  const links = raygun.html
    .select(html, 'ul.post-list a.post-link[href]')
    .map((link) => ({ slug: /^\/geek\/([^/]+)\/?$/.exec(link.attr('href') ?? '')?.[1], link }))
    .filter((found): found is { slug: string; link: (typeof found)['link'] } =>
      Boolean(found.slug),
    );
  if (links.length === 0) throw new Error('turnoff.us listed no comics');
  // Newest first: the oldest is number 1.
  return links.map(({ slug, link }, index) => ({
    id: slug,
    name: decode(link.text) || slug.replace(/-/g, ' '),
    number: links.length - index,
    date: null,
  }));
};

const turnoffPictures = async (strip: string) => {
  if (!/^[\w.-]+$/.test(strip)) throw new Error('turnoff.us has no such comic');
  const { html, url } = await page(`${TURNOFF}/geek/${strip}/`);
  const pictures = pictureAddresses(
    url,
    raygun.html
      .select(html, 'img[src]')
      .map((image) => image.attr('src'))
      .filter((source) => /\/image\/en\//.test(source ?? '')),
  );
  if (pictures.length === 0) throw new Error('This comic has no picture to show');
  return pictures;
};

const SERIES: Series[] = [
  {
    id: 'sandra-and-woo',
    title: 'Sandra and Woo',
    coverUrl: `${SANDRA_AND_WOO}/images/cast/woo.png`,
    authors: ['Oliver Knörzer'],
    artists: ['Powree'],
    description:
      'A webcomic about Sandra, her pet raccoon Woo and their friends, written by Oliver ' +
      'Knörzer and drawn by Powree, begun in 2008.\n\n' +
      'Licence: Creative Commons Attribution-NonCommercial-NoDerivs 3.0 (CC BY-NC-ND 3.0).',
    genres: ['Comedy', 'Slice of life', 'Webcomic'],
    status: 'unknown',
    url: SANDRA_AND_WOO,
    keywords: 'sandra woo raccoon knorzer knoerzer powree',
    readStrips: sandraAndWooStrips,
    pictures: sandraAndWooPictures,
  },
  {
    id: 'turnoff',
    title: 'turnoff.us',
    coverUrl: `${TURNOFF}/image/en/bsod.jpeg`,
    authors: ['Daniel Stori'],
    artists: ['Daniel Stori'],
    description:
      'A webcomic about programming, operating systems and life in tech, by Daniel Stori.\n\n' +
      'Licence: Creative Commons Attribution-NonCommercial-ShareAlike 4.0 (CC BY-NC-SA 4.0).',
    genres: ['Comedy', 'Programming', 'Webcomic'],
    status: 'ongoing',
    url: TURNOFF,
    keywords: 'turnoff turn off geek programming linux developer stori',
    readStrips: turnoffStrips,
    pictures: turnoffPictures,
  },
];

const lists = new Map<string, { at: number; strips: Promise<Strip[]> }>();

/** A series' strips, newest first, read again after half an hour. */
const strips = (series: Series) => {
  const kept = lists.get(series.id);
  if (kept && Date.now() - kept.at < LIST_TTL_MS) return kept.strips;
  const current = { at: Date.now(), strips: series.readStrips() };
  current.strips.catch(() => {
    if (lists.get(series.id) === current) lists.delete(series.id);
  });
  lists.set(series.id, current);
  return current.strips;
};

const seriesOf = (id: string) => {
  const found = SERIES.find((series) => series.id === id);
  if (!found) throw new Error('Open webcomics has no such series');
  return found;
};

const summary = (series: Series): MangaSummary => ({
  id: series.id,
  title: series.title,
  coverUrl: series.coverUrl,
  authors: series.authors,
});

const ALL: MangaPage = { items: SERIES.map(summary), hasNextPage: false };

const simple = (text: string) =>
  text
    .normalize('NFD')
    .replace(/\p{M}/gu, '')
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/**
 * The series whose name, authors or words match, or one of whose strips does when its list was
 * read already (a search asks no site for it).
 */
const search = async (query: string, page: number): Promise<MangaPage> => {
  if (page > 1) return { items: [], hasNextPage: false };
  const words = simple(query).split(' ').filter(Boolean);
  if (words.length === 0) return ALL;
  const matches = (text: string) => {
    const haystack = simple(text);
    return words.every((word) => haystack.includes(word));
  };
  const found: Series[] = [];
  for (const series of SERIES) {
    const named = matches(`${series.title} ${series.authors.join(' ')} ${series.keywords}`);
    const kept = lists.get(series.id);
    const strip =
      !named && kept
        ? await kept.strips.then(
            (list) => list.some((each) => matches(each.name)),
            () => false,
          )
        : false;
    if (named || strip) found.push(series);
  }
  return { items: found.map(summary), hasNextPage: false };
};

const details = async (id: string): Promise<MangaDetails> => {
  const series = seriesOf(id);
  return {
    id: series.id,
    title: series.title,
    coverUrl: series.coverUrl,
    authors: series.authors,
    artists: series.artists,
    description: series.description,
    genres: series.genres,
    status: series.status,
    url: series.url,
  };
};

/** A chapter id: the series and its strip. */
const chapterId = (series: string, strip: string) => `${series}/${strip}`;

const chapters = async (id: string): Promise<MangaChapter[]> => {
  const series = seriesOf(id);
  return (await strips(series)).map((strip) => ({
    id: chapterId(series.id, strip.id),
    name: strip.name,
    number: strip.number,
    dateUpload: strip.date,
  }));
};

const pages = async (id: string) => {
  const cut = id.indexOf('/');
  if (cut <= 0) throw new Error('Open webcomics has no such strip');
  return seriesOf(id.slice(0, cut)).pictures(id.slice(cut + 1));
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'webcomics',
    name: 'Open webcomics',
    popular: async () => ALL,
    latest: async () => ALL,
    search: ({ query, page: at }) => search(query, at),
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    pages: async ({ chapterId: id }) => ({ pages: await pages(id) }),
  }),
);
