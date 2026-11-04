// Naver Webtoon (comic.naver.com): Korea's largest webtoon service, in Korean. Its professional
// series (the weekly ones and the finished ones) are free to read except for paid early episodes,
// which are left out, and series for grown-ups, which need a signed-in adult and are marked so the
// app hides them while NSFW content is off. Everything comes from the JSON API the site's own pages
// use; pictures want the site as Referer.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type {
  FilterValues,
  MangaChapter,
  MangaDetails,
  MangaPage,
  MangaStatus,
  MangaSummary,
  SourceFilter,
} from '@raygun/sdk';

const SITE = 'https://comic.naver.com';
const HEADERS = { Referer: `${SITE}/` };
const PAGE_SIZE = 30;
/** The weekday lists change a few times a day; they're read again after this long. */
const LIST_TTL_MS = 10 * 60_000;
/** Episodes come 20 to a page; this many pages are asked at once, up to 1,200 episodes. */
const PARALLEL_PAGES = 4;
const MAX_EPISODE_PAGES = 60;
const DAYS = ['MONDAY', 'TUESDAY', 'WEDNESDAY', 'THURSDAY', 'FRIDAY', 'SATURDAY', 'SUNDAY'];

type Title = { titleId: number; titleName?: string; thumbnailUrl?: string; adult?: boolean };
type PageInfo = { page?: number; totalPages?: number };
type TitleList = { titleList?: Title[]; pageInfo?: PageInfo };
type SearchList = { searchList?: Title[]; pageInfo?: PageInfo };
type Weekday = { titleListMap?: Record<string, Title[]>; dayOfWeek?: string };

const api = async <T>(path: string): Promise<T> => {
  const response = await raygun.http.fetch(`${SITE}/api${path}`, { headers: HEADERS });
  if (response.status === 404) throw new Error('Naver Webtoon no longer has this series');
  if (!response.ok) throw new Error(`Naver Webtoon answered HTTP ${response.status}`);
  return response.json<T>();
};

const summary = (title: Title): MangaSummary | null =>
  Number.isSafeInteger(title.titleId) && title.titleName?.trim()
    ? {
        id: String(title.titleId),
        title: title.titleName.trim(),
        coverUrl: title.thumbnailUrl?.startsWith('https://') ? title.thumbnailUrl : null,
        coverHeaders: HEADERS,
        // Naver shows these to signed-in adults only.
        nsfw: title.adult === true,
      }
    : null;

const summaries = (titles: Title[] | undefined) => {
  const seen = new Set<string>();
  return (titles ?? []).flatMap((title) => {
    const item = summary(title);
    if (!item || seen.has(item.id)) return [];
    seen.add(item.id);
    return [item];
  });
};

const paged = (titles: Title[] | undefined, info: PageInfo | undefined): MangaPage => ({
  items: summaries(titles),
  hasNextPage: (info?.page ?? 1) < (info?.totalPages ?? 0),
});

const slice = (items: MangaSummary[], page: number): MangaPage => {
  const start = (Math.max(page, 1) - 1) * PAGE_SIZE;
  return {
    items: items.slice(start, start + PAGE_SIZE),
    hasNextPage: start + PAGE_SIZE < items.length,
  };
};

const remembered = new Map<string, { at: number; value: Promise<Weekday> }>();

/** Every weekday's series in `order` (`user` = popular, `update` = newest episode first). */
const weekdays = (order: 'user' | 'update') => {
  const hit = remembered.get(order);
  if (hit && Date.now() - hit.at < LIST_TTL_MS) return hit.value;
  const value = api<Weekday>(`/webtoon/titlelist/weekday?order=${order}`);
  remembered.set(order, { at: Date.now(), value });
  value.catch(() => remembered.delete(order));
  return value;
};

/** The weekly series by popularity: each day's most popular in turn, today's first. */
const popular = async (page: number) => {
  const lists = await weekdays('user');
  const today = Math.max(DAYS.indexOf(lists.dayOfWeek ?? ''), 0);
  const days = DAYS.map((_, index) => lists.titleListMap?.[DAYS[(today + index) % 7]!] ?? []);
  const longest = Math.max(0, ...days.map((day) => day.length));
  const interleaved: Title[] = [];
  for (let rank = 0; rank < longest; rank += 1) {
    for (const day of days) {
      const title = day[rank];
      if (title) interleaved.push(title);
    }
  }
  return slice(summaries(interleaved), page);
};

/** Today's series, the newest episode first. */
const latest = async (page: number) => {
  const lists = await weekdays('update');
  return slice(summaries(lists.titleListMap?.[lists.dayOfWeek ?? ''] ?? []), page);
};

const GENRES = [
  { value: 'PURE', label: 'Romance' },
  { value: 'FANTASY', label: 'Fantasy' },
  { value: 'ACTION', label: 'Action' },
  { value: 'DAILY', label: 'Daily life' },
  { value: 'THRILL', label: 'Thriller' },
  { value: 'COMIC', label: 'Comedy' },
  { value: 'DRAMA', label: 'Drama' },
  { value: 'SENSIBILITY', label: 'Emotional' },
  { value: 'HISTORICAL', label: 'Historical' },
  { value: 'SPORTS', label: 'Sports' },
];
const SORTS = [
  { value: 'USER', label: 'Popular' },
  { value: 'UPDATE', label: 'Latest episode' },
  { value: 'VIEW', label: 'Most viewed' },
];
const STATUSES = [
  { value: 'any', label: 'Any' },
  { value: 'finished', label: 'Finished' },
];

const filters = async (): Promise<SourceFilter[]> => [
  {
    kind: 'select',
    key: 'genre',
    label: 'Genre',
    default: 'any',
    options: [{ value: 'any', label: 'Any' }, ...GENRES],
  },
  { kind: 'select', key: 'status', label: 'Status', default: 'any', options: STATUSES },
  { kind: 'select', key: 'sort', label: 'Sort by', default: 'USER', options: SORTS },
];

const pick = (chosen: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = chosen?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

const search = async (query: string, page: number, chosen?: FilterValues): Promise<MangaPage> => {
  const at = Math.max(page, 1);
  const text = query.trim();
  if (text) {
    const found = await api<SearchList>(
      `/search/webtoon?keyword=${encodeURIComponent(text)}&page=${at}`,
    );
    return paged(found.searchList, found.pageInfo);
  }
  const sort = pick(chosen, 'sort', SORTS) ?? 'USER';
  const genre = pick(chosen, 'genre', GENRES);
  if (genre) {
    const found = await api<TitleList>(
      `/webtoon/titlelist/genre?genre=${genre}&order=${sort}&page=${at}`,
    );
    return paged(found.titleList, found.pageInfo);
  }
  if (pick(chosen, 'status', STATUSES) === 'finished') {
    const found = await api<TitleList>(`/webtoon/titlelist/finished?order=${sort}&page=${at}`);
    return paged(found.titleList, found.pageInfo);
  }
  return sort === 'UPDATE' ? latest(at) : popular(at);
};

// ---- A series ----

type Info = {
  titleName?: string;
  thumbnailUrl?: string;
  posterThumbnailUrl?: string;
  synopsis?: string;
  finished?: boolean;
  rest?: boolean;
  age?: { type?: string };
  communityArtists?: { name?: string; artistTypeList?: string[] }[];
  curationTagList?: { tagName?: string; curationType?: string }[];
};

const titleNumber = (id: string) => {
  if (!/^\d+$/.test(id)) throw new Error(`Naver Webtoon has no series ${id}`);
  return id;
};

const details = async (id: string): Promise<MangaDetails> => {
  const info = await api<Info>(`/article/list/info?titleId=${titleNumber(id)}`);
  const artists = info.communityArtists ?? [];
  const named = (role: string) => [
    ...new Set(
      artists
        .filter((artist) => artist.artistTypeList?.includes(role))
        .map((artist) => artist.name?.trim() ?? '')
        .filter(Boolean),
    ),
  ];
  const tags = info.curationTagList ?? [];
  // Genre tags first, then the site's own tags (themes, studios).
  const genres = [
    ...tags.filter((tag) => tag.curationType?.startsWith('GENRE_')),
    ...tags.filter((tag) => !tag.curationType?.startsWith('GENRE_')),
  ]
    .map((tag) => tag.tagName?.trim() ?? '')
    .filter(Boolean)
    .slice(0, 10);
  const status: MangaStatus = info.finished ? 'completed' : info.rest ? 'hiatus' : 'ongoing';
  const cover = info.posterThumbnailUrl ?? info.thumbnailUrl;
  return {
    id,
    title: info.titleName?.trim() || 'Untitled',
    coverUrl: cover?.startsWith('https://') ? cover : null,
    coverHeaders: HEADERS,
    authors: [...named('ARTIST_WRITER'), ...named('ARTIST_NOVEL_ORIGIN')],
    artists: named('ARTIST_PAINTER'),
    description: info.synopsis?.trim() || null,
    // The app hides titles with an adult genre while NSFW content is off.
    genres: [...new Set([...genres, ...(info.age?.type === 'RATE_18' ? ['Adult'] : [])])],
    status,
    url: `${SITE}/webtoon/list?titleId=${id}`,
  };
};

type Article = {
  no: number;
  subtitle?: string;
  charge?: boolean;
  serviceDateDescription?: string;
};
type ArticleList = { articleList?: Article[]; pageInfo?: PageInfo };

/** "26.06.09" (year, month, day); recent episodes may say "3일 전" instead, and get no date. */
const dateOf = (text: string | undefined) => {
  const day = /^(\d{2})\.(\d{2})\.(\d{2})$/.exec(text?.trim() ?? '');
  return day ? Date.UTC(2000 + Number(day[1]), Number(day[2]) - 1, Number(day[3])) : null;
};

const chapters = async (id: string): Promise<MangaChapter[]> => {
  const series = titleNumber(id);
  const pageOf = (page: number) =>
    api<ArticleList>(`/article/list?titleId=${series}&page=${page}`).then(
      (list) => list.articleList ?? [],
    );
  const first = await api<ArticleList>(`/article/list?titleId=${series}&page=1`);
  const lists = [first.articleList ?? []];
  const last = Math.min(first.pageInfo?.totalPages ?? 1, MAX_EPISODE_PAGES);
  for (let start = 2; start <= last; start += PARALLEL_PAGES) {
    const batch: Promise<Article[]>[] = [];
    for (let page = start; page < start + PARALLEL_PAGES && page <= last; page += 1) {
      batch.push(pageOf(page));
    }
    lists.push(...(await Promise.all(batch)));
  }
  const seen = new Set<number>();
  // Newest first, as the list comes; paid early episodes are left out.
  return lists.flat().flatMap((article) => {
    if (!Number.isSafeInteger(article.no) || article.charge || seen.has(article.no)) return [];
    seen.add(article.no);
    return [
      {
        id: `${series}:${article.no}`,
        name: article.subtitle?.trim() || null,
        number: article.no,
        dateUpload: dateOf(article.serviceDateDescription),
      },
    ];
  });
};

const pages = async (chapterId: string) => {
  const [series = '', episode = ''] = chapterId.split(':');
  if (!/^\d+$/.test(series) || !/^\d+$/.test(episode)) {
    throw new Error(`Naver Webtoon has no episode ${chapterId}`);
  }
  let html: string;
  try {
    const response = await raygun.http.fetch(
      `${SITE}/webtoon/detail?titleId=${series}&no=${episode}`,
      { headers: HEADERS },
    );
    if (!response.ok) throw new Error(`Naver Webtoon answered HTTP ${response.status}`);
    html = await response.text();
  } catch (error) {
    // Series for grown-ups send readers to Naver's sign-in page.
    if (String(error).includes('not permitted')) {
      throw new Error('Naver Webtoon shows this episode only to signed-in adults', {
        cause: error,
      });
    }
    throw error;
  }
  // The episode's pictures are `content_image_<n>`; other viewers keep theirs without ids.
  const inViewer = raygun.html.select(html, '#sectionContWide img');
  const numbered = inViewer.filter((image) =>
    (image.attr('id') ?? '').startsWith('content_image_'),
  );
  const images = (numbered.length > 0 ? numbered : inViewer)
    .map((image) => image.attr('src') ?? '')
    .filter((url) => url.startsWith('https://image-comic.pstatic.net/'));
  if (images.length === 0) {
    throw new Error('Naver Webtoon shows this episode only in its app or to signed-in readers');
  }
  return images.map((url) => ({ url, headers: HEADERS }));
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'naver',
    // Not Mihon's "Naver Webtoon": its backups name series by addresses whose id is a query
    // parameter, which a Mihon import can't carry over, so they're skipped rather than broken.
    name: 'Naver Webtoon (Korean)',
    popular: ({ page }) => popular(page),
    latest: ({ page }) => latest(page),
    search: ({ query, page, filters: chosen }) => search(query, page, chosen),
    filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    pages: async ({ chapterId }) => ({ pages: await pages(chapterId) }),
  }),
);
