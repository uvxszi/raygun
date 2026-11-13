import { shaderParams, withEffectDefaults } from './effectParams';
import { canvasWith, fakeGl } from './fakeGl';
import { EffectsRenderer, PASSES, VERTEX_SOURCE } from './glRenderer';

const video = (width = 320, height = 180) => {
  const element = document.createElement('video');
  Object.defineProperty(element, 'videoWidth', { value: width });
  Object.defineProperty(element, 'videoHeight', { value: height });
  return element;
};

describe('the effect passes', () => {
  it('put the shared part in front of each pass (GLSL ES 1.00, as on Android)', () => {
    for (const source of Object.values(PASSES)) {
      expect(source.startsWith('// Put in front of each effect pass')).toBe(true);
      expect(source).toContain('precision highp float;');
      expect(source).toContain('void main()');
      expect(source).not.toMatch(/#version/);
    }
    expect(VERTEX_SOURCE).toContain('attribute vec4 aFramePosition;');
    expect(PASSES.final).toContain('uniform mat3 uColour;');
    expect(PASSES.clean).toContain('uniform sampler2D uHistory;');
  });

  it('draws colours alone in one pass onto the canvas', () => {
    const { gl, draws } = fakeGl();
    const renderer = new EffectsRenderer(canvasWith(gl));
    const params = shaderParams(withEffectDefaults({ contrast: 100, gamma: 40 }));
    renderer.draw(video(), params, 0.04);
    expect(draws.map((draw) => draw.pass)).toEqual(['final']);
    const [final] = draws;
    expect(final?.into).toBeNull();
    expect(final?.viewport).toEqual([0, 0, 320, 180]);
    expect(final?.uniforms.uColour).toEqual([2, 0, 0, 0, 2, 0, 0, 0, 2]);
    expect(final?.uniforms.uOffset).toEqual([-0.5, -0.5, -0.5]);
    expect(final?.uniforms.uDeflicker).toBe(0);
    expect(final?.uniforms.uGamma).toBeCloseTo(0.7579, 3);
  });

  it('measures the light, denoises against the frame before, then draws the look', () => {
    const { gl, draws } = fakeGl();
    const renderer = new EffectsRenderer(canvasWith(gl));
    const params = shaderParams(withEffectDefaults({ deflicker: 50, denoise: 50 }));
    const element = video();

    renderer.draw(element, params, 1);
    expect(draws.map((draw) => draw.pass)).toEqual(['blocks', 'mean', 'clean', 'final']);
    const [blocks, mean, first, final] = draws;
    expect(blocks?.viewport).toEqual([0, 0, 16, 9]);
    expect(mean?.viewport).toEqual([0, 0, 1, 1]);
    // No history yet: the light starts from this frame, nothing leans on a frame before.
    expect(mean?.uniforms.uFresh).toBe(1);
    expect(first?.uniforms.uTemporal).toBe(0);
    expect(first?.uniforms.uDeflicker).toBe(1);
    // Denoising applied the gain already; the final pass reads its frame.
    expect(final?.uniforms.uDeflicker).toBe(0);
    expect(final?.textures.uFrame).toBe(first?.into);

    draws.length = 0;
    renderer.draw(element, params, 1.04);
    const second = draws.find((draw) => draw.pass === 'clean');
    expect(draws.find((draw) => draw.pass === 'mean')?.uniforms.uFresh).toBe(0);
    expect(second?.uniforms.uTemporal).toBeCloseTo(params.temporal);
    // The frame before is the history; this one goes into the other target.
    expect(second?.textures.uHistory).toBe(first?.into);
    expect(second?.into).not.toBe(first?.into);

    // The same frame again (paused, a slider moved) goes from the same history.
    draws.length = 0;
    renderer.draw(element, params, 1.04);
    const again = draws.find((draw) => draw.pass === 'clean');
    expect(again?.into).toBe(second?.into);
    expect(again?.textures.uHistory).toBe(first?.into);

    // After a seek, nothing before counts.
    renderer.restart();
    draws.length = 0;
    renderer.draw(element, params, 30);
    expect(draws.find((draw) => draw.pass === 'clean')?.uniforms.uTemporal).toBe(0);
    expect(draws.find((draw) => draw.pass === 'mean')?.uniforms.uFresh).toBe(1);
  });

  it('says so when there is no WebGL 2', () => {
    expect(() => new EffectsRenderer(canvasWith(null))).toThrow('No WebGL 2');
  });
});
