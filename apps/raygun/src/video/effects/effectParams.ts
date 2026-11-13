import type { PictureEffects, PictureLook, SmartAudio } from '@raygun/bindings';

/**
 * The video player's effects as numbers (`raygun_model::PictureEffects`, `SmartAudio`): how each
 * setting maps onto the shared shaders' uniforms (`glsl/fx_*.glsl`). Android's player maps them
 * the same way (`PlayerEffectsMath.kt`); the tests of both check the same numbers.
 */

/** Every field present: Rust always sends them all, but the generated types mark them optional. */
export type Effects = Required<PictureEffects>;
export type Audio = Required<SmartAudio>;

export const NO_EFFECTS: Effects = {
  brightness: 0,
  contrast: 0,
  saturation: 0,
  gamma: 0,
  hue: 0,
  sharpen: 0,
  look: 'none',
  deflicker: 0,
  denoise: 0,
  smoothMotion: false,
};

export const NO_SMART_AUDIO: Audio = {
  dialogue: false,
  night: false,
  normalize: false,
  leveling: false,
};

export const withEffectDefaults = (effects?: PictureEffects | null): Effects => ({
  ...NO_EFFECTS,
  ...effects,
});

export const withAudioDefaults = (audio?: SmartAudio | null): Audio => ({
  ...NO_SMART_AUDIO,
  ...audio,
});

/** The signed and strength settings, in the order the panels list them. */
export type AdjustKey = Exclude<keyof Effects, 'look' | 'smoothMotion'>;

export type Adjustment = { key: AdjustKey; min: number; max: number };

export const ADJUSTMENTS: Adjustment[] = [
  { key: 'brightness', min: -100, max: 100 },
  { key: 'contrast', min: -100, max: 100 },
  { key: 'saturation', min: -100, max: 100 },
  { key: 'gamma', min: -100, max: 100 },
  { key: 'hue', min: -180, max: 180 },
  { key: 'sharpen', min: 0, max: 100 },
  { key: 'deflicker', min: 0, max: 100 },
  { key: 'denoise', min: 0, max: 100 },
];

export const LOOKS: PictureLook[] = ['none', 'warm', 'cool', 'vivid', 'grayscale', 'sepia'];

export const isNeutral = (effects: Effects) =>
  (Object.keys(NO_EFFECTS) as (keyof Effects)[]).every((key) => effects[key] === NO_EFFECTS[key]);

export const sameEffects = (left: Effects, right: Effects) =>
  (Object.keys(NO_EFFECTS) as (keyof Effects)[]).every((key) => left[key] === right[key]);

/** The picture's own adjustments, smooth motion left out (presets set only these). */
export const adjustmentsOf = (effects: Effects): Effects => ({ ...effects, smoothMotion: false });

export const isAudioOff = (audio: Audio) =>
  !audio.dialogue && !audio.night && !audio.normalize && !audio.leveling;

/** The parts that take passes of their own or a frame's history, heaviest first: what a slow
 * computer drops first. */
export type HeavyKind = 'smoothMotion' | 'denoise' | 'deflicker' | 'sharpen';

/** Whether `kind` is on in `effects`. */
export const isOn = (effects: Effects, kind: HeavyKind) =>
  kind === 'smoothMotion' ? effects.smoothMotion : effects[kind] > 0;

export const heavyKinds = (effects: Effects): HeavyKind[] =>
  (['smoothMotion', 'denoise', 'deflicker', 'sharpen'] as const).filter((kind) =>
    isOn(effects, kind),
  );

export const without = (effects: Effects, kinds: Iterable<HeavyKind>): Effects => {
  const result = { ...effects };
  for (const kind of kinds) {
    if (kind === 'smoothMotion') result.smoothMotion = false;
    else result[kind] = 0;
  }
  return result;
};

/** One-tap starting points (the same as `EffectMath.PRESETS` on Android). */
export type PresetId =
  'presetOff' | 'presetOldFilm' | 'presetCleanUp' | 'presetBrighter' | 'presetCinema';

export const EFFECT_PRESETS: { id: PresetId; effects: Effects }[] = [
  { id: 'presetOff', effects: NO_EFFECTS },
  {
    id: 'presetOldFilm',
    effects: { ...NO_EFFECTS, contrast: 10, sharpen: 15, deflicker: 60, denoise: 35 },
  },
  { id: 'presetCleanUp', effects: { ...NO_EFFECTS, sharpen: 20, denoise: 45 } },
  { id: 'presetBrighter', effects: { ...NO_EFFECTS, brightness: 5, gamma: 30 } },
  { id: 'presetCinema', effects: { ...NO_EFFECTS, contrast: 15, saturation: 10, gamma: -10 } },
];

// ---- How the values map ----

/** Brightness adds up to a quarter of full scale either way. */
export const brightnessOffset = (value: number) => (value / 100) * 0.25;
/** Contrast halves (−100) or doubles (100) the distance from mid-grey. */
export const contrastFactor = (value: number) => 2 ** (value / 100);
/** Saturation from none (−100, grey) to twice (100). */
export const saturationFactor = (value: number) => 1 + value / 100;
/** Gamma's exponent: 0.5 at 100 (lighter midtones) to 2 at −100. */
export const gammaExponent = (value: number) => 2 ** (-value / 100);
export const sharpenAmount = (value: number) => (value / 100) * 1.6;
/** The deflicker's moving mean: a stronger setting follows each frame less (more frames). */
export const deflickerFollow = (value: number) => 0.4 - 0.35 * (value / 100);

/** An affine colour change: out = matrix · rgb + offset, the matrix row by row. */
export type ColourTransform = { matrix: number[]; offset: number[] };

const IDENTITY: ColourTransform = { matrix: [1, 0, 0, 0, 1, 0, 0, 0, 1], offset: [0, 0, 0] };

const at = (values: number[], index: number) => values[index] ?? 0;

/** `second` after `first`. */
export const compose = (second: ColourTransform, first: ColourTransform): ColourTransform => {
  const matrix: number[] = [];
  for (let row = 0; row < 3; row += 1) {
    for (let column = 0; column < 3; column += 1) {
      let sum = 0;
      for (let k = 0; k < 3; k += 1) {
        sum += at(second.matrix, row * 3 + k) * at(first.matrix, k * 3 + column);
      }
      matrix.push(sum);
    }
  }
  const offset = [0, 1, 2].map((row) => {
    let sum = at(second.offset, row);
    for (let k = 0; k < 3; k += 1) sum += at(second.matrix, row * 3 + k) * at(first.offset, k);
    return sum;
  });
  return { matrix, offset };
};

const scale = (red: number, green: number, blue: number): ColourTransform => ({
  matrix: [red, 0, 0, 0, green, 0, 0, 0, blue],
  offset: [0, 0, 0],
});

/** CSS's `saturate()` matrix. */
export const saturation = (s: number): ColourTransform => ({
  matrix: [
    0.213 + 0.787 * s,
    0.715 - 0.715 * s,
    0.072 - 0.072 * s,
    0.213 - 0.213 * s,
    0.715 + 0.285 * s,
    0.072 - 0.072 * s,
    0.213 - 0.213 * s,
    0.715 - 0.715 * s,
    0.072 + 0.928 * s,
  ],
  offset: [0, 0, 0],
});

/** CSS's `hue-rotate()` matrix. */
export const hueRotation = (degrees: number): ColourTransform => {
  if (degrees === 0) return IDENTITY;
  const angle = (degrees * Math.PI) / 180;
  const c = Math.cos(angle);
  const s = Math.sin(angle);
  return {
    matrix: [
      0.213 + c * 0.787 - s * 0.213,
      0.715 - c * 0.715 - s * 0.715,
      0.072 - c * 0.072 + s * 0.928,
      0.213 - c * 0.213 + s * 0.143,
      0.715 + c * 0.285 + s * 0.14,
      0.072 - c * 0.072 - s * 0.283,
      0.213 - c * 0.213 - s * 0.787,
      0.715 - c * 0.715 + s * 0.715,
      0.072 + c * 0.928 + s * 0.072,
    ],
    offset: [0, 0, 0],
  };
};

/** Contrast round mid-grey. */
export const contrast = (factor: number): ColourTransform => ({
  matrix: [factor, 0, 0, 0, factor, 0, 0, 0, factor],
  offset: [0, 1, 2].map(() => (1 - factor) * 0.5),
});

const LUMA = [0.2126, 0.7152, 0.0722];

export const look = (name: PictureLook): ColourTransform => {
  switch (name) {
    case 'warm':
      return scale(1.07, 1, 0.89);
    case 'cool':
      return scale(0.92, 1, 1.09);
    case 'vivid':
      return compose(contrast(1.08), saturation(1.35));
    case 'grayscale':
      return { matrix: [...LUMA, ...LUMA, ...LUMA], offset: [0, 0, 0] };
    case 'sepia':
      // CSS's `sepia(1)`.
      return {
        matrix: [0.393, 0.769, 0.189, 0.349, 0.686, 0.168, 0.272, 0.534, 0.131],
        offset: [0, 0, 0],
      };
    default:
      return IDENTITY;
  }
};

/** The colour part, in order: the look, saturation, hue, contrast, then brightness. */
export const colourTransform = (effects: Effects): ColourTransform => {
  const chain = [
    hueRotation(effects.hue),
    saturation(saturationFactor(effects.saturation)),
    look(effects.look),
  ].reduce(compose, contrast(contrastFactor(effects.contrast)));
  const lift = brightnessOffset(effects.brightness);
  return { matrix: chain.matrix, offset: chain.offset.map((value) => value + lift) };
};

/** The matrix column by column, as GLSL's `mat3` takes it. */
export const columns = (transform: ColourTransform) =>
  new Float32Array(
    Array.from({ length: 9 }, (_, index) =>
      at(transform.matrix, (index % 3) * 3 + Math.floor(index / 3)),
    ),
  );

/** The shaders' uniforms for a set of effects. */
export type ShaderParams = {
  colour: ColourTransform;
  /** The gamma exponent; 1 leaves it. */
  gamma: number;
  /** The unsharp mask's amount; 0 leaves it. */
  sharpen: number;
  deflicker: boolean;
  /** How much of each frame the steady light takes. */
  follow: number;
  denoise: boolean;
  spatial: number;
  spatialSigma: number;
  temporal: number;
  temporalSigma: number;
};

export const shaderParams = (effects: Effects): ShaderParams => {
  const noise = effects.denoise / 100;
  return {
    colour: colourTransform(effects),
    gamma: gammaExponent(effects.gamma),
    sharpen: sharpenAmount(effects.sharpen),
    deflicker: effects.deflicker > 0,
    follow: deflickerFollow(effects.deflicker),
    denoise: effects.denoise > 0,
    spatial: 0.15 + 0.6 * noise,
    spatialSigma: 0.02 + 0.05 * noise,
    temporal: 0.35 + 0.4 * noise,
    temporalSigma: 0.02 + 0.04 * noise,
  };
};

/** How a value reads beside its name: "+20", "−20", "40", "+30°". */
export const shownValue = (key: AdjustKey, value: number) => {
  const signed = ADJUSTMENTS.find((adjustment) => adjustment.key === key)?.min !== 0;
  const text = !signed || value === 0 ? String(value) : value > 0 ? `+${value}` : `−${-value}`;
  return key === 'hue' ? `${text}°` : text;
};
