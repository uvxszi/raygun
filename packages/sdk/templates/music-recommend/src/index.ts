// __NAME__: recommendations (`music.recommend`) from Openverse (https://openverse.org), which
// indexes Creative Commons and public-domain music. Raygun's Made for You mixes and song radios
// ask recommendation providers one after another until one has something: an artist's songs,
// artists like them, and songs like theirs. Every call is optional; answer nothing for an artist
// the source doesn't know.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test openverse radio artist=OnClassical`.
import { defineProvider, query, registerProvider } from '@raygun/sdk';
import type { Artist, Track } from '@raygun/sdk';

const API = 'https://api.openverse.org/v1/audio/';
/** The most results Openverse gives a request without an API key. */
const PAGE_SIZE = 20;

type Result = {
  id: string;
  title: string | null;
  creator: string | null;
  url: string | null;
  duration: number | null;
  thumbnail: string | null;
  audio_set: { title: string | null } | null;
};

const simple = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();
const NOBODY = new Set(['various artists', 'various', 'unspecified', 'unknown', 'anonymous']);

const results = async (address: string): Promise<Result[]> => {
  const response = await raygun.http.fetch(address);
  if (response.status === 429) throw new Error('Openverse is busy; try again in a minute');
  if (!response.ok) throw new Error(`HTTP ${response.status} from Openverse`);
  return (await response.json<{ results?: Result[] }>()).results ?? [];
};

/** The artist's own songs: Openverse's creator search, kept to the exact name. */
const byArtist = async (artist: string) =>
  (
    await results(`${API}?${query({ creator: artist, category: 'music', page_size: PAGE_SIZE })}`)
  ).filter((result) => simple(result.creator ?? '') === simple(artist));

/** Songs like the artist's first one, by other artists too. */
const related = async (artist: string) => {
  const own = (await byArtist(artist))[0];
  return own ? results(`${API}${own.id}/related/?${query({ page_size: PAGE_SIZE })}`) : [];
};

const toTracks = (list: Result[], limit: number): Track[] =>
  list
    .flatMap((result): Track[] =>
      result.url && result.title
        ? [
            {
              id: result.id,
              title: result.title,
              artist: result.creator,
              album: result.audio_set?.title ?? null,
              durationMs: result.duration,
              artworkUrl: result.thumbnail,
              source: { kind: 'url', url: result.url, headers: {} },
            },
          ]
        : [],
    )
    .slice(0, limit);

registerProvider(
  defineProvider({
    kind: 'music.recommend',
    id: 'openverse',
    name: '__NAME__',

    topTracks: async ({ artist, limit }) => ({ tracks: toTracks(await byArtist(artist), limit) }),

    radio: async ({ artist, limit }) => ({ tracks: toTracks(await related(artist), limit) }),

    similarArtists: async ({ artist, limit }) => {
      const names = new Map<string, Artist>();
      for (const result of await related(artist)) {
        const name = result.creator?.trim() ?? '';
        const key = simple(name);
        // "Various artists" and the like name nobody.
        if (!key || key === simple(artist) || NOBODY.has(key) || names.has(key)) continue;
        names.set(key, { id: name, name });
      }
      return { artists: [...names.values()].slice(0, limit) };
    },
    // `newReleases` is left out: Openverse doesn't date releases. Raygun asks another provider.
  }),
);
