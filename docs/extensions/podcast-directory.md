# Podcasts: `podcast.directory`

A podcast directory finds podcasts. It only names their feeds: Raygun reads a feed itself (RSS or
Atom, with its episodes, artwork and transcripts) when someone opens or follows the podcast, and
checks followed feeds for new episodes. Search → Music → Podcasts and the podcast list before a
search come from directories.

## Calls

| Call     | Receives            | Answers           | Required |
| -------- | ------------------- | ----------------- | -------- |
| `search` | `{ query, limit }`  | `PodcastResult[]` | yes      |
| `top`    | `{ limit }`         | `PodcastResult[]` | no       |

Both answer a plain list, not an object, and have 30 s; at most 100 results are kept.

```ts
import type { PodcastResult } from '@raygun/sdk';

const podcast: PodcastResult = {
  title: 'Houston We Have a Podcast',
  author: 'NASA',
  // RSS or Atom, http(s); results repeating a feed are dropped.
  feedUrl: 'https://www.nasa.gov/feeds/podcasts/houston-we-have-a-podcast',
  description: 'The official podcast of NASA Johnson Space Center, on human spaceflight.',
};

export { podcast };
```

`title` and `feedUrl` are required; `artworkUrl` (square) and `description` are optional. The
feed's host needs no permission: the extension never fetches it. A directory backed by a search
API fetches that API in `search` and `top`, and lists its host in `permissions.net`.

## Example: public-domain podcasts

NASA's podcasts, works of the US government, and LibriVox's community podcast, as a fixed list
that `search` filters by words.

```sh
pnpm raygun-ext init ../my-extensions/open-podcasts --kind podcast.directory --id dev.example.open-podcasts --name "Open podcasts"
```

<!-- example: podcast.directory dev.example.open-podcasts Open podcasts -->

```json
{
  "id": "dev.example.open-podcasts",
  "name": "Open podcasts",
  "version": "0.1.0",
  "apiVersion": 1,
  "runtime": "js",
  "entry": "dist/index.js",
  "description": "NASA's podcasts and LibriVox's community podcast.",
  "provides": ["podcast.directory"],
  "contentRating": "safe"
}
```

```ts
// Open podcasts: a podcast directory (`podcast.directory`) of public-domain podcasts: NASA's (works
// of the US government) and LibriVox's community podcast. A directory only names feeds: Raygun
// reads a feed itself when someone opens or follows the podcast, so this one needs no network
// access at all. A directory backed by a search API would fetch it in `search` and `top`.
//
// Extensions run in QuickJS, not a browser or Node: there is no `fetch`, `URL`, `setTimeout` or
// `require`. Use the `raygun` global and the helpers from '@raygun/sdk'. Try a call with
// `pnpm exec raygun-ext test podcasts search query=space`.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { PodcastResult } from '@raygun/sdk';

const PODCASTS: PodcastResult[] = [
  {
    title: 'Houston We Have a Podcast',
    author: 'NASA',
    feedUrl: 'https://www.nasa.gov/feeds/podcasts/houston-we-have-a-podcast',
    description: 'The official podcast of NASA Johnson Space Center, on human spaceflight.',
  },
  {
    title: "NASA's Curious Universe",
    author: 'NASA',
    feedUrl: 'https://www.nasa.gov/feeds/podcasts/curious-universe',
    description: 'Short adventures with NASA scientists, engineers and astronauts.',
  },
  {
    title: 'Small Steps, Giant Leaps',
    author: 'NASA',
    feedUrl: 'https://www.nasa.gov/feeds/podcasts/small-steps-giant-leaps',
    description: "Lessons learned from NASA's missions and projects.",
  },
  {
    title: 'LibriVox Community Podcast',
    author: 'LibriVox',
    feedUrl: 'https://librivox.org/category/librivox-community-podcast/feed/',
    description: 'News and conversations from the volunteers who record public-domain audiobooks.',
  },
];

const simple = (text: string) =>
  text
    .toLowerCase()
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

registerProvider(
  defineProvider({
    kind: 'podcast.directory',
    id: 'podcasts',
    name: 'Open podcasts',

    // Both calls answer a plain list.
    search: async ({ query, limit }) => {
      const words = simple(query).split(' ').filter(Boolean);
      return PODCASTS.filter((podcast) => {
        const about = simple(
          `${podcast.title} ${podcast.author ?? ''} ${podcast.description ?? ''}`,
        );
        return words.every((word) => about.includes(word));
      }).slice(0, limit);
    },

    // Shown before anything is typed.
    top: async ({ limit }) => PODCASTS.slice(0, limit),
  }),
);
```

## Try it

```sh
pnpm exec raygun-ext test podcasts top
pnpm exec raygun-ext test podcasts search query=space
```
