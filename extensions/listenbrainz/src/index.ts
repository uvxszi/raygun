// ListenBrainz: charts and recommendations from the open listening-history project of the
// MetaBrainz Foundation (its statistics and similarity data are CC0). Songs carry no stream;
// Raygun plays them through the music sources the user has. Cover art comes from the Cover Art
// Archive, loaded by the app.
//
// - Charts: the songs most listened to this week, month, year and of all time; any genre's songs
//   as a radio (`tagSongs`, LB Radio's tag mode).
// - Recommendations (`music.recommend`): artists like an artist (ListenBrainz Labs' session-based
//   similarity), an artist's most listened-to songs, a radio of similar artists' songs or of
//   songs like one song, and an artist's fresh releases. An artist's name becomes a MusicBrainz
//   id through MusicBrainz's search, kept for a month.
//
// Every call here is one ListenBrainz offers without an account.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { Artist, Chart, Track } from '@raygun/sdk';

const API = 'https://api.listenbrainz.org/1';
const LABS = 'https://labs.api.listenbrainz.org';
const MUSICBRAINZ = 'https://musicbrainz.org/ws/2';
const HEADERS = {
  'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )',
  Accept: 'application/json',
};
/** ListenBrainz gives at most this many songs a chart request. */
const MOST = 100;
/** The similarity data sets ListenBrainz Labs keeps; these are the ones its own pages use. */
const SIMILAR_ARTISTS =
  'session_based_days_7500_session_300_contribution_5_threshold_10_limit_100_filter_True_skip_30';
const SIMILAR_RECORDINGS =
  'session_based_days_9000_session_300_contribution_5_threshold_15_limit_50_skip_30';
const HOUR = 60 * 60 * 1000;
const MONTH = 30 * 24 * HOUR;

/** The periods ListenBrainz counts over, as charts. */
const RANGES: { id: string; name: string; description: string }[] = [
  { id: 'week', name: 'Top songs this week', description: 'Most listened to on ListenBrainz' },
  { id: 'month', name: 'Top songs this month', description: 'Most listened to on ListenBrainz' },
  { id: 'year', name: 'Top songs this year', description: 'Most listened to on ListenBrainz' },
  {
    id: 'all_time',
    name: 'Top songs of all time',
    description: 'Most listened to on ListenBrainz',
  },
];

type Recording = {
  track_name: string;
  artist_name?: string | null;
  release_name?: string | null;
  recording_mbid?: string | null;
  caa_id?: number | null;
  caa_release_mbid?: string | null;
};

type Answer = { payload?: { recordings?: Recording[] } };

/** The Cover Art Archive's 250 px picture of a release, when ListenBrainz names one. */
const coverArt = (caaId: number | null | undefined, release: string | null | undefined) =>
  caaId && release ? `https://coverartarchive.org/release/${release}/${caaId}-250.jpg` : null;

export const toTrack = (recording: Recording, index: number): Track => ({
  id: recording.recording_mbid ?? `${index}`,
  title: recording.track_name,
  artist: recording.artist_name ?? null,
  album: recording.release_name ?? null,
  artworkUrl: coverArt(recording.caa_id, recording.caa_release_mbid),
});

const get = <T>(url: string) => fetchJson<T>(url, { headers: HEADERS });

/** Lower case, accents and punctuation gone: how two spellings of a name are compared. */
const normalized = (text: string) =>
  text
    .normalize('NFKD')
    .replace(/[\u0300-\u036f]/g, '')
    .toLowerCase()
    .replace(/&/g, ' and ')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .replace(/^the /, '')
    .trim();

/** A song's title without versions and remasters, for telling repeats apart. */
const songKey = (title: string) =>
  normalized(title.replace(/\s*[([].*?[)\]]\s*/g, ' ').replace(/\s+-\s+.*$/, ''));

const clamp = (value: number, low: number, high: number) =>
  Math.max(low, Math.min(Number.isFinite(value) ? Math.floor(value) : low, high));

// ---- MusicBrainz ids ----

/** MusicBrainz asks for one request a second; requests here wait their turn. */
let musicBrainzTurn: Promise<unknown> = Promise.resolve();
let lastMusicBrainz = 0;

const musicBrainz = <T>(url: string): Promise<T> => {
  const ask = musicBrainzTurn.then(async () => {
    const wait = lastMusicBrainz + 1100 - Date.now();
    if (wait > 0) await raygun.time.sleep(wait);
    lastMusicBrainz = Date.now();
    const response = await raygun.http.fetch(url, { headers: HEADERS });
    lastMusicBrainz = Date.now();
    if (response.status === 503) {
      // Over the limit for a moment: one more try.
      await raygun.time.sleep(1500);
      lastMusicBrainz = Date.now();
      return fetchJson<T>(url, { headers: HEADERS });
    }
    if (!response.ok) throw new Error(`HTTP ${response.status} from ${url}`);
    return response.json<T>();
  });
  musicBrainzTurn = ask.catch(() => undefined);
  return ask;
};

type FoundArtist = { id: string; name: string; score?: number; aliases?: { name: string }[] };
type Remembered = { id: string | null; at: number };

/** The MusicBrainz id of the artist named `name`, or null when MusicBrainz doesn't know one. */
const artistId = async (name: string): Promise<string | null> => {
  const wanted = normalized(name);
  if (!wanted) return null;
  const key = `artist:${wanted}`;
  const kept = raygun.storage.get<Remembered>(key);
  if (kept && Date.now() - kept.at < MONTH) return kept.id;
  const found = await musicBrainz<{ artists?: FoundArtist[] }>(
    `${MUSICBRAINZ}/artist/?${query({ query: `artist:"${name.replace(/["\\]/g, ' ')}"`, fmt: 'json', limit: 5 })}`,
  );
  const artists = found.artists ?? [];
  const exact = artists.find(
    (artist) =>
      normalized(artist.name) === wanted ||
      (artist.aliases ?? []).some((alias) => normalized(alias.name) === wanted),
  );
  const id = exact?.id ?? (artists[0] && (artists[0].score ?? 0) >= 95 ? artists[0].id : null);
  remember(key, { id, at: Date.now() });
  return id;
};

/** Keeps a value, making room by forgetting the oldest artist ids when the store is full. */
const remember = (key: string, value: Remembered) => {
  try {
    raygun.storage.set(key, value);
  } catch {
    const old = raygun.storage
      .keys()
      .filter((name) => name.startsWith('artist:'))
      .map((name) => ({ name, at: raygun.storage.get<Remembered>(name)?.at ?? 0 }))
      .sort((a, b) => a.at - b.at);
    for (const entry of old.slice(0, Math.ceil(old.length / 2))) raygun.storage.remove(entry.name);
    raygun.storage.set(key, value);
  }
};

const needArtist = async (name: string) => {
  const id = await artistId(name);
  if (!id) throw new Error(`MusicBrainz doesn't know an artist named ${name}`);
  return id;
};

// ---- Songs by MusicBrainz id ----

type RecordingMetadata = {
  recording?: { name?: string; length?: number | null };
  artist?: { name?: string };
  release?: { name?: string; caa_id?: number | null; caa_release_mbid?: string | null };
};

/** Songs for recording ids, in the order given, those ListenBrainz can't name left out. */
const songsOf = async (ids: string[]): Promise<Track[]> => {
  const tracks: Track[] = [];
  for (let start = 0; start < ids.length; start += 50) {
    const batch = ids.slice(start, start + 50);
    const found = await get<Record<string, RecordingMetadata>>(
      `${API}/metadata/recording/?${query({ recording_mbids: batch.join(','), inc: 'artist release' })}`,
    );
    for (const id of batch) {
      const meta = found[id];
      const title = meta?.recording?.name?.trim();
      if (!title) continue;
      tracks.push({
        id,
        title,
        artist: meta?.artist?.name ?? null,
        album: meta?.release?.name ?? null,
        durationMs: meta?.recording?.length ?? null,
        artworkUrl: coverArt(meta?.release?.caa_id, meta?.release?.caa_release_mbid),
      });
    }
  }
  return tracks;
};

/** Songs without repeats of the same song (other versions, remasters). */
const distinct = (tracks: Track[]) => {
  const seen = new Set<string>();
  return tracks.filter((track) => {
    const key = `${normalized(track.artist ?? '')}|${songKey(track.title)}`;
    if (seen.has(key)) return false;
    seen.add(key);
    return true;
  });
};

type RadioRecording = {
  recording_mbid: string;
  similar_artist_mbid?: string;
  similar_artist_name?: string;
  total_listen_count?: number;
};

/**
 * LB Radio's songs for an artist: the artist's own (`similar` 0) or theirs and similar artists',
 * picked at random among recordings at least as popular as `popularity` (a percentile).
 */
const artistRadio = (id: string, similar: number, perArtist: number, popularity: number) =>
  get<Record<string, RadioRecording[]>>(
    `${API}/lb-radio/artist/${id}?${query({
      mode: 'easy',
      max_similar_artists: similar,
      max_recordings_per_artist: perArtist,
      pop_begin: popularity,
      pop_end: 100,
    })}`,
  );

/** Round-robin over each artist's songs, so a radio doesn't play one artist for long. */
const interleave = (groups: RadioRecording[][]) => {
  const order: string[] = [];
  const longest = Math.max(0, ...groups.map((group) => group.length));
  for (let index = 0; index < longest; index += 1) {
    for (const group of groups) {
      const recording = group[index];
      if (recording) order.push(recording.recording_mbid);
    }
  }
  return order;
};

// ---- Fresh releases ----

type FreshRelease = {
  artist_credit_name?: string;
  artist_mbids?: string[];
  release_mbid?: string;
  release_name?: string;
  release_date?: string;
  caa_id?: number | null;
  caa_release_mbid?: string | null;
};
type Fresh = { at: number; releases: FreshRelease[] };

/** The site's fresh releases of the last `days` days, kept for an hour. */
const freshKept = new Map<number, Fresh>();

const freshReleases = async (days: number): Promise<FreshRelease[]> => {
  const kept = freshKept.get(days);
  if (kept && Date.now() - kept.at < HOUR) return kept.releases;
  const answer = await get<{ payload?: { releases?: FreshRelease[] } }>(
    `${API}/explore/fresh-releases/?${query({ days, sort: 'release_date', past: true, future: false })}`,
  );
  // Only what's needed is kept: the list is long.
  const releases = (answer.payload?.releases ?? []).map((release) => ({
    artist_mbids: release.artist_mbids,
    release_mbid: release.release_mbid,
    release_name: release.release_name,
    release_date: release.release_date,
    caa_id: release.caa_id,
    caa_release_mbid: release.caa_release_mbid,
  }));
  freshKept.clear();
  freshKept.set(days, { at: Date.now(), releases });
  return releases;
};

type MusicBrainzRelease = {
  title?: string;
  'artist-credit'?: { name?: string; joinphrase?: string }[];
  media?: {
    tracks?: {
      title?: string;
      length?: number | null;
      recording?: { id?: string; title?: string; length?: number | null };
      'artist-credit'?: { name?: string; joinphrase?: string }[];
    }[];
  }[];
};

const credit = (names: { name?: string; joinphrase?: string }[] | undefined) =>
  (names ?? [])
    .map((part) => `${part.name ?? ''}${part.joinphrase ?? ''}`)
    .join('')
    .trim() || null;

/** A release's songs, from MusicBrainz. */
const releaseSongs = async (release: FreshRelease): Promise<Track[]> => {
  const found = await musicBrainz<MusicBrainzRelease>(
    `${MUSICBRAINZ}/release/${release.release_mbid}?${query({ inc: 'recordings artist-credits', fmt: 'json' })}`,
  );
  const album = found.title ?? release.release_name ?? null;
  return (found.media ?? []).flatMap((medium) =>
    (medium.tracks ?? []).flatMap((track): Track[] => {
      const title = (track.title ?? track.recording?.title ?? '').trim();
      if (!title) return [];
      return [
        {
          id: track.recording?.id ?? `${release.release_mbid}:${title}`,
          title,
          artist: credit(track['artist-credit']) ?? credit(found['artist-credit']),
          album,
          durationMs: track.length ?? track.recording?.length ?? null,
          artworkUrl: coverArt(release.caa_id, release.caa_release_mbid),
        },
      ];
    }),
  );
};

// ---- Song radios ----

type FoundRecording = {
  recording_mbid: string;
  recording_name: string;
  artist_credit_name: string;
};
type SimilarRecording = {
  recording_mbid: string;
  recording_name?: string;
  artist_credit_name?: string;
  release_name?: string;
  caa_id?: number | null;
  caa_release_mbid?: string | null;
};

/** The recording of `title` by `artist`, from ListenBrainz Labs' search. */
const recordingOf = async (artist: string, title: string): Promise<string | null> => {
  const found = await get<FoundRecording[]>(
    `${LABS}/recording-search/json?${query({ query: `${artist} ${title}` })}`,
  );
  const wantedArtist = normalized(artist);
  const wantedTitle = songKey(title);
  const match = found.find(
    (recording) =>
      normalized(recording.artist_credit_name ?? '') === wantedArtist &&
      songKey(recording.recording_name ?? '') === wantedTitle,
  );
  return match?.recording_mbid ?? null;
};

const songRadio = async (recording: string): Promise<Track[]> => {
  const similar = await get<SimilarRecording[]>(
    `${LABS}/similar-recordings/json?${query({ recording_mbids: recording, algorithm: SIMILAR_RECORDINGS })}`,
  );
  return similar.flatMap((found): Track[] =>
    found.recording_name?.trim()
      ? [
          {
            id: found.recording_mbid,
            title: found.recording_name.trim(),
            artist: found.artist_credit_name ?? null,
            album: found.release_name ?? null,
            artworkUrl: coverArt(found.caa_id, found.caa_release_mbid),
          },
        ]
      : [],
  );
};

// ---- Genre radios ----

const tagKept = new Map<string, { at: number; tracks: Track[] }>();

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'ListenBrainz',
    async charts() {
      const charts: Chart[] = RANGES.map((range) => ({ ...range, group: 'top' }));
      return { charts };
    },
    async chart({ id, limit }) {
      const range = RANGES.find((known) => known.id === id)?.id ?? 'week';
      const count = Math.max(1, Math.min(limit || 50, MOST));
      const answer = await get<Answer>(
        `${API}/stats/sitewide/recordings?range=${range}&count=${count}`,
      );
      const tracks = (answer.payload?.recordings ?? [])
        .filter((recording) => recording.track_name)
        .map(toTrack);
      return { tracks };
    },
    // LB Radio's tag mode: songs MusicBrainz tags with the genre (or any of its spellings).
    async tagSongs({ tag, aliases, limit }) {
      const tags = [...new Set([tag, ...aliases].map((name) => name.trim().toLowerCase()))]
        .filter(Boolean)
        .slice(0, 4);
      if (tags.length === 0) return { tracks: [] };
      const count = clamp(limit || 25, 1, 50);
      const key = `${tags.join('|')}:${count}`;
      const kept = tagKept.get(key);
      if (kept && Date.now() - kept.at < 6 * HOUR) return { tracks: kept.tracks };
      const found = await get<{ recording_mbid: string }[]>(
        `${API}/lb-radio/tags?${query({ tag: tags, operator: 'or', pop_begin: 0, pop_end: 100, count: count * 2 })}`,
      );
      const tracks = distinct(await songsOf(found.map((recording) => recording.recording_mbid)));
      const shown = tracks.slice(0, count);
      if (tagKept.size > 40) tagKept.clear();
      tagKept.set(key, { at: Date.now(), tracks: shown });
      return { tracks: shown };
    },
  }),
);

registerProvider(
  defineProvider({
    kind: 'music.recommend',
    id: 'recommend',
    name: 'ListenBrainz',

    async similarArtists({ artist, limit }) {
      const id = await artistId(artist);
      if (!id) return { artists: [] };
      const similar = await get<{ artist_mbid: string; name: string }[]>(
        `${LABS}/similar-artists/json?${query({ artist_mbids: id, algorithm: SIMILAR_ARTISTS })}`,
      );
      const artists: Artist[] = similar
        .filter((found) => found.artist_mbid !== id && found.name?.trim())
        .slice(0, clamp(limit || 10, 1, 100))
        .map((found) => ({ id: found.artist_mbid, name: found.name.trim() }));
      return { artists };
    },

    // The artist's most listened-to songs: a large sample of LB Radio's most popular tenth (it
    // picks at random within it), by listens.
    async topTracks({ artist, limit }) {
      const id = await needArtist(artist);
      const count = clamp(limit || 10, 1, 50);
      const radio = await artistRadio(id, 0, 200, 90);
      const own = (radio[id] ?? [])
        .slice()
        .sort((a, b) => (b.total_listen_count ?? 0) - (a.total_listen_count ?? 0))
        .slice(0, count * 3);
      const tracks = distinct(await songsOf(own.map((recording) => recording.recording_mbid)));
      return { tracks: tracks.slice(0, count) };
    },

    // Fresh releases of the last `days` days (at most 30) whose artist is this one, newest first.
    async newReleases({ artist, days, limit }) {
      const id = await artistId(artist);
      if (!id) return { tracks: [] };
      const releases = (await freshReleases(clamp(days || 28, 1, 30)))
        .filter((release) => release.release_mbid && (release.artist_mbids ?? []).includes(id))
        .sort((a, b) => (b.release_date ?? '').localeCompare(a.release_date ?? ''));
      const count = clamp(limit || 10, 1, 50);
      const tracks: Track[] = [];
      for (const release of releases.slice(0, 3)) {
        if (tracks.length >= count) break;
        tracks.push(...(await releaseSongs(release)));
      }
      return { tracks: distinct(tracks).slice(0, count) };
    },

    // Songs like one song (Labs' similar recordings), else songs by the artist and similar ones.
    async radio({ artist, title, limit }) {
      const count = clamp(limit || 25, 1, 100);
      if (title?.trim()) {
        const recording = await recordingOf(artist, title);
        if (recording) {
          const tracks = distinct(await songRadio(recording)).slice(0, count);
          if (tracks.length > 0) return { tracks };
        }
      }
      const id = await needArtist(artist);
      const radio = await artistRadio(id, 10, Math.max(2, Math.ceil(count / 8)), 50);
      // The seed artist's songs go after the first round of others'.
      const groups = Object.entries(radio)
        .sort(([a], [b]) => Number(a === id) - Number(b === id))
        .map(([, recordings]) => recordings);
      const tracks = distinct(await songsOf(interleave(groups).slice(0, count * 2)));
      return { tracks: tracks.slice(0, count) };
    },
  }),
);
