// ListenBrainz charts: the songs most listened to by ListenBrainz's users, from the open
// listening-history project of the MetaBrainz Foundation (its statistics are CC0). Songs carry no
// stream; Raygun plays them through the music sources the user has. Cover art comes from the
// Cover Art Archive, loaded by the app.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Chart, Track } from '@raygun/sdk';

const API = 'https://api.listenbrainz.org/1/stats/sitewide/recordings';
const HEADERS = {
  'User-Agent': 'Raygun/1.0 ( https://github.com/uvxszi/raygun )',
  Accept: 'application/json',
};
/** ListenBrainz gives at most this many songs a request. */
const MOST = 100;

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
const artwork = (recording: Recording) =>
  recording.caa_id && recording.caa_release_mbid
    ? `https://coverartarchive.org/release/${recording.caa_release_mbid}/${recording.caa_id}-250.jpg`
    : null;

export const toTrack = (recording: Recording, index: number): Track => ({
  id: recording.recording_mbid ?? `${index}`,
  title: recording.track_name,
  artist: recording.artist_name ?? null,
  album: recording.release_name ?? null,
  artworkUrl: artwork(recording),
});

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
      const answer = await fetchJson<Answer>(`${API}?range=${range}&count=${count}`, {
        headers: HEADERS,
      });
      const tracks = (answer.payload?.recordings ?? [])
        .filter((recording) => recording.track_name)
        .map(toTrack);
      return { tracks };
    },
  }),
);
