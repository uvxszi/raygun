/**
 * The page being read: the one crossing the reading line, or an earlier page whose top is still
 * on screen. A page shorter than a third of the screen (a wide strip, a spread, a title page)
 * never reaches the line at the very top, so the line alone would call the first page the second,
 * and a page just jumped to (its top at the screen's top) the one after it. `boxOf` gives a
 * page's box on screen; a page without a laid-out box (height 0) isn't judged.
 */
export const pageBeingRead = (
  crossing: number,
  boxOf: (index: number) => { top: number; height: number } | null,
  screenTop: number,
): number => {
  let page = crossing;
  while (page > 0) {
    const before = boxOf(page - 1);
    // A pixel's slack: a page scrolled to the top may sit a fraction above it.
    if (!before || before.height <= 0 || before.top < screenTop - 1) break;
    page -= 1;
  }
  return page;
};
