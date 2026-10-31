import type { QueueItemInput } from '@raygun/bindings';

/** Turns what the user typed into a queue item: http(s) URLs stream, anything else is a file path. */
export const toQueueItem = (location: string, referer: string): QueueItemInput | null => {
  const trimmed = location.trim();
  if (!trimmed) {
    return null;
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
    mediaKind: 'audio',
    source: isUrl ? { kind: 'url', url: trimmed, headers } : { kind: 'localFile', path: trimmed },
  };
};
