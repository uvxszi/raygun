// MangaDex (M24): popular, latest and search from the public API, details with cover and authors,
// the chapter feed in the chosen language, and page images from MangaDex@Home servers.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { MangaChapter, MangaDetails, MangaPage, MangaStatus, MangaSummary } from '@raygun/sdk';

const API = 'https://api.mangadex.org';
// MangaDex answers browser-like clients with a bot challenge; it wants apps to say who they are.
const HEADERS = { 'User-Agent': 'Raygun/1.0 (+https://github.com/uvxszi/raygun)' };
const COVERS = 'https://uploads.mangadex.org/covers';
const PAGE_SIZE = 24;
// The feed allows up to 500 chapters per request; long series need several.
const FEED_PAGE = 500;
const MAX_FEED_PAGES = 10;
const RATINGS: Record<string, string[]> = {
  safe: ['safe'],
  'safe and suggestive': ['safe', 'suggestive'],
  everything: ['safe', 'suggestive', 'erotica', 'pornographic'],
};
const STATUSES: Record<string, MangaStatus> = {
  ongoing: 'ongoing',
  completed: 'completed',
  hiatus: 'hiatus',
  cancelled: 'cancelled',
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

const language = () =>
  (raygun.settings.get<string>('language') ?? 'en').trim().toLowerCase() || 'en';
const ratings = () =>
  RATINGS[raygun.settings.get<string>('contentRating') ?? ''] ?? RATINGS['safe and suggestive']!;

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
  if (!response.ok) throw new Error(`MangaDex answered HTTP ${response.status}`);
  return response.json<T>();
};

/** The title in the chosen language, else English, else any. */
const localized = (values: Localized | undefined, preferred: string): string | null => {
  if (!values) return null;
  return values[preferred] ?? values.en ?? Object.values(values)[0] ?? null;
};

const titleOf = (record: MangaRecord) => {
  const lang = language();
  const direct = localized(record.attributes.title, lang);
  const alt = record.attributes.altTitles?.find((title) => title[lang] ?? title.en);
  return (alt?.[lang] ?? direct ?? alt?.en ?? 'Untitled').trim();
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
});

const listMangas = async (
  order: [string, string],
  page: number,
  extra: [string, string | number][] = [],
): Promise<MangaPage> => {
  const offset = (Math.max(page, 1) - 1) * PAGE_SIZE;
  const result = await get<Listing<MangaRecord>>('/manga', [
    ['limit', PAGE_SIZE],
    ['offset', offset],
    ['includes[]', 'cover_art'],
    ['availableTranslatedLanguage[]', language()],
    ['hasAvailableChapters', 'true'],
    [`order[${order[0]}]`, order[1]],
    ...ratings().map((rating): [string, string] => ['contentRating[]', rating]),
    ...extra,
  ]);
  return {
    items: result.data.map(summary),
    hasNextPage: offset + result.data.length < result.total,
  };
};

const details = async (id: string): Promise<MangaDetails> => {
  const { data } = await get<{ data: MangaRecord }>(`/manga/${id}`, [
    ['includes[]', 'cover_art'],
    ['includes[]', 'author'],
    ['includes[]', 'artist'],
  ]);
  const names = (type: string) =>
    data.relationships
      .filter((relation) => relation.type === type)
      .map((relation) => relation.attributes?.name)
      .filter((name): name is string => Boolean(name));
  return {
    id: data.id,
    title: titleOf(data),
    coverUrl: coverOf(data),
    coverHeaders: HEADERS,
    authors: names('author'),
    artists: names('artist'),
    description: localized(data.attributes.description, language())?.trim() || null,
    genres: (data.attributes.tags ?? [])
      .filter((tag) => tag.attributes.group === 'genre' || tag.attributes.group === 'theme')
      .map((tag) => localized(tag.attributes.name, 'en'))
      .filter((name): name is string => Boolean(name)),
    status: STATUSES[data.attributes.status ?? ''] ?? 'unknown',
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
      ...RATINGS.everything!.map((rating): [string, string] => ['contentRating[]', rating]),
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

registerProvider(
  defineProvider({
    kind: 'manga.source',
    id: 'mangadex',
    name: 'MangaDex',
    popular: ({ page }) => listMangas(['followedCount', 'desc'], page),
    latest: ({ page }) => listMangas(['latestUploadedChapter', 'desc'], page),
    search: ({ query: text, page }) => listMangas(['relevance', 'desc'], page, [['title', text]]),
    details: ({ id }) => details(id),
    chapters: async ({ id }) => ({ chapters: await chapters(id) }),
    pages: async ({ chapterId }) => ({ pages: await pages(chapterId) }),
  }),
);
