// LRCLIB: synced lyrics from the open lrclib.net database (M15). An exact lookup by title, artist,
// album and length first; otherwise a search, keeping the result closest in length.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { LyricsQuery, LyricsResult } from '@raygun/sdk';

const API = 'https://lrclib.net/api';
// LRCLIB asks clients to identify themselves.
const HEADERS = { 'User-Agent': 'Raygun/1.0 (https://github.com/uvxszi/raygun)' };
// Search results further than this from the track's length are probably another version.
const MAX_LENGTH_GAP_S = 10;

type LrcRecord = {
  trackName: string;
  artistName: string;
  duration?: number | null;
  instrumental?: boolean;
  plainLyrics?: string | null;
  syncedLyrics?: string | null;
};

const encode = (params: Record<string, string | number | null | undefined>) =>
  Object.entries(params)
    .filter(([, value]) => value !== null && value !== undefined && value !== '')
    .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
    .join('&');

const TIMESTAMP = /\[(\d+):(\d{1,2})(?:[.:](\d{1,3}))?\]/g;

/** Parses LRC: `[mm:ss.xx]` stamps (several per line allowed); metadata tags are skipped. */
const parseLrc = (lrc: string) => {
  const lines: { timeMs: number; text: string }[] = [];
  for (const row of lrc.split(/\r?\n/)) {
    const stamps = [...row.matchAll(TIMESTAMP)];
    if (stamps.length === 0) continue;
    const text = row.replace(TIMESTAMP, '').trim();
    for (const [, minutes, seconds, fraction = '0'] of stamps) {
      const fractionMs = Number(fraction.padEnd(3, '0').slice(0, 3));
      lines.push({ timeMs: (Number(minutes) * 60 + Number(seconds)) * 1000 + fractionMs, text });
    }
  }
  return lines.sort((a, b) => a.timeMs - b.timeMs);
};

const toResult = (record: LrcRecord): LyricsResult | null => {
  if (record.instrumental) return null;
  const synced = record.syncedLyrics ? parseLrc(record.syncedLyrics) : [];
  const plain = record.plainLyrics?.trim() || null;
  return synced.length > 0 || plain ? { synced, plain } : null;
};

const getExact = async (query: LyricsQuery): Promise<LrcRecord | null> => {
  const params = encode({
    track_name: query.title,
    artist_name: query.artist,
    album_name: query.album,
    duration: query.durationMs ? Math.round(query.durationMs / 1000) : null,
  });
  const response = await raygun.http.fetch(`${API}/get?${params}`, { headers: HEADERS });
  if (response.status === 404) return null;
  if (!response.ok) throw new Error(`LRCLIB answered HTTP ${response.status}`);
  return response.json<LrcRecord>();
};

const searchClosest = async (query: LyricsQuery): Promise<LrcRecord | null> => {
  const params = encode({ track_name: query.title, artist_name: query.artist });
  const response = await raygun.http.fetch(`${API}/search?${params}`, { headers: HEADERS });
  if (!response.ok) throw new Error(`LRCLIB answered HTTP ${response.status}`);
  const records = await response.json<LrcRecord[]>();
  const wanted = query.durationMs ? query.durationMs / 1000 : null;
  const gap = (record: LrcRecord) =>
    wanted && record.duration ? Math.abs(record.duration - wanted) : MAX_LENGTH_GAP_S;
  const usable = records
    .filter((record) => gap(record) <= MAX_LENGTH_GAP_S)
    // Synced first, then the closest length.
    .sort((a, b) => Number(!a.syncedLyrics) - Number(!b.syncedLyrics) || gap(a) - gap(b));
  return usable[0] ?? null;
};

const lyrics = defineProvider({
  kind: 'lyrics',
  id: 'lyrics',
  name: 'LRCLIB',

  async lyrics(query) {
    if (!query.title.trim()) return null;
    const exact = await getExact(query);
    if (exact) {
      const result = toResult(exact);
      // An exact hit without synced lines may still have a synced twin in search.
      if (result?.synced?.length || exact.instrumental) return result;
    }
    const found = await searchClosest(query);
    return (found && toResult(found)) ?? (exact && toResult(exact));
  },
});

registerProvider(lyrics);
