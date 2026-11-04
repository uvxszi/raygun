# Charts: `music.charts`

A chart provider gives a source's charts: its most played songs, what's trending, new releases,
each genre's top songs. Raygun lists every provider's charts in Browse → Music, grouped by their
`group`, opens one with `chart`, and fills Home's popular and trending rows. With `tagSongs`, a
provider also gives songs for any genre or mood page Raygun has. Songs without a `source` play
through [`music.stream`](music-stream.md).

## Calls

| Call       | Receives                                     | Answers                          | Required |
| ---------- | -------------------------------------------- | -------------------------------- | -------- |
| `charts`   | `{ region }`                                 | `{ charts: Chart[] }`            | yes      |
| `chart`    | `{ id, region, limit }`                      | `{ tracks: Track[] }`            | yes      |
| `tagSongs` | `{ tag, aliases, kind, region, limit }`      | `{ tracks: Track[], name? }`     | no       |

`region` is the user's country as an ISO 3166 code (`us`), or `null`; a source without charts by
country ignores it. `charts` and `chart` have 30 s, `tagSongs` 12 s. Raygun keeps charts for a
while, so `charts` isn't asked on every visit.

### Charts

```ts
import type { Chart } from '@raygun/sdk';

const charts: Chart[] = [
  { id: 'week', name: 'Played most this week', group: 'trending' },
  { id: 'genre:Jazz', name: 'Jazz releases', tag: 'Jazz', group: 'genre' },
];

export { charts };
```

- A chart needs an `id` (it comes back to `chart`) and a `name`.
- `group` is one of `top`, `trending`, `new`, `viral`, `genre`, `mood`, `decade` or
  `underground`. Without one, a chart with a `tag` counts as `genre`, else `top`.
- `tag` names the chart's genre ("Rock") so the genre's page can show it.
- `artworkUrl` is optional: Raygun uses the first song's cover without one.

### Songs for a genre or mood

`tagSongs` receives Raygun's name for a genre or mood (`tag`, like "Hip-Hop" or "Chill"), the
other spellings sources use (`aliases`, lower case, like "hip hop" and "rap"), and whether it's a
`genre` or a `mood`. Answer the songs and, optionally, the row's `name` (by default "<tag> on
<source>"); answer no songs when the source has nothing for it.

## Example: Internet Archive netlabels

Music that independent labels publish under Creative Commons licenses, kept by the Internet
Archive: the releases played most this week and of all time, new ones, and the top releases of a
few genres, each release standing for its first song; any genre's or mood's songs by the releases'
subjects.

```sh
pnpm raygun-ext init ../my-extensions/netlabel-charts --kind music.charts --id dev.example.netlabel-charts --name "Netlabels"
```

<!-- example: music.charts dev.example.netlabel-charts Netlabels -->

```json
{
  "id": "dev.example.netlabel-charts",
  "name": "Netlabels",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "The most played Creative Commons netlabel releases on the Internet Archive, overall, this week and by genre.",
  "website": "https://archive.org/details/netlabels",
  "provides": ["music.charts"],
  "permissions": { "net": ["archive.org"] },
  "contentRating": "safe"
}
```

```ts
// Netlabels: charts (`music.charts`) of the Internet Archive's netlabel releases
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
    name: 'Netlabels',

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
```

## Try it

```sh
pnpm exec raygun-ext test netlabels charts
pnpm exec raygun-ext test netlabels chart id=week limit=10
pnpm exec raygun-ext test netlabels tagSongs tag=Hip-Hop aliases='["hip hop","rap"]'
```
