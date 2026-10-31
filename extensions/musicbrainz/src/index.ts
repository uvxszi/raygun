// MusicBrainz: song search from the open music database. Results carry no stream; Raygun finds
// one through `music.stream` providers when they play (M14). Artwork comes from the Cover Art
// Archive, loaded by the app rather than this extension.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

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

type Recording = {
  id: string;
  title: string;
  length?: number | null;
  score?: number;
  'artist-credit'?: { name: string; joinphrase?: string }[];
  releases?: Release[];
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

const search = defineProvider({
  kind: 'music.metadata',
  id: 'recordings',
  name: 'MusicBrainz',

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
