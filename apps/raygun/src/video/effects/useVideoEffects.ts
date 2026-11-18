import type { PictureEffects, SmartAudio } from '@raygun/bindings';
import { useEffect, useMemo, useRef, useState } from 'react';

import {
  NO_EFFECTS,
  sameEffects,
  without,
  withoutWatermark,
  withAudioDefaults,
  withEffectDefaults,
} from './effectParams';
import type { Audio, Effects, HeavyKind } from './effectParams';
import type { Watermark } from './watermarkMath';

/**
 * Where the watermark remover stands: learning the overlay from the frames, removing it (a mask
 * found), looking for one (Find it), or none found.
 */
export type WatermarkStatus = 'learning' | 'ready' | 'finding' | 'notFound' | 'unsure';

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
  /** The Smooth Motion chip is installed and on, its shaders read: smooth motion can play. */
  smoothMotion?: boolean;
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
  /** Smooth motion's chip is there: the Effects panel offers it. */
  smoothMotionAvailable: boolean;
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
  watermarkStatus: WatermarkStatus;
  setWatermarkStatus: (status: WatermarkStatus) => void;
  /** Scene changes Find it has seen so far. */
  watermarkCuts: number;
  setWatermarkCuts: (cuts: number) => void;
  /** The box is being drawn over the picture. */
  drawingWatermark: boolean;
  setDrawingWatermark: (drawing: boolean) => void;
  /** Changes the title's watermark remover (kept with its picture effects). */
  setWatermark: (watermark: Watermark) => void;
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
  smoothMotion = false,
}: Options): VideoEffects => {
  // A watermark is a title's own, never the settings'.
  const savedPicture = withoutWatermark(withEffectDefaults(defaults.pictureEffects));
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
  const [watermarkStatus, setWatermarkStatus] = useState<WatermarkStatus>('learning');
  const [watermarkCuts, setWatermarkCuts] = useState(0);
  const [drawingWatermark, setDrawingWatermark] = useState(false);
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
  // Without its chip, smooth motion waits (the title keeps asking for it).
  const playable = smoothMotion ? picture : { ...picture, smoothMotion: false };
  const appliedKey = JSON.stringify(
    unavailable ? NO_EFFECTS : dropped.length ? without(playable, dropped) : playable,
  );
  const applied = useMemo(() => JSON.parse(appliedKey) as Effects, [appliedKey]);
  const audioKey = JSON.stringify(ownAudio ?? audioDefaults);
  const audio = useMemo(() => JSON.parse(audioKey) as Audio, [audioKey]);
  const forget = (key: 'pictureEffects' | 'smartAudio') => remember({ [key]: null });
  /** The settings' effects from now on, with the title's watermark box kept when it has one. */
  const keepOnlyWatermark = (base: Effects) => {
    const { watermark } = picture;
    if (watermark.width > 0 && watermark.height > 0) {
      const next = { ...base, watermark };
      setOwn(next);
      remember({ pictureEffects: next });
    } else {
      setOwn(null);
      forget('pictureEffects');
    }
  };
  const cancelPending = () => {
    if (pending.current) window.clearTimeout(pending.current.timer);
    pending.current = null;
  };

  return {
    picture,
    applied,
    ownPicture: own !== null && !sameEffects(withoutWatermark(own), pictureDefaults),
    audio,
    ownAudio: ownAudio !== null && audioKey !== JSON.stringify(audioDefaults),
    dropped,
    unavailable,
    smoothMotionAvailable: smoothMotion,
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
      keepOnlyWatermark(pictureDefaults);
    },
    makePictureDefault: () => {
      cancelPending();
      const shared = withoutWatermark(picture);
      savePrefs({ pictureEffects: shared });
      setMadeDefault({ over: settingsKey, picture: shared, audio: made?.audio });
      keepOnlyWatermark(shared);
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
    watermarkStatus,
    setWatermarkStatus,
    watermarkCuts,
    setWatermarkCuts,
    drawingWatermark,
    setDrawingWatermark,
    setWatermark: (watermark) => {
      cancelPending();
      const next = { ...picture, watermark };
      setOwn(next);
      remember({ pictureEffects: next });
    },
  };
};
