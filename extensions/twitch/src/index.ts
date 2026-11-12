// Twitch: a video source (`video.source`) for live channels, past broadcasts and clips, through
// Twitch's official API (Helix) with the person's own registered application. Its popular list is
// what's live now by viewers; search finds channels, or with nothing typed lists live streams by
// category and language; a channel's page has its stream, its videos and its top clips.
//
// Everything plays in Twitch's own player: Twitch's developer agreement requires its embeddable
// player for all Twitch video, so episodes answer pages (`format: 'page'`) that Raygun opens in its
// browser rather than streams for Raygun's player.
import { defineProvider, registerProvider, toggleValue } from '@raygun/sdk';
import type {
  FilterValues,
  MangaPage,
  SelectFilter,
  SourceFilter,
  ToggleFilter,
  VideoEpisode,
} from '@raygun/sdk';

import { helix } from './helix';
import {
  LANGUAGES,
  categoryOptions,
  channelDetails,
  channelItem,
  clipEpisode,
  isLogin,
  liveEpisode,
  playerPages,
  streamItem,
  videoEpisode,
} from './parse';
import type {
  HelixChannel,
  HelixChannelResult,
  HelixClip,
  HelixGame,
  HelixStream,
  HelixUser,
  HelixVideo,
} from './parse';

const PAGE_SIZE = 30;
const ALL = 'all';

const LIVE_ONLY: ToggleFilter = {
  kind: 'toggle',
  key: 'liveOnly',
  label: 'Live channels only',
  default: false,
};
const LANGUAGE: SelectFilter = {
  kind: 'select',
  key: 'language',
  label: 'Language',
  options: [
    { value: ALL, label: 'All languages' },
    ...LANGUAGES.map(([value, label]) => ({ value, label })),
  ],
  default: ALL,
};

/** Each list's next-page cursor, by the list and the page it opens. */
const cursors = new Map<string, string>();

const cursorFor = (list: string, page: number): string | undefined | null => {
  if (page <= 1) return undefined;
  return cursors.get(`${list}|${page}`) ?? null;
};

const remember = (list: string, page: number, cursor: string | undefined) => {
  if (!cursor) return false;
  cursors.set(`${list}|${page + 1}`, cursor);
  if (cursors.size > 200) cursors.delete(cursors.keys().next().value!);
  return true;
};

const EMPTY: MangaPage = { items: [], hasNextPage: false };

/** A page of streams on the air now, most watched first. */
const streams = async (
  filters: { game?: string; language?: string },
  page: number,
): Promise<MangaPage> => {
  const list = `streams|${filters.game ?? ''}|${filters.language ?? ''}`;
  const after = cursorFor(list, page);
  if (after === null) return EMPTY;
  const answer = await helix<HelixStream>('streams', {
    first: PAGE_SIZE,
    after,
    game_id: filters.game,
    language: filters.language,
  });
  const now = Date.now();
  const items = (answer.data ?? [])
    .filter((stream) => isLogin(stream.user_login))
    .map((stream) => streamItem(stream, now));
  return {
    items,
    hasNextPage: remember(list, page, answer.pagination?.cursor) && items.length > 0,
  };
};

/** Channels a search found, live ones first, kept to a category and a language when chosen. */
const channels = async (
  words: string,
  filters: { game?: string; language?: string; liveOnly: boolean },
  page: number,
): Promise<MangaPage> => {
  const list = `channels|${words}|${filters.liveOnly}`;
  const after = cursorFor(list, page);
  if (after === null) return EMPTY;
  const answer = await helix<HelixChannelResult>('search/channels', {
    query: words,
    first: PAGE_SIZE,
    after,
    live_only: filters.liveOnly,
  });
  const found = (answer.data ?? []).filter(
    (channel) =>
      isLogin(channel.broadcaster_login) &&
      (!filters.game || channel.game_id === filters.game) &&
      (!filters.language || channel.broadcaster_language === filters.language),
  );
  const items = [
    ...found.filter((channel) => channel.is_live),
    ...found.filter((channel) => !channel.is_live),
  ].map(channelItem);
  return {
    items,
    hasNextPage: remember(list, page, answer.pagination?.cursor) && (answer.data ?? []).length > 0,
  };
};

/** The categories of Twitch's top games, read once a run. */
let topGames: Promise<HelixGame[]> | null = null;
const games = () => {
  if (!topGames) {
    topGames = helix<HelixGame>('games/top', { first: 100 }).then((answer) => answer.data ?? []);
    topGames.catch(() => {
      topGames = null;
    });
  }
  return topGames;
};

const chosen = (filters: FilterValues | undefined) => {
  const game = filters?.category;
  const language = filters?.language;
  return {
    game: typeof game === 'string' && /^\d+$/.test(game) ? game : undefined,
    language:
      typeof language === 'string' && LANGUAGES.some(([code]) => code === language)
        ? language
        : undefined,
    liveOnly: toggleValue(filters, LIVE_ONLY),
  };
};

/** Channels by login, kept for the run (their ids, names and pictures don't change often). */
const users = new Map<string, Promise<HelixUser>>();
const userOf = (login: string): Promise<HelixUser> => {
  const name = login.toLowerCase();
  if (!isLogin(name)) return Promise.reject(new Error('Not a Twitch channel'));
  let found = users.get(name);
  if (!found) {
    found = helix<HelixUser>('users', { login: name }).then((answer) => {
      const user = answer.data?.[0];
      if (!user) throw new Error(`Twitch has no channel called ${name}`);
      return user;
    });
    found.catch(() => users.delete(name));
    users.set(name, found);
    if (users.size > 100) users.delete(users.keys().next().value!);
  }
  return found;
};

const streamOf = async (user: HelixUser) =>
  (await helix<HelixStream>('streams', { user_id: user.id })).data?.[0] ?? null;

registerProvider(
  defineProvider({
    kind: 'video.source',
    id: 'twitch',
    name: 'Twitch',
    searchPill: 'Twitch',

    popular: ({ page }) => streams({}, page),

    filters: async (): Promise<SourceFilter[]> => {
      const options = categoryOptions(await games().catch(() => []));
      const category: SelectFilter = {
        kind: 'select',
        key: 'category',
        label: 'Category',
        options: [{ value: ALL, label: 'All categories' }, ...options],
        default: ALL,
      };
      return [category, LANGUAGE, LIVE_ONLY];
    },

    search: ({ query: typed, page, filters }) => {
      const words = typed.trim();
      const picked = chosen(filters);
      return words ? channels(words, picked, page) : streams(picked, page);
    },

    details: async ({ id }) => {
      const user = await userOf(id);
      const [channel, stream] = await Promise.all([
        helix<HelixChannel>('channels', { broadcaster_id: user.id }).then(
          (answer) => answer.data?.[0] ?? null,
        ),
        streamOf(user),
      ]);
      return channelDetails(user, channel, stream, Date.now());
    },

    episodes: async ({ id }) => {
      const user = await userOf(id);
      const [stream, videos, clips] = await Promise.all([
        streamOf(user),
        helix<HelixVideo>('videos', { user_id: user.id, first: 50, type: 'all' }).then(
          (answer) => answer.data ?? [],
        ),
        helix<HelixClip>('clips', { broadcaster_id: user.id, first: 20 })
          .then((answer) => answer.data ?? [])
          .catch(() => []),
      ]);
      const episodes: VideoEpisode[] = [
        ...(stream ? [liveEpisode(user.login, stream)] : []),
        ...videos.filter((video) => /^\d+$/.test(video.id)).map(videoEpisode),
        ...clips.filter((clip) => /^[\w-]{1,120}$/.test(clip.id)).map(clipEpisode),
      ];
      if (episodes.length === 0) {
        throw new Error(`${user.display_name} isn't live and has no videos or clips`);
      }
      return { episodes };
    },

    // Twitch's player needs nothing resolved: each episode's one server comes with its pages.
    servers: async ({ episodeId }) => {
      const videos = playerPages(episodeId);
      if (videos.length === 0) throw new Error('Not a Twitch stream, video or clip');
      return { servers: [{ id: episodeId, name: 'Twitch', videos }] };
    },
    videos: async ({ serverId }) => ({ videos: playerPages(serverId) }),
  }),
);
