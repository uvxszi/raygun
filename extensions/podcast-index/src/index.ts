// Podcast Index (https://podcastindex.org): an open, independent index of public podcast feeds,
// as a podcast directory (`podcast.directory`). A directory only names feeds: Raygun reads each
// podcast's own feed when it's opened or followed. Searches look in titles, authors and owners;
// the list before a search is what's trending in the language the settings name.
//
// The API needs a free key and secret per person (https://api.podcastindex.org): its terms keep
// credentials out of open-source code, so people enter their own in this source's settings. Each
// request is signed as the API documents: `Authorization` is the SHA-1 of key, secret and the
// Unix time in `X-Auth-Date`. Explicit feeds are left out unless NSFW content is on, and so are
// feeds the index found dead.
import { defineProvider, query, registerProvider } from '@raygun/sdk';
import type { PodcastResult } from '@raygun/sdk';

import { sha1Hex } from './sha1';

const API = 'https://api.podcastindex.org/api/1.0';
/** The most a search or list is asked for at once. */
const MOST = 100;

const LANGUAGES: Record<string, string> = {
  Arabic: 'ar',
  Chinese: 'zh',
  Dutch: 'nl',
  English: 'en',
  French: 'fr',
  German: 'de',
  Hindi: 'hi',
  Italian: 'it',
  Japanese: 'ja',
  Korean: 'ko',
  Polish: 'pl',
  Portuguese: 'pt',
  Russian: 'ru',
  Spanish: 'es',
  Swedish: 'sv',
  Turkish: 'tr',
};

type Feed = {
  title?: string | null;
  url?: string | null;
  author?: string | null;
  ownerName?: string | null;
  description?: string | null;
  image?: string | null;
  artwork?: string | null;
  explicit?: boolean | null;
  /** 1 when the index found the feed gone. */
  dead?: number | null;
};

type Answer = { status?: string | boolean; feeds?: Feed[]; description?: string };

const plain = (text: string | null | undefined) =>
  (text ?? '')
    .replace(/<[^>]+>/g, ' ')
    .replace(/&nbsp;/g, ' ')
    .replace(/&amp;/g, '&')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;|&rsquo;/g, "'")
    .replace(/\s+/g, ' ')
    .trim();

const isWeb = (address: string | null | undefined): address is string =>
  typeof address === 'string' && /^https?:\/\//i.test(address);

export const toResult = (feed: Feed): PodcastResult | null => {
  const title = plain(feed.title);
  if (!title || !isWeb(feed.url) || feed.dead === 1) return null;
  if (feed.explicit === true && !raygun.content.showNsfw()) return null;
  const description = plain(feed.description);
  return {
    title,
    author: plain(feed.author) || plain(feed.ownerName) || null,
    feedUrl: feed.url,
    artworkUrl: [feed.artwork, feed.image].find(isWeb) ?? null,
    description: description ? description.slice(0, 600) : null,
  };
};

/** The person's credentials, or a plain sentence saying where to put them. */
const credentials = () => {
  const key = raygun.settings.get<string>('apiKey')?.trim();
  const secret = raygun.settings.get<string>('apiSecret')?.trim();
  if (!key || !secret) {
    throw new Error(
      "Podcast Index needs your free API key and secret: get them at api.podcastindex.org and add them in this source's settings",
    );
  }
  return { key, secret };
};

const ask = async (path: string, params: Record<string, string | number | boolean>) => {
  const { key, secret } = credentials();
  const time = String(Math.floor(Date.now() / 1000));
  const address = `${API}/${path}?${query(params)}`;
  const response = await raygun.http.fetch(address, {
    headers: {
      Accept: 'application/json',
      'X-Auth-Key': key,
      'X-Auth-Date': time,
      Authorization: sha1Hex(`${key}${secret}${time}`),
    },
  });
  if (response.status === 401) {
    throw new Error(
      "Podcast Index didn't accept the API key and secret: check them in this source's settings",
    );
  }
  if (!response.ok) throw new Error(`HTTP ${response.status} from ${address}`);
  const answer = await response.json<Answer>();
  const seen = new Set<string>();
  return (answer.feeds ?? []).map(toResult).filter((podcast): podcast is PodcastResult => {
    if (!podcast || seen.has(podcast.feedUrl)) return false;
    seen.add(podcast.feedUrl);
    return true;
  });
};

const count = (limit: number) => Math.max(1, Math.min(limit || 20, MOST));

/** `clean` keeps explicit feeds out on the index's side too; it takes no value. */
const clean = (): Record<string, string> => (raygun.content.showNsfw() ? {} : { clean: '' });

registerProvider(
  defineProvider({
    kind: 'podcast.directory',
    id: 'podcasts',
    name: 'Podcast Index',

    async search({ query: text, limit }) {
      const term = text.trim();
      if (!term) return [];
      const found = await ask('search/byterm', { q: term, max: count(limit), ...clean() });
      return found.slice(0, count(limit));
    },

    async top({ limit }) {
      const language = LANGUAGES[raygun.settings.get<string>('language') ?? 'English'];
      const params: Record<string, string | number> = { max: count(limit) };
      if (language) params.lang = language;
      const trending = await ask('podcasts/trending', params);
      return trending.slice(0, count(limit));
    },
  }),
);
