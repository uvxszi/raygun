// The Live Music Archive (https://archive.org/details/etree): concert recordings the Internet
// Archive keeps with etree.org, all by trade-friendly artists who opted in to free, strictly
// non-commercial sharing of their shows (their commercial releases are never there). Searches
// find concerts with the advanced-search API and read each one's file list for its songs; a
// concert is an album. Shows an artist keeps to streaming only (the `stream_only` collection) are
// left out, since Raygun can save what it plays.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { Album, Track } from '@raygun/sdk';

const API = 'https://archive.org';
/** Every show of the Live Music Archive that may be downloaded. */
const SHOWS = 'collection:etree AND mediatype:etree AND -collection:stream_only';
/** Concerts read for a song search. */
const CONCERTS_PER_SEARCH = 6;

type Doc = {
  identifier: string;
  title?: string | string[];
  creator?: string | string[];
  date?: string;
  year?: string | number;
  collection?: string | string[];
  downloads?: number;
};
type SearchAnswer = { response?: { numFound?: number; docs?: Doc[] } };

type ItemFile = { name: string; format?: string; title?: string; track?: string; length?: string };
type Item = {
  files?: ItemFile[];
  metadata?: {
    title?: string | string[];
    creator?: string | string[];
    date?: string;
    year?: string;
    collection?: string | string[];
  };
};

// Lucene's special characters would turn what was typed into a different search.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const first = (value: string | string[] | undefined): string | null =>
  (Array.isArray(value) ? value[0] : value)?.trim() || null;

const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .replace(/['’]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

const wordsOf = (text: string | null | undefined) => simple(text).split(' ').filter(Boolean);

// `length` is seconds ("245.32") or a clock ("4:05").
const parseLength = (length: string | undefined) => {
  if (!length) return null;
  const seconds = length
    .split(':')
    .reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

const yearOf = (doc: { year?: string | number; date?: string }) => {
  const year = Number.parseInt(String(doc.year ?? doc.date ?? '').slice(0, 4), 10);
  return Number.isFinite(year) && year > 1800 ? year : null;
};

const artworkOf = (identifier: string) => `${API}/services/img/${encodeURIComponent(identifier)}`;

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

const search = async (q: string, rows: number, sort = 'downloads desc') => {
  const params = query({
    q: `${SHOWS} AND ${q}`,
    'fl[]': ['identifier', 'title', 'creator', 'date', 'year', 'collection', 'downloads'],
    'sort[]': sort,
    rows,
    output: 'json',
  });
  const answer = await fetchJson<SearchAnswer>(`${API}/advancedsearch.php?${params}`);
  return answer.response?.docs ?? [];
};

/** The MP3s made from a show's recording; VBR first, the most common derivative. */
const MP3_FORMATS = ['VBR MP3', '64Kbps MP3', 'MP3', '128Kbps MP3'];

const trackNumber = (file: ItemFile) =>
  Number.parseInt(String(file.track ?? '').split('/')[0] ?? '', 10);

/** A song's name without the set list's segue marks ("Scarlet Begonias ->") and notes ("*"). */
export const songTitle = (file: ItemFile) => {
  const named = file.title?.trim() || file.name.replace(/\.mp3$/i, '').replace(/_/g, ' ');
  return named.replace(/(?:\s*(?:-+>|=+>|>+|\*+|#+))+\s*$/, '').trim() || named;
};

type Show = {
  identifier: string;
  title: string;
  artist: string | null;
  year: number | null;
  tracks: Track[];
};

/** A show's songs in order; `null` when it may not be downloaded or has no MP3s. */
const show = async (identifier: string): Promise<Show | null> => {
  const item = await fetchJson<Item>(`${API}/metadata/${encodeURIComponent(identifier)}`);
  const collections = [item.metadata?.collection ?? []].flat();
  // The search index can lag behind an item's own record; the record decides.
  if (collections.includes('stream_only') || !collections.includes('etree')) return null;
  const files = item.files ?? [];
  const format = MP3_FORMATS.find((wanted) => files.some((file) => file.format === wanted));
  if (!format) return null;
  const title = first(item.metadata?.title) ?? identifier;
  const artist = first(item.metadata?.creator);
  const tracks = files
    .filter((file) => file.format === format)
    .sort(
      (a, b) => (trackNumber(a) || 1e9) - (trackNumber(b) || 1e9) || a.name.localeCompare(b.name),
    )
    .map((file): Track => ({
      id: `${identifier}/${file.name}`,
      title: songTitle(file),
      artist,
      album: title,
      durationMs: parseLength(file.length),
      artworkUrl: artworkOf(identifier),
      source: {
        kind: 'url',
        url: `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file.name)}`,
        headers: {},
      },
    }));
  return {
    identifier,
    title,
    artist,
    year: yearOf(item.metadata ?? {}),
    tracks,
  };
};

const toAlbum = (doc: Doc): Album => ({
  id: doc.identifier,
  title: first(doc.title) ?? doc.identifier,
  artist: first(doc.creator),
  year: yearOf(doc),
  artworkUrl: artworkOf(doc.identifier),
  kind: 'live',
});

/**
 * Shows naming every word in one of `fields` (the search's own words would match any of them).
 * At most 8 words, so the query stays short.
 */
const everyWord = (text: string, fields: string[]) =>
  wordsOf(text)
    .slice(0, 8)
    .map((word) => `(${fields.map((field) => `${field}:(${word})`).join(' OR ')})`)
    .join(' AND ');

const SHOW_FIELDS = ['title', 'creator', 'venue', 'coverage'];
const SONG_FIELDS = ['creator', 'title', 'description'];

registerProvider(
  defineProvider({
    kind: 'music.metadata',
    id: 'search',
    name: 'Live Music Archive',

    // Songs: concerts by the artist or whose set lists name the song, then the matching songs
    // in each (a few of each concert when only the artist was asked for).
    async search({ query: text, limit }) {
      const wanted = wordsOf(text);
      if (wanted.length === 0) return { tracks: [] };
      const docs = await search(everyWord(text, SONG_FIELDS), CONCERTS_PER_SEARCH);
      const settled = await Promise.allSettled(docs.map((doc) => show(doc.identifier)));
      const perShow = Math.max(2, Math.ceil(limit / Math.max(1, docs.length)));
      const tracks: Track[] = [];
      for (const result of settled) {
        if (result.status === 'rejected') {
          raygun.log.warn('show skipped:', String(result.reason));
          continue;
        }
        const found = result.value;
        if (!found) continue;
        const artistWords = new Set(wordsOf(found.artist));
        const rest = wanted.filter((word) => !artistWords.has(word));
        const matching =
          rest.length === 0
            ? found.tracks.slice(0, perShow)
            : found.tracks.filter((track) => {
                const title = wordsOf(track.title);
                return rest.every((word) => title.includes(word));
              });
        tracks.push(...matching);
      }
      return { tracks: tracks.slice(0, Math.max(1, limit)) };
    },

    async searchAlbums({ query: text, limit }) {
      if (wordsOf(text).length === 0) return { albums: [] };
      const docs = await search(
        everyWord(text, SHOW_FIELDS),
        Math.max(1, Math.min(limit || 20, 50)),
      );
      return { albums: docs.map(toAlbum) };
    },

    async album({ id }) {
      const found = await show(id);
      if (!found) throw new Error('This concert is no longer in the Live Music Archive');
      return {
        album: {
          id: found.identifier,
          title: found.title,
          artist: found.artist,
          year: found.year,
          artworkUrl: artworkOf(found.identifier),
          trackCount: found.tracks.length,
          kind: 'live',
        },
        tracks: found.tracks,
      };
    },

    // An artist's concerts, newest first, when the artist shares them here. Each artist has a
    // collection of their own; its shows are found through the artist's most played one, since
    // the name alone also finds tribute bands ("… Grateful Dead Revue").
    async artistReleases({ query: name, limit }) {
      const words = escape(name);
      const wanted = simple(name);
      if (!words || !wanted) return { releases: [] };
      const known = await search(`creator:("${words}")`, 10);
      const own = known.find((doc) => simple(first(doc.creator)) === wanted);
      const collection = [own?.collection ?? []]
        .flat()
        .find((id) => id !== 'etree' && id !== 'stream_only' && !id.startsWith('fav-'));
      if (!own || !collection) return { releases: [] };
      const count = Math.max(1, Math.min(limit || 30, 50));
      const docs = await search(
        `collection:"${collection.replace(/["\\]/g, '')}"`,
        Math.min(count * 3, 150),
        'date desc',
      );
      // A night often has several recordings (tapers, soundboards): the most played stands for it.
      const nights = new Map<string, Doc>();
      for (const doc of docs) {
        if (simple(first(doc.creator)) !== wanted) continue;
        const night = (doc.date ?? doc.identifier).slice(0, 10);
        const kept = nights.get(night);
        if (!kept || (doc.downloads ?? 0) > (kept.downloads ?? 0)) nights.set(night, doc);
      }
      return { releases: [...nights.values()].slice(0, count).map(toAlbum) };
    },
  }),
);
