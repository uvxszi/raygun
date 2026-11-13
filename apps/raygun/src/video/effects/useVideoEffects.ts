import type { PictureEffects, SmartAudio } from '@raygun/bindings';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
  NO_EFFECTS,
  sameEffects,
  without,
  withAudioDefaults,
  withEffectDefaults,
} from './effectParams';
import type { Audio, Effects, HeavyKind } from './effectParams';

/** A slider's last value is kept for the title this long after it stops moving. */
const KEEP_AFTER_MS = 400;

type Options = {
  /** What the title remembers (`pictureEffects`, `smartAudio` in its memory). */
  titleState: { pictureEffects?: PictureEffects | null; smartAudio?: SmartAudio | null };
  /** Settings → Video player → Effects. */
  defaults: { pictureEffects?: PictureEffects; smartAudio?: SmartAudio };
  /** Keeps a change for the title (`state`). */
  remember: (change: Record<string, unknown>) => void;
  /** Saves a change to the player's settings (`prefs`). */
  savePrefs: (change: Record<string, unknown>) => void;
};

export type VideoEffects = {
  /** The picture's effects for this video: the title's own, else the settings'. */
  picture: Effects;
  /** What plays: the picture's effects without any dropped for speed. */
  applied: Effects;
  /** The title has effects of its own, other than the settings'. */
  ownPicture: boolean;
  audio: Audio;
  ownAudio: boolean;
  /** Effects turned off because frames went missing. */
  dropped: HeavyKind[];
  /** WebGL 2 isn't there: no picture effects. */
  unavailable: boolean;
  /** Live, kept for the title once it settles. */
  setPicture: (next: Effects) => void;
  /** No effects for this title (the settings' again, when they're none). */
  resetPicture: () => void;
  useDefaultPicture: () => void;
  makePictureDefault: () => void;
  setAudio: (next: Audio) => void;
  makeAudioDefault: () => void;
  drop: (kind: HeavyKind) => void;
  markUnavailable: () => void;
};

/**
 * The desktop player's effects for one video: the title's own (kept with its memory) or
 * Settings → Video player → Effects, and what drops out while frames go missing. Android's player
 * keeps them the same way (`PlayerEffects.kt`).
 */
export const useVideoEffects = ({
  titleState,
  defaults,
  remember,
  savePrefs,
}: Options): VideoEffects => {
  const savedPicture = withEffectDefaults(defaults.pictureEffects);
  const savedAudio = withAudioDefaults(defaults.smartAudio);
  // Made default here: shown as the settings' until Rust's update with them comes (no flicker
  // back to the settings before).
  const [madeDefault, setMadeDefault] = useState<{
    over: string;
    picture?: Effects;
    audio?: Audio;
  } | null>(null);
  const settingsKey = JSON.stringify([savedPicture, savedAudio]);
  const made = madeDefault?.over === settingsKey ? madeDefault : null;
  const pictureDefaults = made?.picture ?? savedPicture;
  const audioDefaults = made?.audio ?? savedAudio;
  const [own, setOwn] = useState<Effects | null>(() =>
    titleState.pictureEffects ? withEffectDefaults(titleState.pictureEffects) : null,
  );
  const [ownAudio, setOwnAudio] = useState<Audio | null>(() =>
    titleState.smartAudio ? withAudioDefaults(titleState.smartAudio) : null,
  );
  const [dropped, setDropped] = useState<HeavyKind[]>([]);
  const [unavailable, setUnavailable] = useState(false);
  const pending = useRef<{ timer: number; change: Effects } | null>(null);
  const rememberRef = useRef(remember);
  useEffect(() => {
    rememberRef.current = remember;
  }, [remember]);

  // A change still waiting is kept when the video closes.
  useEffect(
    () => () => {
      const waiting = pending.current;
      if (!waiting) return;
      window.clearTimeout(waiting.timer);
      rememberRef.current({ pictureEffects: waiting.change });
    },
    [],
  );

  // The same objects while the values stay (the layer and the audio graph follow them).
  const pictureKey = JSON.stringify(own ?? pictureDefaults);
  const picture = useMemo(() => JSON.parse(pictureKey) as Effects, [pictureKey]);
  const appliedKey = JSON.stringify(
    unavailable ? NO_EFFECTS : dropped.length ? without(picture, dropped) : picture,
  );
  const applied = useMemo(() => JSON.parse(appliedKey) as Effects, [appliedKey]);
  const audioKey = JSON.stringify(ownAudio ?? audioDefaults);
  const audio = useMemo(() => JSON.parse(audioKey) as Audio, [audioKey]);
  const forget = (key: 'pictureEffects' | 'smartAudio') => remember({ [key]: null });
  const cancelPending = () => {
    if (pending.current) window.clearTimeout(pending.current.timer);
    pending.current = null;
  };

  return {
    picture,
    applied,
    ownPicture: own !== null && !sameEffects(own, pictureDefaults),
    audio,
    ownAudio: ownAudio !== null && audioKey !== JSON.stringify(audioDefaults),
    dropped,
    unavailable,
    setPicture: (next) => {
      setOwn(next);
      // Asked for again: what dropped for speed gets another try.
      setDropped((list) => list.filter((kind) => next[kind] === 0 || next[kind] === picture[kind]));
      cancelPending();
      pending.current = {
        change: next,
        timer: window.setTimeout(() => {
          pending.current = null;
          remember({ pictureEffects: next });
        }, KEEP_AFTER_MS),
      };
    },
    resetPicture: () => {
      cancelPending();
      setDropped([]);
      const none = withEffectDefaults(null);
      if (sameEffects(pictureDefaults, none)) {
        setOwn(null);
        forget('pictureEffects');
      } else {
        setOwn(none);
        remember({ pictureEffects: none });
      }
    },
    useDefaultPicture: () => {
      cancelPending();
      setOwn(null);
      forget('pictureEffects');
    },
    makePictureDefault: () => {
      cancelPending();
      savePrefs({ pictureEffects: picture });
      setMadeDefault({ over: settingsKey, picture, audio: made?.audio });
      setOwn(null);
      forget('pictureEffects');
    },
    setAudio: (next) => {
      setOwnAudio(next);
      remember({ smartAudio: next });
    },
    makeAudioDefault: () => {
      savePrefs({ smartAudio: audio });
      setMadeDefault({ over: settingsKey, picture: made?.picture, audio });
      setOwnAudio(null);
      forget('smartAudio');
    },
    drop: (kind) => setDropped((list) => (list.includes(kind) ? list : [...list, kind])),
    markUnavailable: () => setUnavailable(true),
  };
};
