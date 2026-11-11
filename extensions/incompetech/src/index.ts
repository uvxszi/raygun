// Incompetech: Kevin MacLeod's music (https://incompetech.com), free under CC BY 4.0. The site
// publishes its catalogue for programs (`pieces.json`, `genre.json` and `collections.json`, listed
// in its llms.txt), with each piece's MP3 under `mp3-royaltyfree/`. The catalogue is read once and
// kept for a while; searches, charts, collections (as albums) and genre or mood songs come from it,
// so only the catalogue and the songs themselves are fetched. Every piece credits "Kevin MacLeod".
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Album, Chart, Track } from '@raygun/sdk';

const SITE = 'https://incompetech.com/music/royalty-free';
const ARTIST = 'Kevin MacLeod';
/** How long a read catalogue is kept before it's read again. */
const KEEP_MS = 12 * 60 * 60 * 1000;

type Piece = {
  title?: string | null;
  filename?: string | null;
  /** "hh:mm:ss". */
  length?: string | null;
  instruments?: string | null;
  /** A genre id, as text. */
  genre?: string | number | null;
  bpm?: string | number | null;
  description?: string | null;
  /** Comma-separated feels ("Calming, Relaxed"). */
  feel?: string | null;
  /** "YYYY-MM-DD". */
  uploaded?: string | null;
  collection?: string | number | null;
};
type GenreItem = { id: number | string; genre: string };
type CollectionItem = {
  id: number | string;
  collection_name: string;
  collection_category?: string;
};

type Catalogue = {
  pieces: Piece[];
  genres: Map<string, string>;
  collections: Map<string, CollectionItem>;
  readAt: number;
};

let kept: Catalogue | null = null;
let reading: Promise<Catalogue> | null = null;

const read = async (): Promise<Catalogue> => {
  const [pieces, genres, collections] = await Promise.all([
    fetchJson<Piece[]>(`${SITE}/pieces.json`),
    fetchJson<GenreItem[]>(`${SITE}/genre.json`),
    fetchJson<CollectionItem[]>(`${SITE}/collections.json`),
  ]);
  return {
    pieces: (Array.isArray(pieces) ? pieces : []).filter(
      (piece) => typeof piece.title === 'string' && piece.title.trim() && piece.filename,
    ),
    genres: new Map((genres ?? []).map((item) => [String(item.id), item.genre])),
    collections: new Map((collections ?? []).map((item) => [String(item.id), item])),
    readAt: Date.now(),
  };
};

/** The catalogue, read at most once at a time and kept for {@link KEEP_MS}. */
const catalogue = async (): Promise<Catalogue> => {
  if (kept && Date.now() - kept.readAt < KEEP_MS) return kept;
  reading ??= read().finally(() => {
    reading = null;
  });
  try {
    kept = await reading;
  } catch (error) {
    // An older catalogue still beats none.
    if (kept) return kept;
    throw error;
  }
  return kept;
};

/** Lowercase words of letters and digits, single-spaced. */
const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .replace(/['’]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

const wordsOf = (text: string) => simple(text).split(' ').filter(Boolean);

const parseLength = (length: string | null | undefined) => {
  if (!length) return null;
  const seconds = length
    .split(':')
    .reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

/** A file name as an address part; a few names are already escaped ("Joey%27s"). */
const fileAddress = (filename: string) => {
  let name = filename;
  try {
    name = decodeURIComponent(filename);
  } catch {
    // A lone "%" stays as written.
  }
  return `${SITE}/mp3-royaltyfree/${encodeURIComponent(name)}`;
};

const albumOf = (piece: Piece, books: Catalogue) => {
  const collection = books.collections.get(String(piece.collection ?? ''));
  return collection?.collection_name?.trim() || null;
};

const toTrack = (piece: Piece, books: Catalogue): Track => ({
  id: piece.filename ?? piece.title ?? '',
  title: (piece.title ?? '').trim(),
  artist: ARTIST,
  album: albumOf(piece, books),
  durationMs: parseLength(piece.length),
  source: { kind: 'url', url: fileAddress(piece.filename ?? ''), headers: {} },
});

const genreOf = (piece: Piece, books: Catalogue) =>
  books.genres.get(String(piece.genre ?? '')) ?? '';

const feelsOf = (piece: Piece) =>
  (piece.feel ?? '')
    .split(',')
    .map((feel) => simple(feel))
    .filter(Boolean);

/** Newest first; pieces without a date last. */
const newestFirst = (a: Piece, b: Piece) => (b.uploaded ?? '').localeCompare(a.uploaded ?? '');

/** Some of the most used pieces, for the well-known chart, in this order. */
const WELL_KNOWN = [
  'Monkeys Spinning Monkeys',
  'Sneaky Snitch',
  'Fluffing a Duck',
  'Carefree',
  'Investigations',
  'Local Forecast - Elevator',
  'Wallpaper',
  'Pixel Peeker Polka - faster',
  'Merry Go',
  'Cipher',
  'Volatile Reaction',
  'Aurea Carmina',
  'Amazing Plan',
  'Spazzmatica Polka',
  'Itty Bitty 8 Bit',
  'Run Amok',
  'Life of Riley',
  'The Builder',
  'Bushwick Tarantella',
  'Thatched Villagers',
  'Sneaky Adventure',
  'Quirky Dog',
  'Fig Leaf Rag',
  'Mining by Moonlight',
  'Batty McFaddin',
  'Hyperfun',
  'Sugar Plum Breakdown',
  'Kool Kats',
  'Wholesome',
  'Cheery Monday',
  'Funkorama',
  'Easy Lemon',
  'Hidden Agenda',
  'Hitman',
  'Night on the Docks - Sax',
  'Lobby Time',
  'Electrodoodle',
  'Pamgaea',
  'Vivacity',
  'Airport Lounge',
  'Bossa Antigua',
  'Getting it Done',
  'Clean Soul',
  'Jaunty Gumption',
].map(simple);

const knownRank = (piece: Piece) => {
  const rank = WELL_KNOWN.indexOf(simple(piece.title));
  return rank < 0 ? Number.MAX_SAFE_INTEGER : rank;
};

/** Well-known pieces first, then the newest. */
const byStanding = (a: Piece, b: Piece) => knownRank(a) - knownRank(b) || newestFirst(a, b);

/** Raygun's genre names for Incompetech's genres, so genre pages find them. */
const GENRE_TAGS: Record<string, string> = {
  Soundtrack: 'Soundtrack',
  'Silent Film Score': 'Soundtrack',
  Electronica: 'Electronic',
  Jazz: 'Jazz',
  Rock: 'Rock',
  Classical: 'Classical',
  Funk: 'Funk',
  Latin: 'Latin',
  Pop: 'Pop',
  Blues: 'Blues',
  Reggae: 'Reggae',
  Disco: 'Disco',
  Ska: 'Ska',
  Urban: 'Hip-Hop',
};
/** Genres too mixed or too short to make a chart of. */
const NO_CHART = new Set(['Stings', 'Unclassifiable']);
/** A genre needs this many pieces for a chart of its own. */
const CHART_MIN = 15;

/** The feels that stand for each of Raygun's moods. */
const MOOD_FEELS: Record<string, string[]> = {
  chill: ['relaxed', 'calming'],
  energetic: ['driving', 'intense', 'action', 'aggressive'],
  happy: ['bright', 'uplifting', 'bouncy', 'humorous'],
  sad: ['somber'],
  dark: ['dark', 'eerie', 'unnerving', 'suspenseful'],
  party: ['grooving', 'bouncy'],
  focus: ['calming', 'mystical'],
  workout: ['driving', 'action', 'intense'],
  sleep: ['calming'],
};
const CHART_MOODS = ['Chill', 'Energetic', 'Happy', 'Sad', 'Dark'];

const inGenre = (wanted: string[], books: Catalogue) => (piece: Piece) => {
  const genre = genreOf(piece, books);
  const tag = GENRE_TAGS[genre];
  return wanted.includes(simple(genre)) || (tag !== undefined && wanted.includes(simple(tag)));
};

const inMood = (wanted: string[]) => {
  const feels = new Set(wanted.flatMap((name) => [name, ...(MOOD_FEELS[name] ?? [])]));
  return (piece: Piece) => feelsOf(piece).some((feel) => feels.has(feel));
};

const atMost = (limit: number, fallback = 50) => Math.max(1, Math.min(limit || fallback, 200));

/** Pieces whose words hold every word asked for; matches in the title first. */
const searchPieces = (text: string, books: Catalogue) => {
  const wanted = wordsOf(text);
  if (wanted.length === 0) return [];
  const scored: { piece: Piece; score: number }[] = [];
  for (const piece of books.pieces) {
    const title = wordsOf(piece.title ?? '');
    const about = new Set([
      ...title,
      ...wordsOf(genreOf(piece, books)),
      ...wordsOf(piece.feel ?? ''),
      ...wordsOf(piece.instruments ?? ''),
      ...wordsOf(piece.description ?? ''),
      ...wordsOf(albumOf(piece, books) ?? ''),
    ]);
    if (!wanted.every((word) => about.has(word) || title.some((part) => part.startsWith(word)))) {
      continue;
    }
    const inTitle = wanted.filter((word) => title.some((part) => part.startsWith(word))).length;
    scored.push({ piece, score: inTitle * 10 + (simple(piece.title) === simple(text) ? 100 : 0) });
  }
  return scored
    .sort((a, b) => b.score - a.score || byStanding(a.piece, b.piece))
    .map((entry) => entry.piece);
};

const toAlbum = (collection: CollectionItem, books: Catalogue): Album => ({
  id: String(collection.id),
  title: collection.collection_name.trim(),
  artist: ARTIST,
  trackCount: books.pieces.filter(
    (piece) => String(piece.collection ?? '') === String(collection.id),
  ).length,
  kind: 'album',
});

const metadata = defineProvider({
  kind: 'music.metadata',
  id: 'search',
  name: 'Incompetech',

  async search({ query, limit }) {
    const books = await catalogue();
    const found = searchPieces(query, books);
    return { tracks: found.slice(0, atMost(limit, 20)).map((piece) => toTrack(piece, books)) };
  },

  async searchAlbums({ query, limit }) {
    const books = await catalogue();
    const wanted = wordsOf(query);
    const artistAsked = wanted.length > 0 && wanted.every((word) => simple(ARTIST).includes(word));
    const albums = [...books.collections.values()]
      .filter((collection) => {
        const about = wordsOf(
          `${collection.collection_name} ${collection.collection_category ?? ''}`,
        );
        return artistAsked || wanted.every((word) => about.some((part) => part.startsWith(word)));
      })
      .map((collection) => toAlbum(collection, books))
      .filter((album) => (album.trackCount ?? 0) > 0);
    return { albums: albums.slice(0, atMost(limit, 20)) };
  },

  async album({ id }) {
    const books = await catalogue();
    const collection = books.collections.get(id);
    if (!collection) throw new Error('Incompetech no longer has this collection');
    const tracks = books.pieces
      .filter((piece) => String(piece.collection ?? '') === id)
      .sort(
        (a, b) =>
          (a.uploaded ?? '').localeCompare(b.uploaded ?? '') ||
          (a.title ?? '').localeCompare(b.title ?? ''),
      )
      .map((piece) => toTrack(piece, books));
    return { album: toAlbum(collection, books), tracks };
  },

  async artistReleases({ query, limit }) {
    if (simple(query) !== simple(ARTIST)) return { releases: [] };
    const books = await catalogue();
    const albums = [...books.collections.values()]
      .map((collection) => toAlbum(collection, books))
      .filter((album) => (album.trackCount ?? 0) > 0);
    return { releases: albums.slice(0, atMost(limit, 50)) };
  },
});

const charts = defineProvider({
  kind: 'music.charts',
  id: 'charts',
  name: 'Incompetech',

  async charts() {
    const books = await catalogue();
    const counts = new Map<string, number>();
    for (const piece of books.pieces) {
      const genre = genreOf(piece, books);
      if (genre) counts.set(genre, (counts.get(genre) ?? 0) + 1);
    }
    const genres: Chart[] = [...books.genres.entries()]
      .filter(([, name]) => !NO_CHART.has(name) && (counts.get(name) ?? 0) >= CHART_MIN)
      .sort((a, b) => (counts.get(b[1]) ?? 0) - (counts.get(a[1]) ?? 0))
      .map(([id, name]) => ({
        id: `genre:${id}`,
        name: `${name} by Kevin MacLeod`,
        description: 'Kevin MacLeod, CC BY 4.0',
        tag: GENRE_TAGS[name] ?? null,
        group: 'genre',
      }));
    const moods: Chart[] = CHART_MOODS.map((mood) => ({
      id: `mood:${mood}`,
      name: `${mood} pieces by Kevin MacLeod`,
      description: 'Kevin MacLeod, CC BY 4.0',
      tag: mood,
      group: 'mood',
    }));
    return {
      charts: [
        {
          id: 'well-known',
          name: 'Kevin MacLeod: well-known pieces',
          description: 'Some of his most used pieces, CC BY 4.0',
          group: 'top',
        },
        {
          id: 'newest',
          name: 'Kevin MacLeod: newest pieces',
          description: 'Added to incompetech.com lately, CC BY 4.0',
          group: 'new',
        },
        ...genres,
        ...moods,
      ],
    };
  },

  async chart({ id, limit }) {
    const books = await catalogue();
    const count = atMost(limit);
    let pieces: Piece[];
    if (id === 'newest') {
      pieces = [...books.pieces].sort(newestFirst);
    } else if (id.startsWith('genre:')) {
      const genre = id.slice('genre:'.length);
      pieces = books.pieces.filter((piece) => String(piece.genre ?? '') === genre).sort(byStanding);
    } else if (id.startsWith('mood:')) {
      pieces = books.pieces.filter(inMood([simple(id.slice('mood:'.length))])).sort(byStanding);
    } else {
      pieces = books.pieces
        .filter((piece) => knownRank(piece) < Number.MAX_SAFE_INTEGER)
        .sort(byStanding);
    }
    return { tracks: pieces.slice(0, count).map((piece) => toTrack(piece, books)) };
  },

  async tagSongs({ tag, aliases, kind, limit }) {
    const books = await catalogue();
    const wanted = [tag, ...aliases].map(simple).filter(Boolean);
    let pieces: Piece[];
    if (kind === 'mood') {
      pieces = books.pieces.filter(inMood(wanted));
    } else if (wanted.includes('instrumental')) {
      // Nearly everything here is instrumental.
      pieces = [...books.pieces];
    } else {
      pieces = books.pieces.filter(inGenre(wanted, books));
    }
    pieces.sort(byStanding);
    return {
      tracks: pieces.slice(0, atMost(limit, 30)).map((piece) => toTrack(piece, books)),
      name: `${tag} by Kevin MacLeod`,
    };
  },
});

// Streams for his pieces found elsewhere (a saved or imported song by Kevin MacLeod).
const streams = defineProvider({
  kind: 'music.stream',
  id: 'stream',
  name: 'Incompetech',

  async candidates({ title, artist, limit }) {
    if (!artist || !simple(artist).includes(simple(ARTIST))) return { candidates: [] };
    const wanted = simple(title.replace(/\([^)]*\)|\[[^\]]*\]/g, ' '));
    if (!wanted) return { candidates: [] };
    const books = await catalogue();
    // The piece itself; else its versions ("Pixel Peeker Polka - faster").
    const exact = books.pieces.filter((piece) => simple(piece.title) === wanted);
    const matching =
      exact.length > 0
        ? exact
        : books.pieces.filter((piece) => simple(piece.title).startsWith(`${wanted} `));
    return {
      candidates: matching.slice(0, Math.max(1, limit)).map((piece) => {
        const track = toTrack(piece, books);
        return {
          id: String(track.id),
          title: track.title,
          artist: ARTIST,
          durationMs: track.durationMs ?? null,
          quality: 'MP3',
          source: track.source ?? null,
        };
      }),
    };
  },
});

registerProvider(metadata);
registerProvider(charts);
registerProvider(streams);
