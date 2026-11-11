# Song search: `music.metadata`

A song search finds songs, and optionally artists, albums, playlists, discographies and concerts.
Raygun asks every song search at once (16 at a time) when someone searches music and shows each
one's results as they come, merged and ranked; Search → Music's Songs, Artists, Albums and
Playlists tabs, album and playlist pages, artist discographies and concerts come from the
optional calls.

A song can come with its stream (`source`) or without one: a song without a `source` plays
through [`music.stream`](music-stream.md) providers, which Raygun asks for its title, artist and
length when it plays. A metadata-only source, such as an open music database, answers songs
without sources.

## Calls

| Call              | Receives                      | Answers                                 | Required |
| ----------------- | ----------------------------- | --------------------------------------- | -------- |
| `search`          | `{ query, limit }`            | `{ tracks: Track[] }`                   | yes      |
| `searchArtists`   | `{ query, limit }`            | `{ artists: Artist[] }`                 | no       |
| `searchAlbums`    | `{ query, limit }`            | `{ albums: Album[] }`                   | no       |
| `album`           | `{ id }`                      | `{ album: Album, tracks: Track[] }`     | no       |
| `artistReleases`  | `{ query, limit }` (the name) | `{ releases: Album[] }`, newest first   | no       |
| `artistEvents`    | `{ query, limit }` (the name) | `{ events: ArtistEvent[] }`, soonest first | no    |
| `searchPlaylists` | `{ query, limit, featured }`  | `{ playlists: Playlist[] }`             | no       |
| `playlist`        | `{ id }`                      | `{ playlist: Playlist, tracks: Track[] }` | no     |

A search waits 12 s for a source; `album` and `playlist` 30 s. A source still working then goes
on in the background (30 s in all), and the same search again gets its answer at once. Raygun
leaves a source out of whatever it has no call for. `featured` asks for the service's own
playlists only.

### Songs

```ts
import type { Track } from '@raygun/sdk';

const track: Track = {
  id: '136ab1f3-4bcd-4444-95bf-7e10c6db52f5',
  title: 'Partita No. 2 in D minor, BWV 1004 - V. Ciaccona',
  artist: 'OnClassical',
  album: 'Classical Music - The Essential Chamber Music',
  durationMs: 841000,
  artworkUrl: 'https://api.openverse.org/v1/audio/136ab1f3-4bcd-4444-95bf-7e10c6db52f5/thumb/',
  // `headers` is required: `{}` when the stream needs none.
  source: {
    kind: 'url',
    url: 'https://prod-1.storage.jamendo.com/?trackid=1433326&format=mp32',
    headers: {},
  },
};

export { track };
```

- `title` is required; a song without one is dropped, and so is one whose `source` isn't valid.
- A `source` is `{ kind: 'url', url, headers }` (an `http(s)` stream; Raygun's stream proxy sends
  the headers) or `{ kind: 'youTube', videoId }`. Local files are never accepted.
- `explicit: true` marks an explicit song: Raygun hides it while Explicit music is off.
- `artworkUrl` must be `http(s)`; Raygun finds a cover elsewhere when a song has none.

Artists need an `id` and a `name`, albums and playlists an `id` and a `title`; ids come back to
`album` and `playlist`. An album's `kind` (`album`, `single`, `ep`, `compilation`, `live` or
`other`) groups an artist's discography. An `album` or `playlist` answer without its `album` or
`playlist` fails: the page says the source changed.

### Errors and empty answers

Answer `{ tracks: [] }` for nothing found, and for an empty query. Throw for failures: a rate
limit is worth its own sentence ("Openverse is busy; try again in a minute").

## Example: Openverse

Creative Commons and public-domain music from Jamendo, Wikimedia Commons, Freesound and others,
found through Openverse's API, which needs no key. Every song comes with its stream.

```sh
pnpm raygun-ext init ../my-extensions/openverse-music --kind music.metadata --id dev.example.openverse-music --name "Openverse"
```

<!-- example: music.metadata dev.example.openverse-music Openverse -->

```json
{
  "id": "dev.example.openverse-music",
  "name": "Openverse",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Creative Commons and public-domain music found through Openverse, playable in full.",
  "website": "https://openverse.org",
  "provides": ["music.metadata"],
  "permissions": { "net": ["api.openverse.org"] },
  "contentRating": "safe"
}
```

```ts
// Openverse: a song search (`music.metadata`) over Openverse (https://openverse.org), which
// indexes Creative Commons and public-domain audio from Jamendo, Wikimedia Commons, Freesound and
// others. Raygun asks every song search at once and shows each one's results as they come. These
// songs come with their streams, so they play as found; songs without a `source` would play
// through `music.stream` providers instead.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test openverse search query=bach`.
import { defineProvider, query, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

const API = 'https://api.openverse.org/v1/audio/';
/** The most results Openverse gives a request without an API key. */
const PAGE_SIZE = 20;

/** One result of Openverse's audio search. */
type Result = {
  id: string;
  title: string | null;
  creator: string | null;
  url: string | null;
  duration: number | null;
  thumbnail: string | null;
  audio_set: { title: string | null } | null;
  mature: boolean;
};

const toTrack = (result: Result): Track[] =>
  result.url && result.title && !result.mature
    ? [
        {
          id: result.id,
          title: result.title,
          artist: result.creator,
          album: result.audio_set?.title ?? null,
          durationMs: result.duration,
          artworkUrl: result.thumbnail,
          // The file plays as it is; `headers` is required, even when empty.
          source: { kind: 'url', url: result.url, headers: {} },
        },
      ]
    : [];

registerProvider(
  defineProvider({
    kind: 'music.metadata',
    id: 'openverse',
    name: 'Openverse',

    async search({ query: words, limit }) {
      if (!words.trim()) return { tracks: [] };
      const params = query({
        q: words,
        category: 'music',
        mature: false,
        page_size: Math.min(limit, PAGE_SIZE),
      });
      const response = await raygun.http.fetch(`${API}?${params}`);
      // Without a key, Openverse lets each address make only so many searches a minute.
      if (response.status === 429) throw new Error('Openverse is busy; try again in a minute');
      if (!response.ok) throw new Error(`HTTP ${response.status} from Openverse`);
      const { results = [] } = await response.json<{ results?: Result[] }>();
      return { tracks: results.flatMap(toTrack) };
    },

    // Optional calls fill Search's Artists, Albums and Playlists tabs, album and playlist pages,
    // discographies and concerts: searchArtists, searchAlbums, album, artistReleases,
    // artistEvents, searchPlaylists, playlist. Raygun leaves a source out of what it can't do.
  }),
);
```

## Try it

```sh
pnpm exec raygun-ext test openverse search query=bach
pnpm exec raygun-ext test openverse search query="maple leaf rag" limit=5
```
