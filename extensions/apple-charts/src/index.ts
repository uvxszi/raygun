// Apple Music charts: the most played songs in a country, from Apple's public RSS feed generator
// (M46.9), and each genre's top songs from the iTunes Store's genre feeds (M46.19). Neither needs a
// key. Songs carry no stream; Raygun finds one when they play.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Chart, Track } from '@raygun/sdk';

const FEED = 'https://rss.marketingtools.apple.com/api/v2';
const ITUNES = 'https://itunes.apple.com';
const DEFAULT_REGION = 'us';
// The feeds' largest page.
const MAX_SONGS = 100;
// Raygun names itself, as it does to other open services.
const HEADERS = { 'User-Agent': 'Raygun/1.0 (https://github.com/uvxszi/raygun)' };

/** iTunes genre ids, by the app's genre names. */
const GENRES: Record<string, number> = {
  Pop: 14,
  Rock: 21,
  'Hip-Hop': 18,
  'R&B': 15,
  Electronic: 7,
  Dance: 17,
  House: 1048,
  Jazz: 11,
  Classical: 5,
  Country: 6,
  Metal: 1153,
  Indie: 1004,
  Folk: 1289,
  Latin: 12,
  Brazilian: 1122,
  'K-Pop': 51,
  Reggae: 24,
  Blues: 2,
  Soul: 1143,
  Punk: 1006,
  Ambient: 1056,
  Soundtrack: 16,
  Anime: 29,
  Afrobeats: 100051,
  Gospel: 1101,
  Christian: 22,
  Alternative: 20,
};

/** Genres listed as charts of their own; every genre above has songs on its page. */
const CHART_GENRES = ['Pop', 'Hip-Hop', 'Rock', 'Alternative', 'R&B', 'Country', 'Dance', 'Latin'];

/** The feeds sometimes answer 5xx for a moment; one retry after a pause covers it. */
const fetchJson = async <T>(url: string): Promise<T> => {
  let response = await raygun.http.fetch(url, { headers: HEADERS });
  if (response.status >= 500) {
    await raygun.time.sleep(1000);
    response = await raygun.http.fetch(url, { headers: HEADERS });
  }
  if (!response.ok) throw new Error(`Apple's charts answered HTTP ${response.status}`);
  return response.json<T>();
};

type FeedSong = {
  id: string;
  name: string;
  artistName?: string;
  artworkUrl100?: string;
};
type Feed = { feed: { title?: string; country?: string; results?: FeedSong[] } };

type Label = { label?: string };
type ItunesEntry = {
  'im:name'?: Label;
  'im:artist'?: Label;
  'im:image'?: Label[];
  'im:collection'?: { 'im:name'?: Label };
  id?: { attributes?: { 'im:id'?: string } };
};
type ItunesFeed = { feed?: { entry?: ItunesEntry[] | ItunesEntry } };

const region = (asked: string | null) =>
  asked && /^[a-z]{2}$/i.test(asked) ? asked.toLowerCase() : DEFAULT_REGION;

/** A small artwork link, asked for at 600 px (Apple's image server sizes on request). */
const bigArtwork = (url?: string) => (url ? url.replace(/\/\d+x\d+bb\./, '/600x600bb.') : null);

const countryName = (code: string) => {
  try {
    return new Intl.DisplayNames(['en'], { type: 'region' }).of(code.toUpperCase()) ?? code;
  } catch {
    return code.toUpperCase();
  }
};

/** A genre's top songs in a country; `explicit=true` is needed or the feed leaves songs out. */
const genreSongs = async (genre: number, country: string, limit: number): Promise<Track[]> => {
  const count = Math.min(Math.max(limit, 1), MAX_SONGS);
  const page = await fetchJson<ItunesFeed>(
    `${ITUNES}/${country}/rss/topsongs/limit=${count}/genre=${genre}/explicit=true/json`,
  );
  const entries = page.feed?.entry;
  const list = Array.isArray(entries) ? entries : entries ? [entries] : [];
  return list.flatMap((entry) => {
    const title = entry['im:name']?.label;
    if (!title) return [];
    const images = entry['im:image'] ?? [];
    return [
      {
        id: entry.id?.attributes?.['im:id'],
        title,
        artist: entry['im:artist']?.label ?? null,
        album: entry['im:collection']?.['im:name']?.label?.replace(/ - Single$/, '') ?? null,
        artworkUrl: bigArtwork(images[images.length - 1]?.label),
      },
    ];
  });
};

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'Apple Music',
    async charts(args) {
      const country = region(args.region);
      const charts: Chart[] = [
        {
          id: 'most-played',
          name: `Top songs: ${countryName(country)}`,
          description: 'The most played songs on Apple Music',
          group: 'top',
        },
      ];
      for (const name of CHART_GENRES) {
        charts.push({
          id: `genre:${GENRES[name]}`,
          name: `${name}: ${countryName(country)}`,
          description: `Top ${name} songs on the iTunes Store`,
          tag: name,
          group: 'genre',
        });
      }
      return { charts };
    },
    async chart({ id, region: asked, limit }) {
      const country = region(asked);
      if (id.startsWith('genre:')) {
        const genre = Number(id.slice('genre:'.length));
        if (!Number.isInteger(genre)) throw new Error(`Unknown chart ${id}`);
        return { tracks: await genreSongs(genre, country, limit) };
      }
      if (id !== 'most-played') throw new Error(`Unknown chart ${id}`);
      const count = Math.min(Math.max(limit, 1), MAX_SONGS);
      const page = await fetchJson<Feed>(
        `${FEED}/${country}/music/most-played/${count}/songs.json`,
      );
      const tracks: Track[] = (page.feed.results ?? []).map((song) => ({
        id: song.id,
        title: song.name,
        artist: song.artistName ?? null,
        artworkUrl: bigArtwork(song.artworkUrl100),
      }));
      return { tracks };
    },
    async tagSongs({ tag, region: asked, limit }) {
      const genre = GENRES[tag];
      if (genre === undefined) return { tracks: [] };
      return {
        tracks: await genreSongs(genre, region(asked), limit),
        name: `${tag} on Apple Music`,
      };
    },
  }),
);
