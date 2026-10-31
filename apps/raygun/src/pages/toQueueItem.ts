import type { QueueItemInput } from '@raygun/bindings';

const YOUTUBE_ID = /^[A-Za-z0-9_-]{11}$/;

/** Extracts a video ID from YouTube / YouTube Music links (or a bare ID), else null. */
export const parseYouTubeId = (input: string): string | null => {
  const trimmed = input.trim();
  if (YOUTUBE_ID.test(trimmed)) {
    return trimmed;
  }
  let url: URL;
  try {
    url = new URL(trimmed);
  } catch {
    return null;
  }
  const host = url.hostname.replace(/^(www|m|music)\./, '');
  const candidate =
    host === 'youtu.be'
      ? url.pathname.slice(1)
      : host === 'youtube.com'
        ? (url.searchParams.get('v') ??
          url.pathname.match(/^\/(?:shorts|embed|live)\/([^/]+)/)?.[1] ??
          null)
        : null;
  return candidate && YOUTUBE_ID.test(candidate) ? candidate : null;
};

/**
 * Turns what the user typed into a queue item: YouTube links play in YouTube's player, other
 * http(s) URLs stream, anything else is a file path.
 */
export const toQueueItem = (location: string, referer: string): QueueItemInput | null => {
  const trimmed = location.trim();
  if (!trimmed) {
    return null;
  }
  const videoId = parseYouTubeId(trimmed);
  if (videoId) {
    return {
      title: `YouTube · ${videoId}`,
      artist: null,
      album: null,
      durationMs: null,
      artworkUrl: `https://i.ytimg.com/vi/${videoId}/hqdefault.jpg`,
      mediaKind: 'video',
      source: { kind: 'youTube', videoId },
    };
  }
  const isUrl = /^https?:\/\//i.test(trimmed);
  const lastSegment = trimmed.split(/[?#]/)[0]?.split(/[\\/]/).filter(Boolean).pop() ?? trimmed;
  let title = lastSegment;
  try {
    title = decodeURIComponent(lastSegment);
  } catch {
    // Keep the raw segment when it isn't valid percent-encoding.
  }
  const headers: Record<string, string> = referer.trim() ? { Referer: referer.trim() } : {};
  return {
    title,
    artist: null,
    album: null,
    durationMs: null,
    artworkUrl: null,
    mediaKind: 'audio',
    source: isUrl ? { kind: 'url', url: trimmed, headers } : { kind: 'localFile', path: trimmed },
  };
};
