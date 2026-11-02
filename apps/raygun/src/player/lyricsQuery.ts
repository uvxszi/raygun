import { commands } from '@raygun/bindings';
import type { Lyrics, QueueItem, TrackQuery } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';

import { unwrapResult } from '../lib/commandResult';

/** The metadata lyrics are looked up by: a lookup's own query, else the item's fields. */
export const trackQueryFor = (item: QueueItem): TrackQuery =>
  item.source.kind === 'lookup'
    ? {
        title: item.source.title,
        artist: item.source.artist,
        album: item.source.album,
        durationMs: item.source.durationMs,
      }
    : { title: item.title, artist: item.artist, album: item.album, durationMs: item.durationMs };

/** A song's lyrics, with its own edits and offset applied by Rust (M46.12). */
export const useLyricsFor = (query: TrackQuery) =>
  useQuery<Lyrics | null>({
    queryKey: ['lyrics', query.title, query.artist, query.album, query.durationMs],
    queryFn: () => unwrapResult(commands.lyricsGet(query)),
    staleTime: Infinity,
    // Providers already wait out slow services; one more try is enough before showing the error.
    retry: 1,
  });

/** The line playing at `positionMs`: lines show `offsetMs` later than their own time. */
export const currentLine = (lines: { timeMs: number }[], positionMs: number, offsetMs: number) => {
  let current = -1;
  for (const [index, line] of lines.entries()) {
    if (line.timeMs + offsetMs <= positionMs) current = index;
    else break;
  }
  return current;
};
