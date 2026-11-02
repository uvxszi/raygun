// Apple Music charts: the most played songs in a country, from Apple's public RSS feed generator
// (M46.9), which needs no key. Songs carry no stream; Raygun finds one when they play.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

const FEED = 'https://rss.marketingtools.apple.com/api/v2';
const DEFAULT_REGION = 'us';
// The feed's largest page.
const MAX_SONGS = 100;
// Raygun names itself, as it does to other open services.
const HEADERS = { 'User-Agent': 'Raygun/1.0 (https://github.com/uvxszi/raygun)' };

/** The feed sometimes answers 5xx for a moment; one retry after a pause covers it. */
const fetchFeed = async (url: string): Promise<Feed> => {
  let response = await raygun.http.fetch(url, { headers: HEADERS });
  if (response.status >= 500) {
    await raygun.time.sleep(1000);
    response = await raygun.http.fetch(url, { headers: HEADERS });
  }
  if (!response.ok) throw new Error(`Apple's charts answered HTTP ${response.status}`);
  return response.json<Feed>();
};

type FeedSong = {
  id: string;
  name: string;
  artistName?: string;
  artworkUrl100?: string;
};
type Feed = { feed: { title?: string; country?: string; results?: FeedSong[] } };

const region = (asked: string | null) =>
  asked && /^[a-z]{2}$/i.test(asked) ? asked.toLowerCase() : DEFAULT_REGION;

/** The feed's 100 px artwork, asked for at 600 px (Apple's image server sizes on request). */
const bigArtwork = (url?: string) => (url ? url.replace(/\/\d+x\d+bb\./, '/600x600bb.') : null);

const countryName = (code: string) => {
  try {
    return new Intl.DisplayNames(['en'], { type: 'region' }).of(code.toUpperCase()) ?? code;
  } catch {
    return code.toUpperCase();
  }
};

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'Apple Music',
    async charts(args) {
      const country = region(args.region);
      return {
        charts: [
          {
            id: 'most-played',
            name: `Top songs: ${countryName(country)}`,
            description: 'The most played songs on Apple Music',
          },
        ],
      };
    },
    async chart({ id, region: asked, limit }) {
      if (id !== 'most-played') throw new Error(`Unknown chart ${id}`);
      const count = Math.min(Math.max(limit, 1), MAX_SONGS);
      const page = await fetchFeed(
        `${FEED}/${region(asked)}/music/most-played/${count}/songs.json`,
      );
      const tracks: Track[] = (page.feed.results ?? []).map((song) => ({
        id: song.id,
        title: song.name,
        artist: song.artistName ?? null,
        artworkUrl: bigArtwork(song.artworkUrl100),
      }));
      return { tracks };
    },
  }),
);
