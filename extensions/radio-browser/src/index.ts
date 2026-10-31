// Radio Browser: searches the community-run station directory at radio-browser.info and returns
// live streams. The directory runs on several mirrors; the list is cached in storage and the next
// mirror is tried when one fails.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

const DIRECTORY = 'https://all.api.radio-browser.info';
const FALLBACK_SERVERS = ['de1.api.radio-browser.info', 'de2.api.radio-browser.info'];
const SERVERS_KEY = 'servers';
const SERVERS_MAX_AGE_MS = 24 * 60 * 60 * 1000;
// The directory asks clients to identify themselves.
const HEADERS = { 'User-Agent': 'Raygun/1.0 (+https://github.com/uvxszi/raygun)' };

type Station = {
  stationuuid: string;
  name: string;
  url_resolved: string;
  favicon: string;
  tags: string;
  country: string;
  codec: string;
  bitrate: number;
  hls: number;
};

type ServerList = { fetchedAt: number; names: string[] };

const ORDERS: Record<string, string> = {
  popularity: 'clickcount',
  votes: 'votes',
  name: 'name',
};

const servers = async (): Promise<string[]> => {
  const cached = raygun.storage.get<ServerList>(SERVERS_KEY);
  if (cached && Date.now() - cached.fetchedAt < SERVERS_MAX_AGE_MS && cached.names.length > 0) {
    return cached.names;
  }
  try {
    const list = await fetchJson<{ name: string }[]>(`${DIRECTORY}/json/servers`, {
      headers: HEADERS,
    });
    const names = [...new Set(list.map((server) => server.name))].filter((name) =>
      name.endsWith('.api.radio-browser.info'),
    );
    if (names.length > 0) {
      raygun.storage.set(SERVERS_KEY, { fetchedAt: Date.now(), names });
      return names;
    }
  } catch (error) {
    raygun.log.warn('server list unavailable, using fallbacks:', String(error));
  }
  return FALLBACK_SERVERS;
};

/** Tries each mirror in turn until one answers. */
const query = async <T>(path: string): Promise<T> => {
  let lastError: unknown;
  for (const server of await servers()) {
    try {
      return await fetchJson<T>(`https://${server}${path}`, { headers: HEADERS });
    } catch (error) {
      lastError = error;
      raygun.log.warn(`${server} failed:`, String(error));
    }
  }
  throw lastError instanceof Error ? lastError : new Error('no radio-browser server answered');
};

const describe = (station: Station) => {
  const tags = station.tags
    .split(',')
    .map((tag) => tag.trim())
    .filter(Boolean)
    .slice(0, 3);
  return tags.length > 0 ? tags.join(', ') : null;
};

const toTrack = (station: Station): Track => ({
  id: station.stationuuid,
  title: station.name.trim(),
  artist: describe(station),
  album: [station.country, station.bitrate ? `${station.bitrate} kbps` : null]
    .filter(Boolean)
    .join(' · '),
  durationMs: null,
  // The webview only loads https images in release builds.
  artworkUrl: station.favicon.startsWith('https://') ? station.favicon : null,
  source: { kind: 'url', url: station.url_resolved, headers: {} },
});

const search = defineProvider({
  kind: 'music.metadata',
  id: 'stations',
  name: 'Radio Browser',

  async search({ query: text, limit }) {
    const name = text.trim();
    if (name === '') return { tracks: [] };
    const order = ORDERS[raygun.settings.get<string>('order') ?? 'popularity'] ?? 'clickcount';
    const country = (raygun.settings.get<string>('country') ?? '').trim().toUpperCase();
    const params = [
      `name=${encodeURIComponent(name)}`,
      `limit=${Math.min(limit * 2, 100)}`,
      'hidebroken=true',
      `order=${order}`,
      `reverse=${order === 'name' ? 'false' : 'true'}`,
      country ? `countrycode=${encodeURIComponent(country)}` : null,
    ].filter(Boolean);
    const stations = await query<Station[]>(`/json/stations/search?${params.join('&')}`);
    const playable = stations.filter(
      // The native player has no HLS module yet; skip HLS stations.
      (station) =>
        station.hls !== 1 && /^https?:\/\//.test(station.url_resolved) && station.name.trim(),
    );
    // The directory often lists one stream under several entries; keep the first of each.
    const seen = new Set<string>();
    const unique = playable.filter((station) => {
      const key = station.url_resolved.replace(/^https?:\/\//, '').replace(/\/+$/, '');
      if (seen.has(key)) return false;
      seen.add(key);
      return true;
    });
    return { tracks: unique.slice(0, limit).map(toTrack) };
  },
});

registerProvider(search);
