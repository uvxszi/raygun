/**
 * Pages Raygun draws itself (a PDF's, at the proxy's `/pdf/` route) are asked for as wide as
 * they're shown: the screen's pixels, more once zoomed in, rounded up to one of a few widths (the
 * proxy keeps one copy per width), so zooming draws a sharper page that replaces the shown one
 * once it's ready.
 */
const WIDTHS = [480, 720, 960, 1200, 1440, 1680, 1920, 2400, 2880, 3360, 3840, 4096] as const;
const WIDEST = 4096;

/** Whether `url` is a page Raygun draws. */
export const isDrawnPage = (url: string) => /^https?:\/\/[^/]+\/pdf\//.test(url);

/** The width to draw a page shown `cssWidth` CSS pixels wide on a screen of `pixelRatio`. */
export const drawnWidth = (cssWidth: number, pixelRatio: number, zoom: number) => {
  const wanted = Math.max(cssWidth, 1) * Math.max(pixelRatio, 1) * Math.max(zoom, 1);
  return WIDTHS.find((width) => width >= wanted) ?? WIDEST;
};

/** `url` asked for at `width` when it's a drawn page; other pages as they are. */
export const sizedPage = (url: string, width: number) =>
  isDrawnPage(url) ? `${url}${url.includes('?') ? '&' : '?'}w=${width}` : url;

/** A page's address without the width it's drawn at: the same page at any width. */
export const pageIdentity = (url: string) =>
  isDrawnPage(url) ? url.replace(/[?&]w=\d+(?=&|$)/, '') : url;
