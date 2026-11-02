// Deezer: charts (the top songs, and each genre's) and the genres of songs, from Deezer's public
// API, which needs no key (M46.9). Charts follow the country Deezer sees the request from. Songs
// carry no stream; Raygun finds one through `music.stream` providers when they play.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Album, Artist, Chart, LyricsQuery, MusicTag, Track } from '@raygun/sdk';

const API = 'https://api.deezer.com';
// Deezer allows 50 requests in 5 seconds; one retry covers a brief "quota" answer.
const RETRY_AFTER_MS = 1500;

type DeezerTrack = {
  id: number;
  title: string;
  title_short?: string;
  duration?: number;
  artist?: { name: string };
  album?: { id: number; title: string; cover_xl?: string; cover_big?: string };
};
type DeezerGenre = { id: number; name: string; picture_xl?: string; picture_big?: string };
type DeezerArtist = { id: number; name: string; picture_xl?: string; picture_big?: string };
type DeezerAlbum = {
  id: number;
  title: string;
  cover_xl?: string;
  cover_big?: string;
  nb_tracks?: number;
  release_date?: string;
  artist?: { name: string };
};
type DeezerList<T> = { data?: T[]; error?: { message?: string; code?: number } };

/** Deezer answers errors with status 200 and an `error` object; quota errors are worth a retry. */
const get = async <T>(path: string): Promise<T> => {
  for (let attempt = 0; ; attempt += 1) {
    const response = await raygun.http.fetch(`${API}${path}`);
    if (!response.ok) throw new Error(`Deezer answered HTTP ${response.status}`);
    const body = await response.json<T & { error?: { message?: string; code?: number } }>();
    if (!body.error) return body;
    if (body.error.code === 4 && attempt === 0) {
      await raygun.time.sleep(RETRY_AFTER_MS);
      continue;
    }
    throw new Error(`Deezer: ${body.error.message ?? 'error'}`);
  }
};

const toTrack = (track: DeezerTrack): Track => ({
  id: String(track.id),
  title: track.title,
  artist: track.artist?.name ?? null,
  album: track.album?.title ?? null,
  durationMs: track.duration ? track.duration * 1000 : null,
  artworkUrl: track.album?.cover_xl ?? track.album?.cover_big ?? null,
});

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'Deezer',
    async charts() {
      const genres = await get<DeezerList<DeezerGenre>>('/genre');
      const charts: Chart[] = [
        { id: '0', name: 'Top songs on Deezer', description: 'The most played songs today' },
      ];
      for (const genre of genres.data ?? []) {
        if (genre.id === 0) continue;
        charts.push({
          id: String(genre.id),
          name: genre.name,
          description: `Top ${genre.name} songs on Deezer`,
          artworkUrl: genre.picture_xl ?? genre.picture_big ?? null,
          tag: genre.name,
        });
      }
      return { charts };
    },
    async chart({ id, limit }) {
      if (!/^\d+$/.test(id)) throw new Error(`Unknown chart ${id}`);
      const page = await get<DeezerList<DeezerTrack>>(
        `/chart/${id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`,
      );
      return { tracks: (page.data ?? []).map(toTrack) };
    },
  }),
);

const simple = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .replace(/\(.*?\)|\[.*?\]/g, ' ')
    .replace(/[^a-z0-9]+/g, ' ')
    .trim();

/** The search result that is this song: same title and an artist in common. */
const sameSong = (track: DeezerTrack, query: LyricsQuery) => {
  const title = simple(query.title);
  const found = simple(track.title_short ?? track.title);
  if (!title || !(found === title || found.startsWith(`${title} `))) return false;
  if (!query.artist) return true;
  const wanted = simple(query.artist);
  const artist = simple(track.artist?.name ?? '');
  return Boolean(artist) && (wanted.includes(artist) || artist.includes(wanted));
};

registerProvider(
  defineProvider({
    kind: 'music.tags',
    id: 'genres',
    name: 'Deezer',
    async tags(query) {
      const terms = [query.artist, query.title].filter(Boolean).join(' ');
      const found = await get<DeezerList<DeezerTrack>>(
        `/search?q=${encodeURIComponent(terms)}&limit=10`,
      );
      const match = (found.data ?? []).find((track) => sameSong(track, query));
      if (!match?.album) return { tags: [] };
      const album = await get<{ genres?: { data?: { name: string }[] } }>(
        `/album/${match.album.id}`,
      );
      // Albums list their main genre first.
      const tags: MusicTag[] = (album.genres?.data ?? []).map((genre, index) => ({
        name: genre.name,
        weight: index === 0 ? 1 : 0.7,
      }));
      return { tags };
    },
  }),
);

const toAlbum = (album: DeezerAlbum): Album => ({
  id: String(album.id),
  title: album.title,
  artist: album.artist?.name ?? null,
  year: album.release_date ? Number(album.release_date.slice(0, 4)) || null : null,
  artworkUrl: album.cover_xl ?? album.cover_big ?? null,
  trackCount: album.nb_tracks ?? null,
});

const limitOf = (limit: number) => Math.min(Math.max(limit, 1), 50);

/** Songs, artists and albums from Deezer's catalogue (M46.9); songs play through other sources. */
registerProvider(
  defineProvider({
    kind: 'music.metadata',
    id: 'catalogue',
    name: 'Deezer',
    async search({ query, limit }) {
      const found = await get<DeezerList<DeezerTrack>>(
        `/search?q=${encodeURIComponent(query)}&limit=${limitOf(limit)}`,
      );
      return { tracks: (found.data ?? []).map(toTrack) };
    },
    async searchArtists({ query, limit }) {
      const found = await get<DeezerList<DeezerArtist>>(
        `/search/artist?q=${encodeURIComponent(query)}&limit=${limitOf(limit)}`,
      );
      const artists: Artist[] = (found.data ?? []).map((artist) => ({
        id: String(artist.id),
        name: artist.name,
        imageUrl: artist.picture_xl ?? artist.picture_big ?? null,
      }));
      return { artists };
    },
    async searchAlbums({ query, limit }) {
      const found = await get<DeezerList<DeezerAlbum>>(
        `/search/album?q=${encodeURIComponent(query)}&limit=${limitOf(limit)}`,
      );
      return { albums: (found.data ?? []).map(toAlbum) };
    },
    async album({ id }) {
      if (!/^\d+$/.test(id)) throw new Error(`Unknown album ${id}`);
      const album = await get<DeezerAlbum & { tracks?: { data?: DeezerTrack[] } }>(`/album/${id}`);
      const tracks = (album.tracks?.data ?? []).map((track) =>
        toTrack({
          ...track,
          album: track.album ?? {
            id: album.id,
            title: album.title,
            cover_xl: album.cover_xl,
            cover_big: album.cover_big,
          },
        }),
      );
      return { album: toAlbum(album), tracks };
    },
  }),
);
