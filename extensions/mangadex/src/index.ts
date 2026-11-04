// MangaDex (M24): popular, latest and search from the public API, details with cover and authors,
// the chapter feed in the chosen language, and page images from MangaDex@Home servers.
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

const API = 'https://api.mangadex.org';
// MangaDex answers browser-like clients with a bot challenge; it wants apps to say who they are.
const HEADERS = { 'User-Agent': 'Raygun/1.0 (+https://github.com/uvxszi/raygun)' };
const COVERS = 'https://uploads.mangadex.org/covers';
const PAGE_SIZE = 24;
/** MangaDex lists stop at offset + limit = 10,000. */
const MAX_RESULTS = 10_000;
// The feed allows up to 500 chapters per request; long series need several.
const FEED_PAGE = 500;
const MAX_FEED_PAGES = 10;
/** Latest updates read this many new chapters a page and show the series they belong to. */
const LATEST_CHAPTERS = 100;
const RATINGS: Record<string, string[]> = {
  safe: ['safe'],
  'safe and suggestive': ['safe', 'suggestive'],
  everything: ['safe', 'suggestive', 'erotica', 'pornographic'],
};
/** MangaDex's adult ratings, left out while the app's NSFW switch is off. */
const ADULT = ['erotica', 'pornographic'];
const STATUSES: Record<string, MangaStatus> = {
  ongoing: 'ongoing',
  completed: 'completed',
  hiatus: 'hiatus',
  cancelled: 'cancelled',
};
const DEMOGRAPHICS: Record<string, string> = {
  shounen: 'Shounen',
  shoujo: 'Shoujo',
  seinen: 'Seinen',
  josei: 'Josei',
};

/** The languages chapters are translated into, by the names the setting shows. */
const LANGUAGES: Record<string, string> = {
  English: 'en',
  'Spanish (Latin America)': 'es-la',
  'Spanish (Spain)': 'es',
  'Portuguese (Brazil)': 'pt-br',
  'Portuguese (Portugal)': 'pt',
  French: 'fr',
  German: 'de',
  Italian: 'it',
  Russian: 'ru',
  Ukrainian: 'uk',
  Polish: 'pl',
  Czech: 'cs',
  Hungarian: 'hu',
  Romanian: 'ro',
  Turkish: 'tr',
  Arabic: 'ar',
  Persian: 'fa',
  Hebrew: 'he',
  Indonesian: 'id',
  Malay: 'ms',
  Filipino: 'tl',
  Vietnamese: 'vi',
  Thai: 'th',
  Japanese: 'ja',
  Korean: 'ko',
  'Chinese (Simplified)': 'zh',
  'Chinese (Traditional)': 'zh-hk',
};

type Localized = Record<string, string>;
type Relationship = {
  id: string;
  type: string;
  attributes?: { fileName?: string; name?: string } | null;
};
type MangaRecord = {
  id: string;
  attributes: {
    title: Localized;
    altTitles?: Localized[];
    description?: Localized;
    status?: string;
    contentRating?: string;
    publicationDemographic?: string | null;
    lastChapter?: string | null;
    lastVolume?: string | null;
    tags?: { attributes: { name: Localized; group: string } }[];
  };
  relationships: Relationship[];
};
type ChapterRecord = {
  id: string;
  attributes: {
    volume: string | null;
    chapter: string | null;
    title: string | null;
    publishAt?: string;
    pages?: number;
    externalUrl?: string | null;
  };
  relationships: Relationship[];
};
type Listing<T> = { data: T[]; total: number; offset: number; limit: number };

/** The chosen language's code; an older setting held the code itself ("es-la"). */
const language = () => {
  const chosen = (raygun.settings.get<string>('language') ?? '').trim();
  if (LANGUAGES[chosen]) return LANGUAGES[chosen]!;
  const code = chosen.toLowerCase();
  return /^[a-z]{2}(-[a-z]{2})?$/.test(code) ? code : 'en';
};
/** The setting's choice ("Safe and suggestive"; older versions stored it in lower case). */
const ratings = () => {
  const choice = (raygun.settings.get<string>('contentRating') ?? '').trim().toLowerCase();
  const chosen = RATINGS[choice] ?? RATINGS['safe and suggestive']!;
  return raygun.content.showNsfw() ? chosen : chosen.filter((rating) => !ADULT.includes(rating));
};
const ratingParams = (list: string[]) =>
  list.map((rating): [string, string] => ['contentRating[]', rating]);
const isAdult = (record: MangaRecord) => ADULT.includes(record.attributes.contentRating ?? '');

const query = (params: [string, string | number][]) =>
  params
    .map(([key, value]) => `${encodeURIComponent(key)}=${encodeURIComponent(String(value))}`)
    .join('&');

const get = async <T>(path: string, params: [string, string | number][]): Promise<T> => {
  const response = await raygun.http.fetch(`${API}${path}?${query(params)}`, {
    headers: HEADERS,
  });
  if (response.status === 429)
    throw new Error('MangaDex is rate limiting requests; try again shortly');
  if (response.status === 404) throw new Error('MangaDex no longer has this title or chapter');
  if (!response.ok) throw new Error(`MangaDex answered HTTP ${response.status}`);
  return response.json<T>();
};

/** The text in the chosen language, else English, else any. */
const localized = (values: Localized | undefined, preferred: string): string | null => {
  if (!values) return null;
  return values[preferred] ?? values.en ?? Object.values(values)[0] ?? null;
};

/**
 * The title in the chosen language (main, then alternative titles), else in English, else the
 * main title as MangaDex has it (often romanized Japanese).
 */
const titleOf = (record: MangaRecord) => {
  const main = record.attributes.title ?? {};
  const alternatives = record.attributes.altTitles ?? [];
  const inLanguage = (code: string) =>
    main[code] ?? alternatives.find((title) => title[code])?.[code];
  const lang = language();
  const title =
    inLanguage(lang) ?? (lang === 'en' ? undefined : inLanguage('en')) ?? Object.values(main)[0];
  return title?.trim() || 'Untitled';
};

const coverOf = (record: MangaRecord) => {
  const file = record.relationships.find((relation) => relation.type === 'cover_art')?.attributes
    ?.fileName;
  return file ? `${COVERS}/${record.id}/${file}.512.jpg` : null;
};

const summary = (record: MangaRecord): MangaSummary => ({
  id: record.id,
  title: titleOf(record),
  coverUrl: coverOf(record),
  coverHeaders: HEADERS,
  nsfw: isAdult(record),
});

const listMangas = async (
  order: [string, string],
  page: number,
  extra: [string, string | number][] = [],
): Promise<MangaPage> => {
  const offset = (Math.max(page, 1) - 1) * PAGE_SIZE;
  if (offset + PAGE_SIZE > MAX_RESULTS) return { items: [], hasNextPage: false };
  const result = await get<Listing<MangaRecord>>('/manga', [
    ['limit', PAGE_SIZE],
    ['offset', offset],
    ['includes[]', 'cover_art'],
    ['availableTranslatedLanguage[]', language()],
    ['hasAvailableChapters', 'true'],
    [`order[${order[0]}]`, order[1]],
    ...ratingParams(ratings()),
    ...extra,
  ]);
  const next = offset + result.data.length;
  return {
    items: result.data.map(summary),
    hasNextPage: next < result.total && next + PAGE_SIZE <= MAX_RESULTS,
  };
};

/** The series of a latest page, so later pages leave out the ones already shown. */
const latestShown = new Map<string, string[]>();

/**
 * Series with new chapters in the chosen language, the newest first (as Mihon's latest updates):
 * MangaDex's own "latest upload" order counts chapters in every language.
 */
const latest = async (page: number): Promise<MangaPage> => {
  const at = Math.max(page, 1);
  const offset = (at - 1) * LATEST_CHAPTERS;
  if (offset + LATEST_CHAPTERS > MAX_RESULTS) return { items: [], hasNextPage: false };
  const lang = language();
  const allowed = ratings();
  const feed = await get<Listing<ChapterRecord>>('/chapter', [
    ['limit', LATEST_CHAPTERS],
    ['offset', offset],
    ['translatedLanguage[]', lang],
    ['order[readableAt]', 'desc'],
    ['includeFutureUpdates', '0'],
    ['includeExternalUrl', '0'],
    ['includeEmptyPages', '0'],
    ...ratingParams(allowed),
  ]);
  const key = `${lang}|${allowed.join(',')}`;
  if (at === 1) {
    for (const shown of [...latestShown.keys()]) {
      if (shown.startsWith(`${key}|`)) latestShown.delete(shown);
    }
  }
  const earlier = new Set<string>();
  for (let before = 1; before < at; before += 1) {
    for (const id of latestShown.get(`${key}|${before}`) ?? []) earlier.add(id);
  }
  const ids = [
    ...new Set(
      feed.data.flatMap((chapter) =>
        chapter.relationships
          .filter((relation) => relation.type === 'manga')
          .map((relation) => relation.id),
      ),
    ),
  ].filter((id) => !earlier.has(id));
  latestShown.set(`${key}|${at}`, ids);
  const next = offset + feed.data.length;
  const hasNextPage = next < feed.total && next + LATEST_CHAPTERS <= MAX_RESULTS;
  if (ids.length === 0) return { items: [], hasNextPage };
  const series = await get<Listing<MangaRecord>>('/manga', [
    ['limit', ids.length],
    ...ids.map((id): [string, string] => ['ids[]', id]),
    ['includes[]', 'cover_art'],
    ...ratingParams(allowed),
  ]);
  const byId = new Map(series.data.map((record) => [record.id, record]));
  return {
    items: ids.flatMap((id) => {
      const record = byId.get(id);
      return record ? [summary(record)] : [];
    }),
    hasNextPage,
  };
};

type Aggregate = {
  volumes?: Record<string, { volume: string; chapters?: Record<string, { chapter: string }> }>;
};

/**
 * MangaDex calls a series completed when its publication ended; whether the translation reached
 * the end too comes from the chapters there are in the chosen language.
 */
const completion = async (record: MangaRecord): Promise<MangaStatus> => {
  const { lastChapter, lastVolume } = record.attributes;
  if (!lastChapter && !lastVolume) return 'completed';
  try {
    const aggregate = await get<Aggregate>(`/manga/${record.id}/aggregate`, [
      ['translatedLanguage[]', language()],
    ]);
    const volumes = Object.values(aggregate.volumes ?? {});
    const chapters = new Set(
      volumes.flatMap((volume) =>
        Object.values(volume.chapters ?? {}).map((chapter) => chapter.chapter),
      ),
    );
    const reached = lastChapter
      ? chapters.has(lastChapter)
      : volumes.some((volume) => volume.volume === lastVolume);
    return reached ? 'completed' : 'publishingFinished';
  } catch {
    return 'completed';
  }
};

const details = async (id: string): Promise<MangaDetails> => {
  const { data } = await get<{ data: MangaRecord }>(`/manga/${id}`, [
    ['includes[]', 'cover_art'],
    ['includes[]', 'author'],
    ['includes[]', 'artist'],
  ]);
  const names = (type: string) => [
    ...new Set(
      data.relationships
        .filter((relation) => relation.type === type)
        .map((relation) => relation.attributes?.name?.trim())
        .filter((name): name is string => Boolean(name)),
    ),
  ];
  const status = STATUSES[data.attributes.status ?? ''] ?? 'unknown';
  const demographic = DEMOGRAPHICS[data.attributes.publicationDemographic ?? ''];
  return {
    id: data.id,
    title: titleOf(data),
    coverUrl: coverOf(data),
    coverHeaders: HEADERS,
    authors: names('author'),
    artists: names('artist'),
    description: localized(data.attributes.description, language())?.trim() || null,
    genres: [
      ...(demographic ? [demographic] : []),
      ...(data.attributes.tags ?? [])
        .filter((tag) => tag.attributes.group === 'genre' || tag.attributes.group === 'theme')
        .map((tag) => localized(tag.attributes.name, 'en'))
        .filter((name): name is string => Boolean(name)),
      // The app hides titles with an adult genre while NSFW content is off.
      ...(isAdult(data)
        ? [data.attributes.contentRating === 'erotica' ? 'Erotica' : 'Pornographic']
        : []),
    ],
    status: status === 'completed' ? await completion(data) : status,
    url: `https://mangadex.org/title/${data.id}`,
  };
};

const chapters = async (id: string): Promise<MangaChapter[]> => {
  const found: MangaChapter[] = [];
  for (let page = 0; page < MAX_FEED_PAGES; page += 1) {
    const result = await get<Listing<ChapterRecord>>(`/manga/${id}/feed`, [
      ['limit', FEED_PAGE],
      ['offset', page * FEED_PAGE],
      ['translatedLanguage[]', language()],
      ['order[volume]', 'desc'],
      ['order[chapter]', 'desc'],
      ['includes[]', 'scanlation_group'],
      // Chapters a group holds back until later, hosted elsewhere or without pages can't be read.
      ['includeFuturePublishAt', '0'],
      ['includeExternalUrl', '0'],
      ['includeEmptyPages', '0'],
      ...ratingParams(RATINGS.everything!),
    ]);
    for (const record of result.data) {
      // Chapters hosted elsewhere (MangaPlus and the like) have no pages here.
      if (record.attributes.externalUrl || record.attributes.pages === 0) continue;
      const number = record.attributes.chapter === null ? null : Number(record.attributes.chapter);
      const label = record.attributes.chapter ? `Chapter ${record.attributes.chapter}` : 'Oneshot';
      const title = record.attributes.title?.trim();
      found.push({
        id: record.id,
        name: title ? `${label}: ${title}` : label,
        number: number !== null && Number.isFinite(number) ? number : null,
        volume: record.attributes.volume,
        scanlator:
          record.relationships
            .filter((relation) => relation.type === 'scanlation_group')
            .map((relation) => relation.attributes?.name)
            .filter(Boolean)
            .join(', ') || null,
        dateUpload: record.attributes.publishAt ? Date.parse(record.attributes.publishAt) : null,
      });
    }
    if ((page + 1) * FEED_PAGE >= result.total) break;
  }
  return found;
};

const pages = async (chapterId: string) => {
  const server = await get<{
    baseUrl: string;
    chapter: { hash: string; data: string[]; dataSaver: string[] };
  }>(`/at-home/server/${chapterId}`, []);
  const saver = raygun.settings.get<boolean>('dataSaver') === true;
  const files = saver ? server.chapter.dataSaver : server.chapter.data;
  const folder = saver ? 'data-saver' : 'data';
  return files.map((file) => ({
    url: `${server.baseUrl}/${folder}/${server.chapter.hash}/${file}`,
    headers: HEADERS,
  }));
};

// ---- Search filters (M40) ----

const SORTS: Record<string, [string, string]> = {
  relevance: ['relevance', 'desc'],
  popular: ['followedCount', 'desc'],
  latest: ['latestUploadedChapter', 'desc'],
  rating: ['rating', 'desc'],
  newest: ['createdAt', 'desc'],
  title: ['title', 'asc'],
};
type TagRecord = { id: string; attributes: { name: Localized; group: string } };
type Tag = { value: string; label: string; group: string };
/** MangaDex's tags change rarely; they're fetched once per session. */
let tagCache: Promise<Tag[]> | null = null;

const tags = () => {
  if (!tagCache) {
    const groupOrder = ['genre', 'theme', 'format', 'content'];
    tagCache = get<{ data: TagRecord[] }>('/manga/tag', []).then((result) =>
      result.data
        .map((tag) => ({
          value: tag.id,
          label: localized(tag.attributes.name, 'en') ?? tag.id,
          group: tag.attributes.group,
        }))
        .sort(
          (a, b) =>
            groupOrder.indexOf(a.group) - groupOrder.indexOf(b.group) ||
            a.label.localeCompare(b.label),
        ),
    );
    tagCache.catch(() => {
      tagCache = null;
    });
  }
  return tagCache;
};

const tagFilter = async (): Promise<SourceFilter> => ({
  kind: 'tags',
  key: 'tags',
  label: 'Tags',
  // Content tags (gore, sexual violence) are adult-leaning; leave them out unless NSFW is on.
  options: (await tags())
    .filter((tag) => tag.group !== 'content' || raygun.content.showNsfw())
    .map(({ value, label }) => ({ value, label })),
});

const filters = async (): Promise<SourceFilter[]> => [
  {
    kind: 'select',
    key: 'sort',
    label: 'Sort by',
    default: 'relevance',
    options: [
      { value: 'relevance', label: 'Best match' },
      { value: 'popular', label: 'Most followed' },
      { value: 'latest', label: 'Latest upload' },
      { value: 'rating', label: 'Top rated' },
      { value: 'newest', label: 'Recently added' },
      { value: 'title', label: 'Title' },
    ],
  },
  {
    kind: 'select',
    key: 'status',
    label: 'Status',
    default: 'any',
    options: [
      { value: 'any', label: 'Any' },
      { value: 'ongoing', label: 'Ongoing' },
      { value: 'completed', label: 'Completed' },
      { value: 'hiatus', label: 'Hiatus' },
      { value: 'cancelled', label: 'Cancelled' },
    ],
  },
  {
    kind: 'select',
    key: 'demographic',
    label: 'Demographic',
    default: 'any',
    options: [
      { value: 'any', label: 'Any' },
      { value: 'shounen', label: 'Shounen' },
      { value: 'shoujo', label: 'Shoujo' },
      { value: 'seinen', label: 'Seinen' },
      { value: 'josei', label: 'Josei' },
    ],
  },
  {
    kind: 'select',
    key: 'origin',
    label: 'Original language',
    default: 'any',
    options: [
      { value: 'any', label: 'Any' },
      { value: 'ja', label: 'Japanese (manga)' },
      { value: 'ko', label: 'Korean (manhwa)' },
      { value: 'zh', label: 'Chinese (manhua)' },
      { value: 'en', label: 'English' },
    ],
  },
  await tagFilter(),
];

const search = (text: string, page: number, chosen: FilterValues = {}) => {
  const extra: [string, string][] = [];
  if (text.trim()) extra.push(['title', text.trim()]);
  const status = chosen.status;
  if (typeof status === 'string' && status !== 'any') extra.push(['status[]', status]);
  const demographic = chosen.demographic;
  if (typeof demographic === 'string' && demographic !== 'any') {
    extra.push(['publicationDemographic[]', demographic]);
  }
  const origin = chosen.origin;
  if (typeof origin === 'string' && origin !== 'any') {
    extra.push(['originalLanguage[]', origin]);
    // Chinese series come from the mainland and from Hong Kong.
    if (origin === 'zh') extra.push(['originalLanguage[]', 'zh-hk']);
  }
  const chosenTags = chosen.tags;
  if (chosenTags && typeof chosenTags === 'object') {
    for (const id of chosenTags.include) extra.push(['includedTags[]', id]);
    for (const id of chosenTags.exclude) extra.push(['excludedTags[]', id]);
  }
  // "Best match" needs words to match; without them it's the most followed.
  const sort =
    typeof chosen.sort === 'string' && (text.trim() || chosen.sort !== 'relevance')
      ? chosen.sort
      : '';
  return listMangas(SORTS[sort] ?? SORTS[text.trim() ? 'relevance' : 'popular']!, page, extra);
};

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'mangadex',
    name: 'MangaDex',
    popular: ({ page }) => listMangas(['followedCount', 'desc'], page),
    latest: ({ page }) => latest(page),
    search: ({ query: text, page, filters: chosen }) => search(text, page, chosen),
    filters,
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    pages: async ({ chapterId }) => ({ pages: await pages(chapterId) }),
  }),
);
