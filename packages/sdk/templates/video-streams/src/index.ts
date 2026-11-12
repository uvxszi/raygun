// __NAME__: streams (`video.streams`) of the Internet Archive's public-domain feature films
// (https://archive.org/details/feature_films) for films other sources list. Raygun asks every
// stream provider for the movie or episode playing, by its IMDb and TMDB ids, title and year;
// this one finds the archive item marked with the film's IMDb id (else the items of its title and
// year) and offers the item's MP4 copies, at their heights, as streams.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global (requests reach only the manifest's `permissions.net`) and
// the helpers from '@raygun/sdk'. Try a call with `pnpm exec raygun-ext test films streams`.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { VideoStream, VideoStreamsQuery, VideoSubtitle } from '@raygun/sdk';

const API = 'https://archive.org';
/** Items whose files are read for one film. */
const ITEMS = 3;

type SearchAnswer = { response?: { docs?: { identifier: string }[] } };
type ItemFile = { name: string; format?: string; size?: string; height?: string };
type Item = { files?: ItemFile[] };

// Lucene's special characters would change what a search means.
const escape = (text: string) => text.replace(/[+\-!(){}[\]^"~*?:\\/&|]/g, ' ').trim();

const encodePath = (path: string) => path.split('/').map(encodeURIComponent).join('/');
const download = (identifier: string, file: string) =>
  `${API}/download/${encodeURIComponent(identifier)}/${encodePath(file)}`;

const FILMS = 'collection:(feature_films) AND mediatype:(movies)';

/** The film's archive items: the one marked with its IMDb id, else those of its title and year. */
const itemsOf = async ({ imdbId, title, year }: VideoStreamsQuery): Promise<string[]> => {
  const asks: string[] = [];
  if (imdbId) asks.push(`external-identifier:("urn:imdb:${escape(imdbId)}") AND ${FILMS}`);
  const words = escape(title ?? '');
  if (words) asks.push(`title:(${words})${year ? ` AND year:(${year})` : ''} AND ${FILMS}`);
  for (const q of asks) {
    const params = query({
      q,
      'fl[]': 'identifier',
      'sort[]': 'downloads desc',
      rows: ITEMS,
      output: 'json',
    });
    const answer = await fetchJson<SearchAnswer>(`${API}/advancedsearch.php?${params}`);
    const found = (answer.response?.docs ?? []).map((doc) => doc.identifier);
    if (found.length > 0) return found;
  }
  return [];
};

/** An item's MP4 copies, the tallest first, with the subtitles beside them. */
const streamsOf = async (identifier: string): Promise<VideoStream[]> => {
  const files = (await fetchJson<Item>(`${API}/metadata/${encodeURIComponent(identifier)}`)).files;
  const subtitles: VideoSubtitle[] = (files ?? [])
    .filter((file) => /\.(srt|vtt)$/i.test(file.name))
    .map((file) => ({ url: download(identifier, file.name), lang: 'en', label: 'English' }));
  return (files ?? [])
    .filter((file) => /\.(mp4|m4v)$/i.test(file.name) && /h\.264|mpeg4/i.test(file.format ?? ''))
    .map((file): VideoStream => {
      const height = Number.parseInt(file.height ?? '', 10);
      const size = Number.parseInt(file.size ?? '', 10);
      return {
        url: download(identifier, file.name),
        name: height > 0 ? `Internet Archive ${height}p` : 'Internet Archive',
        description: file.name,
        quality: height > 0 ? height : undefined,
        size: size > 0 ? size : undefined,
        subtitles,
      };
    })
    .sort((a, b) => Number(b.quality ?? 0) - Number(a.quality ?? 0));
};

registerProvider(
  defineProvider({
    kind: 'video.streams',
    id: 'films',
    name: '__NAME__',

    // Feature films only: an episode of a series isn't in the collection.
    streams: async (asked) => {
      if (asked.type !== 'movie') return { streams: [] };
      const items = await itemsOf(asked);
      const streams = (await Promise.all(items.map(streamsOf))).flat();
      return { streams: streams.slice(0, 100) };
    },
  }),
);
