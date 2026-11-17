import type { VideoSound } from '@raygun/bindings';
import { useEffect, useRef, useState } from 'react';

import type { MusicEq } from '../protocol';

/**
 * A title's sound in the video player (`raygun_model::VideoSound`, `videoSound` in its memory):
 * pitch in tenths of a semitone (the tempo is the title's speed), and an equalizer of its own or
 * the music player's (`musicEq`). Android's player keeps it the same way.
 */

export type FullVideoSound = Required<VideoSound>;

export const FLAT_BANDS = [0, 0, 0, 0, 0, 0, 0, 0, 0, 0];

export const NO_VIDEO_SOUND: FullVideoSound = {
  pitch: 0,
  ownEq: false,
  eqEnabled: false,
  eqPreset: 'flat',
  eqBands: FLAT_BANDS,
  preamp: 0,
};

export const withVideoSoundDefaults = (sound?: VideoSound | null): FullVideoSound => ({
  ...NO_VIDEO_SOUND,
  ...sound,
  eqBands: sound?.eqBands?.length === FLAT_BANDS.length ? sound.eqBands : FLAT_BANDS,
});

/** The equalizer that applies: what the graph is given. */
export type AppliedEq = { enabled: boolean; bands: number[]; preamp: number };

/** The title's own equalizer, else the music player's. */
export const appliedEq = (sound: FullVideoSound, music: MusicEq | null | undefined): AppliedEq => {
  const from = sound.ownEq ? sound : music;
  const bands = from?.eqBands?.length === FLAT_BANDS.length ? from.eqBands : FLAT_BANDS;
  return { enabled: from?.eqEnabled ?? false, bands, preamp: from?.preamp ?? 0 };
};

/** The sound graph's part for these: pitch, and the equalizer (off: none). */
export type SoundTuning = { pitch: number; eq: AppliedEq };

/** Nothing changes the sound: no graph is needed for it. */
export const isTuningOff = (tuning: SoundTuning) =>
  tuning.pitch === 0 &&
  (!tuning.eq.enabled || (tuning.eq.preamp === 0 && tuning.eq.bands.every((gain) => gain === 0)));

/** A slider's last value is kept for the title this long after it stops moving. */
const KEEP_AFTER_MS = 400;

/** The title's sound: live as it changes, kept for the title once a slider settles. */
export const useVideoSound = (
  saved: VideoSound | null | undefined,
  remember: (change: Record<string, unknown>) => void,
) => {
  const [sound, setSound] = useState<FullVideoSound>(() => withVideoSoundDefaults(saved));
  const pending = useRef<{ timer: number; change: FullVideoSound } | null>(null);
  const rememberRef = useRef(remember);
  useEffect(() => {
    rememberRef.current = remember;
  }, [remember]);
  useEffect(
    () => () => {
      const waiting = pending.current;
      if (!waiting) return;
      window.clearTimeout(waiting.timer);
      rememberRef.current({ videoSound: waiting.change });
    },
    [],
  );
  const cancel = () => {
    if (pending.current) window.clearTimeout(pending.current.timer);
    pending.current = null;
  };
  return {
    sound,
    setSound: (next: FullVideoSound) => {
      setSound(next);
      cancel();
      pending.current = {
        change: next,
        timer: window.setTimeout(() => {
          pending.current = null;
          rememberRef.current({ videoSound: next });
        }, KEEP_AFTER_MS),
      };
    },
    /** Pitch 0 and the music player's equalizer again. */
    resetSound: () => {
      cancel();
      setSound(NO_VIDEO_SOUND);
      rememberRef.current({ videoSound: null });
    },
  };
};
