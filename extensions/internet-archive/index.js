// Internet Archive audio search (M11): finds audio items with the advanced-search API, reads each
// hit's file list in parallel and returns its MP3s as directly playable tracks.
'use strict';

const API = 'https://archive.org';

// Lucene syntax characters would turn a user's query into a malformed or different search.
const escapeQuery = (query) => query.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const firstString = (value) => (Array.isArray(value) ? value[0] : value) ?? null;

// `length` is seconds ("245.32") or a clock ("4:05", "1:02:03").
const parseLength = (length) => {
  if (typeof length !== 'string' || length === '') return null;
  const seconds = length
    .split(':')
    .reduce((total, part) => total * 60 + Number.parseFloat(part), 0);
  return Number.isFinite(seconds) && seconds > 0 ? Math.round(seconds * 1000) : null;
};

const trackNumber = (file) => Number.parseInt(String(file.track ?? '').split('/')[0], 10);

const isMp3 = (file) => typeof file.format === 'string' && file.format.includes('MP3');

const searchItems = async (query, rows) => {
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
  const response = await raygun.http.fetch(`${API}/advancedsearch.php?${params.join('&')}`);
  if (!response.ok) {
    throw new Error(`search failed (HTTP ${response.status})`);
  }
  return (await response.json()).response?.docs ?? [];
};

const itemTracks = async (doc, max) => {
  const id = doc.identifier;
  const response = await raygun.http.fetch(`${API}/metadata/${encodeURIComponent(id)}`);
  if (!response.ok) return [];
  const item = await response.json();
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
      url: `${API}/download/${encodeURIComponent(id)}/${file.name.split('/').map(encodeURIComponent).join('/')}`,
      headers: {},
    },
  }));
};

raygun.providers.register({
  kind: 'music.metadata',
  id: 'search',
  name: 'Internet Archive',

  async search({ query, limit = 30 }) {
    const cleaned = escapeQuery(String(query ?? ''));
    if (cleaned === '') return { tracks: [] };
    const rows = Math.max(1, Math.min(Number(raygun.settings.get('itemsPerSearch')) || 6, 12));
    const docs = await searchItems(cleaned, rows);
    const perItem = Math.max(1, Math.ceil(limit / Math.max(docs.length, 1)));
    // One huge or broken item shouldn't sink the whole search.
    const settled = await Promise.allSettled(docs.map((doc) => itemTracks(doc, perItem)));
    for (const result of settled) {
      if (result.status === 'rejected') {
        raygun.log.warn('item skipped:', result.reason);
      }
    }
    const tracks = settled.flatMap((result) => (result.status === 'fulfilled' ? result.value : []));
    return { tracks: tracks.slice(0, limit) };
  },
});
