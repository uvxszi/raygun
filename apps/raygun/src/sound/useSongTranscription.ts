import { commands, events } from '@raygun/bindings';
import type { Captions, TrackQuery } from '@raygun/bindings';
import { useMutation, useQueryClient } from '@tanstack/react-query';
import { useEffect } from 'react';

import { unwrapResult } from '../lib/commandResult';
import { trackKey } from '../music/trackKey';
import { useCaptions } from '../player/spoken';

/** Where hearing a song's words stands. */
export type TranscriptionStage = 'idle' | 'queued' | 'preparing' | 'hearing';

/** The captions key Rust hears a song under (`captions::song_key`). */
export const songCaptionsKey = (song: TrackQuery) => `song:${trackKey(song.title, song.artist)}`;

export const transcriptionStage = (captions: Captions | undefined): TranscriptionStage => {
  if (!captions) return 'idle';
  if (captions.running) return captions.preparing ? 'preparing' : 'hearing';
  if (captions.queued !== null && captions.queued !== undefined) return 'queued';
  return 'idle';
};

/** How much of the recording was heard, 0–100, when its length is known. */
export const heardPercent = (captions: Captions | undefined) => {
  const duration = captions?.durationMs;
  if (!captions || !duration) return null;
  const heard = captions.covered.reduce(
    (sum, [start, end]) => sum + Math.max(0, Math.min(end, duration) - start),
    0,
  );
  return Math.min(100, Math.round((heard / duration) * 100));
};

/**
 * Hearing one song's words (M46.36): where it stands in Rust's song queue, and starting or
 * stopping it. Starting waits for the new state, so the button never flashes back to idle.
 */
export const useSongTranscription = (song: TrackQuery) => {
  const client = useQueryClient();
  const key = songCaptionsKey(song);
  const captions = useCaptions(key);
  const refresh = () => client.invalidateQueries({ queryKey: ['captions', key] });
  const start = useMutation({
    mutationFn: () => unwrapResult(commands.lyricsTranscribe(song)),
    onSettled: refresh,
  });
  const stop = useMutation({
    mutationFn: () => commands.captionsStop(key),
    onSettled: refresh,
  });
  const stage = transcriptionStage(captions.data);
  return {
    key,
    captions: captions.data,
    stage,
    busy: start.isPending || stage !== 'idle',
    percent: heardPercent(captions.data),
    error: start.error?.message ?? captions.data?.error ?? null,
    canTranscribe: captions.data?.canTranscribe ?? true,
    start: () => start.mutate(),
    stop: () => stop.mutate(),
  };
};

/**
 * Shows a song's transcribed words wherever lyrics are shown once Rust has kept them as the
 * song's lyrics, whichever screen asked (a list's Transcribe lyrics, the lyrics menu).
 */
export const useTranscribedLyricsSync = () => {
  const client = useQueryClient();
  useEffect(() => {
    const done = events.lyricsTranscribedEvent.listen(({ payload }) => {
      void client.invalidateQueries({ queryKey: ['lyrics'] });
      void client.invalidateQueries({ queryKey: ['songEdit'] });
      void client.invalidateQueries({ queryKey: ['lyricsOptions'] });
      void client.invalidateQueries({ queryKey: ['captions', payload] });
    });
    return () => {
      void done.then((stop) => stop());
    };
  }, [client]);
};
