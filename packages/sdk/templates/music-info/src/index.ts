// __NAME__: artist facts and song credits (`music.info`) from MusicBrainz
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
const HEADERS = { 'User-Agent': `__ID__/0.1.0 ( ${CONTACT} )`, Accept: 'application/json' };
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
    name: '__NAME__',

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
