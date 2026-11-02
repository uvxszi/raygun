// ListenBrainz: the songs most listened to by everyone on the open ListenBrainz service this week,
// month or year (M46.9), the singles people are playing from the last two weeks' releases, and
// songs for any genre or mood from its tag radio (M46.19). No key needed; artwork comes from the
// Cover Art Archive. Songs carry no stream; Raygun finds one when they play.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Chart, Track } from '@raygun/sdk';

const API = 'https://api.listenbrainz.org/1';
// ListenBrainz asks clients to identify themselves.
const HEADERS = { 'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )' };
const RANGES = {
  week: { name: 'Most listened this week', description: 'Everyone on ListenBrainz, last week' },
  month: { name: 'Most listened this month', description: 'Everyone on ListenBrainz, last month' },
  year: { name: 'Most listened this year', description: 'Everyone on ListenBrainz, this year' },
} as const;
type Range = keyof typeof RANGES;
const FRESH = 'fresh';
// Metadata lookups take a list of ids; this many keep the address short.
const LOOKUP_BATCH = 25;

type Recording = {
  track_name: string;
  artist_name?: string;
  release_name?: string;
  recording_mbid?: string | null;
  caa_id?: number | null;
  caa_release_mbid?: string | null;
};
type Stats = { payload?: { recordings?: Recording[] } };
type FreshRelease = {
  artist_credit_name: string;
  release_name: string;
  release_group_primary_type?: string | null;
  listen_count?: number;
  caa_id?: number | null;
  caa_release_mbid?: string | null;
  release_mbid?: string;
};
type Fresh = { payload?: { releases?: FreshRelease[] } };
type TagHit = { recording_mbid: string; percent?: number };
type Metadata = Record<
  string,
  {
    artist?: { name?: string };
    recording?: { name?: string; length?: number | null };
    release?: { name?: string; caa_id?: number | null; caa_release_mbid?: string | null };
  }
>;

const cover = (release: string | null | undefined, id: number | null | undefined) =>
  release && id ? `https://coverartarchive.org/release/${release}/${id}-500.jpg` : null;

const lookup = async (mbids: string[]): Promise<Track[]> => {
  const tracks: Track[] = [];
  for (let start = 0; start < mbids.length; start += LOOKUP_BATCH) {
    const batch = mbids.slice(start, start + LOOKUP_BATCH);
    const found = await fetchJson<Metadata>(
      `${API}/metadata/recording/?recording_mbids=${batch.join(',')}&inc=artist%20release`,
      { headers: HEADERS },
    );
    for (const mbid of batch) {
      const entry = found[mbid];
      const title = entry?.recording?.name;
      if (!entry || !title) continue;
      tracks.push({
        id: mbid,
        title,
        artist: entry.artist?.name ?? null,
        album: entry.release?.name ?? null,
        durationMs: entry.recording?.length ?? null,
        artworkUrl: cover(entry.release?.caa_release_mbid, entry.release?.caa_id),
      });
    }
  }
  return tracks;
};

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'ListenBrainz',
    async charts() {
      const charts: Chart[] = (Object.keys(RANGES) as Range[]).map((range) => ({
        id: range,
        ...RANGES[range],
        group: 'top',
      }));
      charts.push({
        id: FRESH,
        name: 'Fresh singles',
        description: 'Singles from the last two weeks that ListenBrainz listeners play most',
        group: 'new',
      });
      return { charts };
    },
    async chart({ id, limit }) {
      const count = Math.min(Math.max(limit, 1), 100);
      if (id === FRESH) {
        const fresh = await fetchJson<Fresh>(
          `${API}/explore/fresh-releases/?days=14&sort=release_date&past=true&future=false`,
          { headers: HEADERS },
        );
        const singles = (fresh.payload?.releases ?? [])
          .filter((release) => release.release_group_primary_type === 'Single')
          .sort((a, b) => (b.listen_count ?? 0) - (a.listen_count ?? 0))
          .slice(0, count);
        const tracks: Track[] = singles.map((release) => ({
          id: release.release_mbid,
          title: release.release_name,
          artist: release.artist_credit_name,
          album: release.release_name,
          artworkUrl: cover(release.caa_release_mbid, release.caa_id),
        }));
        return { tracks };
      }
      if (!(id in RANGES)) throw new Error(`Unknown chart ${id}`);
      const stats = await fetchJson<Stats>(
        `${API}/stats/sitewide/recordings?count=${count}&range=${id}`,
        { headers: HEADERS },
      );
      const tracks: Track[] = (stats.payload?.recordings ?? []).map((recording) => ({
        id: recording.recording_mbid ?? undefined,
        title: recording.track_name,
        artist: recording.artist_name ?? null,
        album: recording.release_name ?? null,
        artworkUrl: cover(recording.caa_release_mbid, recording.caa_id),
      }));
      return { tracks };
    },
    // Tag radio: recordings listeners tagged with the genre or mood (or one of its other names),
    // the better known ones first.
    async tagSongs({ tag, aliases, limit }) {
      const names = [tag, ...aliases]
        .map((name) => name.toLowerCase())
        .filter((name, index, all) => all.indexOf(name) === index)
        .slice(0, 5);
      const params = [
        ...names.map((name) => `tag=${encodeURIComponent(name)}`),
        'operator=or',
        'pop_begin=30',
        'pop_end=100',
        `count=${Math.min(Math.max(limit, 1), 100)}`,
      ];
      const hits = await fetchJson<TagHit[]>(`${API}/lb-radio/tags?${params.join('&')}`, {
        headers: HEADERS,
      });
      const mbids = [...new Set(hits.map((hit) => hit.recording_mbid))].slice(0, limit);
      return { tracks: await lookup(mbids), name: `${tag} on ListenBrainz` };
    },
  }),
);
