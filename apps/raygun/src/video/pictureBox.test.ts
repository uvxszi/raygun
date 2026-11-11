import { pictureBox, subtitleFontPx } from './pictureBox';
import { VIGNETTE, easedOut } from './vignette';

describe('where the picture shows', () => {
  it('fits a wide film into a tall window by the film, not the window', () => {
    // A 2.4:1 film in a portrait window.
    const box = pictureBox(1000, 2000, 1920, 800, 'fit');
    expect(box.width).toBe(1000);
    expect(box.height).toBeCloseTo(416.67, 1);
    expect(box.top).toBeCloseTo(791.67, 1);
    // Subtitles are a share of the picture's height, the size setting a factor of it.
    expect(subtitleFontPx(box, 100)).toBeCloseTo(416.67 * 0.046, 1);
    expect(subtitleFontPx(box, 150)).toBeCloseTo(416.67 * 0.046 * 1.5, 1);
    expect(subtitleFontPx(box, 100)).toBeLessThan(2000 * 0.046 * 0.25);
  });

  it('fits a tall picture into a wide window by its height', () => {
    const box = pictureBox(1600, 900, 1080, 1920, 'fit');
    expect(box.height).toBe(900);
    expect(box.width).toBeCloseTo(506.25, 1);
    expect(box.left).toBeCloseTo(546.88, 1);
  });

  it('covers the whole player when cropped or stretched', () => {
    for (const aspect of ['crop', 'stretch'] as const) {
      expect(pictureBox(1280, 720, 640, 480, aspect)).toEqual({
        left: 0,
        top: 0,
        width: 1280,
        height: 720,
        stageWidth: 1280,
        stageHeight: 720,
      });
    }
  });

  it('takes a 16:9 picture before the video says its size', () => {
    const box = pictureBox(900, 1600, 0, 0, 'crop');
    expect(box.width).toBe(900);
    expect(box.height).toBeCloseTo(506.25, 1);
  });
});

describe('the shade behind the controls', () => {
  it('fades out flat, with no line where it stops', () => {
    expect(easedOut(0)).toBe(1);
    expect(easedOut(1)).toBe(0);
    // Flat at both ends: barely changes over the last and first hundredth.
    expect(easedOut(0.99)).toBeLessThan(0.001);
    expect(1 - easedOut(0.01)).toBeLessThan(0.001);
    expect(easedOut(0.5)).toBeCloseTo(0.5);
  });

  it('is sized to the player in any shape', () => {
    expect(VIGNETTE).toMatch(/^linear-gradient\(to bottom, rgba\(0, 0, 0, 0\.6\) 0%/);
    expect(VIGNETTE).toContain('rgba(0, 0, 0, 0) 28%');
    expect(VIGNETTE).toContain('linear-gradient(to top, rgba(0, 0, 0, 0.7) 0%');
    expect(VIGNETTE).toContain('rgba(0, 0, 0, 0) 36%');
    expect(VIGNETTE).toContain('radial-gradient(ellipse farthest-corner at 50% 50%');
    expect(VIGNETTE).toContain('rgba(0, 0, 0, 0.25) 100%');
  });
});
