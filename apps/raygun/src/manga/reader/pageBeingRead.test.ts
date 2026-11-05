import { pageBeingRead } from './pageBeingRead';

/** Pages `height` px tall, one under another, the strip scrolled `scrolled` px. */
const strip = (height: number, scrolled: number) => (index: number) => ({
  top: index * height - scrolled,
  height,
});

describe('The page being read in the strip', () => {
  it('is the first at the very top, even when the reading line crosses a later short page', () => {
    // A third of an 800 px screen is 267 px: the line crosses the third 120 px page.
    expect(pageBeingRead(2, strip(120, 0), 0)).toBe(0);
  });

  it('is a page just jumped to, its top at the top of the screen', () => {
    expect(pageBeingRead(6, strip(120, 4 * 120), 0)).toBe(4);
    // Scrolled there a fraction short.
    expect(pageBeingRead(6, strip(120, 4 * 120 + 0.5), 0)).toBe(4);
  });

  it('moves on once the earlier pages have left the top', () => {
    expect(pageBeingRead(6, strip(120, 4 * 120 + 30), 0)).toBe(5);
  });

  it('is the page crossing the line when pages are taller than the screen', () => {
    expect(pageBeingRead(3, strip(1200, 3 * 1200 - 300), 0)).toBe(3);
    expect(pageBeingRead(0, strip(1200, 0), 0)).toBe(0);
  });

  it('measures from the scroller under a bar, and skips pages not laid out', () => {
    expect(pageBeingRead(2, (index) => ({ top: 56 + index * 120, height: 120 }), 56)).toBe(0);
    expect(pageBeingRead(2, () => ({ top: 0, height: 0 }), 0)).toBe(2);
    expect(pageBeingRead(2, () => null, 0)).toBe(2);
  });
});
