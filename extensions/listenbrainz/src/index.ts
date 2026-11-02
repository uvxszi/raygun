// ListenBrainz: the songs most listened to by everyone on the open ListenBrainz service this week,
// month or year (M46.9). No key needed; artwork comes from the Cover Art Archive. Songs carry no
// stream; Raygun finds one when they play.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

const API = 'https://api.listenbrainz.org/1';
// ListenBrainz asks clients to identify themselves.
const HEADERS = { 'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )' };
const RANGES = {
  week: { name: 'Most listened this week', description: 'Everyone on ListenBrainz, last week' },
  month: { name: 'Most listened this month', description: 'Everyone on ListenBrainz, last month' },
  year: { name: 'Most listened this year', description: 'Everyone on ListenBrainz, this year' },
} as const;
type Range = keyof typeof RANGES;

type Recording = {
  track_name: string;
  artist_name?: string;
  release_name?: string;
  recording_mbid?: string | null;
  caa_id?: number | null;
  caa_release_mbid?: string | null;
};
type Stats = { payload?: { recordings?: Recording[] } };

const artwork = (recording: Recording) =>
  recording.caa_id && recording.caa_release_mbid
    ? `https://coverartarchive.org/release/${recording.caa_release_mbid}/${recording.caa_id}-500.jpg`
    : null;

registerProvider(
  defineProvider({
    kind: 'music.charts',
    id: 'charts',
    name: 'ListenBrainz',
    async charts() {
      return {
        charts: (Object.keys(RANGES) as Range[]).map((range) => ({ id: range, ...RANGES[range] })),
      };
    },
    async chart({ id, limit }) {
      if (!(id in RANGES)) throw new Error(`Unknown chart ${id}`);
      const count = Math.min(Math.max(limit, 1), 100);
      const stats = await fetchJson<Stats>(
        `${API}/stats/sitewide/recordings?count=${count}&range=${id}`,
        { headers: HEADERS },
      );
      const tracks: Track[] = (stats.payload?.recordings ?? []).map((recording) => ({
        id: recording.recording_mbid ?? undefined,
        title: recording.track_name,
        artist: recording.artist_name ?? null,
        album: recording.release_name ?? null,
        artworkUrl: artwork(recording),
      }));
      return { tracks };
    },
  }),
);
