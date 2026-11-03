// YouTube (M15, keyless since M46.7): music search and stream candidates that play in YouTube's
// own embedded player. Since M46.35 also a video source with its own Search pill: videos and live
// streams found through YouTube's web search, played in the same embed (`youtube:<id>` links).
//
// Search goes through YouTube Music's web API (InnerTube, the requests music.youtube.com makes),
// as Grayjay, NewPipe and FreeTube do, so no key is needed. With the user's own YouTube Data API
// key set, the official API is used instead.
//
// M46.45: the video source also finds channels and playlists (each opens as a series of its
// videos) and reads a video's comments. The response shapes (lockup view models, comment entity
// payloads) were read from Grayjay's YouTube plugin (futo-org/grayjay-plugin-youtube, AGPL-3.0).
//
// Deliberately no direct stream extraction (signature deciphering, proof-of-origin tokens): those
// defeat YouTube's bot checks. The embed pauses when Android backgrounds the app, so the resolver
// prefers direct streams and keeps YouTube as the fallback.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { LyricsQuery, MangaPage, MediaSource, Playlist, Track } from '@raygun/sdk';

type Video = {
  videoId: string;
  title: string;
  artist: string;
  album: string | null;
  durationMs: number | null;
  artworkUrl: string | null;
  /** YouTube Music's E badge (M46.36). */
  explicit?: boolean;
};

/** Auto-generated artist channels are named "Artist - Topic". */
const channelArtist = (channel: string) => channel.replace(/\s*-\s*Topic$/, '');

// ---- YouTube Music (InnerTube) ----

const MUSIC = 'https://music.youtube.com';
/** A recent web client; YouTube rejects very old ones, so a 400 re-reads it from the page. */
const FALLBACK_CLIENT_VERSION = '1.20261027.10.00';
const USER_AGENT =
  'Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/142.0.0.0 Safari/537.36';
/** Search filters, as music.youtube.com's Songs and Videos chips send them. */
const FILTER = {
  songs: 'EgWKAQIIAWoKEAkQBRAKEAMQBA==',
  videos: 'EgWKAQIQAWoKEAkQChAFEAMQBA==',
} as const;

type Run = {
  text: string;
  navigationEndpoint?: {
    watchEndpoint?: { videoId?: string };
    browseEndpoint?: {
      browseEndpointContextSupportedConfigs?: {
        browseEndpointContextMusicConfig?: { pageType?: string };
      };
    };
  };
};
type ListItem = {
  flexColumns?: { musicResponsiveListItemFlexColumnRenderer?: { text?: { runs?: Run[] } } }[];
  navigationEndpoint?: { browseEndpoint?: { browseId?: string } };
  playlistItemData?: { videoId?: string };
  badges?: { musicInlineBadgeRenderer?: { icon?: { iconType?: string } } }[];
  thumbnail?: {
    musicThumbnailRenderer?: { thumbnail?: { thumbnails?: { url: string; width?: number }[] } };
  };
};
type SearchResponse = {
  contents?: {
    tabbedSearchResultsRenderer?: {
      tabs?: {
        tabRenderer?: {
          content?: {
            sectionListRenderer?: {
              contents?: {
                musicShelfRenderer?: {
                  contents?: { musicResponsiveListItemRenderer?: ListItem }[];
                };
              }[];
            };
          };
        };
      }[];
    };
  };
};

let clientVersion = FALLBACK_CLIENT_VERSION;

/** Reads the web client's current version from music.youtube.com, for when ours is refused. */
const refreshClientVersion = async () => {
  const page = await raygun.http.fetch(`${MUSIC}/`, { headers: { 'User-Agent': USER_AGENT } });
  const version = /"INNERTUBE_CLIENT_VERSION":"([\d.]+)"/.exec(await page.text())?.[1];
  if (!version || version === clientVersion) return false;
  clientVersion = version;
  return true;
};

const region = () => (raygun.settings.get<string>('region') ?? '').trim().toUpperCase();

const innertube = async <T>(endpoint: string, body: Record<string, unknown>): Promise<T> => {
  const send = () =>
    raygun.http.fetch(`${MUSIC}/youtubei/v1/${endpoint}?prettyPrint=false`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'User-Agent': USER_AGENT,
        Origin: MUSIC,
        Referer: `${MUSIC}/`,
        'X-YouTube-Client-Name': '67',
        'X-YouTube-Client-Version': clientVersion,
      },
      body: JSON.stringify({
        context: {
          client: {
            clientName: 'WEB_REMIX',
            clientVersion,
            hl: 'en',
            ...(region() ? { gl: region() } : {}),
          },
        },
        ...body,
      }),
    });
  let response = await send();
  if (response.status === 400 && (await refreshClientVersion().catch(() => false))) {
    response = await send();
  }
  if (response.status === 429) {
    throw new Error('YouTube is limiting requests from this network; try again in a while');
  }
  if (!response.ok) throw new Error(`YouTube Music answered HTTP ${response.status}`);
  return response.json<T>();
};

const pageType = (run: Run) =>
  run.navigationEndpoint?.browseEndpoint?.browseEndpointContextSupportedConfigs
    ?.browseEndpointContextMusicConfig?.pageType;

const TIME = /^(?:(\d+):)?(\d{1,2}):(\d{2})$/;
/** "1:02:03" or "3:21" → milliseconds. */
const clockDuration = (text: string) => {
  const match = TIME.exec(text.trim());
  if (!match) return null;
  const [, hours = '0', minutes = '0', seconds = '0'] = match;
  return ((Number(hours) * 60 + Number(minutes)) * 60 + Number(seconds)) * 1000;
};

/** "…=w120-h120-l90-rj" → a 544 px square, the size music.youtube.com shows on its player. */
const largeThumbnail = (url: string) => url.replace(/=w\d+-h\d+/, '=w544-h544');

/** One search result row: the title's column, then "Artist, Artist • Album • 3:45". */
const parseItem = (item: ListItem): Video | null => {
  const columns = (item.flexColumns ?? []).map(
    (column) => column.musicResponsiveListItemFlexColumnRenderer?.text?.runs ?? [],
  );
  const titleRun = columns[0]?.[0];
  const videoId =
    item.playlistItemData?.videoId ?? titleRun?.navigationEndpoint?.watchEndpoint?.videoId;
  if (!titleRun || !videoId) return null;
  const details = columns[1] ?? [];
  // Linked artists; a video's uploader is a plain channel link, so fall back to the first run.
  const artists = details.filter((run) => pageType(run) === 'MUSIC_PAGE_TYPE_ARTIST');
  const uploader = details[0]?.text;
  const artist =
    artists.length > 0
      ? artists.map((run) => run.text).join(', ')
      : uploader && !TIME.test(uploader)
        ? uploader
        : '';
  const album = details.find((run) => pageType(run) === 'MUSIC_PAGE_TYPE_ALBUM')?.text ?? null;
  const duration = details.map((run) => clockDuration(run.text)).find((ms) => ms !== null);
  const thumbnails = item.thumbnail?.musicThumbnailRenderer?.thumbnail?.thumbnails ?? [];
  const thumbnail = thumbnails.at(-1)?.url;
  return {
    videoId,
    title: titleRun.text,
    artist: channelArtist(artist),
    album,
    durationMs: duration ?? null,
    artworkUrl: thumbnail ? largeThumbnail(thumbnail) : null,
    explicit: (item.badges ?? []).some(
      (badge) => badge.musicInlineBadgeRenderer?.icon?.iconType === 'MUSIC_EXPLICIT_BADGE',
    ),
  };
};

const searchMusic = async (query: string, filter: keyof typeof FILTER) => {
  const response = await innertube<SearchResponse>('search', { query, params: FILTER[filter] });
  const sections =
    response.contents?.tabbedSearchResultsRenderer?.tabs?.[0]?.tabRenderer?.content
      ?.sectionListRenderer?.contents ?? [];
  return sections
    .flatMap((section) => section.musicShelfRenderer?.contents ?? [])
    .map((entry) =>
      entry.musicResponsiveListItemRenderer
        ? parseItem(entry.musicResponsiveListItemRenderer)
        : null,
    )
    .filter((video): video is Video => video !== null);
};

/** Songs first (official audio, with albums), then music videos to fill up to `limit`. */
const searchKeyless = async (query: string, limit: number) => {
  const songs = await searchMusic(query, 'songs');
  if (songs.length >= limit) return songs.slice(0, limit);
  const seen = new Set(songs.map((song) => song.videoId));
  const videos = await searchMusic(query, 'videos').catch((): Video[] => []);
  return [...songs, ...videos.filter((video) => !seen.has(video.videoId))].slice(0, limit);
};

// ---- The official YouTube Data API v3, with the user's key ----

const API = 'https://www.googleapis.com/youtube/v3';
const MUSIC_CATEGORY = '10';

type SearchItem = {
  id: { videoId?: string };
  snippet: {
    title: string;
    channelTitle: string;
    thumbnails?: Record<string, { url: string } | undefined>;
  };
};
type VideoItem = { id: string; contentDetails?: { duration?: string } };
type ApiError = { error?: { code?: number; message?: string; errors?: { reason?: string }[] } };

const apiKey = () => (raygun.settings.get<string>('apiKey') ?? '').trim();

const call = async <T>(path: string, params: Record<string, string | number>): Promise<T> => {
  const query = Object.entries({ ...params, key: apiKey() })
    .map(([name, value]) => `${name}=${encodeURIComponent(String(value))}`)
    .join('&');
  const response = await raygun.http.fetch(`${API}/${path}?${query}`);
  if (response.ok) return response.json<T>();
  const body = await response.json<ApiError>().catch((): ApiError => ({}));
  const reason = body.error?.errors?.[0]?.reason;
  if (reason === 'quotaExceeded' || reason === 'dailyLimitExceeded') {
    throw new Error(
      "Today's YouTube API quota is used up; it resets at midnight Pacific time. Clear the key to search without it",
    );
  }
  if (reason === 'keyInvalid' || response.status === 400) {
    throw new Error('YouTube rejected the API key; check it in the extension settings or clear it');
  }
  throw new Error(body.error?.message ?? `YouTube answered HTTP ${response.status}`);
};

const ENTITIES: Record<string, string> = {
  '&amp;': '&',
  '&quot;': '"',
  '&#39;': "'",
  '&lt;': '<',
  '&gt;': '>',
};
/** The API returns HTML-escaped titles. */
const unescape = (text: string) =>
  text.replace(/&(amp|quot|#39|lt|gt);/g, (entity) => ENTITIES[entity] ?? entity);

/** "PT1H2M3S" → milliseconds. */
const isoDuration = (value: string | undefined) => {
  const match = value?.match(/^PT(?:(\d+)H)?(?:(\d+)M)?(?:(\d+)S)?$/);
  if (!match) return null;
  const [, hours = '0', minutes = '0', seconds = '0'] = match;
  return ((Number(hours) * 60 + Number(minutes)) * 60 + Number(seconds)) * 1000 || null;
};

const searchWithKey = async (q: string, limit: number): Promise<Video[]> => {
  const search = await call<{ items?: SearchItem[] }>('search', {
    part: 'snippet',
    type: 'video',
    videoCategoryId: MUSIC_CATEGORY,
    maxResults: Math.min(Math.max(limit, 1), 25),
    q,
    ...(region() ? { regionCode: region() } : {}),
  });
  const items = (search.items ?? []).filter((item) => item.id.videoId);
  if (items.length === 0) return [];
  // Lengths come from a second, cheap call (1 quota unit instead of search's 100).
  const videos = await call<{ items?: VideoItem[] }>('videos', {
    part: 'contentDetails',
    id: items.map((item) => item.id.videoId).join(','),
  });
  const lengths = new Map(
    (videos.items ?? []).map((video) => [video.id, isoDuration(video.contentDetails?.duration)]),
  );
  return items.map((item) => {
    const videoId = item.id.videoId as string;
    const thumbnails = item.snippet.thumbnails ?? {};
    return {
      videoId,
      title: unescape(item.snippet.title),
      artist: channelArtist(unescape(item.snippet.channelTitle)),
      album: null,
      durationMs: lengths.get(videoId) ?? null,
      artworkUrl: (thumbnails.high ?? thumbnails.medium ?? thumbnails.default)?.url ?? null,
    };
  });
};

// ---- Providers ----

const searchVideos = (query: string, limit: number) =>
  apiKey() ? searchWithKey(query, limit) : searchKeyless(query, Math.max(limit, 1));

const embed = (videoId: string): MediaSource => ({ kind: 'youTube', videoId });

// ---- Playlists and lyrics (M46.39), read the way Kreate and RiMusic read them ----

/** music.youtube.com's Community playlists and Featured playlists search chips. */
const PLAYLIST_FILTER = {
  community: 'EgeKAQQoAEABagoQAxAEEAoQCRAF',
  featured: 'EgeKAQQoADgBagwQDhAKEAMQBRAJEAQ=',
} as const;

/** Every list row anywhere in an answer, in order: playlist pages nest them differently. */
const listItems = (value: unknown, found: ListItem[] = [], depth = 0): ListItem[] => {
  if (depth > 14 || value === null || typeof value !== 'object') return found;
  if (Array.isArray(value)) {
    for (const item of value) listItems(item, found, depth + 1);
    return found;
  }
  for (const [key, child] of Object.entries(value as Record<string, unknown>)) {
    if (key === 'musicResponsiveListItemRenderer') found.push(child as ListItem);
    else listItems(child, found, depth + 1);
  }
  return found;
};

/** The first object under `key` anywhere in an answer. */
const findKey = (value: unknown, key: string, depth = 0): Record<string, unknown> | null => {
  if (depth > 14 || value === null || typeof value !== 'object') return null;
  if (Array.isArray(value)) {
    for (const item of value) {
      const found = findKey(item, key, depth + 1);
      if (found) return found;
    }
    return null;
  }
  const record = value as Record<string, unknown>;
  if (key in record && record[key] && typeof record[key] === 'object') {
    return record[key] as Record<string, unknown>;
  }
  for (const child of Object.values(record)) {
    const found = findKey(child, key, depth + 1);
    if (found) return found;
  }
  return null;
};

const runsText = (value: unknown) =>
  ((value as { runs?: Run[] } | undefined)?.runs ?? []).map((run) => run.text).join('');

const thumbnailOf = (value: unknown) => {
  const thumbs =
    (
      findKey(value, 'musicThumbnailRenderer') as {
        thumbnail?: { thumbnails?: { url: string }[] };
      } | null
    )?.thumbnail?.thumbnails ?? [];
  const url = thumbs.at(-1)?.url;
  return url ? largeThumbnail(url) : null;
};

/** A playlist row of a search: "Playlist • Author • 1.2M views" or "YouTube Music • 50 songs". */
const parsePlaylist = (item: ListItem, featured: boolean): Playlist | null => {
  const browseId = item.navigationEndpoint?.browseEndpoint?.browseId;
  const columns = (item.flexColumns ?? []).map(
    (column) => column.musicResponsiveListItemFlexColumnRenderer?.text?.runs ?? [],
  );
  const title = columns[0]?.[0]?.text;
  if (!browseId || !title) return null;
  const details = (columns[1] ?? []).map((run) => run.text.trim()).filter(Boolean);
  const parts = details.filter((part) => part !== '•' && part !== 'Playlist');
  const count = parts.map((part) => /^([\d,.]+)\s+(songs?|tracks?)$/i.exec(part)).find(Boolean);
  return {
    id: browseId,
    title,
    author: parts.find((part) => !/views?$|songs?$|tracks?$/i.test(part)) ?? null,
    artworkUrl: thumbnailOf(item),
    trackCount: count ? Number(count[1]?.replace(/[,.]/g, '')) : null,
    featured,
  };
};

const searchPlaylists = async (query: string, featured: boolean, limit: number) => {
  const response = await innertube<unknown>('search', {
    query,
    params: featured ? PLAYLIST_FILTER.featured : PLAYLIST_FILTER.community,
  });
  return listItems(response)
    .map((item) => parsePlaylist(item, featured))
    .filter((playlist): playlist is Playlist => playlist !== null)
    .slice(0, limit);
};

/** A playlist's page: its header and every song row. */
const playlistPage = async (id: string) => {
  const browseId = id.startsWith('VL') || id.startsWith('MPRE') ? id : `VL${id}`;
  const response = await innertube<unknown>('browse', { browseId });
  const header =
    findKey(response, 'musicResponsiveHeaderRenderer') ??
    findKey(response, 'musicDetailHeaderRenderer') ??
    findKey(response, 'musicEditablePlaylistDetailHeaderRenderer');
  const songs = listItems(response)
    .map(parseItem)
    .filter((video): video is Video => video !== null);
  const playlist: Playlist = {
    id: browseId,
    title: runsText(header?.title) || 'Playlist',
    author: runsText(header?.straplineTextOne) || runsText(header?.subtitle) || null,
    artworkUrl: thumbnailOf(header) ?? songs[0]?.artworkUrl ?? null,
    trackCount: songs.length,
    featured: false,
  };
  return { playlist, songs };
};

const toTrack = (video: Video): Track => ({
  id: video.videoId,
  title: video.title,
  artist: video.artist,
  album: video.album,
  durationMs: video.durationMs,
  artworkUrl: video.artworkUrl,
  source: embed(video.videoId),
  explicit: video.explicit ?? null,
});

const simpleWords = (text: string) =>
  text
    .toLowerCase()
    .replace(/\(.*?\)|\[.*?\]/g, ' ')
    .replace(/[^\p{L}\p{N}]+/gu, ' ')
    .trim();

/** YouTube Music's own lyrics for a song (plain): its Lyrics tab, found through `next`. */
const youTubeLyrics = async (query: LyricsQuery) => {
  const words = [query.artist, query.title].filter(Boolean).join(' ');
  const songs = await searchMusic(words, 'songs');
  const wanted = simpleWords(query.title);
  const song = songs.find((candidate) => simpleWords(candidate.title).includes(wanted));
  if (!song) return null;
  const next = await innertube<unknown>('next', { videoId: song.videoId, isAudioOnly: true });
  const tabs =
    (
      findKey(next, 'watchNextTabbedResultsRenderer') as {
        tabs?: { tabRenderer?: { endpoint?: { browseEndpoint?: { browseId?: string } } } }[];
      } | null
    )?.tabs ?? [];
  const browseId = tabs
    .map((tab) => tab.tabRenderer?.endpoint?.browseEndpoint?.browseId)
    .find((id) => id?.startsWith('MPLY'));
  if (!browseId) return null;
  const page = await innertube<unknown>('browse', { browseId });
  const shelf = findKey(page, 'musicDescriptionShelfRenderer');
  const text = runsText(shelf?.description).trim();
  return text ? { synced: [], plain: text } : null;
};

const search = defineProvider({
  kind: 'music.metadata',
  id: 'search',
  name: 'YouTube',

  async searchPlaylists({ query, limit, featured }) {
    if (!query.trim()) return { playlists: [] };
    return { playlists: await searchPlaylists(query, featured, Math.max(limit, 1)) };
  },

  async playlist({ id }) {
    const { playlist, songs } = await playlistPage(id);
    return { playlist, tracks: songs.map(toTrack) };
  },

  async search({ query, limit }) {
    if (!query.trim()) return { tracks: [] };
    const videos = await searchVideos(query, limit);
    const tracks: Track[] = videos.map((video) => ({
      id: video.videoId,
      title: video.title,
      artist: video.artist,
      album: video.album,
      durationMs: video.durationMs,
      artworkUrl: video.artworkUrl,
      source: embed(video.videoId),
      explicit: video.explicit ?? null,
    }));
    return { tracks };
  },
});

const streams = defineProvider({
  kind: 'music.stream',
  id: 'stream',
  name: 'YouTube',

  async candidates({ title, artist, limit }) {
    if (!title.trim()) return { candidates: [] };
    const videos = await searchVideos(
      [artist, title].filter(Boolean).join(' '),
      Math.min(limit, 5),
    );
    return {
      candidates: videos.map((video) => ({
        id: video.videoId,
        title: video.title,
        artist: video.artist,
        durationMs: video.durationMs,
        quality: 'YouTube',
        source: embed(video.videoId),
      })),
    };
  },
});

// ---- Videos and live streams (www.youtube.com's InnerTube, M46.35) ----

const WEB = 'https://www.youtube.com';
let webClientVersion = '2.20261028.01.00';

type Text = { simpleText?: string; runs?: { text: string }[] };
type Thumbnails = { thumbnails?: { url: string; width?: number }[] };
type VideoRenderer = {
  videoId?: string;
  title?: Text;
  thumbnail?: Thumbnails;
};
/** YouTube's newer cards: videos in channels and playlists, playlists in search. */
type Lockup = {
  contentId?: string;
  contentType?: string;
  metadata?: { lockupMetadataViewModel?: { title?: { content?: string } } };
  contentImage?: {
    thumbnailViewModel?: { image?: { sources?: { url: string }[] } };
    collectionThumbnailViewModel?: {
      primaryThumbnail?: { thumbnailViewModel?: { image?: { sources?: { url: string }[] } } };
    };
  };
};
type Item = {
  videoRenderer?: VideoRenderer;
  channelRenderer?: { channelId?: string; title?: Text; thumbnail?: Thumbnails };
  lockupViewModel?: Lockup;
  itemSectionRenderer?: { contents?: Item[] };
  shelfRenderer?: { content?: { expandedShelfContentsRenderer?: { items?: Item[] } } };
  richItemRenderer?: { content?: Item };
  continuationItemRenderer?: {
    continuationEndpoint?: { continuationCommand?: { token?: string } };
  };
};
type WebResponse = {
  contents?: {
    twoColumnSearchResultsRenderer?: {
      primaryContents?: { sectionListRenderer?: { contents?: Item[] } };
    };
  };
  onResponseReceivedCommands?: {
    appendContinuationItemsAction?: { continuationItems?: Item[] };
  }[];
};

const textOf = (text: Text | undefined) =>
  text?.simpleText ?? text?.runs?.map((run) => run.text).join('') ?? '';

const webInnertube = async <T>(endpoint: string, body: Record<string, unknown>): Promise<T> => {
  const send = () =>
    raygun.http.fetch(`${WEB}/youtubei/v1/${endpoint}?prettyPrint=false`, {
      method: 'POST',
      headers: {
        'Content-Type': 'application/json',
        'User-Agent': USER_AGENT,
        Origin: WEB,
        Referer: `${WEB}/`,
        'X-YouTube-Client-Name': '1',
        'X-YouTube-Client-Version': webClientVersion,
      },
      body: JSON.stringify({
        context: {
          client: {
            clientName: 'WEB',
            clientVersion: webClientVersion,
            hl: 'en',
            ...(region() ? { gl: region() } : {}),
          },
        },
        ...body,
      }),
    });
  let response = await send();
  if (response.status === 400) {
    // An outdated client version: read the current one from the page and try once more.
    const page = await raygun.http.fetch(`${WEB}/`, { headers: { 'User-Agent': USER_AGENT } });
    const version = /"INNERTUBE_CLIENT_VERSION":"([\d.]+)"/.exec(await page.text())?.[1];
    if (version && version !== webClientVersion) {
      webClientVersion = version;
      response = await send();
    }
  }
  if (!response.ok) throw new Error(`YouTube answered ${response.status}`);
  return (await response.json()) as T;
};

type Entry = { id: string; title: string; coverUrl: string | null };

const PREFIX = { channel: 'channel:', playlist: 'playlist:' } as const;

/** A thumbnail's address without its size query; protocol-relative ones get https. */
const imageUrl = (url: string | undefined) => {
  if (!url) return null;
  const full = url.startsWith('//') ? `https:${url}` : url;
  return full.includes('ytimg.com') ? (full.split('?')[0] ?? full) : full;
};

const lockupImage = (lockup: Lockup) =>
  imageUrl(
    (
      lockup.contentImage?.thumbnailViewModel ??
      lockup.contentImage?.collectionThumbnailViewModel?.primaryThumbnail?.thumbnailViewModel
    )?.image?.sources?.at(-1)?.url,
  );

/**
 * Every video, channel and playlist in a list of items, however deeply YouTube nested them, in
 * order (M46.45: channels and playlists open as a series of their videos). `videosOnly` for a
 * channel's or a playlist's own videos.
 */
const entriesIn = (items: Item[] | undefined, videosOnly = false, out: Entry[] = []): Entry[] => {
  for (const item of items ?? []) {
    const video = item.videoRenderer;
    if (video?.videoId) {
      out.push({
        id: video.videoId,
        title: textOf(video.title),
        coverUrl: imageUrl(video.thumbnail?.thumbnails?.at(-1)?.url),
      });
    }
    const channel = item.channelRenderer;
    if (channel?.channelId && !videosOnly) {
      out.push({
        id: `${PREFIX.channel}${channel.channelId}`,
        title: textOf(channel.title),
        coverUrl: imageUrl(channel.thumbnail?.thumbnails?.at(-1)?.url),
      });
    }
    const lockup = item.lockupViewModel;
    if (lockup?.contentId) {
      const title = lockup.metadata?.lockupMetadataViewModel?.title?.content ?? '';
      if (lockup.contentType === 'LOCKUP_CONTENT_TYPE_VIDEO') {
        out.push({ id: lockup.contentId, title, coverUrl: lockupImage(lockup) });
      } else if (lockup.contentType === 'LOCKUP_CONTENT_TYPE_PLAYLIST' && !videosOnly) {
        out.push({
          id: `${PREFIX.playlist}${lockup.contentId}`,
          title,
          coverUrl: lockupImage(lockup),
        });
      }
    }
    if (item.itemSectionRenderer) entriesIn(item.itemSectionRenderer.contents, videosOnly, out);
    if (item.shelfRenderer) {
      entriesIn(item.shelfRenderer.content?.expandedShelfContentsRenderer?.items, videosOnly, out);
    }
    if (item.richItemRenderer?.content) entriesIn([item.richItemRenderer.content], videosOnly, out);
  }
  return out;
};

const continuationIn = (items: Item[] | undefined): string | null => {
  for (const item of items ?? []) {
    const token = item.continuationItemRenderer?.continuationEndpoint?.continuationCommand?.token;
    if (token) return token;
    const nested = continuationIn(item.itemSectionRenderer?.contents);
    if (nested) return nested;
  }
  return null;
};

const toPage = (found: Entry[], next: string | null): MangaPage => ({
  items: found,
  hasNextPage: next !== null,
});

// Each search's next-page token, by query and page.
const pages = new Map<string, string>();

const searchWeb = async (query: string, page: number): Promise<MangaPage> => {
  const key = (n: number) => `${query}|${n}`;
  let items: Item[] | undefined;
  if (page > 1) {
    const token = pages.get(key(page));
    if (!token) return { items: [], hasNextPage: false };
    const response = await webInnertube<WebResponse>('search', { continuation: token });
    items = response.onResponseReceivedCommands?.flatMap(
      (command) => command.appendContinuationItemsAction?.continuationItems ?? [],
    );
  } else {
    const response = await webInnertube<WebResponse>('search', { query });
    items =
      response.contents?.twoColumnSearchResultsRenderer?.primaryContents?.sectionListRenderer
        ?.contents;
  }
  const next = continuationIn(items);
  if (next) pages.set(key(page + 1), next);
  if (pages.size > 60) pages.delete(pages.keys().next().value!);
  return toPage(entriesIn(items), next);
};

type OEmbed = { title?: string; author_name?: string; thumbnail_url?: string };

type BrowseResponse = {
  metadata?: {
    channelMetadataRenderer?: { title?: string; description?: string; avatar?: Thumbnails };
    playlistMetadataRenderer?: { title?: string; description?: string };
  };
  contents?: {
    twoColumnBrowseResultsRenderer?: {
      tabs?: {
        tabRenderer?: {
          selected?: boolean;
          content?: {
            richGridRenderer?: { contents?: Item[] };
            sectionListRenderer?: { contents?: Item[] };
          };
        };
      }[];
    };
  };
};

/** The Videos tab, as youtube.com's channel page asks for it. */
const CHANNEL_VIDEOS = 'EgZ2aWRlb3PyBgQKAjoA';

/** A channel's (Videos tab) or a playlist's page. */
const browsePage = (id: string) =>
  id.startsWith(PREFIX.channel)
    ? webInnertube<BrowseResponse>('browse', {
        browseId: id.slice(PREFIX.channel.length),
        params: CHANNEL_VIDEOS,
      })
    : webInnertube<BrowseResponse>('browse', {
        browseId: `VL${id.slice(PREFIX.playlist.length)}`,
      });

/** The videos a channel's or a playlist's page lists. */
const browseVideos = (page: BrowseResponse): Entry[] => {
  const tabs = page.contents?.twoColumnBrowseResultsRenderer?.tabs ?? [];
  const tab = tabs.find((each) => each.tabRenderer?.selected) ?? tabs[0];
  const content = tab?.tabRenderer?.content;
  return entriesIn(
    content?.richGridRenderer?.contents ?? content?.sectionListRenderer?.contents,
    true,
  );
};

const isCollection = (id: string) =>
  id.startsWith(PREFIX.channel) || id.startsWith(PREFIX.playlist);

// ---- Comments (M46.45) ----

type CommentPayload = {
  author?: { displayName?: string; avatarThumbnailUrl?: string };
  properties?: { content?: { content?: string }; publishedTime?: string };
  toolbar?: { likeCountNotliked?: string; replyCount?: string };
};
type ThreadItem = Item & {
  commentThreadRenderer?: {
    commentViewModel?: { commentViewModel?: { commentKey?: string } };
  };
};
type ContinuationItems = { continuationItems?: ThreadItem[] };
type NextResponse = {
  contents?: {
    twoColumnWatchNextResults?: {
      results?: {
        results?: {
          contents?: {
            itemSectionRenderer?: { targetId?: string; contents?: Item[] };
          }[];
        };
      };
    };
  };
  onResponseReceivedEndpoints?: {
    reloadContinuationItemsCommand?: ContinuationItems;
    appendContinuationItemsAction?: ContinuationItems;
  }[];
  frameworkUpdates?: {
    entityBatchUpdate?: {
      mutations?: { entityKey?: string; payload?: { commentEntityPayload?: CommentPayload } }[];
    };
  };
};

const tokenOf = (item: Item | undefined) =>
  item?.continuationItemRenderer?.continuationEndpoint?.continuationCommand?.token ?? null;

/** A video's comments, a page at a time: `page` is the token the last page gave. */
const videoComments = async (id: string, page?: string | null) => {
  if (isCollection(id)) return { comments: [], next: null };
  let token = page ?? null;
  if (!token) {
    const watch = await webInnertube<NextResponse>('next', { videoId: id });
    const sections = watch.contents?.twoColumnWatchNextResults?.results?.results?.contents ?? [];
    const section = sections.find(
      (each) => each.itemSectionRenderer?.targetId === 'comments-section',
    );
    token = tokenOf(section?.itemSectionRenderer?.contents?.[0]);
    // Turned off, or none yet.
    if (!token) return { comments: [], next: null };
  }
  const answer = await webInnertube<NextResponse>('next', { continuation: token });
  const items = (answer.onResponseReceivedEndpoints ?? []).flatMap(
    (endpoint) =>
      (endpoint.reloadContinuationItemsCommand ?? endpoint.appendContinuationItemsAction)
        ?.continuationItems ?? [],
  );
  const payloads = new Map<string, CommentPayload>();
  for (const mutation of answer.frameworkUpdates?.entityBatchUpdate?.mutations ?? []) {
    const payload = mutation.payload?.commentEntityPayload;
    if (mutation.entityKey && payload) payloads.set(mutation.entityKey, payload);
  }
  const comments = items.flatMap((item) => {
    const key = item.commentThreadRenderer?.commentViewModel?.commentViewModel?.commentKey;
    const comment = key ? payloads.get(key) : undefined;
    if (!comment) return [];
    const replies = Number.parseInt(comment.toolbar?.replyCount ?? '', 10);
    return [
      {
        author: comment.author?.displayName ?? '',
        avatarUrl: comment.author?.avatarThumbnailUrl ?? null,
        text: comment.properties?.content?.content ?? '',
        likes: comment.toolbar?.likeCountNotliked?.trim() || null,
        published: comment.properties?.publishedTime ?? null,
        replies: Number.isFinite(replies) ? replies : null,
      },
    ];
  });
  return { comments, next: tokenOf(items.at(-1)) };
};

const videos = defineProvider({
  kind: 'video.source',
  id: 'videos',
  name: 'YouTube',
  searchPill: 'YouTube',

  // With nothing typed: what's trending.
  popular: ({ page }) => searchWeb('trending', page),

  latest: ({ page }) => searchWeb('live now', page),

  search: ({ query, page }) => searchWeb(query.trim() || 'trending', page),

  async details({ id }) {
    // A channel or a playlist (M46.45): a series of its videos.
    if (isCollection(id)) {
      const page = await browsePage(id);
      const channel = page.metadata?.channelMetadataRenderer;
      const playlist = page.metadata?.playlistMetadataRenderer;
      const own = id.startsWith(PREFIX.channel)
        ? `${WEB}/channel/${id.slice(PREFIX.channel.length)}`
        : `${WEB}/playlist?list=${id.slice(PREFIX.playlist.length)}`;
      return {
        id,
        title: channel?.title ?? playlist?.title ?? id,
        coverUrl:
          imageUrl(channel?.avatar?.thumbnails?.at(-1)?.url) ??
          browseVideos(page)[0]?.coverUrl ??
          null,
        description: channel?.description ?? playlist?.description ?? null,
        genres: [],
        type: 'series' as const,
        url: own,
      };
    }
    const watch = `${WEB}/watch?v=${encodeURIComponent(id)}`;
    const response = await raygun.http.fetch(
      `${WEB}/oembed?format=json&url=${encodeURIComponent(watch)}`,
      { headers: { 'User-Agent': USER_AGENT } },
    );
    const meta = response.ok ? ((await response.json()) as OEmbed) : {};
    return {
      id,
      title: meta.title ?? id,
      coverUrl: meta.thumbnail_url ?? `https://i.ytimg.com/vi/${id}/hqdefault.jpg`,
      description: meta.author_name ? `By ${meta.author_name}` : null,
      genres: [],
      type: 'movie' as const,
      url: watch,
    };
  },

  episodes: async ({ id }) => {
    if (!isCollection(id)) return { episodes: [{ id, name: 'Video' }] };
    const videos = browseVideos(await browsePage(id));
    return {
      episodes: videos.map((video, index) => ({
        id: video.id,
        name: video.title,
        number: index + 1,
      })),
    };
  },

  comments: ({ id, page }) => videoComments(id, page),

  // Played in YouTube's embed: one link, no stream extraction.
  servers: async ({ episodeId }) => ({
    servers: [
      {
        id: episodeId,
        name: 'YouTube',
        videos: [{ url: `youtube:${episodeId}`, label: 'YouTube' }],
      },
    ],
  }),

  videos: async ({ serverId }) => ({
    videos: [{ url: `youtube:${serverId}`, label: 'YouTube' }],
  }),
});

registerProvider(search);
registerProvider(streams);
registerProvider(videos);
registerProvider(
  defineProvider({
    kind: 'lyrics',
    id: 'lyrics',
    name: 'YouTube Music',
    lyrics: youTubeLyrics,
  }),
);
