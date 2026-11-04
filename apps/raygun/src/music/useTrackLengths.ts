import { commands, events } from '@raygun/bindings';
import type { TrackDuration } from '@raygun/bindings';
import { useQuery } from '@tanstack/react-query';
import { useEffect, useMemo, useState } from 'react';

import { unwrapResult } from '../lib/commandResult';
import type { TableTrack } from './tableView';

/** Lengths learned this session, by Rust's track key (filled by answers and events). */
const learned = new Map<string, number>();

/** What a row needs for its length to be filled. */
export type LengthRow = Pick<TableTrack, 'rowKey' | 'title' | 'artist' | 'album' | 'durationMs'>;

/**
 * Fills the lengths songs lack (M46.26): Rust answers with what it knows and looks the rest up in
 * the background, sending `TrackDurationsEvent`s as lengths arrive. Rows keep a length they have.
 */
export const useTrackLengths = <Row extends LengthRow>(tracks: Row[]): Row[] => {
  const missing = useMemo(() => tracks.filter((track) => !track.durationMs), [tracks]);
  const asked = useQuery({
    queryKey: [
      'trackDurations',
      missing.map((track) => `${track.artist ?? ''}|${track.title}`).join('\n'),
    ],
    queryFn: () =>
      unwrapResult(
        commands.trackDurations(
          missing.map((track) => ({
            title: track.title,
            artist: track.artist,
            album: track.album,
            durationMs: null,
          })),
        ),
      ),
    enabled: missing.length > 0,
    staleTime: 10 * 60_000,
  });
  // A copy of what's learned, so lengths that arrive show.
  const [known, setKnown] = useState(() => new Map(learned));
  useEffect(() => {
    const unlisten = events.trackDurationsEvent.listen((event) => {
      let changed = false;
      for (const found of event.payload as TrackDuration[]) {
        if (found.durationMs && learned.get(found.trackKey) !== found.durationMs) {
          learned.set(found.trackKey, found.durationMs);
          changed = true;
        }
      }
      if (changed) setKnown(new Map(learned));
    });
    return () => {
      void unlisten.then((stop) => stop());
    };
  }, []);

  return useMemo(() => {
    if (!asked.data || missing.length === 0) return tracks;
    // Rust answers in the order asked, naming each song's key.
    const keyOf = new Map<string, string>();
    asked.data.forEach((answer, index) => {
      const row = missing[index];
      if (!row) return;
      keyOf.set(row.rowKey, answer.trackKey);
      if (answer.durationMs) learned.set(answer.trackKey, answer.durationMs);
    });
    return tracks.map((track) => {
      if (track.durationMs) return track;
      const key = keyOf.get(track.rowKey);
      const durationMs = key ? (known.get(key) ?? learned.get(key)) : undefined;
      return durationMs ? { ...track, durationMs } : track;
    });
  }, [tracks, missing, asked.data, known]);
};
