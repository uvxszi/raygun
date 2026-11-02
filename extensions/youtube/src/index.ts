// YouTube (M15, keyless since M46.7): music search and stream candidates that play in YouTube's
// own embedded player.
//
// Search goes through YouTube Music's web API (InnerTube, the requests music.youtube.com makes),
// as Grayjay, NewPipe and FreeTube do, so no key is needed. With the user's own YouTube Data API
// key set, the official API is used instead.
//
// Deliberately no direct stream extraction (signature deciphering, proof-of-origin tokens): those
// defeat YouTube's bot checks. The embed pauses when Android backgrounds the app, so the resolver
// prefers direct streams and keeps YouTube as the fallback.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { MediaSource, Track } from '@raygun/sdk';

type Video = {
  videoId: string;
  title: string;
  artist: string;
  album: string | null;
  durationMs: number | null;
  artworkUrl: string | null;
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
  playlistItemData?: { videoId?: string };
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

const search = defineProvider({
  kind: 'music.metadata',
  id: 'search',
  name: 'YouTube',

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

registerProvider(search);
registerProvider(streams);
