// __NAME__: charts (`music.charts`) of the Internet Archive's netlabel releases
// (https://archive.org/details/netlabels): music that independent labels publish under Creative
// Commons licenses. Raygun lists every chart provider's charts in Browse, grouped by their
// `group`, and opens one with `chart`; genre and mood pages ask `tagSongs`. Each release in a
// chart stands for its first song.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with `pnpm exec raygun-ext test netlabels charts`,
// then `pnpm exec raygun-ext test netlabels chart id=week limit=10`.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { Chart, Track } from '@raygun/sdk';

const API = 'https://archive.org';
/** Netlabel releases that carry a Creative Commons license (public-domain marks included). */
const RELEASES =
  'collection:(netlabels) AND mediatype:(audio) AND licenseurl:(*creativecommons.org*)';
/** Releases one chart holds at most: each costs a request. */
const MOST = 25;

/** The charts, with what each searches and how it sorts. */
const CHARTS: (Chart & { search: string; sort: string })[] = [
  {
    id: 'week',
    name: 'Played most this week',
    group: 'trending',
    search: RELEASES,
    sort: 'week desc',
  },
  {
    id: 'all',
    name: 'Played most of all time',
    group: 'top',
    search: RELEASES,
    sort: 'downloads desc',
  },
  { id: 'new', name: 'New releases', group: 'new', search: RELEASES, sort: 'publicdate desc' },
  ...['Electronic', 'Ambient', 'Rock', 'Hip-Hop', 'Jazz', 'Experimental'].map((genre) => ({
    id: `genre:${genre}`,
    name: `${genre} releases`,
    tag: genre,
    group: 'genre' as const,
    search: `${RELEASES} AND subject:("${genre.toLowerCase()}")`,
    sort: 'downloads desc',
  })),
];

type SearchAnswer = { response?: { docs?: { identifier: string }[] } };
type ItemFile = { name: string; format?: string; title?: string; track?: string; length?: string };
type Item = {
  files?: ItemFile[];
  metadata?: { title?: string | string[]; creator?: string | string[] };
};

const first = (value: string | string[] | undefined) =>
  (Array.isArray(value) ? value[0] : value) ?? null;

const lengthMs = (length: string | undefined) => {
  const seconds = (length ?? '').split(':').reduce((total, part) => total * 60 + Number(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

/** A release's first song, playable from the archive; `null` for one without MP3s. */
const firstSong = async (identifier: string): Promise<Track | null> => {
  const item = await fetchJson<Item>(`${API}/metadata/${encodeURIComponent(identifier)}`);
  const songs = (item.files ?? [])
    .filter((file) => file.format === 'VBR MP3')
    .sort(
      (a, b) =>
        (Number.parseInt(a.track ?? '', 10) || 999) - (Number.parseInt(b.track ?? '', 10) || 999),
    );
  const song = songs[0];
  if (!song) return null;
  return {
    id: `${identifier}/${song.name}`,
    title: song.title || song.name.replace(/\.mp3$/i, '').replace(/_/g, ' '),
    artist: first(item.metadata?.creator),
    album: first(item.metadata?.title),
    durationMs: lengthMs(song.length),
    artworkUrl: `${API}/services/img/${encodeURIComponent(identifier)}`,
    source: {
      kind: 'url',
      url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(song.name)}`,
      headers: {},
    },
  };
};

/** The first songs of the releases a search finds, in its order. */
const songs = async (search: string, sort: string, limit: number): Promise<Track[]> => {
  const params = query({
    q: search,
    'fl[]': 'identifier',
    'sort[]': sort,
    rows: Math.min(Math.max(limit, 1), MOST),
    output: 'json',
  });
  const answer = await fetchJson<SearchAnswer>(`${API}/advancedsearch.php?${params}`);
  // All at once (Raygun lets six through to a site at a time); a release that fails is skipped.
  const found = await Promise.allSettled(
    (answer.response?.docs ?? []).map((doc) => firstSong(doc.identifier)),
  );
  return found.flatMap((result) =>
    result.status === 'fulfilled' && result.value ? [result.value] : [],
  );
};

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'netlabels',
    name: '__NAME__',

    // The archive's charts are the same everywhere, so `region` goes unused.
    charts: async () => ({
      charts: CHARTS.map(({ id, name, group, tag }) => ({ id, name, group, tag: tag ?? null })),
    }),

    chart: async ({ id, limit }) => {
      const chart = CHARTS.find((known) => known.id === id);
      if (!chart) throw new Error('This chart is gone; open Browse again');
      return { tracks: await songs(chart.search, chart.sort, limit) };
    },

    // Songs for any genre or mood Raygun has a page for, by the release's subjects.
    tagSongs: async ({ tag, aliases, limit }) => {
      const names = [tag, ...aliases].map((name) => `"${name.toLowerCase().replace(/"/g, ' ')}"`);
      const tracks = await songs(
        `${RELEASES} AND subject:(${names.join(' OR ')})`,
        'week desc',
        limit,
      );
      return { tracks, name: `${tag} on netlabels` };
    },
  }),
);
