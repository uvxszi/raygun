# Artist facts and credits: `music.info`

An info provider says who an artist is and who made a song. Raygun shows it in the full player's
About the artist and Credits cards and on artist pages. It asks info providers one after another,
the ones that answered well first, until one knows, so a provider answers only when it's sure of
the match.

## Calls

| Call      | Receives                               | Answers                                     | Required |
| --------- | -------------------------------------- | ------------------------------------------- | -------- |
| `artist`  | `{ name }`                             | `{ artist: ArtistInfo \| null }`           | no       |
| `credits` | `{ title, artist, album, durationMs }` | `{ credits: CreditGroup[], url? }`          | no       |

Both are optional, and both have 12 s. Raygun keeps the answers for a while.

### The answers

```ts
import type { ArtistInfo, CreditGroup } from '@raygun/sdk';

const artist: ArtistInfo = {
  name: 'Clara Schumann',
  kind: 'Person',
  // A few sentences of plain text, at most 4,000 characters.
  bio: 'German pianist and composer, one of the most distinguished pianists of the Romantic era.',
  origin: 'Leipzig',
  active: '1819–1896',
  url: 'https://musicbrainz.org/artist/36df8e64-6ec6-4342-9045-e962ab9effb3',
  members: [],
};

const credits: CreditGroup[] = [
  { role: 'Composed by', names: ['Scott Joplin'] },
  { role: 'Performed by', names: ['Scott Joplin'] },
];

export { artist, credits };
```

- `artist` answers `{ artist: null }` when the source doesn't know the artist, or isn't sure it's
  the right one: Raygun asks the next provider. An `ArtistInfo` needs a `name`.
- `credits` answers groups of a `role` and its `names` (at most 40 of each), and the song's
  page as `url`; no groups when unsure.
- `imageUrl` and `url` must be `http(s)`.

## Example: MusicBrainz

Artist facts (kind, origin, years active, members) and song credits from the open MusicBrainz
database, one request a second as MusicBrainz asks.

```sh
pnpm raygun-ext init ../my-extensions/musicbrainz-info --kind music.info --id dev.example.musicbrainz-info --name "MusicBrainz"
```

<!-- example: music.info dev.example.musicbrainz-info MusicBrainz -->

```json
{
  "id": "dev.example.musicbrainz-info",
  "name": "MusicBrainz",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Artist facts and song credits from the open MusicBrainz database.",
  "website": "https://musicbrainz.org",
  "provides": ["music.info"],
  "permissions": { "net": ["musicbrainz.org"] },
  "contentRating": "safe"
}
```

```ts
// MusicBrainz: artist facts and song credits (`music.info`) from MusicBrainz
// (https://musicbrainz.org), the open music database. They fill the player's About the artist and
// Credits cards and the artist pages. Raygun asks info providers one after another until one
// knows, so answer `{ artist: null }` or no credits when unsure.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test info artist name="Clara Schumann"`.
import { defineProvider, query, registerProvider } from '@raygun/sdk';
import type { CreditGroup } from '@raygun/sdk';

const API = 'https://musicbrainz.org/ws/2';
// MusicBrainz asks every client to name itself and a way to reach its author: put yours here.
const CONTACT = 'https://example.org/your-extension';
const HEADERS = { 'User-Agent': `dev.example.musicbrainz-info/0.1.0 ( ${CONTACT} )`, Accept: 'application/json' };
/** MusicBrainz takes one request a second from each address. */
const INTERVAL_MS = 1100;

type Relation = { type: string; direction?: string; artist?: { name: string } };
type Artist = {
  id: string;
  name: string;
  type?: string;
  score?: number;
  area?: { name: string } | null;
  'begin-area'?: { name: string } | null;
  'life-span'?: { begin?: string | null; end?: string | null; ended?: boolean };
  relations?: Relation[];
};
type Recording = {
  id: string;
  title: string;
  'artist-credit'?: { name: string }[];
  relations?: Relation[];
};

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();
const simple = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

let lastRequest = 0;

/** One request at a time, a second apart, as MusicBrainz asks. */
const paced = async <T>(path: string, params: Record<string, string | number>): Promise<T> => {
  const wait = lastRequest + INTERVAL_MS - Date.now();
  if (wait > 0) await raygun.time.sleep(wait);
  lastRequest = Date.now();
  const response = await raygun.http.fetch(`${API}/${path}?${query({ ...params, fmt: 'json' })}`, {
    headers: HEADERS,
  });
  if (response.status === 503) throw new Error('MusicBrainz is busy; try again in a moment');
  if (!response.ok) throw new Error(`HTTP ${response.status} from MusicBrainz`);
  return response.json<T>();
};

/** How a relationship reads as a credit. */
const ROLES: Record<string, string> = {
  composer: 'Composed by',
  lyricist: 'Lyrics by',
  writer: 'Written by',
  producer: 'Produced by',
  arranger: 'Arranged by',
  conductor: 'Conducted by',
  instrument: 'Instruments',
  vocal: 'Vocals',
};

registerProvider(
  defineProvider({
    kind: 'music.info',
    id: 'info',
    name: 'MusicBrainz',

    async artist({ name }) {
      const found = await paced<{ artists?: Artist[] }>('artist', {
        query: `artist:"${escape(name)}"`,
        limit: 5,
      });
      const match = (found.artists ?? []).find(
        (artist) => simple(artist.name) === simple(name) && (artist.score ?? 0) >= 90,
      );
      if (!match) return { artist: null };
      const details = await paced<Artist>(`artist/${match.id}`, { inc: 'artist-rels' });
      const members = (details.relations ?? [])
        .filter(
          (relation) => relation.type === 'member of band' && relation.direction === 'backward',
        )
        .flatMap((relation) => (relation.artist ? [relation.artist.name] : []));
      const span = details['life-span'];
      const begin = span?.begin?.slice(0, 4);
      return {
        artist: {
          name: details.name,
          kind: details.type ?? null,
          url: `https://musicbrainz.org/artist/${details.id}`,
          origin: details['begin-area']?.name ?? details.area?.name ?? null,
          active: begin ? `${begin}–${span?.ended ? (span.end?.slice(0, 4) ?? '') : ''}` : null,
          members: [...new Set(members)],
        },
      };
    },

    async credits({ title, artist }) {
      const search = [`recording:"${escape(title)}"`];
      if (artist) search.push(`artist:"${escape(artist)}"`);
      const found = await paced<{ recordings?: Recording[] }>('recording', {
        query: search.join(' AND '),
        limit: 5,
      });
      const match = (found.recordings ?? []).find(
        (recording) => simple(recording.title) === simple(title),
      );
      if (!match) return { credits: [] };
      const details = await paced<Recording>(`recording/${match.id}`, {
        inc: 'artist-credits+artist-rels',
      });
      const groups = new Map<string, string[]>();
      const add = (role: string, names: string[]) =>
        groups.set(role, [...new Set([...(groups.get(role) ?? []), ...names])]);
      add(
        'Performed by',
        (details['artist-credit'] ?? []).map((credit) => credit.name),
      );
      for (const relation of details.relations ?? []) {
        if (relation.artist) add(ROLES[relation.type] ?? relation.type, [relation.artist.name]);
      }
      const credits: CreditGroup[] = [...groups].map(([role, names]) => ({ role, names }));
      return { credits, url: `https://musicbrainz.org/recording/${details.id}` };
    },
  }),
);
```

## Try it

```sh
pnpm exec raygun-ext test info artist name="Clara Schumann"
pnpm exec raygun-ext test info credits title="Maple Leaf Rag" artist="Scott Joplin"
```
