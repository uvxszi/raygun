import type { PictureLook } from '@raygun/bindings';

import type { AdjustKey, Audio, HeavyKind } from './effectParams';
import type { MotionModel } from './motionMath';

/** The words of each adjustment (`videoPlayer.*`, shared with Android's player). */
export const ADJUSTMENT_LABEL: Record<AdjustKey, string> = {
  brightness: 'videoPlayer.effectBrightness',
  contrast: 'videoPlayer.effectContrast',
  saturation: 'videoPlayer.effectSaturation',
  gamma: 'videoPlayer.effectGamma',
  hue: 'videoPlayer.effectHue',
  sharpen: 'videoPlayer.effectSharpen',
  deflicker: 'videoPlayer.effectDeflicker',
  denoise: 'videoPlayer.effectDenoise',
};

/** The words of what drops when frames go missing. */
export const HEAVY_LABEL: Record<HeavyKind, string> = {
  smoothMotion: 'videoPlayer.effectSmoothMotion',
  denoise: ADJUSTMENT_LABEL.denoise,
  deflicker: ADJUSTMENT_LABEL.deflicker,
  sharpen: ADJUSTMENT_LABEL.sharpen,
};

export const LOOK_LABEL: Record<PictureLook, string> = {
  none: 'videoPlayer.effectsLookNone',
  warm: 'videoPlayer.effectsLookWarm',
  cool: 'videoPlayer.effectsLookCool',
  vivid: 'videoPlayer.effectsLookVivid',
  grayscale: 'videoPlayer.effectsLookGrayscale',
  sepia: 'videoPlayer.effectsLookSepia',
};

/** Smart audio's switches: their words and the line under them. */
export const SMART_AUDIO: { key: keyof Audio; label: string; hint: string }[] = [
  { key: 'dialogue', label: 'videoPlayer.smartDialogue', hint: 'videoPlayer.smartDialogueHint' },
  { key: 'night', label: 'videoPlayer.smartNight', hint: 'videoPlayer.smartNightHint' },
  { key: 'normalize', label: 'videoPlayer.smartNormalize', hint: 'videoPlayer.smartNormalizeHint' },
  { key: 'leveling', label: 'videoPlayer.smartLeveling', hint: 'videoPlayer.smartLevelingHint' },
];

/** Smooth motion's models: their words and what each does. */
export const MOTION_MODEL_LABEL: Record<MotionModel, string> = {
  blend: 'videoPlayer.motionBlend',
  balanced: 'videoPlayer.motionBalanced',
  sharp: 'videoPlayer.motionSharp',
};

export const MOTION_MODEL_HINT: Record<MotionModel, string> = {
  blend: 'videoPlayer.motionBlendHint',
  balanced: 'videoPlayer.motionBalancedHint',
  sharp: 'videoPlayer.motionSharpHint',
};
