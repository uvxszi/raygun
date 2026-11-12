# Streams for films and series: `video.streams`

A stream source finds streams for films and series that other sources list. The titles of Stremio
addons' catalogues (Cinemeta's, say) are known by their IMDb and TMDB ids; when one plays, Raygun
asks every stream source for the movie or episode by those ids, and each source's streams are a
source of their own in the play session (the player's Sources sheet and Quality menu) and on the
title's page, beside the addons' streams. Nuvio's providers run as stream sources
([other formats](other-formats.md)).

## Calls

| Call      | Receives            | Answers                      | Required |
| --------- | ------------------- | ---------------------------- | -------- |
| `streams` | `VideoStreamsQuery` | `{ streams: VideoStream[] }` | yes      |

### What it's asked

`streams` receives a movie or an episode:

- `type` is `movie` or `series`.
- `tmdbId` is TMDB's id (digits) when the title's details give it, else null.
- `imdbId` is IMDb's (`tt…`), else null. An anime entry's episode comes with the IMDb episode its
  addon maps it to; one without that mapping, and live channels, aren't asked about.
- `season` and `episode` count an episode as TMDB and IMDb do; both are null for a movie.
- `title` and `year` are for sources that look titles up by them.

A source answers no streams for what it doesn't have; that's no failure. Raygun asks stream
sources a few at a time, each within 20 seconds, the ones that answered lately first; one that
keeps failing sits out a while. In a play session each source's streams join as it answers, so a
slow one never holds up the others.

### Streams

```ts
import type { VideoStream } from '@raygun/sdk';

const stream: VideoStream = {
  url: 'https://archive.org/download/night_of_the_living_dead/night_of_the_living_dead_512kb.mp4',
  // The picker's first line, and the line under it.
  name: 'Internet Archive 480p',
  description: 'night_of_the_living_dead_512kb.mp4',
  // The height in pixels, or as written ("1080p", "4K"): the Quality menu orders streams by it.
  quality: 480,
  // Bytes, or as written ("1.4 GB").
  size: 163_577_856,
  subtitles: [{ url: 'https://example.org/en.vtt', lang: 'en', label: 'English' }],
};

export { stream };
```

- `url` is `http(s)`; a torrent is its magnet link (or a `.torrent` address) with
  `format: 'torrent'`, played by Raygun's torrent engine while torrents are on.
- `format` is `hls` or `dash` for streaming manifests; leave it out for a plain file.
- `headers` (a Referer, a User-Agent) are sent with every request the player makes.
- `subtitles` are WebVTT or SubRip files, beside the ones subtitle addons find.
- A source gives at most 100 streams for a title.

## Example: Internet Archive films

Streams of the Internet Archive's public-domain feature films: the archive item marked with the
film's IMDb id, else the items of its title and year, and their MP4 copies at their heights.

```sh
pnpm raygun-ext init ../my-extensions/archive-film-streams --kind video.streams --id dev.example.archive-film-streams --name "Archive film streams"
```

<!-- example: video.streams dev.example.archive-film-streams Archive film streams -->

```json
{
  "id": "dev.example.archive-film-streams",
  "name": "Archive film streams",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "Streams of public-domain feature films on the Internet Archive, for films other sources list.",
  "website": "https://archive.org/details/feature_films",
  "provides": ["video.streams"],
  "permissions": { "net": ["archive.org"] },
  "contentRating": "safe"
}
```

```ts
// Archive film streams: streams (`video.streams`) of the Internet Archive's public-domain feature films
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
    name: 'Archive film streams',

    // Feature films only: an episode of a series isn't in the collection.
    streams: async (asked) => {
      if (asked.type !== 'movie') return { streams: [] };
      const items = await itemsOf(asked);
      const streams = (await Promise.all(items.map(streamsOf))).flat();
      return { streams: streams.slice(0, 100) };
    },
  }),
);
```

## Try it

Without arguments, `test` asks for Night of the Living Dead (1968); others are named by their ids,
title and year:

```sh
pnpm exec raygun-ext test films streams
pnpm exec raygun-ext test films streams imdbId=tt0013442 tmdbId=null title=Nosferatu year=1922
```
