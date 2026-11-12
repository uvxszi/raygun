# Films and series: `video.source`

A video source has the catalogue calls of a [comics source](manga-source.md), then each title's
episodes, each episode's servers, and each server's videos. Raygun shows it in Browse → Sources
and Search → Videos, keeps saved titles in Library → Videos with Continue watching, and plays the
videos in its own player (HLS, DASH and plain files, subtitles, chapters). The model follows
Aniyomi's: a server is a host the video is on, resolved on its own, so a slow or broken host
doesn't hold up the others; Raygun plays the first link that works and falls back to the next.

## Calls

| Call       | Receives                    | Answers                         | Required |
| ---------- | --------------------------- | ------------------------------- | -------- |
| `popular`  | `{ page }`                  | `MangaPage`                     | yes      |
| `latest`   | `{ page }`                  | `MangaPage`                     | no       |
| `search`   | `{ query, page, filters? }` | `MangaPage`                     | yes      |
| `filters`  | nothing                     | `SourceFilter[]`                | no       |
| `details`  | `{ id }`                    | `VideoDetails`                  | yes      |
| `episodes` | `{ id }`                    | `{ episodes: VideoEpisode[] }`  | yes      |
| `servers`  | `{ episodeId }`             | `{ servers: VideoServer[] }`    | yes      |
| `videos`   | `{ serverId }`              | `{ videos: Video[] }`           | yes      |
| `comments` | `{ id, page? }`             | `{ comments, next? }`           | no       |

Lists, paging, filters and errors work as for [comics](manga-source.md#lists). `searchPill`
(at most 24 characters) gives a source its own pill in Search, its results then staying out of
the Videos pill.

### Details and episodes

`details` answers the comics fields plus `type` (`movie`, `series`, `anime` or `other`), `year`,
`cast` and `directors`. `episodes` answers the episodes in order; a film has one, which can be
named "Film". Each needs an `id` and a `name` or a `number`; `season` groups a series. Raygun
reads numbers and seasons from names like "S2 E5" or "Episode 12" when they're left out, and
keeps each id once.

### Servers and videos

```ts
import type { Video, VideoServer } from '@raygun/sdk';

const video: Video = {
  url: 'https://archive.org/download/ThePhantomoftheOpera/Phantom_of_the_Opera_512kb.mp4',
  // Shown in the picker; defaults to "<quality>p".
  label: '240p',
  // The height in pixels: Raygun orders links by the preferred quality in its settings.
  quality: 240,
  subtitles: [{ url: 'https://example.org/en.vtt', lang: 'en', label: 'English' }],
  // Chapters, in seconds; the player offers to skip openings, endings and recaps.
  timestamps: [{ start: 0, end: 85, name: 'Opening', type: 'opening' }],
};

// A server whose videos cost nothing to find comes with them: Raygun never calls `videos` for it.
const server: VideoServer = { id: 'archive/1', name: 'Internet Archive', videos: [video] };

export { server };
```

- `url` is `http(s)`, or `youtube:<11-character id>` to play in YouTube's embedded player.
- `format` is `hls` or `dash` for streaming manifests; leave it out for a plain file. `page` is an
  https web page that plays the video in its site's own player, for a site whose terms allow only
  its embeddable player (Twitch's): Raygun opens it in its browser instead of its player, and
  never downloads it.
- `live: true` marks a live stream: the player starts at its live edge, shows how far behind it
  plays, and keeps no place to resume; a live stream isn't downloaded.
- `headers` (a Referer, a User-Agent) are sent with every request the player makes.
- `subtitles` are WebVTT or SubRip files.
- A server has at most 50 videos, a video 50 subtitles and 100 chapters; an episode has at most
  50 servers.

### Live streams

A listed title can be a channel on the air: `live: true` marks its card live, with `viewers`
(a whole number) and `startedAt` (Unix ms) when the site says, and `subtitle` (at most 120
characters) is a line under its title, such as the channel and what it streams. A channel's
`details` describe it; its `episodes` can start with the stream on the air, then its past
broadcasts and clips, each played by its own servers.

```ts
import type { MangaSummary } from '@raygun/sdk';

const onTheAir: MangaSummary = {
  id: 'chesschannel',
  title: 'Blitz with viewers',
  coverUrl: 'https://img.example/live/chesschannel-640x360.jpg',
  subtitle: 'ChessChannel · Chess',
  live: true,
  viewers: 12_345,
  startedAt: Date.parse('2026-11-11T18:00:00Z'),
};

export { onTheAir };
```

### Raygun's Twitch source

Raygun's repository folder (`repo/`) has a Twitch source (`dev.raygun.twitch`) that Paraline,
which lists only open sources, doesn't. It reads Twitch's official API (Helix) with the Client ID
and Client Secret of an application you register at `dev.twitch.tv/console` (any name; the
redirect address `http://localhost` will do), entered in its settings: what's live now by
viewers, live streams by category and language, channel search, and each channel's page with its
stream, past broadcasts, highlights and top clips. Twitch's developer agreement requires its
embeddable player for all Twitch video, and Twitch's embeds load only in https pages, so every
episode is a `page`: Twitch's page for it (with the chat) or Twitch's pop-out player, opened in
Raygun's browser. Install it with Browse → Sources → Add and its package's address:
`https://raw.githubusercontent.com/uvxszi/raygun/main/repo/packages/dev.raygun.twitch-1.0.0.zip`
(a newer version has its number in the name).

### Comments

`comments` answers a page of `{ author, text, avatarUrl?, likes?, published?, replies? }` and
the `next` page's key, which comes back as `page`. A comment without an author or a text makes
the page unreadable, so leave such comments out.

## Example: Internet Archive films

Public-domain feature films, cartoons, silent films and film noir from the Internet Archive, with
filters for the collection and the order. A title is an archive item, each original video file in
it an episode, and its MP4 copies the videos of one server, which comes resolved.

```sh
pnpm raygun-ext init ../my-extensions/archive-films --kind video.source --id dev.example.archive-films --name "Archive films"
```

<!-- example: video.source dev.example.archive-films Archive films -->

```json
{
  "id": "dev.example.archive-films",
  "name": "Archive films",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Public-domain feature films, cartoons and silent films from the Internet Archive.",
  "website": "https://archive.org/details/moviesandfilms",
  "provides": ["video.source"],
  "permissions": { "net": ["archive.org"] },
  "contentRating": "safe"
}
```

```ts
// Archive films: a video source (`video.source`) for the Internet Archive's public-domain film
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
    name: 'Archive films',

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
```

## Try it

```sh
pnpm exec raygun-ext test films search query=nosferatu
pnpm exec raygun-ext test films search '{"query":"","page":1,"filters":{"collection":"silent_films"}}'
pnpm exec raygun-ext test films episodes id=ThePhantomoftheOpera
pnpm exec raygun-ext test films servers episodeId=ThePhantomoftheOpera/Phantom_of_the_Opera.mpeg
```
