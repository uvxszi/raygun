// NASA Image and Video Library (images.nasa.gov): NASA's videos through the library's public
// API, no key needed. NASA's own media are not under copyright in the United States; videos whose
// description names another rights holder are left out. Popular is the library's own list of its
// most viewed media; latest is this year's and last year's videos. A video plays from the
// library's MP4 encodes (1080p down to 180p, and the original), with its captions.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type {
  FilterValues,
  MangaPage,
  MangaSummary,
  SourceFilter,
  Video,
  VideoSubtitle,
} from '@raygun/sdk';

const API = 'https://images-api.nasa.gov';
const ASSETS = 'https://images-assets.nasa.gov';
const PAGE_SIZE = 30;

type ItemData = {
  nasa_id: string;
  title?: string;
  description?: string;
  date_created?: string;
  center?: string;
  keywords?: string[];
  photographer?: string;
  secondary_creator?: string;
  media_type?: string;
};
type ItemLink = { href: string; rel?: string; render?: string; width?: number };
type Item = { data?: ItemData[]; links?: ItemLink[] };
type Collection = {
  collection?: { items?: Item[]; metadata?: { total_hits?: number }; links?: { rel?: string }[] };
};
type Assets = { collection?: { items?: { href: string }[] } };
type FileMetadata = Record<string, unknown>;

/** The library's MP4 encodes by name, with their usual heights (a 16:9 video's). */
const ENCODES = [
  { name: 'large', height: 1080 },
  { name: 'medium', height: 720 },
  { name: 'small', height: 360 },
  { name: 'preview', height: 270 },
  { name: 'mobile', height: 180 },
];
/** Another rights holder named in a video's credits or description. */
const RIGHTS =
  /©|\(c\)\s*\d{4}|copyright|all rights reserved|getty images|reuters|associated press|\bap photo\b/i;

const CENTERS = [
  { value: 'all', label: 'All centers' },
  { value: 'HQ', label: 'Headquarters' },
  { value: 'ARC', label: 'Ames' },
  { value: 'AFRC', label: 'Armstrong' },
  { value: 'GRC', label: 'Glenn' },
  { value: 'GSFC', label: 'Goddard' },
  { value: 'JPL', label: 'Jet Propulsion Laboratory' },
  { value: 'JSC', label: 'Johnson' },
  { value: 'KSC', label: 'Kennedy' },
  { value: 'LaRC', label: 'Langley' },
  { value: 'MSFC', label: 'Marshall' },
  { value: 'SSC', label: 'Stennis' },
];
const DECADES = [
  { value: 'any', label: 'Any time' },
  { value: '2020', label: '2020s' },
  { value: '2010', label: '2010s' },
  { value: '2000', label: '2000s' },
  { value: '1990', label: '1990s' },
  { value: '1980', label: '1980s' },
  { value: '1970', label: '1970s' },
  { value: '1960', label: '1960s' },
];

/** Asset addresses come with spaces and other characters as they are; players need them escaped. */
const assetUrl = (href: string) => {
  const path = href.replace(/^https?:\/\/[^/]+\//, '');
  return `${ASSETS}/${path.split('/').map(encodeURIComponent).join('/')}`;
};

const plainText = (html: string) =>
  html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<[^>]+>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    // Spaces of any kind (no-break ones too), but not line breaks.
    .replace(/[^\S\n]+/g, ' ')
    .trim();

/**
 * Titles are sometimes the file's name ("JPL-20200210-MARSf-0001-Mars Sample Return-1080",
 * "jsc2025m000018_Meet_Roscosmos_Cosmonaut"): the catalogue numbers and sizes go.
 */
const tidyTitle = (title: string) => {
  const spaced = title.includes(' ') ? title : title.replace(/_/g, ' ');
  const tidy = spaced
    .replace(/^[A-Z]{2,6}-\d{8}-[A-Za-z0-9]+-\d{4}-\s*/, '')
    .replace(/^[a-z]{2,6}\d{4}m\d+[\s_-]*/i, '')
    .replace(/[\s_-]+(?:1080p?|720p?|480p?|4k|uhd|hd|cc)$/i, '')
    .trim();
  return tidy || title.trim();
};

const publicDomain = (data: ItemData) =>
  !RIGHTS.test([data.title, data.description, data.photographer, data.secondary_creator].join(' '));

/** The largest picture of the item up to 800 pixels wide. */
const coverOf = (item: Item) => {
  const pictures = (item.links ?? []).filter(
    (link) => link.render === 'image' && (link.width ?? 0) <= 800,
  );
  const best = pictures.sort((a, b) => (b.width ?? 0) - (a.width ?? 0))[0];
  return best ? assetUrl(best.href) : null;
};

const summaryOf = (item: Item): MangaSummary[] => {
  const data = item.data?.[0];
  if (!data?.nasa_id || data.media_type !== 'video' || !publicDomain(data)) return [];
  return [
    { id: data.nasa_id, title: tidyTitle(data.title ?? data.nasa_id), coverUrl: coverOf(item) },
  ];
};

const search = async (
  params: Record<string, string | number>,
  page: number,
  { newestFirst = false } = {},
): Promise<MangaPage> => {
  const query = Object.entries({
    ...params,
    media_type: 'video',
    page_size: PAGE_SIZE,
    page: Math.max(1, page),
  })
    .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
    .join('&');
  const answer = await fetchJson<Collection>(`${API}/search?${query}`);
  const items = [...(answer.collection?.items ?? [])];
  // The library doesn't sort by date; each page is put in order at least.
  if (newestFirst) {
    const date = (item: Item) => item.data?.[0]?.date_created ?? '';
    items.sort((a, b) => date(b).localeCompare(date(a)));
  }
  const total = answer.collection?.metadata?.total_hits ?? 0;
  return {
    items: items.flatMap(summaryOf),
    hasNextPage:
      items.length > 0 &&
      ((answer.collection?.links ?? []).some((link) => link.rel === 'next') ||
        page * PAGE_SIZE < total),
  };
};

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : null;
};

const lookup = async (id: string): Promise<Item> => {
  const answer = await fetchJson<Collection>(`${API}/search?nasa_id=${encodeURIComponent(id)}`);
  const item = answer.collection?.items?.find((found) => found.data?.[0]?.nasa_id === id);
  if (!item?.data?.[0]) throw new Error('This video is no longer in NASA’s library');
  return item;
};

/** The original file's height, from the description the library keeps of it. */
const originalHeight = (metadata: FileMetadata | null): number | undefined => {
  if (!metadata) return undefined;
  const frame = metadata['XMP:VideoFrameSize'] as { H?: number } | undefined;
  const height = Number(
    frame?.H ?? metadata['QuickTime:ImageHeight'] ?? metadata['Track1:ImageHeight'],
  );
  return Number.isFinite(height) && height > 0 ? height : undefined;
};

/** A video's encodes, the original last of equals, and its captions. */
const videosOf = async (id: string): Promise<Video[]> => {
  const [assets, metadata] = await Promise.all([
    fetchJson<Assets>(`${API}/asset/${encodeURIComponent(id)}`),
    fetchJson<FileMetadata>(`${ASSETS}/video/${encodeURIComponent(id)}/metadata.json`).catch(
      () => null,
    ),
  ]);
  const files = (assets.collection?.items ?? []).map((item) => item.href);
  const captions = files.filter((href) => /\.(vtt|srt)$/i.test(href));
  // WebVTT first: every player reads it.
  captions.sort((a, b) => Number(/\.srt$/i.test(a)) - Number(/\.srt$/i.test(b)));
  const subtitles: VideoSubtitle[] = captions.slice(0, 1).map((href) => ({
    url: assetUrl(href),
    lang: 'en',
    label: 'English',
  }));
  const videos: Video[] = [];
  for (const encode of ENCODES) {
    const href = files.find((file) => file.endsWith(`~${encode.name}.mp4`));
    if (href)
      videos.push({
        url: assetUrl(href),
        label: `${encode.height}p`,
        quality: encode.height,
        subtitles,
      });
  }
  const original = files.find((file) => /~orig\.mp4$/i.test(file));
  if (original) {
    const height = originalHeight(metadata);
    videos.push({
      url: assetUrl(original),
      label: height ? `Original · ${height}p` : 'Original',
      quality: height,
      subtitles,
    });
  }
  if (videos.length === 0) throw new Error('NASA’s library has no MP4 of this video');
  return videos;
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'center', label: 'Center', options: CENTERS },
  { kind: 'select', key: 'decade', label: 'Decade', options: DECADES },
];

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'videos',
    name: 'NASA',

    // The library's own list of its most viewed media, videos among them.
    async popular({ page }) {
      if (page > 1) return { items: [], hasNextPage: false };
      const answer = await fetchJson<Collection>(`${ASSETS}/popular.json`);
      return { items: (answer.collection?.items ?? []).flatMap(summaryOf), hasNextPage: false };
    },

    latest: ({ page }) =>
      search({ year_start: new Date().getUTCFullYear() - 1 }, page, { newestFirst: true }),

    search: ({ query, page, filters: values }) => {
      const words = query.trim();
      const center = pick(values, 'center', CENTERS);
      const decade = pick(values, 'decade', DECADES);
      const params: Record<string, string | number> = {};
      if (words) params.q = words;
      if (center && center !== 'all') params.center = center;
      if (decade && decade !== 'any') {
        params.year_start = Number(decade);
        params.year_end = Number(decade) + 9;
      }
      // The library wants something to search by.
      if (Object.keys(params).length === 0) params.q = 'nasa';
      return search(params, page);
    },

    filters: async () => filters,

    async details({ id }) {
      const item = await lookup(id);
      const data = item.data![0]!;
      const year = Number.parseInt((data.date_created ?? '').slice(0, 4), 10);
      const center = CENTERS.find((option) => option.value === data.center)?.label;
      const credit = data.photographer ?? data.secondary_creator;
      return {
        id,
        title: tidyTitle(data.title ?? id),
        coverUrl: coverOf(item),
        description:
          [
            data.description ? plainText(data.description) : '',
            [center && `NASA ${center}`, credit && credit !== 'NASA' && `By ${credit}`]
              .filter(Boolean)
              .join(' · '),
          ]
            .filter(Boolean)
            .join('\n\n') || null,
        genres: (data.keywords ?? []).slice(0, 8),
        directors: credit && credit !== 'NASA' ? [credit] : [],
        type: 'other',
        year: Number.isFinite(year) ? year : null,
        status: 'completed',
        url: `https://images.nasa.gov/details/${encodeURIComponent(id)}`,
      };
    },

    episodes: async ({ id }) => ({ episodes: [{ id, name: 'Video' }] }),

    // One server, resolved at once: the library serves every encode itself.
    servers: async ({ episodeId }) => ({
      servers: [{ id: episodeId, name: 'NASA', videos: await videosOf(episodeId) }],
    }),

    videos: async ({ serverId }) => ({ videos: await videosOf(serverId) }),
  }),
);
