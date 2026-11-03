// Deezer, from its public API, which needs no key (M46.9): charts (the top songs, each genre's and
// the decades'), songs for any genre or mood (M46.19: its chart, a Deezer radio for it, or else the
// best playlist named after it), the genres of songs, and recommendations for Made for You
// (M46.20). Charts follow the country Deezer sees the request from. Songs carry no stream; Raygun
// finds one through `music.stream` providers when they play.
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
type DeezerArtist = {
  id: number;
  name: string;
  picture_xl?: string;
  picture_big?: string;
  nb_fan?: number;
};
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
type DeezerPlaylist = {
  id: number;
  title: string;
  nb_tracks?: number;
  user?: { id?: number; name?: string };
};

/** Deezer's genre charts, by the app's genre names. */
const GENRE_CHARTS: Record<string, number> = {
  Pop: 132,
  'Hip-Hop': 116,
  Rap: 116,
  Reggaeton: 122,
  Rock: 152,
  Dance: 113,
  'R&B': 165,
  Alternative: 85,
  Christian: 186,
  Electronic: 106,
  Folk: 466,
  Reggae: 144,
  Jazz: 129,
  Country: 84,
  Salsa: 67,
  Classical: 98,
  Soundtrack: 173,
  Metal: 464,
  Soul: 169,
  Funk: 169,
  Afrobeats: 2,
  African: 2,
  Blues: 153,
  Brazilian: 75,
  Cumbia: 71,
  Indian: 81,
  Kids: 95,
  Latin: 197,
};

/** Deezer radios for moods, decades and genres without a chart. */
const RADIOS: Record<string, number> = {
  Happy: 38235,
  Sad: 39051,
  Chill: 37121,
  Focus: 38225,
  Party: 36871,
  Workout: 36901,
  Romantic: 36861,
  Sleep: 38955,
  Energetic: 648,
  EDM: 648,
  'Bossa Nova': 39101,
  Ska: 31121,
  Indie: 30771,
  Techno: 30851,
  Disco: 30841,
  Gospel: 39071,
  Grime: 40622,
  House: 36891,
  Dubstep: 38215,
  Acoustic: 38435,
  Opera: 37061,
  "'60s": 38285,
  "'70s": 38295,
  "'80s": 38305,
  "'90s": 38315,
  '2000s': 38325,
};

const DECADES = ["'60s", "'70s", "'80s", "'90s", '2000s'];

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
        {
          id: '0',
          name: 'Top songs on Deezer',
          description: 'The most played songs today',
          group: 'top',
        },
      ];
      for (const genre of genres.data ?? []) {
        if (genre.id === 0) continue;
        charts.push({
          id: String(genre.id),
          name: genre.name,
          description: `Top ${genre.name} songs on Deezer`,
          artworkUrl: genre.picture_xl ?? genre.picture_big ?? null,
          tag: genre.name,
          group: 'genre',
        });
      }
      for (const decade of DECADES) {
        charts.push({
          id: `radio:${RADIOS[decade]}`,
          name: `The ${decade}`,
          description: `Deezer's radio of the ${decade}`,
          group: 'decade',
        });
      }
      return { charts };
    },
    async chart({ id, limit }) {
      if (id.startsWith('radio:')) return { tracks: await radio(id.slice('radio:'.length), limit) };
      if (!/^\d+$/.test(id)) throw new Error(`Unknown chart ${id}`);
      return { tracks: await genreChart(Number(id), limit) };
    },
    async tagSongs({ tag, aliases, limit }) {
      const names = [tag, ...aliases];
      const chart = names.map((name) => GENRE_CHARTS[name]).find((id) => id !== undefined);
      if (chart !== undefined) {
        return { tracks: await genreChart(chart, limit), name: `${tag} on Deezer` };
      }
      const station = names.map((name) => RADIOS[name]).find((id) => id !== undefined);
      if (station !== undefined) {
        return { tracks: await radio(String(station), limit), name: `${tag} radio on Deezer` };
      }
      return playlistFor(tag, limit);
    },
  }),
);

const genreChart = async (id: number, limit: number) => {
  const page = await get<DeezerList<DeezerTrack>>(
    `/chart/${id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`,
  );
  return (page.data ?? []).map(toTrack);
};

const radio = async (id: string, limit: number) => {
  if (!/^\d+$/.test(id)) throw new Error(`Unknown radio ${id}`);
  const page = await get<DeezerList<DeezerTrack>>(
    `/radio/${id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`,
  );
  return (page.data ?? []).map(toTrack);
};

/**
 * Songs for a genre or mood Deezer has no chart or radio for: one of Deezer's own playlists for it,
 * else the fullest listener playlist with the name in its title.
 */
const playlistFor = async (tag: string, limit: number) => {
  const found = await get<DeezerList<DeezerPlaylist>>(
    `/search/playlist?q=${encodeURIComponent(tag)}&limit=25`,
  );
  const wanted = simple(tag);
  const named = (found.data ?? []).filter(
    (playlist) => simple(playlist.title).includes(wanted) && (playlist.nb_tracks ?? 0) >= 15,
  );
  const editorial = named.find((playlist) => /deezer/i.test(playlist.user?.name ?? ''));
  const chosen =
    editorial ?? [...named].sort((a, b) => (b.nb_tracks ?? 0) - (a.nb_tracks ?? 0))[0] ?? null;
  if (!chosen) return { tracks: [] };
  const page = await get<DeezerList<DeezerTrack>>(
    `/playlist/${chosen.id}/tracks?limit=${Math.min(Math.max(limit, 1), 100)}`,
  );
  return { tracks: (page.data ?? []).map(toTrack), name: `${chosen.title} (Deezer)` };
};

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

// Artists already looked up, so making mixes asks Deezer about each name once.
const artists = new Map<string, DeezerArtist | null>();

/**
 * The Deezer artist with this name: of the hits whose name matches, the one with the most fans
 * (Deezer often lists a little-known namesake first), else the first hit.
 */
const artistNamed = async (name: string): Promise<DeezerArtist | null> => {
  const wanted = simple(name);
  const known = artists.get(wanted);
  if (known) return known;
  const found = await get<DeezerList<DeezerArtist>>(
    `/search/artist?q=${encodeURIComponent(name)}&limit=10`,
  );
  const list = found.data ?? [];
  const named = list
    .filter((candidate) => simple(candidate.name) === wanted)
    .sort((a, b) => (b.nb_fan ?? 0) - (a.nb_fan ?? 0));
  const artist = named[0] ?? list[0] ?? null;
  // Only finds are kept: a miss (or a slow answer) is asked again next time.
  if (artist) {
    if (artists.size > 500) artists.clear();
    artists.set(wanted, artist);
  }
  return artist;
};

const DAY_MS = 24 * 60 * 60 * 1000;

/**
 * Recommendations (M46.20) for Made for You: artists Deezer relates, their top songs, releases
 * from the last weeks, and Deezer's artist radio.
 */
registerProvider(
  defineProvider({
    kind: 'music.recommend',
    id: 'recommend',
    name: 'Deezer',
    async similarArtists({ artist, limit }) {
      const found = await artistNamed(artist);
      if (!found) return { artists: [] };
      const related = await get<DeezerList<DeezerArtist>>(
        `/artist/${found.id}/related?limit=${limitOf(limit)}`,
      );
      const artists: Artist[] = (related.data ?? []).map((other) => ({
        id: String(other.id),
        name: other.name,
        imageUrl: other.picture_xl ?? other.picture_big ?? null,
      }));
      return { artists };
    },
    async topTracks({ artist, limit }) {
      const found = await artistNamed(artist);
      if (!found) return { tracks: [] };
      const top = await get<DeezerList<DeezerTrack>>(
        `/artist/${found.id}/top?limit=${limitOf(limit)}`,
      );
      return { tracks: (top.data ?? []).map(toTrack) };
    },
    async newReleases({ artist, days, limit }) {
      const found = await artistNamed(artist);
      if (!found) return { tracks: [] };
      const albums = await get<DeezerList<DeezerAlbum>>(`/artist/${found.id}/albums?limit=25`);
      const since = Date.now() - Math.max(1, days) * DAY_MS;
      const recent = (albums.data ?? [])
        .filter((album) => album.release_date && Date.parse(album.release_date) >= since)
        .sort((a, b) => Date.parse(b.release_date ?? '') - Date.parse(a.release_date ?? ''))
        .slice(0, 3);
      const tracks: Track[] = [];
      for (const album of recent) {
        const page = await get<DeezerList<DeezerTrack>>(`/album/${album.id}/tracks?limit=50`);
        for (const track of page.data ?? []) {
          tracks.push(
            toTrack({
              ...track,
              artist: track.artist ?? { name: found.name },
              album: {
                id: album.id,
                title: album.title,
                cover_xl: album.cover_xl,
                cover_big: album.cover_big,
              },
            }),
          );
        }
      }
      return { tracks: tracks.slice(0, limitOf(limit)) };
    },
    async radio({ artist, limit }) {
      const found = await artistNamed(artist);
      if (!found) return { tracks: [] };
      const radio = await get<DeezerList<DeezerTrack>>(
        `/artist/${found.id}/radio?limit=${Math.min(Math.max(limit, 1), 100)}`,
      );
      return { tracks: (radio.data ?? []).map(toTrack) };
    },
  }),
);
