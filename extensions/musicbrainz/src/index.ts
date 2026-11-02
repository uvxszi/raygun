// MusicBrainz: song search from the open music database. Results carry no stream; Raygun finds
// one through `music.stream` providers when they play (M14). Artwork comes from the Cover Art
// Archive, loaded by the app rather than this extension.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Album, Track } from '@raygun/sdk';

const API = 'https://musicbrainz.org/ws/2';
// MusicBrainz asks every client to identify itself with a contact URL.
const HEADERS = {
  'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )',
  Accept: 'application/json',
};
// MusicBrainz allows one request per second per IP address and answers 503 beyond that.
const MIN_INTERVAL_MS = 1100;
const RETRIES = 2;
// Fetched per search, then re-ranked: free-text search ranks covers that mention the artist in
// their title above the artist's own recordings.
const FETCHED = 50;

type Release = {
  id: string;
  title: string;
  status?: string;
  'release-group'?: { 'primary-type'?: string };
};

type Tag = { name: string; count?: number };

type Recording = {
  id: string;
  title: string;
  length?: number | null;
  score?: number;
  'artist-credit'?: { name: string; joinphrase?: string; artist?: { id: string } }[];
  releases?: Release[];
  tags?: Tag[];
};

// Lucene syntax characters would change the meaning of the user's query.
const escapeQuery = (query: string) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const words = (text: string) =>
  text
    .toLowerCase()
    .normalize('NFD')
    .replace(/[̀-ͯ]/g, '')
    .split(/[^a-z0-9]+/)
    .filter((word) => word.length > 2);

const credit = (recording: Recording) =>
  (recording['artist-credit'] ?? [])
    .map((part) => part.name + (part.joinphrase ?? ''))
    .join('')
    .trim();

/** Prefers an official album over compilations and singles for the "album" line and artwork. */
const bestRelease = (releases: Release[] = []) =>
  releases.find(
    (release) =>
      release.status === 'Official' && release['release-group']?.['primary-type'] === 'Album',
  ) ??
  releases.find((release) => release.status === 'Official') ??
  releases[0];

const rank = (recordings: Recording[], query: string) => {
  const asked = new Set(words(query));
  const scored = recordings.map((recording) => {
    const byNamedArtist = words(credit(recording)).some((word) => asked.has(word));
    const score = (recording.score ?? 0) + (byNamedArtist ? 60 : 0) + (recording.length ? 10 : 0);
    return { recording, score };
  });
  scored.sort((a, b) => b.score - a.score);
  // The same song appears on many releases; keep the best-ranked one of each.
  const seen = new Set<string>();
  return scored
    .map(({ recording }) => recording)
    .filter((recording) => {
      const key = `${words(recording.title).join(' ')}|${words(credit(recording)).join(' ')}`;
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
};

const toTrack = (recording: Recording): Track => {
  const release = bestRelease(recording.releases);
  return {
    id: recording.id,
    title: recording.title,
    artist: credit(recording) || null,
    album: release?.title ?? null,
    durationMs: recording.length ?? null,
    artworkUrl: release ? `https://coverartarchive.org/release/${release.id}/front-250` : null,
  };
};

let lastRequestAt = 0;

/** Fetches within the rate limit, retrying after a pause when MusicBrainz says it's busy. */
const fetchPaced = async (url: string) => {
  for (let attempt = 0; ; attempt += 1) {
    const wait = lastRequestAt + MIN_INTERVAL_MS - Date.now();
    if (wait > 0) await raygun.time.sleep(wait);
    lastRequestAt = Date.now();
    const response = await raygun.http.fetch(url, { headers: HEADERS });
    if (response.status !== 503 || attempt >= RETRIES) return response;
    raygun.log.info('MusicBrainz is busy, retrying');
    await raygun.time.sleep(MIN_INTERVAL_MS * (attempt + 1));
  }
};

type ReleaseGroup = {
  id: string;
  title: string;
  'first-release-date'?: string;
  'primary-type'?: string;
  'artist-credit'?: { name: string; joinphrase?: string }[];
};

type ReleaseWithTracks = {
  id: string;
  title: string;
  media?: { tracks?: { title: string; length?: number | null; recording?: { id: string } }[] }[];
  'artist-credit'?: { name: string; joinphrase?: string }[];
};

const creditOf = (credits: { name: string; joinphrase?: string }[] = []) =>
  credits
    .map((part) => part.name + (part.joinphrase ?? ''))
    .join('')
    .trim();

const toAlbum = (group: ReleaseGroup): Album => ({
  id: group.id,
  title: group.title,
  artist: creditOf(group['artist-credit']) || null,
  year: Number(group['first-release-date']?.slice(0, 4)) || null,
  artworkUrl: `https://coverartarchive.org/release-group/${group.id}/front-250`,
});

const search = defineProvider({
  kind: 'music.metadata',
  id: 'recordings',
  name: 'MusicBrainz',

  async searchArtists({ query, limit }) {
    const cleaned = escapeQuery(query);
    if (cleaned === '') return { artists: [] };
    const found = await getJson<{ artists?: { id: string; name: string }[] }>(
      `${API}/artist?query=${encodeURIComponent(cleaned)}&limit=${Math.min(limit, 25)}&fmt=json`,
    );
    return {
      artists: (found.artists ?? []).map((artist) => ({ id: artist.id, name: artist.name })),
    };
  },

  async searchAlbums({ query, limit }) {
    const cleaned = escapeQuery(query);
    if (cleaned === '') return { albums: [] };
    const found = await getJson<{ 'release-groups'?: ReleaseGroup[] }>(
      `${API}/release-group?query=${encodeURIComponent(cleaned)}&limit=${Math.min(limit, 25)}&fmt=json`,
    );
    return { albums: (found['release-groups'] ?? []).map(toAlbum) };
  },

  async album({ id }) {
    if (!/^[0-9a-f-]{36}$/.test(id)) throw new Error(`Unknown album ${id}`);
    const group = await getJson<ReleaseGroup>(
      `${API}/release-group/${id}?inc=artist-credits&fmt=json`,
    );
    // An official release of the group carries the track list.
    const releases = await getJson<{ releases?: ReleaseWithTracks[] }>(
      `${API}/release?release-group=${id}&inc=recordings+artist-credits&status=official&limit=1&fmt=json`,
    );
    const release = releases.releases?.[0];
    const artist = creditOf(release?.['artist-credit'] ?? group['artist-credit']) || null;
    const tracks: Track[] = (release?.media ?? []).flatMap((medium) =>
      (medium.tracks ?? []).map((track) => ({
        id: track.recording?.id,
        title: track.title,
        artist,
        album: group.title,
        durationMs: track.length ?? null,
        artworkUrl: `https://coverartarchive.org/release-group/${id}/front-250`,
      })),
    );
    return { album: toAlbum(group), tracks };
  },

  async search({ query, limit }) {
    const cleaned = escapeQuery(query);
    if (cleaned === '') return { tracks: [] };
    const url = `${API}/recording?query=${encodeURIComponent(cleaned)}&limit=${FETCHED}&fmt=json`;
    const response = await fetchPaced(url);
    if (response.status === 503) {
      throw new Error('MusicBrainz is busy; try again in a moment');
    }
    if (!response.ok) {
      throw new Error(`MusicBrainz answered HTTP ${response.status}`);
    }
    const body = await response.json<{ recordings?: Recording[] }>();
    return {
      tracks: rank(body.recordings ?? [], cleaned)
        .slice(0, limit)
        .map(toTrack),
    };
  },
});

registerProvider(search);

const simple = (text: string) => words(text).join(' ');

/** Tags weighted by votes, the most voted 1 (M46.9). The app keeps the ones that are genres. */
const weighted = (tags: Tag[] = []) => {
  const most = Math.max(1, ...tags.map((tag) => tag.count ?? 1));
  return tags
    .filter((tag) => (tag.count ?? 1) > 0)
    .map((tag) => ({ name: tag.name, weight: (tag.count ?? 1) / most }));
};

const getJson = async <T>(url: string): Promise<T> => {
  const response = await fetchPaced(url);
  if (!response.ok) throw new Error(`MusicBrainz answered HTTP ${response.status}`);
  return response.json<T>();
};

/**
 * A song's genres (M46.9): the recording's own tags, or its artist's genres when nobody tagged the
 * recording. One request a second, as MusicBrainz asks.
 */
registerProvider(
  defineProvider({
    kind: 'music.tags',
    id: 'genres',
    name: 'MusicBrainz',
    async tags({ title, artist }) {
      const parts = [`recording:"${escapeQuery(title)}"`];
      if (artist) parts.push(`artist:"${escapeQuery(artist)}"`);
      const found = await getJson<{ recordings?: Recording[] }>(
        `${API}/recording?query=${encodeURIComponent(parts.join(' AND '))}&limit=10&fmt=json`,
      );
      const wanted = simple(title);
      const matches = (found.recordings ?? []).filter(
        (recording) =>
          simple(recording.title) === wanted &&
          (!artist || words(credit(recording)).some((word) => words(artist).includes(word))),
      );
      // The same song is recorded on many releases; some copies carry the tags.
      const match = matches.find((recording) => recording.tags?.length) ?? matches[0];
      if (!match) return { tags: [] };
      if (match.tags?.length) return { tags: weighted(match.tags) };
      const artistId = match['artist-credit']?.[0]?.artist?.id;
      if (!artistId) return { tags: [] };
      const byArtist = await getJson<{ genres?: Tag[]; tags?: Tag[] }>(
        `${API}/artist/${artistId}?inc=genres+tags&fmt=json`,
      );
      return { tags: weighted(byArtist.genres?.length ? byArtist.genres : byArtist.tags) };
    },
  }),
);

// ---- Artist facts and song credits (M46.14) ----

/** Wikimedia asks clients to name themselves too. */
const WIKI_HEADERS = HEADERS;

type Relation = {
  type: string;
  direction?: string;
  attributes?: string[];
  url?: { resource: string };
  artist?: { name: string };
  work?: { id: string; title: string; relations?: Relation[] };
};

type ArtistRecord = {
  id: string;
  name: string;
  type?: string;
  score?: number;
  country?: string;
  area?: { name: string } | null;
  'begin-area'?: { name: string } | null;
  'life-span'?: { begin?: string | null; end?: string | null; ended?: boolean };
  relations?: Relation[];
};

const unique = (names: string[]) => [...new Set(names.filter((name) => name.trim() !== ''))];

const year = (date: string | null | undefined) => date?.slice(0, 4) ?? '';

/** Wikipedia's summary of the page Wikidata links the artist to: a few sentences and a picture. */
const wikipediaSummary = async (wikidataUrl: string) => {
  const id = wikidataUrl.split('/').pop();
  if (!id || !/^Q\d+$/.test(id)) return null;
  const entity = await raygun.http.fetch(
    `https://www.wikidata.org/wiki/Special:EntityData/${id}.json`,
    { headers: WIKI_HEADERS },
  );
  if (!entity.ok) return null;
  const data = await entity.json<{
    entities?: Record<string, { sitelinks?: Record<string, { title: string }> }>;
  }>();
  const title = data.entities?.[id]?.sitelinks?.enwiki?.title;
  if (!title) return null;
  const summary = await raygun.http.fetch(
    `https://en.wikipedia.org/api/rest_v1/page/summary/${encodeURIComponent(title.replace(/ /g, '_'))}`,
    { headers: WIKI_HEADERS },
  );
  if (!summary.ok) return null;
  const page = await summary.json<{
    extract?: string;
    thumbnail?: { source: string };
    originalimage?: { source: string };
    content_urls?: { desktop?: { page?: string } };
  }>();
  return {
    bio: page.extract ?? null,
    imageUrl: page.thumbnail?.source ?? page.originalimage?.source ?? null,
    url: page.content_urls?.desktop?.page ?? null,
  };
};

const capital = (text: string) => text.charAt(0).toUpperCase() + text.slice(1);

/** How a relationship reads as a credit: "Produced by", "Guitar", "Lead vocals"... */
const roleOf = (relation: Relation): string => {
  const attribute = relation.attributes?.[0];
  switch (relation.type) {
    case 'producer':
      return 'Produced by';
    case 'mix':
      return 'Mixed by';
    case 'mastering':
      return 'Mastered by';
    case 'engineer':
    case 'audio':
    case 'recording':
    case 'sound':
      return 'Engineered by';
    case 'remixer':
      return 'Remixed by';
    case 'arranger':
    case 'instrument arranger':
    case 'vocal arranger':
      return 'Arranged by';
    case 'composer':
      return 'Composed by';
    case 'lyricist':
      return 'Lyrics by';
    case 'writer':
      return 'Written by';
    case 'vocal':
      return attribute ? capital(attribute) : 'Vocals';
    case 'instrument':
      return attribute ? capital(attribute) : 'Instruments';
    default:
      return capital(relation.type);
  }
};

/** The credits shown first, in Spotify's order; everything else follows in the order found. */
const FIRST_ROLES = ['Performed by', 'Written by', 'Composed by', 'Lyrics by', 'Produced by'];

registerProvider(
  defineProvider({
    kind: 'music.info',
    id: 'info',
    name: 'MusicBrainz',
    async artist({ name }) {
      const found = await getJson<{ artists?: ArtistRecord[] }>(
        `${API}/artist?query=${encodeURIComponent(`artist:"${escapeQuery(name)}"`)}&limit=5&fmt=json`,
      );
      const wanted = simple(name);
      const candidates = found.artists ?? [];
      const match =
        candidates.find((candidate) => simple(candidate.name) === wanted) ??
        candidates.find((candidate) => (candidate.score ?? 0) >= 95);
      if (!match) return { artist: null };
      const details = await getJson<ArtistRecord>(
        `${API}/artist/${match.id}?inc=url-rels+artist-rels&fmt=json`,
      );
      const relations = details.relations ?? [];
      const members = unique(
        relations
          .filter(
            (relation) => relation.type === 'member of band' && relation.direction === 'backward',
          )
          .map((relation) => relation.artist?.name ?? ''),
      );
      const wikidata = relations.find((relation) => relation.type === 'wikidata')?.url?.resource;
      let summary: Awaited<ReturnType<typeof wikipediaSummary>> = null;
      if (wikidata) {
        try {
          summary = await wikipediaSummary(wikidata);
        } catch (error) {
          raygun.log.info(`Wikipedia had no summary: ${String(error)}`);
        }
      }
      const span = details['life-span'];
      const begin = year(span?.begin);
      const active = begin ? `${begin}–${span?.ended ? year(span.end) : ''}` : null;
      return {
        artist: {
          name: details.name,
          kind: details.type ?? null,
          bio: summary?.bio ?? null,
          imageUrl: summary?.imageUrl ?? null,
          url: summary?.url ?? `https://musicbrainz.org/artist/${details.id}`,
          origin: details['begin-area']?.name ?? details.area?.name ?? details.country ?? null,
          active,
          members,
        },
      };
    },
    async credits({ title, artist }) {
      const parts = [`recording:"${escapeQuery(title)}"`];
      if (artist) parts.push(`artist:"${escapeQuery(artist)}"`);
      const found = await getJson<{ recordings?: Recording[] }>(
        `${API}/recording?query=${encodeURIComponent(parts.join(' AND '))}&limit=10&fmt=json`,
      );
      const wanted = simple(title);
      const match = (found.recordings ?? []).find(
        (recording) =>
          simple(recording.title) === wanted &&
          (!artist || words(credit(recording)).some((word) => words(artist).includes(word))),
      );
      if (!match) return { credits: [] };
      const details = await getJson<Recording & { relations?: Relation[] }>(
        `${API}/recording/${match.id}?inc=artist-credits+artist-rels+work-rels+work-level-rels&fmt=json`,
      );
      const groups = new Map<string, string[]>();
      const add = (role: string, names: string[]) =>
        groups.set(role, unique([...(groups.get(role) ?? []), ...names]));
      add(
        'Performed by',
        (details['artist-credit'] ?? []).map((part) => part.name),
      );
      for (const relation of details.relations ?? []) {
        if (relation.artist) add(roleOf(relation), [relation.artist.name]);
        for (const workRelation of relation.work?.relations ?? []) {
          if (workRelation.artist) add(roleOf(workRelation), [workRelation.artist.name]);
        }
      }
      const ordered = [
        ...FIRST_ROLES.filter((role) => groups.has(role)),
        ...[...groups.keys()].filter((role) => !FIRST_ROLES.includes(role)),
      ];
      return {
        credits: ordered.map((role) => ({ role, names: groups.get(role) ?? [] })),
        url: `https://musicbrainz.org/recording/${details.id}`,
      };
    },
  }),
);
