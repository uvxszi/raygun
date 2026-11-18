import type { PictureLook } from '@raygun/bindings';
import { AudioWaveform, ChartNoAxesColumn, Moon, Speech } from 'lucide-react';
import type { LucideIcon } from 'lucide-react';

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

type SmartAudioLabel = {
  key: keyof Audio;
  label: string;
  hint: string;
  /** A word or two for the Audio panel's small switches. */
  short: string;
  icon: LucideIcon;
};

/** Smart audio's switches: their words, the line under them, and the Audio panel's icon. */
export const SMART_AUDIO: SmartAudioLabel[] = [
  {
    key: 'dialogue',
    label: 'videoPlayer.smartDialogue',
    hint: 'videoPlayer.smartDialogueHint',
    short: 'videoPlayer.smartDialogueShort',
    icon: Speech,
  },
  {
    key: 'night',
    label: 'videoPlayer.smartNight',
    hint: 'videoPlayer.smartNightHint',
    short: 'videoPlayer.smartNightShort',
    icon: Moon,
  },
  {
    key: 'normalize',
    label: 'videoPlayer.smartNormalize',
    hint: 'videoPlayer.smartNormalizeHint',
    short: 'videoPlayer.smartNormalizeShort',
    icon: ChartNoAxesColumn,
  },
  {
    key: 'leveling',
    label: 'videoPlayer.smartLeveling',
    hint: 'videoPlayer.smartLevelingHint',
    short: 'videoPlayer.smartLevelingShort',
    icon: AudioWaveform,
  },
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
