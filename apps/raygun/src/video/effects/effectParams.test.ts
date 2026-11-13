import {
  colourTransform,
  columns,
  EFFECT_PRESETS,
  gammaExponent,
  heavyKinds,
  isAudioOff,
  isNeutral,
  NO_EFFECTS,
  NO_SMART_AUDIO,
  sameEffects,
  shaderParams,
  shownValue,
  withEffectDefaults,
  without,
} from './effectParams';

const close = (actual: number[], expected: number[]) =>
  expected.forEach((value, index) => expect(actual[index]).toBeCloseTo(value, 5));

describe('picture effects as numbers', () => {
  it('leave the picture as it is until changed', () => {
    expect(isNeutral(NO_EFFECTS)).toBe(true);
    expect(isNeutral(withEffectDefaults({ look: 'sepia' }))).toBe(false);
    expect(isAudioOff(NO_SMART_AUDIO)).toBe(true);
    const shader = shaderParams(NO_EFFECTS);
    close(shader.colour.matrix, [1, 0, 0, 0, 1, 0, 0, 0, 1]);
    close(shader.colour.offset, [0, 0, 0]);
    expect(shader).toMatchObject({ gamma: 1, sharpen: 0, deflicker: false, denoise: false });
  });

  it('map light, contrast and saturation as on Android', () => {
    const both = colourTransform(withEffectDefaults({ contrast: 100, brightness: 100 }));
    close(both.matrix, [2, 0, 0, 0, 2, 0, 0, 0, 2]);
    close(both.offset, [-0.25, -0.25, -0.25]);
    const grey = colourTransform(withEffectDefaults({ saturation: -100 })).matrix;
    close(grey.slice(0, 3), [0.213, 0.715, 0.072]);
    close(grey.slice(6, 9), grey.slice(0, 3));
    expect(gammaExponent(100)).toBeCloseTo(0.5);
    expect(gammaExponent(-100)).toBeCloseTo(2);
  });

  // The same numbers as PlayerEffectsTest.kt, so both players draw alike.
  it('compose a mix in order: look, saturation, hue, contrast, brightness', () => {
    const warm = colourTransform(
      withEffectDefaults({ brightness: 10, contrast: 50, saturation: 20, hue: 30, look: 'warm' }),
    );
    close(
      warm.matrix,
      [1.36654, -0.646366, 0.697261, 0.117189, 1.548819, -0.217274, -0.727182, 0.567029, 1.358846],
    );
    close(warm.offset, [-0.182107, -0.182107, -0.182107]);
    const sepia = colourTransform(withEffectDefaults({ look: 'sepia', gamma: 40, contrast: -30 }));
    close(
      sepia.matrix,
      [0.319215, 0.624622, 0.153516, 0.283476, 0.557205, 0.136458, 0.220933, 0.433743, 0.106405],
    );
    close(sepia.offset, [0.093874, 0.093874, 0.093874]);
    const shader = shaderParams(
      withEffectDefaults({ gamma: 40, deflicker: 60, denoise: 35, sharpen: 15 }),
    );
    expect(shader.gamma).toBeCloseTo(0.757858, 5);
    expect(shader.sharpen).toBeCloseTo(0.24);
    expect(shader.follow).toBeCloseTo(0.19);
    expect(shader.spatial).toBeCloseTo(0.36);
    expect(shader.spatialSigma).toBeCloseTo(0.0375);
    expect(shader.temporal).toBeCloseTo(0.49);
    expect(shader.temporalSigma).toBeCloseTo(0.034);
  });

  it("hands GLSL's mat3 its columns", () => {
    expect(Array.from(columns({ matrix: [1, 2, 3, 4, 5, 6, 7, 8, 9], offset: [0, 0, 0] }))).toEqual(
      [1, 4, 7, 2, 5, 8, 3, 6, 9],
    );
  });

  it('drops the heaviest effects first and reads values with their signs', () => {
    const heavy = withEffectDefaults({ sharpen: 5, deflicker: 5, denoise: 5, contrast: 10 });
    expect(heavyKinds(heavy)).toEqual(['denoise', 'deflicker', 'sharpen']);
    expect(without(heavy, ['denoise'])).toEqual({ ...heavy, denoise: 0 });
    expect(shownValue('contrast', 20)).toBe('+20');
    expect(shownValue('gamma', -15)).toBe('−15');
    expect(shownValue('hue', 30)).toBe('+30°');
    expect(shownValue('denoise', 40)).toBe('40');
  });

  it('offers presets, the first of them none', () => {
    expect(EFFECT_PRESETS[0]?.id).toBe('presetOff');
    expect(isNeutral(EFFECT_PRESETS[0]!.effects)).toBe(true);
    const oldFilm = EFFECT_PRESETS.find((preset) => preset.id === 'presetOldFilm')!.effects;
    expect(heavyKinds(oldFilm)).toEqual(['denoise', 'deflicker', 'sharpen']);
    expect(sameEffects(oldFilm, { ...oldFilm })).toBe(true);
  });
});
