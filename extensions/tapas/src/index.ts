// Tapas (tapas.io): comics by ranking, genre and search, with the episodes anyone may read for free.
// The others need coins or the app's wait-until-free timer, which need an account, so they're left
// out. Lists come from the story API the site's pages use, search and series details from its web
// pages, episodes from its episode list; pages are signed addresses that need no headers.
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

const SITE = 'https://tapas.io';
const API = 'https://story-api.tapas.io/cosmos/api/v1/landing';
const PAGE_SIZE = 25;
/** The episode list comes 20 to a page (it refuses more); this many pages are asked at once. */
const EPISODES_PER_PAGE = 20;
const PARALLEL_PAGES = 4;
/** Longer series stop at 1,000 episodes. */
const MAX_EPISODE_PAGES = 50;

type Series = {
  seriesId: number;
  title?: string;
  mature?: boolean;
  assetProperty?: {
    bookCoverImage?: { path?: string };
    thumbnailImage?: { path?: string };
  };
};
type Landing = { data?: { items?: Series[] }; meta?: { pagination?: { last?: boolean } } };

const coverOf = (series: Series) => {
  const path =
    series.assetProperty?.bookCoverImage?.path ?? series.assetProperty?.thumbnailImage?.path;
  // The bare address answers 404; the picture is there as JPEG (and as a far larger PNG).
  return path?.startsWith('https://') ? `${path}.jpg` : null;
};

const landing = async (
  path: 'ranking' | 'genre',
  params: Record<string, string | number>,
): Promise<MangaPage> => {
  const query = Object.entries({ category_type: 'COMIC', subtab_id: 17, ...params })
    .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
    .join('&');
  const response = await raygun.http.fetch(`${API}/${path}?${query}`);
  if (!response.ok) throw new Error(`Tapas answered HTTP ${response.status}`);
  const body = await response.json<Landing>();
  const items = (body.data?.items ?? [])
    .filter((series) => Number.isSafeInteger(series.seriesId) && series.title?.trim())
    .map((series): MangaSummary => ({
      id: String(series.seriesId),
      title: series.title!.trim(),
      coverUrl: coverOf(series),
      // Tapas shows these to signed-in adults only.
      nsfw: series.mature === true,
    }));
  return { items, hasNextPage: body.meta?.pagination?.last === false };
};

const ranking = (page: number) =>
  landing('ranking', { size: PAGE_SIZE, page: Math.max(page, 1) - 1 });

const byGenre = (page: number, sort: string, genre: string | null, price: string) =>
  landing('genre', {
    pageSize: PAGE_SIZE,
    page: Math.max(page, 1) - 1,
    sort_option: sort,
    bm_type: price,
    ...(genre ? { genre_type: genre } : {}),
  });

const GENRES = [
  { value: 'ROMANCE_FANTASY', label: 'Romance fantasy' },
  { value: 'ROMANCE', label: 'Romance' },
  { value: 'DRAMA', label: 'Drama' },
  { value: 'ACTION_FANTASY', label: 'Action fantasy' },
  { value: 'ACTION', label: 'Action' },
  { value: 'THRILLER_HORROR', label: 'Thriller and horror' },
  { value: 'BL', label: 'BL' },
  { value: 'GL', label: 'GL' },
  { value: 'LGBTQ', label: 'LGBTQ+' },
  { value: 'COMEDY', label: 'Comedy' },
  { value: 'FANTASY', label: 'Fantasy' },
];
const SORTS = [
  { value: 'POPULAR', label: 'Popular' },
  { value: 'NEWEST_EPISODE', label: 'Latest episode' },
  { value: 'NEWEST_SERIES', label: 'Newest series' },
];
const PRICES = [
  { value: 'ALL', label: 'Any' },
  { value: 'FREE', label: 'Free to read' },
  { value: 'WUF', label: 'Wait until free' },
];

const filters = async (): Promise<SourceFilter[]> => [
  {
    kind: 'select',
    key: 'genre',
    label: 'Genre',
    default: 'any',
    options: [{ value: 'any', label: 'Any' }, ...GENRES],
  },
  { kind: 'select', key: 'sort', label: 'Sort by', default: 'POPULAR', options: SORTS },
  { kind: 'select', key: 'price', label: 'Episodes', default: 'ALL', options: PRICES },
];

const pick = (chosen: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = chosen?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

/** Search marks the words it matched like `#_h_i_g_h_L_i_g_h_t_#word#/_h_i_g_h_L_i_g_h_t_#`. */
const unmarked = (text: string) =>
  text
    .replace(/#\/?_h_i_g_h_L_i_g_h_t_#/g, '')
    .replace(/\s+/g, ' ')
    .trim();

const searchPage = async (query: string, page: number): Promise<MangaPage> => {
  const response = await raygun.http.fetch(
    `${SITE}/search?q=${encodeURIComponent(query)}&t=COMICS&pageNumber=${Math.max(page, 1)}`,
  );
  if (!response.ok) throw new Error(`Tapas search answered HTTP ${response.status}`);
  const html = await response.text();
  const seen = new Set<string>();
  const items: MangaSummary[] = [];
  for (const result of raygun.html.select(html, '.search-item-wrap')) {
    const link = result.select('.title-section .title a')[0];
    const id = link?.attr('data-series-id') ?? '';
    const title = link ? unmarked(link.text) : '';
    if (!/^\d+$/.test(id) || !title || seen.has(id)) continue;
    seen.add(id);
    const image = result.select('.item-thumb-wrap img')[0]?.attr('src');
    items.push({ id, title, coverUrl: image?.startsWith('https://') ? image : null });
  }
  return {
    items,
    hasNextPage: raygun.html.select(html, 'a.paging__button--next').length > 0,
  };
};

const search = (query: string, page: number, chosen?: FilterValues) => {
  const text = query.trim();
  // The site's search knows no genres or prices; they sort and narrow the lists instead.
  if (text) return searchPage(text, page);
  return byGenre(
    page,
    pick(chosen, 'sort', SORTS) ?? 'POPULAR',
    pick(chosen, 'genre', GENRES),
    pick(chosen, 'price', PRICES) ?? 'ALL',
  );
};

// ---- A series ----

const seriesNumber = (id: string) => {
  if (!/^\d+$/.test(id)) throw new Error(`Tapas has no series ${id}`);
  return id;
};

const textsOf = (html: string, selector: string) => [
  ...new Set(
    raygun.html
      .select(html, selector)
      .map((element) => element.text.replace(/\s+/g, ' ').trim())
      .filter(Boolean),
  ),
];

const statusOf = (schedule: string[]): MangaStatus => {
  const words = schedule.join(' ');
  if (/complete/i.test(words)) return 'completed';
  if (/hiatus|break/i.test(words)) return 'hiatus';
  if (/update|every|daily|weekly/i.test(words)) return 'ongoing';
  return 'unknown';
};

const details = async (id: string): Promise<MangaDetails> => {
  const response = await raygun.http.fetch(`${SITE}/series/${seriesNumber(id)}/info`);
  if (response.status === 404) throw new Error('Tapas no longer has this series');
  if (!response.ok) throw new Error(`Tapas answered HTTP ${response.status}`);
  const html = await response.text();
  const title =
    textsOf(html, '.info__right .title-wrapper .title')[0] ??
    textsOf(html, '.info__right .title')[0] ??
    'Untitled';
  const cover = raygun.html.select(html, '.thumb.js-thumbnail img')[0]?.attr('src');
  return {
    id,
    title,
    coverUrl: cover?.startsWith('https://') ? cover : null,
    authors: textsOf(html, '.creator a.creator__item'),
    artists: [],
    description: textsOf(html, '.description__body')[0] ?? null,
    genres: textsOf(html, '.genre-btn-group a.genre-btn'),
    status: statusOf(textsOf(html, '.schedule-label')),
    url: response.url.startsWith(SITE) ? response.url : `${SITE}/series/${id}/info`,
  };
};

type Episode = {
  id: number;
  title?: string;
  scene?: number;
  publish_date?: string;
  free?: boolean;
  scheduled?: boolean;
};
type EpisodeList = {
  data?: { episodes?: Episode[]; pagination?: { has_next?: boolean; total?: number } };
};

const episodePage = async (id: string, page: number) => {
  const response = await raygun.http.fetch(
    `${SITE}/series/${id}/episodes?page=${page}&sort=NEWEST&large=true&last_access=0`,
    { headers: { Accept: 'application/json' } },
  );
  if (!response.ok) throw new Error(`Tapas's episode list answered HTTP ${response.status}`);
  return (await response.json<EpisodeList>()).data ?? {};
};

const chapters = async (id: string): Promise<MangaChapter[]> => {
  const series = seriesNumber(id);
  const first = await episodePage(series, 1);
  const lists = [first.episodes ?? []];
  const total = first.pagination?.total;
  if (typeof total === 'number') {
    const last = Math.min(Math.ceil(total / EPISODES_PER_PAGE), MAX_EPISODE_PAGES);
    for (let start = 2; start <= last; start += PARALLEL_PAGES) {
      const batch: Promise<Episode[]>[] = [];
      for (let page = start; page < start + PARALLEL_PAGES && page <= last; page += 1) {
        batch.push(episodePage(series, page).then((data) => data.episodes ?? []));
      }
      lists.push(...(await Promise.all(batch)));
    }
  } else {
    let more = first.pagination?.has_next === true;
    for (let page = 2; more && page <= MAX_EPISODE_PAGES; page += 1) {
      const next = await episodePage(series, page);
      lists.push(next.episodes ?? []);
      more = next.pagination?.has_next === true;
    }
  }
  const seen = new Set<number>();
  return lists
    .flat()
    .filter(
      (episode) =>
        Number.isSafeInteger(episode.id) &&
        !seen.has(episode.id) &&
        Boolean(seen.add(episode.id)) &&
        episode.free === true &&
        episode.scheduled !== true,
    )
    .map((episode) => {
      const date = episode.publish_date ? Date.parse(episode.publish_date) : NaN;
      return {
        id: String(episode.id),
        name: episode.title?.trim() || null,
        number: typeof episode.scene === 'number' ? episode.scene : null,
        dateUpload: Number.isFinite(date) ? date : null,
      };
    });
};

const pages = async (chapterId: string) => {
  if (!/^\d+$/.test(chapterId)) throw new Error(`Tapas has no episode ${chapterId}`);
  const response = await raygun.http.fetch(`${SITE}/episode/${chapterId}`);
  if (!response.ok) throw new Error(`Tapas answered HTTP ${response.status}`);
  const html = await response.text();
  // The `src` is a blank placeholder until the page scrolls to it; `data-src` is the picture.
  const images = raygun.html
    .select(html, 'img.content__img')
    .map((image) => image.attr('data-src') ?? image.attr('src') ?? '')
    .filter((url) => url.startsWith('https://'));
  if (images.length === 0) {
    throw new Error('Tapas keeps this episode for readers with coins or an account');
  }
  return images;
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'tapas',
    name: 'Tapas',
    popular: ({ page }) => ranking(page),
    latest: ({ page }) => byGenre(page, 'NEWEST_EPISODE', null, 'ALL'),
    search: ({ query, page, filters: chosen }) => search(query, page, chosen),
    filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    pages: async ({ chapterId }) => ({ pages: await pages(chapterId) }),
  }),
);
