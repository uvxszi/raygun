import { dragTarget } from './seekDrag';

describe('dragTarget', () => {
  it('moves from where the video was by the share of the bar dragged', () => {
    expect(dragTarget(60_000, 100, 1_000, 600_000)).toBe(120_000);
    expect(dragTarget(60_000, -50, 1_000, 600_000)).toBe(30_000);
  });

  it('stays within the video', () => {
    expect(dragTarget(10_000, -500, 1_000, 600_000)).toBe(0);
    expect(dragTarget(590_000, 500, 1_000, 600_000)).toBe(600_000);
  });

  it('is the start for a video of no known length', () => {
    expect(dragTarget(10_000, 100, 1_000, 0)).toBe(0);
  });
});
