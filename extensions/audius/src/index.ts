// Audius: the open music platform where artists upload their own songs (M46.19). Its public API
// needs no key, only an app name. Charts are its trending and underground lists, overall and by
// genre; songs play in full from Audius itself, so it is also a search and stream source.
// Songs an artist gated (for supporters or buyers) are left out: they don't play without an account.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Chart, MediaSource, Track } from '@raygun/sdk';

const API = 'https://api.audius.co/v1';
const APP = 'raygun';
const MAX = 100;

type AudiusTrack = {
  id: string;
  title: string;
  duration?: number;
  genre?: string | null;
  mood?: string | null;
  is_streamable?: boolean;
  is_delete?: boolean;
  stream_conditions?: unknown;
  artwork?: Record<string, string> | null;
  user?: { name?: string; handle?: string } | null;
};
type Answer<T> = { data?: T };

/** Audius's genres, by the app's name for each (its own spelling where they differ). */
const GENRES: Record<string, string> = {
  Electronic: 'Electronic',
  'Hip-Hop': 'Hip-Hop/Rap',
  'Lo-fi': 'Lo-Fi',
  Rock: 'Rock',
  Metal: 'Metal',
  Jazz: 'Jazz',
  Classical: 'Classical',
  Ambient: 'Ambient',
  Hyperpop: 'Hyperpop',
  'Drum & Bass': 'Drum & Bass',
  Dubstep: 'Dubstep',
  House: 'House',
  Techno: 'Techno',
  Trance: 'Trance',
  Pop: 'Pop',
  'R&B': 'R&B/Soul',
  Soul: 'R&B/Soul',
  Country: 'Country',
  Folk: 'Folk',
  Latin: 'Latin',
  Reggae: 'Reggae',
  Punk: 'Punk',
  Soundtrack: 'Soundtrack',
  Blues: 'Blues',
  Alternative: 'Alternative',
  'Jersey Club': 'Jersey Club',
  Phonk: 'Phonk',
  Trap: 'Trap',
  Experimental: 'Experimental',
  Funk: 'Funk',
  Disco: 'Disco',
  Acoustic: 'Acoustic',
  Hardstyle: 'Hardstyle',
  Downtempo: 'Downtempo',
  Kids: 'Kids',
  World: 'World',
};

/** Genres shown as charts of their own; the rest are reached from their genre's page. */
const CHART_GENRES = [
  'Electronic',
  'Hip-Hop',
  'Lo-fi',
  'Pop',
  'R&B',
  'Rock',
  'Alternative',
  'Drum & Bass',
  'House',
  'Phonk',
  'Hyperpop',
  'Ambient',
];

const get = async <T>(path: string): Promise<T> => {
  const join = path.includes('?') ? '&' : '?';
  const response = await raygun.http.fetch(`${API}${path}${join}app_name=${APP}`);
  if (!response.ok) throw new Error(`Audius answered HTTP ${response.status}`);
  const body = await response.json<Answer<T>>();
  if (body.data === undefined) throw new Error('Audius sent nothing back');
  return body.data;
};

const playable = (track: AudiusTrack) =>
  track.is_streamable !== false && !track.is_delete && !track.stream_conditions;

const stream = (id: string): MediaSource => ({
  kind: 'url',
  url: `${API}/tracks/${encodeURIComponent(id)}/stream?app_name=${APP}`,
  headers: {},
});

const toTrack = (track: AudiusTrack): Track => ({
  id: track.id,
  title: track.title,
  artist: track.user?.name ?? track.user?.handle ?? null,
  album: null,
  durationMs: track.duration ? track.duration * 1000 : null,
  artworkUrl: track.artwork?.['1000x1000'] ?? track.artwork?.['480x480'] ?? null,
  source: stream(track.id),
});

const tracks = (found: AudiusTrack[], limit: number) =>
  found.filter(playable).slice(0, limit).map(toTrack);

/** Chart ids: `trending:<time>[:<genre>]` or `underground`. */
const TIMES = { week: 'This week', month: 'This month', allTime: 'All time' } as const;
type Time = keyof typeof TIMES;

const trending = async (time: Time, genre: string | null, limit: number) => {
  const params = [`time=${time}`, `limit=${Math.min(MAX, limit * 2)}`];
  if (genre) params.push(`genre=${encodeURIComponent(genre)}`);
  return tracks(await get<AudiusTrack[]>(`/tracks/trending?${params.join('&')}`), limit);
};

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'Audius',
    async charts() {
      const charts: Chart[] = [
        {
          id: 'trending:week',
          name: 'Trending on Audius',
          description: 'What artists on Audius are being played most this week',
          group: 'trending',
        },
        {
          id: 'underground',
          name: 'Underground trending',
          description: 'Rising artists on Audius with smaller followings',
          group: 'underground',
        },
        {
          id: 'trending:allTime',
          name: 'Audius all-time',
          description: 'The most played songs on Audius',
          group: 'top',
        },
      ];
      for (const name of CHART_GENRES) {
        charts.push({
          id: `trending:week:${GENRES[name]}`,
          name: `${name} on Audius`,
          description: `Trending ${name} songs on Audius this week`,
          tag: name,
          group: 'genre',
        });
      }
      return { charts };
    },
    async chart({ id, limit }) {
      if (id === 'underground') {
        return {
          tracks: tracks(
            await get<AudiusTrack[]>(
              `/tracks/trending/underground?limit=${Math.min(MAX, limit * 2)}`,
            ),
            limit,
          ),
        };
      }
      const [kind, time, ...rest] = id.split(':');
      if (kind !== 'trending' || !time || !(time in TIMES)) throw new Error(`Unknown chart ${id}`);
      return { tracks: await trending(time as Time, rest.join(':') || null, limit) };
    },
    async tagSongs({ tag, limit }) {
      const genre = GENRES[tag];
      if (!genre) return { tracks: [] };
      return { tracks: await trending('month', genre, limit), name: `${tag} on Audius` };
    },
  }),
);

registerProvider(
  defineProvider({
    kind: 'music.metadata',
    id: 'search',
    name: 'Audius',
    async search({ query, limit }) {
      if (!query.trim()) return { tracks: await trending('week', null, limit) };
      const found = await get<AudiusTrack[]>(
        `/tracks/search?query=${encodeURIComponent(query)}&limit=${Math.min(MAX, limit * 2)}`,
      );
      return { tracks: tracks(found, limit) };
    },
  }),
);

// Streams for songs found elsewhere: Audius's own uploads of them, matched by Raygun's ranking.
registerProvider(
  defineProvider({
    kind: 'music.stream',
    id: 'stream',
    name: 'Audius',
    async candidates({ title, artist, limit }) {
      const query = [artist, title].filter(Boolean).join(' ').trim();
      if (!query) return { candidates: [] };
      const found = await get<AudiusTrack[]>(
        `/tracks/search?query=${encodeURIComponent(query)}&limit=${Math.min(20, limit * 2)}`,
      );
      return {
        candidates: found
          .filter(playable)
          .slice(0, limit)
          .map((track) => ({
            id: track.id,
            title: track.title,
            artist: track.user?.name ?? null,
            durationMs: track.duration ? track.duration * 1000 : null,
            // Audius streams MP3 at 320 kbps.
            quality: '320 kbps MP3',
            bitrateKbps: 320,
            source: stream(track.id),
          })),
      };
    },
  }),
);
