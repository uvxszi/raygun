// YouTube through the official YouTube Data API v3 (M15): music search, and stream candidates that
// play in YouTube's own embedded player. Needs the user's API key (a setting).
//
// Deliberately no direct stream extraction (InnerTube signature deciphering): it works around
// YouTube's protections and its terms. The embed pauses when Android backgrounds the app, so the
// resolver prefers direct streams and keeps YouTube as the fallback.
import { defineProvider, registerProvider } from '@raygun/sdk';
import type { MediaSource, Track } from '@raygun/sdk';

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

const apiKey = () => {
  const key = (raygun.settings.get<string>('apiKey') ?? '').trim();
  if (!key) {
    throw new Error('Add your YouTube Data API key in Settings → Extensions → YouTube');
  }
  return key;
};

const call = async <T>(path: string, params: Record<string, string | number>): Promise<T> => {
  const query = Object.entries({ ...params, key: apiKey() })
    .map(([name, value]) => `${name}=${encodeURIComponent(String(value))}`)
    .join('&');
  const response = await raygun.http.fetch(`${API}/${path}?${query}`);
  if (response.ok) return response.json<T>();
  const body = await response.json<ApiError>().catch((): ApiError => ({}));
  const reason = body.error?.errors?.[0]?.reason;
  if (reason === 'quotaExceeded' || reason === 'dailyLimitExceeded') {
    throw new Error("Today's YouTube API quota is used up; it resets at midnight Pacific time");
  }
  if (reason === 'keyInvalid' || response.status === 400) {
    throw new Error('YouTube rejected the API key; check it in the extension settings');
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

/** Auto-generated artist channels are named "Artist - Topic". */
const channelArtist = (channel: string) => channel.replace(/\s*-\s*Topic$/, '');

const searchVideos = async (q: string, limit: number) => {
  const region = (raygun.settings.get<string>('region') ?? '').trim().toUpperCase();
  const search = await call<{ items?: SearchItem[] }>('search', {
    part: 'snippet',
    type: 'video',
    videoCategoryId: MUSIC_CATEGORY,
    maxResults: Math.min(Math.max(limit, 1), 25),
    q,
    ...(region ? { regionCode: region } : {}),
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
      durationMs: lengths.get(videoId) ?? null,
      artworkUrl: (thumbnails.high ?? thumbnails.medium ?? thumbnails.default)?.url ?? null,
    };
  });
};

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
