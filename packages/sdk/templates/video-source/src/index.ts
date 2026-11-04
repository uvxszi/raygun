// __NAME__: a video source (`video.source`) for the Internet Archive's public-domain film
// collections (https://archive.org/details/moviesandfilms). A title is an archive item; each
// original video file in it is an episode (most items hold one film), and the item's MP4 copies of
// it, at their heights, are the videos of its one server.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with `pnpm exec raygun-ext test films popular`.
import { defineProvider, fetchJson, query, registerProvider, selectValue } from '@raygun/sdk';
import type {
  MangaPage,
  SelectFilter,
  Video,
  VideoEpisode,
  VideoServer,
  VideoSubtitle,
} from '@raygun/sdk';

const API = 'https://archive.org';
const ROWS = 30;

type SearchAnswer = {
  response?: { numFound?: number; docs?: { identifier: string; title?: string | string[] }[] };
};
type ItemFile = {
  name: string;
  format?: string;
  source?: string;
  original?: string;
  height?: string;
};
type Item = {
  files?: ItemFile[];
  metadata?: {
    title?: string | string[];
    description?: string | string[];
    date?: string;
    creator?: string | string[];
    subject?: string | string[];
  };
};

const COLLECTION: SelectFilter = {
  kind: 'select',
  key: 'collection',
  label: 'Collection',
  options: [
    { value: 'feature_films', label: 'Feature films' },
    { value: 'animationandcartoons', label: 'Animation and cartoons' },
    { value: 'silent_films', label: 'Silent films' },
    { value: 'Film_Noir', label: 'Film noir' },
  ],
};
const SORT: SelectFilter = {
  kind: 'select',
  key: 'sort',
  label: 'Sort by',
  options: [
    { value: 'downloads desc', label: 'Most viewed' },
    { value: 'addeddate desc', label: 'Recently added' },
    { value: 'titleSorter asc', label: 'Title' },
  ],
};

const first = (value: string | string[] | undefined) =>
  (Array.isArray(value) ? value[0] : value) ?? null;
const list = (value: string | string[] | undefined) =>
  (Array.isArray(value) ? value : value ? [value] : []).filter(Boolean);

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');
const download = (identifier: string, file: string) =>
  `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file)}`;
const cover = (identifier: string) => `${API}/services/img/${encodeURIComponent(identifier)}`;

const searchPage = async (q: string, sort: string, page: number): Promise<MangaPage> => {
  const at = Math.max(page, 1);
  const params = query({
    q,
    'fl[]': ['identifier', 'title'],
    'sort[]': sort,
    rows: ROWS,
    page: at,
    output: 'json',
  });
  const answer = await fetchJson<SearchAnswer>(`${API}/advancedsearch.php?${params}`);
  const docs = answer.response?.docs ?? [];
  return {
    items: docs.map((doc) => ({
      id: doc.identifier,
      title: first(doc.title) ?? doc.identifier,
      coverUrl: cover(doc.identifier),
    })),
    hasNextPage: at * ROWS < (answer.response?.numFound ?? 0),
  };
};

const inCollection = (collection: string) => `collection:(${collection}) AND mediatype:(movies)`;

// An item's metadata answers details, episodes and servers; the last few are kept.
const items = new Map<string, Promise<Item>>();
const item = (identifier: string) => {
  let found = items.get(identifier);
  if (!found) {
    found = fetchJson<Item>(`${API}/metadata/${encodeURIComponent(identifier)}`);
    found.catch(() => items.delete(identifier));
    items.set(identifier, found);
    if (items.size > 20) items.delete(items.keys().next().value!);
  }
  return found;
};

/** The MP4 files of an item, grouped by the original each was made from: one group an episode. */
const episodeFiles = (metadata: Item) => {
  const groups = new Map<string, ItemFile[]>();
  for (const file of metadata.files ?? []) {
    if (!/\.(mp4|m4v)$/i.test(file.name) || !/h\.264|mpeg4/i.test(file.format ?? '')) continue;
    const original = file.source === 'derivative' && file.original ? file.original : file.name;
    groups.set(original, [...(groups.get(original) ?? []), file]);
  }
  return [...groups].sort(([a], [b]) => a.localeCompare(b));
};

/** An episode's one server, with its videos: the archive serves every file itself. */
const server = async (episodeId: string): Promise<VideoServer> => {
  const slash = episodeId.indexOf('/');
  const identifier = slash < 0 ? episodeId : episodeId.slice(0, slash);
  const metadata = await item(identifier);
  const groups = episodeFiles(metadata);
  const original = slash < 0 ? groups[0]?.[0] : episodeId.slice(slash + 1);
  const files = groups.find(([name]) => name === original)?.[1] ?? [];
  const base = (original ?? '').replace(/\.[^.]+$/, '');
  const subtitles: VideoSubtitle[] = (metadata.files ?? [])
    .filter((file) => /\.(srt|vtt)$/i.test(file.name) && file.name.startsWith(base))
    .map((file) => ({ url: download(identifier, file.name), lang: 'en', label: 'English' }));
  const videos = files
    .map((file): Video => {
      const height = Number.parseInt(file.height ?? '', 10);
      return {
        url: download(identifier, file.name),
        label: height > 0 ? `${height}p` : (file.format ?? 'MP4'),
        quality: height > 0 ? height : undefined,
        subtitles,
      };
    })
    .sort((a, b) => (b.quality ?? 0) - (a.quality ?? 0));
  return { id: episodeId, name: 'Internet Archive', videos };
};

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'films',
    name: '__NAME__',

    popular: ({ page }) => searchPage(inCollection('feature_films'), 'downloads desc', page),
    latest: ({ page }) => searchPage(inCollection('feature_films'), 'addeddate desc', page),
    filters: async () => [COLLECTION, SORT],
    search: ({ query: words, page, filters }) => {
      const collection = inCollection(selectValue(filters, COLLECTION));
      const cleaned = escape(words);
      const q = cleaned
        ? `(title:(${cleaned}) OR subject:(${cleaned})) AND ${collection}`
        : collection;
      return searchPage(q, selectValue(filters, SORT), page);
    },

    details: async ({ id }) => {
      const meta = (await item(id)).metadata;
      if (!meta) throw new Error('The Internet Archive has no such item');
      const year = Number.parseInt((meta.date ?? '').slice(0, 4), 10);
      const description = first(meta.description);
      return {
        id,
        title: first(meta.title) ?? id,
        coverUrl: cover(id),
        // Descriptions are HTML; the app shows plain text.
        description: description
          ? description
              .replace(/<[^>]+>/g, ' ')
              .replace(/\s+/g, ' ')
              .trim()
          : null,
        // Subjects come as a list, or as one text separated by semicolons.
        genres: [...new Set(list(meta.subject).flatMap((subject) => subject.split(/\s*;\s*/)))]
          .filter(Boolean)
          .slice(0, 8),
        directors: list(meta.creator),
        type: 'movie',
        year: Number.isFinite(year) ? year : null,
        status: 'completed',
        url: `${API}/details/${encodeURIComponent(id)}`,
      };
    },

    episodes: async ({ id }) => {
      const groups = episodeFiles(await item(id));
      if (groups.length === 0) throw new Error('This item has no video Raygun can play');
      const episodes: VideoEpisode[] = groups.map(([original], index) => ({
        id: `${id}/${original}`,
        name: groups.length === 1 ? 'Film' : original.replace(/\.[^.]+$/, '').replace(/_/g, ' '),
        number: index + 1,
      }));
      return { episodes };
    },

    // The one server comes with its videos, so Raygun never needs `videos` for it.
    servers: async ({ episodeId }) => ({ servers: [await server(episodeId)] }),
    videos: async ({ serverId }) => ({ videos: (await server(serverId)).videos ?? [] }),
  }),
);
