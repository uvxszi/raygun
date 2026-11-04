// WEBTOON (webtoons.com): the publisher's Originals and its Canvas of creators' own series, free to
// read on the site. Lists come from the site's rankings, schedule, genre and search pages, episodes
// from the mobile site's episode API, and pages from each episode's viewer. The picture host
// refuses requests that don't come from the site, so covers and pages carry its Referer.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type {
  FilterValues,
  HtmlElement,
  MangaChapter,
  MangaDetails,
  MangaPage,
  MangaStatus,
  MangaSummary,
  SourceFilter,
} from '@raygun/sdk';

const SITE = 'https://www.webtoons.com';
const MOBILE = 'https://m.webtoons.com';
const IMAGE_HEADERS = { Referer: `${SITE}/` };
const PAGE_SIZE = 30;
/** Lists read from the site are kept this long, so paging through one doesn't fetch it again. */
const LIST_TTL_MS = 10 * 60_000;

const LANGUAGES: Record<string, string> = {
  English: 'en',
  Spanish: 'es',
  French: 'fr',
  German: 'de',
  Indonesian: 'id',
  Thai: 'th',
  'Chinese (Traditional)': 'zh-hant',
};
const KNOWN_LANGUAGES = new Set(Object.values(LANGUAGES));
const language = () => LANGUAGES[raygun.settings.get<string>('language') ?? ''] ?? 'en';

/** Originals are `webtoon`, Canvas series `canvas` (the site's own names for its two sections). */
type Kind = 'webtoon' | 'canvas';
type SeriesRef = { lang: string; kind: Kind; titleNo: string };

/** `en:webtoon:2480`: stable when a series is renamed, unlike the site's addresses. */
const seriesId = (series: SeriesRef) => `${series.lang}:${series.kind}:${series.titleNo}`;

const parseSeriesId = (id: string): SeriesRef => {
  const [lang = '', kind, titleNo = ''] = id.split(':');
  if (
    !KNOWN_LANGUAGES.has(lang) ||
    (kind !== 'webtoon' && kind !== 'canvas') ||
    !/^\d+$/.test(titleNo)
  ) {
    throw new Error(`WEBTOON has no series ${id}`);
  }
  return { lang, kind, titleNo };
};

/** The series page; the site redirects from any genre and name in the path to the right ones. */
const seriesUrl = (series: SeriesRef) =>
  `${SITE}/${series.lang}/${series.kind === 'canvas' ? 'canvas' : 'series'}/series/list?title_no=${series.titleNo}`;

/** Picture addresses sometimes hold spaces ("4) Thumb_Poster.jpg"). */
const fixUrl = (url: string) => url.trim().replace(/ /g, '%20');

/** A page of the site and where it ended up after redirects. */
const fetchPage = async (url: string): Promise<{ html: string; url: string }> => {
  const response = await raygun.http.fetch(url, {
    // Episodes of series for grown-ups open only past the site's age question; the app's NSFW
    // switch is that answer.
    headers: raygun.content.showNsfw() ? { Cookie: 'ageGatePass=true' } : {},
  });
  if (response.status === 404) throw new Error('WEBTOON no longer has this page');
  if (!response.ok) throw new Error(`WEBTOON answered HTTP ${response.status}`);
  return { html: await response.text(), url: response.url };
};

const get = async (url: string) => (await fetchPage(url)).html;

const remembered = new Map<string, { at: number; value: Promise<unknown> }>();

/** `load()` once per key for a while; failures aren't kept. */
const cached = <T>(key: string, load: () => Promise<T>): Promise<T> => {
  const hit = remembered.get(key);
  if (hit && Date.now() - hit.at < LIST_TTL_MS) return hit.value as Promise<T>;
  const value = load();
  remembered.set(key, { at: Date.now(), value });
  value.catch(() => remembered.delete(key));
  while (remembered.size > 40) remembered.delete(remembered.keys().next().value!);
  return value;
};

const CARD_LINK = /^https:\/\/www\.webtoons\.com\/([a-z-]+)\/([^/]+)\/[^/]+\/list\?title_no=(\d+)/;

const cardOf = (card: HtmlElement): MangaSummary | null => {
  const match = CARD_LINK.exec(card.attr('href') ?? '');
  // Rankings, genres and search name a series in `.title`; the Canvas lists in `.subj`.
  const title = card.select('.title, .subj')[0]?.text.replace(/\s+/g, ' ').trim();
  if (!match || !title || !KNOWN_LANGUAGES.has(match[1]!)) return null;
  const image = card.select('img')[0]?.attr('src');
  return {
    id: seriesId({
      lang: match[1]!,
      kind: match[2] === 'canvas' ? 'canvas' : 'webtoon',
      titleNo: match[3]!,
    }),
    title,
    coverUrl: image?.startsWith('http') ? fixUrl(image) : null,
    coverHeaders: IMAGE_HEADERS,
  };
};

/** The series cards of a list page, each once, in the page's order. */
const cardsIn = (html: string): MangaSummary[] => {
  const seen = new Set<string>();
  const cards: MangaSummary[] = [];
  for (const element of raygun.html.select(
    html,
    'ul.webtoon_list li a, .challenge_lst li a.challenge_item',
  )) {
    const card = cardOf(element);
    if (card && !seen.has(card.id)) {
      seen.add(card.id);
      cards.push(card);
    }
  }
  return cards;
};

/** The series cards of the list page at `url`. */
const cardsOf = (url: string) => get(url).then(cardsIn);

const unique = (lists: MangaSummary[][]) => {
  const seen = new Set<string>();
  return lists.flat().filter((card) => !seen.has(card.id) && Boolean(seen.add(card.id)));
};

/** Every list's cards; fails only when every list does. */
const allOf = async (urls: string[]): Promise<MangaSummary[]> => {
  const results = await Promise.allSettled(urls.map(cardsOf));
  const lists = results.flatMap((result) => (result.status === 'fulfilled' ? [result.value] : []));
  const failure = results.find((result) => result.status === 'rejected');
  if (lists.length === 0 && failure?.status === 'rejected') throw failure.reason;
  return unique(lists);
};

const slice = (cards: MangaSummary[], page: number): MangaPage => {
  const start = (Math.max(page, 1) - 1) * PAGE_SIZE;
  return {
    items: cards.slice(start, start + PAGE_SIZE),
    hasNextPage: start + PAGE_SIZE < cards.length,
  };
};

/** Whether a paged list (search, Canvas genres) links to the page after `page`. */
const linksPageAfter = (html: string, page: number) =>
  raygun.html
    .select(html, 'a[href*="page="]')
    .some((link) => new RegExp(`[?&]page=${page + 1}(?:&|$)`).test(link.attr('href') ?? ''));

// ---- Lists ----

/** The site's four rankings, top series first: popular, trending, Originals and Canvas. */
const rankings = (lang: string) =>
  cached(`rankings:${lang}`, () =>
    allOf(
      ['popular', 'trending', 'originals', 'canvas'].map(
        (name) => `${SITE}/${lang}/ranking/${name}`,
      ),
    ),
  );

/** Today's Originals by their latest episode. */
const latest = (lang: string) =>
  cached(`latest:${lang}`, () => cardsOf(`${SITE}/${lang}/originals?sortOrder=UPDATE`));

// Genres: the Originals' codes, and the Canvas genre each matches.
const GENRES: [label: string, originals: string | null, canvas: string | null][] = [
  ['Action', 'action', 'ACTION'],
  ['Comedy', 'comedy', 'COMEDY'],
  ['Drama', 'drama', 'DRAMA'],
  ['Fantasy', 'fantasy', 'FANTASY'],
  ['Graphic novel', 'graphic_novel', null],
  ['Heartwarming', 'heartwarming', 'HEARTWARMING'],
  ['Historical', 'historical', 'HISTORICAL'],
  ['Horror', 'horror', 'HORROR'],
  ['Mystery', 'mystery', 'CRIME_MYSTERY'],
  ['Romance', 'romance', 'ROMANCE'],
  ['Romantic fantasy', null, 'ROMANTIC_FANTASY'],
  ['Sci-fi', 'sf', 'SF'],
  ['Slice of life', 'slice_of_life', 'SLICE_OF_LIFE'],
  ['Sports', 'sports', 'SPORTS'],
  ['Superhero', 'super_hero', 'SUPER_HERO'],
  ['Supernatural', 'supernatural', 'SUPERNATURAL'],
  ['Thriller', 'thriller', 'THRILLER'],
  ['Informative', 'tiptoon', 'TIPTOON'],
  ['All ages', null, 'ALL_AGES'],
  ['Animals', null, 'ANIMALS'],
  ['BL / GL', null, 'BL_GL'],
  ['Inspirational', null, 'INSPIRATIONAL'],
  ['Post-apocalyptic', null, 'POST_APOCALYPTIC'],
  ['School', null, 'SCHOOL'],
  ['Short story', null, 'SHORT_STORY'],
  ['Zombies', null, 'ZOMBIES'],
];
const genreValue = ([, originals, canvas]: (typeof GENRES)[number]) =>
  originals ?? canvas!.toLowerCase();

const SORTS = [
  { value: 'MANA', label: 'Popular' },
  { value: 'LIKEIT', label: 'Most liked' },
  { value: 'UPDATE', label: 'Latest episode' },
];

const filters = async (): Promise<SourceFilter[]> => [
  {
    kind: 'select',
    key: 'type',
    label: 'Series',
    default: 'all',
    options: [
      { value: 'all', label: 'Originals and Canvas' },
      { value: 'webtoon', label: 'Originals' },
      { value: 'canvas', label: 'Canvas' },
    ],
  },
  {
    kind: 'select',
    key: 'genre',
    label: 'Genre',
    default: 'any',
    options: [
      { value: 'any', label: 'Any' },
      ...GENRES.map((genre) => ({ value: genreValue(genre), label: genre[0] })),
    ],
  },
  { kind: 'select', key: 'sort', label: 'Sort by', default: 'MANA', options: SORTS },
];

const choice = (chosen: FilterValues | undefined, key: string, fallback: string) => {
  const value = chosen?.[key];
  return typeof value === 'string' ? value : fallback;
};

/** A page of the site's search, of Originals or of Canvas. */
const searchSection = async (
  lang: string,
  section: 'originals' | 'canvas',
  query: string,
  page: number,
): Promise<MangaPage> => {
  const html = await get(
    `${SITE}/${lang}/search/${section}?keyword=${encodeURIComponent(query)}&page=${page}`,
  );
  return { items: cardsIn(html), hasNextPage: linksPageAfter(html, page) };
};

/** A genre's Originals (one long list) or Canvas series (paged by the site). */
const genreSection = async (
  lang: string,
  kind: Kind,
  genre: (typeof GENRES)[number],
  sort: string,
  page: number,
): Promise<MangaPage> => {
  const [, originals, canvas] = genre;
  if (kind === 'webtoon') {
    if (!originals) return { items: [], hasNextPage: false };
    const cards = await cached(`genre:${lang}:${originals}:${sort}`, () =>
      cardsOf(`${SITE}/${lang}/genres/${originals}?sortOrder=${sort}`),
    );
    return slice(cards, page);
  }
  const html = await get(
    `${SITE}/${lang}/canvas/list?genreTab=${canvas ?? 'ALL'}&sortOrder=${sort}&page=${page}`,
  );
  return { items: cardsIn(html), hasNextPage: linksPageAfter(html, page) };
};

/** Originals first, then Canvas, a page of each at a time. */
const both = async (pages: [Promise<MangaPage>, Promise<MangaPage>]): Promise<MangaPage> => {
  const [originals, canvas] = await Promise.allSettled(pages);
  const answered = [originals, canvas].flatMap((result) =>
    result.status === 'fulfilled' ? [result.value] : [],
  );
  if (answered.length === 0 && originals.status === 'rejected') throw originals.reason;
  return {
    items: unique(answered.map((answer) => answer.items)),
    hasNextPage: answered.some((answer) => answer.hasNextPage),
  };
};

const search = async (query: string, page: number, chosen?: FilterValues): Promise<MangaPage> => {
  const lang = language();
  const at = Math.max(page, 1);
  const type = choice(chosen, 'type', 'all');
  const picked = choice(chosen, 'sort', 'MANA');
  const sort = SORTS.some((option) => option.value === picked) ? picked : 'MANA';
  const text = query.trim();
  if (text) {
    if (type === 'webtoon') return searchSection(lang, 'originals', text, at);
    if (type === 'canvas') return searchSection(lang, 'canvas', text, at);
    return both([
      searchSection(lang, 'originals', text, at),
      searchSection(lang, 'canvas', text, at),
    ]);
  }
  const genre = GENRES.find((option) => genreValue(option) === choice(chosen, 'genre', 'any'));
  if (genre) {
    if (type === 'webtoon' || type === 'canvas') {
      return genreSection(lang, type, genre, sort, at);
    }
    return both([
      genreSection(lang, 'webtoon', genre, sort, at),
      genreSection(lang, 'canvas', genre, sort, at),
    ]);
  }
  if (type === 'canvas') {
    const html = await get(`${SITE}/${lang}/canvas/list?genreTab=ALL&sortOrder=${sort}&page=${at}`);
    return { items: cardsIn(html), hasNextPage: linksPageAfter(html, at) };
  }
  if (sort === 'UPDATE') return slice(await latest(lang), at);
  const ranked = await rankings(lang);
  return slice(
    type === 'webtoon' ? ranked.filter((card) => card.id.includes(':webtoon:')) : ranked,
    at,
  );
};

// ---- A series ----

const STATUS_WORDS: [RegExp, MangaStatus][] = [
  // "COMPLETED", "TERMINÉ", "TAMAT", "จบแล้ว", "完結", "ABGESCHLOSSEN", "FINALIZADO".
  [/complete|termin|tamat|จบ|完結|abgeschlossen|finaliz/i, 'completed'],
  [/hiatus|pause/i, 'hiatus'],
  [/up|every|tous|cada|jeden|setiap|ทุก|每/i, 'ongoing'],
];

const textOf = (html: string, selector: string) =>
  raygun.html.select(html, selector)[0]?.text.replace(/\s+/g, ' ').trim() || null;

const metaOf = (html: string, property: string) =>
  raygun.html.select(html, `meta[property="${property}"]`)[0]?.attr('content')?.trim() || null;

const details = async (id: string): Promise<MangaDetails> => {
  const series = parseSeriesId(id);
  const { html, url } = await fetchPage(seriesUrl(series));
  // The heading breaks long names with <br>, which leaves no space in its text.
  const title =
    metaOf(html, 'og:title') ?? textOf(html, '.detail_header .info .subj') ?? 'Untitled';
  const authorArea = raygun.html.select(html, '.detail_header .author_area')[0];
  const authors = authorArea
    ? authorArea.text
        .replace(/author info/gi, '')
        .split(',')
        // A long list ends in "..." where the site cuts it short.
        .map((name) =>
          name
            .replace(/\s+/g, ' ')
            .trim()
            .replace(/\s*(?:\.{3}|…)$/, '')
            .trim(),
        )
        .filter(Boolean)
    : [];
  const genres = raygun.html
    .select(html, '.detail_header .info .genre')
    .map((genre) => genre.text.replace(/\s+/g, ' ').trim())
    .filter(Boolean);
  const schedule = textOf(html, '#_asideDetail .day_info') ?? '';
  const status = STATUS_WORDS.find(([words]) => words.test(schedule))?.[1] ?? 'unknown';
  const cover = metaOf(html, 'og:image');
  return {
    id,
    title,
    coverUrl: cover?.startsWith('http') ? fixUrl(cover) : null,
    coverHeaders: IMAGE_HEADERS,
    authors: [...new Set(authors)],
    artists: [],
    description: textOf(html, '#_asideDetail .summary'),
    genres: [...new Set(genres)],
    status,
    url: url.startsWith(SITE) ? url : seriesUrl(series),
  };
};

type EpisodeList = {
  result?: {
    episodeList?: {
      episodeNo: number;
      episodeTitle?: string;
      exposureDateMillis?: number;
    }[];
  };
};

const chapters = async (id: string): Promise<MangaChapter[]> => {
  const series = parseSeriesId(id);
  const extra = series.kind === 'canvas' ? `&readingLanguageCode=${series.lang}` : '';
  const response = await raygun.http.fetch(
    `${MOBILE}/api/v1/${series.kind}/${series.titleNo}/episodes?pageSize=99999${extra}`,
    { headers: { Referer: `${MOBILE}/` } },
  );
  if (!response.ok) throw new Error(`WEBTOON's episode list answered HTTP ${response.status}`);
  const episodes = (await response.json<EpisodeList>()).result?.episodeList ?? [];
  // The list runs from the first episode; the app wants the newest first.
  return episodes
    .filter((episode) => Number.isInteger(episode.episodeNo))
    .map((episode) => ({
      id: `${id}:${episode.episodeNo}`,
      name: episode.episodeTitle?.trim() || null,
      number: episode.episodeNo,
      dateUpload: episode.exposureDateMillis ?? null,
    }))
    .reverse();
};

const pages = async (chapterId: string) => {
  const cut = chapterId.lastIndexOf(':');
  const series = parseSeriesId(chapterId.slice(0, cut));
  const episode = chapterId.slice(cut + 1);
  if (!/^\d+$/.test(episode)) throw new Error(`WEBTOON has no episode ${chapterId}`);
  const section = series.kind === 'canvas' ? 'canvas' : 'series';
  const html = await get(
    `${SITE}/${series.lang}/${section}/series/episode/viewer?title_no=${series.titleNo}&episode_no=${episode}`,
  );
  // Each picture waits in `data-url` behind a transparent placeholder.
  const images = raygun.html
    .select(html, '#_imageList img')
    .map((image) => image.attr('data-url') ?? image.attr('src') ?? '')
    .filter((url) => url.startsWith('http') && !url.includes('/bg_transparency.'));
  if (images.length === 0) {
    throw new Error('WEBTOON shows this episode only on its website or app');
  }
  return images.map((url) => ({ url: fixUrl(url), headers: IMAGE_HEADERS }));
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'webtoons',
    name: 'WEBTOON',
    popular: async ({ page }) => slice(await rankings(language()), page),
    latest: async ({ page }) => slice(await latest(language()), page),
    search: ({ query, page, filters: chosen }) => search(query, page, chosen),
    filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    pages: async ({ chapterId }) => ({ pages: await pages(chapterId) }),
  }),
);
