import { stripLead, stripWidth } from '../readerDisplay';
import { pageLayout } from './pageLayout';
import { anchoredScroll, clampScroll, contentPoint } from './zoomMath';
import type { Point } from './zoomMath';

/**
 * A page `size` big at zoom 1 in a scroller `screen` big, laid out as the reader does: in the
 * middle where it's smaller than the screen, from the top left where it's bigger.
 */
const layout = (
  screen: { width: number; height: number },
  size: { width: number; height: number },
  zoom: number,
) => {
  const width = size.width * zoom;
  const height = size.height * zoom;
  const offset = {
    x: Math.max(0, (screen.width - width) / 2),
    y: Math.max(0, (screen.height - height) / 2),
  };
  const max = { x: width - screen.width, y: height - screen.height };
  /** The page's top left on screen at `scroll`. */
  const box = (scroll: Point) => ({ x: offset.x - scroll.x, y: offset.y - scroll.y });
  return { box, max };
};

/** Zooms from `from` to `to` at `at`, as the reader does: where the tapped point ends up. */
const zoomed = (
  screen: { width: number; height: number },
  size: { width: number; height: number },
  { from, to, scroll, at }: { from: number; to: number; scroll: Point; at: Point },
) => {
  const before = layout(screen, size, from);
  const point = contentPoint(before.box(scroll), at, from);
  const after = layout(screen, size, to);
  // The browser keeps the scroll within the new range on its own before the reader moves it.
  const kept = { x: clampScroll(scroll.x, after.max.x), y: clampScroll(scroll.y, after.max.y) };
  const next = anchoredScroll({
    scroll: kept,
    box: after.box(kept),
    point,
    zoom: to,
    at,
    max: after.max,
  });
  const box = after.box(next);
  return { scroll: next, shown: { x: box.x + point.x * to, y: box.y + point.y * to } };
};

const PHONE = { width: 400, height: 800 };

describe('the reader’s zoom', () => {
  it('keeps the tapped point under the finger', () => {
    // A tall page, partly read, zoomed in twice at a point near its lower right.
    const page = { width: 400, height: 1000 };
    const result = zoomed(PHONE, page, {
      from: 1,
      to: 2,
      scroll: { x: 0, y: 100 },
      at: { x: 300, y: 700 },
    });
    expect(result.shown).toEqual({ x: 300, y: 700 });
    expect(result.scroll).toEqual({ x: 300, y: 900 });
    // And back out at the same point: it stays there too while the page allows.
    const back = zoomed(PHONE, page, {
      from: 2,
      to: 1.5,
      scroll: result.scroll,
      at: { x: 300, y: 700 },
    });
    expect(back.shown).toEqual({ x: 300, y: 700 });
  });

  it('keeps a point within the page’s edges, not past them', () => {
    const page = { width: 400, height: 1000 };
    // Near the top left: the page can't scroll before its start.
    const corner = zoomed(PHONE, page, {
      from: 1,
      to: 2,
      scroll: { x: 0, y: 0 },
      at: { x: 0, y: 0 },
    });
    expect(corner.scroll).toEqual({ x: 0, y: 0 });
    // Zoomed out at the right edge: the page fits sideways again and stops at its left.
    const out = zoomed(PHONE, page, {
      from: 2,
      to: 1,
      scroll: { x: 400, y: 600 },
      at: { x: 390, y: 400 },
    });
    expect(out.scroll.x).toBe(0);
    expect(out.scroll.y).toBeGreaterThanOrEqual(0);
    expect(out.scroll.y).toBeLessThanOrEqual(1000 - 800);
  });

  it('holds a short wide strip in the middle of the screen, scrolling it only sideways', () => {
    // A strip three times as wide as tall fits the phone's width: 400 × 133, in the middle.
    const strip = { width: 400, height: 400 / 3 };
    const tapped = { x: 100, y: 430 };
    const result = zoomed(PHONE, strip, { from: 1, to: 2, scroll: { x: 0, y: 0 }, at: tapped });
    // Sideways the point stays under the finger; up and down the strip stays in the middle.
    expect(result.scroll).toEqual({ x: 100, y: 0 });
    expect(result.shown.x).toBe(100);
    const top = (800 - 800 / 3) / 2;
    expect(result.shown.y).toBeCloseTo(top + (tapped.y - (800 - 400 / 3) / 2) * 2);
  });

  it('keeps scrolls within the page', () => {
    expect(clampScroll(-20, 300)).toBe(0);
    expect(clampScroll(120, 300)).toBe(120);
    expect(clampScroll(480, 300)).toBe(300);
    // A page smaller than the screen doesn't scroll at all.
    expect(clampScroll(50, -100)).toBe(0);
  });

  it('measures the point zoomed at in the page’s own pixels', () => {
    expect(contentPoint({ x: -100, y: 50 }, { x: 100, y: 250 }, 2)).toEqual({ x: 100, y: 100 });
  });
});

describe('the zoomed page’s size', () => {
  it('scales a page evenly, from the fit the paged modes chose', () => {
    const shape = { ratio: 3, width: 1200 };
    const fit = 'min(1200px, 100cqw, 100cqh * 3)';
    expect(pageLayout('fitScreen', 1, shape).style.width).toBe(fit);
    expect(pageLayout('fitScreen', 2, shape).style.width).toBe(`calc(${fit} * 2)`);
    // Height follows the picture's own shape: its box is the picture, no bands around it.
    expect(pageLayout('fitScreen', 2, shape).className).toContain('h-auto');
    expect(pageLayout('fitWidth', 1.5).style.width).toBe('150cqw');
    expect(pageLayout('fitHeight', 2).style.height).toBe('200cqh');
    expect(pageLayout('original', 2).style.zoom).toBe(2);
  });

  it('gives the strip room above its first page in the screen’s own units', () => {
    expect(stripWidth(0, 1)).toBe('min(100cqw, 48rem)');
    expect(stripWidth(20, 2)).toBe('calc(min(100cqw, calc(48rem * 1.2)) * 2)');
    // Half of what the page leaves of the screen's height, never less than none.
    expect(stripLead(0, 2, 3)).toBe(
      'max(0px, calc((100cqh - calc(min(100cqw, 48rem) * 2) / 3) / 2))',
    );
  });
});
