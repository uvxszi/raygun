// Last.fm (last.fm): charts, genres, recommendations and artist biographies from Last.fm's API,
// under its terms (https://www.last.fm/api/tos): for non-commercial use, credited to Last.fm and
// linked to its pages. Last.fm's images and audio are outside what its terms allow, so songs and
// artists come without pictures and songs without streams: Raygun plays them through the music
// sources the person has.
//
// Each person creates their own free API key (https://www.last.fm/api/account/create) and enters
// it in this source's settings. The setting is required, so Raygun asks nothing while it's empty,
// and a key Last.fm refuses or has suspended says so with `needsSetup`.
//
// - Charts (`charts.ts`): the songs most played on Last.fm and in the person's country, and the
//   top songs of a list of genres; any genre's or mood's songs from its tag.
// - Tags: a song's most used tags, else its artist's at half weight, without tags that aren't
//   genres ("seen live", years, the artist's own name).
// - Recommendations: artists like an artist, an artist's most played songs, and radios: songs
//   like a song, else the most played songs of the artist's closest artists, taking turns.
// - Artist facts: the biography's summary, with the artist's Last.fm page as its link.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Artist, MusicTag, Track } from '@raygun/sdk';

import { clamp, distinct, lastfm, listOf, numberOf, songsIn, textOf, words } from './api';
import type { LastfmArtist, TrackAnswer } from './api';
import { charts } from './charts';

const SITE = 'https://www.last.fm';

// ---- Tags ----

type LastfmTag = { name?: unknown; count?: unknown };
type TopTags = { toptags?: { tag?: LastfmTag | LastfmTag[] | null } | null };

/** Tags people give songs that say nothing of their genre or mood. */
const NOT_GENRES = new Set([
  'seen live',
  'favorite',
  'favourite',
  'favorites',
  'favourites',
  'my favorite',
  'my favourite',
  'my favorites',
  'my favourites',
  'favorite songs',
  'favourite songs',
  'loved',
  'loved tracks',
  'albums i own',
  'awesome',
  'spotify',
  'under 2000 listeners',
]);
/** Last.fm counts a song's tags from 0 to 100, its most used 100: rarer ones are noise. */
const MIN_COUNT = 10;
const MOST_TAGS = 10;

const genreTags = (
  tags: LastfmTag | LastfmTag[] | null | undefined,
  scale: number,
  artist: string,
): MusicTag[] => {
  const own = words(artist);
  const seen = new Set<string>();
  return listOf(tags)
    .flatMap((tag) => {
      const name = textOf(tag.name);
      const key = words(name);
      const count = Math.min(numberOf(tag.count) ?? 0, 100);
      const kept =
        key !== '' &&
        name.length <= 60 &&
        count >= MIN_COUNT &&
        !NOT_GENRES.has(key) &&
        !/^\d{4}$/.test(key) &&
        key !== own &&
        !seen.has(key);
      if (!kept) return [];
      seen.add(key);
      return [{ name, weight: (count / 100) * scale }];
    })
    .slice(0, MOST_TAGS);
};

const tags = defineProvider({
  kind: 'music.tags',
  id: 'tags',
  name: 'Last.fm',
  async tags({ title, artist }) {
    const name = artist?.trim() ?? '';
    const song = title.trim();
    if (!name || !song) return { tags: [] };
    const own = await lastfm<TopTags>('track.getTopTags', {
      artist: name,
      track: song,
      autocorrect: 1,
    });
    const found = genreTags(own?.toptags?.tag, 1, name);
    if (found.length > 0) return { tags: found };
    // A song nobody tagged gets its artist's tags, less certain.
    const byArtist = await lastfm<TopTags>('artist.getTopTags', { artist: name, autocorrect: 1 });
    return { tags: genreTags(byArtist?.toptags?.tag, 0.5, name) };
  },
});

// ---- Recommendations ----

type SimilarArtists = {
  similarartists?: { artist?: LastfmArtist | LastfmArtist[] | null } | null;
};

/** Artists like `artist`, the closest first. */
const similarTo = async (artist: string, count: number): Promise<Artist[]> => {
  const answer = await lastfm<SimilarArtists>('artist.getSimilar', {
    artist,
    autocorrect: 1,
    limit: count,
  });
  const seen = new Set<string>([words(artist)]);
  return listOf(answer?.similarartists?.artist)
    .flatMap((found) => {
      const name = textOf(found.name);
      if (!name || seen.has(words(name))) return [];
      seen.add(words(name));
      return [{ id: textOf(found.mbid) || textOf(found.url) || name, name }];
    })
    .slice(0, count);
};

/** The artist's most played songs. */
const topTracksOf = async (artist: string, count: number) =>
  songsIn(
    await lastfm<TrackAnswer>('artist.getTopTracks', { artist, autocorrect: 1, limit: count }),
  ).slice(0, count);

/** Round-robin over each artist's songs, so a radio doesn't play one artist for long. */
const interleave = (groups: Track[][]) => {
  const order: Track[] = [];
  const longest = Math.max(0, ...groups.map((group) => group.length));
  for (let index = 0; index < longest; index += 1) {
    for (const group of groups) {
      const track = group[index];
      if (track) order.push(track);
    }
  }
  return order;
};

/** Artists whose songs make a radio of an artist's own. */
const MIX_ARTISTS = 8;

const recommend = defineProvider({
  kind: 'music.recommend',
  id: 'recommend',
  name: 'Last.fm',

  async similarArtists({ artist, limit }) {
    const name = artist.trim();
    return { artists: name ? await similarTo(name, clamp(limit || 10, 1, 100)) : [] };
  },

  async topTracks({ artist, limit }) {
    const name = artist.trim();
    return { tracks: name ? await topTracksOf(name, clamp(limit || 10, 1, 100)) : [] };
  },

  // Songs like the song, when Last.fm knows it; else the most played songs of the artist's
  // closest artists, a few each, taking turns.
  async radio({ artist, title, limit }) {
    const name = artist.trim();
    if (!name) return { tracks: [] };
    const count = clamp(limit || 25, 1, 100);
    const song = title?.trim();
    if (song) {
      const similar = await lastfm<TrackAnswer>('track.getSimilar', {
        artist: name,
        track: song,
        autocorrect: 1,
        limit: Math.min(count * 2, 200),
      });
      const tracks = songsIn(similar).slice(0, count);
      if (tracks.length > 0) return { tracks };
    }
    const others = await similarTo(name, MIX_ARTISTS);
    const each = Math.max(2, Math.ceil((count * 3) / 2 / Math.max(1, others.length)));
    const groups: Track[][] = [];
    for (const other of others) {
      try {
        groups.push(await topTracksOf(other.name, each));
      } catch (error) {
        // One artist's songs missing still leaves a radio; a refused key doesn't.
        if (String(error).includes('needs-setup')) throw error;
        raygun.log.info(`Last.fm had no songs of ${other.name}: ${String(error)}`);
      }
    }
    return { tracks: distinct(interleave(groups)).slice(0, count) };
  },
});

// ---- Artist facts ----

type ArtistInfoAnswer = {
  artist?: {
    name?: unknown;
    url?: unknown;
    bio?: { summary?: unknown; content?: unknown } | null;
  } | null;
};

const decode = (text: string) =>
  text
    .replace(/&nbsp;|&#160;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;|&apos;/g, "'")
    .replace(/&#x([0-9a-f]+);/gi, (_match, code: string) =>
      String.fromCodePoint(parseInt(code, 16)),
    )
    .replace(/&#(\d+);/g, (_match, code: string) => String.fromCodePoint(Number(code)))
    .replace(/&amp;/g, '&');

/**
 * A biography as plain text in paragraphs, without Last.fm's "Read more on Last.fm" link or the
 * licence line after it: the artist's page, linked beside it, says both.
 */
const biography = (html: unknown) =>
  decode(
    (typeof html === 'string' ? html : '')
      .replace(/<a\b[^>]*>\s*Read more on Last\.fm\s*<\/a>\.?/gi, '')
      .replace(
        /User-contributed text is available under the Creative Commons By-SA License;?\s*(?:additional terms may apply\.?)?/gi,
        '',
      )
      .replace(/<br\s*\/?>|<\/p>/gi, '\n')
      .replace(/<[^>]+>/g, ''),
  )
    .split(/\n+/)
    .map((paragraph) => paragraph.replace(/\s+/g, ' ').trim())
    .filter(Boolean)
    .join('\n\n');

/** At most `most` characters, ending at a sentence's end when one is near. */
const cut = (text: string, most: number) => {
  if (text.length <= most) return text;
  const part = text.slice(0, most);
  const end = Math.max(part.lastIndexOf('. '), part.lastIndexOf('.\n'));
  return end > most / 2 ? part.slice(0, end + 1) : `${part.trimEnd()}…`;
};

const info = defineProvider({
  kind: 'music.info',
  id: 'info',
  name: 'Last.fm',
  async artist({ name }) {
    const wanted = name.trim();
    if (!wanted) return { artist: null };
    const answer = await lastfm<ArtistInfoAnswer>('artist.getInfo', {
      artist: wanted,
      autocorrect: 1,
    });
    const found = answer?.artist;
    const shown = textOf(found?.name);
    const bio = cut(biography(found?.bio?.summary) || biography(found?.bio?.content), 1500);
    // Without a biography Last.fm says no more than the name: other sources may know more.
    if (!shown || !bio) return { artist: null };
    const page = textOf(found?.url);
    return {
      artist: {
        name: shown,
        bio,
        url: /^https?:\/\//i.test(page) ? page : `${SITE}/music/${encodeURIComponent(shown)}`,
      },
    };
  },
});

registerProvider(charts);
registerProvider(tags);
registerProvider(recommend);
registerProvider(info);
