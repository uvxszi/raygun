// The first ten presets' shapes come from RiPlay (GPL-3.0),
// `extensions/equalizer/InternalEqualizerScreen.kt` (`applyPreset`): ten points from 0 to 1, where
// 0.5 is flat and each step of 0.1 is 1.6 dB. Here they sit on Raygun's fixed octave bands, in
// tenths of a dB. The rest are Raygun's own.

/** An equalizer preset: its id (stored in `SoundPrefs.eqPreset`) and its ten band gains. */
export type EqPreset = { id: string; bands: number[] };

/** RiPlay's 0–1 shape in tenths of a dB: (v − 0.5) × 16 dB. */
const fromShape = (shape: number[]) => shape.map((value) => Math.round((value - 0.5) * 160));

export const EQ_PRESETS: EqPreset[] = [
  { id: 'flat', bands: [0, 0, 0, 0, 0, 0, 0, 0, 0, 0] },
  { id: 'bassBoost', bands: fromShape([1.0, 0.9, 0.8, 0.7, 0.6, 0.5, 0.5, 0.5, 0.5, 0.5]) },
  { id: 'rock', bands: fromShape([0.8, 0.7, 0.5, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1.0]) },
  { id: 'pop', bands: fromShape([0.8, 0.7, 0.6, 0.6, 0.7, 0.7, 0.6, 0.5, 0.5, 0.5]) },
  { id: 'jazz', bands: fromShape([0.7, 0.7, 0.6, 0.6, 0.7, 0.7, 0.6, 0.5, 0.5, 0.5]) },
  { id: 'classical', bands: fromShape([0.6, 0.6, 0.5, 0.5, 0.5, 0.5, 0.6, 0.7, 0.8, 0.9]) },
  { id: 'electronic', bands: fromShape([1.0, 0.9, 0.7, 0.5, 0.4, 0.5, 0.7, 0.9, 1.0, 1.0]) },
  { id: 'dance', bands: fromShape([0.9, 0.8, 0.6, 0.5, 0.5, 0.7, 0.8, 0.9, 1.0, 1.0]) },
  { id: 'vocal', bands: fromShape([0.4, 0.5, 0.6, 0.8, 0.9, 0.9, 0.8, 0.7, 0.6, 0.5]) },
  { id: 'acoustic', bands: fromShape([0.6, 0.6, 0.7, 0.7, 0.7, 0.7, 0.7, 0.6, 0.6, 0.6]) },
  { id: 'hipHop', bands: [50, 40, 10, 30, -10, -10, 10, -10, 20, 30] },
  { id: 'loudness', bands: [50, 40, 10, 0, -10, 0, 0, 10, 40, 50] },
  { id: 'trebleBoost', bands: [0, 0, 0, 0, 0, 10, 30, 50, 70, 80] },
  { id: 'trebleReducer', bands: [0, 0, 0, 0, 0, -10, -30, -50, -70, -80] },
  { id: 'bassReducer', bands: [-80, -64, -48, -32, -16, 0, 0, 0, 0, 0] },
  { id: 'smallSpeakers', bands: [-60, -40, 10, 40, 30, 10, 0, 10, 20, 20] },
  { id: 'spokenWord', bands: [-40, -30, -10, 10, 30, 40, 40, 30, 10, -20] },
];

/** The preset whose bands these are, or "custom". */
export const presetFor = (bands: number[]) =>
  EQ_PRESETS.find((preset) => preset.bands.every((gain, index) => gain === bands[index]))?.id ??
  'custom';
