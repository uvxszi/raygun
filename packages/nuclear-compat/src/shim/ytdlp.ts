// Nuclear's `Ytdlp` host (plugin-sdk `types/ytdlp.ts`, AGPL-3.0) over `raygun.youtube`: YouTube's
// own web API, answered by Raygun (raygun-ext-host::youtube), with yt-dlp behind it on desktop.
// Nuclear's answers have no room for a stream's headers, a video's artist or what a lookup is
// for, so those are kept here, by video and by address, for the provider wrappers (providers.ts).
import type { YouTubeStream } from '@raygun/sdk';

import type { YtdlpPlaylistInfo, YtdlpSearchResult, YtdlpStreamInfo } from './nuclear-types';

/** What a stream lookup is for: the quality to aim for, and whether Raygun decodes it. */
export type StreamWish = { targetKbps?: number | null; decodable?: boolean };

/** Entries kept in each table; the oldest go first. */
const KEPT = 500;

const keep = <V>(table: Map<string, V>, key: string, value: V) => {
  table.delete(key);
  table.set(key, value);
  if (table.size > KEPT) table.delete(table.keys().next().value as string);
};

/** Who made each video a search or a playlist found: what candidates are matched by. */
const channels = new Map<string, string>();
/** What the next stream lookup of each video is for. */
const wishes = new Map<string, StreamWish>();
/** The streams handed out, by address: their headers and expiry. */
const streams = new Map<string, YouTubeStream>();
/** Who made each playlist read. */
const authors = new Map<string, string>();

/** The 11-character video id an id or a YouTube address names. */
export const videoIdOf = (text: string | null | undefined): string | null => {
  const trimmed = (text ?? '').trim();
  if (/^[\w-]{11}$/.test(trimmed)) return trimmed;
  return (
    /(?:[?&]v=|youtu\.be\/|\/(?:shorts|live|embed|v)\/)([\w-]{11})(?![\w-])/.exec(trimmed)?.[1] ??
    null
  );
};

/** The channel (a song's artists) a search or playlist named for a video. */
export const channelOf = (video: string | null | undefined): string | null => {
  const id = videoIdOf(video);
  return id ? (channels.get(id) ?? null) : null;
};

/** Says what the next stream lookup of `video` is for. */
export const wishFor = (video: string | null | undefined, wish: StreamWish) => {
  const id = videoIdOf(video);
  if (id) keep(wishes, id, wish);
};

/** A stream this host handed out, by its address. */
export const streamAt = (url: string): YouTubeStream | null => streams.get(url) ?? null;

/** Who made a playlist read through `getPlaylist`. */
export const playlistAuthorOf = (id: string | null | undefined) =>
  id ? (authors.get(id) ?? null) : null;

const seconds = (ms: number | null) => (ms == null ? null : ms / 1000);

export const createYtdlp = () => ({
  async search(query: string, maxResults?: number): Promise<YtdlpSearchResult[]> {
    const found = await raygun.youtube.search(String(query), maxResults);
    return found.map((video) => {
      if (video.channel) keep(channels, video.id, video.channel);
      return {
        id: video.id,
        title: video.title,
        duration: seconds(video.durationMs),
        thumbnail: video.thumbnailUrl,
        channel: video.channel,
      };
    });
  },

  async getStream(url: string): Promise<YtdlpStreamInfo> {
    const id = videoIdOf(url);
    const stream = await raygun.youtube.stream(String(url), (id && wishes.get(id)) || {});
    keep(streams, stream.url, stream);
    return {
      stream_url: stream.url,
      duration: seconds(stream.durationMs),
      title: stream.title,
      container: stream.container,
      codec: stream.codec,
      album: stream.album,
      artists: stream.artists,
      album_artists: [],
      upload_date: stream.uploadDate,
    };
  },

  async getPlaylist(url: string): Promise<YtdlpPlaylistInfo> {
    const playlist = await raygun.youtube.playlist(String(url));
    if (playlist.author) keep(authors, playlist.id, playlist.author);
    return {
      id: playlist.id,
      title: playlist.title,
      entries: playlist.videos.map((video) => {
        if (video.channel) keep(channels, video.id, video.channel);
        return {
          id: video.id,
          title: video.title,
          duration: seconds(video.durationMs),
          thumbnails: video.thumbnails,
          channel: video.channel,
        };
      }),
    };
  },
});
