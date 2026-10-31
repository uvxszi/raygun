// Internet Archive audio search: finds audio items with the advanced-search API, reads each hit's
// file list in parallel and returns its MP3s as directly playable tracks.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

const API = 'https://archive.org';

type SearchDoc = { identifier: string; title?: string | string[]; creator?: string | string[] };
type SearchResponse = { response?: { docs?: SearchDoc[] } };

type ItemFile = {
  name: string;
  format?: string;
  title?: string;
  track?: string;
  length?: string;
  album?: string;
  creator?: string | string[];
  artist?: string | string[];
};
type ItemMetadata = {
  files?: ItemFile[];
  metadata?: { title?: string | string[]; creator?: string | string[] };
};

// Lucene syntax characters would turn a user's query into a malformed or different search.
const escapeQuery = (query: string) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const firstString = (value: string | string[] | undefined): string | null =>
  (Array.isArray(value) ? value[0] : value) ?? null;

// `length` is seconds ("245.32") or a clock ("4:05", "1:02:03").
const parseLength = (length: string | undefined): number | null => {
  if (!length) return null;
  const seconds = length
    .split(':')
    .reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

const trackNumber = (file: ItemFile) =>
  Number.parseInt(String(file.track ?? '').split('/')[0] ?? '', 10);

const isMp3 = (file: ItemFile) => file.format?.includes('MP3') ?? false;

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

const searchItems = async (query: string, rows: number): Promise<SearchDoc[]> => {
  const q = `(title:(${query}) OR creator:(${query})) AND mediatype:audio`;
  const params = [
    `q=${encodeURIComponent(q)}`,
    'fl[]=identifier',
    'fl[]=title',
    'fl[]=creator',
    `rows=${rows}`,
    'sort[]=downloads+desc',
    'output=json',
  ];
  const result = await fetchJson<SearchResponse>(`${API}/advancedsearch.php?${params.join('&')}`);
  return result.response?.docs ?? [];
};

const itemTracks = async (doc: SearchDoc, max: number): Promise<Track[]> => {
  const id = doc.identifier;
  const item = await fetchJson<ItemMetadata>(`${API}/metadata/${encodeURIComponent(id)}`);
  const files = (item.files ?? []).filter(isMp3);
  // Items often carry the same recording at several bitrates; prefer VBR, keep one per title.
  const preferred = files.some((file) => file.format === 'VBR MP3')
    ? files.filter((file) => file.format === 'VBR MP3')
    : files;
  preferred.sort((a, b) => (trackNumber(a) || 1e9) - (trackNumber(b) || 1e9));
  const album = firstString(item.metadata?.title) ?? firstString(doc.title);
  const creator = firstString(item.metadata?.creator) ?? firstString(doc.creator);
  return preferred.slice(0, max).map((file) => ({
    id: `${id}/${file.name}`,
    title: file.title || file.name.replace(/\.mp3$/i, '').replace(/_/g, ' '),
    artist: firstString(file.creator) ?? firstString(file.artist) ?? creator,
    album: file.album || album,
    durationMs: parseLength(file.length),
    artworkUrl: `${API}/services/img/${encodeURIComponent(id)}`,
    source: {
      kind: 'url',
      url: `${API}/download/${encodeURIComponent(id)}/${encodePath(file.name)}`,
      headers: {},
    },
  }));
};

const search = defineProvider({
  kind: 'music.metadata',
  id: 'search',
  name: 'Internet Archive',

  async search({ query, limit }) {
    const cleaned = escapeQuery(query);
    if (cleaned === '') return { tracks: [] };
    const rows = Math.max(1, Math.min(raygun.settings.get<number>('itemsPerSearch') ?? 6, 12));
    const docs = await searchItems(cleaned, rows);
    const perItem = Math.max(1, Math.ceil(limit / Math.max(docs.length, 1)));
    // One huge or broken item shouldn't sink the whole search.
    const settled = await Promise.allSettled(docs.map((doc) => itemTracks(doc, perItem)));
    for (const result of settled) {
      if (result.status === 'rejected') {
        raygun.log.warn('item skipped:', String(result.reason));
      }
    }
    const tracks = settled.flatMap((result) => (result.status === 'fulfilled' ? result.value : []));
    return { tracks: tracks.slice(0, limit) };
  },
});

registerProvider(search);
