import { commands, events } from '@raygun/bindings';
import type { Caption, QueueItem, SpokenItem } from '@raygun/bindings';
import { useMutation, useQuery, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { useSettings } from '../settings/useSettings';

const SPOKEN = 'spokenItem';
const CAPTIONS = 'captions';

/**
 * Whether a queue item is a podcast episode or a book's chapter, and whose (M46.32); `null` for
 * songs, so the player shows captions and the show instead of lyrics and artists.
 */
export const useSpokenItem = (item: QueueItem | undefined): SpokenItem | null => {
  const source = item?.source;
  const lookable = source?.kind === 'url' || source?.kind === 'localFile';
  const query = useQuery({
    queryKey: [SPOKEN, source],
    queryFn: () => (source ? unwrapResult(commands.spokenItem(source)) : Promise.resolve(null)),
    enabled: lookable,
    staleTime: Infinity,
  });
  return lookable ? (query.data ?? null) : null;
};

/**
 * An episode's captions, read again whenever Rust says they changed, and when transcription is
 * set up (a setting changes or a model finishes downloading), which changes what the card offers.
 */
export const useCaptions = (episodeId: string) => {
  const client = useQueryClient();
  const prefs = useSettings().data?.transcription;
  useEffect(() => {
    const changed = events.captionsEvent.listen(({ payload }) => {
      if (payload === episodeId) {
        void client.invalidateQueries({ queryKey: [CAPTIONS, episodeId] });
      }
    });
    const models = events.aiModelEvent.listen(({ payload }) => {
      if (payload.state !== 'downloading') {
        void client.invalidateQueries({ queryKey: [CAPTIONS, episodeId] });
      }
    });
    return () => {
      void changed.then((stop) => stop());
      void models.then((stop) => stop());
    };
  }, [client, episodeId]);
  return useQuery({
    queryKey: [CAPTIONS, episodeId, prefs],
    queryFn: () => unwrapResult(commands.captions(episodeId)),
    // While the setup changes, the same episode's lines stay; another episode's never show.
    placeholderData: (previous, previousQuery) =>
      previousQuery?.queryKey[1] === episodeId ? previous : undefined,
  });
};

export const useTranscribe = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: ({ episodeId, fromMs }: { episodeId: string; fromMs: number }) =>
      unwrapResult(commands.captionsTranscribe(episodeId, Math.max(0, Math.round(fromMs)))),
    onSettled: (_data, _error, { episodeId }) =>
      client.invalidateQueries({ queryKey: [CAPTIONS, episodeId] }),
  });
};

export const useStopCaptions = () => {
  const client = useQueryClient();
  return useMutation({
    mutationFn: (episodeId: string) => commands.captionsStop(episodeId),
    onSettled: (_data, _error, episodeId) =>
      client.invalidateQueries({ queryKey: [CAPTIONS, episodeId] }),
  });
};

/** The line being said: the last one started by `positionMs`. */
export const currentCue = (cues: Caption[], positionMs: number) => {
  let current = -1;
  for (const [index, cue] of cues.entries()) {
    if (cue.startMs <= positionMs) current = index;
    else break;
  }
  return current;
};
