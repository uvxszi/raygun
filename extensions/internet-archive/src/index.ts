// Internet Archive audio search: finds audio items with the advanced-search API, reads each hit's
// file list in parallel and returns its MP3s as directly playable tracks. Only items that carry a
// public-domain mark or a Creative Commons licence are searched or played. Films are in `films.ts`.
import { defineProvider, fetchJson, registerProvider } from '@raygun/sdk';
import type { Track } from '@raygun/sdk';

import { films } from './films';

const API = 'https://archive.org';
/** A public-domain mark or dedication, or a Creative Commons licence. */
const OPEN_LICENCE = /creativecommons\.org|publicdomain/i;
/** The search terms for the same rule. */
const OPEN = 'licenseurl:(*publicdomain* OR *creativecommons*)';

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
  metadata?: { title?: string | string[]; creator?: string | string[]; licenseurl?: string };
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

/** What a file format sounds like in kbps: lossless counts as CD audio, IA's VBR MP3s average ~200. */
const formatKbps = (format: string | undefined): number | null => {
  if (!format) return null;
  if (/flac|lossless/i.test(format)) return 1411;
  const stated = /(\d+)\s*kbps/i.exec(format);
  if (stated) return Number(stated[1]);
  return format === 'VBR MP3' ? 200 : null;
};

const isPlayable = (file: ItemFile) =>
  (file.format?.includes('MP3') ?? false) || /flac/i.test(file.format ?? '');

/**
 * The format to play from an item that has the same recordings at several bitrates (M46.18):
 * VBR MP3 when nothing is asked, the nearest to `targetKbps`, or the highest for `null`.
 */
const pickFormat = (files: ItemFile[], targetKbps: number | null | undefined): string | null => {
  const formats = [...new Set(files.map((file) => file.format ?? ''))].filter(
    (format) => formatKbps(format) !== null,
  );
  if (formats.length === 0) return null;
  if (targetKbps === undefined) {
    return formats.includes('VBR MP3')
      ? 'VBR MP3'
      : (formats.find((format) => format.includes('MP3')) ?? formats[0] ?? null);
  }
  const distance = (format: string) => {
    const kbps = formatKbps(format) ?? 160;
    if (targetKbps === null) return -kbps;
    return Math.max(kbps, targetKbps) / Math.max(1, Math.min(kbps, targetKbps));
  };
  return [...formats].sort((a, b) => distance(a) - distance(b))[0] ?? null;
};

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');

const searchDocs = async (q: string, rows: number): Promise<SearchDoc[]> => {
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

const searchItems = (query: string, rows: number) =>
  searchDocs(`(title:(${query}) OR creator:(${query})) AND mediatype:audio AND ${OPEN}`, rows);

/** "Hello Dolly (Live) [Remastered]" -> "Hello Dolly": archive titles rarely carry the extras. */
const bareTitle = (title: string) => title.replace(/\([^)]*\)|\[[^\]]*\]/g, ' ').trim() || title;

/** "Ella Fitzgerald & Louis Armstrong" -> "Ella Fitzgerald": credits are joined many ways. */
const mainArtist = (artist: string) =>
  artist.split(/\s*(?:&|,|;|\/|\bfeat\.?|\bft\.?|\band\b|\bwith\b|\bx\b)\s*/i)[0]?.trim() || artist;

const words = (text: string) => escapeQuery(text).toLowerCase().split(/\s+/).filter(Boolean);

type ItemTrack = { track: Track; format: string | null };

const itemTracks = async (
  doc: SearchDoc,
  max: number,
  targetKbps?: number | null,
): Promise<ItemTrack[]> => {
  const id = doc.identifier;
  const item = await fetchJson<ItemMetadata>(`${API}/metadata/${encodeURIComponent(id)}`);
  // The search index can lag behind an item's own record; the record decides.
  if (!OPEN_LICENCE.test(item.metadata?.licenseurl ?? '')) return [];
  const files = (item.files ?? []).filter(isPlayable);
  // Items often carry the same recording at several bitrates; one format, so one file per title.
  const format = pickFormat(files, targetKbps);
  const preferred = format === null ? files : files.filter((file) => file.format === format);
  preferred.sort((a, b) => (trackNumber(a) || 1e9) - (trackNumber(b) || 1e9));
  const album = firstString(item.metadata?.title) ?? firstString(doc.title);
  const creator = firstString(item.metadata?.creator) ?? firstString(doc.creator);
  return preferred.slice(0, max).map((file) => ({
    format: file.format ?? null,
    track: {
      id: `${id}/${file.name}`,
      title: file.title || file.name.replace(/\.(mp3|flac)$/i, '').replace(/_/g, ' '),
      artist: firstString(file.creator) ?? firstString(file.artist) ?? creator,
      album: file.album || album,
      durationMs: parseLength(file.length),
      artworkUrl: `${API}/services/img/${encodeURIComponent(id)}`,
      source: {
        kind: 'url',
        url: `${API}/download/${encodeURIComponent(id)}/${encodePath(file.name)}`,
        headers: {},
      },
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
    const tracks: Track[] = settled
      .flatMap((result) => (result.status === 'fulfilled' ? result.value : []))
      .map((item) => item.track);
    return { tracks: tracks.slice(0, limit) };
  },
});

// Streams for tracks found elsewhere (M14): items by the artist that match the title, then the
// files inside them whose title has every word of the track's title. Raygun ranks the results.
const streams = defineProvider({
  kind: 'music.stream',
  id: 'stream',
  name: 'Internet Archive',

  async candidates({ title, artist, limit, targetKbps }) {
    const wantedTitle = escapeQuery(bareTitle(title));
    if (wantedTitle === '') return { candidates: [] };
    const wantedArtist = artist ? escapeQuery(mainArtist(artist)) : '';
    // Raygun ranks what comes back, so a looser title-only search is a safe last resort.
    const queries = wantedArtist
      ? [
          `title:(${wantedTitle}) AND creator:(${wantedArtist})`,
          `creator:(${wantedArtist}) AND (${wantedTitle})`,
          `title:(${wantedTitle})`,
        ]
      : [`title:(${wantedTitle})`];
    const lists = await Promise.allSettled(
      queries.map((query) => searchDocs(`${query} AND mediatype:audio AND ${OPEN}`, 4)),
    );
    const failures = lists.flatMap((list) => (list.status === 'rejected' ? [list.reason] : []));
    // If every search failed, say why instead of claiming there's nothing.
    if (failures.length === lists.length) throw failures[0];
    for (const failure of failures) raygun.log.warn('stream search failed:', String(failure));
    const docs = [
      ...new Map(
        lists
          .flatMap((list) => (list.status === 'fulfilled' ? list.value : []))
          .map((doc) => [doc.identifier, doc]),
      ).values(),
    ].slice(0, 6);

    const wanted = words(bareTitle(title));
    const settled = await Promise.allSettled(docs.map((doc) => itemTracks(doc, 500, targetKbps)));
    for (const result of settled) {
      if (result.status === 'rejected') raygun.log.warn('item skipped:', String(result.reason));
    }
    const matching = settled
      .flatMap((result) => (result.status === 'fulfilled' ? result.value : []))
      .filter(({ track }) => {
        const offered = words(track.title);
        return wanted.every((word) => offered.includes(word));
      });
    raygun.log.info(
      `stream candidates for "${title}": ${docs.length} items, ${matching.length} matching files`,
    );
    return {
      candidates: matching.slice(0, limit).map(({ track, format }) => ({
        id: track.id ?? track.title,
        title: track.title,
        artist: track.artist ?? null,
        durationMs: track.durationMs ?? null,
        quality: format ?? 'MP3',
        bitrateKbps: formatKbps(format ?? undefined),
        source: track.source ?? null,
      })),
    };
  },
});

registerProvider(search);
registerProvider(streams);
registerProvider(films);
