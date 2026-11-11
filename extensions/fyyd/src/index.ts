// fyyd: a podcast directory (`podcast.directory`) over fyyd's public API (https://fyyd.de), an
// independent search engine that crawls public podcast feeds. A directory only names feeds: Raygun
// reads each podcast's own feed when it's opened or followed. Searches look in titles, authors and
// descriptions; the list before a search is fyyd's "hot" podcasts in the language the settings
// name: the most active of the last eight days, by listeners and new episodes. Feeds fyyd last
// found broken are left out. The API's reading calls need no account.
import { defineProvider, fetchJson, query, registerProvider } from '@raygun/sdk';
import type { PodcastResult } from '@raygun/sdk';

const API = 'https://api.fyyd.de/0.2';
/** The most a search or list is asked for at once. */
const MOST = 50;

/** fyyd's codes for the languages it ranks hot podcasts in. */
const LANGUAGES: Record<string, string> = {
  Arabic: 'ar',
  Catalan: 'ca',
  Dutch: 'nl',
  English: 'en',
  French: 'fr',
  German: 'de',
  Italian: 'it',
  Persian: 'fa',
  Polish: 'pl',
  Portuguese: 'pt',
  Russian: 'ru',
  Spanish: 'es',
  Swedish: 'sv',
};

type Podcast = {
  title?: string | null;
  author?: string | null;
  xmlURL?: string | null;
  imgURL?: string | null;
  layoutImageURL?: string | null;
  description?: string | null;
  subtitle?: string | null;
  /** The HTTP status fyyd got from the feed when it last looked. */
  status?: number | string | null;
};

type Answer = { status?: number; msg?: string; data?: Podcast[] | null };

const HEADERS = { Accept: 'application/json' };

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

/** The podcast's own artwork when it's served over https, else fyyd's copy. */
const artwork = (podcast: Podcast) => {
  if (isWeb(podcast.imgURL) && podcast.imgURL.startsWith('https://')) return podcast.imgURL;
  return isWeb(podcast.layoutImageURL) ? podcast.layoutImageURL : null;
};

export const toResult = (podcast: Podcast): PodcastResult | null => {
  const title = plain(podcast.title);
  if (!title || !isWeb(podcast.xmlURL)) return null;
  // A feed fyyd couldn't read last time is likely gone.
  const status = Number(podcast.status ?? 200);
  if (Number.isFinite(status) && status >= 400) return null;
  const description = plain(podcast.description) || plain(podcast.subtitle);
  return {
    title,
    author: plain(podcast.author) || null,
    feedUrl: podcast.xmlURL,
    artworkUrl: artwork(podcast),
    description: description ? description.slice(0, 600) : null,
  };
};

const ask = async (path: string, params: Record<string, string | number>) => {
  const answer = await fetchJson<Answer>(`${API}/${path}?${query(params)}`, { headers: HEADERS });
  // fyyd answers a failed request with status 0 and a message.
  if (answer.status === 0)
    throw new Error(`fyyd couldn't answer: ${answer.msg ?? 'unknown error'}`);
  const seen = new Set<string>();
  return (answer.data ?? []).map(toResult).filter((podcast): podcast is PodcastResult => {
    if (!podcast || seen.has(podcast.feedUrl)) return false;
    seen.add(podcast.feedUrl);
    return true;
  });
};

const count = (limit: number) => Math.max(1, Math.min(limit || 20, MOST));

const simple = (text: string | null | undefined) =>
  (text ?? '')
    .toLowerCase()
    .replace(/['’]/g, '')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/** Whether the podcast's title, author or description holds every word asked for. */
const naming = (words: string[]) => (podcast: PodcastResult) => {
  const about = ` ${simple(`${podcast.title} ${podcast.author ?? ''} ${podcast.description ?? ''}`)} `;
  return words.every((word) => about.includes(` ${word}`));
};

registerProvider(
  defineProvider({
    kind: 'podcast.directory',
    id: 'podcasts',
    name: 'fyyd',

    // Titles first; then the full-text search, which matches any word anywhere in a feed, kept to
    // podcasts that name every word.
    async search({ query: text, limit }) {
      const term = text.trim();
      if (!term) return [];
      const words = simple(term).split(' ').filter(Boolean);
      const [byTitle, byTerm] = await Promise.allSettled([
        ask('search/podcast', { title: term, count: count(limit) }),
        ask('search/podcast', { term, count: MOST }),
      ]);
      if (byTitle.status === 'rejected' && byTerm.status === 'rejected') throw byTitle.reason;
      const seen = new Set<string>();
      return [
        ...(byTitle.status === 'fulfilled' ? byTitle.value : []),
        ...(byTerm.status === 'fulfilled' ? byTerm.value.filter(naming(words)) : []),
      ]
        .filter((podcast) => {
          if (seen.has(podcast.feedUrl)) return false;
          seen.add(podcast.feedUrl);
          return true;
        })
        .slice(0, count(limit));
    },

    async top({ limit }) {
      const language = LANGUAGES[raygun.settings.get<string>('language') ?? 'English'];
      const params: Record<string, string | number> = { count: count(limit) };
      if (language) params.language = language;
      const hot = await ask('feature/podcast/hot', params);
      return hot.slice(0, count(limit));
    },
  }),
);
