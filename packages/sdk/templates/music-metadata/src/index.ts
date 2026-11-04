// __NAME__: a song search (`music.metadata`) over Openverse (https://openverse.org), which
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
    name: '__NAME__',

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
