// Podcast directory (M37): search and top shows from Apple's public iTunes Search API. The app
// subscribes to the RSS feed each result points to and reads it itself.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { PodcastResult } from '@raygun/sdk';

const SEARCH = 'https://itunes.apple.com/search';
const LOOKUP = 'https://itunes.apple.com/lookup';
const TOP = 'https://rss.marketingtools.apple.com/api/v2';
const MAX_LIMIT = 50;

type ItunesPodcast = {
  collectionName?: string;
  artistName?: string;
  feedUrl?: string;
  artworkUrl600?: string;
  artworkUrl100?: string;
  primaryGenreName?: string;
};
type TopShow = { id: string; name: string; artistName?: string; artworkUrl100?: string };

const query = (params: Record<string, string | number>) =>
  Object.entries(params)
    .map(([key, value]) => `${key}=${encodeURIComponent(String(value))}`)
    .join('&');

const getJson = async <T>(url: string): Promise<T> => {
  const response = await raygun.http.fetch(url);
  if (!response.ok) throw new Error(`The podcast directory answered ${response.status}`);
  return response.json<T>();
};

const limitOf = (limit: number) => Math.min(Math.max(Math.floor(limit) || 20, 1), MAX_LIMIT);

const toResult = (podcast: ItunesPodcast): PodcastResult | null =>
  podcast.collectionName && podcast.feedUrl
    ? {
        title: podcast.collectionName,
        author: podcast.artistName ?? null,
        feedUrl: podcast.feedUrl,
        artworkUrl: podcast.artworkUrl600 ?? podcast.artworkUrl100 ?? null,
        description: podcast.primaryGenreName ?? null,
      }
    : null;

const country = () =>
  (raygun.settings.get<string>('country') ?? 'us').trim().toLowerCase().slice(0, 2) || 'us';

registerProvider(
  defineProvider({
    kind: 'podcast.directory',
    id: 'itunes',
    name: 'Podcast directory',
    async search({ query: term, limit }) {
      const result = await getJson<{ results: ItunesPodcast[] }>(
        `${SEARCH}?${query({ media: 'podcast', entity: 'podcast', limit: limitOf(limit), term })}`,
      );
      return result.results.map(toResult).filter((item): item is PodcastResult => item !== null);
    },
    async top({ limit }) {
      // The charts list ids; a lookup gives their feeds.
      const chart = await getJson<{ feed: { results: TopShow[] } }>(
        `${TOP}/${country()}/podcasts/top/${limitOf(limit)}/podcasts.json`,
      );
      const ids = chart.feed.results.map((show) => show.id);
      if (ids.length === 0) return [];
      const found = await getJson<{ results: (ItunesPodcast & { collectionId?: number })[] }>(
        `${LOOKUP}?${query({ id: ids.join(','), entity: 'podcast' })}`,
      );
      const byId = new Map(found.results.map((podcast) => [String(podcast.collectionId), podcast]));
      return ids
        .map((id) => byId.get(id))
        .filter((podcast): podcast is ItunesPodcast => podcast !== undefined)
        .map(toResult)
        .filter((item): item is PodcastResult => item !== null);
    },
  }),
);
