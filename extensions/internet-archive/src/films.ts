// Internet Archive films (M42): public-domain film collections as a video source. A title is an
// archive item; each original video file in it is an episode (most items hold one film), and its
// MP4 derivatives, with their heights, are the videos of a single server.
//
// Uploaders mark their own items, and the collections' uploads aren't all checked (recent ones
// include commercial films), so a film is listed only when its public-domain status holds up:
// published more than 95 years ago, or from before 1978 (when US copyright still needed a notice
// and a renewal) and carrying a public-domain mark. Uploads marked CC0 or with a Creative Commons
// licence otherwise are left out: commercial films turn up marked that way, CC0 above all.
// Prelinger Archives' films are in the public domain by Prelinger's own uploads. An item reached
// another way (a link, an old library entry) is held to the same rule when it opens.
import { defineProvider, fetchJson } from '@raygun/sdk';
import type {
  FilterValues,
  MangaPage,
  SourceFilter,
  Video,
  VideoEpisode,
  VideoServer,
  VideoSubtitle,
} from '@raygun/sdk';

const API = 'https://archive.org';
const ROWS = 30;

type SearchDoc = { identifier: string; title?: string | string[]; year?: number | string };
type SearchResponse = { response?: { numFound?: number; docs?: SearchDoc[] } };

type ItemFile = {
  name: string;
  format?: string;
  source?: string;
  original?: string;
  height?: string;
  title?: string;
};
type ItemMetadata = {
  files?: ItemFile[];
  metadata?: {
    title?: string | string[];
    description?: string | string[];
    date?: string;
    year?: string;
    creator?: string | string[];
    subject?: string | string[];
    licenseurl?: string;
    collection?: string | string[];
  };
};

const COLLECTIONS = [
  { value: 'feature_films', label: 'Feature films' },
  { value: 'silent_films', label: 'Silent films' },
  { value: 'Film_Noir', label: 'Film noir' },
  { value: 'SciFi_Horror', label: 'Sci-fi and horror' },
  { value: 'Comedy_Films', label: 'Comedy films' },
  { value: 'classic_cartoons', label: 'Classic cartoons' },
  { value: 'animationandcartoons', label: 'Animation' },
  { value: 'prelinger', label: 'Prelinger Archives' },
  { value: 'classic_tv', label: 'Classic TV' },
];
/** Works published before this year are in the US public domain: 95 years have passed. */
const PUBLIC_DOMAIN_BEFORE = new Date().getUTCFullYear() - 95;
/** The last year whose works needed a notice and a renewal to stay under US copyright. */
const MARKS_BELIEVED_UNTIL = 1977;
/** A public-domain mark, in the current form or the one the Archive's uploader used to set. */
const MARK = /publicdomain\/mark|licenses\/publicdomain/i;
const PUBLIC_DOMAIN_MARK =
  '(licenseurl:*publicdomain\\/mark* OR licenseurl:*licenses\\/publicdomain*)';
/** The search terms for a film in the public domain by age, or marked so and old enough. */
const PUBLIC_DOMAIN = `(year:[1800 TO ${PUBLIC_DOMAIN_BEFORE - 1}] OR (${PUBLIC_DOMAIN_MARK} AND year:[1800 TO ${MARKS_BELIEVED_UNTIL}]))`;
/** Prelinger Archives' films carry Prelinger's own public-domain dedication. */
const PRELINGER = 'licenseurl:*publicdomain*';
const SORTS = [
  { value: 'downloads desc', label: 'Most viewed' },
  { value: 'addeddate desc', label: 'Recently added' },
  { value: 'titleSorter asc', label: 'Title' },
];

// Lucene syntax characters would turn a user's query into a malformed or different search.
const escapeQuery = (query: string) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const firstString = (value: string | string[] | undefined): string | null =>
  (Array.isArray(value) ? value[0] : value) ?? null;

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

const cover = (identifier: string) => `${API}/services/img/${encodeURIComponent(identifier)}`;

/** Descriptions are HTML; the app shows plain text. */
const plainText = (html: string) =>
  html
    .replace(/<br\s*\/?>/gi, '\n')
    .replace(/<\/p>/gi, '\n\n')
    .replace(/<[^>]*>/g, '')
    .replace(/&nbsp;/g, ' ')
    .replace(/&quot;/g, '"')
    .replace(/&#39;|&apos;/g, "'")
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&amp;/g, '&')
    .replace(/\n{3,}/g, '\n\n')
    .trim();

const pick = (filters: FilterValues | undefined, key: string, options: { value: string }[]) => {
  const value = filters?.[key];
  return typeof value === 'string' && options.some((option) => option.value === value)
    ? value
    : options[0]!.value;
};

const searchPage = async (q: string, sort: string, page: number): Promise<MangaPage> => {
  const params = [
    `q=${encodeURIComponent(q)}`,
    'fl[]=identifier',
    'fl[]=title',
    `rows=${ROWS}`,
    `page=${Math.max(1, page)}`,
    `sort[]=${encodeURIComponent(sort)}`,
    'output=json',
  ];
  const result = await fetchJson<SearchResponse>(`${API}/advancedsearch.php?${params.join('&')}`);
  const docs = result.response?.docs ?? [];
  return {
    items: docs.map((doc) => ({
      id: doc.identifier,
      title: firstString(doc.title) ?? doc.identifier,
      coverUrl: cover(doc.identifier),
    })),
    hasNextPage: page * ROWS < (result.response?.numFound ?? 0),
  };
};

const inCollection = (collection: string) =>
  `collection:(${collection}) AND mediatype:(movies) AND ${
    collection === 'prelinger' ? PRELINGER : PUBLIC_DOMAIN
  }`;

// Item metadata is read for details, episodes and servers; keep the last few.
const items = new Map<string, Promise<ItemMetadata>>();
const fetchItem = (identifier: string): Promise<ItemMetadata> => {
  let item = items.get(identifier);
  if (!item) {
    item = fetchJson<ItemMetadata>(`${API}/metadata/${encodeURIComponent(identifier)}`);
    item.catch(() => items.delete(identifier));
    items.set(identifier, item);
    if (items.size > 20) items.delete(items.keys().next().value!);
  }
  return item;
};

/** Whether an item is free to watch, by its own record, under the rule the search asks for. */
const isOpen = (item: ItemMetadata) => {
  const licence = item.metadata?.licenseurl ?? '';
  const collections = [item.metadata?.collection ?? []].flat();
  if (collections.includes('prelinger') && /publicdomain/i.test(licence)) return true;
  const year = Number.parseInt((item.metadata?.year ?? item.metadata?.date ?? '').slice(0, 4), 10);
  if (!Number.isFinite(year)) return false;
  return year < PUBLIC_DOMAIN_BEFORE || (MARK.test(licence) && year <= MARKS_BELIEVED_UNTIL);
};

/** An item's metadata, for an item that is free to watch only (one reached by a link too). */
const metadata = async (identifier: string): Promise<ItemMetadata> => {
  const item = await fetchItem(identifier);
  if (!item.metadata) throw new Error('The Internet Archive has no such item');
  if (!isOpen(item)) {
    throw new Error(
      "The Internet Archive doesn't list this item as public domain or openly licensed, so Raygun doesn't play it",
    );
  }
  return item;
};

/** MP4 files Android's player handles (MPEG-2, Ogg Theora and the like are left out). */
const isPlayable = (file: ItemFile) =>
  /\.(mp4|m4v)$/i.test(file.name) && /h\.264|mpeg4|mpeg-4/i.test(file.format ?? '');

/** Playable files grouped by the original they were made from: one group per episode. */
const episodeFiles = (item: ItemMetadata): Map<string, ItemFile[]> => {
  const groups = new Map<string, ItemFile[]>();
  for (const file of item.files ?? []) {
    if (!isPlayable(file)) continue;
    const key = file.source === 'derivative' && file.original ? file.original : file.name;
    groups.set(key, [...(groups.get(key) ?? []), file]);
  }
  return new Map([...groups].sort(([a], [b]) => a.localeCompare(b, 'en', { numeric: true })));
};

const baseName = (name: string) => name.replace(/\.[^./]+$/, '');

const subtitlesFor = (identifier: string, item: ItemMetadata, original: string) => {
  const base = baseName(original);
  return (item.files ?? [])
    .filter((file) => /\.(srt|vtt)$/i.test(file.name) && file.name.startsWith(base))
    .map((file): VideoSubtitle => ({
      url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
      // `.asr.srt` files are the archive's automatic transcripts.
      label: /\.asr\.(srt|vtt)$/i.test(file.name) ? 'Automatic (English)' : 'Subtitles',
      lang: 'en',
    }));
};

/** `<identifier>/<original file>`; the identifier never holds a slash. */
const splitEpisode = (episodeId: string) => {
  const slash = episodeId.indexOf('/');
  return slash < 0
    ? { identifier: episodeId, original: null }
    : { identifier: episodeId.slice(0, slash), original: episodeId.slice(slash + 1) };
};

/** An episode's one server, resolved: its MP4s, best first, with the item's subtitles. */
const resolveServer = async (episodeId: string): Promise<VideoServer> => {
  const { identifier, original } = splitEpisode(episodeId);
  const item = await metadata(identifier);
  const groups = episodeFiles(item);
  const key = original ?? [...groups.keys()][0] ?? '';
  const subtitles = subtitlesFor(identifier, item, key);
  const videos = (groups.get(key) ?? [])
    .map((file): Video => {
      const height = Number.parseInt(file.height ?? '', 10);
      const known = Number.isFinite(height) && height > 0;
      return {
        url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
        label: known ? `${height}p · ${file.format}` : file.format,
        quality: known ? height : undefined,
        subtitles,
      };
    })
    .sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
  return { id: episodeId, name: 'Internet Archive', videos };
};

const filters: SourceFilter[] = [
  { kind: 'select', key: 'collection', label: 'Collection', options: COLLECTIONS },
  { kind: 'select', key: 'sort', label: 'Sort', options: SORTS },
];

export const films = defineProvider({
  kind: 'video.source',
  id: 'films',
  name: 'Internet Archive Films',

  popular: ({ page }) => searchPage(inCollection('feature_films'), 'downloads desc', page),

  latest: ({ page }) => searchPage(inCollection('feature_films'), 'addeddate desc', page),

  async search({ query, page, filters: values }) {
    const collection = pick(values, 'collection', COLLECTIONS);
    const sort = pick(values, 'sort', SORTS);
    const cleaned = escapeQuery(query);
    const q = cleaned
      ? `(title:(${cleaned}) OR subject:(${cleaned})) AND ${inCollection(collection)}`
      : inCollection(collection);
    return searchPage(q, sort, page);
  },

  filters: async () => filters,

  async details({ id }) {
    const item = await metadata(id);
    const meta = item.metadata ?? {};
    const description = firstString(meta.description);
    const year = Number.parseInt((meta.year ?? meta.date ?? '').slice(0, 4), 10);
    const subjects = (Array.isArray(meta.subject) ? meta.subject : [meta.subject ?? ''])
      .flatMap((subject) => subject.split(/\s*;\s*/))
      .map((subject) => subject.trim())
      .filter(Boolean);
    const creators = Array.isArray(meta.creator)
      ? meta.creator
      : meta.creator
        ? [meta.creator]
        : [];
    return {
      id,
      title: firstString(meta.title) ?? id,
      coverUrl: cover(id),
      description: description ? plainText(description) : null,
      genres: [...new Set(subjects)].slice(0, 8),
      directors: creators,
      type: 'movie',
      year: Number.isFinite(year) ? year : null,
      status: 'completed',
      url: `${API}/details/${encodeURIComponent(id)}`,
    };
  },

  async episodes({ id }) {
    const groups = [...episodeFiles(await metadata(id))];
    if (groups.length === 0) throw new Error('This item has no video Raygun can play');
    if (groups.length === 1) return { episodes: [{ id: `${id}/${groups[0]![0]}`, name: 'Film' }] };
    return {
      episodes: groups.map(([original, files], index): VideoEpisode => ({
        id: `${id}/${original}`,
        name: files.find((file) => file.title)?.title ?? baseName(original).replace(/_/g, ' '),
        number: index + 1,
      })),
    };
  },

  // One server, resolved at once: the archive serves every file itself.
  servers: async ({ episodeId }) => ({ servers: [await resolveServer(episodeId)] }),

  // Never needed (`servers` comes resolved), but part of the contract.
  videos: async ({ serverId }) => ({ videos: (await resolveServer(serverId)).videos ?? [] }),
});
