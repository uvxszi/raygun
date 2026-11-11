import type { SongEdit, SoundPrefs } from '@raygun/bindings';

/** Sound settings with every field present (Rust's `SoundPrefs` fills missing ones the same way). */
export type FullSound = Required<SoundPrefs>;
export type FullSongEdit = Required<SongEdit>;

export const SOUND_DEFAULTS: FullSound = {
  eqEnabled: false,
  eqPreset: 'flat',
  eqBands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0],
  preamp: 0,
  bassBoost: 0,
  reverb: 'off',
  balance: 0,
  mono: false,
  normalize: false,
  normalizeTarget: -14,
  // Rust's CROSSFADE_DEFAULT_MS.
  crossfadeMs: 2000,
};

export const withSoundDefaults = (sound?: SoundPrefs | null): FullSound => ({
  eqEnabled: sound?.eqEnabled ?? SOUND_DEFAULTS.eqEnabled,
  eqPreset: sound?.eqPreset ?? SOUND_DEFAULTS.eqPreset,
  eqBands:
    sound?.eqBands?.length === SOUND_DEFAULTS.eqBands.length
      ? sound.eqBands
      : SOUND_DEFAULTS.eqBands,
  preamp: sound?.preamp ?? SOUND_DEFAULTS.preamp,
  bassBoost: sound?.bassBoost ?? SOUND_DEFAULTS.bassBoost,
  reverb: sound?.reverb ?? SOUND_DEFAULTS.reverb,
  balance: sound?.balance ?? SOUND_DEFAULTS.balance,
  mono: sound?.mono ?? SOUND_DEFAULTS.mono,
  normalize: sound?.normalize ?? SOUND_DEFAULTS.normalize,
  normalizeTarget: sound?.normalizeTarget ?? SOUND_DEFAULTS.normalizeTarget,
  crossfadeMs: sound?.crossfadeMs ?? SOUND_DEFAULTS.crossfadeMs,
});

export const withEditDefaults = (edit?: SongEdit | null): FullSongEdit => ({
  startMs: edit?.startMs ?? 0,
  endMs: edit?.endMs ?? null,
  gain: edit?.gain ?? 0,
  lyricsOffsetMs: edit?.lyricsOffsetMs ?? 0,
  lyrics: edit?.lyrics ?? null,
});
